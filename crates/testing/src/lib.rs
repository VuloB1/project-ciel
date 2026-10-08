//! Shared mock providers (sección 111 del brief) so higher-level crates
//! (`orchestrator`, `services/chat`, `services/settings`) and their tests
//! never need a real LM Studio instance or the OS keyring.

mod mock_llm;
mod mock_memory;
mod mock_secret_store;
mod mock_stt;
mod mock_tts;

pub use mock_llm::{text_response, tool_call_response, MockLLMProvider};
pub use mock_memory::{ExtractCall, MockMemoryManager};
pub use mock_secret_store::MockSecretStore;
pub use mock_stt::MockSTTProvider;
pub use mock_tts::MockTTSProvider;
