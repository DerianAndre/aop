import { listDir, readFile, searchFiles } from './tools.js'
import type {
  BridgeAction,
  BridgeDirEntry,
  BridgeRequest,
  DirectoryListing,
  SearchMatch,
  SearchResult,
  TargetFileContent,
} from './types.js'

type BridgeResult = DirectoryListing | TargetFileContent | SearchResult

interface BridgeExecutionContext {
  source: 'local' | 'mcp' | 'mcp_fallback_local'
  warnings: string[]
}

interface McpToolDescriptor {
  name: string
}

interface McpToolResponse {
  content?: Array<{ type: string; text?: string }>
}

interface McpClientLike {
  connect(transport: unknown): Promise<void>
  close(): Promise<void>
  listTools(): Promise<{ tools?: McpToolDescriptor[] }>
  callTool(args: { name: string; arguments: Record<string, unknown> }): Promise<McpToolResponse | unknown>
}

interface McpClientCtor {
  new (
    clientInfo: { name: string; version: string },
    options?: Record<string, unknown>,
  ): McpClientLike
}

interface StdioClientTransportCtor {
  new (config: { command: string; args?: string[]; env?: NodeJS.ProcessEnv }): unknown
}

function withMetadata<T extends { source: 'local' | 'mcp' | 'mcp_fallback_local'; warnings: string[] }>(
  result: T,
  context: BridgeExecutionContext,
): T {
  return {
    ...result,
    source: context.source,
    warnings: context.warnings,
  }
}

function normalizeListDirPayload(payload: unknown): DirectoryListing {
  const data = payload as Partial<DirectoryListing>
  if (!data || !Array.isArray(data.entries)) {
    throw new Error('MCP response for list_dir is invalid')
  }

  const entries: BridgeDirEntry[] = data.entries.map((entry) => ({
    name: String(entry.name ?? ''),
    path: String(entry.path ?? ''),
    isDir: Boolean(entry.isDir),
    size: entry.size == null ? null : Number(entry.size),
  }))

  return {
    root: String(data.root ?? ''),
    cwd: String(data.cwd ?? '.'),
    parent: data.parent == null ? null : String(data.parent),
    entries,
    source: 'mcp',
    warnings: [],
  }
}

function normalizeReadFilePayload(payload: unknown): TargetFileContent {
  const data = payload as Partial<TargetFileContent>
  if (!data || typeof data.content !== 'string') {
    throw new Error('MCP response for read_file is invalid')
  }

  return {
    root: String(data.root ?? ''),
    path: String(data.path ?? ''),
    size: Number(data.size ?? 0),
    content: data.content,
    source: 'mcp',
    warnings: [],
  }
}

function normalizeSearchPayload(payload: unknown, pattern: string): SearchResult {
  const data = payload as Partial<SearchResult>
  if (!data || !Array.isArray(data.matches)) {
    throw new Error('MCP response for search_files is invalid')
  }

  const matches: SearchMatch[] = data.matches.map((match) => ({
    path: String(match.path ?? ''),
    line: match.line == null ? null : Number(match.line),
    preview: match.preview == null ? null : String(match.preview),
  }))

  return {
    root: String(data.root ?? ''),
    pattern: String(data.pattern ?? pattern),
    matches,
    source: 'mcp',
    warnings: [],
  }
}

function parseToolPayload(payload: unknown): unknown {
  if (typeof payload !== 'string') {
    return payload
  }

  try {
    return JSON.parse(payload)
  } catch {
    return payload
  }
}

async function callMcpTool(request: BridgeRequest): Promise<BridgeResult> {
  if (!request.mcp?.command) {
    throw new Error('Missing MCP command configuration')
  }

  const { Client } = (await import('@modelcontextprotocol/sdk/client/index.js')) as {
    Client: McpClientCtor
  }
  const { StdioClientTransport } = (await import('@modelcontextprotocol/sdk/client/stdio.js')) as {
    StdioClientTransport: StdioClientTransportCtor
  }

  const transport = new StdioClientTransport({
    command: request.mcp.command,
    args: request.mcp.args ?? [],
    env: process.env,
  })

  const client = new Client(
    {
      name: 'aop-mcp-bridge',
      version: '0.1.0',
    },
    { capabilities: {} },
  )

  await client.connect(transport)

  try {
    const tools = await client.listTools()
    const availableToolNames = new Set<string>((tools.tools ?? []).map((tool) => tool.name))

    const candidates: Record<BridgeAction, string[]> = {
      read_file: ['read_file', 'readFile'],
      list_dir: ['list_dir', 'list_directory', 'listDir'],
      search_files: ['search_files', 'searchFiles'],
    }

    const selectedTool = candidates[request.action].find((candidate) => availableToolNames.has(candidate))
    if (!selectedTool) {
      throw new Error(`Target MCP server does not expose ${request.action}`)
    }

    const toolArgs =
      request.action === 'search_files'
        ? { path: request.path, pattern: request.pattern, limit: request.limit }
        : { path: request.path }

    const rawToolResult = (await client.callTool({
      name: selectedTool,
      arguments: toolArgs,
    })) as McpToolResponse

    const textPayload =
      rawToolResult?.content
        ?.filter((item) => item.type === 'text')
        ?.map((item) => item.text ?? '')
        ?.join('\n') ?? ''

    const payload = parseToolPayload(textPayload || rawToolResult)

    if (request.action === 'list_dir') {
      return normalizeListDirPayload(payload)
    }

    if (request.action === 'read_file') {
      return normalizeReadFilePayload(payload)
    }

    return normalizeSearchPayload(payload, request.pattern ?? '')
  } finally {
    await client.close()
  }
}

async function executeLocal(request: BridgeRequest): Promise<BridgeResult> {
  if (request.action === 'list_dir') {
    return listDir(request.targetProject, request.path)
  }

  if (request.action === 'read_file') {
    return readFile(request.targetProject, request.path)
  }

  return searchFiles(request.targetProject, request.pattern ?? '', request.limit ?? 40)
}

export async function executeBridgeRequest(request: BridgeRequest): Promise<BridgeResult> {
  if (!request.mcp?.command) {
    return executeLocal(request)
  }

  try {
    const mcpResult = await callMcpTool(request)
    return withMetadata(mcpResult, { source: 'mcp', warnings: [] })
  } catch (error) {
    const fallback = await executeLocal(request)
    return withMetadata(fallback, {
      source: 'mcp_fallback_local',
      warnings: [error instanceof Error ? error.message : String(error)],
    })
  }
}
