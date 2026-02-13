use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{QueryBuilder, Sqlite, SqlitePool};
use tokio::time::sleep;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct AgentRunRecord {
    pub id: String,
    pub root_task_id: Option<String>,
    pub task_id: Option<String>,
    pub tier: Option<i64>,
    pub actor: String,
    pub persona: Option<String>,
    pub skill: Option<String>,
    pub provider: Option<String>,
    pub model_id: Option<String>,
    pub adapter_kind: Option<String>,
    pub status: String,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub tokens_in: i64,
    pub tokens_out: i64,
    pub token_delta: i64,
    pub cost_usd: Option<f64>,
    pub metadata_json: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct AgentEventRecord {
    pub id: i64,
    pub run_id: Option<String>,
    pub root_task_id: Option<String>,
    pub task_id: Option<String>,
    pub tier: Option<i64>,
    pub actor: String,
    pub action: String,
    pub status: Option<String>,
    pub phase: Option<String>,
    pub message: Option<String>,
    pub provider: Option<String>,
    pub model_id: Option<String>,
    pub persona: Option<String>,
    pub skill: Option<String>,
    pub mcp_server: Option<String>,
    pub mcp_tool: Option<String>,
    pub latency_ms: Option<i64>,
    pub retry_count: Option<i64>,
    pub tokens_in: Option<i64>,
    pub tokens_out: Option<i64>,
    pub token_delta: Option<i64>,
    pub cost_usd: Option<f64>,
    pub payload_json: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ModelHealthRecord {
    pub provider: String,
    pub model_id: String,
    pub total_calls: i64,
    pub success_calls: i64,
    pub failed_calls: i64,
    pub avg_latency_ms: f64,
    pub avg_cost_usd: f64,
    pub quality_score: f64,
    pub last_error: Option<String>,
    pub last_used_at: Option<i64>,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListAgentRunsInput {
    pub root_task_id: Option<String>,
    pub task_id: Option<String>,
    pub actor: Option<String>,
    pub tier: Option<i64>,
    pub status: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListAgentEventsInput {
    pub root_task_id: Option<String>,
    pub task_id: Option<String>,
    pub actor: Option<String>,
    pub action: Option<String>,
    pub since_id: Option<i64>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetMissionControlSnapshotInput {
    pub root_task_id: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MissionControlSnapshot {
    pub generated_at: i64,
    pub active_runs: Vec<AgentRunRecord>,
    pub recent_events: Vec<AgentEventRecord>,
    pub model_health: Vec<ModelHealthRecord>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveTelemetryInput {
    pub retention_days: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveTelemetryResult {
    pub retention_days: u32,
    pub events_archived: usize,
    pub runs_archived: usize,
    pub archive_file: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct NewAgentEvent {
    pub run_id: Option<String>,
    pub root_task_id: Option<String>,
    pub task_id: Option<String>,
    pub tier: Option<i64>,
    pub actor: String,
    pub action: String,
    pub status: Option<String>,
    pub phase: Option<String>,
    pub message: Option<String>,
    pub provider: Option<String>,
    pub model_id: Option<String>,
    pub persona: Option<String>,
    pub skill: Option<String>,
    pub adapter_kind: Option<String>,
    pub mcp_server: Option<String>,
    pub mcp_tool: Option<String>,
    pub latency_ms: Option<i64>,
    pub retry_count: Option<i64>,
    pub tokens_in: Option<i64>,
    pub tokens_out: Option<i64>,
    pub token_delta: Option<i64>,
    pub cost_usd: Option<f64>,
    pub payload: Option<Value>,
}

#[derive(Debug, Clone, Default)]
pub struct ModelCallOutcomeInput {
    pub provider: String,
    pub model_id: String,
    pub success: bool,
    pub latency_ms: Option<i64>,
    pub cost_usd: Option<f64>,
    pub error: Option<String>,
}

#[derive(Debug, Default)]
struct ParsedTelemetryFields {
    status: Option<String>,
    phase: Option<String>,
    message: Option<String>,
    provider: Option<String>,
    model_id: Option<String>,
    persona: Option<String>,
    skill: Option<String>,
    mcp_server: Option<String>,
    mcp_tool: Option<String>,
    latency_ms: Option<i64>,
    retry_count: Option<i64>,
    tokens_in: Option<i64>,
    tokens_out: Option<i64>,
    token_delta: Option<i64>,
    cost_usd: Option<f64>,
    payload: Option<Value>,
}

pub async fn record_task_activity_event(
    pool: &SqlitePool,
    actor: &str,
    action: &str,
    task_id: Option<&str>,
    details: Option<&str>,
) -> Result<(), String> {
    if actor.trim().is_empty() || action.trim().is_empty() {
        return Ok(());
    }

    let parsed = parse_details(details);
    let inferred_status = status_from_action(action);
    let mut status = parsed.status.clone().or(inferred_status.clone());
    if status.is_none() {
        status = Some("executing".to_string());
    }

    let event = NewAgentEvent {
        task_id: task_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned),
        actor: actor.trim().to_string(),
        action: action.trim().to_string(),
        status,
        phase: parsed.phase,
        message: parsed.message.or_else(|| details.map(ToOwned::to_owned)),
        provider: parsed.provider,
        model_id: parsed.model_id,
        persona: parsed.persona,
        skill: parsed.skill,
        mcp_server: parsed.mcp_server,
        mcp_tool: parsed.mcp_tool,
        latency_ms: parsed.latency_ms,
        retry_count: parsed.retry_count,
        tokens_in: parsed.tokens_in,
        tokens_out: parsed.tokens_out,
        token_delta: parsed.token_delta,
        cost_usd: parsed.cost_usd,
        payload: parsed.payload,
        ..Default::default()
    };

    record_agent_event(pool, event).await
}

pub async fn record_agent_event(pool: &SqlitePool, mut event: NewAgentEvent) -> Result<(), String> {
    if event.actor.trim().is_empty() {
        return Err("actor is required".to_string());
    }
    if event.action.trim().is_empty() {
        return Err("action is required".to_string());
    }

    if event.task_id.is_none() {
        event.task_id = parse_task_id_from_message(event.message.as_deref());
    }

    if let Some(task_id) = event.task_id.clone() {
        let (root_task_id, tier) = infer_task_scope(pool, task_id.as_str()).await?;
        if event.root_task_id.is_none() {
            event.root_task_id = root_task_id;
        }
        if event.tier.is_none() {
            event.tier = tier;
        }
    }

    let now = Utc::now().timestamp();
    let run_id = event.run_id.clone().unwrap_or_else(|| {
        format!(
            "{}::{}",
            event.actor.trim(),
            event
                .task_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("global")
        )
    });

    let status = event
        .status
        .clone()
        .or_else(|| status_from_action(event.action.as_str()))
        .unwrap_or_else(|| "executing".to_string());
    let ended_at = if is_terminal_status(status.as_str()) {
        Some(now)
    } else {
        None
    };
    let payload_json = event.payload.as_ref().map(|value| value.to_string());

    upsert_run(
        pool,
        &AgentRunRecord {
            id: run_id.clone(),
            root_task_id: event.root_task_id.clone(),
            task_id: event.task_id.clone(),
            tier: event.tier,
            actor: event.actor.clone(),
            persona: event.persona.clone(),
            skill: event.skill.clone(),
            provider: event.provider.clone(),
            model_id: event.model_id.clone(),
            adapter_kind: event.adapter_kind.clone(),
            status: status.clone(),
            started_at: now,
            ended_at,
            tokens_in: event.tokens_in.unwrap_or(0),
            tokens_out: event.tokens_out.unwrap_or(0),
            token_delta: event.token_delta.unwrap_or(0),
            cost_usd: event.cost_usd,
            metadata_json: None,
        },
    )
    .await?;

    sqlx::query(
        r#"
        INSERT INTO aop_agent_events (
            run_id, root_task_id, task_id, tier, actor, action, status, phase, message,
            provider, model_id, persona, skill, mcp_server, mcp_tool, latency_ms, retry_count,
            tokens_in, tokens_out, token_delta, cost_usd, payload_json, created_at
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(run_id)
    .bind(event.root_task_id)
    .bind(event.task_id)
    .bind(event.tier)
    .bind(event.actor.trim())
    .bind(event.action.trim())
    .bind(status)
    .bind(event.phase)
    .bind(event.message)
    .bind(event.provider)
    .bind(event.model_id)
    .bind(event.persona)
    .bind(event.skill)
    .bind(event.mcp_server)
    .bind(event.mcp_tool)
    .bind(event.latency_ms)
    .bind(event.retry_count)
    .bind(event.tokens_in)
    .bind(event.tokens_out)
    .bind(event.token_delta)
    .bind(event.cost_usd)
    .bind(payload_json)
    .bind(now)
    .execute(pool)
    .await
    .map_err(|error| format!("Failed to record agent event: {error}"))?;

    Ok(())
}

pub async fn list_agent_runs(
    pool: &SqlitePool,
    input: ListAgentRunsInput,
) -> Result<Vec<AgentRunRecord>, String> {
    let limit = i64::from(input.limit.unwrap_or(80).clamp(1, 500));
    let mut query_builder: QueryBuilder<'_, Sqlite> =
        QueryBuilder::new("SELECT id, root_task_id, task_id, tier, actor, persona, skill, provider, model_id, adapter_kind, status, started_at, ended_at, tokens_in, tokens_out, token_delta, cost_usd, metadata_json FROM aop_agent_runs WHERE 1=1");

    if let Some(root_task_id) = input
        .root_task_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        query_builder.push(" AND root_task_id = ").push_bind(root_task_id);
    }
    if let Some(task_id) = input
        .task_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        query_builder.push(" AND task_id = ").push_bind(task_id);
    }
    if let Some(actor) = input
        .actor
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        query_builder.push(" AND actor = ").push_bind(actor);
    }
    if let Some(tier) = input.tier {
        query_builder.push(" AND tier = ").push_bind(tier);
    }
    if let Some(status) = input
        .status
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        query_builder.push(" AND status = ").push_bind(status);
    }

    query_builder
        .push(" ORDER BY started_at DESC LIMIT ")
        .push_bind(limit);

    query_builder
        .build_query_as::<AgentRunRecord>()
        .fetch_all(pool)
        .await
        .map_err(|error| format!("Failed to list agent runs: {error}"))
}

pub async fn list_agent_events(
    pool: &SqlitePool,
    input: ListAgentEventsInput,
) -> Result<Vec<AgentEventRecord>, String> {
    let limit = i64::from(input.limit.unwrap_or(250).clamp(1, 2000));
    let mut query_builder: QueryBuilder<'_, Sqlite> = QueryBuilder::new(
        "SELECT id, run_id, root_task_id, task_id, tier, actor, action, status, phase, message, provider, model_id, persona, skill, mcp_server, mcp_tool, latency_ms, retry_count, tokens_in, tokens_out, token_delta, cost_usd, payload_json, created_at FROM aop_agent_events WHERE 1=1",
    );

    if let Some(root_task_id) = input
        .root_task_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        query_builder.push(" AND root_task_id = ").push_bind(root_task_id);
    }
    if let Some(task_id) = input
        .task_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        query_builder.push(" AND task_id = ").push_bind(task_id);
    }
    if let Some(actor) = input
        .actor
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        query_builder.push(" AND actor = ").push_bind(actor);
    }
    if let Some(action) = input
        .action
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        query_builder.push(" AND action = ").push_bind(action);
    }
    if let Some(since_id) = input.since_id {
        query_builder.push(" AND id > ").push_bind(since_id);
    }

    query_builder
        .push(" ORDER BY id DESC LIMIT ")
        .push_bind(limit);

    query_builder
        .build_query_as::<AgentEventRecord>()
        .fetch_all(pool)
        .await
        .map_err(|error| format!("Failed to list agent events: {error}"))
}

pub async fn list_model_health(
    pool: &SqlitePool,
    limit: Option<u32>,
) -> Result<Vec<ModelHealthRecord>, String> {
    let effective_limit = i64::from(limit.unwrap_or(100).clamp(1, 500));
    sqlx::query_as::<_, ModelHealthRecord>(
        r#"
        SELECT
            provider, model_id, total_calls, success_calls, failed_calls,
            avg_latency_ms, avg_cost_usd, quality_score, last_error, last_used_at, updated_at
        FROM aop_model_health
        ORDER BY updated_at DESC
        LIMIT ?
        "#,
    )
    .bind(effective_limit)
    .fetch_all(pool)
    .await
    .map_err(|error| format!("Failed to list model health: {error}"))
}

pub async fn get_model_health(
    pool: &SqlitePool,
    provider: &str,
    model_id: &str,
) -> Result<Option<ModelHealthRecord>, String> {
    sqlx::query_as::<_, ModelHealthRecord>(
        r#"
        SELECT
            provider, model_id, total_calls, success_calls, failed_calls,
            avg_latency_ms, avg_cost_usd, quality_score, last_error, last_used_at, updated_at
        FROM aop_model_health
        WHERE provider = ? AND model_id = ?
        "#,
    )
    .bind(provider.trim())
    .bind(model_id.trim())
    .fetch_optional(pool)
    .await
    .map_err(|error| format!("Failed to fetch model health: {error}"))
}

pub async fn update_model_health(
    pool: &SqlitePool,
    input: ModelCallOutcomeInput,
) -> Result<ModelHealthRecord, String> {
    let provider = input.provider.trim();
    let model_id = input.model_id.trim();
    if provider.is_empty() || model_id.is_empty() {
        return Err("provider and modelId are required".to_string());
    }

    let now = Utc::now().timestamp();
    let current = get_model_health(pool, provider, model_id).await?;
    let next = if let Some(current) = current {
        let total_calls = current.total_calls.saturating_add(1);
        let success_calls = current.success_calls + i64::from(input.success);
        let failed_calls = current.failed_calls + i64::from(!input.success);
        let avg_latency_ms = input
            .latency_ms
            .map(|value| ewma(current.avg_latency_ms, value as f64, 0.20))
            .unwrap_or(current.avg_latency_ms);
        let avg_cost_usd = input
            .cost_usd
            .map(|value| ewma(current.avg_cost_usd, value, 0.20))
            .unwrap_or(current.avg_cost_usd);
        let quality_score = if input.success {
            (current.quality_score + 0.02).clamp(0.05, 0.99)
        } else {
            (current.quality_score - 0.08).clamp(0.05, 0.99)
        };
        let last_error = if input.success {
            None
        } else {
            input.error.clone().filter(|value| !value.trim().is_empty())
        };

        sqlx::query(
            r#"
            UPDATE aop_model_health
            SET total_calls = ?, success_calls = ?, failed_calls = ?,
                avg_latency_ms = ?, avg_cost_usd = ?, quality_score = ?,
                last_error = ?, last_used_at = ?, updated_at = ?
            WHERE provider = ? AND model_id = ?
            "#,
        )
        .bind(total_calls)
        .bind(success_calls)
        .bind(failed_calls)
        .bind(avg_latency_ms)
        .bind(avg_cost_usd)
        .bind(quality_score)
        .bind(last_error.clone())
        .bind(now)
        .bind(now)
        .bind(provider)
        .bind(model_id)
        .execute(pool)
        .await
        .map_err(|error| format!("Failed to update model health: {error}"))?;

        ModelHealthRecord {
            provider: provider.to_string(),
            model_id: model_id.to_string(),
            total_calls,
            success_calls,
            failed_calls,
            avg_latency_ms,
            avg_cost_usd,
            quality_score,
            last_error,
            last_used_at: Some(now),
            updated_at: now,
        }
    } else {
        let total_calls = 1_i64;
        let success_calls = i64::from(input.success);
        let failed_calls = i64::from(!input.success);
        let avg_latency_ms = input.latency_ms.unwrap_or(0) as f64;
        let avg_cost_usd = input.cost_usd.unwrap_or(0.0);
        let quality_score = if input.success { 0.72 } else { 0.62 };
        let last_error = if input.success {
            None
        } else {
            input.error.clone().filter(|value| !value.trim().is_empty())
        };

        sqlx::query(
            r#"
            INSERT INTO aop_model_health (
                provider, model_id, total_calls, success_calls, failed_calls,
                avg_latency_ms, avg_cost_usd, quality_score, last_error, last_used_at, updated_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(provider)
        .bind(model_id)
        .bind(total_calls)
        .bind(success_calls)
        .bind(failed_calls)
        .bind(avg_latency_ms)
        .bind(avg_cost_usd)
        .bind(quality_score)
        .bind(last_error.clone())
        .bind(now)
        .bind(now)
        .execute(pool)
        .await
        .map_err(|error| format!("Failed to insert model health: {error}"))?;

        ModelHealthRecord {
            provider: provider.to_string(),
            model_id: model_id.to_string(),
            total_calls,
            success_calls,
            failed_calls,
            avg_latency_ms,
            avg_cost_usd,
            quality_score,
            last_error,
            last_used_at: Some(now),
            updated_at: now,
        }
    };

    Ok(next)
}

pub async fn get_mission_control_snapshot(
    pool: &SqlitePool,
    input: GetMissionControlSnapshotInput,
) -> Result<MissionControlSnapshot, String> {
    let limit = input.limit.unwrap_or(80).clamp(10, 300);
    let root_filter = input
        .root_task_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);

    let active_runs = list_agent_runs(
        pool,
        ListAgentRunsInput {
            root_task_id: root_filter.clone(),
            task_id: None,
            actor: None,
            tier: None,
            status: None,
            limit: Some(limit),
        },
    )
    .await?
    .into_iter()
    .filter(|run| matches!(run.status.as_str(), "pending" | "executing" | "paused"))
    .collect::<Vec<_>>();

    let recent_events = list_agent_events(
        pool,
        ListAgentEventsInput {
            root_task_id: root_filter,
            task_id: None,
            actor: None,
            action: None,
            since_id: None,
            limit: Some(limit.saturating_mul(3)),
        },
    )
    .await?;

    let model_health = list_model_health(pool, Some(100)).await?;

    Ok(MissionControlSnapshot {
        generated_at: Utc::now().timestamp(),
        active_runs,
        recent_events,
        model_health,
    })
}

pub async fn archive_telemetry(
    pool: &SqlitePool,
    archive_root: &Path,
    retention_days: u32,
) -> Result<ArchiveTelemetryResult, String> {
    let effective_days = retention_days.clamp(1, 365);
    let cutoff = Utc::now().timestamp() - i64::from(effective_days) * 86_400;

    let events = sqlx::query_as::<_, AgentEventRecord>(
        r#"
        SELECT id, run_id, root_task_id, task_id, tier, actor, action, status, phase, message,
               provider, model_id, persona, skill, mcp_server, mcp_tool, latency_ms, retry_count,
               tokens_in, tokens_out, token_delta, cost_usd, payload_json, created_at
        FROM aop_agent_events
        WHERE created_at < ?
        ORDER BY id ASC
        "#,
    )
    .bind(cutoff)
    .fetch_all(pool)
    .await
    .map_err(|error| format!("Failed to query old agent events: {error}"))?;

    let runs = sqlx::query_as::<_, AgentRunRecord>(
        r#"
        SELECT id, root_task_id, task_id, tier, actor, persona, skill, provider, model_id,
               adapter_kind, status, started_at, ended_at, tokens_in, tokens_out, token_delta,
               cost_usd, metadata_json
        FROM aop_agent_runs
        WHERE ended_at IS NOT NULL AND ended_at < ?
        ORDER BY ended_at ASC
        "#,
    )
    .bind(cutoff)
    .fetch_all(pool)
    .await
    .map_err(|error| format!("Failed to query old agent runs: {error}"))?;

    if events.is_empty() && runs.is_empty() {
        return Ok(ArchiveTelemetryResult {
            retention_days: effective_days,
            events_archived: 0,
            runs_archived: 0,
            archive_file: None,
        });
    }

    fs::create_dir_all(archive_root)
        .map_err(|error| format!("Failed to create telemetry archive dir: {error}"))?;
    let filename = format!(
        "telemetry_{}.jsonl",
        Utc::now().format("%Y%m%d_%H%M%S")
    );
    let archive_file = archive_root.join(filename);
    let file = File::create(&archive_file)
        .map_err(|error| format!("Failed to create archive file: {error}"))?;
    let mut writer = BufWriter::new(file);

    for event in &events {
        let line = json!({
            "type": "agent_event",
            "data": event,
        })
        .to_string();
        writer
            .write_all(line.as_bytes())
            .map_err(|error| format!("Failed to write event archive: {error}"))?;
        writer
            .write_all(b"\n")
            .map_err(|error| format!("Failed to write event archive newline: {error}"))?;
    }
    for run in &runs {
        let line = json!({
            "type": "agent_run",
            "data": run,
        })
        .to_string();
        writer
            .write_all(line.as_bytes())
            .map_err(|error| format!("Failed to write run archive: {error}"))?;
        writer
            .write_all(b"\n")
            .map_err(|error| format!("Failed to write run archive newline: {error}"))?;
    }

    writer
        .flush()
        .map_err(|error| format!("Failed to flush telemetry archive: {error}"))?;

    sqlx::query("DELETE FROM aop_agent_events WHERE created_at < ?")
        .bind(cutoff)
        .execute(pool)
        .await
        .map_err(|error| format!("Failed to delete old agent events: {error}"))?;
    sqlx::query("DELETE FROM aop_agent_runs WHERE ended_at IS NOT NULL AND ended_at < ?")
        .bind(cutoff)
        .execute(pool)
        .await
        .map_err(|error| format!("Failed to delete old agent runs: {error}"))?;

    Ok(ArchiveTelemetryResult {
        retention_days: effective_days,
        events_archived: events.len(),
        runs_archived: runs.len(),
        archive_file: Some(archive_file.to_string_lossy().to_string()),
    })
}

pub fn spawn_retention_worker(pool: SqlitePool, archive_root: PathBuf, retention_days: u32) {
    let effective_days = retention_days.clamp(1, 365);
    tauri::async_runtime::spawn(async move {
        loop {
            let _ = archive_telemetry(&pool, &archive_root, effective_days).await;
            sleep(Duration::from_secs(1_800)).await;
        }
    });
}

async fn upsert_run(pool: &SqlitePool, next: &AgentRunRecord) -> Result<(), String> {
    let existing = sqlx::query_as::<_, AgentRunRecord>(
        r#"
        SELECT id, root_task_id, task_id, tier, actor, persona, skill, provider, model_id,
               adapter_kind, status, started_at, ended_at, tokens_in, tokens_out, token_delta,
               cost_usd, metadata_json
        FROM aop_agent_runs
        WHERE id = ?
        "#,
    )
    .bind(next.id.as_str())
    .fetch_optional(pool)
    .await
    .map_err(|error| format!("Failed to query agent run: {error}"))?;

    if let Some(current) = existing {
        let merged = AgentRunRecord {
            id: current.id,
            root_task_id: next.root_task_id.clone().or(current.root_task_id),
            task_id: next.task_id.clone().or(current.task_id),
            tier: next.tier.or(current.tier),
            actor: next.actor.clone(),
            persona: next.persona.clone().or(current.persona),
            skill: next.skill.clone().or(current.skill),
            provider: next.provider.clone().or(current.provider),
            model_id: next.model_id.clone().or(current.model_id),
            adapter_kind: next.adapter_kind.clone().or(current.adapter_kind),
            status: next.status.clone(),
            started_at: current.started_at,
            ended_at: next.ended_at.or(current.ended_at),
            tokens_in: if next.tokens_in == 0 {
                current.tokens_in
            } else {
                next.tokens_in
            },
            tokens_out: if next.tokens_out == 0 {
                current.tokens_out
            } else {
                next.tokens_out
            },
            token_delta: if next.token_delta == 0 {
                current.token_delta
            } else {
                next.token_delta
            },
            cost_usd: next.cost_usd.or(current.cost_usd),
            metadata_json: next.metadata_json.clone().or(current.metadata_json),
        };

        sqlx::query(
            r#"
            UPDATE aop_agent_runs
            SET root_task_id = ?, task_id = ?, tier = ?, actor = ?, persona = ?, skill = ?,
                provider = ?, model_id = ?, adapter_kind = ?, status = ?, ended_at = ?,
                tokens_in = ?, tokens_out = ?, token_delta = ?, cost_usd = ?, metadata_json = ?
            WHERE id = ?
            "#,
        )
        .bind(merged.root_task_id)
        .bind(merged.task_id)
        .bind(merged.tier)
        .bind(merged.actor)
        .bind(merged.persona)
        .bind(merged.skill)
        .bind(merged.provider)
        .bind(merged.model_id)
        .bind(merged.adapter_kind)
        .bind(merged.status)
        .bind(merged.ended_at)
        .bind(merged.tokens_in)
        .bind(merged.tokens_out)
        .bind(merged.token_delta)
        .bind(merged.cost_usd)
        .bind(merged.metadata_json)
        .bind(next.id.as_str())
        .execute(pool)
        .await
        .map_err(|error| format!("Failed to update agent run: {error}"))?;
    } else {
        sqlx::query(
            r#"
            INSERT INTO aop_agent_runs (
                id, root_task_id, task_id, tier, actor, persona, skill, provider, model_id,
                adapter_kind, status, started_at, ended_at, tokens_in, tokens_out, token_delta,
                cost_usd, metadata_json
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(next.id.as_str())
        .bind(next.root_task_id.clone())
        .bind(next.task_id.clone())
        .bind(next.tier)
        .bind(next.actor.trim())
        .bind(next.persona.clone())
        .bind(next.skill.clone())
        .bind(next.provider.clone())
        .bind(next.model_id.clone())
        .bind(next.adapter_kind.clone())
        .bind(next.status.trim())
        .bind(next.started_at)
        .bind(next.ended_at)
        .bind(next.tokens_in)
        .bind(next.tokens_out)
        .bind(next.token_delta)
        .bind(next.cost_usd)
        .bind(next.metadata_json.clone())
        .execute(pool)
        .await
        .map_err(|error| format!("Failed to insert agent run: {error}"))?;
    }

    Ok(())
}

async fn infer_task_scope(
    pool: &SqlitePool,
    task_id: &str,
) -> Result<(Option<String>, Option<i64>), String> {
    let mut current_id = task_id.trim().to_string();
    if current_id.is_empty() {
        return Ok((None, None));
    }

    let mut root_id = Some(current_id.clone());
    let mut tier = None;
    let mut safety = 0_u8;

    while safety < 48 {
        let row = sqlx::query_as::<_, (Option<String>, i64)>(
            "SELECT parent_id, tier FROM aop_tasks WHERE id = ?",
        )
        .bind(current_id.as_str())
        .fetch_optional(pool)
        .await
        .map_err(|error| format!("Failed to infer task scope: {error}"))?;

        let Some((parent_id, task_tier)) = row else {
            break;
        };
        if tier.is_none() {
            tier = Some(task_tier);
        }
        root_id = Some(current_id.clone());
        if let Some(parent) = parent_id.filter(|value| !value.trim().is_empty()) {
            current_id = parent;
            safety = safety.saturating_add(1);
            continue;
        }
        break;
    }

    Ok((root_id, tier))
}

fn parse_task_id_from_message(message: Option<&str>) -> Option<String> {
    let Some(message) = message else {
        return None;
    };
    for token in message.split_whitespace() {
        if let Some(value) = token.strip_prefix("task:") {
            let trimmed = value.trim().trim_matches(|ch| ch == '"' || ch == '\'');
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

fn parse_details(details: Option<&str>) -> ParsedTelemetryFields {
    let mut parsed = ParsedTelemetryFields::default();
    let Some(details) = details.map(str::trim).filter(|value| !value.is_empty()) else {
        return parsed;
    };

    if let Ok(value) = serde_json::from_str::<Value>(details) {
        parsed.payload = Some(value.clone());
        if let Some(status) = value.get("status").and_then(Value::as_str) {
            parsed.status = Some(status.to_string());
        }
        if let Some(phase) = value.get("phase").and_then(Value::as_str) {
            parsed.phase = Some(phase.to_string());
        }
        if let Some(message) = value.get("message").and_then(Value::as_str) {
            parsed.message = Some(message.to_string());
        }
        if let Some(provider) = value.get("provider").and_then(Value::as_str) {
            parsed.provider = Some(provider.to_string());
        }
        if let Some(model_id) = value
            .get("modelId")
            .and_then(Value::as_str)
            .or_else(|| value.get("model_id").and_then(Value::as_str))
        {
            parsed.model_id = Some(model_id.to_string());
        }
    }

    for raw in details.split(|ch: char| ch.is_whitespace() || ch == ';' || ch == ',') {
        let token = raw.trim();
        if token.is_empty() || !token.contains('=') {
            continue;
        }
        let mut split = token.splitn(2, '=');
        let key = split.next().unwrap_or_default().trim().to_ascii_lowercase();
        let value = split
            .next()
            .unwrap_or_default()
            .trim()
            .trim_matches('"')
            .trim_matches('\'')
            .trim_matches('[')
            .trim_matches(']');
        if value.is_empty() {
            continue;
        }

        match key.as_str() {
            "status" => parsed.status = Some(value.to_string()),
            "phase" => parsed.phase = Some(value.to_string()),
            "message" | "objective" => parsed.message = Some(value.to_string()),
            "persona" => parsed.persona = Some(value.to_string()),
            "skill" => parsed.skill = Some(value.to_string()),
            "provider" => parsed.provider = Some(value.to_string()),
            "modelid" | "model_id" => parsed.model_id = Some(value.to_string()),
            "model" => {
                if let Some((provider, model_id)) = value.split_once('/') {
                    parsed.provider = Some(provider.to_string());
                    parsed.model_id = Some(model_id.to_string());
                } else {
                    parsed.model_id = Some(value.to_string());
                }
            }
            "mcp" | "mcpserver" | "mcp_server" => parsed.mcp_server = Some(value.to_string()),
            "mcptool" | "mcp_tool" | "tool" => parsed.mcp_tool = Some(value.to_string()),
            "latency" | "latencyms" | "latency_ms" => parsed.latency_ms = value.parse().ok(),
            "retry" | "retries" | "retrycount" | "retry_count" => {
                parsed.retry_count = value.parse().ok()
            }
            "tokensin" | "tokens_in" => parsed.tokens_in = value.parse().ok(),
            "tokensout" | "tokens_out" => parsed.tokens_out = value.parse().ok(),
            "tokendelta" | "token_delta" | "tokensused" | "tokens_used" => {
                parsed.token_delta = value.parse().ok()
            }
            "cost" | "costusd" | "cost_usd" => parsed.cost_usd = value.parse().ok(),
            _ => {}
        }
    }

    if parsed.persona.is_none() && details.contains("tier3_") {
        if let Some(fragment) = details.split("tier3_").nth(1) {
            let persona = fragment
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .trim_matches(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_');
            if !persona.is_empty() {
                parsed.persona = Some(persona.to_string());
            }
        }
    }

    parsed
}

fn status_from_action(action: &str) -> Option<String> {
    let normalized = action.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return None;
    }

    if normalized.contains("started") || normalized.contains("executing") {
        return Some("executing".to_string());
    }
    if normalized.contains("pause") {
        return Some("paused".to_string());
    }
    if normalized.contains("failed")
        || normalized.contains("error")
        || normalized.contains("stopped")
    {
        return Some("failed".to_string());
    }
    if normalized.contains("completed")
        || normalized.contains("applied")
        || normalized.contains("finished")
    {
        return Some("completed".to_string());
    }

    None
}

fn is_terminal_status(status: &str) -> bool {
    matches!(status, "completed" | "failed")
}

fn ewma(previous: f64, next: f64, alpha: f64) -> f64 {
    if previous == 0.0 {
        return next.max(0.0);
    }
    ((alpha * next) + ((1.0 - alpha) * previous)).max(0.0)
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
    async fn records_structured_agent_event_and_run() {
        let pool = setup_test_pool().await;
        let task = tasks::create_task(
            &pool,
            CreateTaskInput {
                parent_id: None,
                tier: 1,
                domain: "platform".to_string(),
                objective: "mission control".to_string(),
                token_budget: 5000,
            },
        )
        .await
        .expect("task should be created");

        record_task_activity_event(
            &pool,
            "tier1_orchestrator",
            "orchestration_started",
            Some(task.id.as_str()),
            Some("model=claude_code/sonnet persona=planner tokensUsed=250"),
        )
        .await
        .expect("telemetry should be recorded");

        let runs = list_agent_runs(
            &pool,
            ListAgentRunsInput {
                root_task_id: None,
                task_id: Some(task.id.clone()),
                actor: None,
                tier: None,
                status: None,
                limit: Some(20),
            },
        )
        .await
        .expect("runs should list");
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].provider.as_deref(), Some("claude_code"));

        let events = list_agent_events(
            &pool,
            ListAgentEventsInput {
                root_task_id: None,
                task_id: Some(task.id),
                actor: None,
                action: None,
                since_id: None,
                limit: Some(20),
            },
        )
        .await
        .expect("events should list");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].model_id.as_deref(), Some("sonnet"));
    }

    #[tokio::test]
    async fn updates_model_health_scores() {
        let pool = setup_test_pool().await;

        let success = update_model_health(
            &pool,
            ModelCallOutcomeInput {
                provider: "claude_code".to_string(),
                model_id: "sonnet".to_string(),
                success: true,
                latency_ms: Some(420),
                cost_usd: Some(0.02),
                error: None,
            },
        )
        .await
        .expect("model health should update");
        assert_eq!(success.total_calls, 1);
        assert_eq!(success.success_calls, 1);

        let failure = update_model_health(
            &pool,
            ModelCallOutcomeInput {
                provider: "claude_code".to_string(),
                model_id: "sonnet".to_string(),
                success: false,
                latency_ms: Some(900),
                cost_usd: Some(0.03),
                error: Some("timeout".to_string()),
            },
        )
        .await
        .expect("model health should update");
        assert_eq!(failure.total_calls, 2);
        assert_eq!(failure.failed_calls, 1);
    }
}
