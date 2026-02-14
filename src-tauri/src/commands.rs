use tauri::State;

use crate::agents::domain_leader::{self, ExecuteDomainTaskInput, IntentSummary};
use crate::agents::orchestrator::{
    self, AnalyzeObjectiveInput, ApproveOrchestrationPlanInput, GeneratePlanInput, GeneratedPlan,
    ObjectiveAnalysis, OrchestrationResult, PlanExecutionResult, UserObjectiveInput,
};
use crate::db::budget_requests::{
    self, BudgetRequestRecord, CreateBudgetRequestInput, ListTaskBudgetRequestsInput,
    ResolveBudgetRequestInput,
};
use crate::db::metrics::{
    self, AgentTerminalSession, AuditLogEntry, ListAgentTerminalsInput, ListAuditLogInput,
    ListTaskActivityInput, ListTerminalEventsInput, TerminalEventRecord,
};
use crate::db::telemetry::{
    self, AgentEventRecord, AgentRunRecord, ArchiveTelemetryInput, ArchiveTelemetryResult,
    GetMissionControlSnapshotInput, ListAgentEventsInput, ListAgentRunsInput,
    MissionControlSnapshot,
};
use crate::db::mutations::{
    self, ListTaskMutationsInput, MutationRecord, UpdateMutationStatusInput,
};
use crate::db::tasks::{
    self, ControlTaskInput, CreateTaskInput, TaskControlAction, TaskRecord, UpdateTaskStatusInput,
};
use crate::mcp_bridge::tool_caller::{
    self, DirectoryListing, ListTargetDirInput, ReadTargetFileInput, SearchResult,
    SearchTargetFilesInput, TargetFileContent,
};
use crate::model_registry::ModelRegistrySnapshot;
use crate::mutation_pipeline::{self, MutationPipelineResult, RunMutationPipelineInput};
use crate::mutation_revision::{self, MutationRevisionResult, RequestMutationRevisionInput};
use crate::runtime_config::{RuntimeFlags, RuntimeFlagsUpdateResult, SetRuntimeFlagsInput};
use crate::secret_vault::{
    GetProviderSecretStatusInput, ProviderSecretStatus, RevealProviderSecretInput,
    RevealProviderSecretResult, SecretOperationResult, SetProviderSecretInput,
};
use crate::vector::indexer;
use crate::vector::search;
use crate::vector::{ContextChunk, IndexProjectInput, IndexProjectResult, QueryCodebaseInput};
use crate::AppState;
use serde::Deserialize;
use std::time::Instant;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlExecutionScopeInput {
    pub root_task_id: String,
    pub action: String,
    pub scope_type: String,
    pub tier: Option<i64>,
    pub agent_task_id: Option<String>,
    pub reason: Option<String>,
}

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
pub async fn analyze_objective(
    state: State<'_, AppState>,
    input: AnalyzeObjectiveInput,
) -> Result<ObjectiveAnalysis, String> {
    orchestrator::analyze_objective(&state.db_pool, &state.model_registry, input).await
}

#[tauri::command]
pub async fn submit_answers_and_plan(
    state: State<'_, AppState>,
    input: GeneratePlanInput,
) -> Result<GeneratedPlan, String> {
    orchestrator::generate_plan(&state.db_pool, &state.model_registry, input).await
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
    let mcp_server = input
        .mcp_command
        .clone()
        .unwrap_or_else(|| "local".to_string());
    let tool = "list_dir";
    let started_at = Instant::now();
    let result = tool_caller::list_dir(&state.bridge_client, input.clone()).await;
    let elapsed = started_at.elapsed().as_millis() as i64;

    let (status, message, payload) = match &result {
        Ok(value) => (
            "completed",
            format!("source={} entries={}", value.source, value.entries.len()),
            serde_json::json!({
                "tool": tool,
                "source": value.source,
                "warnings": sanitize_mcp_warnings(&value.warnings),
            }),
        ),
        Err(error) => (
            "failed",
            error.clone(),
            serde_json::json!({
                "tool": tool,
                "error": sanitize_mcp_text(error),
            }),
        ),
    };

    let _ = telemetry::record_agent_event(
        &state.db_pool,
        telemetry::NewAgentEvent {
            actor: "mcp_bridge".to_string(),
            action: "mcp_call".to_string(),
            status: Some(status.to_string()),
            phase: Some("io".to_string()),
            message: Some(message),
            mcp_server: Some(sanitize_mcp_text(&mcp_server)),
            mcp_tool: Some(tool.to_string()),
            latency_ms: Some(elapsed),
            payload: Some(payload),
            ..Default::default()
        },
    )
    .await;

    result
}

