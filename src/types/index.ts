export type TaskStatus = 'pending' | 'executing' | 'completed' | 'failed' | 'paused'

export interface TaskRecord {
  id: string
  parentId: string | null
  tier: number
  domain: string
  objective: string
  status: TaskStatus
  tokenBudget: number
  tokenUsage: number
  contextEfficiencyRatio: number
  riskFactor: number
  complianceScore: number
  checksumBefore: string | null
  checksumAfter: string | null
  errorMessage: string | null
  retryCount: number
  createdAt: number
  updatedAt: number
}

export interface CreateTaskInput {
  parentId?: string | null
  tier: 1 | 2 | 3
  domain: string
  objective: string
  tokenBudget: number
}

export interface UpdateTaskStatusInput {
  taskId: string
  status: TaskStatus
  errorMessage?: string | null
}
