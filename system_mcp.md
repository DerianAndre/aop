# Technical Specification: Communication Infrastructure and Semantic Memory

**Engineering Document**: AOP-DET-001  
**System Reference**: system.md (AOP_Master_Engineering_Prompt_v2.md)  
**Architect**: Derian Castillo (Lead Systems Architect)  
**Confidentiality Level**: Critical Operational  
**Last Updated**: February 2026

---

## 0. Dependencies and Versions

All versions are verified as stable as of February 2026.

### Rust (Cargo.toml) — Dependencies specific to this document

| Crate                  | Version     | Purpose                                           |
| ---------------------- | ----------- | ------------------------------------------------- |
| tree-sitter            | 0.26.3      | Incremental AST parser for code fragmentation     |
| tree-sitter-typescript | 0.23.2      | TypeScript/TSX grammar for tree-sitter            |
| tree-sitter-rust       | 0.23.2      | Rust grammar for tree-sitter                      |
| tree-sitter-javascript | 0.23.1      | JavaScript grammar for tree-sitter                |
| notify                 | 8.2.0       | Cross-platform filesystem watcher (change events) |
| ort                    | 2.0.0-rc.11 | ONNX Runtime bindings — local embedding inference |
| lancedb                | 0.23        | Embedded vector database, serverless              |
| arrow                  | 57.2        | Columnar format for exchange with LanceDB         |
| sha2                   | 0.10.9      | SHA-256 hash for change detection                 |

> **Note**: Dependencies shared with the AOP core (tauri, sqlx, serde, tokio, uuid, chrono) are defined in `system.md` and are NOT duplicated here.

### Node.js (package.json) — MCP Sidecar

| Package                   | Version | Purpose                                           |
| ------------------------- | ------- | ------------------------------------------------- |
| @modelcontextprotocol/sdk | ^1.26.0 | Official MCP SDK (stdio transport + JSON-RPC 2.0) |
| zod                       | ^3.25.0 | Schema validation (SDK peer dependency)           |
| typescript                | ^5.7.0  | TypeScript compiler                               |

---

## 1. Universal MCP Bridge Architecture (Sidecar)

The MCP bridge acts as the abstraction layer between AOP's sovereign core and the MCP servers of target projects. It's implemented as a child process (Sidecar) to ensure fault isolation and compatibility with the Node.js ecosystem.

### 1.1 Transport Protocol and Serialization

- **Transport**: stdio (standard input/output) using bidirectional pipes.
- **Message Format**: JSON-RPC 2.0.
- **Concurrency**: `Promise.allSettled` to process multiple simultaneous tool calls from the agent pool.

**Message format (request)**:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "read_file",
    "arguments": {
      "path": "src/components/App.tsx"
    }
  }
}
```

**Message format (response)**:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "// file contents..."
      }
    ]
  }
}
```

