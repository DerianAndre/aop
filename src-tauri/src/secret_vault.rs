use std::path::PathBuf;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri_plugin_stronghold::stronghold::Stronghold;
use uuid::Uuid;

pub struct SecretVault {
    app_data_dir: PathBuf,
    stronghold: Option<Stronghold>,
    client_id: Vec<u8>,
    confirmation: Option<(String, i64)>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetProviderSecretStatusInput {
    pub provider: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetProviderSecretInput {
    pub provider: String,
    pub secret: String,
    pub session_token: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RevealProviderSecretInput {
    pub provider: String,
    pub session_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSecretStatus {
    pub provider: String,
    pub configured: bool,
    pub backend: String,
    pub developer_mode: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretOperationResult {
    pub provider: String,
    pub configured: bool,
    pub confirmation_required: bool,
    pub confirmation_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RevealProviderSecretResult {
    pub provider: String,
    pub secret: String,
}

impl SecretVault {
    pub fn new(app_data_dir: PathBuf) -> Self {
        Self {
            app_data_dir,
            stronghold: None,
            client_id: b"aop_provider_secrets".to_vec(),
            confirmation: None,
        }
    }

    fn ensure_stronghold(&mut self) -> Result<(), String> {
        if self.stronghold.is_some() {
            return Ok(());
        }

        let snapshot_path = self.app_data_dir.join("aop_stronghold.hold");
        let password = std::env::var("AOP_STRONGHOLD_PASSWORD")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "aop-dev-stronghold-password".to_string());
        let password_hash = hash_password(password.as_str());

        let stronghold = Stronghold::new(snapshot_path, password_hash)
            .map_err(|error| format!("Failed to initialize Stronghold: {error}"))?;

        if stronghold.get_client(self.client_id.clone()).is_err() {
            stronghold
                .create_client(self.client_id.clone())
                .map_err(|error| format!("Failed to create Stronghold client: {error}"))?;
            stronghold
                .save()
                .map_err(|error| format!("Failed to save Stronghold snapshot: {error}"))?;
        }

        self.stronghold = Some(stronghold);
        Ok(())
    }

    fn stronghold(&self) -> &Stronghold {
        self.stronghold.as_ref().expect("ensure_stronghold must be called first")
    }

    pub fn get_status(
        &mut self,
        provider: &str,
        developer_mode: bool,
    ) -> Result<ProviderSecretStatus, String> {
        let normalized = normalize_provider(provider)?;
        let configured = self.get_secret_bytes(normalized.as_str())?.is_some();
        Ok(ProviderSecretStatus {
            provider: normalized,
            configured,
            backend: "stronghold".to_string(),
            developer_mode,
        })
    }

    pub fn set_secret(
        &mut self,
        provider: &str,
        secret: &str,
        developer_mode: bool,
        session_token: Option<&str>,
    ) -> Result<SecretOperationResult, String> {
        let normalized = normalize_provider(provider)?;
        let trimmed_secret = secret.trim();
        if trimmed_secret.is_empty() {
            return Err("secret is required".to_string());
        }

        let already_configured = self.get_secret_bytes(normalized.as_str())?.is_some();
        if already_configured && !self.confirmed_session(developer_mode, session_token) {
            let token = self.rotate_confirmation_token();
            return Ok(SecretOperationResult {
                provider: normalized,
                configured: true,
                confirmation_required: true,
                confirmation_token: Some(token),
            });
        }

        self.ensure_stronghold()?;
        let client = self.stronghold()
            .get_client(self.client_id.clone())
            .map_err(|error| format!("Failed to access Stronghold client: {error}"))?;
        client
            .store()
            .insert(
                normalized.as_bytes().to_vec(),
                trimmed_secret.as_bytes().to_vec(),
                None,
            )
            .map_err(|error| format!("Failed to persist provider secret: {error}"))?;
        self.stronghold()
            .save()
            .map_err(|error| format!("Failed to save Stronghold snapshot: {error}"))?;

        Ok(SecretOperationResult {
            provider: normalized,
            configured: true,
            confirmation_required: false,
            confirmation_token: None,
        })
    }

    pub fn reveal_secret(
        &mut self,
        provider: &str,
        developer_mode: bool,
        session_token: Option<&str>,
    ) -> Result<RevealProviderSecretResult, String> {
        if !developer_mode {
            return Err(
                "Developer mode is required to reveal provider secrets (AOP_DEV_MODE=1)."
                    .to_string(),
            );
        }

        if !self.confirmed_session(developer_mode, session_token) {
            let token = self.rotate_confirmation_token();
            return Err(format!(
                "CONFIRMATION_REQUIRED: Provide sessionToken to reveal secret. token={token}"
            ));
        }

        let normalized = normalize_provider(provider)?;
        let secret = self
            .get_secret_bytes(normalized.as_str())?
            .ok_or_else(|| format!("No secret configured for provider '{normalized}'"))?;
        let decoded = String::from_utf8(secret)
            .map_err(|error| format!("Stored secret for '{normalized}' is not UTF-8: {error}"))?;

        Ok(RevealProviderSecretResult {
            provider: normalized,
            secret: decoded,
        })
    }

    fn get_secret_bytes(&mut self, provider: &str) -> Result<Option<Vec<u8>>, String> {
        self.ensure_stronghold()?;
        let client = self.stronghold()
            .get_client(self.client_id.clone())
            .map_err(|error| format!("Failed to access Stronghold client: {error}"))?;
        client
            .store()
            .get(provider.as_bytes())
            .map_err(|error| format!("Failed to read provider secret: {error}"))
    }

    fn rotate_confirmation_token(&mut self) -> String {
        let token = Uuid::new_v4().to_string();
        let expires_at = Utc::now().timestamp() + 600;
        self.confirmation = Some((token.clone(), expires_at));
        token
    }

    fn confirmed_session(&self, developer_mode: bool, session_token: Option<&str>) -> bool {
        if !developer_mode {
            return false;
        }
        let Some((token, expires_at)) = &self.confirmation else {
            return false;
        };
        if Utc::now().timestamp() > *expires_at {
            return false;
        }
        let Some(session_token) = session_token else {
            return false;
        };
        session_token.trim() == token.as_str()
    }
}

fn normalize_provider(provider: &str) -> Result<String, String> {
    let normalized = provider.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return Err("provider is required".to_string());
    }
    Ok(normalized)
}

fn hash_password(password: &str) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(password.as_bytes());
    hasher.finalize().to_vec()
}
