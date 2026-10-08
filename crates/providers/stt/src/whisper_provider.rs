use std::sync::Arc;

use async_trait::async_trait;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext};

use crate::error::SttError;
use crate::types::TranscribeRequest;
use crate::STTProvider;

/// How many CPU threads whisper.cpp uses per transcription. Fixed rather
/// than "all cores": this runs on the same machine as everything else in
/// the app (capture thread, segmenter thread, the whole Tokio runtime) and
/// call latency already has an end-to-end target to hit — starving the
/// rest of the app for one point of marginal speedup isn't worth it.
const TRANSCRIBE_THREADS: std::ffi::c_int = 4;

/// Wraps an already-loaded `WhisperContext` — cheap to construct, unlike
/// the model load itself (see `SttModelManager::ensure_loaded`, which owns
/// caching). `WhisperContext` is `Send + Sync`, so this can be built fresh
/// per call from a shared `Arc` without any locking.
pub struct WhisperCppProvider {
    context: Arc<WhisperContext>,
}

impl WhisperCppProvider {
    pub fn new(context: Arc<WhisperContext>) -> Self {
        Self { context }
    }
}

#[async_trait]
impl STTProvider for WhisperCppProvider {
    async fn transcribe(&self, request: TranscribeRequest) -> Result<String, SttError> {
        let context = self.context.clone();

        tokio::task::spawn_blocking(move || {
            let audio = pcm_i16_to_f32(&request.pcm_i16);

            let mut state = context.create_state().map_err(|err| SttError::ModelLoad(err.to_string()))?;

            // Greedy — latency-first. Beam search would search for a
            // marginally better transcript at a real time cost, and
            // end-to-end call latency is already this project's hardest
            // technical problem (docs/architecture.md §6).
            let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
            params.set_n_threads(TRANSCRIBE_THREADS);
            params.set_print_progress(false);
            params.set_print_special(false);
            params.set_print_realtime(false);
            params.set_print_timestamps(false);
            if let Some(language) = request.language.as_deref() {
                params.set_language(Some(language));
            }
            if let Some(prompt) = request.prompt.as_deref().filter(|p| !p.trim().is_empty()) {
                // Whisper.cpp's initial-prompt conditioning — same
                // technique/purpose as the old HTTP path's `prompt` field:
                // biases recognition toward uncommon words (a character's
                // name) it would otherwise mishear as the closest ordinary
                // words it knows. See `CallService::transcription_prompt`.
                params.set_initial_prompt(prompt);
            }

            state.full(params, &audio).map_err(|err| SttError::Transcription(err.to_string()))?;

            let text: String = state.as_iter().map(|segment| segment.to_string()).collect();
            Ok(text.trim().to_string())
        })
        .await
        .map_err(|err| SttError::Transcription(format!("tarea de transcripción cancelada: {err}")))?
    }
}

/// whisper.cpp expects `f32` samples in `[-1.0, 1.0]` — capture/the
/// segmenter already hand over 16kHz mono `i16` (see
/// `crates/services/call/src/capture.rs`), so this is the only conversion
/// needed on the hot path.
fn pcm_i16_to_f32(pcm: &[i16]) -> Vec<f32> {
    pcm.iter().map(|&sample| sample as f32 / 32768.0).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pcm_i16_to_f32_maps_the_full_range_into_minus_one_to_one() {
        let samples = pcm_i16_to_f32(&[0, i16::MAX, i16::MIN]);
        assert_eq!(samples[0], 0.0);
        assert!((samples[1] - 0.999969).abs() < 1e-5);
        assert_eq!(samples[2], -1.0);
    }

    #[test]
    fn pcm_i16_to_f32_handles_empty_input() {
        assert!(pcm_i16_to_f32(&[]).is_empty());
    }
}