**Message format (error)**:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "error": {
    "code": -32001,
    "message": "SECURITY_VIOLATION: path escapes project root",
    "data": {
      "requested_path": "/etc/passwd",
      "project_root": "/home/user/my-project"
    }
  }
}
```

### 1.2 Zero-Trust Security Layer

The bridge doesn't trust agent instructions. The following validators are implemented:

**Scope Guardian** (Path Sanitizer):
- Every received path is normalized with `path.resolve()`.
- If the result falls outside the project's root directory, the operation is aborted with `SECURITY_VIOLATION` error.
- **Symlink protection**: Before resolving, `fs.realpathSync()` is executed to detect symlinks pointing outside the project. If `realpath !== resolvedPath`, the operation is blocked.
- Paths with `..`, `~`, or null characters are rejected before normalization.

**Tool Sandbox**:
- Only tools explicitly declared in the project's `aop_config.json` are exposed.
- File schema:

```json
{
  "$schema": "https://aop.dev/schemas/aop_config.v1.json",
  "project_root": "./",
  "mcp_servers": {
    "filesystem": {
      "command": "npx",
      "args": ["-y", "@anthropic/mcp-server-filesystem", "./src"],
      "allowed_tools": ["read_file", "list_directory", "search_files"],
      "denied_tools": ["write_file", "move_file", "delete_file"]
    }
  },
  "security": {
    "max_calls_per_minute": 120,
    "max_concurrent_calls": 10,
    "write_enabled": false,
    "allowed_extensions": [".ts", ".tsx", ".js", ".jsx", ".css", ".json", ".md"]
  }
}
```

**Rate Limiter**:
- Configurable maximum MCP calls per minute (default: 120).
- Configurable maximum concurrent calls (default: 10).
- If the limit is exceeded, calls are queued with backpressure. If the queue exceeds 50 items, `RATE_LIMIT_EXCEEDED` error is returned.

**Circuit Breaker**:
- If an MCP server fails 5 consecutive times, the circuit opens for 30 seconds.
- During the open circuit, all calls to that server return `SERVER_UNAVAILABLE` error without attempting connection.
- After 30s, 1 test call is allowed (half-open). If successful, the circuit closes.

**Immutable Read**:
- By default, all operations in Phases 1-4 are read-only.
- Write permissions are activated only after Shadow Testing validation.
- The `security.write_enabled` field in `aop_config.json` explicitly controls this.

### 1.3 Sidecar Lifecycle

```
┌─────────┐    ┌───────────┐    ┌───────────┐    ┌──────────┐
│  INIT   │───▶│ HANDSHAKE │───▶│ EXECUTION │───▶│ SHUTDOWN │
└─────────┘    └───────────┘    └───────────┘    └──────────┘
     │              │                 │                │
     ▼              ▼                 ▼                ▼
  Tauri spawn   mcp_ready +      JSON-RPC 2.0     SIGTERM →
  sidecar       tool list        request/response  3s timeout →
  process       (3s timeout)     (with heartbeat)  SIGKILL
```

**Detailed phases**:

1. **Init**: Tauri invokes the sidecar binary at application startup via `Command::new().stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())`.

2. **Handshake**: The sidecar sends an `mcp_ready` event with the list of available tools.
   - **Timeout**: If `mcp_ready` is not received within 3 seconds, spawn is retried (maximum 3 attempts).
   - **Fallback**: If after 3 attempts there's no handshake, an `mcp_failed` event is emitted to the frontend with the error.

3. **Execution**: The Rust core dispatches JSON-RPC commands via stdin/stdout of the child process.

4. **Heartbeat**: Every 15 seconds, the core sends a ping to the sidecar. If there's no response within 5 seconds, the sidecar is considered dead and the recovery flow executes:
   - Log the state at the moment of failure.
   - Kill the zombie process (SIGKILL).
   - Automatic re-spawn (maximum 3 attempts per session).
   - If re-spawn fails, notify the user via the frontend.

5. **Shutdown**: When closing AOP:
   - SIGTERM signal is sent to the sidecar.
   - Wait 3 seconds for clean shutdown.
   - If it doesn't respond, SIGKILL to avoid orphan processes.
   - Clean up stdin/stdout/stderr pipes.

---

## 2. Vector Indexing Engine (Semantic Engine)

The vector engine provides the "Long-Term Memory" necessary for agents to understand code architecture without needing to read all files on each turn.

### 2.1 AST-Aware Fragmentation Strategy

Unlike character-based chunking, AOP decomposes code based on its logical structure using `tree-sitter` (v0.26.3).

**Granularity Levels**:

| Level          | Captured AST Nodes                                           | Example                                  |
| -------------- | ------------------------------------------------------------ | ---------------------------------------- |
| L1: Symbol     | function, class, interface, type_alias, variable declaration | `function getUserData(id: string) {...}` |
| L2: Block      | if_statement, for_statement, try_statement, export_statement | `export const config = {...}`            |
| L3: Expression | call_expression, object_expression, arrow_function           | `users.map(u => u.id)`                   |

**Fragmentation Algorithm**:

```
For each file:
  1. Parse with tree-sitter → AST
  2. Traverse depth-first
  3. For each node:
     - If level L1 → create fragment (with full text of node)
     - If level L2 AND parent is not L1 → create fragment
     - If level L3 AND standalone → create fragment
  4. Deduplicate overlapping fragments (keep the most specific)
  5. Add metadata:
     - start_line, end_line, start_char, end_char
     - language (ts/tsx/js/jsx/rs)
     - parent_symbol (if nested)
     - imports (from file header)
