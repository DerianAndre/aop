use tauri::State;

use crate::db::tasks::{self, CreateTaskInput, TaskRecord, UpdateTaskStatusInput};
use crate::AppState;

#[tauri::command]
pub async fn create_task(
    state: State<'_, AppState>,
    input: CreateTaskInput,
) -> Result<TaskRecord, String> {
    tasks::create_task(&state.db_pool, input).await
}

#[tauri::command]
pub async fn get_tasks(state: State<'_, AppState>) -> Result<Vec<TaskRecord>, String> {
    tasks::get_tasks(&state.db_pool).await
}

#[tauri::command]
pub async fn update_task_status(
    state: State<'_, AppState>,
    input: UpdateTaskStatusInput,
) -> Result<TaskRecord, String> {
    tasks::update_task_status(&state.db_pool, input).await
}
