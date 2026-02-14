import { invoke } from '@tauri-apps/api/core'

import type {
  AgentEventRecord,
  AgentRunRecord,
  AgentTerminalSession,
  AnalyzeObjectiveInput,
  ArchiveTelemetryInput,
  ArchiveTelemetryResult,
  ApproveOrchestrationPlanInput,
  AuditLogEntry,
  BudgetRequestRecord,
  ControlExecutionScopeInput,
  CreateTaskInput,
  ContextChunk,
  ControlTaskInput,
  DirectoryListing,
  ExecuteDomainTaskInput,
  GeneratePlanInput,
  GeneratedPlan,
  IndexProjectResult,
  IndexTargetProjectInput,
  IntentSummary,
  ListAuditLogInput,
  ListAgentTerminalsInput,
  ListAgentEventsInput,
  ListAgentRunsInput,
  ListTaskBudgetRequestsInput,
  ListTaskActivityInput,
  ListTerminalEventsInput,
  ListTargetDirInput,
  ListTaskMutationsInput,
  MutationPipelineResult,
  MutationRecord,
  ModelRegistrySnapshot,
  MissionControlSnapshot,
  MutationRevisionResult,
  ObjectiveAnalysis,
  OrchestrationResult,
  PlanExecutionResult,
  QueryCodebaseInput,
  ReadTargetFileInput,
  RequestTaskBudgetIncreaseInput,
  RequestMutationRevisionInput,
  ResolveTaskBudgetRequestInput,
  RunMutationPipelineInput,
  SearchResult,
  SetProviderSecretInput,
  SecretOperationResult,
  RevealProviderSecretInput,
  RevealProviderSecretResult,
  RuntimeFlags,
  RuntimeFlagsUpdateResult,
  SetRuntimeFlagsInput,
  GetProviderSecretStatusInput,
  ProviderSecretStatus,
  GetMissionControlSnapshotInput,
  SearchTargetFilesInput,
  SetMutationStatusInput,
  TerminalEventRecord,
  TargetFileContent,
  TaskRecord,
  UserObjectiveInput,
  UpdateTaskStatusInput,
} from '@/types'

export async function getTasks(): Promise<TaskRecord[]> {
  return invoke<TaskRecord[]>('get_tasks')
}

export async function createTask(input: CreateTaskInput): Promise<TaskRecord> {
  return invoke<TaskRecord>('create_task', { input })
}

export async function updateTaskStatus(input: UpdateTaskStatusInput): Promise<TaskRecord> {
  return invoke<TaskRecord>('update_task_status', { input })
}

export async function controlTask(input: ControlTaskInput): Promise<TaskRecord[]> {
  return invoke<TaskRecord[]>('control_task', { input })
}

export async function requestTaskBudgetIncrease(input: RequestTaskBudgetIncreaseInput): Promise<BudgetRequestRecord> {
  return invoke<BudgetRequestRecord>('request_task_budget_increase', { input })
}

export async function listTaskBudgetRequests(input: ListTaskBudgetRequestsInput): Promise<BudgetRequestRecord[]> {
  return invoke<BudgetRequestRecord[]>('list_task_budget_requests', { input })
}

export async function resolveTaskBudgetRequest(input: ResolveTaskBudgetRequestInput): Promise<BudgetRequestRecord> {
  return invoke<BudgetRequestRecord>('resolve_task_budget_request', { input })
}

export async function orchestrateObjective(input: UserObjectiveInput): Promise<OrchestrationResult> {
  return invoke<OrchestrationResult>('orchestrate_objective', { input })
}

export async function analyzeObjective(input: AnalyzeObjectiveInput): Promise<ObjectiveAnalysis> {
  return invoke<ObjectiveAnalysis>('analyze_objective', { input })
}

export async function submitAnswersAndPlan(input: GeneratePlanInput): Promise<GeneratedPlan> {
  return invoke<GeneratedPlan>('submit_answers_and_plan', { input })
}

export async function approveOrchestrationPlan(input: ApproveOrchestrationPlanInput): Promise<PlanExecutionResult> {
  return invoke<PlanExecutionResult>('approve_orchestration_plan', { input })
}

export async function executeDomainTask(input: ExecuteDomainTaskInput): Promise<IntentSummary> {
  return invoke<IntentSummary>('execute_domain_task', { input })
}

