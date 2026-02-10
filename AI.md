# AI.md

Machine-to-machine compact context for this repository.

## Identity

- Project: `aop`
- Type: Tauri desktop orchestrator for code mutation workflows.
- Frontend: React/Vite (`src/`)
- Backend: Rust (`src-tauri/src/`)
- Sidecar: Node MCP bridge (`mcp-bridge/`)
- Source of truth DB: SQLite.

## Runtime Entry Points

- Desktop boot: `src-tauri/src/lib.rs::run`
- Global state: `AppState { db_pool, bridge_client, model_registry }`
- UI bridge: `src/hooks/useTauri.ts` -> `src-tauri/src/commands.rs`

## Critical Backend Modules

- `agents/orchestrator.rs` (Tier 1 decomposition)
- `agents/domain_leader.rs` (Tier 2 coordination)
- `agents/specialist.rs` (Tier 3 diff proposal)
- `mutation_pipeline.rs` (validate/apply)
- `mutation_revision.rs` (revision flow)
- `vector/indexer.rs` + `vector/search.rs` (semantic context)
- `mcp_bridge/client.rs` + `mcp_bridge/tool_caller.rs` (target access)
- `model_registry.rs` (tier/persona model routing)
- `llm_adapter.rs` (provider adapters, currently `claude_code`)

## Command Surface (Tauri)

- Tasks: `create_task`, `get_tasks`, `update_task_status`
- Orchestration: `orchestrate_objective`, `execute_domain_task`
- Mutations: `list_task_mutations`, `run_mutation_pipeline`, `set_mutation_status`, `request_mutation_revision`
- Audit: `list_audit_log`
- Target IO: `get_default_target_project`, `list_target_dir`, `read_target_file`, `search_target_files`
- Semantic: `index_target_project`, `query_codebase`
- Models: `get_model_registry`

## Model Routing Contract

Config file: `aop_models.json` (`version: 2`).

- `tiers.<1|2|3>`: `ModelProfile | ModelProfile[]`
- `personaOverrides.<persona>`: `ModelProfile | ModelProfile[]`
- `ModelProfile = { provider, modelId, temperature?, maxOutputTokens? }`
- Resolution rule:
  - candidate source: persona override if exists, else tier
  - candidate order preserved
  - select first provider with available adapter in `llm_adapter`
  - error if none available

## Adapter Contract

- Adapter input: `{ provider, model_id, system_prompt, user_prompt }`
- Adapter output: `{ text, input_tokens?, output_tokens?, total_cost_usd?, resolved_model? }`
- Current provider aliases:
  - `claude_code`
  - `claude-code`
  - `anthropic_claude_code`

## Specialist Generation Behavior

- Default behavior: deterministic local diff proposal.
- Remote adapter call enabled in runtime by default, disabled in tests by default.
- Env toggles:
  - `AOP_MODEL_ADAPTER_ENABLED` (`1|true|yes|on` to force enable)
  - `AOP_MODEL_ADAPTER_STRICT` (fail hard on adapter failure)
  - `AOP_CLAUDE_MAX_BUDGET_USD` (optional CLI call budget)

## MCP/Bridge Safety

- Local tool path constraints and symlink protections in `mcp-bridge/src/tools.ts`.
- Rust bridge has rate limiting, concurrency cap, queue backpressure in `mcp_bridge/client.rs`.

## Fast Verification Commands

```bash
pnpm lint
pnpm test
pnpm bridge:test
pnpm build
cargo test --manifest-path src-tauri/Cargo.toml
```

## Editing Rules For Agents

- Keep command names stable in `commands.rs`.
- Preserve DB schema compatibility.
- Do not bypass model routing in agent modules.
- Add tests for any new provider adapter or resolution logic.
