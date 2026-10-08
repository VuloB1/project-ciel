# ADR-003: Arquitectura de providers intercambiables

## Estado

Aceptado e implementado. Este ADR fijó el contrato antes de la primera
implementación (Fase 2) y se mantuvo. Actualización (2026-10): los traits son
los de este documento, hay adaptadores reales para LLM (OpenAI-compatible,
Anthropic, Gemini), voz (nativo, ElevenLabs, Google, Azure, Fish Audio, Deepgram
y OpenAI-compatible), transcripción (whisper.cpp), visión y embeddings, y la
app admite **varios proveedores a la vez** con cambio automático para IA — ver
[ADR-010](010-multi-provider-and-failover.md).

## Contexto

La app debe soportar múltiples fuentes de IA (LM Studio local, servidores
OpenAI-compatible genéricos, y opcionalmente proveedores cloud oficiales)
para texto, voz (TTS/STT), visión y generación de imágenes — sin que ningún
componente de UI o de canal (Chat/Twitch/Discord/Call) dependa directamente
de un proveedor concreto, y sin asumir que todos los modelos soportan todas
las funciones.

## Decisión

Cada capacidad de IA se modela como un **trait de Rust** con implementaciones
concretas intercambiables:

- `LLMProvider`, `STTProvider`, `TTSProvider`, `EmbeddingProvider`,
  `VisionProvider`, `ImageGenerationProvider`, `AvatarProvider`.
- Cada `LLMProvider` expone `capabilities(model) -> ModelCapabilities` con
  flags explícitos (`text`, `vision`, `audio_input`, `audio_output`,
  `tool_calling`, `streaming`, `embeddings`, `image_generation`,
  `json_mode`) — la UI nunca asume que un modelo soporta una función; la
  consulta y reacciona (sección 84-85 del brief: "capability-driven UI").
- `MockLLMProvider`/`MockTTSProvider`/`MockSTTProvider` (crate `testing`,
  Fase 2 en adelante) implementan los mismos traits para poder probar UI y
  orchestrator sin servicios externos.
- Ningún secreto vive en la configuración del provider en SQLite — solo un
  `secret_ref` hacia `SecretStore` (ver [security.md](../security.md)).

## Razones

- Permite añadir LM Studio, un OpenAI-compatible genérico, y proveedores
  cloud opcionales sin tocar `AIOrchestrator` ni ningún componente de canal.
- El trait de capacidades hace explícito, en el tipo, lo que el brief pide
  como comportamiento: "no asumir que todos los modelos soportan todas las
  funciones" — un botón de imagen deshabilitado no es una decisión de UI
  aislada, es una consecuencia directa de consultar `ModelCapabilities`.

## Consecuencias

- Cada provider nuevo implica escribir un adaptador, no solo una llamada
  HTTP suelta — más código por integración, a cambio de que
  `AIOrchestrator` y la UI nunca necesiten saber qué proveedor está activo.
- `crates/providers/*` existieron como crates vacíos desde Fase 0
  (`aivt-provider-llm`, `-stt`, `-tts`, `-vision`, `-embedding`, `-avatar`)
  para que la estructura del workspace ya reflejara esta decisión; hoy todos
  tienen contenido real (más `-tools`).
- La capa importa: `aivt-provider-llm` no depende de eventbus ni de settings.
  Cuando un proveedor necesita avisar algo hacia arriba (p. ej. un cambio de
  proveedor), lo hace por un canal genérico y es el servicio de settings quien lo
  convierte en un `AppEvent`.