```

### 2.2 Dual Embedding Model

**Local Model (Offline Priority)**:
- Model: `BAAI/bge-m3` (ONNX int8 quantization, ~543MB).
- Dimensions: 1024.
- Latency: ~15ms per fragment on Ryzen 7 5800X.
- Languages: 100+.
- Max tokens: 8192.

**Cloud Model (Fallback)**:
- Model: `text-embedding-3-small` (OpenAI).
- Dimensions: 1536.
- Latency: ~80ms per fragment (with API call).
- Used when: local model fails or when reindexing >1000 files (parallel batching).

**Fallback Logic**:

```
Try local model
    │
    ├── Success → insert embedding into LanceDB
    │
    └── Failure (OOM, crash, etc.)
        │
        └── Try cloud model
            │
            ├── Success → insert embedding
            │
            └── Failure (no internet, quota exceeded)
                │
                └── Add to pending_embeddings queue
                    │
                    └── Retry every 5 minutes
```

### 2.3 LanceDB Schema

```rust
struct CodeChunk {
    id: String,              // UUID v7
    file_path: String,       // Relative to project root
    language: String,        // "typescript" | "rust" | "javascript"
    content: String,         // The code fragment
    start_line: u32,
    end_line: u32,
    parent_symbol: Option<String>, // e.g. "class User" if nested
    imports: Vec<String>,    // Only direct imports from the file
    embedding: Vec<f32>,     // 1024 dims (BGE-M3) or 1536 (OpenAI)
    embedding_model: String, // "bge-m3-onnx" | "text-embedding-3-small"
    created_at: i64,         // Unix timestamp
    file_modified_at: i64,   // mtime of the source file
    hash: String,            // SHA-256 of content (for staleness detection)
}
```

**Indexes**:
- ANN index on `embedding` using IVF-PQ (Inverted File with Product Quantization).
- B-tree index on `file_path` for fast invalidation on file changes.
- B-tree index on `hash` for duplicate detection.

### 2.4 Incremental Reindexing

**Filesystem Watcher** (`notify` v8.2.0):
- Listens to `Create`, `Modify`, `Remove`, `Rename` events in the project directory.
- **Debounce**: Groups events that occur within 500ms to avoid redundant reindexing (e.g., when an IDE saves multiple times).
- **Ignored paths**: `node_modules`, `.git`, `dist`, `build` (configurable in `aop_config.json`).

**Reindex Logic**:

```
File modified event detected
    │
    ▼
Compute SHA-256 of new content
    │
    ▼
Query LanceDB: SELECT * WHERE file_path = ? AND hash = ?
    │
    ├── Hash matches → ignore (no real change, just touch)
    │
    └── Hash differs → proceed
        │
        ▼
    Delete old chunks for that file_path
        │
        ▼
    Fragment + embed + insert new chunks
        │
        ▼
    Emit event "file_reindexed" to frontend
```

**Performance Optimizations**:
- Parallelized with tokio (up to 4 files simultaneously).
- If >50 files change simultaneously (e.g., git checkout), they're queued and processed in batches of 10.
- Priority queue: Files currently open in the editor have priority.

### 2.5 Semantic Search

**Relevance Formula**:

```
S(c, q) = cos(emb(c), emb(q)) × freshness(c) × lang_boost(c, q)

where:
  - cos() = cosine similarity between embeddings
  - freshness(c) = e^(-days_old / 30)  // exponential decay
  - lang_boost(c, q) = 1.2 if language matches agent's context, else 1.0
```

**Search Algorithm**:

```
1. Embed the query with the same model used for chunks (BGE-M3 local).
2. LanceDB.search(embedding_query, top_k=20)
   - Uses ANN (Approximate Nearest Neighbor) with IVF-PQ.
   - Latency: ~50ms for 100k chunks, ~100ms for 1M chunks.
