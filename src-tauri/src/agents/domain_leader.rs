use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::agents::specialist::{self, DiffProposal, SpecialistTask};
use crate::agents::CodeBlock;
use crate::db::mutations::{self, CreateMutationInput};
use crate::db::tasks::{
    self, CreateTaskRecordInput, TaskStatus, UpdateTaskOutcomeInput, UpdateTaskStatusInput,
};
use crate::mcp_bridge::client::BridgeClient;
use crate::mcp_bridge::tool_caller::{self, ReadTargetFileInput, SearchTargetFilesInput};
use crate::model_intelligence::{self, ModelSelectionRequest};
use crate::model_registry::ModelRegistry;
use crate::task_runtime;
use crate::vector::search;
use crate::vector::ContextChunk;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecuteDomainTaskInput {
    pub task_id: String,
    pub target_project: String,
    pub top_k: Option<u32>,
    pub mcp_command: Option<String>,
    pub mcp_args: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictReport {
    pub agent_a: String,
    pub agent_b: String,
    pub semantic_distance: f32,
    pub description: String,
    pub requires_human_review: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntentSummary {
    pub task_id: String,
    pub domain: String,
    pub status: String,
    pub proposals: Vec<DiffProposal>,
    pub compliance_score: i64,
    pub tokens_spent: u32,
    pub summary: String,
    pub conflicts: Option<ConflictReport>,
}

pub async fn execute_domain_task(
    pool: &SqlitePool,
    bridge_client: &BridgeClient,
    model_registry: &ModelRegistry,
    input: ExecuteDomainTaskInput,
) -> Result<IntentSummary, String> {
    validate_input(&input)?;
    let tier2_model = model_intelligence::select_model(
        pool,
        model_registry,
        ModelSelectionRequest {
            task_id: Some(input.task_id.trim()),
            actor: "tier2_domain_leader",
            tier: 2,
            persona: None,
            skill: Some("domain_coordination"),
        },
    )
    .await?
    .selection;
    let task = tasks::get_task_by_id(pool, input.task_id.trim()).await?;

    if task.tier != 2 {
        return Err(format!(
            "Task '{}' is tier {}. execute_domain_task only supports tier 2 tasks.",
            task.id, task.tier
        ));
    }

    let stored_target_files: Vec<String> = task
        .target_files
        .as_deref()
        .and_then(|json| serde_json::from_str(json).ok())
        .unwrap_or_default();

    tasks::update_task_status(
        pool,
        UpdateTaskStatusInput {
            task_id: task.id.clone(),
            status: TaskStatus::Executing,
            error_message: None,
        },
    )
    .await?;
    task_runtime::record_task_activity(
        pool,
        "tier2_domain_leader",
        "tier2_execution_started",
        &task.id,
        &format!(
            "domain={} model={}/{} objective={} storedTargetFiles={}",
            task.domain, tier2_model.provider, tier2_model.model_id, task.objective,
            stored_target_files.len()
        ),
    )
    .await?;

    // Skip expensive vector search when we already have target files from the plan
    let (chunks, candidate_files) = if !stored_target_files.is_empty() {
        (Vec::new(), stored_target_files.clone())
    } else {
        let chunks = search::query_codebase(
            pool,
            &input.target_project,
            &task.objective,
            input.top_k.unwrap_or(8).max(3),
        )
        .await
        .unwrap_or_default();

        let candidate_files = collect_candidate_files(
            bridge_client,
            &input.target_project,
            &task.domain,
            &task.objective,
            &chunks,
            &input,
        )
        .await;
        (chunks, candidate_files)
    };
    task_runtime::record_task_activity(
        pool,
        "tier2_domain_leader",
        "tier2_context_ready",
        &task.id,
        &format!(
            "semanticChunks={} candidateFiles={} personas={} storedFiles={}",
            chunks.len(),
            candidate_files.len(),
            personas_for_domain(&task.domain).len(),
            stored_target_files.len()
        ),
    )
    .await?;

    let personas = personas_for_domain(&task.domain);
    let specialist_budgets = distribute_budget_for_specialists(task.token_budget, personas.len());
    let mut proposals: Vec<DiffProposal> = Vec::with_capacity(personas.len());
    let mut tokens_spent = 0_u32;

    for (idx, persona) in personas.iter().enumerate() {
        task_runtime::cooperative_checkpoint(
            pool,
            &task.id,
            "tier2_domain_leader",
            &format!("persona_{persona}_queue"),
        )
        .await?;
        task_runtime::ensure_budget_headroom(
            pool,
            &task.id,
            "tier2_domain_leader",
            &format!("persona_{persona}_budget_check"),
            specialist_budgets[idx],
        )
        .await?;

        let specialist_objective =
            build_specialist_objective(&task.domain, &task.objective, persona.as_str(), idx);
        // File selection priority:
        // 0. Stored target files from the LLM plan (persisted in DB)
        // 1. Explicit path mentioned in the objective (e.g., "modify src/lib/format.ts")
        // 2. For "create new" objectives: inferred path from keywords
        // 3. Best candidate from vector search (for "modify existing" objectives)
        // 4. Inferred path as final fallback
        let target_file = if !stored_target_files.is_empty() {
            stored_target_files[idx.min(stored_target_files.len() - 1)].clone()
        } else {
            extract_explicit_file_path(&specialist_objective)
                .or_else(|| {
                    if objective_implies_new_file(&specialist_objective) {
                        Some(infer_target_path_from_objective(&specialist_objective))
                    } else {
                        candidate_files.first().cloned()
                    }
                })
                .unwrap_or_else(|| infer_target_path_from_objective(&specialist_objective))
        };
        let code_context = hydrate_code_context(&chunks, &target_file, 2);
        let specialist_task_record = tasks::create_task_record(
            pool,
            CreateTaskRecordInput {
                parent_id: Some(task.id.clone()),
                tier: 3,
                domain: task.domain.clone(),
                objective: specialist_objective.clone(),
                token_budget: specialist_budgets[idx] as i64,
                risk_factor: task.risk_factor,
                status: TaskStatus::Pending,
                target_files: Some(
                    serde_json::to_string(&vec![target_file.clone()]).unwrap_or_default(),
                ),
            },
        )
        .await?;
        task_runtime::record_task_activity(
            pool,
            "tier2_domain_leader",
            "tier3_task_created",
            &specialist_task_record.id,
            &format!(
                "parent={} persona={} targetFile={} budget={}",
                task.id, persona, target_file, specialist_budgets[idx]
            ),
        )
        .await?;
        let specialist_model = model_intelligence::select_model(
            pool,
            model_registry,
            ModelSelectionRequest {
                task_id: Some(specialist_task_record.id.as_str()),
                actor: "tier2_domain_leader",
                tier: 3,
                persona: Some(persona.as_str()),
                skill: Some("specialist_assignment"),
            },
        )
        .await?
        .selection;

        let file_content = read_file_with_fallback(bridge_client, &input, &target_file).await;
        task_runtime::cooperative_checkpoint(
            pool,
            &specialist_task_record.id,
            "tier2_domain_leader",
            &format!("persona_{persona}_pre_execute"),
        )
        .await?;
        tasks::update_task_status(
            pool,
            UpdateTaskStatusInput {
                task_id: specialist_task_record.id.clone(),
                status: TaskStatus::Executing,
                error_message: None,
            },
        )
        .await?;
        task_runtime::record_task_activity(
            pool,
            &format!("tier3_{}", persona),
            "specialist_execution_started",
            &specialist_task_record.id,
            &format!(
                "model={}/{} objective={}",
                specialist_model.provider, specialist_model.model_id, specialist_objective
            ),
        )
        .await?;

        let specialist_task = SpecialistTask {
            task_id: specialist_task_record.id.clone(),
            parent_id: task.id.clone(),
            tier: 3,
            persona: persona.clone(),
            objective: specialist_objective,
            token_budget: specialist_budgets[idx],
            target_files: vec![target_file.clone()],
            code_context,
            constraints: {
                let mut constraints =
                    build_constraints_for_specialist(&task.domain, task.risk_factor as f32);
                constraints.push(format!(
                    "tier2_coordinator_model: {}/{}",
                    tier2_model.provider.as_str(),
                    tier2_model.model_id.as_str()
                ));
                constraints.push(format!(
                    "specialist_model: {}/{}",
                    specialist_model.provider.as_str(),
                    specialist_model.model_id.as_str()
                ));
                constraints
            },
            model_provider: Some(specialist_model.provider.clone()),
            model_id: Some(specialist_model.model_id.clone()),
        };

        let model_started_at = Instant::now();
        match specialist::run_specialist_task(&specialist_task, file_content.as_deref()) {
            Ok(proposal) => {
                model_intelligence::record_model_call_outcome(
                    pool,
                    specialist_model.provider.as_str(),
                    specialist_model.model_id.as_str(),
                    true,
                    Some(model_started_at.elapsed().as_millis() as i64),
                    None,
                    None,
                )
                .await;
                if let Err(error) = task_runtime::cooperative_checkpoint(
                    pool,
                    &task.id,
                    "tier2_domain_leader",
                    &format!("persona_{persona}_post_execute"),
                )
                .await
                {
                    tasks::update_task_status(
                        pool,
                        UpdateTaskStatusInput {
                            task_id: specialist_task_record.id.clone(),
                            status: TaskStatus::Failed,
                            error_message: Some(error),
                        },
                    )
                    .await?;
                    continue;
                }

                tokens_spent = tokens_spent.saturating_add(proposal.tokens_used);
                tasks::update_task_outcome(
                    pool,
                    UpdateTaskOutcomeInput {
                        task_id: task.id.clone(),
                        status: TaskStatus::Executing,
                        token_usage: Some(i64::from(tokens_spent)),
                        context_efficiency_ratio: None,
                        compliance_score: None,
                        checksum_before: None,
                        checksum_after: None,
                        error_message: None,
                    },
                )
                .await?;

                mutations::create_mutation(
                    pool,
                    CreateMutationInput {
                        task_id: task.id.clone(),
                        agent_uid: proposal.agent_uid.clone(),
                        file_path: proposal.file_path.clone(),
                        diff_content: proposal.diff_content.clone(),
                        intent_description: Some(proposal.intent_description.clone()),
                        intent_hash: Some(proposal.intent_hash.clone()),
                        confidence: proposal.confidence as f64,
                    },
                )
                .await?;

                tasks::update_task_outcome(
                    pool,
                    UpdateTaskOutcomeInput {
                        task_id: specialist_task_record.id,
                        status: TaskStatus::Completed,
                        token_usage: Some(i64::from(proposal.tokens_used)),
                        context_efficiency_ratio: None,
                        compliance_score: None,
                        checksum_before: None,
                        checksum_after: None,
                        error_message: None,
                    },
                )
                .await?;
                task_runtime::record_task_activity(
                    pool,
                    &format!("tier3_{}", persona),
                    "specialist_proposal_persisted",
                    &task.id,
                    &format!(
                        "agentUid={} file={} confidence={:.2} tokensUsed={}",
                        proposal.agent_uid,
                        proposal.file_path,
                        proposal.confidence,
                        proposal.tokens_used
                    ),
                )
                .await?;

                proposals.push(proposal);
            }
            Err(error) => {
                model_intelligence::record_model_call_outcome(
                    pool,
                    specialist_model.provider.as_str(),
                    specialist_model.model_id.as_str(),
                    false,
                    Some(model_started_at.elapsed().as_millis() as i64),
                    None,
                    Some(error.clone()),
                )
                .await;
                let specialist_task_id = specialist_task_record.id.clone();
                tasks::update_task_outcome(
                    pool,
                    UpdateTaskOutcomeInput {
                        task_id: specialist_task_id.clone(),
                        status: TaskStatus::Failed,
                        token_usage: Some(0),
                        context_efficiency_ratio: None,
                        compliance_score: None,
                        checksum_before: None,
                        checksum_after: None,
                        error_message: Some(error.clone()),
                    },
                )
                .await?;
                task_runtime::record_task_activity(
                    pool,
                    &format!("tier3_{}", persona),
                    "specialist_execution_failed",
                    &specialist_task_id,
                    &format!(
                        "model={}/{} file={} error: {}",
                        specialist_model.provider,
                        specialist_model.model_id,
                        target_file,
                        error
                    ),
                )
                .await?;
            }
        }
    }

    let conflict = detect_conflict(&proposals);
    let status = summarize_status(&proposals, conflict.as_ref());
    let compliance_score =
        compute_compliance_score(&task.domain, task.risk_factor as f32, &proposals, &status);
    let summary = build_summary(
        &task.domain,
        &status,
        &proposals,
        tokens_spent,
        conflict.as_ref(),
    );

    let (final_task_status, final_error_message) = if status == "ready_for_review" {
        (
            TaskStatus::Paused,
            Some(
                "ready_for_review: proposals generated; run mutation pipeline to apply code."
                    .to_string(),
            ),
        )
    } else if status == "blocked" {
        (
            TaskStatus::Failed,
            Some("blocked: no valid specialist proposals were produced.".to_string()),
        )
    } else {
        (
            TaskStatus::Paused,
            Some("consensus_failed: human review required before apply.".to_string()),
        )
    };

    tasks::update_task_outcome(
        pool,
        UpdateTaskOutcomeInput {
            task_id: task.id.clone(),
            status: final_task_status,
            token_usage: Some(i64::from(tokens_spent)),
            context_efficiency_ratio: None,
            compliance_score: Some(compliance_score),
            checksum_before: None,
            checksum_after: None,
            error_message: final_error_message,
        },
    )
    .await?;
    task_runtime::record_task_activity(
        pool,
        "tier2_domain_leader",
        "tier2_review_gate",
        &task.id,
        &format!(
            "status={} proposals={} complianceScore={} tokensSpent={}",
            status,
            proposals.len(),
            compliance_score,
            tokens_spent
        ),
    )
    .await?;

    Ok(IntentSummary {
        task_id: task.id,
        domain: task.domain,
        status,
        proposals,
        compliance_score,
        tokens_spent,
        summary,
        conflicts: conflict,
    })
}

fn validate_input(input: &ExecuteDomainTaskInput) -> Result<(), String> {
    if input.task_id.trim().is_empty() {
        return Err("taskId is required".to_string());
    }
    if input.target_project.trim().is_empty() {
        return Err("targetProject is required".to_string());
    }

    Ok(())
}

async fn collect_candidate_files(
    bridge_client: &BridgeClient,
    target_project: &str,
    domain: &str,
    objective: &str,
    chunks: &[ContextChunk],
    input: &ExecuteDomainTaskInput,
) -> Vec<String> {
    let mut ordered = Vec::new();
    let mut seen = HashSet::new();

    for chunk in chunks {
        if seen.insert(chunk.file_path.clone()) {
            ordered.push(chunk.file_path.clone());
        }
    }

    if ordered.len() >= 3 {
        return prioritize_candidate_files(ordered, domain, objective)
            .into_iter()
            .take(6)
            .collect();
    }

    let fallback_pattern = objective
        .split(|ch: char| !ch.is_alphanumeric() && ch != '_')
        .find(|part| part.len() >= 4)
        .unwrap_or("src");

    if let Ok(search_result) = tool_caller::search_files(
        bridge_client,
        SearchTargetFilesInput {
            target_project: target_project.to_string(),
            pattern: fallback_pattern.to_string(),
            limit: Some(8),
            mcp_command: input.mcp_command.clone(),
            mcp_args: input.mcp_args.clone(),
        },
    )
    .await
    {
        for item in search_result.matches {
            if seen.insert(item.path.clone()) {
                ordered.push(item.path);
            }
        }
    }

    prioritize_candidate_files(ordered, domain, objective)
        .into_iter()
        .take(6)
        .collect()
}

fn prioritize_candidate_files(
    mut files: Vec<String>,
    domain: &str,
    objective: &str,
) -> Vec<String> {
    if files.len() <= 1 {
        return files;
    }

    let objective_lower = objective.to_ascii_lowercase();
    let domain_lower = domain.to_ascii_lowercase();
    let is_frontend_focus = domain_lower == "frontend"
        || objective_contains_any(
            &objective_lower,
            &[
                "frontend",
                "react",
                "ui",
                "component",
                "view",
                "page",
                "tsx",
                "tailwind",
            ],
        );
    let explicit_tauri_or_rust = objective_contains_any(
        &objective_lower,
        &["tauri", "rust", "cargo", "src-tauri", ".rs"],
    );

    if !is_frontend_focus || explicit_tauri_or_rust {
        return files;
    }

    files.sort_by(|left, right| {
        score_frontend_path(right)
            .cmp(&score_frontend_path(left))
            .then_with(|| left.cmp(right))
    });
    files
}

fn objective_contains_any(value: &str, parts: &[&str]) -> bool {
    parts.iter().any(|part| value.contains(part))
}

fn score_frontend_path(path: &str) -> i64 {
    let lower = path.to_ascii_lowercase();
    let mut score = 0_i64;

    if lower.ends_with(".tsx") {
        score += 28;
    } else if lower.ends_with(".ts") || lower.ends_with(".jsx") || lower.ends_with(".js") {
        score += 18;
    } else if lower.ends_with(".css") {
        score += 12;
    }

    if lower.contains("/src/") {
        score += 12;
    }
    if lower.contains("/components/") || lower.contains("/views/") || lower.contains("/pages/") {
        score += 14;
    }
    if lower.contains("/hooks/") || lower.contains("/layouts/") || lower.contains("/store/") {
        score += 8;
    }

    if lower.contains("src-tauri/") {
        score -= 35;
    }
    if lower.ends_with(".rs") || lower.contains("cargo.toml") {
        score -= 45;
    }

    score
}

fn personas_for_domain(domain: &str) -> Vec<String> {
    // Use a single primary specialist per domain.
    // Multi-persona (3 specialists on different files) caused:
    // - Round-robin file assignment giving irrelevant files to some specialists
    // - 3x token cost for essentially the same objective
    // - Conflicting diffs on the same file or irrelevant diffs on wrong files
    // A single focused specialist produces one clean diff on the best file.
    match domain {
        "auth" => vec!["security_analyst".to_string()],
        "database" => vec!["database_optimizer".to_string()],
        "frontend" => vec!["react_specialist".to_string()],
        "api" => vec!["api_engineer".to_string()],
        _ => vec!["generalist".to_string()],
    }
}

fn distribute_budget_for_specialists(tier2_budget: i64, specialists: usize) -> Vec<u32> {
    if specialists == 0 {
        return Vec::new();
    }

    let available = ((tier2_budget.max(1) as f32) * 0.90).floor() as u32;
    let base = (available / specialists as u32).max(1);
    let mut budgets = vec![base; specialists];
    let mut assigned = budgets.iter().sum::<u32>();

    while assigned < available {
        for value in &mut budgets {
            if assigned >= available {
                break;
            }
            *value += 1;
            assigned += 1;
        }
    }

    budgets
}

fn build_specialist_objective(
    _domain: &str,
    parent_objective: &str,
    _persona: &str,
    _specialist_idx: usize,
) -> String {
    // Pass the user's actual objective directly to the specialist.
    // The persona hint is already included in the SpecialistTask struct
    // and referenced in the LLM prompt — prepending generic persona-based
    // instructions caused the LLM to ignore the real objective.
    parent_objective.trim().to_string()
}

/// Detect whether an objective is about creating something NEW (vs modifying existing code).
/// "Add a TypeScript utility function" → new file (no existing file to modify)
/// "Add error handling to the login form" → modify existing
/// "Create a React component for user settings" → new file
/// "Fix the bug in UserForm.tsx" → modify existing
fn objective_implies_new_file(objective: &str) -> bool {
    let lower = objective.to_ascii_lowercase();

    // Phrases that strongly indicate creating a new artifact
    let creation_patterns = [
        "add a ", "add an ", "add new ",
        "create a ", "create an ", "create new ",
        "implement a ", "implement an ", "implement new ",
        "write a ", "write an ", "write new ",
        "build a ", "build an ", "build new ",
        "add unit test", "add test", "write test", "create test",
    ];

    if !creation_patterns.iter().any(|p| lower.contains(p)) {
        return false;
    }

    // Exclude objectives that reference an EXISTING specific file to modify
    // "Add error handling to UserForm.tsx" → modifying existing
    let modification_signals = [" to the ", " in the ", " on the ", " from the "];
    let has_modification_target = modification_signals.iter().any(|s| lower.contains(s));

    // If there's a modification target AND no explicit new-file indicator, treat as modify
    if has_modification_target && !lower.contains("new ") {
        return false;
    }

    true
}

/// Extract an explicit file path mentioned in the objective text.
/// Objectives like "Add tests in `src/lib/__tests__/format.test.ts`" or
/// "modify src/components/Foo.tsx" contain the target path directly — this
/// is always more reliable than vector search which can return irrelevant files.
fn extract_explicit_file_path(objective: &str) -> Option<String> {
    let extensions = [
        ".ts", ".tsx", ".js", ".jsx", ".py", ".rs", ".css", ".json", ".md", ".vue", ".svelte",
    ];

    for word in objective.split_whitespace() {
        // Strip surrounding punctuation, backticks, quotes, parentheses
        let cleaned = word.trim_matches(|ch: char| {
            ch == '`' || ch == '\'' || ch == '"' || ch == '(' || ch == ')' || ch == ','
                || ch == ';' || ch == ':' || ch == '.' && !word.contains('/')
        });
        if cleaned.len() < 4 {
            continue;
        }

        let has_extension = extensions.iter().any(|ext| cleaned.ends_with(ext));
        let has_path_sep = cleaned.contains('/') || cleaned.contains('\\');

        if has_extension && has_path_sep {
            // Normalize backslashes to forward slashes
            return Some(cleaned.replace('\\', "/"));
        }
    }
    None
}

/// Infer a reasonable target file path when vector search finds no candidates.
/// This extracts keywords from the objective to build a plausible path like
/// `src/utils/format-tokens.ts` instead of the useless `src/main.ts` fallback.
fn infer_target_path_from_objective(objective: &str) -> String {
    let lower = objective.to_ascii_lowercase();

    // Extract meaningful words (skip common verbs and articles)
    let skip_words: HashSet<&str> = [
        "add", "create", "implement", "make", "build", "write", "update", "fix", "a", "an",
        "the", "that", "which", "with", "for", "from", "into", "to", "and", "or", "of", "in",
        "new", "function", "class", "method", "module", "file", "utility", "helper", "component",
        "unit", "test", "tests", "testing", "regression", "checks", "typescript", "react",
    ]
    .into_iter()
    .collect();

    let keywords: Vec<&str> = lower
        .split(|ch: char| !ch.is_alphanumeric() && ch != '_' && ch != '-')
        .filter(|word| word.len() >= 3 && !skip_words.contains(word))
        .take(3)
        .collect();

    let is_test = lower.contains("test") || lower.contains("spec");

    let filename = if keywords.is_empty() {
        "index".to_string()
    } else {
        let base = keywords.join("-");
        if is_test && !base.contains("test") {
            format!("{base}.test")
        } else {
            base
        }
    };

    // Determine extension from objective context
    let ext = if lower.contains("typescript") || lower.contains(".ts") {
        "ts"
    } else if lower.contains("react") || lower.contains("component") || lower.contains(".tsx") {
        "tsx"
    } else if lower.contains("python") || lower.contains(".py") {
        "py"
    } else if lower.contains("rust") || lower.contains(".rs") {
        "rs"
    } else {
        "ts"
    };

    // Determine directory from objective context
    let dir = if is_test {
        "src/__tests__"
    } else if lower.contains("util") || lower.contains("helper") {
        "src/utils"
    } else if lower.contains("component") || lower.contains("react") {
        "src/components"
    } else if lower.contains("hook") {
        "src/hooks"
    } else if lower.contains("test") || lower.contains("spec") {
        "src/__tests__"
    } else {
        "src"
    };

    format!("{dir}/{filename}.{ext}")
}

fn hydrate_code_context(
    chunks: &[ContextChunk],
    target_file: &str,
    limit: usize,
) -> Vec<CodeBlock> {
    let mut selected = chunks
        .iter()
        .filter(|chunk| chunk.file_path == target_file)
        .take(limit)
        .map(|chunk| CodeBlock {
            file_path: chunk.file_path.clone(),
            start_line: chunk.start_line,
            end_line: chunk.end_line,
            content: chunk.content.clone(),
            embedding: None,
        })
        .collect::<Vec<_>>();

    if selected.is_empty() {
        selected = chunks
            .iter()
            .take(limit)
            .map(|chunk| CodeBlock {
                file_path: chunk.file_path.clone(),
                start_line: chunk.start_line,
                end_line: chunk.end_line,
                content: chunk.content.clone(),
                embedding: None,
            })
            .collect();
    }

    selected
}

async fn read_file_with_fallback(
    bridge_client: &BridgeClient,
    input: &ExecuteDomainTaskInput,
    target_file: &str,
) -> Option<String> {
    if let Ok(result) = tool_caller::read_file(
        bridge_client,
        ReadTargetFileInput {
            target_project: input.target_project.clone(),
            file_path: target_file.to_string(),
            mcp_command: input.mcp_command.clone(),
            mcp_args: input.mcp_args.clone(),
        },
    )
    .await
    {
        return Some(result.content);
    }

    let root = PathBuf::from(input.target_project.trim());
    let path = to_local_path(&root, target_file);
    fs::read_to_string(path).ok()
}

fn to_local_path(root: &Path, relative_path: &str) -> PathBuf {
    relative_path
        .split('/')
        .filter(|part| !part.is_empty())
        .fold(root.to_path_buf(), |acc, part| acc.join(part))
}

fn build_constraints_for_specialist(domain: &str, risk: f32) -> Vec<String> {
    let mut constraints = vec![
        "preserve existing behavior unless objective explicitly changes it".to_string(),
        "prefer minimal and reviewable diff scope".to_string(),
    ];

    if domain == "auth" {
        constraints.push("never relax token/session validation rules".to_string());
    }
    if domain == "database" {
        constraints.push("changes must preserve forward and rollback migration safety".to_string());
    }
    if risk > 0.7 {
        constraints.push("high risk: prioritize safer incremental edits".to_string());
    }

    constraints
}

fn detect_conflict(proposals: &[DiffProposal]) -> Option<ConflictReport> {
    if proposals.len() < 2 {
        return None;
    }

    let mut strongest: Option<(usize, usize, f32)> = None;
    for a in 0..proposals.len() {
        for b in (a + 1)..proposals.len() {
            let distance = specialist::semantic_distance(&proposals[a], &proposals[b]);
            if strongest.is_none_or(|(_, _, current)| distance > current) {
                strongest = Some((a, b, distance));
            }
        }
    }

    let Some((a_idx, b_idx, distance)) = strongest else {
        return None;
    };

    if distance <= 0.3 {
        return None;
    }

    Some(ConflictReport {
        agent_a: proposals[a_idx].agent_uid.clone(),
        agent_b: proposals[b_idx].agent_uid.clone(),
        semantic_distance: distance,
        description: format!(
            "Specialist intent distance is {:.3}; proposals diverge beyond Tier 2 merge threshold.",
            distance
        ),
        requires_human_review: true,
    })
}

fn summarize_status(proposals: &[DiffProposal], conflict: Option<&ConflictReport>) -> String {
    if proposals.is_empty() {
        return "blocked".to_string();
    }
    if conflict.is_some() {
        return "consensus_failed".to_string();
    }
    "ready_for_review".to_string()
}

fn compute_compliance_score(
    _domain: &str,
    risk_factor: f32,
    proposals: &[DiffProposal],
    status: &str,
) -> i64 {
    let mut score = 55_i64;
    score += (proposals.len() as i64) * 12;
    score += ((1.0 - risk_factor.clamp(0.0, 1.0)) * 20.0) as i64;

    if status == "consensus_failed" {
        score -= 18;
    } else if status == "blocked" {
        score -= 30;
    }

    score.clamp(0, 100)
}

fn build_summary(
    domain: &str,
    status: &str,
    proposals: &[DiffProposal],
    tokens_spent: u32,
    conflict: Option<&ConflictReport>,
) -> String {
    match status {
        "ready_for_review" => format!(
            "Tier 2 {} domain leader generated {} specialist proposals and persisted them for review. Total estimated tokens spent: {}.",
            domain,
            proposals.len(),
            tokens_spent
        ),
        "consensus_failed" => format!(
            "Tier 2 {} domain leader generated {} proposals but detected semantic disagreement (distance {:.3}). Human review is required before moving forward.",
            domain,
            proposals.len(),
            conflict.map(|value| value.semantic_distance).unwrap_or(0.0)
        ),
        _ => format!(
            "Tier 2 {} domain leader could not produce valid specialist proposals. Task is blocked and needs objective or context adjustments.",
            domain
        ),
    }
}

#[cfg(test)]
mod tests {
    use sqlx::sqlite::SqlitePoolOptions;
    use tempfile::tempdir;

    use crate::db;
    use crate::db::mutations::{list_mutations_for_task, ListTaskMutationsInput};
    use crate::db::tasks::{self, CreateTaskInput};
    use crate::mcp_bridge::client::BridgeClient;
    use crate::model_registry::ModelRegistry;
    use crate::vector::indexer;

    use super::*;

    async fn setup_test_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite should initialize");

        db::run_migrations(&pool)
            .await
            .expect("migrations should run in tests");

        pool
    }

    #[tokio::test]
    async fn executes_tier2_task_and_persists_proposals() {
        let pool = setup_test_pool().await;
        let project = tempdir().expect("temp project should initialize");
        let src_dir = project.path().join("src/auth");
        fs::create_dir_all(&src_dir).expect("auth source dir should be created");
        fs::write(
            src_dir.join("session.ts"),
            "export function useSession() {\n  return { loading: false, user: null };\n}\n",
        )
        .expect("fixture should be created");
        fs::write(
            src_dir.join("guard.ts"),
            "export function requireAuth() {\n  return true;\n}\n",
        )
        .expect("fixture should be created");

        indexer::index_project(&pool, &project.path().to_string_lossy())
            .await
            .expect("vector indexing should succeed");

        let parent = tasks::create_task(
            &pool,
            CreateTaskInput {
                parent_id: None,
                tier: 1,
                domain: "auth".to_string(),
                objective: "Orchestrate objective".to_string(),
                token_budget: 1000,
            },
        )
        .await
        .expect("parent should be created");
        let tier2 = tasks::create_task(
            &pool,
            CreateTaskInput {
                parent_id: Some(parent.id),
                tier: 2,
                domain: "auth".to_string(),
                objective: "Refactor auth module".to_string(),
                token_budget: 3600,
            },
        )
        .await
        .expect("tier2 should be created");

        let runtime = tempdir().expect("runtime should initialize");
        fs::create_dir_all(runtime.path().join("mcp-bridge")).expect("bridge dir should exist");
        let bridge_client = BridgeClient::new(runtime.path());

        let model_registry = ModelRegistry::default();
        let result = execute_domain_task(
            &pool,
            &bridge_client,
            &model_registry,
            ExecuteDomainTaskInput {
                task_id: tier2.id.clone(),
                target_project: project.path().to_string_lossy().to_string(),
                top_k: Some(5),
                mcp_command: None,
                mcp_args: None,
            },
        )
        .await
        .expect("domain execution should succeed");

        assert!(matches!(
            result.status.as_str(),
            "ready_for_review" | "consensus_failed"
        ));
        assert!(!result.proposals.is_empty());

        let mutations = list_mutations_for_task(
            &pool,
            ListTaskMutationsInput {
                task_id: tier2.id.clone(),
            },
        )
        .await
        .expect("mutations should list");

        assert_eq!(mutations.len(), result.proposals.len());
    }
}
