use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::db::tasks::{self, CreateTaskRecordInput, TaskRecord, TaskStatus};
use crate::llm_adapter;
use crate::model_registry::ModelRegistry;
use crate::task_runtime;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserObjectiveInput {
    pub objective: String,
    pub target_project: String,
    pub global_token_budget: u32,
    pub max_risk_tolerance: f32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskAssignment {
    pub task_id: String,
    pub parent_id: String,
    pub tier: u8,
    pub domain: String,
    pub objective: String,
    pub token_budget: u32,
    pub risk_factor: f32,
    pub constraints: Vec<String>,
    pub relevant_files: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrchestrationResult {
    pub root_task: TaskRecord,
    pub assignments: Vec<TaskAssignment>,
    pub overhead_budget: u32,
    pub reserve_budget: u32,
    pub distributed_budget: u32,
}

#[derive(Debug, Clone)]
struct AssignmentDraft {
    domain: String,
    objective: String,
}

pub async fn orchestrate_and_persist(
    pool: &SqlitePool,
    model_registry: &ModelRegistry,
    input: UserObjectiveInput,
) -> Result<OrchestrationResult, String> {
    validate_objective_input(&input)?;

    let tier1_model = model_registry.resolve_with_supported_providers(
        1,
        None,
        &llm_adapter::supported_provider_aliases(),
    )?;
    let objective = input.objective.trim().to_string();
    let domain = infer_primary_domain(&objective);
    let assignment_count = desired_assignment_count(&objective);
    let drafts = build_assignment_drafts(&domain, assignment_count);
    let target_root = normalize_project_root(&input.target_project)?;
    let all_candidate_files = collect_source_files(&target_root, 600)?;

    let overhead_budget = ((input.global_token_budget as f32) * 0.10).round() as u32;
    let reserve_budget = ((input.global_token_budget as f32) * 0.10).round() as u32;
    let distributed_budget = input
        .global_token_budget
        .saturating_sub(overhead_budget + reserve_budget);

    let root_task = tasks::create_task_record(
        pool,
        CreateTaskRecordInput {
            parent_id: None,
            tier: 1,
            domain: domain.clone(),
            objective: format!(
                "Orchestrate objective: {objective} [model {}/{}]",
                tier1_model.provider, tier1_model.model_id
            ),
            token_budget: overhead_budget.max(1) as i64,
            risk_factor: 0.0,
            status: TaskStatus::Pending,
        },
    )
    .await?;

    tasks::update_task_status(
        pool,
        tasks::UpdateTaskStatusInput {
            task_id: root_task.id.clone(),
            status: TaskStatus::Executing,
            error_message: None,
        },
    )
    .await?;
    task_runtime::record_task_activity(
        pool,
        "tier1_orchestrator",
        "orchestration_started",
        &root_task.id,
        &format!(
            "objective={} target={} model={}/{} budget={}",
            objective,
            input.target_project.trim(),
            tier1_model.provider,
            tier1_model.model_id,
            input.global_token_budget
        ),
    )
    .await?;
    task_runtime::record_task_activity(
        pool,
        "tier1_orchestrator",
        "orchestration_context_built",
        &root_task.id,
        &format!(
            "assignmentCount={} candidateFiles={} distributedBudget={}",
            drafts.len(),
            all_candidate_files.len(),
            distributed_budget
        ),
    )
    .await?;

    let mut risk_weights = Vec::with_capacity(drafts.len());
    let mut per_draft_context: Vec<(Vec<String>, f32, Vec<String>)> =
        Vec::with_capacity(drafts.len());

    for draft in &drafts {
        let relevant_files =
            find_relevant_files(&all_candidate_files, &draft.domain, &draft.objective, 16);
        let p_failure = estimate_failure_probability(&objective, &draft.objective, &draft.domain);
        let impact = estimate_impact(relevant_files.len());
        let coverage = estimate_test_coverage(&relevant_files);
        let risk = calculate_pra_risk(p_failure, impact, coverage);
        let constraints = build_constraints(
            &draft.domain,
            risk,
            input.max_risk_tolerance.clamp(0.0, 1.0),
            &objective,
        );

        risk_weights.push(1.0 + (risk * 2.2));
        per_draft_context.push((relevant_files, risk, constraints));
    }

    let budgets = allocate_token_budgets(distributed_budget.max(1), &risk_weights);
    let mut assignments = Vec::with_capacity(drafts.len());

    for (idx, draft) in drafts.iter().enumerate() {
        if let Err(error) = task_runtime::cooperative_checkpoint(
            pool,
            &root_task.id,
            "tier1_orchestrator",
            &format!("assignment_{idx}_planning"),
        )
        .await
        {
            let _ = task_runtime::record_task_activity(
                pool,
                "tier1_orchestrator",
                "orchestration_stopped",
                &root_task.id,
                &error,
            )
            .await;
            return Err(error);
        }

        let (relevant_files, risk_factor, constraints) = &per_draft_context[idx];
        let created = tasks::create_task_record(
            pool,
            CreateTaskRecordInput {
                parent_id: Some(root_task.id.clone()),
                tier: 2,
                domain: draft.domain.clone(),
                objective: draft.objective.clone(),
                token_budget: budgets[idx].max(1) as i64,
                risk_factor: f64::from(*risk_factor),
                status: TaskStatus::Pending,
            },
        )
        .await?;

        task_runtime::record_task_activity(
            pool,
            "tier1_orchestrator",
            "tier2_assignment_created",
            &created.id,
            &format!(
                "parent={} domain={} risk={:.3} tokenBudget={} files={} constraints={}",
                root_task.id,
                draft.domain,
                risk_factor,
                budgets[idx],
                relevant_files.len(),
                constraints.join(" | ")
            ),
        )
        .await?;

        assignments.push(TaskAssignment {
            task_id: created.id,
            parent_id: root_task.id.clone(),
            tier: 2,
            domain: draft.domain.clone(),
            objective: draft.objective.clone(),
            token_budget: budgets[idx],
            risk_factor: *risk_factor,
            constraints: constraints.clone(),
            relevant_files: relevant_files.clone(),
        });
    }

    tasks::update_task_status(
        pool,
        tasks::UpdateTaskStatusInput {
            task_id: root_task.id.clone(),
            status: TaskStatus::Completed,
            error_message: None,
        },
    )
    .await?;
    task_runtime::record_task_activity(
        pool,
        "tier1_orchestrator",
        "orchestration_completed",
        &root_task.id,
        &format!(
            "assignments={} reserveBudget={}",
            assignments.len(),
            reserve_budget
        ),
    )
    .await?;

    Ok(OrchestrationResult {
        root_task,
        assignments,
        overhead_budget,
        reserve_budget,
        distributed_budget,
    })
}

fn validate_objective_input(input: &UserObjectiveInput) -> Result<(), String> {
    if input.objective.trim().is_empty() {
        return Err("objective is required".to_string());
    }
    if input.target_project.trim().is_empty() {
        return Err("targetProject is required".to_string());
    }
    if input.global_token_budget < 100 {
        return Err("globalTokenBudget must be at least 100".to_string());
    }
    if !(0.0..=1.0).contains(&input.max_risk_tolerance) {
        return Err("maxRiskTolerance must be between 0.0 and 1.0".to_string());
    }

    Ok(())
}

fn infer_primary_domain(objective: &str) -> String {
    let value = objective.to_lowercase();
    if contains_any(
        &value,
        &["auth", "login", "session", "oauth", "token", "credential"],
    ) {
        return "auth".to_string();
    }
    if contains_any(&value, &["database", "query", "sql", "migration", "index"]) {
        return "database".to_string();
    }
    if contains_any(&value, &["frontend", "react", "ui", "component", "render"]) {
        return "frontend".to_string();
    }
    if contains_any(&value, &["api", "endpoint", "http", "route"]) {
        return "api".to_string();
    }
    "platform".to_string()
}

fn desired_assignment_count(objective: &str) -> usize {
    let value = objective.to_lowercase();
    if contains_any(&value, &["refactor", "rewrite", "migrate", "overhaul"]) {
        5
    } else if value.len() > 80 {
        4
    } else {
        3
    }
}

fn build_assignment_drafts(domain: &str, count: usize) -> Vec<AssignmentDraft> {
    let templates: Vec<(&str, &str)> =
        match domain {
            "auth" => {
                vec![
            ("auth", "Audit current auth module boundaries and high-risk dependencies."),
            (
                "auth",
                "Refactor authentication domain services and contracts for clearer ownership.",
            ),
            (
                "auth",
                "Update auth persistence/session adapters to match the new interfaces.",
            ),
            (
                "frontend",
                "Adjust auth consumers (middleware, hooks, guards) to the refactored flow.",
            ),
            ("testing", "Expand auth regression tests and failure-mode coverage."),
        ]
            }
            "database" => vec![
                (
                    "database",
                    "Map current data model and identify schema/query refactor points.",
                ),
                (
                    "database",
                    "Refactor repository/query layer to reduce coupling and improve clarity.",
                ),
                (
                    "database",
                    "Validate indexes, constraints, and migration safety for new data paths.",
                ),
                (
                    "api",
                    "Adapt API/use-case boundaries to updated persistence contracts.",
                ),
                (
                    "testing",
                    "Add database regression and performance guardrail tests.",
                ),
            ],
            "frontend" => vec![
                (
                    "frontend",
                    "Assess component boundaries and rendering hotspots in scope.",
                ),
                (
                    "frontend",
                    "Refactor core components/hooks into clearer state and dependency boundaries.",
                ),
                (
                    "frontend",
                    "Update shared UI contracts and integration points used by other modules.",
                ),
                (
                    "api",
                    "Align client data-access layer with refactored frontend boundaries.",
                ),
                (
                    "testing",
                    "Expand UI smoke/regression tests for critical user flows in scope.",
                ),
            ],
            _ => vec![
                (
                    "platform",
                    "Analyze module boundaries, dependencies, and high-risk change surfaces.",
                ),
                (
                    "platform",
                    "Refactor core business logic into explicit contracts and stable seams.",
                ),
                (
                    "platform",
                    "Update adapters/integration layers to honor the new boundaries.",
                ),
                (
                    "testing",
                    "Increase regression and edge-case coverage for the refactored scope.",
                ),
                (
                    "platform",
                    "Review rollout risk and verification checklist for safe adoption.",
                ),
            ],
        };

    templates
        .into_iter()
        .take(count.clamp(3, 5))
        .map(|(draft_domain, objective)| AssignmentDraft {
            domain: draft_domain.to_string(),
            objective: objective.to_string(),
        })
        .collect()
}

fn contains_any(value: &str, patterns: &[&str]) -> bool {
    patterns.iter().any(|pattern| value.contains(pattern))
}

fn normalize_project_root(target_project: &str) -> Result<PathBuf, String> {
    let root = PathBuf::from(target_project.trim());
    let normalized = fs::canonicalize(root)
        .map_err(|error| format!("Unable to resolve target project path: {error}"))?;

    if !normalized.is_dir() {
        return Err(format!(
            "Target project path '{}' is not a directory",
            normalized.display()
        ));
    }

    Ok(normalized)
}

fn collect_source_files(root: &Path, limit: usize) -> Result<Vec<String>, String> {
    let mut queue = VecDeque::from([root.to_path_buf()]);
    let mut files = Vec::new();

    while let Some(current_dir) = queue.pop_front() {
        if files.len() >= limit {
            break;
        }

        let entries = fs::read_dir(&current_dir).map_err(|error| {
            format!(
                "Failed to read directory '{}': {error}",
                current_dir.display()
            )
        })?;

        for entry in entries {
            let entry =
                entry.map_err(|error| format!("Failed to read directory entry: {error}"))?;
            let path = entry.path();
            let file_type = entry
                .file_type()
                .map_err(|error| format!("Failed to inspect '{}': {error}", path.display()))?;

            if file_type.is_dir() {
                let name = entry.file_name().to_string_lossy().to_string();
                if should_skip_dir(&name) {
                    continue;
                }
                queue.push_back(path);
                continue;
            }

            if file_type.is_file() && is_supported_extension(&path) {
                let relative = path
                    .strip_prefix(root)
                    .map_err(|error| format!("Failed to compute relative path: {error}"))?;
                let relative = relative
                    .components()
                    .map(|component| component.as_os_str().to_string_lossy().to_string())
                    .collect::<Vec<_>>()
                    .join("/");
                files.push(relative);
            }

            if files.len() >= limit {
                break;
            }
        }
    }

    Ok(files)
}

fn should_skip_dir(name: &str) -> bool {
    matches!(
        name,
        ".git" | "node_modules" | "target" | "dist" | "build" | ".next" | ".turbo"
    )
}

fn is_supported_extension(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|value| value.to_str()),
        Some("ts")
            | Some("tsx")
            | Some("js")
            | Some("jsx")
            | Some("rs")
            | Some("json")
            | Some("md")
            | Some("py")
            | Some("java")
            | Some("go")
    )
}

