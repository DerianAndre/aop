# Autonomous Orchestration Platform (AOP)

Phase 1 foundation for a Tauri v2 desktop app with React frontend and SQLite backend.

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

## Run

```bash
npm install
npm run tauri dev
```

## Validate

```bash
npm run lint
npm run test
npm run build
cd src-tauri && cargo test
```

## Build Installers

```bash
npm run tauri -- build
```