export async function listTaskMutations(input: ListTaskMutationsInput): Promise<MutationRecord[]> {
  return invoke<MutationRecord[]>('list_task_mutations', { input })
}

export async function runMutationPipeline(input: RunMutationPipelineInput): Promise<MutationPipelineResult> {
  return invoke<MutationPipelineResult>('run_mutation_pipeline', { input })
}

export async function setMutationStatus(input: SetMutationStatusInput): Promise<MutationRecord> {
  return invoke<MutationRecord>('set_mutation_status', { input })
}

export async function requestMutationRevision(input: RequestMutationRevisionInput): Promise<MutationRevisionResult> {
  return invoke<MutationRevisionResult>('request_mutation_revision', { input })
}

export async function listAuditLog(input: ListAuditLogInput): Promise<AuditLogEntry[]> {
  return invoke<AuditLogEntry[]>('list_audit_log', { input })
}

export async function listTaskActivity(input: ListTaskActivityInput): Promise<AuditLogEntry[]> {
  return invoke<AuditLogEntry[]>('list_task_activity', { input })
}

export async function listAgentTerminals(input: ListAgentTerminalsInput): Promise<AgentTerminalSession[]> {
  return invoke<AgentTerminalSession[]>('list_agent_terminals', { input })
}

export async function listTerminalEvents(input: ListTerminalEventsInput): Promise<TerminalEventRecord[]> {
  return invoke<TerminalEventRecord[]>('list_terminal_events', { input })
}

export async function getDefaultTargetProject(): Promise<string> {
  return invoke<string>('get_default_target_project')
}

export async function listTargetDir(input: ListTargetDirInput): Promise<DirectoryListing> {
  return invoke<DirectoryListing>('list_target_dir', { input })
}

export async function readTargetFile(input: ReadTargetFileInput): Promise<TargetFileContent> {
  return invoke<TargetFileContent>('read_target_file', { input })
}

export async function searchTargetFiles(input: SearchTargetFilesInput): Promise<SearchResult> {
  return invoke<SearchResult>('search_target_files', { input })
}

export async function indexTargetProject(input: IndexTargetProjectInput): Promise<IndexProjectResult> {
  return invoke<IndexProjectResult>('index_target_project', { input })
}

export async function queryCodebase(input: QueryCodebaseInput): Promise<ContextChunk[]> {
  return invoke<ContextChunk[]>('query_codebase', { input })
}

export async function getModelRegistry(): Promise<ModelRegistrySnapshot> {
  return invoke<ModelRegistrySnapshot>('get_model_registry')
}

export async function getMissionControlSnapshot(input: GetMissionControlSnapshotInput): Promise<MissionControlSnapshot> {
  return invoke<MissionControlSnapshot>('get_mission_control_snapshot', { input })
}

export async function listAgentRuns(input: ListAgentRunsInput): Promise<AgentRunRecord[]> {
  return invoke<AgentRunRecord[]>('list_agent_runs', { input })
}

export async function listAgentEvents(input: ListAgentEventsInput): Promise<AgentEventRecord[]> {
  return invoke<AgentEventRecord[]>('list_agent_events', { input })
}

export async function controlExecutionScope(input: ControlExecutionScopeInput): Promise<TaskRecord[]> {
  return invoke<TaskRecord[]>('control_execution_scope', { input })
}

export async function getRuntimeFlags(): Promise<RuntimeFlags> {
  return invoke<RuntimeFlags>('get_runtime_flags')
}

export async function setRuntimeFlags(input: SetRuntimeFlagsInput): Promise<RuntimeFlagsUpdateResult> {
  return invoke<RuntimeFlagsUpdateResult>('set_runtime_flags', { input })
}

export async function getProviderSecretStatus(input: GetProviderSecretStatusInput): Promise<ProviderSecretStatus> {
  return invoke<ProviderSecretStatus>('get_provider_secret_status', { input })
}

export async function setProviderSecret(input: SetProviderSecretInput): Promise<SecretOperationResult> {
  return invoke<SecretOperationResult>('set_provider_secret', { input })
}

export async function revealProviderSecret(input: RevealProviderSecretInput): Promise<RevealProviderSecretResult> {
  return invoke<RevealProviderSecretResult>('reveal_provider_secret', { input })
}

export async function archiveTelemetry(input: ArchiveTelemetryInput): Promise<ArchiveTelemetryResult> {
  return invoke<ArchiveTelemetryResult>('archive_telemetry', { input })
}
