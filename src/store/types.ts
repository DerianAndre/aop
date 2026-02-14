import type { TaskRecord, MutationRecord, ContextChunk, OrchestrationResult } from '@/types'

export type AppTab =
  | 'command-center'
  | 'mission-control'
  | 'tasks'
  | 'dashboard'
  | 'context'
  | 'mutations'
  | 'terminal'
  | 'logs'
  | 'system'

export type CcPhase = 'empty' | 'planning' | 'ready' | 'running' | 'review' | 'completed' | 'failed'

export type CcLiveFeedTab = 'activity' | 'terminal' | 'budget' | 'errors'

export interface CcInspectorItem {
  type: 'task' | 'mutation' | 'terminal'
  id: string
}

export interface AopStoreState {
  // Core data
  tasks: Map<string, TaskRecord>
  mutations: Map<string, MutationRecord>
  contextQueries: ContextQuery[]

  // UI state
  selectedTaskId: string | null
  selectedMutationId: string | null
  activeTab: AppTab

  // Filters
  taskFilter: {
    status?: string[]
    tier?: number[]
    searchQuery?: string
  }

  // System state
  indexStatus: IndexStatus | null
  sidecarHealth: SidecarHealth | null
  targetProject: string
  mcpCommand: string
  mcpArgs: string

  // Command Center state
  ccRootTaskId: string | null
  ccOrchestrationResult: OrchestrationResult | null
  ccInspectorItem: CcInspectorItem | null
  ccLiveFeedTab: CcLiveFeedTab
  ccInspectorCollapsed: boolean
  ccLiveFeedCollapsed: boolean
}

export interface ContextQuery {
  agent_id: string
  agent_tier: number
  query: string
  results: ContextChunk[]
  latency_ms: number
  embedding_source: 'local' | 'cloud'
  timestamp: number
}

export interface IndexStatus {
  indexed_files: number
  indexed_chunks: number
  stale_chunks: number
  last_indexed_at: number | null
  index_size_bytes: number
}

export interface SidecarHealth {
  uptime_seconds: number
  active_servers: number
  circuit_breakers: Record<string, CircuitState>
}

export type CircuitState = 'closed' | 'open' | 'half-open'

export interface AopStoreActions {
  // Data mutations
  addTask: (task: TaskRecord) => void
  updateTask: (taskId: string, updates: Partial<TaskRecord>) => void
  addMutation: (mutation: MutationRecord) => void
  updateMutation: (mutationId: string, updates: Partial<MutationRecord>) => void

  // UI actions
  selectTask: (taskId: string | null) => void
  selectMutation: (mutationId: string | null) => void
  setActiveTab: (tab: AppTab) => void
  setTaskFilter: (filter: Partial<AopStoreState['taskFilter']>) => void

  // System actions
  setIndexStatus: (status: IndexStatus) => void
  setSidecarHealth: (health: SidecarHealth) => void
  setTargetProject: (value: string) => void
  setMcpCommand: (value: string) => void
  setMcpArgs: (value: string) => void

  // Command Center actions
  setCcRootTaskId: (id: string | null) => void
  setCcOrchestrationResult: (result: OrchestrationResult | null) => void
  setCcInspectorItem: (item: CcInspectorItem | null) => void
  setCcLiveFeedTab: (tab: CcLiveFeedTab) => void
  toggleCcInspector: () => void
  toggleCcLiveFeed: () => void

  // Event handler
  handleTauriEvent: (event: AopEvent) => void
}

export type AopEvent =
  | { type: 'task_created'; task: TaskRecord }
  | { type: 'task_status_changed'; task_id: string; new_status: string }
  | { type: 'mutation_proposed'; mutation: MutationRecord }
  | { type: 'mutation_status_changed'; mutation_id: string; new_status: string }
  | { type: 'token_usage'; task_id: string; tokens_spent: number }
  | { type: 'context_query'; query: ContextQuery }
  | { type: 'index_updated'; affected_files: string[] }
