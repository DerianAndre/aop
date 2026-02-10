import { invoke } from '@tauri-apps/api/core'

import type { CreateTaskInput, TaskRecord, UpdateTaskStatusInput } from '@/types'

export async function getTasks(): Promise<TaskRecord[]> {
  return invoke<TaskRecord[]>('get_tasks')
}

export async function createTask(input: CreateTaskInput): Promise<TaskRecord> {
  return invoke<TaskRecord>('create_task', { input })
}

export async function updateTaskStatus(input: UpdateTaskStatusInput): Promise<TaskRecord> {
  return invoke<TaskRecord>('update_task_status', { input })
}
