use async_trait::async_trait;
use candle_core::{DType, Tensor};

use crate::error::EmbeddingError;
use crate::model_manager::{EmbeddingModelManager, LoadedEmbeddingModel};
use crate::EmbeddingProvider;

/// The one production `EmbeddingProvider`. Wraps `EmbeddingModelManager`
/// directly and calls `ensure_loaded()` on every `embed()` (a cheap
/// cache-hit after the first real load) rather than taking an
/// already-resolved model the way `WhisperCppProvider` takes an
/// already-resolved `WhisperContext` — there's no per-call configuration
/// to resolve first (no size/GPU choice, just one model), so there's
/// nothing for a caller to resolve on its behalf.
pub struct LocalEmbeddingProvider {
    manager: EmbeddingModelManager,
}

impl LocalEmbeddingProvider {
    pub fn new(manager: EmbeddingModelManager) -> Self {
        Self { manager }
    }
}

#[async_trait]
impl EmbeddingProvider for LocalEmbeddingProvider {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        let loaded = self.manager.ensure_loaded().await?;
        let text = text.to_string();
        tokio::task::spawn_blocking(move || embed_blocking(&loaded, &text))
            .await
            .map_err(|err| EmbeddingError::Inference(format!("tarea de embedding cancelada: {err}")))?
    }
}

/// Tokenize -> BERT forward pass -> mean-pool over real tokens (using the
/// attention mask, so padding never dilutes the average) -> L2-normalize.
/// Follows candle's own reference sentence-embedding example, not a
/// hand-invented recipe — see `docs/ADR/005-local-embedding-search.md`
/// once Etapa 3 writes it.
fn embed_blocking(loaded: &LoadedEmbeddingModel, text: &str) -> Result<Vec<f32>, EmbeddingError> {
    let encoding = loaded.tokenizer.encode(text, true).map_err(|err| EmbeddingError::Inference(err.to_string()))?;

    let to_tensor = |ids: &[u32]| -> candle_core::Result<Tensor> { Tensor::new(ids, &loaded.device)?.unsqueeze(0) };

    let ids = to_tensor(encoding.get_ids()).map_err(|err| EmbeddingError::Inference(err.to_string()))?;
    let type_ids = to_tensor(encoding.get_type_ids()).map_err(|err| EmbeddingError::Inference(err.to_string()))?;
    let mask = to_tensor(encoding.get_attention_mask()).map_err(|err| EmbeddingError::Inference(err.to_string()))?;

    let hidden =
        loaded.model.forward(&ids, &type_ids, Some(&mask)).map_err(|err| EmbeddingError::Inference(err.to_string()))?;

    let mean = mean_pool(&hidden, &mask).map_err(|err| EmbeddingError::Inference(err.to_string()))?;
    let mut vector: Vec<f32> =
        mean.squeeze(0).and_then(|t| t.to_vec1()).map_err(|err| EmbeddingError::Inference(err.to_string()))?;

    l2_normalize(&mut vector);
    Ok(vector)
}

/// Masked mean pooling: padding tokens (mask == 0) contribute nothing to
/// either the sum or the count, so a short sentence in a padded batch
/// isn't diluted by its own padding. `hidden` is `[1, seq, hidden_size]`,
/// `mask` is `[1, seq]`.
fn mean_pool(hidden: &Tensor, mask: &Tensor) -> candle_core::Result<Tensor> {
    let mask = mask.to_dtype(DType::F32)?.unsqueeze(2)?.broadcast_as(hidden.shape())?;
    let summed = (hidden * &mask)?.sum(1)?;
    let counts = mask.sum(1)?;
    summed.broadcast_div(&counts)
}

/// Plain-`[f32]` L2 normalization, deliberately kept out of the candle
/// `Tensor` pipeline so it has a fast, model-free unit test. Vectors
/// stored/compared by this app are always the output of this function —
/// unit length, so cosine similarity between two of them is just their
/// dot product (see `aivt_database::MemoriesRepository::search_by_embedding`
/// once Etapa 2 lands).
fn l2_normalize(v: &mut [f32]) {
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in v.iter_mut() {
            *x /= norm;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l2_normalize_scales_a_vector_to_unit_length() {
        let mut v = [3.0, 4.0];
        l2_normalize(&mut v);
        assert!((v[0] - 0.6).abs() < 1e-6);
        assert!((v[1] - 0.8).abs() < 1e-6);
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-6);
    }

    #[test]
    fn l2_normalize_leaves_a_zero_vector_alone() {
        let mut v = [0.0, 0.0];
        l2_normalize(&mut v);
        assert_eq!(v, [0.0, 0.0]);
    }
}
