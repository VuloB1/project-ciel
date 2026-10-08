# ADR-016: `SettingsService` dividido por área

## Estado

Aceptado e implementado (2026-10). Cambio de organización sin efecto visible: la API
pública del crate, los formatos guardados en `app_settings` y el comportamiento son los
mismos. Las 90 pruebas del crate y las 638 del workspace pasan igual que antes.

## Contexto

`crates/services/settings/src/service.rs` había crecido hasta unas 3.200 líneas (~145 KB):
los tipos y métodos de LLM, voz, Twitch, Discord, Llamada, búsqueda web, apariencia y el
resto, más sus pruebas, en un solo archivo. Cada cambio obligaba a navegar un archivo
enorme y casi todo choca en él (conflictos, revisión difícil).

## Decisión

Sigue siendo **un solo `SettingsService`** (una estructura, `Clone`, con el pool y el
almacén de secretos); lo que cambia es dónde vive el código. Cada archivo agrega su propio
bloque `impl SettingsService` junto a los tipos que maneja:

| Archivo | Contenido |
|---|---|
| `service.rs` | la estructura, `get_json`/`set_json`, idioma, avatar del usuario, `test_service()` para pruebas |
| `llm.rs` | proveedores de IA, orden y pausa de la rotación automática, migración de la conexión única |
| `tts.rs` | proveedores de voz y migración |
| `twitch.rs`, `discord.rs` | conexión, tokens, política (y emotes en Twitch) |
| `call.rs` | política de la Llamada |
| `web_search.rs`, `stt.rs` | búsqueda web y modelo de transcripción |
| `preferences.rs` | apariencia, segundo plano, audio del chat, dictado, personas, avatar para OBS, silencio de voz, onboarding |

- Las pruebas viajan con su área (`mod tests` en cada archivo); el ayudante común es
  `service::test_service()`.
- `lib.rs` reexporta los mismos nombres de siempre: ningún otro crate cambió.
- Lo que antes era privado del archivo y ahora se comparte entre áreas es mínimo y
  `pub(crate)`: los campos `pool`/`secrets`, `SECRET_KEY_API_KEY` y `OPENAI_BASE_URL`
  (la voz "OpenAI" reutiliza la URL del LLM).

## Alternativas descartadas

- **Varios servicios separados** (`LlmSettings`, `TtsSettings`...): obligaba a cambiar
  `AppState` y los ~30 sitios que reciben `SettingsService`, y a repartir el pool y los
  secretos, para un beneficio que el cambio de archivos ya da.
- **Un crate por área:** más fricción de dependencias (la voz usa constantes del LLM) sin
  necesidad.

## Consecuencias

- Una área nueva se añade como archivo nuevo en vez de crecer uno existente.
- El archivo más grande sigue siendo `llm.rs` (~1.100 líneas, la mitad pruebas): es el
  área con más reglas (rotación, pausas, migración). Si crece más, separar `llm/failover.rs`.
