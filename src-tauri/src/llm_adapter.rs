use std::collections::HashMap;
use std::process::Command;

use serde::Deserialize;
use serde_json::{json, Value};

const CLAUDE_CODE_PROVIDER_ALIASES: &[&str] =
    &["claude_code", "claude-code", "anthropic_claude_code"];
const OPENAI_PROVIDER_ALIASES: &[&str] = &["openai", "openai_api"];

#[derive(Debug, Clone)]
pub struct AdapterRequest {
    pub provider: String,
    pub model_id: String,
    pub system_prompt: String,
    pub user_prompt: String,
}

#[derive(Debug, Clone)]
pub struct AdapterResponse {
    pub text: String,
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
    #[allow(dead_code)]
    pub total_cost_usd: Option<f64>,
    #[allow(dead_code)]
    pub resolved_model: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClaudeUsage {
    #[serde(default)]
    input_tokens: Option<u32>,
    #[serde(default)]
    output_tokens: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClaudePrintResult {
    #[serde(default)]
    subtype: String,
    #[serde(default)]
    is_error: bool,
    #[serde(default)]
    result: Option<String>,
    #[serde(default)]
    usage: Option<ClaudeUsage>,
    #[serde(default)]
    total_cost_usd: Option<f64>,
    #[serde(default, rename = "modelUsage")]
    model_usage: HashMap<String, Value>,
    #[serde(default)]
    errors: Vec<Value>,
}

pub fn generate(request: &AdapterRequest) -> Result<AdapterResponse, String> {
    let normalized = normalize_provider(&request.provider);
    if CLAUDE_CODE_PROVIDER_ALIASES
        .iter()
        .any(|alias| normalized == *alias)
    {
        return call_claude_code(request);
    }
    if OPENAI_PROVIDER_ALIASES
        .iter()
        .any(|alias| normalized == *alias)
    {
        return call_openai_chat_completions(request);
    }

    Err(format!(
        "No remote adapter configured for provider '{}'",
        request.provider.trim()
    ))
}

pub fn supports_provider(provider: &str) -> bool {
    let normalized = normalize_provider(provider);
    if CLAUDE_CODE_PROVIDER_ALIASES
        .iter()
        .any(|alias| normalized == *alias)
    {
        return true;
    }
    if OPENAI_PROVIDER_ALIASES
        .iter()
        .any(|alias| normalized == *alias)
    {
        return std::env::var("OPENAI_API_KEY")
            .ok()
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false);
    }
    false
}

pub fn supported_provider_aliases() -> Vec<String> {
    let mut values = CLAUDE_CODE_PROVIDER_ALIASES
        .iter()
        .map(|value| value.to_string())
        .collect::<Vec<_>>();
    if std::env::var("OPENAI_API_KEY")
        .ok()
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
    {
        values.extend(OPENAI_PROVIDER_ALIASES.iter().map(|value| value.to_string()));
    }
    values
}

fn normalize_provider(provider: &str) -> String {
    provider.trim().to_ascii_lowercase()
}

fn call_claude_code(request: &AdapterRequest) -> Result<AdapterResponse, String> {
    if request.model_id.trim().is_empty() {
        return Err("Model adapter requires non-empty modelId".to_string());
    }

    let mut command = Command::new("claude");
    command
        .arg("-p")
        .arg("--output-format")
        .arg("json")
        .arg("--disable-slash-commands")
        .arg("--no-session-persistence")
        .arg("--model")
        .arg(request.model_id.trim())
        .arg("--system-prompt")
        .arg(request.system_prompt.trim())
        .arg(request.user_prompt.trim());

    if let Some(max_budget) = read_optional_max_budget() {
        command.arg("--max-budget-usd").arg(max_budget);
    }

    let output = command
        .output()
        .map_err(|error| format!("Failed to execute Claude Code CLI: {error}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(format!(
            "Claude Code adapter failed with status {}.\nstdout:\n{}\nstderr:\n{}",
            output.status, stdout, stderr
        ));
    }

    let stdout = String::from_utf8(output.stdout)
        .map_err(|error| format!("Claude Code stdout is not UTF-8: {error}"))?;
    let parsed: ClaudePrintResult = parse_last_json_line(&stdout)?;

