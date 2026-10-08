use std::sync::Mutex;

use async_trait::async_trait;

use aivt_provider_tts::{SpeakRequest, TtsBackend, TtsError, VoiceInfo, TTSProvider};

/// In-memory `TTSProvider` — records every `speak` call so tests can assert
/// *whether* and *with what text* the auto-play hook fired, without needing
/// a real OS speech engine or HTTP server.
#[derive(Default)]
pub struct MockTTSProvider {
    speak_calls: Mutex<Vec<SpeakRequest>>,
    stop_calls: Mutex<u32>,
    voices: Mutex<Vec<VoiceInfo>>,
    /// What `speak_and_capture` hands back — `None` mimics the `Native`
    /// backend (nothing to capture), `Some(bytes)` mimics `Http`/
    /// `ElevenLabs` for tests exercising Chat's audio-persistence path.
    captured_audio: Mutex<Option<Vec<u8>>>,
}

impl MockTTSProvider {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_voices(voices: Vec<VoiceInfo>) -> Self {
        Self { voices: Mutex::new(voices), ..Default::default() }
    }

    pub fn with_captured_audio(bytes: Vec<u8>) -> Self {
        Self { captured_audio: Mutex::new(Some(bytes)), ..Default::default() }
    }

    pub fn speak_calls(&self) -> Vec<SpeakRequest> {
        self.speak_calls.lock().expect("mock mutex poisoned").iter().cloned().collect()
    }

    pub fn stop_call_count(&self) -> u32 {
        *self.stop_calls.lock().expect("mock mutex poisoned")
    }
}

#[async_trait]
impl TTSProvider for MockTTSProvider {
    async fn list_voices(&self, _backend: &TtsBackend) -> Result<Vec<VoiceInfo>, TtsError> {
        Ok(self.voices.lock().expect("mock mutex poisoned").clone())
    }

    async fn speak_and_capture(&self, request: SpeakRequest) -> Result<Option<Vec<u8>>, TtsError> {
        self.speak_calls.lock().expect("mock mutex poisoned").push(request);
        Ok(self.captured_audio.lock().expect("mock mutex poisoned").clone())
    }

    async fn stop(&self) -> Result<(), TtsError> {
        *self.stop_calls.lock().expect("mock mutex poisoned") += 1;
        Ok(())
    }
}
