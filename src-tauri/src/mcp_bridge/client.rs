use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine as _;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::process::Command;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::mcp_bridge::tool_caller::BridgeRequest;

#[derive(Debug, Clone)]
pub struct BridgeClient {
    bridge_dir: Arc<PathBuf>,
    rate_limiter: Arc<Mutex<RateLimiterState>>,
    max_calls_per_minute: usize,
    concurrent_calls: Arc<Semaphore>,
    queued_calls: Arc<AtomicUsize>,
}

#[derive(Debug, Serialize, Deserialize)]
struct BridgeEnvelope {
    ok: bool,
    data: Option<Value>,
    error: Option<String>,
}

#[derive(Debug, Default)]
struct RateLimiterState {
    call_timestamps: VecDeque<Instant>,
}

#[derive(Debug)]
struct BridgeCallGuard {
    _permit: OwnedSemaphorePermit,
}

impl BridgeClient {
    const DEFAULT_MAX_CALLS_PER_MINUTE: usize = 120;
    const DEFAULT_MAX_CONCURRENT_CALLS: usize = 10;
    const MAX_QUEUED_CALLS: usize = 50;
    const RATE_LIMIT_WINDOW: Duration = Duration::from_secs(60);

    pub fn new(workspace_root: &Path) -> Self {
        Self::with_limits(
            workspace_root,
            Self::DEFAULT_MAX_CALLS_PER_MINUTE,
            Self::DEFAULT_MAX_CONCURRENT_CALLS,
        )
    }

    fn with_limits(
        workspace_root: &Path,
        max_calls_per_minute: usize,
        max_concurrent_calls: usize,
    ) -> Self {
        let effective_max_calls = max_calls_per_minute.max(1);
        let effective_max_concurrent = max_concurrent_calls.max(1);

        Self {
            bridge_dir: Arc::new(resolve_bridge_dir(workspace_root)),
            rate_limiter: Arc::new(Mutex::new(RateLimiterState::default())),
            max_calls_per_minute: effective_max_calls,
            concurrent_calls: Arc::new(Semaphore::new(effective_max_concurrent)),
            queued_calls: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub async fn call<T: DeserializeOwned>(&self, request: &BridgeRequest) -> Result<T, String> {
        let _call_guard = self.reserve_call_slot().await?;

        if !self.bridge_dir.exists() {
            return Err(format!(
                "MCP bridge directory does not exist: {}. Ensure `mcp-bridge/` is present.",
                self.bridge_dir.display()
            ));
        }

        let request_json = serde_json::to_string(request)
            .map_err(|error| format!("Failed to encode bridge request: {error}"))?;
        let request_json_base64 = BASE64_STANDARD.encode(request_json.as_bytes());

        let output = Command::new("pnpm")
            .arg("--silent")
            .arg("--dir")
            .arg(self.bridge_dir.as_ref())
            .arg("exec")
            .arg("tsx")
            .arg("src/index.ts")
            .arg("--request-base64")
            .arg(request_json_base64)
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

    async fn reserve_call_slot(&self) -> Result<BridgeCallGuard, String> {
        let queued = self.queued_calls.fetch_add(1, Ordering::SeqCst) + 1;
        if queued > Self::MAX_QUEUED_CALLS {
            self.queued_calls.fetch_sub(1, Ordering::SeqCst);
            return Err(format!(
                "RATE_LIMIT_EXCEEDED: queued MCP calls exceeded {}",
                Self::MAX_QUEUED_CALLS
            ));
        }

        let permit = self
            .concurrent_calls
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| {
                "SERVER_UNAVAILABLE: MCP bridge concurrency limiter unavailable".to_string()
            })?;

        self.queued_calls.fetch_sub(1, Ordering::SeqCst);
        self.check_rate_limit_with_now(Instant::now())?;
        Ok(BridgeCallGuard { _permit: permit })
    }

    fn check_rate_limit_with_now(&self, now: Instant) -> Result<(), String> {
        let mut guard = self
            .rate_limiter
            .lock()
            .map_err(|_| "SERVER_UNAVAILABLE: MCP bridge rate limiter unavailable".to_string())?;

        while let Some(timestamp) = guard.call_timestamps.front().copied() {
            if now.duration_since(timestamp) >= Self::RATE_LIMIT_WINDOW {
                guard.call_timestamps.pop_front();
            } else {
                break;
            }
        }

        if guard.call_timestamps.len() >= self.max_calls_per_minute {
            return Err(format!(
                "RATE_LIMIT_EXCEEDED: exceeded {} MCP calls per minute",
                self.max_calls_per_minute
            ));
        }

        guard.call_timestamps.push_back(now);
        Ok(())
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
    use std::sync::atomic::Ordering;
    use std::time::{Duration, Instant};

    use tempfile::tempdir;

    use super::{resolve_bridge_dir, BridgeClient};

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

    #[test]
    fn rate_limiter_blocks_call_121_within_a_minute() {
        let temp = tempdir().expect("temp dir should be created");
        let root = temp.path();
        let client = BridgeClient::with_limits(root, 120, 10);
        let now = Instant::now();

        for _ in 0..120 {
            client
                .check_rate_limit_with_now(now)
                .expect("first 120 calls should be accepted");
        }

        let error = client
            .check_rate_limit_with_now(now)
            .expect_err("121st call should be rejected");
        assert!(error.contains("RATE_LIMIT_EXCEEDED"));
    }

    #[test]
    fn rate_limiter_recovers_after_window_expires() {
        let temp = tempdir().expect("temp dir should be created");
        let root = temp.path();
        let client = BridgeClient::with_limits(root, 2, 10);
        let now = Instant::now();

        client
            .check_rate_limit_with_now(now)
            .expect("first call should pass");
        client
            .check_rate_limit_with_now(now)
            .expect("second call should pass");
        let limited = client.check_rate_limit_with_now(now);
        assert!(limited.is_err(), "third call in same window should fail");

        let later = now + Duration::from_secs(61);
        client
            .check_rate_limit_with_now(later)
            .expect("call after 60s window should pass");
    }

    #[tokio::test]
    async fn queue_limit_rejects_when_backpressure_exceeds_capacity() {
        let temp = tempdir().expect("temp dir should be created");
        let root = temp.path();
        let client = BridgeClient::with_limits(root, 120, 1);

        let permit = client
            .concurrent_calls
            .clone()
            .acquire_owned()
            .await
            .expect("semaphore should provide permit");

        client
            .queued_calls
            .store(BridgeClient::MAX_QUEUED_CALLS, Ordering::SeqCst);

        let error = client
            .reserve_call_slot()
            .await
            .expect_err("queue overflow should fail");
        assert!(error.contains("RATE_LIMIT_EXCEEDED"));

        drop(permit);
    }
}
