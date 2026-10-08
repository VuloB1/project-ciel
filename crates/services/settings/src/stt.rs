//! Modelo de transcripción de voz (STT).

use serde::{Deserialize, Serialize};

use crate::service::SettingsService;
use crate::SettingsError;

const KEY_STT_MODEL: &str = "stt_model";

/// Fase 6 (revisado): STT corre embebido en el binario vía whisper.cpp
/// (`aivt_provider_stt::WhisperCppProvider`) — sin servidor externo, sin
/// Docker — así que no queda ningún concepto de conexión/API key aquí, a
/// diferencia de `TtsProviderEntry`/`LlmConnectionSettings`. Lo único
/// que se guarda es qué tamaño de modelo GGML está seleccionado; si ya
/// está descargado o no vive en el propio filesystem
/// (`aivt_provider_stt::SttModelManager::is_downloaded`), no en este
/// servicio. Ver docs/architecture.md §6 para el porqué de este cambio
/// (venderlo como producto sin pedirle Docker al cliente final).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SttModelSize {
    Tiny,
    Base,
    Small,
    Medium,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredSttModel {
    size: SttModelSize,
    /// Whether to load the model with Vulkan GPU acceleration
    /// (`aivt_provider_stt`'s `vulkan` build feature) instead of CPU-only.
    /// Off by default — CPU is the one behavior guaranteed identical on
    /// every customer machine regardless of GPU vendor (see
    /// docs/architecture.md §6); Vulkan is an opt-in speed boost for
    /// whoever's watching this and has a compatible GPU, with an automatic
    /// fallback to CPU if none is found at load time. `#[serde(default)]`
    /// so settings saved before this field existed still load.
    #[serde(default)]
    gpu_enabled: bool,
}

impl Default for StoredSttModel {
    fn default() -> Self {
        // Mismo tamaño que el usuario ya usaba con éxito vía
        // `Systran/faster-whisper-small` en el servidor HTTP anterior.
        Self { size: SttModelSize::Small, gpu_enabled: false }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SttModelSettings {
    pub size: SttModelSize,
    pub gpu_enabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SttModelInput {
    pub size: SttModelSize,
    pub gpu_enabled: bool,
}

impl SettingsService {
    /// Unlike `llm_connection`, this never returns `None` — an empty
    pub async fn stt_model(&self) -> Result<SttModelSettings, SettingsError> {
        let stored: StoredSttModel = self.get_json(KEY_STT_MODEL).await?.unwrap_or_default();
        Ok(SttModelSettings { size: stored.size, gpu_enabled: stored.gpu_enabled })
    }

    pub async fn set_stt_model(&self, input: SttModelInput) -> Result<SttModelSettings, SettingsError> {
        let stored = StoredSttModel { size: input.size, gpu_enabled: input.gpu_enabled };
        self.set_json(KEY_STT_MODEL, &stored).await?;
        Ok(SttModelSettings { size: stored.size, gpu_enabled: stored.gpu_enabled })
    }
}

#[cfg(test)]
mod tests {
    use aivt_database::AppSettingsRepository;
    

    use super::*;
    use crate::service::test_service;

    #[tokio::test]
    async fn stt_model_defaults_to_small_with_gpu_off() {
        let service = test_service().await;
        let model = service.stt_model().await.unwrap();
        assert_eq!(model.size, SttModelSize::Small);
        assert!(!model.gpu_enabled);
    }

    #[tokio::test]
    async fn set_stt_model_round_trips() {
        let service = test_service().await;

        let saved = service.set_stt_model(SttModelInput { size: SttModelSize::Base, gpu_enabled: true }).await.unwrap();
        assert_eq!(saved.size, SttModelSize::Base);
        assert!(saved.gpu_enabled);

        let reloaded = service.stt_model().await.unwrap();
        assert_eq!(reloaded.size, SttModelSize::Base);
        assert!(reloaded.gpu_enabled);
    }

    #[tokio::test]
    async fn an_stt_model_saved_before_gpu_enabled_existed_still_loads() {
        let service = test_service().await;
        AppSettingsRepository::new(&service.pool).set(KEY_STT_MODEL, r#"{"size":"base"}"#).await.unwrap();

        let model = service.stt_model().await.unwrap();
        assert_eq!(model.size, SttModelSize::Base);
        assert!(!model.gpu_enabled);
    }
}
