use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct AuditLogEntry {
    pub id: i64,
    pub timestamp: i64,
    pub actor: String,
    pub action: String,
    pub target_id: Option<String>,
    pub details: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListAuditLogInput {
    pub target_id: Option<String>,
    pub limit: Option<u32>,
}

pub async fn record_audit_event(
    pool: &SqlitePool,
    actor: &str,
    action: &str,
    target_id: Option<&str>,
    details: Option<&str>,
) -> Result<(), String> {
    if actor.trim().is_empty() {
        return Err("actor is required".to_string());
    }
    if action.trim().is_empty() {
        return Err("action is required".to_string());
    }

    sqlx::query(
        r#"
        INSERT INTO aop_audit_log (timestamp, actor, action, target_id, details)
        VALUES (?, ?, ?, ?, ?)
        "#,
    )
    .bind(Utc::now().timestamp())
    .bind(actor.trim())
    .bind(action.trim())
    .bind(target_id.map(str::trim).filter(|value| !value.is_empty()))
    .bind(details)
    .execute(pool)
    .await
    .map_err(|error| format!("Failed to record audit event: {error}"))?;

    Ok(())
}

pub async fn list_audit_log(
    pool: &SqlitePool,
    input: ListAuditLogInput,
) -> Result<Vec<AuditLogEntry>, String> {
    let limit = i64::from(input.limit.unwrap_or(50).clamp(1, 200));

    match input.target_id.map(|value| value.trim().to_string()) {
        Some(target_id) if !target_id.is_empty() => sqlx::query_as::<_, AuditLogEntry>(
            r#"
            SELECT id, timestamp, actor, action, target_id, details
            FROM aop_audit_log
            WHERE target_id = ?
            ORDER BY id DESC
            LIMIT ?
            "#,
        )
        .bind(target_id)
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(|error| format!("Failed to list audit logs for target: {error}")),
        _ => sqlx::query_as::<_, AuditLogEntry>(
            r#"
            SELECT id, timestamp, actor, action, target_id, details
            FROM aop_audit_log
            ORDER BY id DESC
            LIMIT ?
            "#,
        )
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(|error| format!("Failed to list audit logs: {error}")),
    }
}

#[cfg(test)]
mod tests {
    use sqlx::sqlite::SqlitePoolOptions;

    use crate::db;

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
    async fn records_and_filters_audit_logs() {
        let pool = setup_test_pool().await;

        record_audit_event(
            &pool,
            "tier1",
            "diff_proposed",
            Some("mutation-1"),
            Some("{\"status\":\"ok\"}"),
        )
        .await
        .expect("audit event should be recorded");
        record_audit_event(&pool, "tier1", "diff_applied", Some("mutation-2"), None)
            .await
            .expect("audit event should be recorded");

        let filtered = list_audit_log(
            &pool,
            ListAuditLogInput {
                target_id: Some("mutation-1".to_string()),
                limit: Some(10),
            },
        )
        .await
        .expect("audit logs should filter");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].action, "diff_proposed");

        let all = list_audit_log(
            &pool,
            ListAuditLogInput {
                target_id: None,
                limit: Some(10),
            },
        )
        .await
        .expect("audit logs should list");
        assert_eq!(all.len(), 2);
    }
}
