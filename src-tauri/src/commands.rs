use tauri::State;

use crate::agents::domain_leader::{self, ExecuteDomainTaskInput, IntentSummary};
use crate::agents::orchestrator::{
    self, ApproveOrchestrationPlanInput, OrchestrationResult, PlanExecutionResult,
    UserObjectiveInput,
};
use crate::db::budget_requests::{
    self, BudgetRequestRecord, CreateBudgetRequestInput, ListTaskBudgetRequestsInput,
    ResolveBudgetRequestInput,
};
use crate::db::metrics::{
    self, AgentTerminalSession, AuditLogEntry, ListAgentTerminalsInput, ListAuditLogInput,
    ListTaskActivityInput, ListTerminalEventsInput, TerminalEventRecord,
};
use crate::db::mutations::{
    self, ListTaskMutationsInput, MutationRecord, UpdateMutationStatusInput,
};
use crate::db::tasks::{
    self, ControlTaskInput, CreateTaskInput, TaskRecord, UpdateTaskStatusInput,
};
use crate::mcp_bridge::tool_caller::{
    self, DirectoryListing, ListTargetDirInput, ReadTargetFileInput, SearchResult,
    SearchTargetFilesInput, TargetFileContent,
};
use crate::model_registry::ModelRegistrySnapshot;
use crate::mutation_pipeline::{self, MutationPipelineResult, RunMutationPipelineInput};
use crate::mutation_revision::{self, MutationRevisionResult, RequestMutationRevisionInput};
use crate::vector::indexer;
use crate::vector::search;
use crate::vector::{ContextChunk, IndexProjectInput, IndexProjectResult, QueryCodebaseInput};
use crate::AppState;

#[tauri::command]
pub async fn create_task(
    state: State<'_, AppState>,
    input: CreateTaskInput,
) -> Result<TaskRecord, String> {
    tasks::create_task(&state.db_pool, input).await
}

#[tauri::command]
pub async fn get_tasks(state: State<'_, AppState>) -> Result<Vec<TaskRecord>, String> {
    tasks::get_tasks(&state.db_pool).await
}

#[tauri::command]
pub async fn update_task_status(
    state: State<'_, AppState>,
    input: UpdateTaskStatusInput,
) -> Result<TaskRecord, String> {
    tasks::update_task_status(&state.db_pool, input).await
}

#[tauri::command]
pub async fn control_task(
    state: State<'_, AppState>,
    input: ControlTaskInput,
) -> Result<Vec<TaskRecord>, String> {
    let action = input.action.as_str().to_string();
    let task_id = input.task_id.clone();
    let include_descendants = input.include_descendants.unwrap_or(true);
    let updated = tasks::control_task(&state.db_pool, input).await?;

    metrics::record_audit_event(
        &state.db_pool,
        "ui",
        "task_control",
        Some(task_id.as_str()),
        Some(&format!(
            "{{\"action\":\"{}\",\"updated\":{},\"cascade\":{}}}",
            action,
            updated.len(),
            include_descendants
        )),
    )
    .await?;

    Ok(updated)
}

#[tauri::command]
pub async fn request_task_budget_increase(
    state: State<'_, AppState>,
    input: CreateBudgetRequestInput,
) -> Result<BudgetRequestRecord, String> {
    let requested_increment = input.requested_increment;
    let requested_by = input.requested_by.clone();
    let auto_approve = input.auto_approve.unwrap_or(false);
    let request = budget_requests::create_budget_request(&state.db_pool, input).await?;
    let action = if request.status == "approved" {
        "task_budget_auto_approved"
    } else {
        "task_budget_requested"
    };

    metrics::record_audit_event(
        &state.db_pool,
        requested_by.as_str(),
        action,
        Some(request.task_id.as_str()),
        Some(&format!(
            "{{\"requestId\":\"{}\",\"requestedIncrement\":{},\"status\":\"{}\",\"autoApprove\":{}}}",
            request.id, requested_increment, request.status, auto_approve
        )),
    )
    .await?;

    Ok(request)
}

#[tauri::command]
pub async fn list_task_budget_requests(
    state: State<'_, AppState>,
    input: ListTaskBudgetRequestsInput,
) -> Result<Vec<BudgetRequestRecord>, String> {
    budget_requests::list_task_budget_requests(&state.db_pool, input).await
}

#[tauri::command]
pub async fn resolve_task_budget_request(
    state: State<'_, AppState>,
    input: ResolveBudgetRequestInput,
) -> Result<BudgetRequestRecord, String> {
    let decision = input.decision.as_str().to_string();
    let decided_by = input
        .decided_by
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("ui")
        .to_string();
    let resolved = budget_requests::resolve_budget_request(&state.db_pool, input).await?;

    metrics::record_audit_event(
        &state.db_pool,
        decided_by.as_str(),
        "task_budget_request_resolved",
        Some(resolved.task_id.as_str()),
        Some(&format!(
            "{{\"requestId\":\"{}\",\"decision\":\"{}\",\"status\":\"{}\",\"approvedIncrement\":{}}}",
            resolved.id,
            decision,
            resolved.status,
            resolved.approved_increment.unwrap_or(0)
        )),
    )
    .await?;

    Ok(resolved)
}

