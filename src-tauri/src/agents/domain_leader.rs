use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::agents::specialist::{self, DiffProposal, SpecialistTask};
use crate::agents::CodeBlock;
use crate::db::mutations::{self, CreateMutationInput};
use crate::db::tasks::{self, CreateTaskRecordInput, TaskStatus, UpdateTaskStatusInput};
use crate::mcp_bridge::client::BridgeClient;
use crate::mcp_bridge::tool_caller::{self, ReadTargetFileInput, SearchTargetFilesInput};
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
    input: ExecuteDomainTaskInput,
) -> Result<IntentSummary, String> {
    validate_input(&input)?;
    let task = tasks::get_task_by_id(pool, input.task_id.trim()).await?;

    if task.tier != 2 {
        return Err(format!(
            "Task '{}' is tier {}. execute_domain_task only supports tier 2 tasks.",
            task.id, task.tier
        ));
    }

    tasks::update_task_status(
        pool,
        UpdateTaskStatusInput {
            task_id: task.id.clone(),
            status: TaskStatus::Executing,
            error_message: None,
        },
    )
    .await?;

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
        &task.objective,
        &chunks,
        &input,
    )
    .await;

    let personas = personas_for_domain(&task.domain);
    let specialist_budgets = distribute_budget_for_specialists(task.token_budget, personas.len());
    let mut proposals: Vec<DiffProposal> = Vec::with_capacity(personas.len());
    let mut tokens_spent = 0_u32;

    for (idx, persona) in personas.iter().enumerate() {
        let specialist_objective =
            build_specialist_objective(&task.domain, &task.objective, persona.as_str(), idx);
        let target_file = candidate_files
            .get(if candidate_files.is_empty() {
                0
            } else {
                idx % candidate_files.len()
            })
            .cloned()
            .unwrap_or_else(|| "src/main.ts".to_string());
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
            },
        )
        .await?;

        let file_content = read_file_with_fallback(bridge_client, &input, &target_file).await;
        tasks::update_task_status(
            pool,
            UpdateTaskStatusInput {
                task_id: specialist_task_record.id.clone(),
                status: TaskStatus::Executing,
                error_message: None,
            },
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
            constraints: build_constraints_for_specialist(&task.domain, task.risk_factor as f32),
        };

        match specialist::run_specialist_task(&specialist_task, file_content.as_deref()) {
            Ok(proposal) => {
                tokens_spent = tokens_spent.saturating_add(proposal.tokens_used);

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

                tasks::update_task_status(
                    pool,
                    UpdateTaskStatusInput {
                        task_id: specialist_task_record.id,
                        status: TaskStatus::Completed,
                        error_message: None,
                    },
                )
                .await?;

                proposals.push(proposal);
            }
            Err(error) => {
                tasks::update_task_status(
                    pool,
                    UpdateTaskStatusInput {
                        task_id: specialist_task_record.id,
                        status: TaskStatus::Failed,
                        error_message: Some(error),
                    },
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

    let final_task_status = if status == "ready_for_review" {
        TaskStatus::Completed
    } else if status == "blocked" {
        TaskStatus::Failed
    } else {
        TaskStatus::Paused
    };

    tasks::update_task_status(
        pool,
        UpdateTaskStatusInput {
            task_id: task.id.clone(),
            status: final_task_status,
            error_message: None,
        },
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
        return ordered.into_iter().take(6).collect();
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

    ordered.into_iter().take(6).collect()
}

fn personas_for_domain(domain: &str) -> Vec<String> {
    match domain {
        "auth" => vec![
            "security_analyst".to_string(),
            "react_specialist".to_string(),
            "test_engineer".to_string(),
        ],
        "database" => vec![
            "database_optimizer".to_string(),
            "test_engineer".to_string(),
            "style_enforcer".to_string(),
        ],
        "frontend" => vec![
            "react_specialist".to_string(),
            "test_engineer".to_string(),
            "style_enforcer".to_string(),
        ],
        "api" => vec![
            "security_analyst".to_string(),
            "test_engineer".to_string(),
            "style_enforcer".to_string(),
        ],
        _ => vec![
            "style_enforcer".to_string(),
            "test_engineer".to_string(),
            "security_analyst".to_string(),
        ],
    }
}

fn distribute_budget_for_specialists(tier2_budget: i64, specialists: usize) -> Vec<u32> {
    if specialists == 0 {
        return Vec::new();
    }

    let available = ((tier2_budget.max(1) as f32) * 0.8).floor() as u32;
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
    domain: &str,
    parent_objective: &str,
    persona: &str,
    specialist_idx: usize,
) -> String {
    let focus = match persona {
        "security_analyst" => "audit auth and input-validation failure paths",
        "react_specialist" => "reduce render churn and stabilize component contracts",
        "database_optimizer" => "improve query paths and migration safety checks",
        "test_engineer" => "add regression checks for success and failure behavior",
        "style_enforcer" => "tighten consistency against existing architectural conventions",
        _ => "refine implementation with lower operational risk",
    };

    format!(
        "[{}:{}] {} for objective: {}",
        domain,
        specialist_idx + 1,
        focus,
        parent_objective.trim()
    )
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

        let result = execute_domain_task(
            &pool,
            &bridge_client,
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
