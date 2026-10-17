# Project Ciel — resumen para análisis externo

Documento pensado para pegarlo en ChatGPT (u otra IA) junto con `architecture.md`, `security.md` y `database.md`
(los tres son cortos). **No adjuntes `DEVELOPMENT_STATUS.md`**: pesa ~240 KB, es una bitácora cronológica y
saturaría el contexto. Los ADR (`docs/ADR/`) se pueden adjuntar uno a uno si quieres que opine de una decisión concreta.

## 1. Qué es

App de escritorio **solo Windows 10/11 (64-bit)** para crear un personaje de IA con identidad, personalidad, memoria,
voz y avatar. Se usa en chat, en llamada de voz, y en Twitch y Discord. Es local-first: sin cuenta, sin suscripción,
sin servidor propio. El usuario trae su IA (modelo local vía LM Studio/Ollama, o sus propias claves de OpenAI,
Anthropic o Gemini).

- Versión actual: **0.1.0**, publicada en itch.io a **10 USD**, pago único, actualizaciones incluidas.
- Autor: una sola persona (Vulito), desarrollo asistido por IA (Claude). Dirección de diseño, pruebas y decisiones son suyas.
- Licencia: propietaria, con EULA en 6 idiomas. Sin abogado revisándola todavía.
- Páginas públicas: https://vulito.itch.io/projectciel y vitrina sin código en https://github.com/VuloB1/project-ciel.
  El código vive en un repo privado.

## 2. Stack

- **Tauri v2** (Rust) + **React 18 + TypeScript + Vite**, Tailwind, Radix UI, zustand, zod, i18next.
- Workspace de Rust con crates por capa: `core`, `database` (sqlx + SQLite, migraciones versionadas, FTS5),
  `eventbus`, `secrets` (keyring / Credential Manager), `service_manager`, `memory`, `orchestrator`,
  `providers/{llm,tts,stt,embedding,vision,tools,avatar}` y `services/{chat,call,twitch,discord,proactive,settings,profiles,backup,diagnostics,activity,avatar}`.
- STT: whisper.cpp embebido (CPU). Embeddings locales para búsqueda semántica de memorias.
- Avatar para OBS: servidor axum propio que se usa como Browser Source.
- 20 ADR documentados en `docs/ADR/` (Tauri vs Electron, SQLite, arquitectura de proveedores, memoria, fallback, privacidad, etc.).

## 3. Arquitectura en una pantalla

Principio rector: **identidad única**. Chat, Llamada, Twitch y Discord son cuatro canales del *mismo* personaje.
Todos pasan por un único `AIOrchestrator`, que compone el prompt (identidad + 19 sliders de personalidad + ánimo +
memorias relevantes + herramientas) y llama al `LLMProvider` activo. Ningún canal reimplementa el prompt.

Capas: UI React → comandos Tauri (capa fina) → servicios de aplicación → orquestador → traits de proveedores →
implementaciones concretas. El frontend nunca habla directamente con LLM, Twitch, Discord ni disco.

## 4. Funciones (0.1.0)

- **Personajes**: identidad, 19 sliders, ánimo dinámico, avatar con estados (idle/thinking/talking), tema por personaje, exportar/importar.
- **Chat**: historial con búsqueda, marcadores de no leído, fijados, imágenes (si el modelo tiene visión), Markdown en streaming, dictado por voz.
- **Llamada de voz**: STT + TTS en tiempo real, medidor de micrófono, selector de salida, guarda anti-eco, historial.
- **TTS**: voz del sistema (offline), ElevenLabs, OpenAI, Google Cloud, Azure, Fish Audio, Deepgram y servidores compatibles con OpenAI.
- **Memoria**: extracción automática desde los chats, edición manual, **privada/pública** por recuerdo, fusión de duplicados, filtro de relevancia (umbral semántico + solapamiento FTS).
- **Twitch/Discord**: respuesta en el chat con registro de espectadores y emotes (7TV/BTTV/FFZ). Solo se usan recuerdos públicos. Discord es solo texto (voz intentada y abandonada: exige DAVE y la librería songbird tiene un bug de recepción).
- **IA propia**: varios proveedores a la vez, **cambio automático** si uno se queda sin crédito, **modos de privacidad por proveedor** (Completo / Solo públicos / Mínimo), panel de uso.
- **Búsqueda web opcional** (Brave) como herramienta del modelo.
- **Mensajes espontáneos** (modo proactivo).
- **Datos**: copia de seguridad completa en un archivo (sin secretos), centro de diagnóstico.
- **UI**: 15 temas, claro/oscuro, acento propio, redondeo, densidad, fuente; 6 idiomas (es, en, pt, ja, ko, zh); el prompt que recibe el modelo también sigue el idioma de la app.