#[tauri::command]
pub async fn orchestrate_objective(
    state: State<'_, AppState>,
    input: UserObjectiveInput,
) -> Result<OrchestrationResult, String> {
    orchestrator::orchestrate_and_persist(&state.db_pool, &state.model_registry, input).await
}

#[tauri::command]
pub async fn approve_orchestration_plan(
    state: State<'_, AppState>,
    input: ApproveOrchestrationPlanInput,
) -> Result<PlanExecutionResult, String> {
    orchestrator::approve_plan_and_spawn(
        &state.db_pool,
        &state.bridge_client,
        &state.model_registry,
        input,
    )
    .await
}

#[tauri::command]
pub async fn execute_domain_task(
    state: State<'_, AppState>,
    input: ExecuteDomainTaskInput,
) -> Result<IntentSummary, String> {
    domain_leader::execute_domain_task(
        &state.db_pool,
        &state.bridge_client,
        &state.model_registry,
        input,
    )
    .await
}

#[tauri::command]
pub async fn list_task_mutations(
    state: State<'_, AppState>,
    input: ListTaskMutationsInput,
) -> Result<Vec<MutationRecord>, String> {
    mutations::list_mutations_for_task(&state.db_pool, input).await
}

#[tauri::command]
pub async fn run_mutation_pipeline(
    state: State<'_, AppState>,
    input: RunMutationPipelineInput,
) -> Result<MutationPipelineResult, String> {
    mutation_pipeline::run_mutation_pipeline(&state.db_pool, input).await
}

#[tauri::command]
pub async fn set_mutation_status(
    state: State<'_, AppState>,
    input: UpdateMutationStatusInput,
) -> Result<MutationRecord, String> {
    let updated = mutations::update_mutation_status(&state.db_pool, input).await?;
    metrics::record_audit_event(
        &state.db_pool,
        "ui",
        "mutation_status_changed",
        Some(updated.id.as_str()),
        Some(&format!("{{\"status\":\"{}\"}}", updated.status)),
    )
    .await?;
    Ok(updated)
}

#[tauri::command]
pub async fn request_mutation_revision(
    state: State<'_, AppState>,
    input: RequestMutationRevisionInput,
) -> Result<MutationRevisionResult, String> {
    mutation_revision::request_mutation_revision(&state.db_pool, &state.model_registry, input).await
}

#[tauri::command]
pub async fn list_audit_log(
    state: State<'_, AppState>,
    input: ListAuditLogInput,
) -> Result<Vec<AuditLogEntry>, String> {
    metrics::list_audit_log(&state.db_pool, input).await
}

#[tauri::command]
pub async fn list_task_activity(
    state: State<'_, AppState>,
    input: ListTaskActivityInput,
) -> Result<Vec<AuditLogEntry>, String> {
    metrics::list_task_activity(&state.db_pool, input).await
}

#[tauri::command]
pub async fn list_agent_terminals(
    state: State<'_, AppState>,
    input: ListAgentTerminalsInput,
) -> Result<Vec<AgentTerminalSession>, String> {
    metrics::list_agent_terminals(&state.db_pool, input).await
}

#[tauri::command]
pub async fn list_terminal_events(
    state: State<'_, AppState>,
    input: ListTerminalEventsInput,
) -> Result<Vec<TerminalEventRecord>, String> {
    metrics::list_terminal_events(&state.db_pool, input).await
}

#[tauri::command]
pub async fn get_default_target_project() -> Result<String, String> {
    std::env::current_dir()
        .map(|path| path.to_string_lossy().to_string())
        .map_err(|error| format!("Unable to resolve current directory: {error}"))
}

#[tauri::command]
pub async fn list_target_dir(
    state: State<'_, AppState>,
    input: ListTargetDirInput,
) -> Result<DirectoryListing, String> {
    tool_caller::list_dir(&state.bridge_client, input).await
}

#[tauri::command]
pub async fn read_target_file(
    state: State<'_, AppState>,
    input: ReadTargetFileInput,
) -> Result<TargetFileContent, String> {
    tool_caller::read_file(&state.bridge_client, input).await
}

#[tauri::command]
pub async fn search_target_files(
    state: State<'_, AppState>,
    input: SearchTargetFilesInput,
) -> Result<SearchResult, String> {
    tool_caller::search_files(&state.bridge_client, input).await
}

#[tauri::command]
pub async fn index_target_project(
    state: State<'_, AppState>,
    input: IndexProjectInput,
) -> Result<IndexProjectResult, String> {
    indexer::index_project(&state.db_pool, &input.target_project).await
}

#[tauri::command]
pub async fn query_codebase(
    state: State<'_, AppState>,
    input: QueryCodebaseInput,
) -> Result<Vec<ContextChunk>, String> {
    search::query_codebase(
        &state.db_pool,
        &input.target_project,
        &input.query,
        input.top_k.unwrap_or(5),
    )
    .await
}

#[tauri::command]
pub async fn get_model_registry(
    state: State<'_, AppState>,
) -> Result<ModelRegistrySnapshot, String> {
    Ok(state.model_registry.snapshot())
}
