use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::collections::VecDeque;
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
    pub target_files: Option<String>,
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
    pub target_files: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTaskStatusInput {
    pub task_id: String,
    pub status: TaskStatus,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UpdateTaskOutcomeInput {
    pub task_id: String,
    pub status: TaskStatus,
    pub token_usage: Option<i64>,
    pub context_efficiency_ratio: Option<f64>,
    pub compliance_score: Option<i64>,
    pub checksum_before: Option<String>,
    pub checksum_after: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskControlAction {
    Pause,
    Resume,
    Stop,
    Restart,
}

impl TaskControlAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskControlAction::Pause => "pause",
            TaskControlAction::Resume => "resume",
            TaskControlAction::Stop => "stop",
            TaskControlAction::Restart => "restart",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlTaskInput {
    pub task_id: String,
    pub action: TaskControlAction,
    pub include_descendants: Option<bool>,
    pub reason: Option<String>,
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
            target_files: None,
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
            retry_count, created_at, updated_at, target_files
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, 0, 0.0, ?, 0, NULL, NULL, NULL, 0, ?, ?, ?)
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
    .bind(input.target_files.as_deref())
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
            checksum_after, error_message, retry_count, created_at, updated_at, target_files
        FROM aop_tasks
        ORDER BY created_at DESC
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|error| format!("Failed to fetch tasks: {error}"))
}

pub async fn collect_task_tree_ids(
    pool: &SqlitePool,
    root_task_id: &str,
) -> Result<Vec<String>, String> {
    let root = root_task_id.trim();
    if root.is_empty() {
        return Err("taskId is required".to_string());
    }

    get_task_by_id(pool, root).await?;

    let mut ordered_ids = vec![root.to_string()];
    let mut queue: VecDeque<String> = VecDeque::from([root.to_string()]);

    while let Some(parent_id) = queue.pop_front() {
        let children = sqlx::query_scalar::<_, String>(
            r#"
            SELECT id
            FROM aop_tasks
            WHERE parent_id = ?
            ORDER BY created_at ASC
            "#,
        )
        .bind(parent_id)
        .fetch_all(pool)
        .await
        .map_err(|error| format!("Failed to resolve task descendants: {error}"))?;

        for child_id in children {
            queue.push_back(child_id.clone());
            ordered_ids.push(child_id);
        }
    }

    Ok(ordered_ids)
}

