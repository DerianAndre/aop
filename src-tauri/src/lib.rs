mod agents;
mod commands;
mod db;
mod llm_adapter;
mod mcp_bridge;
mod model_registry;
mod mutation_pipeline;
mod mutation_revision;
mod task_runtime;
mod vector;

use std::fs;
use std::io;

use sqlx::SqlitePool;
use tauri::Manager;

use mcp_bridge::client::BridgeClient;
use model_registry::ModelRegistry;

pub struct AppState {
    pub db_pool: SqlitePool,
    pub bridge_client: BridgeClient,
    pub model_registry: ModelRegistry,
}

fn initialize_state(app: &mut tauri::App) -> Result<(), String> {
    let app_data_dir = app
        .path()
        .app_local_data_dir()
        .map_err(|error| format!("Failed to determine app data dir: {error}"))?;

    fs::create_dir_all(&app_data_dir)
        .map_err(|error| format!("Failed to create app data dir: {error}"))?;

    let db_path = app_data_dir.join("aop_orchestrator.db");
    let workspace_root = std::env::current_dir()
        .map_err(|error| format!("Failed to determine workspace root: {error}"))?;
    let bridge_client = BridgeClient::new(&workspace_root);
    let model_registry = ModelRegistry::load(&workspace_root);

    let db_pool = tauri::async_runtime::block_on(async {
        let pool = db::connect_pool(&db_path).await?;
        db::run_migrations(&pool).await?;
        Ok::<SqlitePool, String>(pool)
    })?;

    app.manage(AppState {
        db_pool,
        bridge_client,
        model_registry,
    });

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
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
            commands::execute_domain_task,
            commands::list_task_mutations,
            commands::run_mutation_pipeline,
            commands::set_mutation_status,
            commands::request_mutation_revision,
            commands::list_audit_log,
            commands::list_task_activity,
            commands::get_default_target_project,
            commands::list_target_dir,
            commands::read_target_file,
            commands::search_target_files,
            commands::index_target_project,
            commands::query_codebase,
            commands::get_model_registry
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
