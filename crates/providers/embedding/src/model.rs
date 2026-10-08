/// The one embedding model this app ships — multilingual (Spanish
/// conversations, not just English), a plain BERT architecture
/// `candle_transformers::models::bert` can load directly, and mean-pooling
/// per its own `1_Pooling/config.json`. Unlike `WhisperModelSize`, there's
/// no size/quality tradeoff to expose in Settings: this is the only option.
pub const MODEL_REPO: &str = "sentence-transformers/paraphrase-multilingual-MiniLM-L12-v2";

pub const CONFIG_FILE: &str = "config.json";
pub const TOKENIZER_FILE: &str = "tokenizer.json";
pub const WEIGHTS_FILE: &str = "model.safetensors";

/// Output vector length — every stored/compared embedding is exactly this
/// long (see `aivt_database::MemoriesRepository::search_by_embedding`'s
/// dimension check once Etapa 2 lands).
pub const EMBEDDING_DIMENSION: usize = 384;
/// Matches the model's own `sentence_bert_config.json` (`max_seq_length:
/// 128`) — longer inputs get truncated rather than erroring, same
/// "best-effort over hard-fail" stance as the rest of this provider.
pub const MAX_SEQUENCE_LENGTH: usize = 128;
/// Rounded — shown in Settings before the user commits to downloading, same
/// purpose as `WhisperModelSize::approx_size_mb`. The real
/// `model.safetensors` is 470,641,600 bytes; `config.json`/`tokenizer.json`
/// together add a few more MB, not worth reflecting separately here.
pub const APPROX_SIZE_MB: u32 = 470;
/// Written to `memory_embeddings.model_version` once Etapa 2 lands — lets a
/// future model swap detect and re-embed stale rows without a schema
/// migration. Bump this alongside changing `MODEL_REPO`.
pub const MODEL_VERSION: &str = "paraphrase-multilingual-MiniLM-L12-v2-v1";

/// Direct HTTPS download from Hugging Face, no auth — same shape as
/// `WhisperModelSize::download_url`.
pub fn download_url(file_name: &str) -> String {
    format!("https://huggingface.co/{MODEL_REPO}/resolve/main/{file_name}")
}
