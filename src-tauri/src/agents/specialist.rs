use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use similar::{ChangeTag, TextDiff};
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
    modified_content: Option<String>,
    #[serde(default)]
    changes_summary: Option<Vec<String>>,
}

struct RemoteGenerationResult {
    intent_description: String,
    diff_content: String,
    confidence: f32,
    output_tokens: Option<u32>,
}

pub fn run_specialist_task(
    task: &SpecialistTask,
    target_file_content: Option<&str>,
) -> Result<DiffProposal, String> {
    validate_specialist_task(task)?;

    let agent_uid = Uuid::new_v4().to_string();
    let proposal_id = Uuid::new_v4().to_string();
    let file_path = resolve_target_file(task);

    let remote_result = try_remote_model_generation(task, &file_path, target_file_content)?;

    let (intent_description, diff_content, confidence, tokens_used) = match remote_result {
        Some(result) => {
            let baseline_tokens = estimate_tokens_used(task, target_file_content);
            let tokens = result
                .output_tokens
                .map(|ot| {
                    baseline_tokens
                        .saturating_add(ot)
                        .min(task.token_budget)
                        .max(40)
                })
                .unwrap_or(baseline_tokens);
            (
                result.intent_description,
                result.diff_content,
                result.confidence,
                tokens,
            )
        }
        None => {
            let model_tag = model_tag(task);
            let intent = format!(
                "{}{} proposal for {}: {}",
                task.persona,
                model_tag,
                file_path,
                task.objective.trim()
            );
            let diff = build_fallback_diff(
                &file_path,
                target_file_content,
                &task.persona,
                &task.objective,
            );
            let confidence = estimate_fallback_confidence(task, target_file_content);
            let tokens = estimate_tokens_used(task, target_file_content);
            (intent, diff, confidence, tokens)
        }
    };

    let intent_hash = hash_intent_embedding(&intent_description);

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

fn estimate_fallback_confidence(task: &SpecialistTask, target_file_content: Option<&str>) -> f32 {
    let mut confidence = 0.42_f32;
    if !task.code_context.is_empty() {
        confidence += 0.08;
    }
    if !task.constraints.is_empty() {
        confidence += 0.05;
    }
    if target_file_content.is_some() {
        confidence += 0.06;
    }
    confidence.clamp(0.30, 0.65)
}

fn estimate_llm_confidence(task: &SpecialistTask, original: &str, modified: &str) -> f32 {
    let mut confidence = 0.62_f32;

    if !task.code_context.is_empty() {
        confidence += 0.08;
    }
    if !task.constraints.is_empty() {
        confidence += 0.05;
    }

    let original_lines = original.lines().count() as f32;
    if original_lines > 0.0 {
        let diff = TextDiff::from_lines(original, modified);
        let changed_lines = diff
            .iter_all_changes()
            .filter(|change| {
                matches!(change.tag(), ChangeTag::Delete | ChangeTag::Insert)
            })
            .count() as f32;
        let change_ratio = changed_lines / original_lines;
        if change_ratio < 0.15 {
            confidence += 0.12;
        } else if change_ratio < 0.35 {
            confidence += 0.06;
        } else if change_ratio > 0.80 {
            confidence -= 0.10;
        }
    }

    confidence.clamp(0.40, 0.95)
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
) -> Result<Option<RemoteGenerationResult>, String> {
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
            let parsed = parse_specialist_model_output(&response.text);

            let intent_description = parsed
                .as_ref()
                .and_then(|payload| payload.intent_description.clone())
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| {
                    format!(
                        "{} proposal for {}: {}",
                        task.persona,
                        file_path,
                        task.objective.trim()
                    )
                });

            let modified_content = parsed
                .as_ref()
                .and_then(|payload| payload.modified_content.clone())
                .map(|value| strip_code_fences(&value))
                .filter(|value| !value.trim().is_empty());

            let (diff_content, confidence) = match (target_file_content, modified_content) {
                (Some(original), Some(ref modified)) if original.trim() != modified.trim() => {
                    let original_normalized = original.replace("\r\n", "\n");
                    let modified_normalized = modified.replace("\r\n", "\n");
                    let diff = compute_unified_diff(
                        file_path,
                        &original_normalized,
                        &modified_normalized,
                    );
                    if diff.trim().is_empty() {
                        return Err(format!(
                            "LLM returned modifiedContent for {} but computed diff was empty",
                            file_path
                        ));
                    }
                    let confidence =
                        estimate_llm_confidence(task, &original_normalized, &modified_normalized);
                    (diff, confidence)
                }
                (None, Some(ref modified)) => {
                    let modified_normalized = modified.replace("\r\n", "\n");
                    let diff = compute_unified_diff(file_path, "", &modified_normalized);
                    (diff, 0.60)
                }
                (Some(_), Some(_)) => {
                    // LLM returned content identical to original — no-op change
                    return Err(format!(
                        "LLM returned unchanged content for {} — no modifications produced",
                        file_path
                    ));
                }
                (Some(_), None) => {
                    // LLM was called but returned null modifiedContent.
                    // Include raw response excerpt for debugging.
                    let reason = parsed
                        .as_ref()
                        .and_then(|p| p.intent_description.as_deref())
                        .unwrap_or("no reason provided");
                    let raw_excerpt: String = response.text.chars().take(300).collect();
                    return Err(format!(
                        "LLM returned no modifiedContent for {}: {}. Raw response (first 300 chars): {}",
                        file_path, reason, raw_excerpt
                    ));
                }
                (None, None) => {
                    let raw_excerpt: String = response.text.chars().take(300).collect();
                    return Err(format!(
                        "LLM returned no modifiedContent and no file content available for {}. Raw response (first 300 chars): {}",
                        file_path, raw_excerpt
                    ));
                }
            };

            let output_tokens = response.output_tokens.or_else(|| {
                response
                    .input_tokens
                    .map(|input_tokens| (input_tokens / 6).max(1))
            });

            Ok(Some(RemoteGenerationResult {
                intent_description,
                diff_content,
                confidence,
                output_tokens,
            }))
        }
        Err(error) => {
            // Always propagate LLM errors. Silently falling back to a
            // comment-insertion diff hides the real problem and produces
            // mutations that look "successful" but contain no useful changes.
            Err(format!("LLM adapter failed: {error}"))
        }
    }
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
You will receive a file to modify and an objective.
Apply the MINIMUM changes needed to accomplish the objective.

