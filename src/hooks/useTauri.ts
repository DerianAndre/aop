import { invoke } from '@tauri-apps/api/core'

import type {
  AuditLogEntry,
  CreateTaskInput,
  ContextChunk,
  DirectoryListing,
  ExecuteDomainTaskInput,
  IndexProjectResult,
  IndexTargetProjectInput,
  IntentSummary,
  ListAuditLogInput,
  ListTargetDirInput,
  ListTaskMutationsInput,
  MutationPipelineResult,
  MutationRecord,
  MutationRevisionResult,
  OrchestrationResult,
  QueryCodebaseInput,
  ReadTargetFileInput,
  RequestMutationRevisionInput,
  RunMutationPipelineInput,
  SearchResult,
  SearchTargetFilesInput,
  SetMutationStatusInput,
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

export async function orchestrateObjective(input: UserObjectiveInput): Promise<OrchestrationResult> {
  return invoke<OrchestrationResult>('orchestrate_objective', { input })
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
