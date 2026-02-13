use crate::db::telemetry::{self, ModelHealthRecord, NewAgentEvent};
use crate::llm_adapter;
use crate::model_registry::{ModelProfile, ModelRegistry, ModelSelection};
use sqlx::SqlitePool;

#[derive(Debug, Clone)]
pub struct ModelSelectionRequest<'a> {
    pub task_id: Option<&'a str>,
    pub actor: &'a str,
    pub tier: u8,
    pub persona: Option<&'a str>,
    pub skill: Option<&'a str>,
}

#[derive(Debug, Clone)]
pub struct ModelSelectionResult {
    pub selection: ModelSelection,
    pub score: f64,
    pub fallback_used: bool,
}

#[derive(Debug, Clone)]
struct ScoredCandidate {
    profile: ModelProfile,
    score: f64,
}

pub async fn select_model(
    pool: &SqlitePool,
    registry: &ModelRegistry,
    request: ModelSelectionRequest<'_>,
) -> Result<ModelSelectionResult, String> {
    let supported = llm_adapter::supported_provider_aliases();
    let candidates = registry.candidates_with_supported_providers(
        request.tier,
        request.persona,
        &supported,
    )?;
    if candidates.is_empty() {
        return Err(format!(
            "No model candidates available for tier {} persona {:?}",
            request.tier, request.persona
        ));
    }
    let first_candidate_key = candidates
        .first()
        .map(|profile| {
            format!(
                "{}::{}",
                profile.provider.to_ascii_lowercase(),
                profile.model_id.to_ascii_lowercase()
            )
        })
        .unwrap_or_default();

    let mut scored = Vec::new();
    for candidate in candidates {
        if !llm_adapter::supports_provider(candidate.provider.as_str()) {
            continue;
        }
        let health = telemetry::get_model_health(
            pool,
            candidate.provider.as_str(),
            candidate.model_id.as_str(),
        )
        .await?;
        let score = score_candidate(health.as_ref());
        scored.push(ScoredCandidate {
            profile: candidate,
            score,
        });
    }
    if scored.is_empty() {
        return Err(format!(
            "No provider adapters are available for tier {} persona {:?}",
            request.tier, request.persona
        ));
    }

    scored.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                left.profile
                    .provider
                    .cmp(&right.profile.provider)
                    .then_with(|| left.profile.model_id.cmp(&right.profile.model_id))
            })
    });

    let selected = scored
        .first()
        .cloned()
        .ok_or_else(|| "Model selection produced no candidates".to_string())?;
    let selected_key = format!(
        "{}::{}",
        selected.profile.provider.to_ascii_lowercase(),
        selected.profile.model_id.to_ascii_lowercase()
    );
    let fallback_used = !first_candidate_key.is_empty() && selected_key != first_candidate_key;
    let action = if fallback_used {
        "model_fallback"
    } else {
        "model_selected"
    };
    let payload = serde_json::json!({
        "tier": request.tier,
        "persona": request.persona,
        "provider": selected.profile.provider,
        "modelId": selected.profile.model_id,
        "score": selected.score,
        "candidates": scored.len(),
        "source": "quality_first_dynamic"
    });
    let _ = telemetry::record_agent_event(
        pool,
        NewAgentEvent {
            task_id: request.task_id.map(ToOwned::to_owned),
            actor: request.actor.to_string(),
            action: action.to_string(),
            status: Some("executing".to_string()),
            phase: Some("model_routing".to_string()),
            provider: Some(selected.profile.provider.clone()),
            model_id: Some(selected.profile.model_id.clone()),
            persona: request.persona.map(ToOwned::to_owned),
            skill: request.skill.map(ToOwned::to_owned),
            payload: Some(payload),
            ..Default::default()
        },
    )
    .await;

    Ok(ModelSelectionResult {
        selection: ModelSelection {
            tier: request.tier,
            persona: request.persona.map(|value| value.to_ascii_lowercase()),
            provider: selected.profile.provider,
            model_id: selected.profile.model_id,
            source: "scored".to_string(),
        },
        score: selected.score,
        fallback_used,
    })
}

pub async fn record_model_call_outcome(
    pool: &SqlitePool,
    provider: &str,
    model_id: &str,
    success: bool,
    latency_ms: Option<i64>,
    cost_usd: Option<f64>,
    error: Option<String>,
) {
    let _ = telemetry::update_model_health(
        pool,
        telemetry::ModelCallOutcomeInput {
            provider: provider.to_string(),
            model_id: model_id.to_string(),
            success,
            latency_ms,
            cost_usd,
            error,
        },
    )
    .await;
}

fn score_candidate(health: Option<&ModelHealthRecord>) -> f64 {
    let quality = health.map(|value| value.quality_score).unwrap_or(0.70);
    let success_rate = health
        .map(|value| {
            if value.total_calls <= 0 {
                0.90
            } else {
                (value.success_calls as f64 / value.total_calls as f64).clamp(0.0, 1.0)
            }
        })
        .unwrap_or(0.90);
    let latency_norm = health
        .map(|value| (value.avg_latency_ms / 4_000.0).clamp(0.0, 1.0))
        .unwrap_or(0.50);
    let cost_norm = health
        .map(|value| (value.avg_cost_usd / 0.25).clamp(0.0, 1.0))
        .unwrap_or(0.50);
    let failure_penalty = health
        .map(|value| {
            if value.total_calls <= 0 {
                0.0
            } else {
                (value.failed_calls as f64 / value.total_calls as f64).clamp(0.0, 1.0)
            }
        })
        .unwrap_or(0.0);

    let score = (0.55 * quality)
        + (0.20 * success_rate)
        + (0.15 * (1.0 - latency_norm))
        + (0.10 * (1.0 - cost_norm))
        - (0.20 * failure_penalty);
    score.clamp(0.0, 1.0)
}
