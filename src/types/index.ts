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

export interface UserObjectiveInput {
  objective: string
  targetProject: string
  globalTokenBudget: number
  maxRiskTolerance: number
}

export interface TaskAssignment {
  taskId: string
  parentId: string
  tier: 2
  domain: string
  objective: string
  tokenBudget: number
  riskFactor: number
  constraints: string[]
  relevantFiles: string[]
}

export interface OrchestrationResult {
  rootTask: TaskRecord
  assignments: TaskAssignment[]
  overheadBudget: number
  reserveBudget: number
  distributedBudget: number
}

export interface ExecuteDomainTaskInput {
  taskId: string
  targetProject: string
  topK?: number
  mcpCommand?: string
  mcpArgs?: string[]
}

export interface DiffProposal {
  proposalId: string
  taskId: string
  agentUid: string
  filePath: string
  diffContent: string
  intentDescription: string
  intentHash: string
  confidence: number
  tokensUsed: number
}

export interface ConflictReport {
  agentA: string
  agentB: string
  semanticDistance: number
  description: string
  requiresHumanReview: boolean
}

export interface IntentSummary {
  taskId: string
  domain: string
  status: 'ready_for_review' | 'consensus_failed' | 'blocked'
  proposals: DiffProposal[]
  complianceScore: number
  tokensSpent: number
  summary: string
  conflicts?: ConflictReport
}

export interface ListTaskMutationsInput {
  taskId: string
}

export interface MutationRecord {
  id: string
  taskId: string
  agentUid: string
  filePath: string
  diffContent: string
  intentDescription: string | null
  intentHash: string | null
  confidence: number
  testResult: string | null
  testExitCode: number | null
  rejectionReason: string | null
  rejectedAtStep: string | null
  status: string
  proposedAt: number
  appliedAt: number | null
}

export interface UpdateTaskStatusInput {
  taskId: string
  status: TaskStatus
  errorMessage?: string | null
}

export interface ListTargetDirInput {
  targetProject: string
  dirPath?: string
  mcpCommand?: string
  mcpArgs?: string[]
}

export interface ReadTargetFileInput {
  targetProject: string
  filePath: string
  mcpCommand?: string
  mcpArgs?: string[]
}

export interface SearchTargetFilesInput {
  targetProject: string
  pattern: string
  limit?: number
  mcpCommand?: string
  mcpArgs?: string[]
}

export interface DirectoryEntry {
  name: string
  path: string
  isDir: boolean
  size: number | null
}

export interface DirectoryListing {
  root: string
  cwd: string
  parent: string | null
  entries: DirectoryEntry[]
  source: 'local' | 'mcp' | 'mcp_fallback_local'
  warnings: string[]
}

export interface TargetFileContent {
  root: string
  path: string
  size: number
  content: string
  source: 'local' | 'mcp' | 'mcp_fallback_local'
  warnings: string[]
}

export interface SearchMatch {
  path: string
  line: number | null
  preview: string | null
}

export interface SearchResult {
  root: string
  pattern: string
  matches: SearchMatch[]
  source: 'local' | 'mcp' | 'mcp_fallback_local'
  warnings: string[]
}

export interface IndexTargetProjectInput {
  targetProject: string
}

export interface IndexProjectResult {
  targetProject: string
  tableName: string
  indexedFiles: number
  indexedChunks: number
  indexPath: string
}

export interface QueryCodebaseInput {
  targetProject: string
  query: string
  topK?: number
}

export interface ContextChunk {
  id: string
  filePath: string
  startLine: number
  endLine: number
  chunkType: string
  name: string
  content: string
  score: number
}
