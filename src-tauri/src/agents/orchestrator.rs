use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use std::collections::HashMap;

use crate::agents::domain_leader::{self, ExecuteDomainTaskInput};
use crate::agents::specialist::{self, SpecialistTask};
use crate::agents::CodeBlock;
use crate::db::mutations::{self, CreateMutationInput, ListTaskMutationsInput, MutationStatus};
use crate::db::tasks::{
    self, CreateTaskRecordInput, TaskRecord, TaskStatus, UpdateTaskOutcomeInput,
    UpdateTaskStatusInput,
};
use crate::llm_adapter::{self, AdapterRequest};
use crate::mcp_bridge::client::BridgeClient;
use crate::mcp_bridge::tool_caller::{self, ReadTargetFileInput};
use crate::model_intelligence::{self, ModelSelectionRequest};
use crate::model_registry::ModelRegistry;
use crate::mutation_pipeline::{self, RunMutationPipelineInput};
use crate::task_runtime;
use crate::vector::search;
use crate::vector::ContextChunk;

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
    tier: u8,
    domain: String,
    objective: String,
    target_files: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApproveOrchestrationPlanInput {
    pub root_task_id: String,
    pub target_project: String,
    pub top_k: Option<u32>,
    pub mcp_command: Option<String>,
    pub mcp_args: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MutationSummary {
    pub id: String,
    pub task_id: String,
    pub file_path: String,
    pub status: String,
    pub intent_description: Option<String>,
    pub confidence: f64,
    pub rejection_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanExecutionResult {
    pub root_task: TaskRecord,
    pub executed_task_ids: Vec<String>,
    pub tier2_executions: u32,
    pub tier3_executions: u32,
    pub applied_mutations: u32,
    pub failed_executions: u32,
    pub message: String,
    pub mutation_summaries: Vec<MutationSummary>,
}

// --- New types for LLM-driven orchestration ---

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzeObjectiveInput {
    pub objective: String,
    pub target_project: String,
    pub global_token_budget: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectiveAnalysis {
    pub root_task_id: String,
    pub questions: Vec<String>,
    pub initial_analysis: String,
    pub suggested_approach: String,
    pub file_tree_summary: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratePlanInput {
    pub root_task_id: String,
    pub objective: String,
    pub answers: HashMap<String, String>,
    pub target_project: String,
    pub global_token_budget: u32,
    pub max_risk_tolerance: f32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedPlan {
    pub root_task: TaskRecord,
    pub assignments: Vec<TaskAssignment>,
    pub risk_assessment: String,
    pub overhead_budget: u32,
    pub reserve_budget: u32,
    pub distributed_budget: u32,
}

// Internal LLM response types

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LlmAnalysisResponse {
    #[serde(default)]
    questions: Vec<String>,
    #[serde(default)]
    initial_analysis: Option<String>,
    #[serde(default)]
    suggested_approach: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LlmPlanResponse {
    #[serde(default)]
    tasks: Vec<LlmPlannedTask>,
    #[serde(default)]
    risk_assessment: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LlmPlannedTask {
    objective: String,
    domain: String,
    #[serde(default = "default_tier")]
    tier: u8,
    #[serde(default)]
    target_files: Vec<String>,
    #[serde(default)]
    rationale: Option<String>,
}

fn default_tier() -> u8 {
    3
}

// --- Orchestration functions ---

pub async fn orchestrate_and_persist(
    pool: &SqlitePool,
    model_registry: &ModelRegistry,
    input: UserObjectiveInput,
) -> Result<OrchestrationResult, String> {
    validate_objective_input(&input)?;

    let tier1_model = model_intelligence::select_model(
        pool,
        model_registry,
        ModelSelectionRequest {
            task_id: None,
            actor: "tier1_orchestrator",
            tier: 1,
            persona: None,
            skill: Some("orchestration_planning"),
        },
    )
    .await?
    .selection;
    let objective = input.objective.trim().to_string();
    let domain = infer_primary_domain(&objective);
    let target_root = normalize_project_root(&input.target_project)?;
    let all_candidate_files = collect_source_files(&target_root, 600)?;
    let file_tree_summary = build_file_tree_summary(&all_candidate_files, 120);
    let drafts = generate_drafts_with_llm(
        &tier1_model.provider,
        &tier1_model.model_id,
        &objective,
        &domain,
        &file_tree_summary,
        input.global_token_budget,
        input.max_risk_tolerance,
    );

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
            target_files: None,
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
            "assignmentCount={} candidateFiles={} distributedBudget={} llmDriven=true",
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
        let relevant_files = if draft.target_files.is_empty() {
            find_relevant_files(&all_candidate_files, &draft.domain, &draft.objective, 16)
        } else {
            draft.target_files.clone()
        };
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
        let target_files_json = if relevant_files.is_empty() {
            None
        } else {
            Some(serde_json::to_string(relevant_files).unwrap_or_default())
        };
        let created = tasks::create_task_record(
            pool,
            CreateTaskRecordInput {
                parent_id: Some(root_task.id.clone()),
                tier: i64::from(draft.tier),
                domain: draft.domain.clone(),
                objective: draft.objective.clone(),
                token_budget: budgets[idx].max(1) as i64,
                risk_factor: f64::from(*risk_factor),
                status: TaskStatus::Paused,
                target_files: target_files_json,
            },
        )
        .await?;

        task_runtime::record_task_activity(
            pool,
            "tier1_orchestrator",
            "plan_assignment_created",
            &created.id,
            &format!(
                "parent={} tier={} domain={} risk={:.3} tokenBudget={} files={} constraints={}",
                root_task.id,
                draft.tier,
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
            tier: draft.tier,
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
            status: TaskStatus::Paused,
            error_message: Some(
                "plan_ready: awaiting review approval before spawning agents".to_string(),
            ),
        },
    )
    .await?;
    task_runtime::record_task_activity(
        pool,
        "tier1_orchestrator",
        "orchestration_plan_ready",
        &root_task.id,
        &format!(
            "assignments={} reserveBudget={} nextStep=execute_tier2_and_apply_mutations",
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

pub async fn approve_plan_and_spawn(
    pool: &SqlitePool,
    bridge_client: &BridgeClient,
    model_registry: &ModelRegistry,
    input: ApproveOrchestrationPlanInput,
) -> Result<PlanExecutionResult, String> {
    validate_approve_input(&input)?;

    let root_task_id = input.root_task_id.trim();
    let root_task = tasks::get_task_by_id(pool, root_task_id).await?;
    if root_task.tier != 1 {
        return Err(format!(
            "Task '{}' is tier {}. approve_orchestration_plan only supports tier 1 tasks.",
            root_task.id, root_task.tier
        ));
    }

    let task_tree_ids = tasks::collect_task_tree_ids(pool, root_task_id).await?;
    let mut planned_tasks = Vec::new();
    for task_id in task_tree_ids.iter().skip(1) {
        let task = tasks::get_task_by_id(pool, task_id).await?;
        if !matches!(task.tier, 2 | 3) {
            continue;
        }
        if !matches!(task.status.as_str(), "paused" | "pending") {
            continue;
        }
        planned_tasks.push(task);
    }

    if planned_tasks.is_empty() {
        return Err(format!(
            "Root task '{}' has no paused or pending tier 2/3 assignments to execute.",
            root_task.id
        ));
    }

    tasks::update_task_status(
        pool,
        UpdateTaskStatusInput {
            task_id: root_task.id.clone(),
            status: TaskStatus::Executing,
            error_message: None,
        },
    )
    .await?;

    task_runtime::record_task_activity(
        pool,
        "tier1_orchestrator",
        "orchestration_spawn_started",
        &root_task.id,
        &format!(
            "plannedAssignments={} targetProject={} topK={}",
            planned_tasks.len(),
            input.target_project.trim(),
            input.top_k.unwrap_or(8).max(3)
        ),
    )
    .await?;

    planned_tasks.sort_by(|left, right| {
        right
            .risk_factor
            .partial_cmp(&left.risk_factor)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| right.tier.cmp(&left.tier))
            .then_with(|| left.created_at.cmp(&right.created_at))
    });

    let mut executed_task_ids = Vec::new();
    let mut tier2_executions = 0_u32;
    let mut tier3_executions = 0_u32;
    let mut applied_mutations = 0_u32;
    let mut failed_executions = 0_u32;
    let mut notes: Vec<String> = Vec::new();

    for planned_task in planned_tasks {
        let execution = if planned_task.tier == 2 {
            tier2_executions = tier2_executions.saturating_add(1);
            domain_leader::execute_domain_task(
                pool,
                bridge_client,
                model_registry,
                ExecuteDomainTaskInput {
                    task_id: planned_task.id.clone(),
                    target_project: input.target_project.trim().to_string(),
                    top_k: input.top_k,
                    mcp_command: input.mcp_command.clone(),
                    mcp_args: input.mcp_args.clone(),
                },
            )
            .await
            .map(|_summary| ())
        } else {
            tier3_executions = tier3_executions.saturating_add(1);
            execute_planned_tier3_task(pool, bridge_client, model_registry, &planned_task, &input)
                .await
        };

        if let Err(error) = execution {
            failed_executions = failed_executions.saturating_add(1);
            notes.push(format!("task {} failed: {error}", planned_task.id));
            let _ = task_runtime::record_task_activity(
                pool,
                "tier1_orchestrator",
                "assignment_execution_failed",
                &planned_task.id,
                &error,
            )
            .await;
            continue;
        }

        executed_task_ids.push(planned_task.id.clone());

        // Collect all task IDs that may have mutations:
        // - Tier 3 tasks have mutations directly on their own ID
        // - Tier 2 tasks delegate to tier 3 sub-tasks — mutations are on those sub-task IDs
        let mut apply_task_ids = vec![planned_task.id.clone()];
        if planned_task.tier == 2 {
            let sub_tree = tasks::collect_task_tree_ids(pool, &planned_task.id).await?;
            for sub_id in sub_tree.into_iter().skip(1) {
                apply_task_ids.push(sub_id);
            }
        }

        let mut task_applied = 0_u32;
        let mut task_failed_runs = 0_u32;
        let mut task_first_error: Option<String> = None;

        for apply_id in &apply_task_ids {
            let apply_summary =
                apply_mutations_for_task(pool, apply_id, input.target_project.trim()).await?;
            task_applied = task_applied.saturating_add(apply_summary.applied_mutations);
            task_failed_runs = task_failed_runs.saturating_add(apply_summary.failed_runs);
            if task_first_error.is_none() {
                task_first_error = apply_summary.first_error;
            }
        }

        applied_mutations = applied_mutations.saturating_add(task_applied);
        if task_failed_runs > 0 {
            failed_executions = failed_executions.saturating_add(1);
            if let Some(first_error) = task_first_error {
                notes.push(format!(
                    "task {} apply failures={} firstError={}",
                    planned_task.id, task_failed_runs, first_error
                ));
            } else {
                notes.push(format!(
                    "task {} apply failures={}",
                    planned_task.id, task_failed_runs
                ));
            }
        } else if task_applied == 0 {
            notes.push(format!(
                "task {} produced no applied mutations (review gate or no candidates).",
                planned_task.id
            ));
        }
    }

    let message = format!(
        "Executed={} (tier2={} tier3={}) appliedMutations={} failedExecutions={}. {}",
        executed_task_ids.len(),
        tier2_executions,
        tier3_executions,
        applied_mutations,
        failed_executions,
        if notes.is_empty() {
            "All planned assignments completed without critical errors.".to_string()
        } else {
            notes.join(" | ")
        }
    );

    let (final_status, final_error_message) = if failed_executions > 0 && applied_mutations == 0 {
        (TaskStatus::Failed, Some(message.clone()))
    } else if failed_executions > 0 {
        (TaskStatus::Paused, Some(message.clone()))
    } else if applied_mutations > 0 {
        (TaskStatus::Completed, None)
    } else {
        (TaskStatus::Paused, Some(message.clone()))
    };

    let updated_root = tasks::update_task_status(
        pool,
        UpdateTaskStatusInput {
            task_id: root_task.id.clone(),
            status: final_status,
            error_message: final_error_message,
        },
    )
    .await?;

    task_runtime::record_task_activity(
        pool,
        "tier1_orchestrator",
        "orchestration_spawn_completed",
        &root_task.id,
        &format!(
            "executed={} tier2={} tier3={} appliedMutations={} failedExecutions={}",
            executed_task_ids.len(),
            tier2_executions,
            tier3_executions,
            applied_mutations,
            failed_executions
        ),
    )
    .await?;

    // Collect mutation summaries for all executed tasks + their descendants
    let mut all_mutation_task_ids = Vec::new();
    for exec_id in &executed_task_ids {
        all_mutation_task_ids.push(exec_id.clone());
        if let Ok(sub_ids) = tasks::collect_task_tree_ids(pool, exec_id).await {
            for sub_id in sub_ids.into_iter().skip(1) {
                all_mutation_task_ids.push(sub_id);
            }
        }
    }

    let mut mutation_summaries = Vec::new();
    for mt_id in &all_mutation_task_ids {
        if let Ok(muts) = mutations::list_mutations_for_task(
            pool,
            ListTaskMutationsInput {
                task_id: mt_id.clone(),
            },
        )
        .await
        {
            for m in muts {
                mutation_summaries.push(MutationSummary {
                    id: m.id,
                    task_id: m.task_id,
                    file_path: m.file_path,
                    status: m.status,
                    intent_description: m.intent_description,
                    confidence: m.confidence,
                    rejection_reason: m.rejection_reason,
                });
            }
        }
    }

    Ok(PlanExecutionResult {
        root_task: updated_root,
        executed_task_ids,
        tier2_executions,
        tier3_executions,
        applied_mutations,
        failed_executions,
        message,
        mutation_summaries,
    })
}

#[derive(Debug, Clone)]
struct MutationApplySummary {
    applied_mutations: u32,
    failed_runs: u32,
    first_error: Option<String>,
}

async fn apply_mutations_for_task(
    pool: &SqlitePool,
    task_id: &str,
    target_project: &str,
) -> Result<MutationApplySummary, String> {
    let mutations = mutations::list_mutations_for_task(
        pool,
        ListTaskMutationsInput {
            task_id: task_id.to_string(),
        },
    )
    .await?;

    let mut applied_mutations = 0_u32;
    let mut failed_runs = 0_u32;
    let mut first_error: Option<String> = None;

    for mutation in mutations {
        if !matches!(
            mutation.status.as_str(),
            "proposed" | "validated" | "validated_no_tests"
        ) {
            continue;
        }

        match mutation_pipeline::run_mutation_pipeline(
            pool,
            RunMutationPipelineInput {
                mutation_id: mutation.id.clone(),
                target_project: target_project.to_string(),
                tier1_approved: true,
                ci_command: None,
                ci_args: None,
            },
        )
        .await
        {
            Ok(result) => {
                if result.mutation.status == MutationStatus::Applied.as_str() {
                    applied_mutations = applied_mutations.saturating_add(1);
                } else {
                    failed_runs = failed_runs.saturating_add(1);
                    if first_error.is_none() {
                        first_error = Some(
                            result
                                .mutation
                                .rejection_reason
                                .clone()
                                .or(result.task.error_message.clone())
                                .unwrap_or_else(|| {
                                    format!(
                                        "mutation {} finished with status {}",
                                        result.mutation.id, result.mutation.status
                                    )
                                }),
                        );
                    }
                }
            }
            Err(error) => {
                failed_runs = failed_runs.saturating_add(1);
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }
    }

    Ok(MutationApplySummary {
        applied_mutations,
        failed_runs,
        first_error,
    })
}

async fn execute_planned_tier3_task(
    pool: &SqlitePool,
    bridge_client: &BridgeClient,
    model_registry: &ModelRegistry,
    task: &TaskRecord,
    input: &ApproveOrchestrationPlanInput,
) -> Result<(), String> {
    tasks::update_task_status(
        pool,
        UpdateTaskStatusInput {
            task_id: task.id.clone(),
            status: TaskStatus::Executing,
            error_message: None,
        },
    )
    .await?;

    let persona = infer_tier3_persona(&task.domain, &task.objective);
    let tier3_model = model_intelligence::select_model(
        pool,
        model_registry,
        ModelSelectionRequest {
            task_id: Some(task.id.as_str()),
            actor: "tier1_orchestrator",
            tier: 3,
            persona: Some(persona.as_str()),
            skill: Some("tier3_specialist_spawn"),
        },
    )
    .await?
    .selection;

    let stored_target_files: Vec<String> = task
        .target_files
        .as_deref()
        .and_then(|json| serde_json::from_str(json).ok())
        .unwrap_or_default();

    let (target_file, chunks) = if !stored_target_files.is_empty() {
        (stored_target_files[0].clone(), Vec::new())
    } else {
        let chunks = search::query_codebase(
            pool,
            input.target_project.trim(),
            task.objective.trim(),
            input.top_k.unwrap_or(8).max(3),
        )
        .await
        .unwrap_or_default();
        let file = select_tier3_target_file(&chunks, &task.domain, &task.objective)
            .unwrap_or_else(|| "src/App.tsx".to_string());
        (file, chunks)
    };
    let code_context = hydrate_tier3_code_context(&chunks, &target_file, 2);
    let file_content = read_tier3_file_with_fallback(bridge_client, input, &target_file).await;

    task_runtime::record_task_activity(
        pool,
        "tier1_orchestrator",
        "tier3_planned_execution_started",
        &task.id,
        &format!(
            "persona={} model={}/{} targetFile={}",
            persona, tier3_model.provider, tier3_model.model_id, target_file
        ),
    )
    .await?;

    let specialist_task = SpecialistTask {
        task_id: task.id.clone(),
        parent_id: task.parent_id.clone().unwrap_or_else(|| task.id.clone()),
        tier: 3,
        persona: persona.clone(),
        objective: task.objective.clone(),
        token_budget: task.token_budget.max(1) as u32,
        target_files: vec![target_file.clone()],
        code_context,
        constraints: vec![
            "plan approved by tier1 orchestrator".to_string(),
            "keep diff focused to task objective".to_string(),
            "preserve external behavior unless explicitly requested".to_string(),
        ],
        model_provider: Some(tier3_model.provider.clone()),
        model_id: Some(tier3_model.model_id.clone()),
    };

    let model_started_at = Instant::now();
    let proposal = specialist::run_specialist_task(&specialist_task, file_content.as_deref());
    let model_elapsed = model_started_at.elapsed().as_millis() as i64;
    let proposal = match proposal {
        Ok(value) => {
            model_intelligence::record_model_call_outcome(
                pool,
                tier3_model.provider.as_str(),
                tier3_model.model_id.as_str(),
                true,
                Some(model_elapsed),
                None,
                None,
            )
            .await;
            value
        }
        Err(error) => {
            model_intelligence::record_model_call_outcome(
                pool,
                tier3_model.provider.as_str(),
                tier3_model.model_id.as_str(),
                false,
                Some(model_elapsed),
                None,
                Some(error.clone()),
            )
            .await;
            return Err(error);
        }
    };
    let _mutation = mutations::create_mutation(
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
            task_id: task.id.clone(),
            status: TaskStatus::Paused,
            token_usage: Some(i64::from(proposal.tokens_used)),
            context_efficiency_ratio: None,
            compliance_score: Some(70),
            checksum_before: None,
            checksum_after: None,
            error_message: Some(
                "ready_for_review: tier3 proposal generated; running mutation pipeline."
                    .to_string(),
            ),
        },
    )
    .await?;

    task_runtime::record_task_activity(
        pool,
        &format!("tier3_{}", persona),
        "tier3_planned_execution_completed",
        &task.id,
        &format!(
            "targetFile={} confidence={:.2} tokensUsed={}",
            proposal.file_path, proposal.confidence, proposal.tokens_used
        ),
    )
    .await?;

    Ok(())
}

fn infer_tier3_persona(domain: &str, objective: &str) -> String {
    let combined = format!("{} {}", domain, objective).to_ascii_lowercase();
    if contains_any(&combined, &["test", "spec", "qa", "regression"]) {
        return "test_engineer".to_string();
    }
    if contains_any(&combined, &["security", "auth", "permission", "token"]) {
        return "security_analyst".to_string();
    }
    if contains_any(
        &combined,
        &[
            "react",
            "frontend",
            "ui",
            "component",
            "layout",
            "tsx",
            "view",
        ],
    ) {
        return "react_specialist".to_string();
    }
    if contains_any(&combined, &["query", "schema", "migration", "database"]) {
        return "database_optimizer".to_string();
    }
    "style_enforcer".to_string()
}

fn select_tier3_target_file(
    chunks: &[ContextChunk],
    domain: &str,
    objective: &str,
) -> Option<String> {
    if chunks.is_empty() {
        return None;
    }
    let objective_lower = objective.to_ascii_lowercase();
    let frontend_focus = domain == "frontend"
        || contains_any(
            &objective_lower,
            &["frontend", "react", "ui", "component", "view", "tsx"],
        );
    let rust_explicit = contains_any(
        &objective_lower,
        &["rust", "tauri", "src-tauri", ".rs", "cargo"],
    );

    let mut ranked: Vec<(i64, String)> = chunks
        .iter()
        .map(|chunk| {
            let path = chunk.file_path.clone();
            let score = if frontend_focus && !rust_explicit {
                frontend_path_score(path.as_str())
            } else {
                1
            };
            (score, path)
        })
        .collect();

    ranked.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));
    ranked.first().map(|(_, path)| path.clone())
}

fn frontend_path_score(path: &str) -> i64 {
    let lower = path.to_ascii_lowercase();
    let mut score = 0_i64;
    if lower.ends_with(".tsx") {
        score += 20;
    } else if lower.ends_with(".ts") || lower.ends_with(".jsx") || lower.ends_with(".js") {
        score += 12;
    }
    if lower.contains("/src/") {
        score += 8;
    }
    if lower.contains("/components/") || lower.contains("/views/") || lower.contains("/pages/") {
        score += 10;
    }
    if lower.contains("src-tauri/") || lower.ends_with(".rs") {
        score -= 28;
    }
    score
}

fn hydrate_tier3_code_context(
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
            .collect::<Vec<_>>();
    }

    selected
}

async fn read_tier3_file_with_fallback(
    bridge_client: &BridgeClient,
    input: &ApproveOrchestrationPlanInput,
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

fn validate_approve_input(input: &ApproveOrchestrationPlanInput) -> Result<(), String> {
    if input.root_task_id.trim().is_empty() {
        return Err("rootTaskId is required".to_string());
    }
    if input.target_project.trim().is_empty() {
        return Err("targetProject is required".to_string());
    }

    Ok(())
}

// --- LLM-driven orchestration ---

pub async fn analyze_objective(
    pool: &SqlitePool,
    model_registry: &ModelRegistry,
    input: AnalyzeObjectiveInput,
) -> Result<ObjectiveAnalysis, String> {
    if input.objective.trim().is_empty() {
        return Err("objective is required".to_string());
    }
    if input.target_project.trim().is_empty() {
        return Err("targetProject is required".to_string());
    }

    let objective = input.objective.trim().to_string();
    let target_root = normalize_project_root(&input.target_project)?;
    let source_files = collect_source_files(&target_root, 600)?;
    let file_tree_summary = build_file_tree_summary(&source_files, 120);

    let tier1_model = model_intelligence::select_model(
        pool,
        model_registry,
        ModelSelectionRequest {
            task_id: None,
            actor: "tier1_orchestrator",
            tier: 1,
            persona: None,
            skill: Some("objective_analysis"),
        },
    )
    .await?
    .selection;

    let root_task = tasks::create_task_record(
        pool,
        CreateTaskRecordInput {
            parent_id: None,
            tier: 1,
            domain: infer_primary_domain(&objective),
            objective: format!("Analyze objective: {objective}"),
            token_budget: (input.global_token_budget / 10).max(100) as i64,
            risk_factor: 0.0,
            status: TaskStatus::Executing,
            target_files: None,
        },
    )
    .await?;

    task_runtime::record_task_activity(
        pool,
        "tier1_orchestrator",
        "objective_analysis_started",
        &root_task.id,
        &format!(
            "objective={} model={}/{} files={}",
            objective, tier1_model.provider, tier1_model.model_id, source_files.len()
        ),
    )
    .await?;

    let system_prompt = r#"You are a Tier-1 orchestrator for the Autonomous Orchestration Platform (AOP).
Your job is to analyze a user's objective and generate clarifying questions that will help create a precise implementation plan.

Respond with JSON only:
{
  "questions": ["question 1", "question 2"],
  "initialAnalysis": "Your understanding of what needs to be done",
  "suggestedApproach": "High-level approach you would recommend"
}

Rules:
- Generate 2-5 focused questions about ambiguous requirements, constraints, or preferences
- Questions should be answerable in 1-2 sentences
- Focus on: scope boundaries, technology preferences, testing expectations, risk tolerance
- If the objective is crystal clear and simple, you may return 0 questions
- Your initialAnalysis should demonstrate understanding of the codebase context provided"#
        .to_string();

    let user_prompt = format!(
        "OBJECTIVE:\n{}\n\nPROJECT FILE TREE ({} files):\n{}\n\nGenerate clarifying questions and initial analysis.",
        objective,
        source_files.len(),
        file_tree_summary
    );

    let request = AdapterRequest {
        provider: tier1_model.provider.clone(),
        model_id: tier1_model.model_id.clone(),
        system_prompt,
        user_prompt,
    };

    let llm_result = tokio::task::spawn_blocking(move || llm_adapter::generate(&request))
        .await
        .map_err(|error| format!("LLM task panicked: {error}"))?;

    let response = match llm_result {
        Ok(resp) => resp,
        Err(error) => {
            tasks::update_task_status(
                pool,
                UpdateTaskStatusInput {
                    task_id: root_task.id.clone(),
                    status: TaskStatus::Paused,
                    error_message: Some(format!("LLM analysis failed: {error}")),
                },
            )
            .await?;
            return Err(format!("LLM analysis failed: {error}"));
        }
    };

    let analysis = parse_analysis_response(&response.text)?;

    tasks::update_task_status(
        pool,
        UpdateTaskStatusInput {
            task_id: root_task.id.clone(),
            status: TaskStatus::Paused,
            error_message: Some("analysis_complete: awaiting user answers".to_string()),
        },
    )
    .await?;

    task_runtime::record_task_activity(
        pool,
        "tier1_orchestrator",
        "objective_analysis_completed",
        &root_task.id,
        &format!(
            "questions={} model={}/{}",
            analysis.questions.len(),
            tier1_model.provider,
            tier1_model.model_id
        ),
    )
    .await?;

    Ok(ObjectiveAnalysis {
        root_task_id: root_task.id,
        questions: analysis.questions,
        initial_analysis: analysis.initial_analysis.unwrap_or_default(),
        suggested_approach: analysis.suggested_approach.unwrap_or_default(),
        file_tree_summary,
    })
}

pub async fn generate_plan(
    pool: &SqlitePool,
    model_registry: &ModelRegistry,
    input: GeneratePlanInput,
) -> Result<GeneratedPlan, String> {
    if input.root_task_id.trim().is_empty() {
        return Err("rootTaskId is required".to_string());
    }
    if input.objective.trim().is_empty() {
        return Err("objective is required".to_string());
    }
    if input.target_project.trim().is_empty() {
        return Err("targetProject is required".to_string());
    }
    if input.global_token_budget < 100 {
        return Err("globalTokenBudget must be at least 100".to_string());
    }

    let objective = input.objective.trim().to_string();
    let target_root = normalize_project_root(&input.target_project)?;
    let source_files = collect_source_files(&target_root, 600)?;
    let file_tree_summary = build_file_tree_summary(&source_files, 120);

    let tier1_model = model_intelligence::select_model(
        pool,
        model_registry,
        ModelSelectionRequest {
            task_id: Some(&input.root_task_id),
            actor: "tier1_orchestrator",
            tier: 1,
            persona: None,
            skill: Some("plan_generation"),
        },
    )
    .await?
    .selection;

    tasks::update_task_status(
        pool,
        UpdateTaskStatusInput {
            task_id: input.root_task_id.clone(),
            status: TaskStatus::Executing,
            error_message: None,
        },
    )
    .await?;

    task_runtime::record_task_activity(
        pool,
        "tier1_orchestrator",
        "plan_generation_started",
        &input.root_task_id,
        &format!(
            "objective={} answers={} model={}/{}",
            objective,
            input.answers.len(),
            tier1_model.provider,
            tier1_model.model_id
        ),
    )
    .await?;

    let answers_formatted = if input.answers.is_empty() {
        "No clarifying answers provided (user skipped questions).".to_string()
    } else {
        input
            .answers
            .iter()
            .map(|(q, a)| format!("Q: {q}\nA: {a}"))
            .collect::<Vec<_>>()
            .join("\n\n")
    };

    let system_prompt = build_plan_generation_prompt();
    let user_prompt = format!(
        "OBJECTIVE:\n{}\n\nUSER ANSWERS:\n{}\n\nPROJECT FILE TREE ({} files):\n{}\n\nTOKEN BUDGET: {}\nRISK TOLERANCE: {:.2}\n\nGenerate the implementation plan.",
        objective,
        answers_formatted,
        source_files.len(),
        file_tree_summary,
        input.global_token_budget,
        input.max_risk_tolerance
    );

    let request = AdapterRequest {
        provider: tier1_model.provider.clone(),
        model_id: tier1_model.model_id.clone(),
        system_prompt,
        user_prompt,
    };

    let llm_result = tokio::task::spawn_blocking(move || llm_adapter::generate(&request))
        .await
        .map_err(|error| format!("LLM task panicked: {error}"))?;

    let response = match llm_result {
        Ok(resp) => resp,
        Err(error) => {
            tasks::update_task_status(
                pool,
                UpdateTaskStatusInput {
                    task_id: input.root_task_id.clone(),
                    status: TaskStatus::Failed,
                    error_message: Some(format!("LLM plan generation failed: {error}")),
                },
            )
            .await?;
            return Err(format!("LLM plan generation failed: {error}"));
        }
    };

    let plan = parse_plan_response(&response.text)?;
    if plan.tasks.is_empty() {
        return Err("LLM returned an empty task plan".to_string());
    }

    let overhead_budget = ((input.global_token_budget as f32) * 0.10).round() as u32;
    let reserve_budget = ((input.global_token_budget as f32) * 0.10).round() as u32;
    let distributed_budget = input
        .global_token_budget
        .saturating_sub(overhead_budget + reserve_budget);

    let weights: Vec<f32> = plan
        .tasks
        .iter()
        .map(|t| if t.tier <= 2 { 2.0 } else { 1.0 })
        .collect();
    let budgets = allocate_token_budgets(distributed_budget.max(1), &weights);

    let root_task = tasks::get_task_by_id(pool, &input.root_task_id).await?;
    let mut assignments = Vec::with_capacity(plan.tasks.len());

    for (idx, llm_task) in plan.tasks.iter().enumerate() {
        let domain = normalize_domain(&llm_task.domain);
        let tier = llm_task.tier.clamp(2, 3);
        let risk_factor = estimate_failure_probability(&objective, &llm_task.objective, &domain);
        let constraints = build_constraints(
            &domain,
            risk_factor,
            input.max_risk_tolerance.clamp(0.0, 1.0),
            &objective,
        );

        let target_files_json = if llm_task.target_files.is_empty() {
            None
        } else {
            Some(serde_json::to_string(&llm_task.target_files).unwrap_or_default())
        };
        let created = tasks::create_task_record(
            pool,
            CreateTaskRecordInput {
                parent_id: Some(input.root_task_id.clone()),
                tier: i64::from(tier),
                domain: domain.clone(),
                objective: llm_task.objective.clone(),
                token_budget: budgets[idx].max(1) as i64,
                risk_factor: f64::from(risk_factor),
                status: TaskStatus::Paused,
                target_files: target_files_json,
            },
        )
        .await?;

        task_runtime::record_task_activity(
            pool,
            "tier1_orchestrator",
            "plan_assignment_created",
            &created.id,
            &format!(
                "parent={} tier={} domain={} risk={:.3} budget={} files={} rationale={}",
                input.root_task_id,
                tier,
                domain,
                risk_factor,
                budgets[idx],
                llm_task.target_files.join(","),
                llm_task.rationale.as_deref().unwrap_or("—")
            ),
        )
        .await?;

        assignments.push(TaskAssignment {
            task_id: created.id,
            parent_id: input.root_task_id.clone(),
            tier,
            domain,
            objective: llm_task.objective.clone(),
            token_budget: budgets[idx],
            risk_factor,
            constraints,
            relevant_files: llm_task.target_files.clone(),
        });
    }

    tasks::update_task_status(
        pool,
        UpdateTaskStatusInput {
            task_id: input.root_task_id.clone(),
            status: TaskStatus::Paused,
            error_message: Some(
                "plan_ready: LLM-generated plan awaiting review approval".to_string(),
            ),
        },
    )
    .await?;

    task_runtime::record_task_activity(
        pool,
        "tier1_orchestrator",
        "plan_generation_completed",
        &input.root_task_id,
        &format!(
            "assignments={} model={}/{}",
            assignments.len(),
            tier1_model.provider,
            tier1_model.model_id
        ),
    )
    .await?;

    Ok(GeneratedPlan {
        root_task,
        assignments,
        risk_assessment: plan.risk_assessment.unwrap_or_default(),
        overhead_budget,
        reserve_budget,
        distributed_budget,
    })
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

// --- LLM-driven draft generation with fallback ---

fn generate_drafts_with_llm(
    provider: &str,
    model_id: &str,
    objective: &str,
    domain: &str,
    file_tree: &str,
    token_budget: u32,
    risk_tolerance: f32,
) -> Vec<AssignmentDraft> {
    let system_prompt = build_plan_generation_prompt();
    let user_prompt = format!(
        "OBJECTIVE:\n{}\n\nPROJECT FILE TREE:\n{}\n\nTOKEN BUDGET: {}\nRISK TOLERANCE: {:.2}\n\nGenerate the implementation plan.",
        objective, file_tree, token_budget, risk_tolerance
    );

    let request = AdapterRequest {
        provider: provider.to_string(),
        model_id: model_id.to_string(),
        system_prompt,
        user_prompt,
    };

    match llm_adapter::generate(&request) {
        Ok(response) => {
            if let Ok(plan) = parse_plan_response(&response.text) {
                if !plan.tasks.is_empty() {
                    return plan
                        .tasks
                        .iter()
                        .take(6)
                        .map(|t| AssignmentDraft {
                            tier: t.tier.clamp(2, 3),
                            domain: normalize_domain(&t.domain),
                            objective: t.objective.clone(),
                            target_files: t.target_files.clone(),
                        })
                        .collect();
                }
            }
        }
        Err(_) => {}
    }

    build_simple_fallback_drafts(domain, objective)
}

fn build_simple_fallback_drafts(domain: &str, objective: &str) -> Vec<AssignmentDraft> {
    let objective_lower = objective.to_ascii_lowercase();
    let is_complex = contains_any(
        &objective_lower,
        &["refactor", "rewrite", "migrate", "overhaul", "architecture"],
    );

    let mut drafts = Vec::new();

    if is_complex {
        drafts.push(AssignmentDraft {
            tier: 2,
            domain: domain.to_string(),
            objective: format!("Coordinate and plan: {objective}"),
            target_files: Vec::new(),
        });
    }

    drafts.push(AssignmentDraft {
        tier: 3,
        domain: domain.to_string(),
        objective: format!("Implement core changes: {objective}"),
        target_files: Vec::new(),
    });

    drafts.push(AssignmentDraft {
        tier: 3,
        domain: "testing".to_string(),
        objective: format!("Add tests for: {objective}"),
        target_files: Vec::new(),
    });

    if is_complex {
        drafts.push(AssignmentDraft {
            tier: 3,
            domain: domain.to_string(),
            objective: format!("Apply follow-up fixes: {objective}"),
            target_files: Vec::new(),
        });
    }

    drafts
}

fn build_plan_generation_prompt() -> String {
    r#"You are a Tier-1 orchestrator for the Autonomous Orchestration Platform (AOP).
Generate a concrete implementation plan broken into tasks that can be executed by Tier-2 domain leaders and Tier-3 specialists.

Respond with JSON only:
{
  "tasks": [
    {
      "objective": "what this task should accomplish",
      "domain": "frontend|backend|auth|database|api|testing|platform",
      "tier": 2 or 3,
      "targetFiles": ["file/path1.ts", "file/path2.tsx"],
      "rationale": "why this task is needed"
    }
  ],
  "riskAssessment": "overall risk analysis and mitigation notes"
}

Rules:
- Generate 2-6 tasks. Fewer for simple objectives, more for complex ones.
- Tier 2 tasks are for domain leaders who coordinate complex multi-file changes.
- Tier 3 tasks are for specialists who make focused single-file changes.
- Use tier 2 only for tasks that genuinely need coordination across multiple files.
- Simple, focused changes should be tier 3.
- targetFiles should be real paths from the file tree provided.
- Order tasks by dependency (independent tasks first, dependent tasks last).
- Each task objective must be specific and actionable, not vague."#
        .to_string()
}

fn build_file_tree_summary(files: &[String], max_entries: usize) -> String {
    let mut summary = String::new();
    for (i, file) in files.iter().enumerate() {
        if i >= max_entries {
            summary.push_str(&format!(
                "... and {} more files\n",
                files.len() - max_entries
            ));
            break;
        }
        summary.push_str(file);
        summary.push('\n');
    }
    summary
}

fn normalize_domain(domain: &str) -> String {
    match domain.trim().to_ascii_lowercase().as_str() {
        "frontend" | "ui" | "react" | "vue" | "angular" => "frontend".to_string(),
        "backend" | "server" => "api".to_string(),
        "api" | "rest" | "graphql" => "api".to_string(),
        "database" | "db" | "sql" | "migration" => "database".to_string(),
        "auth" | "authentication" | "authorization" | "security" => "auth".to_string(),
        "testing" | "test" | "qa" | "e2e" | "unit" => "testing".to_string(),
        _ => "platform".to_string(),
    }
}

fn strip_code_fences_orch(text: &str) -> &str {
    let trimmed = text.trim();
    if let Some(rest) = trimmed.strip_prefix("```json") {
        if let Some(inner) = rest.strip_suffix("```") {
            return inner.trim();
        }
    }
    if let Some(rest) = trimmed.strip_prefix("```") {
        if let Some(inner) = rest.strip_suffix("```") {
            return inner.trim();
        }
    }
    trimmed
}

fn parse_analysis_response(text: &str) -> Result<LlmAnalysisResponse, String> {
    let cleaned = strip_code_fences_orch(text);
    serde_json::from_str::<LlmAnalysisResponse>(cleaned)
        .map_err(|error| format!("Failed to parse LLM analysis response: {error}\nRaw: {text}"))
}

fn parse_plan_response(text: &str) -> Result<LlmPlanResponse, String> {
    let cleaned = strip_code_fences_orch(text);
    serde_json::from_str::<LlmPlanResponse>(cleaned)
        .map_err(|error| format!("Failed to parse LLM plan response: {error}\nRaw: {text}"))
}

fn contains_any(value: &str, patterns: &[&str]) -> bool {
    patterns.iter().any(|pattern| value.contains(pattern))
}

fn normalize_project_root(target_project: &str) -> Result<PathBuf, String> {
    let root = PathBuf::from(target_project.trim());
    let normalized = strip_unc_prefix(
        fs::canonicalize(root)
            .map_err(|error| format!("Unable to resolve target project path: {error}"))?,
    );

    if !normalized.is_dir() {
        return Err(format!(
            "Target project path '{}' is not a directory",
            normalized.display()
        ));
    }

    Ok(normalized)
}

/// Strip the Windows extended-length path prefix (`\\?\`) that `fs::canonicalize` adds.
/// Git and most external tools cannot handle UNC paths.
fn strip_unc_prefix(path: PathBuf) -> PathBuf {
    let s = path.to_string_lossy();
    if let Some(stripped) = s.strip_prefix(r"\\?\") {
        PathBuf::from(stripped)
    } else {
        path
    }
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

        assert!((2..=6).contains(&result.assignments.len()));
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
