use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const CONFIG_FILE_NAME: &str = "aop_models.json";
const CONFIG_PATH_ENV: &str = "AOP_MODEL_CONFIG_PATH";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelProfile {
    pub provider: String,
    pub model_id: String,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub max_output_tokens: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelRoutingConfig {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default = "default_provider")]
    pub default_provider: String,
    #[serde(default)]
    pub tiers: HashMap<String, ModelProfile>,
    #[serde(default)]
    pub persona_overrides: HashMap<String, ModelProfile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelSelection {
    pub tier: u8,
    pub persona: Option<String>,
    pub provider: String,
    pub model_id: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelRegistrySnapshot {
    pub config_path: String,
    pub loaded_from_file: bool,
    pub load_error: Option<String>,
    pub config: ModelRoutingConfig,
}

#[derive(Debug, Clone)]
pub struct ModelRegistry {
    config_path: PathBuf,
    loaded_from_file: bool,
    load_error: Option<String>,
    config: ModelRoutingConfig,
}

impl Default for ModelRegistry {
    fn default() -> Self {
        let config = default_config();
        Self {
            config_path: PathBuf::from(CONFIG_FILE_NAME),
            loaded_from_file: false,
            load_error: None,
            config,
        }
    }
}

impl ModelRegistry {
    pub fn load(workspace_root: &Path) -> Self {
        let config_path = resolve_config_path(workspace_root);
        if !config_path.exists() {
            return Self {
                config_path,
                loaded_from_file: false,
                load_error: None,
                config: default_config(),
            };
        }

        match fs::read_to_string(&config_path) {
            Ok(raw) => match serde_json::from_str::<ModelRoutingConfig>(&raw) {
                Ok(parsed) => Self {
                    config_path,
                    loaded_from_file: true,
                    load_error: None,
                    config: sanitize_config(parsed),
                },
                Err(error) => Self {
                    config_path,
                    loaded_from_file: false,
                    load_error: Some(format!(
                        "Failed to parse {CONFIG_FILE_NAME}; using defaults: {error}"
                    )),
                    config: default_config(),
                },
            },
            Err(error) => Self {
                config_path,
                loaded_from_file: false,
                load_error: Some(format!(
                    "Failed to read {CONFIG_FILE_NAME}; using defaults: {error}"
                )),
                config: default_config(),
            },
        }
    }

    pub fn snapshot(&self) -> ModelRegistrySnapshot {
        ModelRegistrySnapshot {
            config_path: self.config_path.to_string_lossy().to_string(),
            loaded_from_file: self.loaded_from_file,
            load_error: self.load_error.clone(),
            config: self.config.clone(),
        }
    }

    pub fn resolve(&self, tier: u8, persona: Option<&str>) -> Result<ModelSelection, String> {
        if !(1..=3).contains(&tier) {
            return Err("tier must be 1, 2, or 3".to_string());
        }

        let normalized_persona = persona
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.to_ascii_lowercase());

        if let Some(persona_key) = &normalized_persona {
            if let Some(profile) = self.config.persona_overrides.get(persona_key) {
                return Ok(ModelSelection {
                    tier,
                    persona: normalized_persona,
                    provider: profile.provider.clone(),
                    model_id: profile.model_id.clone(),
                    source: "persona_override".to_string(),
                });
            }
        }

        let tier_key = tier.to_string();
        if let Some(profile) = self.config.tiers.get(&tier_key) {
            return Ok(ModelSelection {
                tier,
                persona: normalized_persona,
                provider: profile.provider.clone(),
                model_id: profile.model_id.clone(),
                source: "tier".to_string(),
            });
        }

        let defaults = default_tier_profiles();
        let fallback = defaults
            .get(&tier_key)
            .ok_or_else(|| format!("No default model mapping for tier {tier}"))?;
        Ok(ModelSelection {
            tier,
            persona: normalized_persona,
            provider: fallback.provider.clone(),
            model_id: fallback.model_id.clone(),
            source: "default".to_string(),
        })
    }
}

fn resolve_config_path(workspace_root: &Path) -> PathBuf {
    if let Some(override_path) = std::env::var_os(CONFIG_PATH_ENV) {
        return PathBuf::from(override_path);
    }
    workspace_root.join(CONFIG_FILE_NAME)
}

