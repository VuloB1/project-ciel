use std::sync::Mutex;

use async_trait::async_trait;

use aivt_provider_stt::{SttError, TranscribeRequest, STTProvider};

/// In-memory `STTProvider` — records every `transcribe` call and returns a
/// canned transcript, so `CallService` tests can assert the pipeline reacts
/// correctly without a real STT server.
pub struct MockSTTProvider {
    transcribe_calls: Mutex<Vec<TranscribeRequest>>,
    response: Mutex<Result<String, String>>,
}

impl MockSTTProvider {
    pub fn with_response(text: impl Into<String>) -> Self {
        Self { transcribe_calls: Mutex::new(Vec::new()), response: Mutex::new(Ok(text.into())) }
    }

    pub fn with_error(message: impl Into<String>) -> Self {
        Self { transcribe_calls: Mutex::new(Vec::new()), response: Mutex::new(Err(message.into())) }
    }

    pub fn transcribe_calls(&self) -> Vec<TranscribeRequest> {
        self.transcribe_calls.lock().expect("mock mutex poisoned").iter().cloned().collect()
    }
}

#[async_trait]
impl STTProvider for MockSTTProvider {
    async fn transcribe(&self, request: TranscribeRequest) -> Result<String, SttError> {
        self.transcribe_calls.lock().expect("mock mutex poisoned").push(request);
        match &*self.response.lock().expect("mock mutex poisoned") {
            Ok(text) => Ok(text.clone()),
            Err(message) => Err(SttError::Server(message.clone())),
        }
    }
}
