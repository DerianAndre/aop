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
