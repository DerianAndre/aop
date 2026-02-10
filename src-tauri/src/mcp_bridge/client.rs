use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::process::Command;

use crate::mcp_bridge::tool_caller::BridgeRequest;

#[derive(Debug, Clone)]
pub struct BridgeClient {
    bridge_dir: PathBuf,
}

#[derive(Debug, Serialize, Deserialize)]
struct BridgeEnvelope {
    ok: bool,
    data: Option<Value>,
    error: Option<String>,
}

impl BridgeClient {
    pub fn new(workspace_root: &Path) -> Self {
        Self {
            bridge_dir: workspace_root.join("mcp-bridge"),
        }
    }

    pub async fn call<T: DeserializeOwned>(&self, request: &BridgeRequest) -> Result<T, String> {
        let request_json = serde_json::to_string(request)
            .map_err(|error| format!("Failed to encode bridge request: {error}"))?;

        let output = Command::new("pnpm")
            .arg("--silent")
            .arg("--dir")
            .arg(&self.bridge_dir)
            .arg("exec")
            .arg("tsx")
            .arg("src/index.ts")
            .arg("--request")
            .arg(request_json)
            .output()
            .await
            .map_err(|error| format!("Failed to execute MCP bridge process: {error}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            return Err(format!(
                "MCP bridge failed with status {}.\nstdout:\n{}\nstderr:\n{}",
                output.status, stdout, stderr
            ));
        }

        let stdout = String::from_utf8(output.stdout)
            .map_err(|error| format!("MCP bridge stdout is not UTF-8: {error}"))?;
        let envelope: BridgeEnvelope = parse_last_json_line(&stdout)?;

        if !envelope.ok {
            return Err(envelope
                .error
                .unwrap_or_else(|| "MCP bridge returned an unknown error".to_string()));
        }

        let payload = envelope
            .data
            .ok_or_else(|| "MCP bridge returned no data payload".to_string())?;

        serde_json::from_value(payload)
            .map_err(|error| format!("Failed to decode bridge payload: {error}"))
    }
}

fn parse_last_json_line<T: DeserializeOwned>(raw_output: &str) -> Result<T, String> {
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
        "Unable to parse JSON response from MCP bridge.\nRaw output:\n{}",
        raw_output
    ))
}