#[tauri::command]
pub async fn read_target_file(
    state: State<'_, AppState>,
    input: ReadTargetFileInput,
) -> Result<TargetFileContent, String> {
    let mcp_server = input
        .mcp_command
        .clone()
        .unwrap_or_else(|| "local".to_string());
    let tool = "read_file";
    let started_at = Instant::now();
    let result = tool_caller::read_file(&state.bridge_client, input.clone()).await;
    let elapsed = started_at.elapsed().as_millis() as i64;

    let (status, message, payload) = match &result {
        Ok(value) => (
            "completed",
            format!("source={} path={} size={}", value.source, value.path, value.size),
            serde_json::json!({
                "tool": tool,
                "path": sanitize_mcp_text(&value.path),
                "source": value.source,
                "warnings": sanitize_mcp_warnings(&value.warnings),
            }),
        ),
        Err(error) => (
            "failed",
            error.clone(),
            serde_json::json!({
                "tool": tool,
                "error": sanitize_mcp_text(error),
            }),
        ),
    };

    let _ = telemetry::record_agent_event(
        &state.db_pool,
        telemetry::NewAgentEvent {
            actor: "mcp_bridge".to_string(),
            action: "mcp_call".to_string(),
            status: Some(status.to_string()),
            phase: Some("io".to_string()),
            message: Some(message),
            mcp_server: Some(sanitize_mcp_text(&mcp_server)),
            mcp_tool: Some(tool.to_string()),
            latency_ms: Some(elapsed),
            payload: Some(payload),
            ..Default::default()
        },
    )
    .await;

    result
}