3. Re-rank the 20 candidates using the full S(c, q) formula.
4. Return top_k=5 to the agent.
```

---

## 3. Rust Core Interfaces

### 3.1 Shared Data Types

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextChunk {
    pub id: String,
    pub file_path: String,
    pub language: String,
    pub content: String,
    pub start_line: u32,
    pub end_line: u32,
    pub relevance_score: f32,  // 0.0 to 1.0
    pub parent_symbol: Option<String>,
    pub imports: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolRequest {
    pub server_id: String,  // "filesystem", "database", etc.
    pub tool_name: String,
    pub args: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolResponse {
    pub content: Vec<ContentBlock>,
    pub is_error: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image { data: String, mime_type: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexStatus {
    pub total_files: u32,
    pub indexed_files: u32,
    pub pending_files: u32,
    pub stale_files: u32,
    pub last_indexed_at: Option<String>,
    pub index_size_bytes: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SidecarHealth {
    pub alive: bool,
    pub uptime_seconds: u64,
    pub available_servers: Vec<String>,
    pub circuit_breaker_status: std::collections::HashMap<String, String>, // server_id -> "closed" | "open" | "half-open"
}
```

### 3.2 Tauri Commands (Internal API)

```rust
// === Vector Engine ===

#[tauri::command]
async fn query_context(
    query: String,
    top_k: Option<u32>,       // default: 5
    language: Option<String>,  // optional language filter
) -> Result<Vec<ContextChunk>, String>;

#[tauri::command]
async fn reindex_file(path: String) -> Result<(), String>;

#[tauri::command]
async fn reindex_project() -> Result<IndexStatus, String>;

#[tauri::command]
async fn get_index_status() -> Result<IndexStatus, String>;

// === MCP Bridge ===

#[tauri::command]
async fn call_mcp_tool(request: McpToolRequest) -> Result<McpToolResponse, String>;

#[tauri::command]
async fn list_mcp_servers() -> Result<Vec<String>, String>;

#[tauri::command]
async fn get_sidecar_health() -> Result<SidecarHealth, String>;

#[tauri::command]
async fn restart_sidecar() -> Result<(), String>;
```

### 3.3 Agent Recovery Logic

Complete flow when an agent (Tier 2/3) needs context:

```
Agent generates semantic query
        │
        ▼
Vector Engine: search for top_k=5 most relevant fragments
        │
        ▼
MCP Bridge: verify if source files have changed
  (compare metadata.hash vs current file's SHA-256 on disk)
        │
        ├── No changes → Use index fragments directly
        │
        └── With changes → target_read in real-time
                │
                ▼
            Re-index the changed file (async, non-blocking)
                │
                ▼
            Hydrate context with fresh content
                │
                ▼
            Send to LLM with updated fragments
```

**Hydration Rules**:
- If more than 3 of the 5 fragments are stale, perform a complete re-index of the affected directory.
- Hydrated context always includes: the fragment + 2 lines before/after + the file's import declarations.
- Total injected context size must not exceed 8000 tokens per agent turn.

---

## 4. Error Handling

### 4.1 Custom Error Codes

| Code   | Name                  | Description                               | Action                      |
| ------ | --------------------- | ----------------------------------------- | --------------------------- |
| -32001 | `SECURITY_VIOLATION`  | Path outside project or malicious symlink | Log + abort + notify user   |
| -32002 | `RATE_LIMIT_EXCEEDED` | More than N calls/minute to MCP           | Queue or reject             |
| -32003 | `SERVER_UNAVAILABLE`  | Circuit breaker open for that server      | Retry after cooldown        |
| -32004 | `TOOL_NOT_ALLOWED`    | Tool not declared in aop_config.json      | Silent abort                |
| -32005 | `SIDECAR_DEAD`        | Sidecar not responding to heartbeat       | Automatic re-spawn          |
| -32006 | `INDEX_STALE`         | More than 50% of index is outdated        | Background re-index         |
| -32007 | `EMBEDDING_FAILED`    | Both local and cloud models failed        | Queue in pending_embeddings |
| -32008 | `WRITE_DENIED`        | Write attempt in read-only mode           | Abort + log                 |

### 4.2 Retry Strategy

```
Attempt 1 → wait 0ms (immediate)
Attempt 2 → wait 500ms
Attempt 3 → wait 2000ms (exponential backoff)
Attempt 4+ → Don't retry. Emit error to agent.
```

