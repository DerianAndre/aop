use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MutationStatus {
    Proposed,
    Validated,
    ValidatedNoTests,
    Applied,
    Rejected,
}

impl MutationStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            MutationStatus::Proposed => "proposed",
            MutationStatus::Validated => "validated",
            MutationStatus::ValidatedNoTests => "validated_no_tests",
            MutationStatus::Applied => "applied",
            MutationStatus::Rejected => "rejected",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct MutationRecord {
    pub id: String,
    pub task_id: String,
    pub agent_uid: String,
    pub file_path: String,
    pub diff_content: String,
    pub intent_description: Option<String>,
    pub intent_hash: Option<String>,
    pub confidence: f64,
    pub test_result: Option<String>,
    pub test_exit_code: Option<i64>,
    pub rejection_reason: Option<String>,
    pub rejected_at_step: Option<String>,
    pub status: String,
    pub proposed_at: i64,
    pub applied_at: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct CreateMutationInput {
    pub task_id: String,
    pub agent_uid: String,
    pub file_path: String,
    pub diff_content: String,
    pub intent_description: Option<String>,
    pub intent_hash: Option<String>,
    pub confidence: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListTaskMutationsInput {
    pub task_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateMutationStatusInput {
    pub mutation_id: String,
    pub status: MutationStatus,
    pub test_result: Option<String>,
    pub test_exit_code: Option<i64>,
    pub rejection_reason: Option<String>,
    pub rejected_at_step: Option<String>,
}

pub async fn create_mutation(
    pool: &SqlitePool,
    input: CreateMutationInput,
) -> Result<MutationRecord, String> {
    validate_create_mutation_input(&input)?;

    let id = Uuid::new_v4().to_string();
    let proposed_at = Utc::now().timestamp();

    sqlx::query(
        r#"
        INSERT INTO aop_mutations (
            id, task_id, agent_uid, file_path, diff_content, intent_description, intent_hash,
            confidence, test_result, test_exit_code, rejection_reason, rejected_at_step,
            status, proposed_at, applied_at
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, NULL, NULL, NULL, NULL, ?, ?, NULL)
        "#,
    )
    .bind(&id)
    .bind(input.task_id.trim())
    .bind(input.agent_uid.trim())
    .bind(input.file_path.trim())
    .bind(input.diff_content.trim())
    .bind(
        input
            .intent_description
            .map(|value| value.trim().to_string()),
    )
    .bind(input.intent_hash.map(|value| value.trim().to_string()))
    .bind(input.confidence)
    .bind(MutationStatus::Proposed.as_str())
    .bind(proposed_at)
    .execute(pool)
    .await
    .map_err(|error| format!("Failed to create mutation: {error}"))?;

    get_mutation_by_id(pool, &id).await
}

pub async fn update_mutation_status(
    pool: &SqlitePool,
    input: UpdateMutationStatusInput,
) -> Result<MutationRecord, String> {
    if input.mutation_id.trim().is_empty() {
        return Err("mutationId is required".to_string());
    }

    let mut current = get_mutation_by_id(pool, input.mutation_id.trim()).await?;
    if let Some(value) = input.test_result {
        current.test_result = Some(value);
    }
    if let Some(value) = input.test_exit_code {
        current.test_exit_code = Some(value);
    }
    if let Some(value) = input.rejection_reason {
        current.rejection_reason = Some(value);
    }
    if let Some(value) = input.rejected_at_step {
        current.rejected_at_step = Some(value);
    }

    let applied_at = if input.status == MutationStatus::Applied {
        Some(Utc::now().timestamp())
    } else {
        current.applied_at
    };

    sqlx::query(
        r#"
        UPDATE aop_mutations
        SET status = ?, test_result = ?, test_exit_code = ?, rejection_reason = ?, rejected_at_step = ?, applied_at = ?
        WHERE id = ?
        "#,
    )
    .bind(input.status.as_str())
    .bind(current.test_result)
    .bind(current.test_exit_code)
    .bind(current.rejection_reason)
    .bind(current.rejected_at_step)
    .bind(applied_at)
    .bind(input.mutation_id.trim())
    .execute(pool)
    .await
    .map_err(|error| format!("Failed to update mutation status: {error}"))?;

    get_mutation_by_id(pool, input.mutation_id.trim()).await
}

pub async fn list_mutations_for_task(
    pool: &SqlitePool,
    input: ListTaskMutationsInput,
) -> Result<Vec<MutationRecord>, String> {
    if input.task_id.trim().is_empty() {
        return Err("taskId is required".to_string());
    }

    sqlx::query_as::<_, MutationRecord>(
        r#"
        SELECT
            id, task_id, agent_uid, file_path, diff_content, intent_description, intent_hash,
            confidence, test_result, test_exit_code, rejection_reason, rejected_at_step,
            status, proposed_at, applied_at
        FROM aop_mutations
        WHERE task_id = ?
        ORDER BY proposed_at DESC
        "#,
    )
    .bind(input.task_id.trim())
    .fetch_all(pool)
    .await
    .map_err(|error| format!("Failed to list mutations for task: {error}"))
}

pub async fn get_mutation_by_id(
    pool: &SqlitePool,
    mutation_id: &str,
) -> Result<MutationRecord, String> {
    sqlx::query_as::<_, MutationRecord>(
        r#"
        SELECT
            id, task_id, agent_uid, file_path, diff_content, intent_description, intent_hash,
            confidence, test_result, test_exit_code, rejection_reason, rejected_at_step,
            status, proposed_at, applied_at
        FROM aop_mutations
        WHERE id = ?
        "#,
    )
    .bind(mutation_id)
    .fetch_optional(pool)
    .await
    .map_err(|error| format!("Failed to fetch mutation: {error}"))?
    .ok_or_else(|| format!("Mutation '{mutation_id}' not found"))
}

fn validate_create_mutation_input(input: &CreateMutationInput) -> Result<(), String> {
    if input.task_id.trim().is_empty() {
        return Err("taskId is required".to_string());
    }
    if input.agent_uid.trim().is_empty() {
        return Err("agentUid is required".to_string());
    }
    if input.file_path.trim().is_empty() {
        return Err("filePath is required".to_string());
    }
    if input.diff_content.trim().is_empty() {
        return Err("diffContent is required".to_string());
    }
    if !(0.0..=1.0).contains(&input.confidence) {
        return Err("confidence must be between 0.0 and 1.0".to_string());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use sqlx::sqlite::SqlitePoolOptions;

    use crate::db;
    use crate::db::tasks::{self, CreateTaskInput};

    use super::*;

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
    async fn create_and_list_mutations_for_task() {
        let pool = setup_test_pool().await;
        let task = tasks::create_task(
            &pool,
            CreateTaskInput {
                parent_id: None,
                tier: 2,
                domain: "frontend".to_string(),
                objective: "Refactor session hook".to_string(),
                token_budget: 2200,
            },
        )
        .await
        .expect("task should be created");

        let created = create_mutation(
            &pool,
            CreateMutationInput {
                task_id: task.id.clone(),
                agent_uid: Uuid::new_v4().to_string(),
                file_path: "src/session.ts".to_string(),
                diff_content: "--- a/src/session.ts\n+++ b/src/session.ts\n".to_string(),
                intent_description: Some("Improve loading state handling".to_string()),
                intent_hash: Some("abc123".to_string()),
                confidence: 0.78,
            },
        )
        .await
        .expect("mutation should be created");

        assert_eq!(created.task_id, task.id);
        assert_eq!(created.status, "proposed");

        let listed = list_mutations_for_task(
            &pool,
            ListTaskMutationsInput {
                task_id: task.id.clone(),
            },
        )
        .await
        .expect("mutations should list");

        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, created.id);
    }

    #[tokio::test]
    async fn updates_mutation_status_to_validated() {
        let pool = setup_test_pool().await;
        let task = tasks::create_task(
            &pool,
            CreateTaskInput {
                parent_id: None,
                tier: 2,
                domain: "frontend".to_string(),
                objective: "Refactor session hook".to_string(),
                token_budget: 2200,
            },
        )
        .await
        .expect("task should be created");

        let created = create_mutation(
            &pool,
            CreateMutationInput {
                task_id: task.id,
                agent_uid: Uuid::new_v4().to_string(),
                file_path: "src/session.ts".to_string(),
                diff_content: "--- a/src/session.ts\n+++ b/src/session.ts\n".to_string(),
                intent_description: Some("Improve loading state handling".to_string()),
                intent_hash: Some("abc123".to_string()),
                confidence: 0.78,
            },
        )
        .await
        .expect("mutation should be created");

        let updated = update_mutation_status(
            &pool,
            UpdateMutationStatusInput {
                mutation_id: created.id,
                status: MutationStatus::Validated,
                test_result: Some("ci passed".to_string()),
                test_exit_code: Some(0),
                rejection_reason: None,
                rejected_at_step: None,
            },
        )
        .await
        .expect("mutation should update");

        assert_eq!(updated.status, "validated");
        assert_eq!(updated.test_exit_code, Some(0));
    }
}
