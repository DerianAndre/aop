import { invoke } from '@tauri-apps/api/core'

import type {
  CreateTaskInput,
  ContextChunk,
  DirectoryListing,
  IndexProjectResult,
  IndexTargetProjectInput,
  ListTargetDirInput,
  QueryCodebaseInput,
  ReadTargetFileInput,
  SearchResult,
  SearchTargetFilesInput,
  TargetFileContent,
  TaskRecord,
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
