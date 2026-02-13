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
  tier: 2 | 3
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

export interface ApproveOrchestrationPlanInput {
  rootTaskId: string
  targetProject: string
  topK?: number
  mcpCommand?: string
  mcpArgs?: string[]
}

export interface PlanExecutionResult {
  rootTask: TaskRecord
  executedTaskIds: string[]
  tier2Executions: number
  tier3Executions: number
  appliedMutations: number
  failedExecutions: number
  message: string
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

export type MutationStatus = 'proposed' | 'validated' | 'validated_no_tests' | 'applied' | 'rejected'

export interface SetMutationStatusInput {
  mutationId: string
  status: MutationStatus
  testResult?: string
  testExitCode?: number
  rejectionReason?: string
  rejectedAtStep?: string
}

export interface RequestMutationRevisionInput {
  mutationId: string
  note: string
}

export interface MutationRevisionResult {
  originalMutation: MutationRecord
  revisedTask: TaskRecord
  revisedMutation: MutationRecord
}

export interface RunMutationPipelineInput {
  mutationId: string
  targetProject: string
  tier1Approved: boolean
  ciCommand?: string
  ciArgs?: string[]
}

export interface PipelineStepResult {
  step: string
  status: string
  details: string
}

export interface MutationPipelineResult {
  mutation: MutationRecord
  task: TaskRecord
  steps: PipelineStepResult[]
  shadowDir: string | null
}

export interface AuditLogEntry {
  id: number
  timestamp: number
  actor: string
  action: string
  targetId: string | null
  details: string | null
}

export interface ListAuditLogInput {
  targetId?: string
  limit?: number
}

export interface ListAgentTerminalsInput {
  rootTaskId?: string
  includeDescendants?: boolean
  includeInactive?: boolean
  limit?: number
}

export interface AgentTerminalSession {
  actor: string
  taskId: string
  eventCount: number
  lastEventId: number
  lastTimestamp: number
  taskStatus: string | null
  taskTier: number | null
  taskDomain: string | null
  lastAction: string
  lastDetails: string | null
}

export interface ListTerminalEventsInput {
  actor: string
  taskId: string
  limit?: number
  sinceId?: number
}

export interface TerminalEventRecord {
  id: number
  timestamp: number
  actor: string
  action: string
  taskId: string
  details: string | null
}

export type TaskControlAction = 'pause' | 'resume' | 'stop' | 'restart'

export interface ControlTaskInput {
  taskId: string
  action: TaskControlAction
  includeDescendants?: boolean
  reason?: string
}

export interface ListTaskActivityInput {
  taskId: string
  includeDescendants?: boolean
  limit?: number
  sinceId?: number
}

export type BudgetRequestStatus = 'pending' | 'approved' | 'rejected'
export type BudgetRequestDecision = 'approve' | 'reject'

export interface BudgetRequestRecord {
  id: string
  taskId: string
  requestedBy: string
  reason: string
  requestedIncrement: number
  currentBudget: number
  currentUsage: number
  status: BudgetRequestStatus
  approvedIncrement: number | null
  resolutionNote: string | null
  createdAt: number
  updatedAt: number
  resolvedAt: number | null
}

export interface RequestTaskBudgetIncreaseInput {
  taskId: string
  requestedBy: string
  reason: string
  requestedIncrement: number
  autoApprove?: boolean
}

export interface ListTaskBudgetRequestsInput {
  taskId: string
  includeDescendants?: boolean
  status?: BudgetRequestStatus
  limit?: number
}

export interface ResolveTaskBudgetRequestInput {
  requestId: string
  decision: BudgetRequestDecision
  approvedIncrement?: number
  reason?: string
  decidedBy?: string
  resumeTask?: boolean
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

export interface ModelProfile {
  provider: string
  modelId: string
  temperature?: number | null
  maxOutputTokens?: number | null
}

export interface ModelRoutingConfig {
  version: number
  defaultProvider: string
  tiers: Record<string, ModelProfile[]>
  personaOverrides: Record<string, ModelProfile[]>
}

export interface ModelRegistrySnapshot {
  configPath: string
  loadedFromFile: boolean
  loadError: string | null
  config: ModelRoutingConfig
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
