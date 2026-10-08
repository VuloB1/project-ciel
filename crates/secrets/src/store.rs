use async_trait::async_trait;

use crate::SecretError;

/// A named secret store, keyed by (service, key) — e.g. service="lm_studio",
/// key="api_key", or service="twitch", key="oauth_token". Never logs or
/// returns secrets in a form that could end up in `activity_events` or a
/// diagnostic export; callers are responsible for treating the returned
/// value as sensitive.
#[async_trait]
pub trait SecretStore: Send + Sync {
    async fn set(&self, service: &str, key: &str, value: &str) -> Result<(), SecretError>;
    async fn get(&self, service: &str, key: &str) -> Result<Option<String>, SecretError>;
    async fn delete(&self, service: &str, key: &str) -> Result<(), SecretError>;
}