#[tauri::command]
pub async fn search_target_files(
    state: State<'_, AppState>,
    input: SearchTargetFilesInput,
) -> Result<SearchResult, String> {
    let mcp_server = input
        .mcp_command
        .clone()
        .unwrap_or_else(|| "local".to_string());
    let tool = "search_files";
    let started_at = Instant::now();
    let result = tool_caller::search_files(&state.bridge_client, input.clone()).await;
    let elapsed = started_at.elapsed().as_millis() as i64;

    let (status, message, payload) = match &result {
        Ok(value) => (
            "completed",
            format!(
                "source={} pattern={} matches={}",
                value.source, value.pattern, value.matches.len()
            ),
            serde_json::json!({
                "tool": tool,
                "pattern": sanitize_mcp_text(&value.pattern),
                "source": value.source,
                "warnings": sanitize_mcp_warnings(&value.warnings),
            }),
        ),
        Err(error) => (
            "failed",
            error.clone(),
            serde_json::json!({
                "tool": tool,
                "error": sanitize_mcp_text(error),
            }),
        ),
    };

    let _ = telemetry::record_agent_event(
        &state.db_pool,
        telemetry::NewAgentEvent {
            actor: "mcp_bridge".to_string(),
            action: "mcp_call".to_string(),
            status: Some(status.to_string()),
            phase: Some("io".to_string()),
            message: Some(message),
            mcp_server: Some(sanitize_mcp_text(&mcp_server)),
            mcp_tool: Some(tool.to_string()),
            latency_ms: Some(elapsed),
            payload: Some(payload),
            ..Default::default()
        },
    )
    .await;

    result
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

#[tauri::command]
pub async fn list_agent_runs(
    state: State<'_, AppState>,
    input: ListAgentRunsInput,
) -> Result<Vec<AgentRunRecord>, String> {
    telemetry::list_agent_runs(&state.db_pool, input).await
}

#[tauri::command]
pub async fn list_agent_events(
    state: State<'_, AppState>,
    input: ListAgentEventsInput,
) -> Result<Vec<AgentEventRecord>, String> {
    telemetry::list_agent_events(&state.db_pool, input).await
}

#[tauri::command]
pub async fn get_mission_control_snapshot(
    state: State<'_, AppState>,
    input: GetMissionControlSnapshotInput,
) -> Result<MissionControlSnapshot, String> {
    telemetry::get_mission_control_snapshot(&state.db_pool, input).await
}

#[tauri::command]
pub async fn control_execution_scope(
    state: State<'_, AppState>,
    input: ControlExecutionScopeInput,
) -> Result<Vec<TaskRecord>, String> {
    let root_task_id = input.root_task_id.trim();
    if root_task_id.is_empty() {
        return Err("rootTaskId is required".to_string());
    }

    let action = parse_task_control_action(input.action.as_str())?;
    let tree_ids = tasks::collect_task_tree_ids(&state.db_pool, root_task_id).await?;
    if tree_ids.is_empty() {
        return Err(format!("Task tree for '{}' is empty", root_task_id));
    }

    let scope = input.scope_type.trim().to_ascii_lowercase();
    let target_ids = match scope.as_str() {
        "tree" => tree_ids,
        "tier" => {
            let target_tier = input
                .tier
                .ok_or_else(|| "tier is required when scopeType='tier'".to_string())?;
            let mut ids = Vec::new();
            for task_id in tree_ids {
                let task = tasks::get_task_by_id(&state.db_pool, task_id.as_str()).await?;
                if task.tier == target_tier {
                    ids.push(task.id);
                }
            }
            ids
        }
        "agent" => {
            let task_id = input
                .agent_task_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| "agentTaskId is required when scopeType='agent'".to_string())?;
            if !tree_ids.iter().any(|value| value == task_id) {
                return Err(format!(
                    "agentTaskId '{}' does not belong to rootTaskId '{}'",
                    task_id, root_task_id
                ));
            }
            vec![task_id.to_string()]
        }
        _ => {
            return Err("scopeType must be 'tree', 'tier', or 'agent'".to_string());
        }
    };

    if target_ids.is_empty() {
        return Err(format!(
            "No tasks matched scopeType='{}' under rootTaskId='{}'",
            scope, root_task_id
        ));
    }

    let mut updated = Vec::new();
    for task_id in target_ids {
        let result = tasks::control_task(
            &state.db_pool,
            ControlTaskInput {
                task_id: task_id.clone(),
                action: action.clone(),
                include_descendants: Some(false),
                reason: input.reason.clone(),
            },
        )
        .await;

        match result {
            Ok(mut records) => updated.append(&mut records),
            Err(error) => {
                if !error.contains("No tasks were updated") {
                    return Err(error);
                }
            }
        }
    }

    if updated.is_empty() {
        return Err(format!(
            "No tasks were updated for action '{}' on scope '{}'",
            action.as_str(),
            scope
        ));
    }

    let details = serde_json::json!({
        "action": action.as_str(),
        "scopeType": scope,
        "updated": updated.len(),
        "tier": input.tier,
        "agentTaskId": input.agent_task_id,
    })
    .to_string();
    metrics::record_audit_event(
        &state.db_pool,
        "ui",
        "execution_scope_control",
        Some(root_task_id),
        Some(details.as_str()),
    )
    .await?;

    Ok(updated)
}

#[tauri::command]
pub async fn get_runtime_flags(state: State<'_, AppState>) -> Result<RuntimeFlags, String> {
    state
        .runtime_flags
        .read()
        .map(|flags| flags.clone())
        .map_err(|error| format!("Failed to read runtime flags: {error}"))
}

