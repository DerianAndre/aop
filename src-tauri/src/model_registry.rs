use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const CONFIG_FILE_NAME: &str = "models.json";
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

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum ModelProfileCandidates {
    Single(ModelProfile),
    Multiple(Vec<ModelProfile>),
}

impl ModelProfileCandidates {
    fn into_vec(self) -> Vec<ModelProfile> {
        match self {
            Self::Single(profile) => vec![profile],
            Self::Multiple(values) => values,
        }
    }
}

fn deserialize_profile_map<'de, D>(
    deserializer: D,
) -> Result<HashMap<String, Vec<ModelProfile>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = HashMap::<String, ModelProfileCandidates>::deserialize(deserializer)?;
    Ok(raw
        .into_iter()
        .map(|(key, value)| (key, value.into_vec()))
        .collect())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelRoutingConfig {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default = "default_provider")]
    pub default_provider: String,
    #[serde(default, deserialize_with = "deserialize_profile_map")]
    pub tiers: HashMap<String, Vec<ModelProfile>>,
    #[serde(default, deserialize_with = "deserialize_profile_map")]
    pub persona_overrides: HashMap<String, Vec<ModelProfile>>,
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
        self.resolve_with_supported_providers(tier, persona, &[])
    }

    pub fn resolve_with_supported_providers(
        &self,
        tier: u8,
        persona: Option<&str>,
        supported_providers: &[String],
    ) -> Result<ModelSelection, String> {
        if !(1..=3).contains(&tier) {
            return Err("tier must be 1, 2, or 3".to_string());
        }

        let normalized_persona = persona
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.to_ascii_lowercase());

        let supported_set = supported_providers
            .iter()
            .map(|value| normalize_provider(value))
            .collect::<HashSet<_>>();
        let filter_by_adapter = !supported_set.is_empty();

        if let Some(persona_key) = &normalized_persona {
            if let Some(candidates) = self.config.persona_overrides.get(persona_key) {
                if let Some(profile) = select_profile(candidates, &supported_set, filter_by_adapter)
                {
                    return Ok(ModelSelection {
                        tier,
                        persona: normalized_persona,
                        provider: profile.provider.clone(),
                        model_id: profile.model_id.clone(),
                        source: "persona_override".to_string(),
                    });
                }
            }
        }

        let tier_key = tier.to_string();
        if let Some(candidates) = self.config.tiers.get(&tier_key) {
            if let Some(profile) = select_profile(candidates, &supported_set, filter_by_adapter) {
                return Ok(ModelSelection {
                    tier,
                    persona: normalized_persona,
                    provider: profile.provider.clone(),
                    model_id: profile.model_id.clone(),
                    source: "tier".to_string(),
                });
            }
        }

        let defaults = default_tier_profiles();
        let fallback_candidates = defaults
            .get(&tier_key)
            .ok_or_else(|| format!("No default model mapping for tier {tier}"))?;
        let profile =
            select_profile(fallback_candidates, &supported_set, filter_by_adapter).ok_or_else(
                || {
                    if filter_by_adapter {
                        format!(
                            "No available provider adapter for tier {}. Config providers: {}. Available adapters: {}",
                            tier,
                            fallback_candidates
                                .iter()
                                .map(|profile| profile.provider.as_str())
                                .collect::<Vec<_>>()
                                .join(", "),
                            supported_set.iter().cloned().collect::<Vec<_>>().join(", ")
                        )
                    } else {
                        format!("No default model mapping for tier {tier}")
                    }
                },
            )?;

        Ok(ModelSelection {
            tier,
            persona: normalized_persona,
            provider: profile.provider.clone(),
            model_id: profile.model_id.clone(),
            source: "default".to_string(),
        })
    }

    pub fn candidates_with_supported_providers(
        &self,
        tier: u8,
        persona: Option<&str>,
        supported_providers: &[String],
    ) -> Result<Vec<ModelProfile>, String> {
        if !(1..=3).contains(&tier) {
            return Err("tier must be 1, 2, or 3".to_string());
        }

        let normalized_persona = persona
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.to_ascii_lowercase());
        let supported_set = supported_providers
            .iter()
            .map(|value| normalize_provider(value))
            .collect::<HashSet<_>>();
        let filter_by_adapter = !supported_set.is_empty();

        let mut result = Vec::new();
        let mut seen = HashSet::new();

        if let Some(persona_key) = &normalized_persona {
            if let Some(candidates) = self.config.persona_overrides.get(persona_key) {
                for profile in candidates {
                    let provider = normalize_provider(profile.provider.as_str());
                    if filter_by_adapter && !supported_set.contains(&provider) {
                        continue;
                    }
                    let key = format!(
                        "{}::{}",
                        provider,
                        profile.model_id.trim().to_ascii_lowercase()
                    );
                    if seen.insert(key) {
                        result.push(profile.clone());
                    }
                }
            }
        }

        let tier_key = tier.to_string();
        if let Some(candidates) = self.config.tiers.get(&tier_key) {
            for profile in candidates {
                let provider = normalize_provider(profile.provider.as_str());
                if filter_by_adapter && !supported_set.contains(&provider) {
                    continue;
                }
                let key = format!(
                    "{}::{}",
                    provider,
                    profile.model_id.trim().to_ascii_lowercase()
                );
                if seen.insert(key) {
                    result.push(profile.clone());
                }
            }
        }

        if result.is_empty() {
            let defaults = default_tier_profiles();
            if let Some(candidates) = defaults.get(&tier_key) {
                for profile in candidates {
                    let provider = normalize_provider(profile.provider.as_str());
                    if filter_by_adapter && !supported_set.contains(&provider) {
                        continue;
                    }
                    let key = format!(
                        "{}::{}",
                        provider,
                        profile.model_id.trim().to_ascii_lowercase()
                    );
                    if seen.insert(key) {
                        result.push(profile.clone());
                    }
                }
            }
        }

        Ok(result)
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
        let fallback_candidates = defaults
            .get(&key)
            .cloned()
            .unwrap_or_else(|| vec![default_tier_profile(tier)]);
        let fallback_profile = fallback_candidates
            .first()
            .cloned()
            .unwrap_or_else(|| default_tier_profile(tier));
        let provided_candidates = config.tiers.get(&key).cloned().unwrap_or_default();
        let normalized = normalize_profiles(
            provided_candidates,
            &fallback_profile,
            &default_provider,
            Some(fallback_candidates.clone()),
        );
        tiers.insert(key, normalized);
    }

    let tier3_fallback = tiers
        .get("3")
        .and_then(|profiles| profiles.first())
        .cloned()
        .unwrap_or_else(|| default_tier_profile(3));

    let mut persona_overrides = HashMap::new();
    for (persona, profiles) in config.persona_overrides {
        let persona_key = persona.trim().to_ascii_lowercase();
        if persona_key.is_empty() {
            continue;
        }
        let normalized = normalize_profiles(
            profiles,
            &tier3_fallback,
            &default_provider,
            Some(vec![tier3_fallback.clone()]),
        );
        persona_overrides.insert(persona_key, normalized);
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

fn normalize_profiles(
    profiles: Vec<ModelProfile>,
    fallback: &ModelProfile,
    default_provider_value: &str,
    fallback_candidates: Option<Vec<ModelProfile>>,
) -> Vec<ModelProfile> {
    let mut normalized = Vec::new();
    let mut dedupe = HashSet::new();

    for profile in profiles {
        let normalized_profile = normalize_profile(profile, fallback, default_provider_value);
        let dedupe_key = format!(
            "{}::{}",
            normalize_provider(&normalized_profile.provider),
            normalized_profile.model_id.to_ascii_lowercase()
        );
        if dedupe.insert(dedupe_key) {
            normalized.push(normalized_profile);
        }
    }

    if !normalized.is_empty() {
        return normalized;
    }

    if let Some(candidates) = fallback_candidates {
        for profile in candidates {
            let normalized_profile = normalize_profile(profile, fallback, default_provider_value);
            let dedupe_key = format!(
                "{}::{}",
                normalize_provider(&normalized_profile.provider),
                normalized_profile.model_id.to_ascii_lowercase()
            );
            if dedupe.insert(dedupe_key) {
                normalized.push(normalized_profile);
            }
        }
    }

    if normalized.is_empty() {
        normalized.push(normalize_profile(
            fallback.clone(),
            fallback,
            default_provider_value,
        ));
    }

    normalized
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

fn select_profile<'a>(
    candidates: &'a [ModelProfile],
    supported_set: &HashSet<String>,
    filter_by_adapter: bool,
) -> Option<&'a ModelProfile> {
    if !filter_by_adapter {
        return candidates.first();
    }

    candidates
        .iter()
        .find(|profile| supported_set.contains(&normalize_provider(&profile.provider)))
}

