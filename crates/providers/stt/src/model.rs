use serde::{Deserialize, Serialize};

/// Which GGML Whisper model the embedded engine loads. Only multilingual
/// variants (no `.en` suffix) — this app needs Spanish, not just English.
/// Larger sizes trade download time and per-utterance latency for
/// transcription accuracy; see `SttModelSize::approx_size_mb` for the
/// numbers shown in Settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WhisperModelSize {
    Tiny,
    Base,
    Small,
    Medium,
}

impl WhisperModelSize {
    pub const ALL: [WhisperModelSize; 4] =
        [WhisperModelSize::Tiny, WhisperModelSize::Base, WhisperModelSize::Small, WhisperModelSize::Medium];

    /// The exact GGML file name `ggerganov/whisper.cpp` publishes on
    /// Hugging Face — also used as the local file name once downloaded.
    pub fn file_name(self) -> &'static str {
        match self {
            WhisperModelSize::Tiny => "ggml-tiny.bin",
            WhisperModelSize::Base => "ggml-base.bin",
            WhisperModelSize::Small => "ggml-small.bin",
            WhisperModelSize::Medium => "ggml-medium.bin",
        }
    }

    pub fn download_url(self) -> String {
        format!("https://huggingface.co/ggerganov/whisper.cpp/resolve/main/{}", self.file_name())
    }

    /// Rounded — real file sizes vary slightly by a few hundred KB. Shown
    /// in Settings so downloading a model isn't a surprise about how much
    /// it'll cost in bandwidth/disk.
    pub fn approx_size_mb(self) -> u32 {
        match self {
            WhisperModelSize::Tiny => 75,
            WhisperModelSize::Base => 142,
            WhisperModelSize::Small => 466,
            WhisperModelSize::Medium => 1500,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            WhisperModelSize::Tiny => "tiny",
            WhisperModelSize::Base => "base",
            WhisperModelSize::Small => "small",
            WhisperModelSize::Medium => "medium",
        }
    }
}