fn find_relevant_files(
    files: &[String],
    domain: &str,
    objective: &str,
    limit: usize,
) -> Vec<String> {
    let mut keywords = vec![domain.to_lowercase()];
    keywords.extend(
        objective
            .split(|ch: char| !ch.is_alphanumeric() && ch != '_')
            .map(|part| part.trim().to_ascii_lowercase())
            .filter(|part| part.len() >= 4)
            .take(8),
    );

    let mut selected = files
        .iter()
        .filter(|path| {
            let lower = path.to_ascii_lowercase();
            keywords.iter().any(|keyword| lower.contains(keyword))
        })
        .take(limit)
        .cloned()
        .collect::<Vec<_>>();

    if selected.is_empty() {
        selected = files.iter().take(limit.min(8)).cloned().collect();
    }

    selected
}

fn estimate_failure_probability(
    global_objective: &str,
    assignment_objective: &str,
    domain: &str,
) -> f32 {
    let mut probability = 0.22_f32;
    let global = global_objective.to_ascii_lowercase();
    let local = assignment_objective.to_ascii_lowercase();

    if contains_any(&global, &["refactor", "rewrite", "migrate", "replace"]) {
        probability += 0.22;
    }
    if contains_any(&global, &["performance", "cache", "concurrency"]) {
        probability += 0.08;
    }
    if contains_any(&global, &["auth", "security", "session", "token"]) || domain == "auth" {
        probability += 0.15;
    }
    if domain == "database" || contains_any(&local, &["schema", "migration", "query"]) {
        probability += 0.12;
    }
    if domain == "testing" {
        probability -= 0.08;
    }

    probability.clamp(0.05, 0.95)
}

