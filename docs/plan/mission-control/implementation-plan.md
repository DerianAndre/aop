# Mission Control Implementation Plan

Document: AOP-MC-001  
Owner: Platform Architecture  
Status: Approved for Implementation  
Last Updated: 2026-02-13

---

## 1. Objective

Implement Mission Control as the primary operational surface for AOP with:

- structured telemetry for T1/T2/T3 execution
- intelligent multi-provider model routing
- runtime configuration via `.env`
- secure provider secret storage via Tauri Stronghold
- budget controls (auto + request) with smart thresholding
- sanitized MCP traces
- 7-day retention with archive workflow

---

## 2. Scope

### In Scope

- New data model and persistence for agent runs/events/model health.
- New Tauri commands for Mission Control, runtime flags, scope controls, secrets, archive.
- Mission Control UI (ops-clean) with density modes and focused root selector.
- Model selection scoring with health-aware fallback.
- MCP trace sanitization and retention jobs.
- Shared formatting utilities for token count display (`src/lib/format.ts`).

### Out of Scope

- Replacing existing `Tasks`, `Dashboard`, `Terminal`, `Logs` views.
- Full provider ecosystem parity in one release. (Initial API adapter path includes OpenAI.)
- Historical migration rewriting old `aop_audit_log` rows.

---

## 3. Architecture

### 3.1 Data Flow

1. Agents/orchestrator emit events through a telemetry service.
2. Telemetry service persists normalized event payloads.
3. Mission Control queries aggregated run/event snapshots.
4. Model router scores candidates using health metrics + provider capability checks.
5. MCP calls emit sanitized trace events.
6. Retention worker archives old telemetry and prunes active tables.

### 3.2 Backward Compatibility

- Keep `aop_audit_log` as compatibility stream.
- Do not remove existing commands/views.
- Existing task and mutation pipeline continues to run.

---

## 4. Data Contracts

### 4.1 Agent Run

- `id`, `root_task_id`, `task_id`, `tier`, `actor`
- `provider`, `model_id`, `persona`, `skill`, `adapter_kind`
- `status`, `started_at`, `ended_at`
- `tokens_in`, `tokens_out`, `token_delta`, `cost_usd`
- `metadata_json`

### 4.2 Agent Event

- `id`, `run_id`, `root_task_id`, `task_id`, `tier`, `actor`
- `action`, `status`, `phase`, `message`
- `provider`, `model_id`, `persona`, `skill`
- `mcp_server`, `mcp_tool`, `latency_ms`, `retry_count`
- `tokens_in`, `tokens_out`, `token_delta`, `cost_usd`
- `payload_json`, `created_at`

### 4.3 Model Health

- `provider`, `model_id`
- `total_calls`, `success_calls`, `failed_calls`
- `avg_latency_ms`, `avg_cost_usd`, `quality_score`
- `last_error`, `last_used_at`, `updated_at`

---

## 5. Public API Additions

### 5.1 Tauri Commands

- `get_mission_control_snapshot`
- `list_agent_runs`
- `list_agent_events`
- `control_execution_scope`
- `get_runtime_flags`
- `set_runtime_flags`
- `get_provider_secret_status`
- `set_provider_secret`
- `reveal_provider_secret`
- `archive_telemetry`

### 5.2 Frontend Types

- `MissionControlSnapshot`
- `AgentRunRecord`
- `AgentEventRecord`
- `ExecutionScopeControlInput`
- `RuntimeFlags`
- `ProviderSecretStatus`
- `ModelHealthRecord`

### 5.3 Frontend Utilities

- `formatTokenCount(count: number): string` — Formats raw token counts with K/M suffixes for human-readable display (e.g., `1500` → `"1.5K"`, `2000000` → `"2.0M"`, values below 1000 returned as-is). Located in `src/lib/format.ts`.

---

## 6. UI Surface

### Mission Control

- Ops-clean visual direction.
- Root selector defaults to one orchestration root.
- Density modes:
  - PRO
  - Balanced
  - Minimal
- Mosaic list of agent cards with provider/model/persona/skill/MCP/tokens/status.
- Token counts displayed using `formatTokenCount` for compact human-readable values.
- Detail panel:
  - live event timeline
  - terminal output stream
  - sanitized payload inspector
- Filters by tier/status/provider/model/persona/skill/MCP/time.

---

## 7. Security and Secrets

