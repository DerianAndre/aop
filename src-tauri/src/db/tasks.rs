use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    Executing,
    Completed,
    Failed,
    Paused,
}

impl TaskStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            TaskStatus::Pending => "pending",
            TaskStatus::Executing => "executing",
            TaskStatus::Completed => "completed",
            TaskStatus::Failed => "failed",
            TaskStatus::Paused => "paused",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct TaskRecord {
    pub id: String,
    pub parent_id: Option<String>,
    pub tier: i64,
    pub domain: String,
    pub objective: String,
    pub status: String,
    pub token_budget: i64,
    pub token_usage: i64,
    pub context_efficiency_ratio: f64,
    pub risk_factor: f64,
    pub compliance_score: i64,
    pub checksum_before: Option<String>,
    pub checksum_after: Option<String>,
    pub error_message: Option<String>,
    pub retry_count: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTaskInput {
    pub parent_id: Option<String>,
    pub tier: i64,
    pub domain: String,
    pub objective: String,
    pub token_budget: i64,
}

#[derive(Debug, Clone)]
pub struct CreateTaskRecordInput {
    pub parent_id: Option<String>,
    pub tier: i64,
    pub domain: String,
    pub objective: String,
    pub token_budget: i64,
    pub risk_factor: f64,
    pub status: TaskStatus,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTaskStatusInput {
    pub task_id: String,
    pub status: TaskStatus,
    pub error_message: Option<String>,
}

pub async fn create_task(pool: &SqlitePool, input: CreateTaskInput) -> Result<TaskRecord, String> {
    create_task_record(
        pool,
        CreateTaskRecordInput {
            parent_id: input.parent_id,
            tier: input.tier,
            domain: input.domain,
            objective: input.objective,
            token_budget: input.token_budget,
            risk_factor: 0.0,
            status: TaskStatus::Pending,
        },
    )
    .await
}

pub async fn create_task_record(
    pool: &SqlitePool,
    input: CreateTaskRecordInput,
) -> Result<TaskRecord, String> {
    validate_create_record_input(&input)?;

    let id = Uuid::new_v4().to_string();
    let now = Utc::now().timestamp();
    let parent_id = input
        .parent_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    sqlx::query(
        r#"
        INSERT INTO aop_tasks (
            id, parent_id, tier, domain, objective, status,
            token_budget, token_usage, context_efficiency_ratio, risk_factor,
            compliance_score, checksum_before, checksum_after, error_message,
            retry_count, created_at, updated_at
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, 0, 0.0, ?, 0, NULL, NULL, NULL, 0, ?, ?)
        "#,
    )
    .bind(&id)
    .bind(parent_id)
    .bind(input.tier)
    .bind(input.domain.trim())
    .bind(input.objective.trim())
    .bind(input.status.as_str())
    .bind(input.token_budget)
    .bind(input.risk_factor)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await
    .map_err(|error| format!("Failed to create task: {error}"))?;

    get_task_by_id(pool, &id).await
}

pub async fn get_tasks(pool: &SqlitePool) -> Result<Vec<TaskRecord>, String> {
    sqlx::query_as::<_, TaskRecord>(
        r#"
        SELECT
            id, parent_id, tier, domain, objective, status, token_budget, token_usage,
            context_efficiency_ratio, risk_factor, compliance_score, checksum_before,
            checksum_after, error_message, retry_count, created_at, updated_at
        FROM aop_tasks
        ORDER BY created_at DESC
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|error| format!("Failed to fetch tasks: {error}"))
}

pub async fn update_task_status(
    pool: &SqlitePool,
    input: UpdateTaskStatusInput,
) -> Result<TaskRecord, String> {
    if input.task_id.trim().is_empty() {
        return Err("taskId is required".to_string());
    }

    let now = Utc::now().timestamp();
    let rows_affected = sqlx::query(
        r#"
        UPDATE aop_tasks
        SET status = ?, error_message = ?, updated_at = ?
        WHERE id = ?
        "#,
    )
    .bind(input.status.as_str())
    .bind(input.error_message)
    .bind(now)
    .bind(input.task_id.trim())
    .execute(pool)
    .await
    .map_err(|error| format!("Failed to update task status: {error}"))?
    .rows_affected();

    if rows_affected == 0 {
        return Err(format!("Task '{}' not found", input.task_id));
    }

    get_task_by_id(pool, input.task_id.trim()).await
}

pub async fn get_task_by_id(pool: &SqlitePool, task_id: &str) -> Result<TaskRecord, String> {
    sqlx::query_as::<_, TaskRecord>(
        r#"
        SELECT
            id, parent_id, tier, domain, objective, status, token_budget, token_usage,
            context_efficiency_ratio, risk_factor, compliance_score, checksum_before,
            checksum_after, error_message, retry_count, created_at, updated_at
        FROM aop_tasks
        WHERE id = ?
        "#,
    )
    .bind(task_id)
    .fetch_optional(pool)
    .await
    .map_err(|error| format!("Failed to fetch task: {error}"))?
    .ok_or_else(|| format!("Task '{task_id}' not found"))
}

fn validate_create_record_input(input: &CreateTaskRecordInput) -> Result<(), String> {
    if !(1..=3).contains(&input.tier) {
        return Err("tier must be 1, 2, or 3".to_string());
    }
    if input.domain.trim().is_empty() {
        return Err("domain is required".to_string());
    }
    if input.objective.trim().is_empty() {
        return Err("objective is required".to_string());
    }
    if input.token_budget <= 0 {
        return Err("tokenBudget must be greater than 0".to_string());
    }
    if !(0.0..=1.0).contains(&input.risk_factor) {
        return Err("riskFactor must be between 0.0 and 1.0".to_string());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use sqlx::sqlite::SqlitePoolOptions;

    use super::*;
    use crate::db;

    async fn setup_test_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite should initialize");

        db::run_migrations(&pool)
            .await
            .expect("migrations should run in tests");

        pool
    }

    #[tokio::test]
    async fn create_list_and_update_task_flow_works() {
        let pool = setup_test_pool().await;

        let created = create_task(
            &pool,
            CreateTaskInput {
                parent_id: None,
                tier: 1,
                domain: "platform".to_string(),
                objective: "Bootstrap foundation".to_string(),
                token_budget: 2500,
            },
        )
        .await
        .expect("task should be created");

        assert_eq!(created.status, "pending");
        assert_eq!(created.domain, "platform");

        let listed = get_tasks(&pool).await.expect("tasks should load");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, created.id);

        let updated = update_task_status(
            &pool,
            UpdateTaskStatusInput {
                task_id: created.id.clone(),
                status: TaskStatus::Executing,
                error_message: None,
            },
        )
        .await
        .expect("status should update");

        assert_eq!(updated.status, "executing");
        assert_eq!(updated.id, created.id);
    }

    #[tokio::test]
    async fn create_task_rejects_invalid_tier() {
        let pool = setup_test_pool().await;

        let result = create_task(
            &pool,
            CreateTaskInput {
                parent_id: None,
                tier: 4,
                domain: "platform".to_string(),
                objective: "Invalid".to_string(),
                token_budget: 1000,
            },
        )
        .await;

        assert!(result.is_err());
    }
}
