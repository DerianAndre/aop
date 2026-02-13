use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeFlags {
    pub dev_mode: bool,
    pub model_adapter_enabled: bool,
    pub model_adapter_strict: bool,
    pub auto_approve_budget_requests: bool,
    pub budget_headroom_percent: f64,
    pub budget_auto_max_percent: f64,
    pub budget_min_increment: i64,
    pub telemetry_retention_days: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeFlagsUpdateResult {
    pub flags: RuntimeFlags,
    pub restart_required: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetRuntimeFlagsInput {
    pub dev_mode: Option<bool>,
    pub model_adapter_enabled: Option<bool>,
    pub model_adapter_strict: Option<bool>,
    pub auto_approve_budget_requests: Option<bool>,
    pub budget_headroom_percent: Option<f64>,
    pub budget_auto_max_percent: Option<f64>,
    pub budget_min_increment: Option<i64>,
    pub telemetry_retention_days: Option<u32>,
}

impl RuntimeFlags {
    pub fn from_env() -> Self {
        Self {
            dev_mode: env_bool("AOP_DEV_MODE", false),
            model_adapter_enabled: env_bool("AOP_MODEL_ADAPTER_ENABLED", true),
            model_adapter_strict: env_bool("AOP_MODEL_ADAPTER_STRICT", false),
            auto_approve_budget_requests: env_bool("AOP_AUTO_APPROVE_BUDGET_REQUESTS", true),
            budget_headroom_percent: env_f64("AOP_BUDGET_HEADROOM_PERCENT", 25.0, 1.0, 95.0),
            budget_auto_max_percent: env_f64("AOP_BUDGET_AUTO_MAX_PERCENT", 40.0, 5.0, 100.0),
            budget_min_increment: env_i64("AOP_BUDGET_MIN_INCREMENT", 250, 50, 100_000),
            telemetry_retention_days: env_u32("AOP_TELEMETRY_RETENTION_DAYS", 7, 1, 365),
        }
    }

    pub fn apply_update(&mut self, input: SetRuntimeFlagsInput) {
        if let Some(value) = input.dev_mode {
            self.dev_mode = value;
        }
        if let Some(value) = input.model_adapter_enabled {
            self.model_adapter_enabled = value;
        }
        if let Some(value) = input.model_adapter_strict {
            self.model_adapter_strict = value;
        }
        if let Some(value) = input.auto_approve_budget_requests {
            self.auto_approve_budget_requests = value;
        }
        if let Some(value) = input.budget_headroom_percent {
            self.budget_headroom_percent = value.clamp(1.0, 95.0);
        }
        if let Some(value) = input.budget_auto_max_percent {
            self.budget_auto_max_percent = value.clamp(5.0, 100.0);
        }
        if let Some(value) = input.budget_min_increment {
            self.budget_min_increment = value.clamp(50, 100_000);
        }
        if let Some(value) = input.telemetry_retention_days {
            self.telemetry_retention_days = value.clamp(1, 365);
        }
    }

    pub fn sync_to_process_env(&self) {
        std::env::set_var("AOP_DEV_MODE", bool_to_env(self.dev_mode));
        std::env::set_var(
            "AOP_MODEL_ADAPTER_ENABLED",
            bool_to_env(self.model_adapter_enabled),
        );
        std::env::set_var(
            "AOP_MODEL_ADAPTER_STRICT",
            bool_to_env(self.model_adapter_strict),
        );
        std::env::set_var(
            "AOP_AUTO_APPROVE_BUDGET_REQUESTS",
            bool_to_env(self.auto_approve_budget_requests),
        );
        std::env::set_var(
            "AOP_BUDGET_HEADROOM_PERCENT",
            self.budget_headroom_percent.to_string(),
        );
        std::env::set_var(
            "AOP_BUDGET_AUTO_MAX_PERCENT",
            self.budget_auto_max_percent.to_string(),
        );
        std::env::set_var(
            "AOP_BUDGET_MIN_INCREMENT",
            self.budget_min_increment.to_string(),
        );
        std::env::set_var(
            "AOP_TELEMETRY_RETENTION_DAYS",
            self.telemetry_retention_days.to_string(),
        );
    }
}

fn env_bool(key: &str, default: bool) -> bool {
    std::env::var(key)
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(default)
}

fn env_f64(key: &str, default: f64, min: f64, max: f64) -> f64 {
    std::env::var(key)
        .ok()
        .and_then(|value| value.trim().parse::<f64>().ok())
        .map(|value| value.clamp(min, max))
        .unwrap_or(default)
}

fn env_i64(key: &str, default: i64, min: i64, max: i64) -> i64 {
    std::env::var(key)
        .ok()
        .and_then(|value| value.trim().parse::<i64>().ok())
        .map(|value| value.clamp(min, max))
        .unwrap_or(default)
}

fn env_u32(key: &str, default: u32, min: u32, max: u32) -> u32 {
    std::env::var(key)
        .ok()
        .and_then(|value| value.trim().parse::<u32>().ok())
        .map(|value| value.clamp(min, max))
        .unwrap_or(default)
}

fn bool_to_env(value: bool) -> &'static str {
    if value {
        "1"
    } else {
        "0"
    }
}
