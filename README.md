# Autonomous Orchestration Platform (AOP)

Phases 1-2 foundation for a Tauri v2 desktop app with React frontend, SQLite backend, and an MCP bridge.

## What Is Implemented

- Tauri v2 app scaffold (`src-tauri`) wired to React frontend.
- SQLite initialization + migrations on startup.
- Full Section 10 schema migration at `src-tauri/migrations/001_initial.sql`.
- Rust commands:
  - `create_task`
  - `get_tasks`
  - `update_task_status`
- Phase 1 UI:
  - Task creation form
  - Persisted task list with status badges
- Phase 2 bridge package at `mcp-bridge/` with:
  - `list_dir`
  - `read_file`
  - `search_files`
  - Optional MCP stdio passthrough (`mcpCommand` + `mcpArgs`)
- New Tauri commands:
  - `get_default_target_project`
  - `list_target_dir`
  - `read_target_file`
  - `search_target_files`
- Phase 2 UI:
  - Target project path input
  - Directory browser
  - File preview
  - Search panel

## Run

```bash
pnpm install
pnpm tauri dev
```

## Validate

```bash
pnpm lint
pnpm test
pnpm bridge:test
pnpm build
cd src-tauri && cargo test
```

## Build Installers

```bash
pnpm tauri build
```