fn estimate_impact(relevant_files: usize) -> f32 {
    if relevant_files == 0 {
        return 0.25;
    }

    ((relevant_files as f32) / 14.0).clamp(0.15, 1.0)
}

fn estimate_test_coverage(relevant_files: &[String]) -> f32 {
    if relevant_files.is_empty() {
        return 0.15;
    }

    let test_files = relevant_files
        .iter()
        .filter(|path| {
            let lower = path.to_ascii_lowercase();
            lower.contains(".test.")
                || lower.contains(".spec.")
                || lower.contains("/tests/")
                || lower.ends_with("_test.rs")
        })
        .count();

    ((test_files as f32) / (relevant_files.len() as f32)).clamp(0.0, 1.0)
}

fn calculate_pra_risk(p_failure: f32, impact: f32, test_coverage: f32) -> f32 {
    (p_failure * impact * (1.0 - test_coverage)).clamp(0.0, 1.0)
}

fn build_constraints(domain: &str, risk: f32, max_tolerance: f32, objective: &str) -> Vec<String> {
    let mut constraints = vec![
        "preserve observable behavior unless explicitly documented".to_string(),
        "respect existing architectural boundaries".to_string(),
    ];

    if domain == "auth" {
        constraints.push("do not weaken authentication or token validation logic".to_string());
    }
    if domain == "database" {
        constraints.push("changes must keep data migration path reversible".to_string());
    }
    if domain == "frontend" {
        constraints.push("avoid regressions in loading and error states".to_string());
    }
    if domain == "testing" {
        constraints.push("tests must validate critical success and failure paths".to_string());
    }

    if risk > max_tolerance {
        constraints.push(format!(
            "risk {risk:.2} exceeds tolerance {max_tolerance:.2}; escalate for Tier 1 approval"
        ));
    } else if risk > 0.7 {
        constraints.push("high risk change: require strict validation before apply".to_string());
    } else if risk >= 0.3 {
        constraints.push("medium risk change: run consensus validation".to_string());
    }

    if objective.to_ascii_lowercase().contains("refactor") {
        constraints.push("maintain compatibility with existing public interfaces".to_string());
    }

    constraints
}

