# Arquitectura

## 1. Principio rector: Identidad Única

Chat, Llamada, Twitch y Discord no son cuatro bots distintos — son cuatro
formas de interactuar con el **mismo** personaje seleccionado. Identidad,
personalidad, lore, relación con el usuario y memoria son compartidas por
todos los canales; solo la fuente de la interacción cambia.

```
                          Chat Service
                               │
                        Call Service
                               │
Twitch Service ───────► AI ORCHESTRATOR ◄──────── Discord Service
                               │
                               ▼
                   Perfil de IA seleccionado (activo)
                               │
              ┌────────────────┼────────────────┐
              │                │                │
          Identity        Personality          Lore
              │                │                │
              └────────────────┼────────────────┘
                               │
                        PromptComposer
                               │
                         MemoryManager  ◄── memories.db (FTS5)
                               │
                        LLMProvider (activo)
                               │
                          ChatResponse
                          /    │    \
                        TTS  Avatar  Canal de origen
```

Cada servicio de canal construye un `InteractionContext` (`crates/core`) y se
lo entrega a `AIOrchestrator::respond()`. El Orchestrator es el **único**
lugar donde "el personaje piensa" — ningún servicio de canal reimplementa
prompt building ni selección de provider.

## 2. Capas

```
UI (React/TS)  — presentación, estado local, validación Zod
        │  Tauri IPC (commands invoke / events emit-listen)
Application Services (Rust) — ChatService, TwitchService, CallService...
        │
AI Orchestrator (Rust) — único punto de entrada para "el personaje responde"
        │
Provider Interfaces (Rust traits) — LLMProvider, STTProvider, TTSProvider...
        │
Providers concretos — LmStudioProvider, NativeTtsProvider, MockProviders...
```

Transversal a todas las capas: `EventBus`, `SecretStore`, `database` (sqlx +
SQLite), Activity/Logging, `ServiceManager`.

El frontend nunca habla directamente con LM Studio, Twitch, Discord ni con
el sistema de archivos — solo invoca comandos Tauri y escucha eventos.

## 3. Stack y justificación de la capa de datos

Ver el `README.md` para la lista de stack. La decisión que requiere
justificación explícita es la capa de acceso a datos:

**Evaluadas:** Diesel (síncrono, DSL pesado, mal ajuste con FTS5), SeaORM
(ORM completo, más magia de la que pide el brief), rusqlite (muy ligero pero
sin migraciones de fábrica y síncrono puro).

**Elegida: `sqlx`**, sin capa ORM completa — patrón Repository con SQL
explícito por módulo. Async nativo (encaja con Tokio/Tauri sin pools
bloqueantes), migraciones versionadas de fábrica (`sqlx::migrate!`), y
control total del SQL — necesario para FTS5, UPSERTs de deduplicación de
memoria y queries de retrieval con scoring que un ORM completo dificultaría.
Ver [ADR-002](ADR/002-sqlite-storage.md).

Nota de implementación: los repositorios usan la API dinámica de sqlx
(`sqlx::query`/`query_as`) en vez de las macros `query!`/`query_as!`
verificadas en tiempo de compilación, porque estas últimas requieren una
base de datos accesible (o un caché `.sqlx` versionado) en el momento de
`cargo build`, y este workspace no lo asume todavía. Migrar las queries más
sensibles a `query!` es un follow-up de bajo riesgo una vez exista ese
entorno.

## 4. Estructura de carpetas