#[tauri::command]
pub async fn set_runtime_flags(
    state: State<'_, AppState>,
    input: SetRuntimeFlagsInput,
) -> Result<RuntimeFlagsUpdateResult, String> {
    let mut guard = state
        .runtime_flags
        .write()
        .map_err(|error| format!("Failed to update runtime flags: {error}"))?;
    guard.apply_update(input);
    guard.sync_to_process_env();
    Ok(RuntimeFlagsUpdateResult {
        flags: guard.clone(),
        restart_required: false,
    })
}

#[tauri::command]
pub async fn get_provider_secret_status(
    state: State<'_, AppState>,
    input: GetProviderSecretStatusInput,
) -> Result<ProviderSecretStatus, String> {
    let developer_mode = state
        .runtime_flags
        .read()
        .map(|flags| flags.dev_mode)
        .unwrap_or(false);
    let mut vault = state.secret_vault.lock().await;
    vault.get_status(input.provider.as_str(), developer_mode)
}

#[tauri::command]
pub async fn set_provider_secret(
    state: State<'_, AppState>,
    input: SetProviderSecretInput,
) -> Result<SecretOperationResult, String> {
    let developer_mode = state
        .runtime_flags
        .read()
        .map(|flags| flags.dev_mode)
        .unwrap_or(false);
    let mut vault = state.secret_vault.lock().await;
    vault.set_secret(
        input.provider.as_str(),
        input.secret.as_str(),
        developer_mode,
        input.session_token.as_deref(),
    )
}

#[tauri::command]
pub async fn reveal_provider_secret(
    state: State<'_, AppState>,
    input: RevealProviderSecretInput,
) -> Result<RevealProviderSecretResult, String> {
    let developer_mode = state
        .runtime_flags
        .read()
        .map(|flags| flags.dev_mode)
        .unwrap_or(false);
    let mut vault = state.secret_vault.lock().await;
    vault.reveal_secret(
        input.provider.as_str(),
        developer_mode,
        input.session_token.as_deref(),
    )
}

#[tauri::command]
pub async fn archive_telemetry(
    state: State<'_, AppState>,
    input: ArchiveTelemetryInput,
) -> Result<ArchiveTelemetryResult, String> {
    let fallback_retention = state
        .runtime_flags
        .read()
        .map(|flags| flags.telemetry_retention_days)
        .unwrap_or(7);
    telemetry::archive_telemetry(
        &state.db_pool,
        state.app_data_dir.join("telemetry-archive").as_path(),
        input.retention_days.unwrap_or(fallback_retention),
    )
    .await
}

fn parse_task_control_action(action: &str) -> Result<TaskControlAction, String> {
    match action.trim().to_ascii_lowercase().as_str() {
        "pause" => Ok(TaskControlAction::Pause),
        "resume" => Ok(TaskControlAction::Resume),
        "stop" => Ok(TaskControlAction::Stop),
        "restart" => Ok(TaskControlAction::Restart),
        _ => Err("action must be pause|resume|stop|restart".to_string()),
    }
}

fn sanitize_mcp_warnings(values: &[String]) -> Vec<String> {
    values.iter().map(|value| sanitize_mcp_text(value)).collect()
}

fn sanitize_mcp_text(value: &str) -> String {
    let sensitive_markers = [
        "api_key",
        "apikey",
        "token",
        "secret",
        "password",
        "authorization",
        "cookie",
    ];
    let mut sanitized = value.to_string();
    for marker in sensitive_markers {
        if sanitized.to_ascii_lowercase().contains(marker) {
            sanitized = sanitized
                .split_whitespace()
                .map(|part| {
                    if part.to_ascii_lowercase().contains(marker) {
                        format!("{marker}=***")
                    } else {
                        part.to_string()
                    }
                })
                .collect::<Vec<_>>()
                .join(" ");
        }
    }
    let max_len = 512;
    if sanitized.chars().count() > max_len {
        let truncated = sanitized.chars().take(max_len).collect::<String>();
        return format!("{truncated}...[truncated]");
    }
    sanitized
}