fn sanitize_config(config: ModelRoutingConfig) -> ModelRoutingConfig {
    let defaults = default_tier_profiles();
    let default_provider = if config.default_provider.trim().is_empty() {
        default_provider()
    } else {
        config.default_provider.trim().to_string()
    };

    let mut tiers = HashMap::new();
    for tier in 1..=3 {
        let key = tier.to_string();
        let fallback = defaults
            .get(&key)
            .cloned()
            .unwrap_or_else(|| default_tier_profile(tier));
        let normalized = config
            .tiers
            .get(&key)
            .cloned()
            .map(|profile| normalize_profile(profile, &fallback, &default_provider))
            .unwrap_or_else(|| normalize_profile(fallback.clone(), &fallback, &default_provider));
        tiers.insert(key, normalized);
    }

    let tier3_fallback = defaults
        .get("3")
        .cloned()
        .unwrap_or_else(|| default_tier_profile(3));

    let mut persona_overrides = HashMap::new();
    for (persona, profile) in config.persona_overrides {
        let persona_key = persona.trim().to_ascii_lowercase();
        if persona_key.is_empty() {
            continue;
        }
        persona_overrides.insert(
            persona_key,
            normalize_profile(profile, &tier3_fallback, &default_provider),
        );
    }

    ModelRoutingConfig {
        version: if config.version == 0 {
            default_version()
        } else {
            config.version
        },
        default_provider,
        tiers,
        persona_overrides,
    }
}

fn normalize_profile(
    profile: ModelProfile,
    fallback: &ModelProfile,
    default_provider_value: &str,
) -> ModelProfile {
    let provider = if profile.provider.trim().is_empty() {
        if fallback.provider.trim().is_empty() {
            default_provider_value.to_string()
        } else {
            fallback.provider.clone()
        }
    } else {
        profile.provider.trim().to_string()
    };

    let model_id = if profile.model_id.trim().is_empty() {
        fallback.model_id.clone()
    } else {
        profile.model_id.trim().to_string()
    };

    let temperature = profile.temperature.map(|value| value.clamp(0.0, 2.0));
    let max_output_tokens = profile.max_output_tokens;

    ModelProfile {
        provider,
        model_id,
        temperature,
        max_output_tokens,
    }
}

fn default_config() -> ModelRoutingConfig {
    ModelRoutingConfig {
        version: default_version(),
        default_provider: default_provider(),
        tiers: default_tier_profiles(),
        persona_overrides: HashMap::new(),
    }
}

fn default_tier_profiles() -> HashMap<String, ModelProfile> {
    HashMap::from([
        ("1".to_string(), default_tier_profile(1)),
        ("2".to_string(), default_tier_profile(2)),
        ("3".to_string(), default_tier_profile(3)),
    ])
}

fn default_tier_profile(tier: u8) -> ModelProfile {
    let model_id = match tier {
        1 => "gpt-5",
        2 => "gpt-5-mini",
        _ => "gpt-5-nano",
    };

    ModelProfile {
        provider: default_provider(),
        model_id: model_id.to_string(),
        temperature: Some(0.2),
        max_output_tokens: None,
    }
}

fn default_provider() -> String {
    "openai".to_string()
}

fn default_version() -> u32 {
    1
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn uses_defaults_when_config_file_is_missing() {
        let temp = tempdir().expect("temp directory should be created");
        let registry = ModelRegistry::load(temp.path());

        assert!(!registry.snapshot().loaded_from_file);
        let tier1 = registry
            .resolve(1, None)
            .expect("tier 1 model should resolve");
        assert_eq!(tier1.model_id, "gpt-5");
    }

    #[test]
    fn loads_tier_and_persona_overrides_from_json() {
        let temp = tempdir().expect("temp directory should be created");
        let config_path = temp.path().join(CONFIG_FILE_NAME);
        fs::write(
            &config_path,
            r#"{
  "version": 1,
  "defaultProvider": "openai",
  "tiers": {
    "1": { "provider": "openai", "modelId": "gpt-5-pro" },
    "2": { "provider": "openai", "modelId": "gpt-5-mini" },
    "3": { "provider": "openai", "modelId": "gpt-5-nano" }
  },
  "personaOverrides": {
    "security_analyst": { "provider": "openai", "modelId": "o3" }
  }
}"#,
        )
        .expect("config should be written");

        let registry = ModelRegistry::load(temp.path());
        let security = registry
            .resolve(3, Some("security_analyst"))
            .expect("persona override should resolve");
        let tier1 = registry
            .resolve(1, None)
            .expect("tier 1 model should resolve");

        assert_eq!(security.source, "persona_override");
        assert_eq!(security.model_id, "o3");
        assert_eq!(tier1.model_id, "gpt-5-pro");
    }

    #[test]
    fn falls_back_to_defaults_when_json_is_invalid() {
        let temp = tempdir().expect("temp directory should be created");
        let config_path = temp.path().join(CONFIG_FILE_NAME);
        fs::write(&config_path, "{ invalid json").expect("config should be written");

        let registry = ModelRegistry::load(temp.path());
        let snapshot = registry.snapshot();

        assert!(!snapshot.loaded_from_file);
        assert!(snapshot.load_error.is_some());
        let tier3 = registry
            .resolve(3, None)
            .expect("tier 3 model should resolve");
        assert_eq!(tier3.model_id, "gpt-5-nano");
    }

    #[test]
    fn rejects_invalid_tier_resolution() {
        let registry = ModelRegistry::default();
        let error = registry
            .resolve(4, Some("react_specialist"))
            .expect_err("tier 4 must be rejected");
        assert_eq!(error, "tier must be 1, 2, or 3");
    }
}
