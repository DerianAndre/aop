use std::time::Duration;

use sqlx::SqlitePool;
use tokio::time::sleep;

use crate::db::budget_requests::{self, CreateBudgetRequestInput};
use crate::db::metrics;
use crate::db::tasks;

pub async fn record_task_activity(
    pool: &SqlitePool,
    actor: &str,
    action: &str,
    task_id: &str,
    details: &str,
) -> Result<(), String> {
    metrics::record_audit_event(pool, actor, action, Some(task_id), Some(details)).await
}

pub async fn cooperative_checkpoint(
    pool: &SqlitePool,
    task_id: &str,
    actor: &str,
    stage: &str,
) -> Result<(), String> {
    let mut observed_pause = false;

    loop {
        let task = tasks::get_task_by_id(pool, task_id).await?;
        match task.status.as_str() {
            "paused" => {
                if !observed_pause {
                    let details = format!(
                        "paused_wait stage={stage} task={} tier={}",
                        task.id, task.tier
                    );
                    let _ = metrics::record_audit_event(
                        pool,
                        actor,
                        "task_pause_observed",
                        Some(task_id),
                        Some(&details),
                    )
                    .await;
                    observed_pause = true;
                }
                sleep(Duration::from_millis(350)).await;
            }
            "failed" => {
                let reason = task
                    .error_message
                    .clone()
                    .unwrap_or_else(|| "task marked as failed".to_string());
                let details = format!("stop_observed stage={stage} reason={reason}");
                let _ = metrics::record_audit_event(
                    pool,
                    actor,
                    "task_stop_observed",
                    Some(task_id),
                    Some(&details),
                )
                .await;
                return Err(format!("Task '{task_id}' stopped: {reason}"));
            }
            "completed" => {
                return Err(format!(
                    "Task '{task_id}' is already completed; execution checkpoint '{stage}' aborted"
                ));
            }
            _ => {
                if observed_pause {
                    let details = format!("resumed stage={stage}");
                    let _ = metrics::record_audit_event(
                        pool,
                        actor,
                        "task_resume_observed",
                        Some(task_id),
                        Some(&details),
                    )
                    .await;
                }
                return Ok(());
            }
        }
    }
}

pub async fn ensure_budget_headroom(
    pool: &SqlitePool,
    task_id: &str,
    actor: &str,
    stage: &str,
    planned_tokens: u32,
) -> Result<(), String> {
    if planned_tokens == 0 {
        return Ok(());
    }

    let task = tasks::get_task_by_id(pool, task_id).await?;
    let remaining = task.token_budget.saturating_sub(task.token_usage);
    let required = i64::from(planned_tokens.max(80));

    if remaining >= required {
        return Ok(());
    }

    if budget_requests::get_latest_pending_request_for_task(pool, task_id)
        .await?
        .is_some()
    {
        let details = format!(
            "stage={stage} task={} remaining={} required={} pendingRequest=true",
            task.id, remaining, required
        );
        let _ = metrics::record_audit_event(
            pool,
            actor,
            "token_budget_increase_pending",
            Some(task_id),
            Some(&details),
        )
        .await;
        return Ok(());
    }

    let suggested_increment = suggested_increment(task.token_budget, remaining, required);
    let auto_approve = auto_approve_budget_requests_enabled();
    let reason = format!(
        "stage={stage}; remaining={remaining}; required={required}; objective={}",
        task.objective
    );
    let request = budget_requests::create_budget_request(
        pool,
        CreateBudgetRequestInput {
            task_id: task.id.clone(),
            requested_by: actor.to_string(),
            reason,
            requested_increment: suggested_increment,
            auto_approve: Some(auto_approve),
        },
    )
    .await?;

    let action = if request.status == "approved" {
        "token_budget_auto_increase_applied"
    } else {
        "token_budget_increase_requested"
    };
    let details = format!(
        "stage={stage} requestId={} task={} status={} requestedIncrement={} approvedIncrement={}",
        request.id,
        task.id,
        request.status,
        request.requested_increment,
        request.approved_increment.unwrap_or(0)
    );
    let _ = metrics::record_audit_event(pool, actor, action, Some(task_id), Some(&details)).await;

    Ok(())
}

fn suggested_increment(current_budget: i64, remaining: i64, required: i64) -> i64 {
    let deficit = required.saturating_sub(remaining).max(0);
    let floor = ((current_budget.max(1) as f64) * 0.25).ceil() as i64;
    deficit.max(floor).max(250)
}

fn auto_approve_budget_requests_enabled() -> bool {
    std::env::var("AOP_AUTO_APPROVE_BUDGET_REQUESTS")
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(true)
}