## 5. Calidad y estado de verificación (honesto)

Verificado en vivo: chat, memoria (privada/pública en los 4 canales), búsqueda web, emotes de Twitch, avatar en OBS real,
multi-proveedor con modelo local, UI base.

**Sin verificar todavía**, solo con pruebas automáticas o sin probar:
- Instalador y zip portable en un **PC limpio** (nunca probados fuera de la máquina de desarrollo).
- Restauración de copia completa con reinicio dentro de la app, centro de diagnóstico, historial de llamadas.
- Modos de privacidad con un proveedor en la nube real, panel de uso con peticiones reales.
- Proveedores cloud (OpenAI/Anthropic/Gemini/TTS en nube) con claves reales: probados poco.
- Auditoría de contraste de los 15 temas: corregida por código, no revisada visualmente.
- Íconos de archivo y doble clic en `.aivtbackup`/`.aivtprofile` desde el instalador.

## 6. Limitaciones conocidas

- Solo Windows. Linux/web evaluados y aplazados hasta ver si la app vende.
- En llamadas, ruido fuerte de teclado puede interpretarse como voz (un filtro se probó y se **retiró** porque no funcionaba). La guarda de eco **no** es cancelación de eco real.
- Discord solo texto.
- Instalador **sin firma de código** (SmartScreen avisa).
- Modelos locales pequeños (< 7-8B) dan mala memoria y repiten respuestas (un qwen3-1.7b entró en bucle).
- Sin auto-actualización, sin mecanismo de licencias, CI solo pensado para una plataforma.
- `read_url` (que el personaje lea páginas) está pausado hasta después del lanzamiento.

## 7. Riesgos de negocio y legales

- Arte generado con IA (ChatGPT y Meta AI): los términos dan permiso de uso comercial, pero no equivalen a derecho de autor exclusivo.
- EULA sin revisión legal; avisos de terceros generados con scripts (`legal/`).
- Sin dependencias GPL (decisión explícita para poder vender).
- Compite con herramientas gratuitas/open source de "AI VTuber"; el diferencial es que es una app lista, local y con privacidad real.

## 8. Backlog

Pruebas en vivo de lo anterior, claves reales de proveedores, firma de código, auto-actualización, licencias,
CI multiplataforma, capturas en inglés, avatar 2D/3D (aplazado a propósito), móvil (aplazado), integración MCP (**descartada
por el autor, no la propongas**), versión Linux/web (solo si hay ventas e interés).

## 9. Qué quiero de ti (edítalo antes de pegarlo)

Actúa como un revisor externo senior (producto + ingeniería). Con este resumen y los documentos adjuntos:

1. Dime los **3 mayores riesgos** técnicos o de producto que veas, ordenados, y cómo comprobarlos barato.
2. Señala **huecos de la arquitectura** (acoplamientos, puntos únicos de fallo, deuda probable al crecer).
3. Propón qué **probar primero** antes de mover más ventas, dado lo que está sin verificar en la sección 5.
4. Opina sobre **precio, posicionamiento y primeras 4 semanas** tras el lanzamiento.
5. No me propongas reescrituras grandes ni MCP. Prefiero mejoras pequeñas y verificables.

Responde con prioridades y razones, no con listas genéricas. Si algo no se puede saber con estos documentos, dime qué
archivo concreto necesitas ver.

> Añade aquí lo que quieres decirle (ideas, quejas, metas, presupuesto, cuánto tiempo tienes por semana):
>
> - …
