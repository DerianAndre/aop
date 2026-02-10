use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::agents::CodeBlock;
use crate::vector::indexer::embed_text;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpecialistTask {
    pub task_id: String,
    pub parent_id: String,
    pub tier: u8,
    pub persona: String,
    pub objective: String,
    pub token_budget: u32,
    pub target_files: Vec<String>,
    pub code_context: Vec<CodeBlock>,
    pub constraints: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffProposal {
    pub proposal_id: String,
    pub task_id: String,
    pub agent_uid: String,
    pub file_path: String,
    pub diff_content: String,
    pub intent_description: String,
    pub intent_hash: String,
    pub confidence: f32,
    pub tokens_used: u32,
}

pub fn run_specialist_task(
    task: &SpecialistTask,
    target_file_content: Option<&str>,
) -> Result<DiffProposal, String> {
    validate_specialist_task(task)?;

    let agent_uid = Uuid::new_v4().to_string();
    let proposal_id = Uuid::new_v4().to_string();
    let file_path = resolve_target_file(task);
    let intent_description = format!(
        "{} proposal for {}: {}",
        task.persona,
        file_path,
        task.objective.trim()
    );
    let intent_hash = hash_intent_embedding(&intent_description);
    let confidence = estimate_confidence(task, target_file_content);
    let tokens_used = estimate_tokens_used(task, target_file_content);
    let note_line = build_note_line(&task.persona, &task.objective, &file_path);
    let diff_content = build_unified_diff(&file_path, &note_line, target_file_content);

    Ok(DiffProposal {
        proposal_id,
        task_id: task.task_id.clone(),
        agent_uid,
        file_path,
        diff_content,
        intent_description,
        intent_hash,
        confidence,
        tokens_used,
    })
}

pub fn semantic_distance(a: &DiffProposal, b: &DiffProposal) -> f32 {
    let vector_a = embed_text(&a.intent_description);
    let vector_b = embed_text(&b.intent_description);
    (1.0 - cosine_similarity(&vector_a, &vector_b)).clamp(0.0, 1.0)
}

fn validate_specialist_task(task: &SpecialistTask) -> Result<(), String> {
    if task.task_id.trim().is_empty() {
        return Err("taskId is required".to_string());
    }
    if task.parent_id.trim().is_empty() {
        return Err("parentId is required".to_string());
    }
    if task.tier != 3 {
        return Err("specialist task tier must be 3".to_string());
    }
    if task.persona.trim().is_empty() {
        return Err("persona is required".to_string());
    }
    if task.objective.trim().is_empty() {
        return Err("objective is required".to_string());
    }
    if task.token_budget == 0 {
        return Err("tokenBudget must be greater than 0".to_string());
    }

    Ok(())
}

fn resolve_target_file(task: &SpecialistTask) -> String {
    if let Some(path) = task.target_files.first() {
        return path.clone();
    }
    if let Some(block) = task.code_context.first() {
        return block.file_path.clone();
    }
    "unknown/file.ts".to_string()
}

fn hash_intent_embedding(intent_description: &str) -> String {
    let embedding = embed_text(intent_description);
    let serialized = embedding
        .iter()
        .map(|value| format!("{value:.6}"))
        .collect::<Vec<_>>()
        .join(",");

    let mut hasher = Sha256::new();
    hasher.update(serialized.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn estimate_confidence(task: &SpecialistTask, target_file_content: Option<&str>) -> f32 {
    let mut confidence = 0.58_f32;

    if !task.code_context.is_empty() {
        confidence += 0.14;
    }
    if !task.constraints.is_empty() {
        confidence += 0.09;
    }
    if target_file_content.is_some() {
        confidence += 0.11;
    }
    if task.persona.contains("test") {
        confidence += 0.04;
    }

    confidence.clamp(0.50, 0.95)
}

fn estimate_tokens_used(task: &SpecialistTask, target_file_content: Option<&str>) -> u32 {
    let objective_tokens = task.objective.split_whitespace().count().saturating_mul(16) as u32;
    let context_tokens = task
        .code_context
        .iter()
        .map(|block| block.content.len() / 4)
        .sum::<usize>() as u32;
    let file_tokens = target_file_content
        .map(|value| (value.len() / 6) as u32)
        .unwrap_or(60);

    (objective_tokens + context_tokens + file_tokens)
        .min(task.token_budget)
        .max(40)
}

fn build_note_line(persona: &str, objective: &str, file_path: &str) -> String {
    let compact_objective = objective
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(120)
        .collect::<String>();

    let note = format!("AOP({persona}): {compact_objective} [{file_path}]");
    match file_path.rsplit('.').next() {
        Some("py") => format!("# {note}"),
        Some("md") => format!("<!-- {note} -->"),
        _ => format!("// {note}"),
    }
}

fn build_unified_diff(
    file_path: &str,
    note_line: &str,
    target_file_content: Option<&str>,
) -> String {
    match target_file_content {
        Some(content) => {
            let first_line = content.lines().next().unwrap_or("");
            if first_line.is_empty() {
                format!("--- a/{file_path}\n+++ b/{file_path}\n@@ -0,0 +1,1 @@\n+{note_line}\n")
            } else {
                format!(
                    "--- a/{file_path}\n+++ b/{file_path}\n@@ -1,1 +1,2 @@\n+{note_line}\n {first_line}\n"
                )
            }
        }
        None => format!("--- a/{file_path}\n+++ b/{file_path}\n@@ -0,0 +1,1 @@\n+{note_line}\n"),
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.is_empty() || b.is_empty() || a.len() != b.len() {
        return 0.0;
    }

    let dot = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum::<f32>();
    let norm_a = a.iter().map(|value| value * value).sum::<f32>().sqrt();
    let norm_b = b.iter().map(|value| value * value).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    dot / (norm_a * norm_b)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_task() -> SpecialistTask {
        SpecialistTask {
            task_id: "task-1".to_string(),
            parent_id: "parent-1".to_string(),
            tier: 3,
            persona: "react_specialist".to_string(),
            objective: "Add loading state guard to SessionProvider".to_string(),
            token_budget: 1200,
            target_files: vec!["src/session.tsx".to_string()],
            code_context: vec![CodeBlock {
                file_path: "src/session.tsx".to_string(),
                start_line: 1,
                end_line: 8,
                content: "export function SessionProvider() { return null }".to_string(),
                embedding: None,
            }],
            constraints: vec!["avoid regressions in loading and error states".to_string()],
        }
    }

    #[test]
    fn specialist_generates_diff_proposal() {
        let proposal = run_specialist_task(
            &make_task(),
            Some("export function SessionProvider() { return null }"),
        )
        .expect("proposal should be generated");

        assert_eq!(proposal.task_id, "task-1");
        assert!(proposal.diff_content.contains("--- a/src/session.tsx"));
        assert!(proposal.confidence >= 0.5);
        assert!(proposal.tokens_used > 0);
    }

    #[test]
    fn semantic_distance_is_bounded() {
        let proposal_a = run_specialist_task(&make_task(), None).expect("proposal should generate");
        let mut alternate_task = make_task();
        alternate_task.objective =
            "Rewrite token refresh flow with stricter validation".to_string();
        let proposal_b =
            run_specialist_task(&alternate_task, None).expect("proposal should generate");

        let distance = semantic_distance(&proposal_a, &proposal_b);
        assert!((0.0..=1.0).contains(&distance));
    }
}
