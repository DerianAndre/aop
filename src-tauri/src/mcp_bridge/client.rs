use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::process::Command;

use crate::mcp_bridge::tool_caller::BridgeRequest;

#[derive(Debug, Clone)]
pub struct BridgeClient {
    bridge_dir: Arc<PathBuf>,
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
            bridge_dir: Arc::new(resolve_bridge_dir(workspace_root)),
        }
    }

    pub async fn call<T: DeserializeOwned>(&self, request: &BridgeRequest) -> Result<T, String> {
        if !self.bridge_dir.exists() {
            return Err(format!(
                "MCP bridge directory does not exist: {}. Ensure `mcp-bridge/` is present.",
                self.bridge_dir.display()
            ));
        }

        let request_json = serde_json::to_string(request)
            .map_err(|error| format!("Failed to encode bridge request: {error}"))?;

        let output = Command::new("pnpm")
            .arg("--silent")
            .arg("--dir")
            .arg(self.bridge_dir.as_ref())
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

fn resolve_bridge_dir(runtime_cwd: &Path) -> PathBuf {
    let mut candidates: Vec<PathBuf> = vec![runtime_cwd.join("mcp-bridge")];

    if let Some(parent) = runtime_cwd.parent() {
        candidates.push(parent.join("mcp-bridge"));
    }

    let manifest_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if let Some(parent) = manifest_root.parent() {
        candidates.push(parent.join("mcp-bridge"));
    }

    if let Some(override_root) = std::env::var_os("AOP_WORKSPACE_ROOT") {
        candidates.insert(0, PathBuf::from(override_root).join("mcp-bridge"));
    }

    for candidate in &candidates {
        if candidate.exists() {
            return candidate.clone();
        }
    }

    // Fallback: keep deterministic behavior even when path is missing.
    candidates
        .into_iter()
        .next()
        .unwrap_or_else(|| runtime_cwd.join("mcp-bridge"))
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

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::resolve_bridge_dir;

    #[test]
    fn resolves_bridge_when_runtime_is_workspace_root() {
        let temp = tempdir().expect("temp dir should be created");
        let root = temp.path();
        fs::create_dir(root.join("mcp-bridge")).expect("bridge dir should be created");

        let resolved = resolve_bridge_dir(root);
        assert!(resolved.ends_with("mcp-bridge"));
        assert!(resolved.exists());
    }

    #[test]
    fn resolves_bridge_when_runtime_is_src_tauri() {
        let temp = tempdir().expect("temp dir should be created");
        let root = temp.path();
        let src_tauri = root.join("src-tauri");
        fs::create_dir(&src_tauri).expect("src-tauri dir should be created");
        fs::create_dir(root.join("mcp-bridge")).expect("bridge dir should be created");

        let resolved = resolve_bridge_dir(&src_tauri);
        assert!(resolved.ends_with("mcp-bridge"));
        assert!(resolved.exists());
    }
}
