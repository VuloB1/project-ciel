use async_trait::async_trait;
use keyring::Entry;

use crate::{SecretError, SecretStore};

/// `SecretStore` backed by the OS credential store via the `keyring` crate:
/// Windows Credential Manager, macOS Keychain, or the Linux Secret Service
/// (synchronous D-Bus client, since every call already runs on a blocking
/// task — see `sync-secret-service` in the workspace `Cargo.toml`).
///
/// `keyring`'s API is synchronous; every call here runs inside
/// `spawn_blocking` so this trait can stay async like the rest of the app
/// without callers needing to know that detail.
#[derive(Default)]
pub struct KeyringSecretStore;

impl KeyringSecretStore {
    pub fn new() -> Self {
        Self
    }
}

fn map_keyring_error(err: keyring::Error) -> SecretError {
    match err {
        keyring::Error::NoStorageAccess(inner) => SecretError::Unavailable(inner.to_string()),
        other => SecretError::Backend(other.to_string()),
    }
}

fn map_join_error(err: tokio::task::JoinError) -> SecretError {
    SecretError::Task(err.to_string())
}

#[async_trait]
impl SecretStore for KeyringSecretStore {
    async fn set(&self, service: &str, key: &str, value: &str) -> Result<(), SecretError> {
        let service = service.to_owned();
        let key = key.to_owned();
        let value = value.to_owned();

        tokio::task::spawn_blocking(move || {
            let entry = Entry::new(&service, &key).map_err(map_keyring_error)?;
            entry.set_password(&value).map_err(map_keyring_error)
        })
        .await
        .map_err(map_join_error)?
    }

    async fn get(&self, service: &str, key: &str) -> Result<Option<String>, SecretError> {
        let service = service.to_owned();
        let key = key.to_owned();

        tokio::task::spawn_blocking(move || {
            let entry = Entry::new(&service, &key).map_err(map_keyring_error)?;
            match entry.get_password() {
                Ok(value) => Ok(Some(value)),
                Err(keyring::Error::NoEntry) => Ok(None),
                Err(other) => Err(map_keyring_error(other)),
            }
        })
        .await
        .map_err(map_join_error)?
    }

    async fn delete(&self, service: &str, key: &str) -> Result<(), SecretError> {
        let service = service.to_owned();
        let key = key.to_owned();

        tokio::task::spawn_blocking(move || {
            let entry = Entry::new(&service, &key).map_err(map_keyring_error)?;
            match entry.delete_credential() {
                Ok(()) => Ok(()),
                Err(keyring::Error::NoEntry) => Ok(()),
                Err(other) => Err(map_keyring_error(other)),
            }
        })
        .await
        .map_err(map_join_error)?
    }
}

// No automated tests here: writing to the real OS credential store from a
// unit test would pollute the developer's (or CI's) keychain. This gets
// covered by a manual round-trip check during Fase 0 verification instead
// — see docs/DEVELOPMENT_STATUS.md. A future `MockSecretStore` (in
// `aivt-testing`, mirroring the `MockLLMProvider` pattern) will let
// higher-level code depend on `SecretStore` in tests without touching the OS.