```
aivt-companion/
├── src/                    # Frontend React (Vite root)
│   ├── app/                # shell, sidebar, PlaceholderPage
│   ├── features/<módulo>/  # una carpeta por pestaña del sidebar
│   ├── design-system/      # Button, Card, Tabs, Toast, Switch, Select...
│   ├── stores/              # zustand: navigationStore, settingsStore, toastStore
│   ├── lib/
│   │   ├── ipc/             # wrappers tipados sobre invoke()/listen()
│   │   ├── i18n/             # i18next + locales es/en
│   │   └── theme/            # cn() + variables CSS (globals.css)
│   └── main.tsx
├── src-tauri/
│   ├── src/
│   │   ├── lib.rs           # bootstrap real (mobile-ready: main.rs solo llama run())
│   │   ├── main.rs
│   │   ├── state.rs         # AppState gestionado por Tauri
│   │   └── commands/        # capa fina: cada #[tauri::command] delega al crate de servicio
│   ├── capabilities/         # ACL de Tauri v2
│   └── tauri.conf.json
├── crates/
│   ├── core/                 # AppError, InteractionContext, redact_secrets — sin dependencias hacia el resto
│   ├── database/              # pool sqlx, migrations/, repositories/
│   ├── eventbus/               # AppEvent + pub/sub tipado
│   ├── secrets/                 # trait SecretStore + impl keyring
│   ├── service_manager/          # ServiceManager: ciclo de vida Stopped/Starting/Running/Error/Stopping
│   ├── providers/llm/              # real: trait LLMProvider + adaptadores (OpenAI-compatible, Anthropic, Gemini) + FailoverLLMProvider (cambio automático entre proveedores)
│   ├── providers/tts/              # real: motores nativo, ElevenLabs, Google, Azure, Fish Audio, Deepgram y OpenAI-compatible (Groq, Together, genérico)
│   ├── providers/stt/              # real: whisper.cpp embebido (CPU) + gestor de modelos
│   ├── providers/{vision,embedding,avatar,tools}/  # reales: visión, embeddings locales, avatar, herramientas (hora, búsqueda web)
│   ├── memory/                    # real: memoria semántica, fusión, personas
│   ├── orchestrator/               # real: PromptComposer/PersonalityPromptBuilder (Fase 1) + AIOrchestrator.stream_respond (Fase 2)
│   ├── services/chat/                # real: ChatService — envío, streaming, cancelación (Fase 2)
│   ├── services/{call,twitch,discord,avatar,proactive}/  # reales (Llamada incluye el dictado por voz del chat)
│   ├── services/activity/           # real: sink de logging estructurado
│   ├── services/settings/            # real: toda la configuración — listas de proveedores LLM/TTS (con `build_llm_provider`/`build_tts_backend`), apariencia, voz, transcripción, etc.
│   ├── services/profiles/            # real: CRUD de perfiles, avatar, import/export (Fase 1)
│   └── testing/                       # real: MockLLMProvider, MockSecretStore (Fase 2)
└── docs/
```

Cargo workspace con un crate por módulo de dominio (no un crate monolítico)
para que los límites de módulo se apliquen en tiempo de compilación —
Twitch no puede importar código interno de Chat — y cada pieza sea testeable
de forma aislada.

Datos de usuario (DB, attachments, avatars, logs) usan el directorio de
datos de la app resuelto por Tauri (`app.path().app_data_dir()` →
`%APPDATA%` / `~/Library/Application Support` / `$XDG_DATA_HOME`), nunca
rutas hardcodeadas.

## 5. Interfaces principales

Viven en Rust; el frontend nunca implementa providers, solo consume DTOs
serde a través de comandos Tauri. Las firmas completas (`LLMProvider`,
`STTProvider`, `TTSProvider`, `AvatarProvider`, `MemoryManager`,
`AIOrchestrator`, `InteractionContext`, `AppEvent`, `SecretStore`) están
documentadas junto a su implementación real en cada crate — se listan aquí
solo las que ya existen:

- **`AppError`** (`crates/core`) — forma única de error que cruza el borde
  IPC: `{ module, code, message }`. Cada capa de comandos mapea el error de
  su crate a un `AppError` explícitamente en vez de usar `From` entre
  crates, para no forzar una dirección de dependencia hacia `core`.
- **`AppEvent` / `EventBus`** (`crates/eventbus`) — pub/sub interno tipado
  sobre `tokio::sync::broadcast`. `src-tauri` reenvía un subconjunto al
  frontend vía `emit("app://event", ...)`; el frontend se suscribe con el
  hook `useAppEvent`.
- **`SecretStore`** (`crates/secrets`) — trait async; implementación real
  vía `keyring` (Windows Credential Manager / macOS Keychain / Linux Secret
  Service). Ver [security.md](security.md).
- **`ManagedService` / `ServiceManager`** (`crates/service_manager`) —
  ciclo de vida `Stopped → Starting → Running → Error → Stopping` para
  cualquier integración de fondo; publica `AppEvent::ServiceStatusChanged`
  en cada transición.
- **`ProfileService`** (`crates/services/profiles`) — CRUD de perfiles,
  avatar en disco, import/export `.aivtprofile`. Ver ADR-004 (memoria) para
  cómo `InteractionContext.allow_memory` ya anticipa el consumo de perfiles
  desde Fase 4.
