//! Secure credential storage using OS keyring.
//!
//! This module provides a provider-agnostic interface for storing and retrieving
//! secrets (tokens, API keys) in the operating system's secure credential store.
//!
//! Secrets are stored with a service name of "tunez" and user-specific keys that
//! include provider, profile, and secret type information.

use thiserror::Error;

/// Service name used for all Tunez credentials in the OS keyring.
const SERVICE_NAME: &str = "tunez";

/// Errors that can occur when accessing the credential store.
#[derive(Debug, Error)]
pub enum SecretsError {
    #[error("credential not found: {key}")]
    NotFound { key: String },

    #[error("keyring access denied: {0}")]
    AccessDenied(String),

    #[error("keyring unavailable: {0}")]
    Unavailable(String),

    #[error("keyring error: {0}")]
    Other(String),
}

impl From<keyring::Error> for SecretsError {
    fn from(err: keyring::Error) -> Self {
        match err {
            keyring::Error::NoEntry => SecretsError::NotFound {
                key: "unknown".into(),
            },
            keyring::Error::NoStorageAccess(e) => SecretsError::AccessDenied(e.to_string()),
            keyring::Error::PlatformFailure(e) => SecretsError::Unavailable(e.to_string()),
            other => SecretsError::Other(other.to_string()),
        }
    }
}

pub type SecretsResult<T> = Result<T, SecretsError>;

/// Key types for different kinds of secrets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretKind {
    /// OAuth/API access token
    AccessToken,
    /// OAuth refresh token
    RefreshToken,
    /// API key
    ApiKey,
}

impl SecretKind {
    fn as_str(&self) -> &'static str {
        match self {
            SecretKind::AccessToken => "access_token",
            SecretKind::RefreshToken => "refresh_token",
            SecretKind::ApiKey => "api_key",
        }
    }
}

/// Credential store backed by the OS keyring.
///
/// Provides secure storage for provider credentials without exposing
/// them in config files or logs.
#[derive(Debug, Clone)]
pub struct CredentialStore {
    service: String,
}

impl Default for CredentialStore {
    fn default() -> Self {
        Self::new()
    }
}

impl CredentialStore {
    /// Create a new credential store using the default service name.
    pub fn new() -> Self {
        Self {
            service: SERVICE_NAME.into(),
        }
    }

    /// Build the keyring user key for a given provider, profile, and secret kind.
    fn build_key(provider: &str, profile: Option<&str>, kind: SecretKind) -> String {
        match profile {
            Some(p) => format!("{}/{}/{}", provider, p, kind.as_str()),
            None => format!("{}/{}", provider, kind.as_str()),
        }
    }

    /// Store a secret in the keyring.
    ///
    /// # Arguments
    /// * `provider` - Provider ID (e.g., "melodee", "filesystem")
    /// * `profile` - Optional profile name (e.g., "home", "work")
    /// * `kind` - The type of secret being stored
    /// * `secret` - The secret value (token, API key, etc.)
    pub fn store(
        &self,
        provider: &str,
        profile: Option<&str>,
        kind: SecretKind,
        secret: &str,
    ) -> SecretsResult<()> {
        let key = Self::build_key(provider, profile, kind);
        let entry = keyring::Entry::new(&self.service, &key)?;
        entry.set_password(secret)?;
        tracing::debug!(provider = provider, kind = ?kind, "stored credential in keyring");
        Ok(())
    }

    /// Retrieve a secret from the keyring.
    ///
    /// Returns `SecretsError::NotFound` if the secret doesn't exist.
    pub fn get(
        &self,
        provider: &str,
        profile: Option<&str>,
        kind: SecretKind,
    ) -> SecretsResult<String> {
        let key = Self::build_key(provider, profile, kind);
        let entry = keyring::Entry::new(&self.service, &key)?;
        match entry.get_password() {
            Ok(secret) => Ok(secret),
            Err(keyring::Error::NoEntry) => Err(SecretsError::NotFound { key }),
            Err(e) => Err(e.into()),
        }
    }

