use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::agents::CodeBlock;
use crate::llm_adapter::{self, AdapterRequest};
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
    #[serde(default)]
    pub model_provider: Option<String>,
    #[serde(default)]
    pub model_id: Option<String>,
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

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SpecialistModelOutput {
    #[serde(default)]
    intent_description: Option<String>,
    #[serde(default)]
    note_line: Option<String>,
}

pub fn run_specialist_task(
    task: &SpecialistTask,
    target_file_content: Option<&str>,
) -> Result<DiffProposal, String> {
    validate_specialist_task(task)?;

    let agent_uid = Uuid::new_v4().to_string();
    let proposal_id = Uuid::new_v4().to_string();
    let file_path = resolve_target_file(task);
    let model_tag = model_tag(task);
    let fallback_intent_description = format!(
        "{}{} proposal for {}: {}",
        task.persona,
        model_tag,
        file_path,
        task.objective.trim()
    );
    let fallback_note_line = build_note_line(
        &task.persona,
        task.model_id.as_deref(),
        &task.objective,
        &file_path,
    );

    let model_generation = try_remote_model_generation(
        task,
        &file_path,
        target_file_content,
        &fallback_intent_description,
        &fallback_note_line,
    )?;
    let (intent_description, note_line, remote_output_tokens) = match model_generation {
        Some((intent, note, output_tokens)) => (intent, note, output_tokens),
        None => (fallback_intent_description, fallback_note_line, None),
    };

    let intent_hash = hash_intent_embedding(&intent_description);
    let confidence = estimate_confidence(task, target_file_content);
    let baseline_tokens = estimate_tokens_used(task, target_file_content);
    let tokens_used = remote_output_tokens
        .map(|output_tokens| {
            baseline_tokens
                .saturating_add(output_tokens)
                .min(task.token_budget)
                .max(40)
        })
        .unwrap_or(baseline_tokens);
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
    if task
        .model_provider
        .as_ref()
        .is_some_and(|value| value.trim().is_empty())
    {
        return Err("modelProvider must not be empty when provided".to_string());
    }
    if task
        .model_id
        .as_ref()
        .is_some_and(|value| value.trim().is_empty())
    {
        return Err("modelId must not be empty when provided".to_string());
    }
    if task.model_provider.is_some() ^ task.model_id.is_some() {
        return Err("modelProvider and modelId must be provided together".to_string());
    }

    Ok(())
}

fn model_tag(task: &SpecialistTask) -> String {
    match (task.model_provider.as_deref(), task.model_id.as_deref()) {
        (Some(provider), Some(model_id)) => format!(" [{}:{}]", provider.trim(), model_id.trim()),
        _ => String::new(),
    }
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

fn try_remote_model_generation(
    task: &SpecialistTask,
    file_path: &str,
    target_file_content: Option<&str>,
    fallback_intent_description: &str,
    fallback_note_line: &str,
) -> Result<Option<(String, String, Option<u32>)>, String> {
    if !remote_model_adapter_enabled() {
        return Ok(None);
    }

    let (provider, model_id) = match (task.model_provider.as_deref(), task.model_id.as_deref()) {
        (Some(provider), Some(model_id)) => (provider.trim(), model_id.trim()),
        _ => return Ok(None),
    };

    let (system_prompt, user_prompt) = build_remote_prompts(task, file_path, target_file_content);
    let request = AdapterRequest {
        provider: provider.to_string(),
        model_id: model_id.to_string(),
        system_prompt,
        user_prompt,
    };

    match llm_adapter::generate(&request) {
        Ok(response) => {
            let parsed_output = parse_specialist_model_output(&response.text);
            let intent_description = parsed_output
                .as_ref()
                .and_then(|payload| payload.intent_description.clone())
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| fallback_intent_description.to_string());

            let model_note_candidate = parsed_output
                .as_ref()
                .and_then(|payload| payload.note_line.clone())
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| response.text.clone());

            let note_line = sanitize_llm_note_line(&model_note_candidate, file_path)
                .unwrap_or_else(|| fallback_note_line.to_string());

            let output_tokens = response.output_tokens.or_else(|| {
                response
                    .input_tokens
                    .map(|input_tokens| (input_tokens / 6).max(1))
            });

            Ok(Some((intent_description, note_line, output_tokens)))
        }
        Err(error) => {
            if strict_model_adapter_enabled() {
                return Err(format!("Model adapter execution failed: {error}"));
            }
            Ok(None)
        }
    }
}