fn normalize_provider(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn default_config() -> ModelRoutingConfig {
    ModelRoutingConfig {
        version: default_version(),
        default_provider: default_provider(),
        tiers: default_tier_profiles(),
        persona_overrides: HashMap::new(),
    }
}

fn default_tier_profiles() -> HashMap<String, Vec<ModelProfile>> {
    HashMap::from([
        ("1".to_string(), vec![default_tier_profile(1)]),
        ("2".to_string(), vec![default_tier_profile(2)]),
        ("3".to_string(), vec![default_tier_profile(3)]),
    ])
}

fn default_tier_profile(_tier: u8) -> ModelProfile {
    ModelProfile {
        provider: default_provider(),
        model_id: "sonnet".to_string(),
        temperature: Some(0.2),
        max_output_tokens: None,
    }
}

fn default_provider() -> String {
    "claude_code".to_string()
}

fn default_version() -> u32 {
    2
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
        assert_eq!(tier1.model_id, "sonnet");
        assert_eq!(tier1.provider, "claude_code");
    }

    #[test]
    fn loads_single_and_multi_candidate_profiles() {
        let temp = tempdir().expect("temp directory should be created");
        let config_path = temp.path().join(CONFIG_FILE_NAME);
        fs::write(
            &config_path,
            r#"{
  "version": 2,
  "defaultProvider": "claude_code",
  "tiers": {
    "1": [
      { "provider": "openai", "modelId": "gpt-5" },
      { "provider": "claude_code", "modelId": "sonnet" }
    ],
    "2": { "provider": "claude_code", "modelId": "sonnet" },
    "3": { "provider": "claude_code", "modelId": "sonnet" }
  },
  "personaOverrides": {
    "security_analyst": [
      { "provider": "openai", "modelId": "o3" },
      { "provider": "claude_code", "modelId": "opus" }
    ]
  }
}"#,
        )
        .expect("config should be written");

        let registry = ModelRegistry::load(temp.path());
        let available = vec!["claude_code".to_string()];
        let security = registry
            .resolve_with_supported_providers(3, Some("security_analyst"), &available)
            .expect("persona override should resolve");
        let tier1 = registry
            .resolve_with_supported_providers(1, None, &available)
            .expect("tier 1 model should resolve");

        assert_eq!(security.source, "persona_override");
        assert_eq!(security.model_id, "opus");
        assert_eq!(tier1.model_id, "sonnet");
        assert_eq!(tier1.provider, "claude_code");
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
        assert_eq!(tier3.model_id, "sonnet");
    }

    #[test]
    fn rejects_invalid_tier_resolution() {
        let registry = ModelRegistry::default();
        let error = registry
            .resolve(4, Some("react_specialist"))
            .expect_err("tier 4 must be rejected");
        assert_eq!(error, "tier must be 1, 2, or 3");
    }

    #[test]
    fn returns_error_if_no_candidate_has_supported_adapter() {
        let registry = ModelRegistry::default();
        let available = vec!["openai".to_string()];
        let error = registry
            .resolve_with_supported_providers(1, None, &available)
            .expect_err("missing supported provider should fail");
        assert!(error.contains("No available provider adapter"));
    }
}
