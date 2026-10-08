/// Everything one transcription call needs. `pcm_i16`/`sample_rate` are raw
/// captured audio — WAV-encoding for the upload is the provider's job, not
/// the caller's, mirroring how `TTSProvider::speak` takes raw text rather
/// than a pre-built request body.
#[derive(Debug, Clone)]
pub struct TranscribeRequest {
    pub pcm_i16: Vec<i16>,
    pub sample_rate: u32,
    pub language: Option<String>,
    /// The OpenAI Whisper API's `prompt` field — context text the model
    /// conditions on, used here to bias recognition toward uncommon words
    /// (a character's name) it would otherwise mishear as the closest
    /// ordinary words it knows (`CallService` passes the active profile's
    /// name/nickname — "Ciel" kept transcribing as "si él" without this).
    /// Standard Whisper prompting technique, not something specific to
    /// this app — see OpenAI's own transcription API docs.
    pub prompt: Option<String>,
}
