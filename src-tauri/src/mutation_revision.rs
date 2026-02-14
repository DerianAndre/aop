use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::agents::specialist::{self, SpecialistTask};
use crate::agents::CodeBlock;
use crate::db::metrics;
use crate::db::mutations::{
    self, CreateMutationInput, MutationRecord, MutationStatus, UpdateMutationStatusInput,
};
use crate::db::tasks::{self, CreateTaskRecordInput, TaskRecord, TaskStatus};
use crate::llm_adapter;
use crate::model_registry::ModelRegistry;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestMutationRevisionInput {
    pub mutation_id: String,
    pub note: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MutationRevisionResult {
    pub original_mutation: MutationRecord,
    pub revised_task: TaskRecord,
    pub revised_mutation: MutationRecord,
}

pub async fn request_mutation_revision(
    pool: &SqlitePool,
    model_registry: &ModelRegistry,
    input: RequestMutationRevisionInput,
) -> Result<MutationRevisionResult, String> {
    validate_input(&input)?;

    let base_mutation = mutations::get_mutation_by_id(pool, input.mutation_id.trim()).await?;
    if base_mutation.status == MutationStatus::Applied.as_str() {
        return Err(
            "Cannot request revision for an already applied mutation. Propose a new mutation instead."
                .to_string(),
        );
    }

    let parent_task = tasks::get_task_by_id(pool, base_mutation.task_id.trim()).await?;
    let revision_note = normalized_note(&input.note);
    let revision_budget = revision_budget(parent_task.token_budget);
    let revision_objective = format!(
        "Revision requested for mutation {} on {}. Note: {}",
        base_mutation.id, base_mutation.file_path, revision_note
    );

    let revised_task = tasks::create_task_record(
        pool,
        CreateTaskRecordInput {
            parent_id: Some(parent_task.id.clone()),
            tier: 3,
            domain: parent_task.domain.clone(),
            objective: revision_objective.clone(),
            token_budget: revision_budget,
            risk_factor: parent_task.risk_factor,
            status: TaskStatus::Pending,
            target_files: Some(
                serde_json::to_string(&vec![base_mutation.file_path.clone()])
                    .unwrap_or_default(),
            ),
        },
    )
    .await?;

    let revision_model = model_registry.resolve_with_supported_providers(
        3,
        Some("revision_specialist"),
        &llm_adapter::supported_provider_aliases(),
    )?;
    let specialist_task = SpecialistTask {
        task_id: revised_task.id.clone(),
        parent_id: parent_task.id.clone(),
        tier: 3,
        persona: "revision_specialist".to_string(),
        objective: revision_objective.clone(),
        token_budget: revision_budget.max(1) as u32,
        target_files: vec![base_mutation.file_path.clone()],
        code_context: vec![CodeBlock {
            file_path: base_mutation.file_path.clone(),
            start_line: 1,
            end_line: 1,
            content: base_mutation.diff_content.chars().take(1200).collect(),
            embedding: None,
        }],
        constraints: vec![
            "apply reviewer-requested revision".to_string(),
            format!("reviewer_note: {}", revision_note),
            format!(
                "revision_model: {}/{}",
                revision_model.provider.as_str(),
                revision_model.model_id.as_str()
            ),
        ],
        model_provider: Some(revision_model.provider.clone()),
        model_id: Some(revision_model.model_id.clone()),
    };
    let proposal = specialist::run_specialist_task(&specialist_task, None)
        .map_err(|error| format!("Failed to generate revised specialist proposal: {error}"))?;

    let revised_mutation = mutations::create_mutation(
        pool,
        CreateMutationInput {
            task_id: revised_task.id.clone(),
            agent_uid: proposal.agent_uid,
            file_path: base_mutation.file_path.clone(),
            diff_content: proposal.diff_content,
            intent_description: Some(proposal.intent_description),
            intent_hash: Some(proposal.intent_hash),
            confidence: (proposal.confidence as f64).clamp(0.10, 1.0),
        },
    )
    .await?;

    let original_mutation = mutations::update_mutation_status(
        pool,
        UpdateMutationStatusInput {
            mutation_id: base_mutation.id.clone(),
            status: MutationStatus::Rejected,
            test_result: None,
            test_exit_code: None,
            rejection_reason: Some(format!("Revision requested: {}", revision_note)),
            rejected_at_step: Some("diff_reviewer_revision_requested".to_string()),
        },
    )
    .await?;

    metrics::record_audit_event(
        pool,
        "ui",
        "mutation_revision_requested",
        Some(original_mutation.id.as_str()),
        Some(&format!(
            "{{\"revisedTaskId\":\"{}\",\"revisedMutationId\":\"{}\"}}",
            revised_task.id, revised_mutation.id
        )),
    )
    .await?;

    Ok(MutationRevisionResult {
        original_mutation,
        revised_task,
        revised_mutation,
    })
}

fn validate_input(input: &RequestMutationRevisionInput) -> Result<(), String> {
    if input.mutation_id.trim().is_empty() {
        return Err("mutationId is required".to_string());
    }
    if input.note.trim().is_empty() {
        return Err("note is required".to_string());
    }

    Ok(())
}

fn normalized_note(note: &str) -> String {
    note.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn revision_budget(parent_budget: i64) -> i64 {
    let proportional = ((parent_budget.max(1) as f32) * 0.35).round() as i64;
    proportional.clamp(250, 2_000)
}

#[cfg(test)]
mod tests {
    use sqlx::sqlite::SqlitePoolOptions;
    use uuid::Uuid;

    use crate::db;
    use crate::db::mutations::{self, CreateMutationInput};
    use crate::db::tasks::{self, CreateTaskInput};
    use crate::model_registry::ModelRegistry;

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
    async fn creates_revision_task_and_mutation_and_rejects_original() {
        let pool = setup_test_pool().await;
        let task = tasks::create_task(
            &pool,
            CreateTaskInput {
                parent_id: None,
                tier: 2,
                domain: "frontend".to_string(),
                objective: "Review session diff".to_string(),
                token_budget: 3400,
            },
        )
        .await
        .expect("task should be created");

        let mutation = mutations::create_mutation(
            &pool,
            CreateMutationInput {
                task_id: task.id.clone(),
                agent_uid: Uuid::new_v4().to_string(),
                file_path: "src/session.ts".to_string(),
                diff_content: "--- a/src/session.ts\n+++ b/src/session.ts\n".to_string(),
                intent_description: Some("Add session loading checks".to_string()),
                intent_hash: Some("abc123".to_string()),
                confidence: 0.74,
            },
        )
        .await
        .expect("mutation should be created");

        let model_registry = ModelRegistry::default();
        let result = request_mutation_revision(
            &pool,
            &model_registry,
            RequestMutationRevisionInput {
                mutation_id: mutation.id.clone(),
                note: "Please isolate loading guard and keep previous behavior.".to_string(),
            },
        )
        .await
        .expect("revision request should succeed");

        assert_eq!(result.original_mutation.status, "rejected");
        assert_eq!(
            result.revised_task.parent_id.as_deref(),
            Some(task.id.as_str())
        );
        assert_eq!(result.revised_task.tier, 3);
        assert_eq!(result.revised_mutation.task_id, result.revised_task.id);
        assert_eq!(result.revised_mutation.status, "proposed");
        assert!(result.revised_mutation.diff_content.contains("AOP("));
        assert_ne!(result.revised_mutation.diff_content, mutation.diff_content);
    }
}
