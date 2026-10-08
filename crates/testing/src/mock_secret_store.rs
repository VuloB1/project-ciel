use std::collections::HashMap;
use std::sync::Mutex;

use async_trait::async_trait;

use aivt_secrets::{SecretError, SecretStore};

/// In-memory `SecretStore` for tests — never touches the real OS
/// credential store, unlike `KeyringSecretStore` (which deliberately has
/// no automated tests of its own for exactly that reason).
#[derive(Default)]
pub struct MockSecretStore {
    values: Mutex<HashMap<(String, String), String>>,
}

impl MockSecretStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl SecretStore for MockSecretStore {
    async fn set(&self, service: &str, key: &str, value: &str) -> Result<(), SecretError> {
        self.values
            .lock()
            .expect("mock mutex poisoned")
            .insert((service.to_string(), key.to_string()), value.to_string());
        Ok(())
    }

    async fn get(&self, service: &str, key: &str) -> Result<Option<String>, SecretError> {
        Ok(self
            .values
            .lock()
            .expect("mock mutex poisoned")
            .get(&(service.to_string(), key.to_string()))
            .cloned())
    }

    async fn delete(&self, service: &str, key: &str) -> Result<(), SecretError> {
        self.values.lock().expect("mock mutex poisoned").remove(&(service.to_string(), key.to_string()));
        Ok(())
    }
}