pub async fn control_task(
    pool: &SqlitePool,
    input: ControlTaskInput,
) -> Result<Vec<TaskRecord>, String> {
    let root_task_id = input.task_id.trim();
    if root_task_id.is_empty() {
        return Err("taskId is required".to_string());
    }

    let include_descendants = input.include_descendants.unwrap_or(true);
    let action = input.action.clone();
    let target_ids = if include_descendants {
        collect_task_tree_ids(pool, root_task_id).await?
    } else {
        get_task_by_id(pool, root_task_id).await?;
        vec![root_task_id.to_string()]
    };

    let stop_reason = input
        .reason
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("stopped by user");
    let mut updated = Vec::new();

    for task_id in target_ids {
        let current = get_task_by_id(pool, &task_id).await?;
        match &action {
            TaskControlAction::Pause => {
                if matches!(current.status.as_str(), "completed" | "failed" | "paused") {
                    continue;
                }

                let pause_marker = format!("__aop_paused_prev_status:{}", current.status);
                let record = update_task_status(
                    pool,
                    UpdateTaskStatusInput {
                        task_id: task_id.clone(),
                        status: TaskStatus::Paused,
                        error_message: Some(pause_marker),
                    },
                )
                .await?;
                updated.push(record);
            }
            TaskControlAction::Resume => {
                if current.status != "paused" {
                    continue;
                }

                let resume_status = paused_previous_status(current.error_message.as_deref());
                let record = update_task_status(
                    pool,
                    UpdateTaskStatusInput {
                        task_id: task_id.clone(),
                        status: resume_status,
                        error_message: None,
                    },
                )
                .await?;
                updated.push(record);
            }
            TaskControlAction::Stop => {
                if matches!(current.status.as_str(), "completed" | "failed") {
                    continue;
                }

                let record = update_task_status(
                    pool,
                    UpdateTaskStatusInput {
                        task_id: task_id.clone(),
                        status: TaskStatus::Failed,
                        error_message: Some(format!("stopped_by_user:{stop_reason}")),
                    },
                )
                .await?;
                updated.push(record);
            }
            TaskControlAction::Restart => {
                if !matches!(current.status.as_str(), "failed" | "completed" | "paused") {
                    continue;
                }

                let now = Utc::now().timestamp();
                let rows_affected = sqlx::query(
                    r#"
                    UPDATE aop_tasks
                    SET status = 'pending', error_message = NULL, retry_count = retry_count + 1, updated_at = ?
                    WHERE id = ?
                    "#,
                )
                .bind(now)
                .bind(task_id.as_str())
                .execute(pool)
                .await
                .map_err(|error| format!("Failed to restart task '{}': {error}", task_id))?
                .rows_affected();

                if rows_affected == 0 {
                    continue;
                }

                let record = get_task_by_id(pool, task_id.as_str()).await?;
                updated.push(record);
            }
        }
    }

    if updated.is_empty() {
        let scope = if include_descendants {
            "task tree"
        } else {
            "task"
        };
        return Err(format!(
            "No tasks were updated for action '{}' on {} '{}'.",
            action.as_str(),
            scope,
            root_task_id
        ));
    }

    Ok(updated)
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

pub async fn increase_task_budget(
    pool: &SqlitePool,
    task_id: &str,
    increment: i64,
) -> Result<TaskRecord, String> {
    let trimmed_task_id = task_id.trim();
    if trimmed_task_id.is_empty() {
        return Err("taskId is required".to_string());
    }
    if increment <= 0 {
        return Err("increment must be greater than 0".to_string());
    }

    let now = Utc::now().timestamp();
    let rows_affected = sqlx::query(
        r#"
        UPDATE aop_tasks
        SET token_budget = token_budget + ?, updated_at = ?
        WHERE id = ?
        "#,
    )
    .bind(increment)
    .bind(now)
    .bind(trimmed_task_id)
    .execute(pool)
    .await
    .map_err(|error| format!("Failed to increase task budget: {error}"))?
    .rows_affected();

    if rows_affected == 0 {
        return Err(format!("Task '{}' not found", trimmed_task_id));
    }

    get_task_by_id(pool, trimmed_task_id).await
}

pub async fn update_task_outcome(
    pool: &SqlitePool,
    input: UpdateTaskOutcomeInput,
) -> Result<TaskRecord, String> {
    if input.task_id.trim().is_empty() {
        return Err("taskId is required".to_string());
    }

    let current = get_task_by_id(pool, input.task_id.trim()).await?;
    let token_usage = input.token_usage.unwrap_or(current.token_usage);
    let context_efficiency_ratio = input
        .context_efficiency_ratio
        .unwrap_or(current.context_efficiency_ratio);
    let compliance_score = input.compliance_score.unwrap_or(current.compliance_score);
    let checksum_before = input.checksum_before.or(current.checksum_before);
    let checksum_after = input.checksum_after.or(current.checksum_after);
    let error_message = input.error_message.or(current.error_message);

    let now = Utc::now().timestamp();
    sqlx::query(
        r#"
        UPDATE aop_tasks
        SET status = ?, token_usage = ?, context_efficiency_ratio = ?, compliance_score = ?,
            checksum_before = ?, checksum_after = ?, error_message = ?, updated_at = ?
        WHERE id = ?
        "#,
    )
    .bind(input.status.as_str())
    .bind(token_usage)
    .bind(context_efficiency_ratio)
    .bind(compliance_score)
    .bind(checksum_before)
    .bind(checksum_after)
    .bind(error_message)
    .bind(now)
    .bind(input.task_id.trim())
    .execute(pool)
    .await
    .map_err(|error| format!("Failed to update task outcome: {error}"))?;

    get_task_by_id(pool, input.task_id.trim()).await
}

pub async fn get_task_by_id(pool: &SqlitePool, task_id: &str) -> Result<TaskRecord, String> {
    sqlx::query_as::<_, TaskRecord>(
        r#"
        SELECT
            id, parent_id, tier, domain, objective, status, token_budget, token_usage,
            context_efficiency_ratio, risk_factor, compliance_score, checksum_before,
            checksum_after, error_message, retry_count, created_at, updated_at, target_files
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

fn paused_previous_status(error_message: Option<&str>) -> TaskStatus {
    const MARKER: &str = "__aop_paused_prev_status:";
    let Some(raw) = error_message else {
        return TaskStatus::Executing;
    };

    let Some(value) = raw.trim().strip_prefix(MARKER) else {
        return TaskStatus::Executing;
    };

    match value {
        "pending" => TaskStatus::Pending,
        "executing" => TaskStatus::Executing,
        _ => TaskStatus::Executing,
    }
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

    #[tokio::test]
    async fn update_task_outcome_sets_checksums_and_compliance() {
        let pool = setup_test_pool().await;

        let created = create_task(
            &pool,
            CreateTaskInput {
                parent_id: None,
                tier: 2,
                domain: "auth".to_string(),
                objective: "Validate mutation pipeline".to_string(),
                token_budget: 3200,
            },
        )
        .await
        .expect("task should be created");

        let updated = update_task_outcome(
            &pool,
            UpdateTaskOutcomeInput {
                task_id: created.id.clone(),
                status: TaskStatus::Completed,
                token_usage: Some(1550),
                context_efficiency_ratio: Some(1.2),
                compliance_score: Some(82),
                checksum_before: Some("before_hash".to_string()),
                checksum_after: Some("after_hash".to_string()),
                error_message: None,
            },
        )
        .await
        .expect("task outcome should update");

        assert_eq!(updated.status, "completed");
        assert_eq!(updated.token_usage, 1550);
        assert_eq!(updated.compliance_score, 82);
        assert_eq!(updated.checksum_before.as_deref(), Some("before_hash"));
        assert_eq!(updated.checksum_after.as_deref(), Some("after_hash"));
    }

    #[tokio::test]
    async fn control_task_pause_resume_stop_and_restart_with_descendants() {
        let pool = setup_test_pool().await;

        let parent = create_task(
            &pool,
            CreateTaskInput {
                parent_id: None,
                tier: 1,
                domain: "platform".to_string(),
                objective: "Parent".to_string(),
                token_budget: 2000,
            },
        )
        .await
        .expect("parent task should be created");
        let child = create_task(
            &pool,
            CreateTaskInput {
                parent_id: Some(parent.id.clone()),
                tier: 2,
                domain: "auth".to_string(),
                objective: "Child".to_string(),
                token_budget: 1600,
            },
        )
        .await
        .expect("child task should be created");

        let paused = control_task(
            &pool,
            ControlTaskInput {
                task_id: parent.id.clone(),
                action: TaskControlAction::Pause,
                include_descendants: Some(true),
                reason: None,
            },
        )
        .await
        .expect("pause control should succeed");
        assert_eq!(paused.len(), 2);
        assert!(paused.iter().all(|task| task.status == "paused"));

        let resumed = control_task(
            &pool,
            ControlTaskInput {
                task_id: parent.id.clone(),
                action: TaskControlAction::Resume,
                include_descendants: Some(true),
                reason: None,
            },
        )
        .await
        .expect("resume control should succeed");
        assert_eq!(resumed.len(), 2);
        assert!(resumed.iter().all(|task| task.status == "pending"));

        let stopped = control_task(
            &pool,
            ControlTaskInput {
                task_id: child.id.clone(),
                action: TaskControlAction::Stop,
                include_descendants: Some(false),
                reason: Some("manual kill".to_string()),
            },
        )
        .await
        .expect("stop control should succeed");
        assert_eq!(stopped.len(), 1);
        assert_eq!(stopped[0].status, "failed");
        assert!(stopped[0]
            .error_message
            .as_deref()
            .unwrap_or_default()
            .contains("stopped_by_user"));

        let restarted = control_task(
            &pool,
            ControlTaskInput {
                task_id: parent.id.clone(),
                action: TaskControlAction::Restart,
                include_descendants: Some(true),
                reason: Some("retry".to_string()),
            },
        )
        .await
        .expect("restart control should succeed");

        assert!(!restarted.is_empty());
        assert!(restarted.iter().all(|task| task.status == "pending"));
    }

    #[tokio::test]
    async fn increase_task_budget_updates_budget() {
        let pool = setup_test_pool().await;

        let created = create_task(
            &pool,
            CreateTaskInput {
                parent_id: None,
                tier: 2,
                domain: "platform".to_string(),
                objective: "Budget test".to_string(),
                token_budget: 1800,
            },
        )
        .await
        .expect("task should be created");

        let updated = increase_task_budget(&pool, &created.id, 700)
            .await
            .expect("budget should increase");
        assert_eq!(updated.token_budget, 2500);
    }
}
