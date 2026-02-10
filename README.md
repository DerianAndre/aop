# Autonomous Orchestration Platform (AOP)

AOP is a desktop app (Tauri + React + Rust) to orchestrate code tasks, analyze codebases, generate mutation proposals, validate them, and apply changes with traceability.

This README is DX-focused: quick setup, key commands, and how to use the capabilities.

## Quick Start

### 1) Prerequisites

- Node.js 20+
- `pnpm` 10+
- Rust toolchain (stable)
- Tauri system requirements for your OS

### 2) Install

```bash
pnpm install
```

### 3) Run in Dev

```bash
pnpm tauri dev
```

## Core Commands

```bash
pnpm lint
pnpm test
pnpm bridge:test
pnpm build
pnpm bridge:build
cargo test --manifest-path src-tauri/Cargo.toml
```

Build desktop bundle:

```bash
pnpm tauri build
```

## What You Can Do

### Task Orchestration

- Create tasks and track status in SQLite.
- Run Tier 1 orchestration to split an objective into Tier 2 tasks.
- Execute Tier 2 tasks to spawn Tier 3 specialists and generate mutation proposals.

### Diff / Mutation Pipeline

- Review proposed mutations.
- Approve/reject/request revision from UI.
- Run validation pipeline before apply.
- Persist full audit trail.

### Target Codebase Access

- Browse target directories/files.
- Search files by pattern.
- Use local tool fallback when MCP call fails.

### Semantic Engine

- Index target project source chunks.
- Query codebase with natural language.
- Hydrate Tier 2/Tier 3 context from semantic chunks.

### Model Routing (Multi-Provider)

- Configure models in `aop_models.json`.
- Each tier/persona can define multiple candidate providers/models.
- Runtime selects the first candidate with an available adapter.
- Current adapter implemented: `claude_code`.

## Model Config (`aop_models.json`)

`aop_models.json` supports:

- `tiers`: tier default candidates (`1`, `2`, `3`)
- `personaOverrides`: persona-specific candidates
- each value can be one profile or an array of profiles

Example shape:

```json
{
  "version": 2,
  "defaultProvider": "claude_code",
  "tiers": {
    "3": [
      { "provider": "openai", "modelId": "gpt-5-nano" },
      { "provider": "claude_code", "modelId": "sonnet" }
    ]
  },
  "personaOverrides": {
    "security_analyst": [
      { "provider": "openai", "modelId": "o3" },
      { "provider": "claude_code", "modelId": "opus" }
    ]
  }
}
```

## Environment Variables

- `AOP_MODEL_CONFIG_PATH`: override path to model config JSON.
- `AOP_MODEL_ADAPTER_ENABLED`: enable/disable remote model adapter.
  - default: enabled in runtime, disabled in tests.
- `AOP_MODEL_ADAPTER_STRICT`: fail hard if adapter call fails.
- `AOP_CLAUDE_MAX_BUDGET_USD`: optional per-call budget for Claude Code CLI.
- `AOP_WORKSPACE_ROOT`: optional workspace root override for bridge resolution.

## Project Docs

- Main system spec: `docs/system.md`
- MCP + semantic infra spec: `docs/system_mcp.md`
- UI planning docs: `docs/plan/ui/`
- Additional architecture docs:
  - `ARCHITECTURE.md`
  - `DIAGRAM.md`
  - `AI.md`