fn allocate_token_budgets(distributed_budget: u32, weights: &[f32]) -> Vec<u32> {
    if weights.is_empty() {
        return Vec::new();
    }

    let total_weight: f32 = weights.iter().sum::<f32>().max(1.0);
    let mut budgets = weights
        .iter()
        .map(|weight| {
            let raw = (distributed_budget as f32) * (*weight / total_weight);
            raw.floor() as u32
        })
        .collect::<Vec<_>>();

    let mut assigned = budgets.iter().sum::<u32>();
    while assigned < distributed_budget {
        for value in &mut budgets {
            if assigned >= distributed_budget {
                break;
            }
            *value += 1;
            assigned += 1;
        }
    }

    budgets
}

#[cfg(test)]
mod tests {
    use std::fs;

    use sqlx::sqlite::SqlitePoolOptions;
    use tempfile::tempdir;

    use super::*;
    use crate::db;
    use crate::model_registry::ModelRegistry;

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
    async fn refactor_auth_module_creates_three_to_five_subtasks() {
        let pool = setup_test_pool().await;
        let project_dir = tempdir().expect("temp project should initialize");
        let auth_dir = project_dir.path().join("src/auth");
        fs::create_dir_all(&auth_dir).expect("auth fixtures should be created");
        fs::write(
            auth_dir.join("session.ts"),
            "export function getSession() { return null }\n",
        )
        .expect("fixture should be written");
        fs::write(
            auth_dir.join("session.test.ts"),
            "test('session', () => expect(true).toBe(true))\n",
        )
        .expect("fixture should be written");

        let model_registry = ModelRegistry::default();
        let result = orchestrate_and_persist(
            &pool,
            &model_registry,
            UserObjectiveInput {
                objective: "Refactor auth module".to_string(),
                target_project: project_dir.path().to_string_lossy().to_string(),
                global_token_budget: 10_000,
                max_risk_tolerance: 0.6,
            },
        )
        .await
        .expect("orchestration should succeed");

        assert!((3..=5).contains(&result.assignments.len()));
        assert_eq!(result.root_task.tier, 1);
        assert!(result
            .assignments
            .iter()
            .all(|assignment| assignment.parent_id == result.root_task.id));
        assert_eq!(
            result
                .assignments
                .iter()
                .map(|assignment| assignment.token_budget)
                .sum::<u32>(),
            result.distributed_budget.max(1)
        );
    }
}
