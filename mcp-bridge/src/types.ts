export type BridgeAction = 'list_dir' | 'read_file' | 'search_files'

export interface BridgeMcpConfig {
  command: string
  args?: string[]
}

export interface BridgeRequest {
  action: BridgeAction
  targetProject: string
  path?: string
  pattern?: string
  limit?: number
  mcp?: BridgeMcpConfig
}

export interface BridgeDirEntry {
  name: string
  path: string
  isDir: boolean
  size: number | null
}

export interface DirectoryListing {
  root: string
  cwd: string
  parent: string | null
  entries: BridgeDirEntry[]
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

export interface BridgeEnvelope<T = unknown> {
  ok: boolean
  data?: T
  error?: string
}