Errors `SECURITY_VIOLATION`, `TOOL_NOT_ALLOWED` and `WRITE_DENIED` are **never** retried.

---

## 5. Implementation Roadmap

### Phase 1: Communication Foundations

- [ ] Implement Sidecar in Node.js using `@modelcontextprotocol/sdk` v1.26.0 with stdio transport.
- [ ] Configure `command_handler` in Rust to manage Sidecar's stdin/stdout/stderr.
- [ ] Implement initial handshake with 3s timeout and 3 retries.
- [ ] Implement heartbeat (ping every 15s, 5s timeout).
- [ ] Implement Scope Guardian with symlink protection.
- [ ] Implement Rate Limiter (120 calls/min default).
- [ ] Create `aop_config.json` schema and its parser/validator.

**Done when**:
- Sidecar starts, handshakes, and responds to `tools/call` from Rust.
- A malicious path (e.g., `../../etc/passwd`) is blocked and logged.
- Sidecar automatically recovers after a manual kill.
- Rate limiter blocks call #121 within a minute.

### Phase 2: Semantic Intelligence

- [ ] Integrate `tree-sitter` v0.26.3 in Rust indexing worker with TS/JS/Rust grammars.
- [ ] Implement AST-aware fragmentation logic with 3 granularity levels.
- [ ] Configure LanceDB v0.23 with complete schema (including `id`, `language`, `embedding_model`, `file_modified_at`).
- [ ] Implement dual embedding service (BGE-M3 local + text-embedding-3-small cloud).
- [ ] Implement automatic fallback local → cloud → pending queue.
- [ ] Implement filesystem watcher with `notify` v8.2.0 and 500ms debounce.
- [ ] Implement relevance formula `S(c, q)` with temporal decay.

**Done when**:
- A TypeScript repository with 200+ files indexes in less than 30 seconds.
- A semantic search returns 5 relevant fragments in less than 100ms.
- When modifying a file, it re-indexes automatically without user intervention.
- If internet disconnects, embeddings are queued and processed upon reconnection.

### Phase 3: Swarm Orchestration

- [ ] Create "Context Provider" in React that visualizes which code fragments are feeding the agent in real-time.
- [ ] Implement Circuit Breaker for MCP servers.
- [ ] Perform stress tests with repositories of >5,000 files to optimize ANN search latency.
- [ ] Implement context re-hydration logic (section 3.3).
- [ ] Implement observability metrics: embedding latency, index hit rate, re-index frequency.

**Done when**:
- A repository with 5,000+ files maintains search latency < 200ms.
- Circuit Breaker opens after 5 consecutive failures and recovers after 30s.
- Context Provider shows in real-time the fragments the agent is using.
- Observability metrics are available in AOP dashboard.

---

## 6. Architecture Decision Records (ADRs)

**ADR-001: Why Sidecar in Node.js and not pure Rust?**
The official MCP SDK (`@modelcontextprotocol/sdk`) is TypeScript-first. Implementing the MCP protocol from scratch in Rust would require reimplementing JSON-RPC 2.0, handshake, and all types. Maintenance cost would be high and compatibility with existing MCP servers wouldn't be guaranteed. The Node.js sidecar is the pragmatic choice.

**ADR-002: Why LanceDB and not Qdrant/Milvus?**
LanceDB is embedded (serverless), integrates natively with Arrow (which we already use), and scales to millions of vectors on disk. Qdrant and Milvus require a separate server, which contradicts AOP's philosophy of being a standalone desktop application.

**ADR-003: Why BGE-M3 as the default local model?**
BGE-M3 supports 100+ languages, processes up to 8192 tokens, and produces 1024-dimensional embeddings (lighter than OpenAI's 1536). The ONNX model weighs ~543MB in int8, which is acceptable for a desktop application. Its performance in code retrieval benchmarks is competitive with cloud models.

**ADR-004: Why tree-sitter and not regex/lines for fragmentation?**
Chunking by lines or regex loses the semantic structure of code. A method split in half is useless to an agent. tree-sitter produces a real AST that allows cutting at logical boundaries (functions, classes, interfaces). The additional parsing cost is ~6ms per 2000-line file.