fn strict_model_adapter_enabled() -> bool {
    std::env::var("AOP_MODEL_ADAPTER_STRICT")
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

fn remote_model_adapter_enabled() -> bool {
    let default_enabled = !cfg!(test);
    std::env::var("AOP_MODEL_ADAPTER_ENABLED")
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(default_enabled)
}

fn build_remote_prompts(
    task: &SpecialistTask,
    file_path: &str,
    target_file_content: Option<&str>,
) -> (String, String) {
    let system_prompt =
        r#"You are a Tier-3 software specialist for Autonomous Orchestration Platform (AOP).
Respond with JSON only:
{
  "intentDescription": "short sentence",
  "noteLine": "single-line code comment to insert at top of file"
}
Rules:
- Do not include markdown fences.
- noteLine must be one line, <= 180 chars.
- Focus on safe, minimal, behavior-preserving intent."#
            .to_string();

    let context_excerpt = task
        .code_context
        .iter()
        .take(2)
        .map(|block| {
            format!(
                "[{}:{}-{}]\n{}",
                block.file_path, block.start_line, block.end_line, block.content
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    let file_excerpt = target_file_content
        .map(|content| content.chars().take(1_400).collect::<String>())
        .unwrap_or_else(|| "<unavailable>".to_string());

    let user_prompt = format!(
        "persona: {}\nobjective: {}\nfilePath: {}\nconstraints: {}\n\nfileExcerpt:\n{}\n\ncodeContext:\n{}\n",
        task.persona.trim(),
        task.objective.trim(),
        file_path,
        task.constraints.join(" | "),
        file_excerpt,
        context_excerpt
    );

    (system_prompt, user_prompt)
}

fn parse_specialist_model_output(raw: &str) -> Option<SpecialistModelOutput> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Ok(parsed) = serde_json::from_str::<SpecialistModelOutput>(trimmed) {
        return Some(parsed);
    }
    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        if let Some(object) = value.as_object() {
            let payload = SpecialistModelOutput {
                intent_description: object
                    .get("intentDescription")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                note_line: object
                    .get("noteLine")
                    .and_then(Value::as_str)
                    .map(str::to_string),
            };
            return Some(payload);
        }
    }
    None
}

fn sanitize_llm_note_line(raw: &str, file_path: &str) -> Option<String> {
    let single_line = raw
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(180)
        .collect::<String>();

    if single_line.is_empty() {
        return None;
    }

    let comment_body = extract_comment_body(&single_line).to_string();
    let normalized_body = if comment_body.contains("AOP(") {
        comment_body
    } else {
        format!("AOP(llm): {comment_body}")
    };

    let wrapped = match file_path.rsplit('.').next() {
        Some("py") => format!("# {normalized_body}"),
        Some("md") => format!("<!-- {normalized_body} -->"),
        _ => format!("// {normalized_body}"),
    };
    Some(wrapped)
}

fn extract_comment_body(single_line: &str) -> &str {
    let trimmed = single_line.trim();
    if let Some(value) = trimmed.strip_prefix("// ") {
        return value.trim();
    }
    if let Some(value) = trimmed.strip_prefix("# ") {
        return value.trim();
    }
    if let Some(value) = trimmed.strip_prefix("<!-- ") {
        if let Some(value) = value.strip_suffix(" -->") {
            return value.trim();
        }
    }
    trimmed
}

fn build_note_line(
    persona: &str,
    model_id: Option<&str>,
    objective: &str,
    file_path: &str,
) -> String {
    let compact_objective = objective
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(120)
        .collect::<String>();

    let model_fragment = model_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| format!(" model={value}"))
        .unwrap_or_default();
    let note = format!("AOP({persona}{model_fragment}): {compact_objective} [{file_path}]");
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
    use std::sync::Mutex;

    use super::*;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

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
            model_provider: Some("openai".to_string()),
            model_id: Some("gpt-5-nano".to_string()),
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

    #[test]
    fn strict_model_adapter_flag_is_respected() {
        let _guard = ENV_LOCK.lock().expect("env lock should be acquired");
        std::env::set_var("AOP_MODEL_ADAPTER_STRICT", "true");
        assert!(strict_model_adapter_enabled());
        std::env::remove_var("AOP_MODEL_ADAPTER_STRICT");
        assert!(!strict_model_adapter_enabled());
    }

    #[test]
    fn sanitize_llm_note_line_wraps_for_language() {
        let ts = sanitize_llm_note_line("short note", "src/file.ts")
            .expect("typescript line should sanitize");
        let py = sanitize_llm_note_line("short note", "src/file.py")
            .expect("python line should sanitize");

        assert!(ts.starts_with("// "));
        assert!(py.starts_with("# "));
    }
}