    let result_text = parsed
        .result
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    if parsed.is_error || parsed.subtype.starts_with("error") || result_text.is_none() {
        let error_details = if parsed.errors.is_empty() {
            "No additional error details".to_string()
        } else {
            parsed
                .errors
                .iter()
                .map(Value::to_string)
                .collect::<Vec<_>>()
                .join("; ")
        };
        return Err(format!(
            "Claude Code returned subtype '{}' without usable result. {}",
            parsed.subtype, error_details
        ));
    }

    let resolved_model = parsed.model_usage.keys().next().cloned();
    let usage = parsed.usage;
    Ok(AdapterResponse {
        text: result_text.unwrap_or_default(),
        input_tokens: usage.as_ref().and_then(|value| value.input_tokens),
        output_tokens: usage.as_ref().and_then(|value| value.output_tokens),
        total_cost_usd: parsed.total_cost_usd,
        resolved_model,
    })
}

fn call_openai_chat_completions(request: &AdapterRequest) -> Result<AdapterResponse, String> {
    if request.model_id.trim().is_empty() {
        return Err("Model adapter requires non-empty modelId".to_string());
    }
    let api_key = std::env::var("OPENAI_API_KEY")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "OPENAI_API_KEY is required for openai provider".to_string())?;

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|error| format!("Failed to build OpenAI HTTP client: {error}"))?;

    let body = json!({
        "model": request.model_id.trim(),
        "messages": [
            { "role": "system", "content": request.system_prompt.trim() },
            { "role": "user", "content": request.user_prompt.trim() }
        ],
        "temperature": 0.2
    });
    let response = client
        .post("https://api.openai.com/v1/chat/completions")
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .map_err(|error| format!("Failed to call OpenAI API: {error}"))?;

    let status = response.status();
    let payload: Value = response
        .json()
        .map_err(|error| format!("Invalid OpenAI response payload: {error}"))?;
    if !status.is_success() {
        return Err(format!(
            "OpenAI adapter failed with status {}: {}",
            status,
            payload
        ));
    }

    let text = payload
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("OpenAI response did not include assistant content: {payload}"))?
        .to_string();

    let input_tokens = payload
        .get("usage")
        .and_then(|usage| usage.get("prompt_tokens"))
        .and_then(Value::as_u64)
        .map(|value| value as u32);
    let output_tokens = payload
        .get("usage")
        .and_then(|usage| usage.get("completion_tokens"))
        .and_then(Value::as_u64)
        .map(|value| value as u32);

    Ok(AdapterResponse {
        text,
        input_tokens,
        output_tokens,
        total_cost_usd: None,
        resolved_model: payload
            .get("model")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
    })
}

fn read_optional_max_budget() -> Option<String> {
    let raw = std::env::var("AOP_CLAUDE_MAX_BUDGET_USD").ok()?;
    let value = raw.trim();
    if value.is_empty() {
        return None;
    }
    value.parse::<f64>().ok().filter(|number| *number > 0.0)?;
    Some(value.to_string())
}

fn parse_last_json_line<T: for<'de> Deserialize<'de>>(raw_output: &str) -> Result<T, String> {
    for line in raw_output.lines().rev() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(parsed) = serde_json::from_str::<T>(trimmed) {
            return Ok(parsed);
        }
    }

    Err(format!(
        "Unable to parse JSON output from model adapter.\nRaw output:\n{}",
        raw_output
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_last_json_line_reads_json_payload() {
        let raw = "log line\n{\"subtype\":\"success\",\"result\":\"OK\"}\n";
        let parsed: ClaudePrintResult =
            parse_last_json_line(raw).expect("JSON payload should parse");
        assert_eq!(parsed.subtype, "success");
        assert_eq!(parsed.result.as_deref(), Some("OK"));
    }

    #[test]
    fn generate_rejects_unknown_provider() {
        let request = AdapterRequest {
            provider: "unsupported_provider".to_string(),
            model_id: "gpt-5-mini".to_string(),
            system_prompt: "system".to_string(),
            user_prompt: "user".to_string(),
        };

        let error = generate(&request).expect_err("unknown provider should fail");
        assert!(error.contains("No remote adapter configured"));
    }

    #[test]
    fn supports_claude_provider_aliases() {
        assert!(supports_provider("claude_code"));
        assert!(supports_provider("claude-code"));
        assert!(supports_provider("anthropic_claude_code"));
        assert!(!supports_provider("openai"));
    }
}
