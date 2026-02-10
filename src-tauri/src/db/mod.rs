pub mod budget_requests;
pub mod metrics;
pub mod mutations;
pub mod tasks;

use std::path::Path;
use std::time::Duration;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;

pub async fn connect_pool(db_path: &Path) -> Result<SqlitePool, String> {
    let connect_options = SqliteConnectOptions::new()
        .filename(db_path)
        .create_if_missing(true)
        .busy_timeout(Duration::from_secs(5))
        .foreign_keys(true);

    SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(connect_options)
        .await
        .map_err(|error| format!("Failed to connect to SQLite: {error}"))
}

pub async fn run_migrations(pool: &SqlitePool) -> Result<(), String> {
    sqlx::migrate!("./migrations")
        .run(pool)
        .await
        .map_err(|error| format!("Failed to run migrations: {error}"))
}
