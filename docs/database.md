# Base de datos

SQLite en modo WAL, con claves foráneas activas. Acceso vía `sqlx` (sin ORM
completo — ver [ADR-002](ADR/002-sqlite-storage.md)), patrón Repository por
tabla en `crates/database/src/repositories/`.

## Migraciones

- Viven en `crates/database/migrations/`, aplicadas por `sqlx::migrate!` en
  `aivt_database::migrate(pool)`, que se ejecuta en cada arranque de la app
  (idempotente — sqlx registra las versiones ya aplicadas).
- **Nunca** se edita una migración ya publicada. Un cambio de esquema es
  siempre un archivo nuevo, numerado secuencialmente
  (`000N_descripcion.sql`).
- Cada tabla se crea en la migración de la fase que realmente la necesita —
  no se crean tablas por adelantado "por si acaso" (evita esquema muerto).

## Tablas

### `app_settings` (`0001_foundation.sql`)

Almacén clave/JSON plano para preferencias de la app (ver "Claves de `app_settings`" más abajo).
Permite añadir preferencias nuevas sin migración mientras no necesiten su
propia tabla relacional.

| Columna | Tipo | Notas |
|---|---|---|
| `key` | TEXT PK | |
| `value` | TEXT | JSON-encoded |
| `updated_at` | TEXT | RFC 3339 |

### `activity_events` (`0002_activity.sql`)

Backing de la pestaña Actividad y del pipe de `tracing` (destino `activity`). Ver
`crates/services/activity`.

| Columna | Tipo | Notas |
|---|---|---|
| `id` | TEXT PK | uuid v4 |
| `timestamp` | TEXT | RFC 3339 |
| `level` | TEXT | DEBUG\|INFO\|WARNING\|ERROR\|CRITICAL |
| `category` | TEXT | CHAT\|LLM\|STT\|TTS\|MEMORY\|TWITCH\|DISCORD\|AVATAR\|CALL\|SYSTEM\|DATABASE\|NETWORK |
| `module` | TEXT | |
| `action` | TEXT | |
| `profile_id` | TEXT | nullable |
| `conversation_id` | TEXT | nullable |
| `request_id` | TEXT | nullable |
| `duration_ms` | INTEGER | nullable |
| `status` | TEXT | |
| `summary` | TEXT | |
| `details_json` | TEXT | nullable |

Índices: `timestamp`, `category`.

### Tablas de dominio

Una por migración de la fase que la necesitó (el detalle y el porqué de cada
una está en el comentario de su archivo `.sql`). Las claves foráneas a
`profiles` son `ON DELETE CASCADE`: borrar un personaje borra todo lo suyo.

| Tabla | Migración | Qué guarda |
|---|---|---|
| `profiles` | `0003`, `0009`, `0016` | Personajes: nombre, apodo, foto, tres imágenes del avatar reactivo (reposo/pensando/hablando), color de acento, tema. |
| `profile_identity` | `0003` | Lore, relación con el usuario, rol, forma de hablar, instrucciones especiales. |
| `profile_personality` | `0003` | Descripción libre y los 19 sliders de personalidad (estática, distinta del ánimo). |
| `conversations` | `0004`, `0007`, `0017` | Conversaciones por perfil y canal (`source`), archivado, fijado y orden de fijadas, hasta dónde se consolidó la memoria. |
| `messages` | `0004` | Mensajes con rol, estado, modelo y tokens. |
| `attachments` | `0005` | Imágenes y audios de mensajes: solo la ruta, el tipo, el tamaño y el hash. |
| `memories` (+ `memories_fts`) | `0006`, `0007`, `0019` | Memoria del personaje con búsqueda de texto completo FTS5 (contenido externo, sincronizada con triggers). `scope` (`private` por defecto / `public`) decide si Twitch y Discord pueden usarla. |
| `memory_embeddings` | `0011` | Embeddings locales para la búsqueda semántica (tabla satélite: sin fila = sin embedding). |
| `people` | `0010` | Registro de personas conocidas (de conversaciones, Twitch y Discord); alimenta el prompt de Whisper. |
| `profile_voice` | `0008` | Voz, velocidad, tono y volumen del personaje, y si lee las respuestas en voz alta. |
| `profile_moods` | `0012` | Estado emocional (valencia y energía); sin fila = neutral. |
| `profile_proactive` | `0013` | Modo proactivo por personaje; sin fila = desactivado. |
| `llm_usage` | `0020` | Uso de cada proveedor de IA por día: peticiones, errores, caracteres enviados y recibidos, último uso y último error. Solo cifras, nunca el texto de los mensajes. |
| `twitch_emotes` | `0018` | Caché de emotes de 7TV/BTTV/FFZ por canal, con su descripción por visión. |

`0014` y `0015` añadieron y luego quitaron una ventana de avatar (se reemplazó por
el servidor para OBS): nunca se edita una migración ya publicada, por eso
conviven.

## Claves de `app_settings`

Cada una es un JSON; las versionadas (`_v1`, `_v2`) existen porque el formato
cambió y la clave vieja se migra una vez y queda sin borrar.

| Clave | Contenido |
|---|---|
| `language` | Idioma de la app (es, en, pt, ja, ko, zh). |
| `llm_providers_v1` | Lista de proveedores de IA + cuál está en uso (ver ADR-010). Reemplaza a `llm_connection`. |
| `tts_providers_v1` | Lista de proveedores de voz + cuál está en uso (ADR-010). Reemplaza a `tts_connection`. |
| `stt_model` | Tamaño del modelo de transcripción y uso de GPU. |
| `dictation_settings` | Envío automático del texto dictado (ADR-011). |
| `call_policy` | Llamada: sensibilidad de voz, silencio, micrófono, frase de activación. |
| `twitch_connection`, `twitch_policy`, `twitch_emote_settings` | Conexión, reglas y emotes de Twitch. |
| `discord_connection`, `discord_policy` | Conexión y reglas de Discord. |
| `people_policy` | Qué personas se registran automáticamente. |
| `voice_muted` | Silencio global de la voz. |
| `chat_audio_settings` | Guardar el audio de los mensajes y por cuántos días. |
| `web_search` | Búsqueda web: activada y si hay key. |
| `avatar_server_settings` | Servidor del avatar para OBS. |
| `appearance_settings_v2`, `background_settings` | Modo claro/oscuro, densidad, fuente, redondez; fondo de la app. |
| `user_avatar_path` | Foto del usuario. |
| `onboarding_completed` | Si ya se completó la pantalla de bienvenida. |

## Secretos

Ningún secreto (API keys, tokens OAuth/bot) se guarda en una tabla ni en
`app_settings`: solo banderas como `has_api_key`. Los valores viven en
`SecretStore`, agrupados por servicio (`aivt.llm_provider`, `aivt.tts_provider`,
`aivt.twitch`, `aivt.discord`, `aivt.web_search`); los proveedores de IA y de voz
usan un slot por proveedor (`api_key.{id}`). Ver [security.md](security.md).

## Attachments

Las imágenes y los audios se guardan en disco bajo
`<app_data_dir>/attachments/<hash[..2]>/<hash>.<ext>`, direccionados por
contenido, nunca como base64 en SQLite: la fila solo referencia `path`,
`mime_type`, `size_bytes` y `hash`. Los avatares viven en
`<app_data_dir>/avatars/<profile_id>/`.
