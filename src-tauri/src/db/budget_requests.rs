use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::{QueryBuilder, Sqlite, SqlitePool};
use uuid::Uuid;

use crate::db::tasks::{self, ControlTaskInput, TaskControlAction};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BudgetRequestStatus {
    Pending,
    Approved,
    Rejected,
}

impl BudgetRequestStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            BudgetRequestStatus::Pending => "pending",
            BudgetRequestStatus::Approved => "approved",
            BudgetRequestStatus::Rejected => "rejected",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BudgetRequestDecision {
    Approve,
    Reject,
}

impl BudgetRequestDecision {
    pub fn as_str(self) -> &'static str {
        match self {
            BudgetRequestDecision::Approve => "approve",
            BudgetRequestDecision::Reject => "reject",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct BudgetRequestRecord {
    pub id: String,
    pub task_id: String,
    pub requested_by: String,
    pub reason: String,
    pub requested_increment: i64,
    pub current_budget: i64,
    pub current_usage: i64,
    pub status: String,
    pub approved_increment: Option<i64>,
    pub resolution_note: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub resolved_at: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateBudgetRequestInput {
    pub task_id: String,
    pub requested_by: String,
    pub reason: String,
    pub requested_increment: i64,
    pub auto_approve: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListTaskBudgetRequestsInput {
    pub task_id: String,
    pub include_descendants: Option<bool>,
    pub status: Option<BudgetRequestStatus>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolveBudgetRequestInput {
    pub request_id: String,
    pub decision: BudgetRequestDecision,
    pub approved_increment: Option<i64>,
    pub reason: Option<String>,
    pub decided_by: Option<String>,
    pub resume_task: Option<bool>,
}

pub async fn create_budget_request(
    pool: &SqlitePool,
    input: CreateBudgetRequestInput,
) -> Result<BudgetRequestRecord, String> {
    validate_create_input(&input)?;

    let task = tasks::get_task_by_id(pool, input.task_id.trim()).await?;
    let now = Utc::now().timestamp();
    let request_id = Uuid::new_v4().to_string();

    sqlx::query(
        r#"
        INSERT INTO aop_budget_requests (
            id, task_id, requested_by, reason, requested_increment,
            current_budget, current_usage, status, approved_increment, resolution_note,
            created_at, updated_at, resolved_at
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, 'pending', NULL, NULL, ?, ?, NULL)
        "#,
    )
    .bind(&request_id)
    .bind(task.id.clone())
    .bind(input.requested_by.trim())
    .bind(input.reason.trim())
    .bind(input.requested_increment)
    .bind(task.token_budget)
    .bind(task.token_usage)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await
    .map_err(|error| format!("Failed to create budget request: {error}"))?;

    if input.auto_approve.unwrap_or(false) {
        return resolve_budget_request(
            pool,
            ResolveBudgetRequestInput {
                request_id,
                decision: BudgetRequestDecision::Approve,
                approved_increment: Some(input.requested_increment),
                reason: Some("auto-approved by runtime".to_string()),
                decided_by: Some(input.requested_by),
                resume_task: Some(false),
            },
        )
        .await;
    }

    get_budget_request_by_id(pool, &request_id).await
}

pub async fn list_task_budget_requests(
    pool: &SqlitePool,
    input: ListTaskBudgetRequestsInput,
) -> Result<Vec<BudgetRequestRecord>, String> {
    let task_id = input.task_id.trim();
    if task_id.is_empty() {
        return Err("taskId is required".to_string());
    }

    let task_ids = if input.include_descendants.unwrap_or(false) {
        tasks::collect_task_tree_ids(pool, task_id).await?
    } else {
        tasks::get_task_by_id(pool, task_id).await?;
        vec![task_id.to_string()]
    };

    if task_ids.is_empty() {
        return Ok(Vec::new());
    }

    let limit = i64::from(input.limit.unwrap_or(50).clamp(1, 200));
    let mut query_builder: QueryBuilder<'_, Sqlite> = QueryBuilder::new(
        r#"
        SELECT id, task_id, requested_by, reason, requested_increment, current_budget, current_usage,
               status, approved_increment, resolution_note, created_at, updated_at, resolved_at
        FROM aop_budget_requests
        WHERE task_id IN (
        "#,
    );

    {
        let mut separated = query_builder.separated(", ");
        for id in &task_ids {
            separated.push_bind(id);
        }
    }
    query_builder.push(")");

    if let Some(status) = input.status {
        query_builder
            .push(" AND status = ")
            .push_bind(status.as_str());
    }

    query_builder
        .push(" ORDER BY created_at DESC LIMIT ")
        .push_bind(limit);

    query_builder
        .build_query_as::<BudgetRequestRecord>()
        .fetch_all(pool)
        .await
        .map_err(|error| format!("Failed to list budget requests: {error}"))
}

pub async fn resolve_budget_request(
    pool: &SqlitePool,
    input: ResolveBudgetRequestInput,
) -> Result<BudgetRequestRecord, String> {
    let request_id = input.request_id.trim();
    if request_id.is_empty() {
        return Err("requestId is required".to_string());
    }

    let current = get_budget_request_by_id(pool, request_id).await?;
    if current.status != BudgetRequestStatus::Pending.as_str() {
        return Err(format!(
            "Budget request '{}' is already resolved with status '{}'",
            request_id, current.status
        ));
    }

    let now = Utc::now().timestamp();
    match input.decision {
        BudgetRequestDecision::Approve => {
            let increment = input
                .approved_increment
                .unwrap_or(current.requested_increment);
            if increment <= 0 {
                return Err("approvedIncrement must be greater than 0".to_string());
            }

            tasks::increase_task_budget(pool, &current.task_id, increment).await?;
            sqlx::query(
                r#"
                UPDATE aop_budget_requests
                SET status = 'approved',
                    approved_increment = ?,
                    resolution_note = ?,
                    updated_at = ?,
                    resolved_at = ?
                WHERE id = ?
                "#,
            )
            .bind(increment)
            .bind(build_resolution_note(
                input.decided_by.as_deref(),
                input.reason.as_deref(),
            ))
            .bind(now)
            .bind(now)
            .bind(request_id)
            .execute(pool)
            .await
            .map_err(|error| format!("Failed to approve budget request: {error}"))?;

            if input.resume_task.unwrap_or(false) {
                let _ = tasks::control_task(
                    pool,
                    ControlTaskInput {
                        task_id: current.task_id.clone(),
                        action: TaskControlAction::Resume,
                        include_descendants: Some(false),
                        reason: Some("resume after budget approval".to_string()),
                    },
                )
                .await?;
            }
        }
        BudgetRequestDecision::Reject => {
            sqlx::query(
                r#"
                UPDATE aop_budget_requests
                SET status = 'rejected',
                    approved_increment = NULL,
                    resolution_note = ?,
                    updated_at = ?,
                    resolved_at = ?
                WHERE id = ?
                "#,
            )
            .bind(build_resolution_note(
                input.decided_by.as_deref(),
                input.reason.as_deref(),
            ))
            .bind(now)
            .bind(now)
            .bind(request_id)
            .execute(pool)
            .await
            .map_err(|error| format!("Failed to reject budget request: {error}"))?;
        }
    }

    get_budget_request_by_id(pool, request_id).await
}

pub async fn get_budget_request_by_id(
    pool: &SqlitePool,
    request_id: &str,
) -> Result<BudgetRequestRecord, String> {
    sqlx::query_as::<_, BudgetRequestRecord>(
        r#"
        SELECT id, task_id, requested_by, reason, requested_increment, current_budget, current_usage,
               status, approved_increment, resolution_note, created_at, updated_at, resolved_at
        FROM aop_budget_requests
        WHERE id = ?
        "#,
    )
    .bind(request_id.trim())
    .fetch_optional(pool)
    .await
    .map_err(|error| format!("Failed to fetch budget request: {error}"))?
    .ok_or_else(|| format!("Budget request '{request_id}' not found"))
}

pub async fn get_latest_pending_request_for_task(
    pool: &SqlitePool,
    task_id: &str,
) -> Result<Option<BudgetRequestRecord>, String> {
    sqlx::query_as::<_, BudgetRequestRecord>(
        r#"
        SELECT id, task_id, requested_by, reason, requested_increment, current_budget, current_usage,
               status, approved_increment, resolution_note, created_at, updated_at, resolved_at
        FROM aop_budget_requests
        WHERE task_id = ? AND status = 'pending'
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .bind(task_id.trim())
    .fetch_optional(pool)
    .await
    .map_err(|error| format!("Failed to fetch pending budget request: {error}"))
}

fn build_resolution_note(decided_by: Option<&str>, reason: Option<&str>) -> Option<String> {
    let actor = decided_by
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("unknown");
    let reason = reason
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("no reason provided");
    Some(format!("decidedBy={actor}; reason={reason}"))
}

fn validate_create_input(input: &CreateBudgetRequestInput) -> Result<(), String> {
    if input.task_id.trim().is_empty() {
        return Err("taskId is required".to_string());
    }
    if input.requested_by.trim().is_empty() {
        return Err("requestedBy is required".to_string());
    }
    if input.reason.trim().is_empty() {
        return Err("reason is required".to_string());
    }
    if input.requested_increment <= 0 {
        return Err("requestedIncrement must be greater than 0".to_string());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use sqlx::sqlite::SqlitePoolOptions;

    use super::*;
    use crate::db;
    use crate::db::tasks::{self, CreateTaskInput, TaskStatus, UpdateTaskStatusInput};

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
    async fn create_and_auto_approve_budget_request_increases_task_budget() {
        let pool = setup_test_pool().await;
        let task = tasks::create_task(
            &pool,
            CreateTaskInput {
                parent_id: None,
                tier: 2,
                domain: "platform".to_string(),
                objective: "task".to_string(),
                token_budget: 2000,
            },
        )
        .await
        .expect("task should be created");

        let request = create_budget_request(
            &pool,
            CreateBudgetRequestInput {
                task_id: task.id.clone(),
                requested_by: "runtime".to_string(),
                reason: "insufficient headroom".to_string(),
                requested_increment: 500,
                auto_approve: Some(true),
            },
        )
        .await
        .expect("request should be created and approved");

        assert_eq!(request.status, "approved");
        assert_eq!(request.approved_increment, Some(500));

        let updated_task = tasks::get_task_by_id(&pool, &task.id)
            .await
            .expect("task should exist");
        assert_eq!(updated_task.token_budget, 2500);
    }

    #[tokio::test]
    async fn pending_request_can_be_approved_and_resume_task() {
        let pool = setup_test_pool().await;
        let task = tasks::create_task(
            &pool,
            CreateTaskInput {
                parent_id: None,
                tier: 2,
                domain: "auth".to_string(),
                objective: "domain task".to_string(),
                token_budget: 1600,
            },
        )
        .await
        .expect("task should be created");

        tasks::update_task_status(
            &pool,
            UpdateTaskStatusInput {
                task_id: task.id.clone(),
                status: TaskStatus::Executing,
                error_message: None,
            },
        )
        .await
        .expect("task should move to executing");

        tasks::control_task(
            &pool,
            ControlTaskInput {
                task_id: task.id.clone(),
                action: TaskControlAction::Pause,
                include_descendants: Some(false),
                reason: None,
            },
        )
        .await
        .expect("task should pause");

        let request = create_budget_request(
            &pool,
            CreateBudgetRequestInput {
                task_id: task.id.clone(),
                requested_by: "tier2".to_string(),
                reason: "need more tokens".to_string(),
                requested_increment: 400,
                auto_approve: Some(false),
            },
        )
        .await
        .expect("request should be pending");

        let resolved = resolve_budget_request(
            &pool,
            ResolveBudgetRequestInput {
                request_id: request.id,
                decision: BudgetRequestDecision::Approve,
                approved_increment: Some(650),
                reason: Some("approved by user".to_string()),
                decided_by: Some("ui".to_string()),
                resume_task: Some(true),
            },
        )
        .await
        .expect("request should approve");

        assert_eq!(resolved.status, "approved");
        assert_eq!(resolved.approved_increment, Some(650));

        let task_after = tasks::get_task_by_id(&pool, &task.id)
            .await
            .expect("task should exist");
        assert_eq!(task_after.token_budget, 2250);
        assert_eq!(task_after.status, "executing");
    }
}