    /// Delete a secret from the keyring.
    ///
    /// Returns `Ok(())` even if the secret didn't exist.
    pub fn delete(
        &self,
        provider: &str,
        profile: Option<&str>,
        kind: SecretKind,
    ) -> SecretsResult<()> {
        let key = Self::build_key(provider, profile, kind);
        let entry = keyring::Entry::new(&self.service, &key)?;
        match entry.delete_credential() {
            Ok(()) => {
                tracing::debug!(provider = provider, kind = ?kind, "deleted credential from keyring");
                Ok(())
            }
            Err(keyring::Error::NoEntry) => Ok(()), // Already gone, not an error
            Err(e) => Err(e.into()),
        }
    }

    /// Check if a secret exists in the keyring.
    pub fn exists(
        &self,
        provider: &str,
        profile: Option<&str>,
        kind: SecretKind,
    ) -> SecretsResult<bool> {
        match self.get(provider, profile, kind) {
            Ok(_) => Ok(true),
            Err(SecretsError::NotFound { .. }) => Ok(false),
            Err(e) => Err(e),
        }
    }

    /// Store an access token for a provider.
    pub fn store_access_token(
        &self,
        provider: &str,
        profile: Option<&str>,
        token: &str,
    ) -> SecretsResult<()> {
        self.store(provider, profile, SecretKind::AccessToken, token)
    }

    /// Get an access token for a provider.
    pub fn get_access_token(&self, provider: &str, profile: Option<&str>) -> SecretsResult<String> {
        self.get(provider, profile, SecretKind::AccessToken)
    }

    /// Store a refresh token for a provider.
    pub fn store_refresh_token(
        &self,
        provider: &str,
        profile: Option<&str>,
        token: &str,
    ) -> SecretsResult<()> {
        self.store(provider, profile, SecretKind::RefreshToken, token)
    }

    /// Get a refresh token for a provider.
    pub fn get_refresh_token(
        &self,
        provider: &str,
        profile: Option<&str>,
    ) -> SecretsResult<String> {
        self.get(provider, profile, SecretKind::RefreshToken)
    }

    /// Store an API key for a provider.
    pub fn store_api_key(
        &self,
        provider: &str,
        profile: Option<&str>,
        api_key: &str,
    ) -> SecretsResult<()> {
        self.store(provider, profile, SecretKind::ApiKey, api_key)
    }

    /// Get an API key for a provider.
    pub fn get_api_key(&self, provider: &str, profile: Option<&str>) -> SecretsResult<String> {
        self.get(provider, profile, SecretKind::ApiKey)
    }

    /// Clear all credentials for a specific provider and profile.
    pub fn clear_provider(&self, provider: &str, profile: Option<&str>) -> SecretsResult<()> {
        // Try to delete all known secret types; ignore NotFound errors
        let _ = self.delete(provider, profile, SecretKind::AccessToken);
        let _ = self.delete(provider, profile, SecretKind::RefreshToken);
        let _ = self.delete(provider, profile, SecretKind::ApiKey);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: These tests require an actual keyring to be available.
    // On CI or headless systems, they may be skipped or fail gracefully.

    #[test]
    fn key_building() {
        let key = CredentialStore::build_key("melodee", Some("home"), SecretKind::AccessToken);
        assert_eq!(key, "melodee/home/access_token");

        let key = CredentialStore::build_key("filesystem", None, SecretKind::ApiKey);
        assert_eq!(key, "filesystem/api_key");
    }

    #[test]
    fn secret_kind_as_str() {
        assert_eq!(SecretKind::AccessToken.as_str(), "access_token");
        assert_eq!(SecretKind::RefreshToken.as_str(), "refresh_token");
        assert_eq!(SecretKind::ApiKey.as_str(), "api_key");
    }
}
