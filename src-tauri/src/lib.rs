mod commands;
mod db;
mod mcp_bridge;

use std::fs;
use std::io;

use sqlx::SqlitePool;
use tauri::Manager;

use mcp_bridge::client::BridgeClient;

pub struct AppState {
    pub db_pool: SqlitePool,
    pub bridge_client: BridgeClient,
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

    let db_pool = tauri::async_runtime::block_on(async {
        let pool = db::connect_pool(&db_path).await?;
        db::run_migrations(&pool).await?;
        Ok::<SqlitePool, String>(pool)
    })?;

    app.manage(AppState {
        db_pool,
        bridge_client,
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
            commands::get_default_target_project,
            commands::list_target_dir,
            commands::read_target_file,
            commands::search_target_files
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