- Runtime flags from root `.env` and `.env.example` baseline.
- Provider secrets stored in Stronghold snapshot.
- Full reveal/edit enabled only in Developer Mode + confirmed session.
- MCP trace redaction strips or masks secret-like keys and values.

---

## 8. Budget Policy

Auto + request mode:

- Auto-increase when remaining budget headroom drops below:
  - `max(percentage threshold, estimated stage cost)`
- Create approval request if required increment exceeds configured cap.
- Default min increment applies.

---

## 9. Retention and Archiving

- Active telemetry retention window: 7 days.
- Older rows exported to JSONL archive snapshots.
- Manual archive command and status surface in UI.

---

## 10. Test Strategy

### Backend

- Migration creation and compatibility with existing schema.
- Structured event write/read tests.
- Scope control tests (`tree`, `tier`, `agent`).
- Model scoring + fallback tests.
- MCP sanitization tests.
- Budget threshold policy tests.
- Archive integrity and retention pruning tests.

### Frontend

- Mission Control render tests (agent metadata).
- Density mode switch behavior.
- Root-scope selection behavior.
- Live timeline refresh behavior.
- `formatTokenCount` unit tests: sub-1000 passthrough, K suffix (1000–999999), M suffix (1000000+), edge cases (0, negative, boundary values).

---

## 11. Rollout Plan

### Slice A

- Stage 0 + Stage 1 schema/events.

### Slice B

- Stage 2 runtime config and secrets.

### Slice C

- Stage 3 model intelligence.

### Slice D

- Stage 4 Mission Control baseline UI.

### Slice E

- Stage 5 control granularity and budget.

### Slice F

- Stage 6 MCP full trace and sanitization.

### Slice G

- Stage 7 retention/archive hardening.

---

## 12. Acceptance Criteria

1. Mission Control shows active agents and structured metadata (provider/model/persona/skill/MCP).
2. Root selection keeps graph and event scope constrained to one orchestration tree by default.
3. Scope controls can pause/resume/stop/restart by tree, tier, or individual agent.
4. Model routing selects candidates by quality-first scoring and falls back on unhealthy/unavailable models.
5. Secrets are persisted securely and guarded by Developer Mode + session confirmation.
6. MCP traces are visible but sanitized.
7. Telemetry older than 7 days is archived and pruned.
8. Token counts across Mission Control UI render with compact K/M suffixes via `formatTokenCount`.

---

## 13. Risks and Controls

- Risk: Model scoring may overfit sparse metrics.
  - Control: Bootstrap defaults + fallback ordering + explicit health thresholds.
- Risk: Secret reveal misuse.
  - Control: Developer Mode gate + time-bound confirmation token.
- Risk: Noisy telemetry volume.
  - Control: Retention policy + archive + indexed queries.
- Risk: Command sprawl complexity.
  - Control: typed interfaces + focused command tests.

---

## 14. Traceability

| Requirement | Implementation Targets | Validation |
|---|---|---|
| Structured telemetry | `src-tauri/src/db/telemetry.rs`, `src-tauri/migrations/*` | telemetry DB tests |
| Mission Control commands | `src-tauri/src/commands.rs` | command integration tests |
| Runtime flags `.env` | `src-tauri/src/runtime_config.rs`, `.env.example` | runtime flag tests |
| Stronghold secrets | `src-tauri/src/secret_vault.rs`, `src-tauri/src/lib.rs` | secret command tests |
| Intelligent model selection | `src-tauri/src/model_intelligence.rs`, `src-tauri/src/model_registry.rs`, `src-tauri/src/llm_adapter.rs` | routing tests |
| Mission Control UI | `src/views/MissionControlView.tsx`, `src/layouts/AppLayout.tsx`, `src/components/aop-sidebar.tsx` | UI tests |
| Token display formatting | `src/lib/format.ts` | `formatTokenCount` unit tests |
| Scope controls | `src-tauri/src/commands.rs`, `src-tauri/src/db/tasks.rs` | control scope tests |
| Smart budget thresholds | `src-tauri/src/task_runtime.rs` | budget policy tests |
| MCP sanitized traces | `src-tauri/src/mcp_bridge/client.rs`, `src-tauri/src/db/telemetry.rs` | sanitization tests |
| 7-day archive retention | `src-tauri/src/db/telemetry.rs`, `src-tauri/src/lib.rs` | archive/prune tests |