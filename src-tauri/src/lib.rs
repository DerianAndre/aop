mod agents;
mod commands;
mod db;
mod llm_adapter;
mod mcp_bridge;
mod model_intelligence;
mod model_registry;
mod mutation_pipeline;
mod mutation_revision;
mod runtime_config;
mod secret_vault;
mod task_runtime;
mod vector;

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use sqlx::SqlitePool;
use tokio::sync::Mutex;
use tauri::Manager;

use mcp_bridge::client::BridgeClient;
use model_registry::ModelRegistry;
use runtime_config::RuntimeFlags;
use secret_vault::SecretVault;
use sha2::{Digest, Sha256};

pub struct AppState {
    pub db_pool: SqlitePool,
    pub bridge_client: BridgeClient,
    pub model_registry: ModelRegistry,
    pub runtime_flags: Arc<RwLock<RuntimeFlags>>,
    pub secret_vault: Arc<Mutex<SecretVault>>,
    pub app_data_dir: PathBuf,
}

fn resolve_workspace_root() -> Result<PathBuf, String> {
    let cwd = std::env::current_dir()
        .map_err(|error| format!("Failed to determine workspace root: {error}"))?;

    // When Tauri runs via `cargo` or `pnpm tauri dev`, the CWD is often `src-tauri/`.
    // Detect this and navigate up to the actual project root.
    if cwd.file_name().map(|n| n == "src-tauri").unwrap_or(false) {
        if let Some(parent) = cwd.parent() {
            return Ok(parent.to_path_buf());
        }
    }

    Ok(cwd)
}

fn initialize_state(app: &mut tauri::App) -> Result<(), String> {
    let workspace_root = resolve_workspace_root()?;
    load_env_files(workspace_root.as_path());

    let app_data_dir = app
        .path()
        .app_local_data_dir()
        .map_err(|error| format!("Failed to determine app data dir: {error}"))?;

    fs::create_dir_all(&app_data_dir)
        .map_err(|error| format!("Failed to create app data dir: {error}"))?;

    let db_path = app_data_dir.join("aop_orchestrator.db");
    let bridge_client = BridgeClient::new(&workspace_root);
    let model_registry = ModelRegistry::load(&workspace_root);
    let runtime_flags = Arc::new(RwLock::new(RuntimeFlags::from_env()));
    if let Ok(flags) = runtime_flags.read() {
        flags.sync_to_process_env();
    }
    let secret_vault = Arc::new(Mutex::new(SecretVault::new(app_data_dir.clone())));

    let db_pool = tauri::async_runtime::block_on(async {
        let pool = db::connect_pool(&db_path).await?;
        db::run_migrations(&pool).await?;
        Ok::<SqlitePool, String>(pool)
    })?;
    let retention_days = runtime_flags
        .read()
        .map(|value| value.telemetry_retention_days)
        .unwrap_or(7);
    db::telemetry::spawn_retention_worker(
        db_pool.clone(),
        app_data_dir.join("telemetry-archive"),
        retention_days,
    );

    app.manage(AppState {
        db_pool,
        bridge_client,
        model_registry,
        runtime_flags,
        secret_vault,
        app_data_dir,
    });

    Ok(())
}

fn load_env_files(workspace_root: &Path) {
    let root_env = workspace_root.join(".env");
    let root_local = workspace_root.join(".env.local");
    let _ = dotenvy::from_path(root_env);
    let _ = dotenvy::from_path(root_local);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_stronghold::Builder::new(|password| {
                let mut hasher = Sha256::new();
                hasher.update(password.as_bytes());
                hasher.finalize().to_vec()
            })
            .build(),
        )
        .setup(|app| {
            initialize_state(app).map_err(io::Error::other)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::create_task,
            commands::get_tasks,
            commands::update_task_status,
            commands::control_task,
            commands::request_task_budget_increase,
            commands::list_task_budget_requests,
            commands::resolve_task_budget_request,
            commands::orchestrate_objective,
            commands::analyze_objective,
            commands::submit_answers_and_plan,
            commands::approve_orchestration_plan,
            commands::execute_domain_task,
            commands::list_task_mutations,
            commands::run_mutation_pipeline,
            commands::set_mutation_status,
            commands::request_mutation_revision,
            commands::list_audit_log,
            commands::list_task_activity,
            commands::list_agent_terminals,
            commands::list_terminal_events,
            commands::get_default_target_project,
            commands::list_target_dir,
            commands::read_target_file,
            commands::search_target_files,
            commands::index_target_project,
            commands::query_codebase,
            commands::get_model_registry,
            commands::get_mission_control_snapshot,
            commands::list_agent_runs,
            commands::list_agent_events,
            commands::control_execution_scope,
            commands::get_runtime_flags,
            commands::set_runtime_flags,
            commands::get_provider_secret_status,
            commands::set_provider_secret,
            commands::reveal_provider_secret,
            commands::archive_telemetry
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