- **`PromptComposer` / `PersonalityPromptBuilder`** (`crates/orchestrator`)
  — combinan identidad, lore, relación y los 19 sliders de personalidad en
  el system prompt del personaje. Es la parte de `AIOrchestrator` que ya
  existe; el resto (selección de provider, memorias, contexto de
  conversación/canal) llega con Fase 2/4 sin cambiar este contrato —
  `PromptComposer::compose` seguirá siendo la única fuente de la porción
  "de personaje" del prompt, sin importar el canal.

Las interfaces de providers (`LLMProvider`, etc.) y `AIOrchestrator` (streaming,
herramientas) están implementadas desde la Fase 2 y se han ampliado desde
entonces: varios proveedores a la vez con cambio automático (ver
[ADR-010](ADR/010-multi-provider-and-failover.md)). La *selección* del proveedor
activo no vive en el Orchestrator sino en `SettingsService::build_llm_provider`.

## 6. Riesgos técnicos conocidos

| Riesgo | Mitigación |
|---|---|
| Linux Secret Service puede no estar disponible (headless, distros mínimas) | `SecretStore` retorna error explícito (`SecretError::Unavailable`), nunca cae a texto plano en silencio. |
| Detección de capacidades de modelo poco fiable vía LM Studio | Heurística + override manual por perfil (Fase 2). |
| Latencia extremo-a-extremo en Llamada es el punto más difícil del proyecto | Streaming desde el día 1 del pipeline (Fase 6), latencia medida por etapa. |
| Backends de IA local (Whisper/Piper/XTTS) varían por plataforma (CPU/CUDA/ROCm/Vulkan/Metal) | TTS sigue hablando HTTP con un servidor externo (`LocalTTSProvider`). STT sí embebe un runtime ML — whisper.cpp vía `whisper-rs` (`aivt-provider-stt`) — con **CPU como comportamiento por defecto y Vulkan como aceleración opcional** (`gpu_enabled` en `SttModelSettings`, apagado de fábrica). Deliberadamente **sin CUDA ni ROCm**: son específicos de un fabricante (NVIDIA/AMD respectivamente) y whisper.cpp además tiene un bug conocido donde, si se compilan varios backends de GPU juntos, CUDA se prioriza automáticamente aunque Vulkan rinda mejor en Windows, y puede fallar si el runtime de CUDA no está instalado (ver https://github.com/ggml-org/whisper.cpp/issues/3205). Vulkan solo, en cambio, da el mismo comportamiento (acelerado si hay GPU compatible, CPU si no) en NVIDIA/AMD/Intel por igual, sin ese riesgo — decisión explícita del usuario tras evaluar ambas opciones. Requiere el Vulkan SDK para compilar (ver README.md); el binario final solo necesita el runtime de Vulkan, que ya trae el driver de cualquier GPU moderna. |
| SQLite con escritores concurrentes | WAL mode + pool `sqlx::SqlitePool` compartido. |

## 7. Roadmap de fases (resumen)

Fase 0 Foundation (actual) → Fase 1 Perfiles/Identidad → Fase 2 Chat → Fase 3
Multimodal → Fase 4 Memoria → Fase 5 Voz → Fase 6 Llamada IA → Fase 7 Avatar
→ Fase 8 Twitch → Fase 9 Discord texto → ~~Fase 10 Discord voz~~ (intentada
y abandonada — bug abierto en `songbird` + DAVE obligatorio desde 2026,
ver docs/DEVELOPMENT_STATUS.md) → Fase 10b Búsqueda web / tool-calling
(verificada en vivo en los cuatro canales) → **Fase 11 Polish, completa**
— tab de Actividad, `ThemeEngine` por perfil (ADR-008/ADR-009),
confirmación al borrar en Chat, Markdown en el Chat, TTS sin Markdown/
URLs, y (post-fase) limpieza de pestañas de Configuración sin usar → Fase
12 investigación mobile.

Fase 10b (pedido explícito del usuario, 2026-09-05, número "b" para no
correr la numeración de las fases ya documentadas): le da al personaje una
herramienta de búsqueda web (`web_search`, Brave Search API), como una
capacidad transversal del orchestrator (`AIOrchestrator::with_tools`) en
vez de una integración por canal — Chat/Twitch/Discord/Call la comparten.
Opt-in, apagada por defecto. Detalle de diseño e implementación en
docs/DEVELOPMENT_STATUS.md.

Detalle de qué se implementó y qué falta: [DEVELOPMENT_STATUS.md](DEVELOPMENT_STATUS.md).
