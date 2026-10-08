//! Abstraction over OS-native credential storage.
//!
//! API keys, OAuth tokens, and bot tokens are never written to SQLite or to
//! disk in plain text — see docs/security.md. `providers`/`twitch`/`discord`
//! store only a `secret_ref` pointing at an entry here.

mod error;
mod keyring_store;
mod store;

pub use error::SecretError;
pub use keyring_store::KeyringSecretStore;
pub use store::SecretStore;
