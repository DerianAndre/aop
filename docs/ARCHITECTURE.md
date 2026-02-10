# AOP Architecture

## Purpose

AOP is a desktop orchestration system that plans, analyzes, proposes, validates, and applies code mutations with strict traceability.

## Runtime Stack

- Frontend: React 19 + Vite + Zustand + React Flow
- Desktop shell: Tauri v2
- Backend: Rust
- Data: SQLite (`aop_orchestrator.db`)
- Bridge: Node.js MCP sidecar (`mcp-bridge/`)

## High-Level Components

### 1) Frontend (`src/`)

- Dashboard and workflow UI.
- Invokes backend commands via `@tauri-apps/api/core`.
- Main capabilities:
  - Task graph and status
  - Diff reviewer
  - Conflict resolution panel
  - Semantic indexing/search controls

### 2) Tauri Command Layer (`src-tauri/src/commands.rs`)

- Stable API boundary between UI and backend domain logic.
- Exposes task, mutation, bridge, semantic, and model-registry commands.

### 3) Agent Layer (`src-tauri/src/agents/`)

- `orchestrator.rs` (Tier 1): decomposes objective into Tier 2 assignments.
- `domain_leader.rs` (Tier 2): gathers context and spawns specialists.
- `specialist.rs` (Tier 3): generates concrete diff proposals.

### 4) Mutation Rail

- `mutation_pipeline.rs`: validate and apply mutation flow.
- `mutation_revision.rs`: request revision and spawn revision specialist.
- `db/mutations.rs`: persistence for proposals and statuses.

### 5) Semantic Engine (`src-tauri/src/vector/`)

- `indexer.rs`: chunking + embedding storage.
- `search.rs`: semantic retrieval for agent context.

### 6) MCP Bridge

- Rust caller: `src-tauri/src/mcp_bridge/`
- Node bridge: `mcp-bridge/src/`
- Supports:
  - local filesystem tools
  - optional MCP stdio forwarding
  - local fallback when MCP call fails

### 7) Model Routing + Adapter

- `model_registry.rs`: resolves model by tier/persona from `aop_models.json`.
- `llm_adapter.rs`: provider adapter execution.
  - current implemented adapter: `claude_code`.
- resolution supports multiple provider candidates and picks first available adapter.

## Main Data Flows

### Objective to Proposals

1. UI sends objective.
2. Tier 1 creates Tier 2 assignments.
3. Tier 2 fetches semantic + file context.
4. Tier 2 spawns Tier 3 specialists.
5. Specialists return `DiffProposal`.
6. Proposals persisted in SQLite.

### Proposal to Apply

1. User reviews proposal.
2. Mutation pipeline validates proposal.
3. On success, mutation is applied and audited.

### Request Revision

1. Reviewer requests revision on mutation.
2. Original mutation marked rejected.
3. New Tier 3 revision task and revised mutation are created.

## Persistence

SQLite is source of truth for:

- Tasks
- Mutations
- Metrics / audit log
- Semantic chunks

## Configuration

### `aop_models.json`

- `tiers` and `personaOverrides` accept candidate lists.
- Runtime resolves using available adapters.

### Important env vars

- `AOP_MODEL_CONFIG_PATH`
- `AOP_MODEL_ADAPTER_ENABLED`
- `AOP_MODEL_ADAPTER_STRICT`
- `AOP_CLAUDE_MAX_BUDGET_USD`
- `AOP_WORKSPACE_ROOT`

## Design Principles

- Zero-trust around file operations.
- Prefer deterministic fallback paths.
- Keep command contracts stable.
- Keep model/provider routing decoupled from agent business logic.