Respond with JSON only:
{
  "intentDescription": "what this change accomplishes",
  "modifiedContent": "the COMPLETE modified file content with your changes applied",
  "changesSummary": ["change 1", "change 2"]
}

Rules:
- Return the FULL file content with your modifications applied in modifiedContent.
- Make minimal, focused changes — do not rewrite unrelated code.
- Preserve existing formatting, style, and indentation.
- If the objective cannot be safely accomplished, set modifiedContent to null and explain in intentDescription.
- Do not wrap the JSON response in markdown fences."#
            .to_string();

    let context_excerpt = task
        .code_context
        .iter()
        .take(3)
        .map(|block| {
            format!(
                "[{}:{}-{}]\n{}",
                block.file_path, block.start_line, block.end_line, block.content
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    let file_content = target_file_content
        .map(|content| {
            if content.len() > 32_000 {
                let truncated: String = content.chars().take(32_000).collect();
                format!("{truncated}\n\n... [truncated at 32000 chars]")
            } else {
                content.to_string()
            }
        })
        .unwrap_or_else(|| "<file not available — create new file content>".to_string());

    let constraints_text = if task.constraints.is_empty() {
        "none".to_string()
    } else {
        task.constraints.join(" | ")
    };

    let user_prompt = format!(
        "persona: {}\nobjective: {}\nfilePath: {}\nconstraints: {}\n\n--- FILE CONTENT START ---\n{}\n--- FILE CONTENT END ---\n\ncodeContext:\n{}\n",
        task.persona.trim(),
        task.objective.trim(),
        file_path,
        constraints_text,
        file_content,
        context_excerpt
    );

    (system_prompt, user_prompt)
}

fn parse_specialist_model_output(raw: &str) -> Option<SpecialistModelOutput> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    // Try direct parse first (clean JSON)
    if let Some(parsed) = try_parse_json(trimmed) {
        return Some(parsed);
    }

    // Try stripping code fences at the start
    let cleaned = strip_code_fences(trimmed);
    if cleaned != trimmed {
        if let Some(parsed) = try_parse_json(&cleaned) {
            return Some(parsed);
        }
    }

    // Try extracting JSON from mixed text (LLM often adds explanation before/after)
    if let Some(json_str) = extract_json_object(trimmed) {
        if let Some(parsed) = try_parse_json(&json_str) {
            return Some(parsed);
        }
    }

    // Try extracting from code fences anywhere in the text
    if let Some(fenced) = extract_fenced_content(trimmed) {
        if let Some(parsed) = try_parse_json(&fenced) {
            return Some(parsed);
        }
    }

    None
}

fn try_parse_json(text: &str) -> Option<SpecialistModelOutput> {
    if let Ok(parsed) = serde_json::from_str::<SpecialistModelOutput>(text) {
        return Some(parsed);
    }
    if let Ok(value) = serde_json::from_str::<Value>(text) {
        if let Some(object) = value.as_object() {
            let payload = SpecialistModelOutput {
                intent_description: object
                    .get("intentDescription")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                modified_content: object
                    .get("modifiedContent")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                changes_summary: object
                    .get("changesSummary")
                    .and_then(Value::as_array)
                    .map(|arr| {
                        arr.iter()
                            .filter_map(Value::as_str)
                            .map(str::to_string)
                            .collect()
                    }),
            };
            return Some(payload);
        }
    }
    None
}

/// Find the first `{...}` JSON object in the text using brace counting.
fn extract_json_object(text: &str) -> Option<String> {
    let start = text.find('{')?;
    let mut depth = 0_i32;
    let mut in_string = false;
    let mut escape_next = false;

    for (i, ch) in text[start..].char_indices() {
        if escape_next {
            escape_next = false;
            continue;
        }
        match ch {
            '\\' if in_string => escape_next = true,
            '"' => in_string = !in_string,
            '{' if !in_string => depth += 1,
            '}' if !in_string => {
                depth -= 1;
                if depth == 0 {
                    return Some(text[start..start + i + 1].to_string());
                }
            }
            _ => {}
        }
    }
    None
}

/// Extract content from code fences anywhere in the text (not just at start).
fn extract_fenced_content(text: &str) -> Option<String> {
    let fence_start = text.find("```")?;
    let after_fence = &text[fence_start + 3..];
    // Skip the language tag (e.g., "json\n")
    let content_start = after_fence.find('\n').map(|pos| pos + 1)?;
    let inner = &after_fence[content_start..];
    let fence_end = inner.find("```")?;
    Some(inner[..fence_end].trim().to_string())
}

/// Compute a proper unified diff using the `similar` crate.
/// This guarantees valid patch format with correct hunk headers.
/// The output always ends with `\n` so `git apply` can parse it.
pub fn compute_unified_diff(file_path: &str, original: &str, modified: &str) -> String {
    let diff = TextDiff::from_lines(original, modified);
    let mut output = diff
        .unified_diff()
        .context_radius(3)
        .header(&format!("a/{file_path}"), &format!("b/{file_path}"))
        .to_string();
    if !output.is_empty() && !output.ends_with('\n') {
        output.push('\n');
    }
    output
}

fn strip_code_fences(input: &str) -> String {
    let trimmed = input.trim();

    if let Some(rest) = trimmed.strip_prefix("```") {
        let after_lang = if let Some(newline_pos) = rest.find('\n') {
            &rest[newline_pos + 1..]
        } else {
            rest
        };
        if let Some(content) = after_lang.strip_suffix("```") {
            return content.trim().to_string();
        }
    }

    trimmed.to_string()
}

fn build_fallback_diff(
    file_path: &str,
    target_file_content: Option<&str>,
    persona: &str,
    objective: &str,
) -> String {
    let compact_objective: String = objective
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(120)
        .collect();

    let comment = match file_path.rsplit('.').next() {
        Some("py") => format!("# AOP({persona}): {compact_objective}"),
        Some("md") => format!("<!-- AOP({persona}): {compact_objective} -->"),
        _ => format!("// AOP({persona}): {compact_objective}"),
    };

    let original = target_file_content.unwrap_or("");
    let original_normalized = original.replace("\r\n", "\n");
    let modified = if original_normalized.is_empty() {
        format!("{comment}\n")
    } else {
        format!("{comment}\n{original_normalized}")
    };

    compute_unified_diff(file_path, &original_normalized, &modified)
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
        assert!(proposal.diff_content.contains("+++ b/src/session.tsx"));
        assert!(proposal.diff_content.contains("@@"));
        assert!(proposal.confidence >= 0.3);
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
    fn compute_unified_diff_generates_valid_patch() {
        let original = "line 1\nline 2\nline 3\n";
        let modified = "line 1\nline 2 modified\nline 3\nnew line 4\n";
        let diff = compute_unified_diff("src/test.ts", original, modified);

        assert!(diff.contains("--- a/src/test.ts"));
        assert!(diff.contains("+++ b/src/test.ts"));
        assert!(diff.contains("@@"));
        assert!(diff.contains("+line 2 modified"));
        assert!(diff.contains("-line 2"));
        assert!(diff.contains("+new line 4"));
    }

    #[test]
    fn compute_unified_diff_handles_new_file() {
        let diff = compute_unified_diff("src/new.ts", "", "export const x = 1;\n");

        assert!(diff.contains("--- a/src/new.ts"));
        assert!(diff.contains("+++ b/src/new.ts"));
        assert!(diff.contains("+export const x = 1;"));
    }

    #[test]
    fn strip_code_fences_removes_json_wrapper() {
        let fenced = "```json\n{\"key\": \"value\"}\n```";
        assert_eq!(strip_code_fences(fenced), "{\"key\": \"value\"}");

        let plain = "{\"key\": \"value\"}";
        assert_eq!(strip_code_fences(plain), "{\"key\": \"value\"}");
    }

    #[test]
    fn fallback_diff_is_valid_unified_format() {
        let diff = build_fallback_diff(
            "src/app.tsx",
            Some("const App = () => null;\n"),
            "react_specialist",
            "Improve component",
        );

        assert!(diff.contains("--- a/src/app.tsx"));
        assert!(diff.contains("+++ b/src/app.tsx"));
        assert!(diff.contains("@@"));
        assert!(diff.contains("AOP(react_specialist)"));
    }

    #[test]
    fn parse_specialist_output_extracts_modified_content() {
        let raw = r#"{"intentDescription":"add guard","modifiedContent":"const x = 1;\n","changesSummary":["added guard"]}"#;
        let parsed = parse_specialist_model_output(raw).expect("should parse");
        assert_eq!(
            parsed.intent_description.as_deref(),
            Some("add guard")
        );
        assert_eq!(
            parsed.modified_content.as_deref(),
            Some("const x = 1;\n")
        );
        assert_eq!(
            parsed.changes_summary.as_deref(),
            Some(&["added guard".to_string()][..])
        );
    }
}
