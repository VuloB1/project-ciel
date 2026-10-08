# Estado de desarrollo

Última actualización: **proveedores de IA y voz múltiples con cambio automático, dictado por voz en el chat y ajustes de interfaz** (2026-10-02) — ver la sección "Proveedores multi-servicio, voz, dictado y ajustes de interfaz" más abajo. La nota que sigue describe el trabajo anterior (localización del prompt de identidad).

Nota (2026-10-06): ajustes de interfaz y filtro de teclado en la Llamada — diálogos limitados a la altura de la ventana, campos de texto que crecen solos, "Importar personaje" movido a Identidad y un filtro básico de golpes de teclado en el segmentador de la Llamada que luego se quitó; ver la sección "Ajustes de interfaz y filtro de teclado (2026-10-06)" al final.

Nota (2026-10-02): panel de uso por proveedor de IA (peticiones, fallos, tokens estimados, último uso y último error, por periodo) en Configuración > Proveedores de IA, con la tabla `llm_usage` (migración 0020); ver ADR-020. Los tokens son una estimación; pendiente de verlo con peticiones reales.

Nota (2026-10-02): modos de privacidad por proveedor de IA (Completo / Solo recuerdos públicos / Mínimo) con insignia Local/Nube; el orquestador manda recuerdos y ánimo aparte y cada proveedor recibe solo lo permitido, también tras un cambio automático; ver ADR-019. Pendiente de probar con un proveedor en la nube real. El panel de uso sigue pendiente.

Nota (2026-10-02): auditoría de accesibilidad de los 15 temas (WCAG AA): contraste medido y corregido en modo Claro, colores de estado, texto secundario, acentos propios y bordes de controles, con una prueba que lo vigila; ver ADR-018. Pendiente de una pasada visual en la app.

Nota (2026-10-02): mejoras de audio de la Llamada — medidor de nivel del micrófono (barra en la llamada y en sus ajustes), guarda contra el eco para quien usa altavoces y selector de salida de audio para las voces; ver ADR-017. Probado con pruebas automáticas, pendiente de probar con micrófono y altavoces reales. La guarda de eco NO es cancelación de eco real (se ignora el micrófono mientras ella habla).

Nota (2026-10-02): `SettingsService` se dividió por área en `crates/services/settings/src/{llm,tts,twitch,discord,call,web_search,stt,preferences}.rs` sin cambiar su API ni los datos guardados — ver ADR-016. Las rutas a `settings/src/service.rs` en esta bitácora se actualizaron al archivo nuevo donde aplica.

Actualizado durante: **localización del prompt de identidad a los 6
idiomas de la app** (2026-09-24), pedida por el usuario tras notar, probando
la app en coreano, que la vista previa de Identidad seguía mostrándose
íntegramente en español — el prompt de identidad/personalidad se había
implementado antes de que existiera el sistema de idiomas y quedó con todo
el texto de conector hardcodeado en español.

- `crates/orchestrator/src/prompt.rs` (`PromptComposer::compose`) y
  `crates/orchestrator/src/personality.rs`
  (`PersonalityPromptBuilder::build`) ahora reciben un `lang: &str`
  (`SettingsService::language`, es/en/pt/ja/ko/zh) y seleccionan, por
  `match`, un juego de frases escrito a mano para ese idioma — no es una
  traducción literal del español sobre la misma plantilla: para ja/ko/zh
  cada rasgo de personalidad se escribió como oraciones completas propias
  (en vez de forzar la estructura española de cláusulas unidas por comas,
  que no es natural en esos idiomas) y simplemente se concatenan. Un idioma
  no reconocido cae a español, igual que `SettingsService::DEFAULT_LANGUAGE`.
  Los 19 sliders de personalidad (sección 11 del brief) y los conectores del
  perfil (biografía, relación, forma de hablar, instrucciones especiales,
  edad/pronombres/especie/ocupación) tienen ahora su propio texto en los 6
  idiomas — unas ~400 frases nuevas en total. Los campos de texto libre del
  propio perfil (nombre, lore, descripción libre, etc.) nunca se traducen —
  son datos del usuario, no texto de la interfaz, así que viajan tal cual
  se escribieron sin importar el idioma activo.
- `crates/orchestrator/src/orchestrator.rs` (`AIOrchestrator::stream_respond`)
  gana un parámetro `language` que reenvía a `PromptComposer::compose` — la
  sección de mood, memoria y herramientas del prompt se queda en español por
  ahora (no es parte del "prompt de identidad" que pidió el usuario).
  `crates/services/chat/src/service.rs` (`GenerationContext`) suma un campo
  `language`, resuelto por la capa de comandos de Tauri
  (`src-tauri/src/commands/chat.rs`) desde `state.settings.language()`, mismo
  patrón que `tts`/`model`. Twitch/Discord/Llamada
  (`crates/services/{twitch,discord,call}/src/service.rs`) y el servicio de
  mensajes espontáneos (`crates/services/proactive/src/service.rs`) resuelven
  el idioma directo vía `self.settings().language()` antes de llamar a
  `stream_respond`/`PromptComposer::compose`. Esto significa que el cambio no
  es solo cosmético en la vista previa: el prompt real que recibe el modelo
  en Chat/Twitch/Discord/Llamada/mensajes espontáneos también sigue el idioma
  activo de la app.
- `src-tauri/src/commands/profiles.rs` (`profiles_preview_prompt`, usado por
  la pestaña Identidad > Vista previa) ahora es async, toma
  `state: tauri::State<AppState>` y resuelve el idioma de la misma forma
  antes de llamar a `PromptComposer::compose` — el frontend
  (`src/features/identity/IdentityEditor.tsx`) no cambió nada, ya manejaba
  el `Result` vía `.catch()`.
- De paso: se encontró y corrigió un error de compilación preexistente en
  `crates/services/twitch/src/service.rs` (test helper `connection()` sin el
  campo `profile_image_url`, agregado en la función de avatar de Twitch/
  Discord de una sesión anterior pero nunca verificado con
  `cargo test -p aivt-service-twitch`), y 3 errores de TypeScript
  preexistentes de la sesión de rediseño de Apariencia
  (`src/features/settings/SettingsDialog.tsx`, `src/lib/theme/accentTheme.ts`,
  `src/lib/theme/fonts.ts`) que tampoco se habían verificado con `npx tsc -b`
  después de esos cambios.
- Bug encontrado por el usuario apenas probó lo anterior: cambiar el idioma
  de la app en Configuración mientras la pestaña Identidad > Vista previa ya
  estaba abierta no actualizaba el texto — el `useEffect` que llama a
  `previewProfilePrompt` en `src/features/identity/IdentityEditor.tsx` solo
  reaccionaba a cambios en `draft` (los campos del perfil), no al idioma
  global de la app, así que la vista previa quedaba con el idioma que estaba
  activo la última vez que se editó algún campo. Arreglado agregando
  `useSettingsStore((s) => s.language)` como dependencia adicional de ese
  efecto.
- Extensión pedida por el usuario a continuación: las secciones de mood y
  de memorias recuperadas del prompt (las dos partes que quedaban afuera
  del alcance original, "no es parte del prompt de identidad") también
  siguen ahora el idioma activo. `crates/orchestrator/src/mood.rs`
  (`MoodPromptBuilder::build`) suma el mismo parámetro `lang: &str` y
  localiza las 8 frases de humor/energía (grilla 3×3 sin el caso neutral).
  `crates/orchestrator/src/orchestrator.rs` (`memory_section`) localiza solo
  el encabezado ("Recuerdos relevantes de conversaciones anteriores:") —
  título/resumen/contenido de cada memoria siguen siendo datos del usuario,
  nunca traducidos, mismo principio que el resto. Los dos call sites
  (`AIOrchestrator::stream_respond` y
  `crates/services/proactive/src/service.rs`) ya tenían `language`
  resuelto de la sesión anterior, así que no hizo falta tocar ningún otro
  servicio.
- Pedido final del usuario, explícito: "que todo cambie al idioma
  seleccionado, para evitar cualquier problema donde la IA elija responder
  en español" — se localizó el resto de lo que le llega al modelo en cada
  turno, no solo lo que ya estaba cubierto:
  - `crates/orchestrator/src/orchestrator.rs`: el aviso de uso de
    herramientas (`tool_usage_hint`) y la línea de fecha/hora
    (`current_datetime_line`/`format_current_datetime_line`, incluida la
    nueva `weekday_name` — antes `spanish_weekday`) ahora siguen `lang`. El
    formato numérico de la fecha/hora (`%d/%m/%Y`, 24h) se deja igual en
    los 6 idiomas a propósito — son dígitos ya neutros, y variarlo (MM/DD
    vs. DD/MM) sumaría ambigüedad sin beneficio real. También los mensajes
    de fallback que se reinyectan a la conversación cuando una tool call
    falla o el modelo pide una herramienta inexistente
    (`tool_execution_error_message`/`unknown_tool_message`) — antes
    quedaban colgados en español aunque el resto del turno estuviera en
    otro idioma.
  - `crates/providers/tools/src/current_time.rs` y `.../brave.rs`
    (`CurrentTimeTool`/`BraveSearchTool`): ambas herramientas ahora
    guardan un `lang: String` propio (resuelto una vez al construirse, ya
    que `Tool::description()`/`parameters_schema()` no reciben argumentos)
    y localizan su `description()`, la descripción del parámetro en
    `parameters_schema()`, y los mensajes de error/fallback
    (`falta el campo 'timezone'/'query'`, zona horaria IANA inválida, "no
    se encontraron resultados") — todo esto viaja al modelo como parte de
    la definición de herramientas o como resultado de una tool call, así
    que también podía sesgar el idioma de respuesta. `CurrentTimeTool::new`
    y `BraveSearchTool::new` ahora piden el idioma como argumento; los 4
    call sites que las construyen (`src-tauri/src/commands/chat.rs` y
    `crates/services/{twitch,discord,call}/src/service.rs`) mueven la
    resolución de `language` a antes de armar la lista de tools.
  - `crates/services/twitch/src/service.rs`: la instrucción "el chat de
    Twitch no interpreta Markdown" (antes `TWITCH_NO_MARKDOWN_INSTRUCTION`,
    ahora `no_markdown_instruction(lang)`) y el texto que introduce la
    lista de emotes (`emote_list_instruction`) también siguen el idioma —
    `Inner::build_extra_instructions` ahora pide `lang`. Los nombres/
    descripciones de los emotes en sí (datos de 7TV/BTTV/FFZ) NO se
    traducen, mismo principio que el resto.
  - Deliberadamente fuera de alcance todavía: los prefijos de
    `ToolError` (`crates/providers/tools/src/error.rs`, vía `thiserror`) —
    solo se disparan en fallos de red/backend genuinos (no en el flujo
    normal), y la mayoría de su contenido ya es texto técnico en inglés
    (errores de `reqwest`/HTTP), no prosa que pudiera sesgar el idioma de
    respuesta. Si en algún momento hace falta, requiere sacar `ToolError`
    de la derivación de `thiserror` para poder parametrizarlo por idioma.
- Pregunta de seguimiento del usuario ("¿el registro de actividad también
  genera actividades en los diferentes idiomas?"): resultó que NO — el
  panel Configuración > Actividad usa un mecanismo totalmente distinto al
  prompt (un puente `tracing` → tabla `activity_events`, ver
  `crates/services/activity/src/layer.rs`), no el sistema de i18n del
  frontend, así que quedó completamente afuera de todo lo anterior. Cada
  fila de la Actividad muestra el campo `summary` de una llamada
  `tracing::info!/warn!(target: "activity", summary = "...")`, tal cual —
  se localizaron los 10 sitios reales que emiten una de estas (no los de
  test):
  - `crates/memory/src/service.rs` (`extract_and_store`/`propose_fusion`):
    los 2 mensajes de "la respuesta del modelo no se pudo interpretar
    como memorias/grupos de fusión". Ambos métodos del trait
    `MemoryManager` ganaron un parámetro `language: &str` (y
    `MockMemoryManager`, en `crates/testing/src/mock_memory.rs`, un
    `_language` sin usar) — se resuelve en
    `src-tauri/src/commands/{chat,memory}.rs` y en
    `ChatService::extract_memories_now`/`maybe_consolidate` (esta última ya
    tenía `ctx.language` disponible desde la primera pasada).
  - `crates/services/chat/src/service.rs`: los 3 resultados de una
    generación (`run_generation`: "Respuesta generada" / "Generación
    cancelada por el usuario" / "Error al generar respuesta: …") y los 2 de
    consolidación de memoria (`maybe_consolidate`: "N memorias nuevas
    extraídas" / "Falló la extracción de memorias: …").
  - `crates/services/twitch/src/service.rs` y
    `crates/services/discord/src/service.rs` (`respond_to`, en ambos): "Respondió
    a {nombre}" — cada uno resuelve `language` fresco vía
    `self.settings().language()` ahí mismo, ya que `respond_to` es una
    función distinta de `generate_reply` (donde ya se resolvía para el
    prompt) y no lo tenía en scope.
  - `src-tauri/src/lib.rs`: "Project Ciel se inició correctamente", el
    evento de arranque — se resuelve el idioma dentro del mismo bloque
    `tauri::async_runtime::block_on` que ya arma `settings`/`pool`, antes
    de que el `tracing::info!` del bootstrap se dispare.
  - Sin cambios en el frontend: `ActivityPage.tsx` ya mostraba
    `event.summary` tal cual (sin pasar por `t()`) — el texto llega
    pre-localizado desde Rust.

Verificación: `cargo build`/`cargo test` en verde para
`aivt-orchestrator` (46 tests, incluye casos nuevos en/ja y de fallback a
español), `aivt-service-chat`, `aivt-service-twitch`, `aivt-service-discord`,
`aivt-service-call` y `aivt-service-proactive` (88 tests + 14 tests tras la
extensión de mood, sin fallos). `npx tsc -b` y `npx vitest run` (216 tests)
en verde tras las correcciones de TypeScript preexistentes y tras el fix
del `useEffect`. Build completo de `aivt-companion` verificado dos veces
(feature de identidad + extensión de mood/memoria). Confirmado en vivo
contra la app real (CDP), con el idioma de la app ya en coreano (como lo
dejó el usuario probando):
Identidad > Vista previa ahora muestra "당신은 Ciel입니다 16세, Humana,
Compañera입니다.", "배경과 설정:", "특별 지침:" y las 19 frases de
personalidad en coreano, mientras que el lore/descripción/instrucciones
especiales que el propio usuario escribió en español se mantienen intactos.
Para el fix del `useEffect`: se reprodujo el bug reportado por el usuario
(cambiar el idioma en Configuración con la pestaña Vista previa ya abierta,
sin tocar ningún campo) y se confirmó en vivo que, tras el fix, la vista
previa pasa de coreano a español sola en cuanto se cierra el diálogo de
Configuración, sin necesidad de editar el perfil. La extensión de mood/
memoria no tiene una pantalla de vista previa propia (solo aparece en el
prompt real de una conversación) — se verificó por test automatizado
(assertions de contenido exacto por idioma) en vez de una captura en vivo;
queda ofrecido hacer una prueba en vivo con un chat real si el usuario la
quiere.
Para la extensión final ("que todo cambie"): `cargo test` en verde para
`aivt-orchestrator` (48 tests, incluye
`datetime_line_and_tool_hint_follow_the_configured_language` y el
fallback a español) y `aivt-provider-tools` (21 tests, incluye
`description_and_schema_follow_the_configured_language` para ambas
herramientas) — 157 tests en total entre todos los crates tocados, sin
fallos. Build completo de `aivt-companion` verificado una tercera vez. Se
intentó una verificación en vivo enviando "What time is it right now in
Tokyo?" con la app en inglés (para forzar la tool call de
`get_current_time`), pero el servidor local del modelo no estaba
corriendo en ese momento ("no se pudo conectar" — un problema aparte, no
relacionado con este cambio) y no se pudo completar; se confía en la
batería de tests automatizados para esta parte en particular. El idioma
de la app se restauró a español (como se lo encontró) antes de relanzar
limpia.
Para el registro de Actividad: `cargo build`/`cargo test` en verde para
`aivt-memory`, `aivt-testing`, `aivt-service-chat`, `aivt-service-twitch`,
`aivt-service-discord`, `aivt-service-call`, `aivt-service-proactive`,
`aivt-orchestrator` y `aivt-provider-tools` (218 tests en total, sin
fallos) y build completo de `aivt-companion`. Confirmado en vivo contra la
app real (CDP): con la app en inglés y relanzada para que el evento de
arranque se generara de nuevo, Configuración > Actividad (con "Show system
events" activado) muestra la entrada más reciente como "Project Ciel
started successfully", mientras que todas las entradas anteriores (de
lanzamientos previos a este cambio) se quedan en español — comportamiento
esperado, cada entrada es un registro histórico congelado en el idioma
que estaba activo en el momento. Se restauró el idioma a español y se
relanzó limpia otra vez al terminar.

**Nota anterior**: filas más compactas en "Personas registradas"
(2026-09-24), pedido del usuario tras ver la lista de Configuración >
Privacidad con demasiado espacio vertical entre cada persona.

- `src/features/settings/PrivacySettings.tsx`: la fila de cada persona pasó
  de `px-3 py-2.5` a `px-3 py-1.5` — el resto (badge de rol, botón de
  borrar) sin cambios.

Verificación: confirmado en vivo contra la app real (CDP) — entran más
filas visibles en el mismo espacio sin recortar contenido.

**Nota anterior**: corrección de desborde de texto en Memoria
(2026-09-24), encontrada por el usuario probando con texto muy largo en
una memoria nueva — tanto la vista previa de la tarjeta como el diálogo de
confirmación de borrado se salían del recuadro cuando el texto (o el
título) no tenía espacios donde cortar.

- `src/features/memory/MemoryPage.tsx` (`MemoryCard`): el párrafo de vista
  previa (antes sin ningún límite) ahora usa `break-words` (corta palabras
  muy largas en vez de desbordar el contenedor) y se recorta a 3 líneas con
  `line-clamp-3`, sin importar si el texto viene del resumen o cae al
  contenido completo. El toggle "Ver más/Ver menos" (antes solo aparecía si
  había un resumen distinto del contenido) ahora también aparece cuando el
  contenido por sí solo es lo bastante largo como para que el recorte de 3
  líneas probablemente esté cortando algo (heurística de caracteres,
  `PREVIEW_CLAMP_HEURISTIC_CHARS`) — antes, una memoria larga sin resumen
  separado no tenía ninguna forma de expandirse. Al expandir siempre se
  muestra `memory.content` completo (con `break-words` también),
  reemplazando la vista recortada en vez de duplicar el texto debajo.
- El diálogo de confirmación de borrado interpolaba `memory.title` (hasta
  80 caracteres, potencialmente una sola palabra sin espacios) directo en
  el título de una sola línea del diálogo, sin ningún corte. Nuevo helper
  `truncateForTitle` corta a 40 caracteres + "…" antes de interpolarlo,
  siguiendo el pedido explícito del usuario de ignorar el resto del nombre
  en vez de envolver el diálogo en varias líneas.

Verificación: `npx tsc -b` y `npx vitest run src/features/memory` en verde
(4 tests, sin regresiones). Confirmado en vivo contra la app real (CDP): el
texto de prueba que reportó el problema ahora se recorta dentro de la
tarjeta, el toggle "Ver detalle" funciona, y se probó además un caso extra
(300 caracteres sin espacios, sin resumen) para confirmar que el arreglo
cubre el contenido solo, no únicamente el resumen. El diálogo de borrado
también se verificó en vivo con el título largo real.

**Nota anterior**: ajustes de texto y layout en Configuración — General,
Memoria y Privacidad (2026-09-23), varios pedidos puntuales de UX en la
misma sesión:

- **General**: el texto adicional de "Notificaciones de mensajes
  espontáneos" se reemplazó por uno más directo ("Una notificación de
  escritorio cuando el personaje te escribe por su cuenta").
- **Memoria** (`MemorySettings.tsx`): se quitó el separador entre la
  descripción de "Búsqueda semántica de memoria" y la fila de descarga del
  modelo (mismo tema, no hacía falta la línea) y los botones "Rellenar
  memorias antiguas"/"Fusionar memorias" (antes debajo de su texto,
  alineados a la izquierda) pasaron a la derecha, en la misma fila que su
  texto — mismo patrón que ya usaba la fila de descarga.
- **Privacidad** (`PrivacySettings.tsx`), reorganizada en dos rondas a
  pedido del usuario: "Guardar audio de los mensajes" + "Borrar
  automáticamente después de" (antes al final, mezclado con el resto) pasó
  a ser la primera tarjeta de la pestaña, separada del resto (tema
  distinto: retención de audio vs. registro de personas), sin separador
  interno entre el switch y el selector de retención (pedido explícito de
  quitarlo). Debajo, "Registro de personas" (título/descripción), los dos
  toggles de aprendizaje automático (Twitch/Discord, antes cada uno en su
  propio bloque con separador) y "Personas registradas" (con su botón y la
  lista completa, antes una `Card` aparte flotando fuera de la tarjeta
  principal) se unificaron en una sola tarjeta, sin separadores entre los
  toggles.

Verificación: `npx tsc -b` limpio en los tres archivos,
`npx vitest run src/features/settings/__tests__/SettingsDialog.test.tsx`
en verde. Las tres pantallas se confirmaron en vivo vía CDP y el usuario
las validó explícitamente ("lo has hecho exactamente como quería,
gracias!").

**Nota anterior**: Twitch/Discord — avatar de la cuenta conectada en la
cabecera (2026-09-23), pedido tras mostrar una captura de referencia:
donde antes la cabecera de cada pestaña mostraba siempre el ícono
genérico de la plataforma, ahora, con la cuenta conectada, muestra la
foto de perfil real de esa cuenta.

- **Backend**: Helix (`GET /users`) ya devolvía `profile_image_url` pero
  `TwitchUser` (`crates/services/twitch/src/helix.rs`) lo descartaba — se
  agregó el campo y se encadenó hasta `mark_twitch_connected`
  (`crates/services/settings/src/twitch.rs`, ahora con un tercer
  parámetro) y `TwitchConnectionSettings`. Discord no tiene un campo
  directo — `DiscordHttpClient::current_user`
  (`crates/services/discord/src/http.rs`) arma la URL del CDN de Discord a
  mano (`https://cdn.discordapp.com/avatars/{id}/{hash}.{png|gif}`) a
  partir del hash de avatar (`ImageHash`) que ya devolvía `GET
  /users/@me`, usando `.gif` cuando el hash es animado. Ambos campos
  nuevos (`profileImageUrl`/`avatarUrl`) son `#[serde(default)]` — sin
  migración, una conexión guardada antes de este cambio carga con el campo
  vacío.
- **Frontend**: `TwitchPage.tsx`/`DiscordPage.tsx` — el círculo de la
  cabecera renderiza un `<img>` con `object-cover` cuando hay cuenta
  conectada y el campo no está vacío, con fallback al ícono genérico en
  cualquier otro caso.
- **Limitación conocida, no un bug**: una cuenta conectada antes de este
  cambio no tiene el avatar guardado — hace falta desconectar y volver a
  conectar una vez. Confirmado con el propio usuario, que reconectó ambas
  cuentas después del cambio.

Verificación: `cargo build -p aivt-companion` y
`cargo test -p aivt-service-settings` (57 tests) en verde. Confirmado en
vivo por el usuario tras reconectar Twitch y Discord ("si se ve las
imágenes, queda genial!").

**Nota anterior**: Twitch/Discord — textos y reorganización del diálogo de
ajustes (2026-09-23), pedido en varias rondas dentro de la misma sesión:

- **Textos reescritos** (Twitch): "canal al que se une" → explica cuándo
  usar un bot dedicado; "cuándo responde el personaje" → aclara que nunca
  responde a la propia cuenta conectada y cómo probarlo; el texto de
  emotes 7TV/FFZ/BTTV se sacó (queda solo la cantidad en caché + si tienen
  descripción); el texto de dónde se genera la descripción de emotes ahora
  explica que hace falta un proveedor de IA con visión; "Probar respuesta"
  ahora aclara que es solo una prueba, sin publicar en el chat real.
- **Textos reescritos** (Discord): "Bot de Discord" ahora aclara que hay
  que conectar el perfil del bot, no la cuenta personal; "Probar
  respuesta" con el mismo texto que Twitch.
- **Reorganización del diálogo de ajustes**, idéntica en ambas
  plataformas: el badge de estado de conexión ("Conectado como —"/"No
  conectado") pasó a compartir la línea del título del diálogo (antes en
  su propia fila debajo); el botón Conectar/Desconectar pasó a la fila de
  "Cuenta de Twitch"/"Bot de Discord", reemplazando el lugar donde antes
  vivía el badge.
- Las claves nuevas/reescritas se tradujeron a los 6 locales (validado
  explícitamente a pedido del usuario, incluyendo dos casos de puntuación
  de ancho completo en japonés que rompieron el primer intento de `Edit`
  — corregidos releyendo la línea exacta antes de reintentar).

Verificación: `npx tsc -b` limpio, `node -e "JSON.parse(...)"` sobre los 6
locales para validar el JSON. Confirmado en vivo vía CDP en ambas
pestañas.

**Nota anterior**: Avatar — limpieza de la página (2026-09-23), dos rondas
de feedback tras probar la app en vivo:

- Se quitó la sección "Vista previa manual" completa (recuadro de imagen +
  selector de estado) — el usuario ya podía ver la reacción real del
  avatar mientras chatea, la vista previa manual era redundante.
- La fila de subida de imágenes por estado ("en reposo"/"pensando"/
  "hablando", antes alineada a la izquierda) se centró.
- Se sacó la frase "El estado 'pensando'/'hablando' suele durar poco —
  quédate en esta pestaña para verlo reaccionar mientras chateas" por
  quedar innecesaria tras los dos cambios anteriores.

Cambio puro de frontend (`AvatarEditor.tsx` + 2 claves i18n eliminadas en
los 6 locales). Confirmado en vivo por el usuario en ambas rondas ("se ve
bien todo, gracias!", "gracias, se ve bien!").

**Nota anterior**: redondez de temas — deja de ser un multiplicador
(2026-09-23), pedido del usuario: que el slider de redondez pueda volver
más recto o redondo cualquier tema, pero que si nunca se toca, cada tema
mantenga su propio diseño.

- **Causa raíz**: `AppearanceSettings.roundness` era un `u8` que
  multiplicaba el radio propio de cada preset (`scaleRadius`). 4 de los 15
  presets (`terminal`, `noir`, `cyberpunk`, `industrial`) tienen radio
  `0px` por diseño — 0 × cualquier cosa sigue siendo 0, así que el slider
  nunca podía redondearlos. Tampoco había forma de distinguir "el usuario
  eligió 100%" de "nunca lo tocó".
- **Arreglo**: `roundness` pasó de `u8` a `Option<u8>` (`None` por
  defecto = sin tocar, cada tema conserva su radio; `Some(pct)` = override
  explícito, aplicado como valor absoluto desde una base compartida nueva
  `CANONICAL_RADIUS` — 6/10/16px — igual para los 15 temas, ya no un
  multiplicador del radio propio). Nuevo helper `applyRoundness`
  (`useActiveProfileTheme.ts`) centraliza el cálculo, usado tanto por el
  hook del tema activo como por las tarjetas de vista previa del selector
  de temas. Clave de almacenamiento subida a `appearance_settings_v2`
  (evita que un `roundness: 100` guardado con la semántica vieja se
  malinterprete como "el usuario lo customizó").
- UI: mensaje "Sigue el radio propio de cada tema" (sin tocar) vs. "Se
  aplica a todos los temas, sin importar cuál elijas" (customizado), con
  un botón "Restablecer".

Verificación: 2 tests de Rust actualizados + 3 tests nuevos de frontend
(`useActiveProfileTheme.test.ts`, 216 en total). Confirmado en vivo: un
tema de esquinas cuadradas (Cyberpunk) se mantiene cuadrado sin tocar el
slider, y se redondea igual que cualquier otro tema en cuanto se
customiza.

**Nota anterior**: rediseño de "Apariencia" (2026-09-22), inspirado en
capturas de la app OpChat — modo claro/oscuro/sistema, densidad de
mensajes, tamaño de fuente y una fuente propia (Google Fonts
auto-hosteadas vía `@fontsource/*`, sin depender de una request externa),
además de la redondez ya mencionada arriba. Planificado con
`ExitPlanMode` por tocar el motor de temas, la config de Tailwind y una
struct de settings global nueva.

- **Decisión de alcance**: el tema/acento por personaje sigue siendo
  config del perfil (sin cambios); las 5 preferencias nuevas (modo/
  redondez/densidad/fuente/tamaño) son globales de la app
  (`AppearanceSettings` en `SettingsService`, mismo patrón
  `get_json`/`set_json` que `BackgroundSettings`).
- **Modo claro/oscuro**: en vez de una rampa clara/oscura compartida y
  genérica para los 15 presets (que rompía la identidad de temas como
  Terminal, donde el acento y el texto son literalmente el mismo verde),
  el modo Oscuro (default) deja cada preset exactamente como está hoy —
  cero regresión — y solo el modo Claro sustituye los 6 neutros por una
  rampa clara compartida, salvo en presets que ya son claros por sí mismos
  (detectado por brillo percibido, fórmula YIQ, no hardcodeado).
- **Reorganización posterior, misma sesión** (2026-09-23): la pestaña
  "Apariencia" independiente se disolvió — el selector de tema/acento por
  personaje se mudó a Identidad, y las 5 preferencias globales a
  Configuración.

Verificación: CDP en vivo (slider de redondez en 0/100/200%, las 3
densidades en burbujas de chat reales, tamaño de fuente en los extremos
80/140% sin overflow, varias fuentes del catálogo aplicadas). Confirmado
en vivo por el usuario ("Quedó bien y genial!").

**Nota anterior**: barra de título personalizada (2026-09-22) — la barra
de título nativa de Windows se reemplazó por una propia
(`src/app/Titlebar.tsx`) con el mismo estilo que el resto de la app
(ícono + "Project Ciel" a la izquierda, minimizar/maximizar/cerrar a la
derecha, cerrar con hover rojo).

- `tauri.conf.json`: ventana principal con `"decorations": false`. Nuevos
  permisos de ventana en `capabilities/default.json` (minimizar/
  maximizar/restaurar/cerrar/arrastrar — ninguno viene en
  `core:window:default`).
- Arrastrar para mover y doble-click para maximizar salen gratis del
  atributo `data-tauri-drag-region`, sin JS manual.
- La barra de título se renderiza incluso en modo inmersivo (a diferencia
  del Sidebar) — sin decoraciones nativas, es la única forma de cerrar/
  minimizar que queda.
- De paso, se corrigió el `<title>`/favicon de `index.html`, que seguía
  diciendo "AIVT Companion".

Verificación: CDP en vivo (maximizar/restaurar, hover rojo en cerrar).
Confirmado en vivo por el usuario ("funciona bien los temas", "funciona
bien, se ve lindo!").

**Nota anterior**: ícono circular del personaje en las notificaciones de
escritorio (2026-09-17), pedido justo después de probar en vivo las
notificaciones de mensajes proactivos recién agregadas — el usuario quería
que se vean como las de WhatsApp (avatar de quien escribe, recortado en
círculo) en vez de solo texto genérico.

- `tauri-plugin-notification`'s propio bridge de JS **no reenvía ningún
  ícono en Windows** (`sendNotification({ icon })` es un no-op ahí — solo
  título/cuerpo/sonido llegan al toast real; confirmado leyendo el código
  fuente de `tauri-plugin-notification`/`notify-rust`, no adivinado). Para
  lograr el ícono hubo que saltarse ese plugin: nuevo comando de Rust
  `notifications_send_proactive` (`src-tauri/src/commands/notifications.rs`)
  que arma el toast directo con `tauri-winrt-notification` (ya vivía en el
  árbol de dependencias vía `notify-rust`, ahora es dependencia directa
  bajo `[target.'cfg(windows)'.dependencies]`), usando
  `Toast::icon(path, IconCrop::Circular, alt)` — exactamente el mecanismo
  que usa Windows para el `appLogoOverride` circular. `src/lib/notifications.ts`
  ahora invoca ese comando (pasando `profileId`) en vez de llamar a
  `sendNotification()` directamente; en plataformas no-Windows el comando
  cae a la vía genérica del plugin (sin ícono, mismo texto).
- **Gotcha de Windows encontrada probando en vivo, no documentada
  claramente en ningún lado**: cualquier archivo dentro de
  `<data_dir>/avatars/...` (la carpeta propia de la app en
  `AppData\Roaming`) renderiza en blanco como ícono del toast — sin error,
  el aviso sale sin él — sin importar tamaño ni formato (se probó tanto el
  avatar original de 500x500/~200 KB como miniaturas de 128x128 generadas
  con el crate `image` y con GDI+/PowerShell; todas en blanco dentro de
  esa carpeta, todas visibles al copiar el mismo archivo a la carpeta
  temporal del sistema). Casi seguro una restricción de ACL/AppContainer
  sobre la carpeta propia de datos de la app que el proceso que dibuja el
  toast no puede leer. Por eso `ProfileService::notification_icon_path`
  (`crates/services/profiles/src/service.rs`) reduce el avatar a 128x128
  con el crate `image` (nueva dependencia de workspace — más que nada
  para no mandarle a Windows una imagen más pesada de lo necesario) y
  escribe el resultado en `std::env::temp_dir()`
  (`aivt_notification_icon_<profile_id>.png`, regenerado en cada envío) en
  vez de junto al avatar.

**Nota anterior**: minimizar a la bandeja + notificaciones de
escritorio + voz en segundo plano (2026-09-17) — planificado con
`ExitPlanMode` dado que tocaba configuración de Tauri, un plugin nuevo y
varias capas del backend (settings, TTS, eventbus). Antes de esto, en la
misma sesión pero sin banner propio: Configuración pasó de pestaña fija a
ventana flotante (`SettingsDialog.tsx`, mismo espíritu que
`CallSettingsDialog`) y la pestaña Actividad se rediseñó como una lista
compacta de una fila por evento en vez de tarjetas espaciosas.

- **Minimizar a la bandeja** (opt-in, apagado por defecto — la X sigue
  cerrando la app como siempre salvo que el usuario prenda "Minimizar a
  la bandeja al cerrar" en Configuración > General): `src-tauri/src/lib.rs`
  registra un `TrayIconBuilder` (ícono `icons/icon.ico`, menú "Mostrar
  Project Ciel"/"Salir") e intercepta `WindowEvent::CloseRequested` de la
  ventana `main` — si el ajuste está activo, `api.prevent_close()` +
  `window.hide()` en vez de dejar que la app cierre; `"Salir"` desde el
  menú de la bandeja sí llama a `app.exit(0)`. `WindowEvent::Focused(true)`
  se usa como el único punto donde se avisa "la ventana volvió a
  mostrarse" (cubre menú, click en el ícono, y relanzar una segunda
  instancia con un solo `match`, porque las tres formas de reaparecer
  terminan en `set_focus()`).
- **Notificación de escritorio en mensajes proactivos**: `tauri-plugin-
  notification` nuevo (Rust + `@tauri-apps/plugin-notification`). El
  evento `AppEvent::ProactiveMessageSent` (`crates/eventbus/src/event.rs`)
  ganó un campo `content: String` — antes solo llevaba los ids, sin nada
  para mostrar en el aviso. `src/lib/notifications.ts` pide el permiso de
  notificaciones una sola vez, la primera vez que hace falta (nunca al
  arrancar), y no vuelve a insistir si lo niegan. El sonido lo pone el
  propio Windows al mostrar el toast — no se sumó ningún archivo de audio.
  Toggle propio ("Notificaciones de mensajes espontáneos", prendido por
  defecto) para quien no lo quiera.
- **Voz silenciada mientras está en la bandeja, salvo opt-in**: la síntesis
  de voz corre enteramente en Rust (`AivtTtsProvider`, reproduce por el
  dispositivo de audio directo, no un `<audio>` del navegador), así que el
  freno vive ahí. `AivtTtsProvider` ganó dos `AtomicBool` más
  (`window_hidden`, `voice_while_hidden`) con el mismo patrón que ya tenía
  `muted` — un chequeo extra al principio de `speak_and_capture`, el único
  choke point de cualquier `speak()`. Apagado por defecto, coherente con
  el pedido explícito del usuario de "que no empiece a sonar la voz a
  menos que se configure".
- Los tres ajustes nuevos viven en un solo struct,
  `BackgroundSettings` (`crates/services/settings/src/preferences.rs`,
  `minimize_to_tray`/`notify_proactive`/`voice_while_hidden`), persistido
  sin migración vía el `get_json`/`set_json` genérico que ya usaba
  `ChatAudioSettings`. `commands::settings::settings_set_background`
  actualiza los flags en memoria (`AppState::minimize_to_tray_enabled`,
  `AivtTtsProvider`) al toque, sin pedir reiniciar la app.

**Nota anterior**: exportar Llamada a Chat + fijar/reordenar
conversaciones (2026-09-16), pedido tras el rediseño de la pantalla de
Llamada — dos mejoras relacionadas con Chat, planificadas con
`ExitPlanMode` antes de tocar código dado que tocaban el esquema.

- **Exportar Llamada a Chat**: la transcripción de la Llamada nunca se
  guardaba (vive solo en memoria del frontend, `callStore.feed` — ver la
  pregunta del propio usuario que lo motivó). Botón nuevo en el header de
  `CallPage.tsx` (`MessageSquarePlus`, junto al engranaje, deshabilitado
  sin transcripción) crea una conversación de Chat con esa transcripción
  ya escrita y navega directo a Chat con ella seleccionada — mismo
  comportamiento que "Nueva conversación" ya tenía.
  - **Sin migración**: `conversations.source` ya era una columna `TEXT`
    libre (`0004_chat.sql`) que ya anticipaba `"call"` como valor futuro
    en su propio comentario — solo hizo falta pasar `"call"` en vez de
    `"chat"` al crear la conversación.
  - `ChatService::import_call_transcript` (nuevo,
    `crates/services/chat/src/service.rs`) inserta cada mensaje directo
    vía `MessagesRepository`, sin pasar por el LLM ni por
    `run_generation` — mismo patrón que `send_proactive_message` ya usa
    para una respuesta de IA proactiva ya decidida. Título con
    `title_from_first_message` sobre el primer mensaje de usuario (o
    "Nueva conversación" si no hay ninguno). Comando Tauri
    `chat_import_call_transcript` filtra las líneas `from: "error"` del
    feed antes de mandarlas — el service ni se entera de que existen.
  - En `ConversationList.tsx`, un ícono de teléfono (atenuado) antes del
    título marca las conversaciones con `source === "call"`.
- **Fijar conversaciones + reordenar arrastrando**: migración nueva
  `0017_conversation_pins.sql` (`is_pinned INTEGER`, `pin_order INTEGER`
  — un entero simple, no timestamp, porque el orden es elegido a mano,
  no "fijado más reciente primero"). `ConversationsRepository::
  list_for_profile` ahora ordena `is_pinned DESC, pin_order ASC,
  updated_at DESC`. `set_pinned` pone el recién fijado arriba de todos
  los demás fijados (un valor por debajo del mínimo actual);
  `reorder_pinned` reescribe el `pin_order` completo a partir de la
  lista de ids en el orden final que manda el frontend — simple y
  agnóstico de si el reordenamiento vino de arrastrar o de otra cosa el
  día de mañana.
  - **Arrastrar y soltar sin librería nueva**: `ConversationList.tsx`
    usa atributos HTML5 nativos (`draggable`, `onDragStart`, `onDragOver`,
    `onDrop`) — mismo criterio ya aplicado al Command Palette ("no sumar
    dependencia nueva tipo cmdk"). Solo las filas del grupo "Fijados" son
    arrastrables entre sí; fijar/desfijar sigue siendo una acción
    explícita del menú "⋯", nunca un efecto de soltar en el grupo de
    arriba. Grupo "Fijados" (rótulo nuevo) solo aparece si hay al menos
    un fijado — sin encabezado para el resto, igual que antes.
  - `chatStore.ts` gana `togglePin`/`reorderPinned`, ambos optimistas
    (aplican el cambio local antes de confirmar con el backend, revierten
    si falla) — mismo criterio que `callStore.toggleMute` ya usa.

Verificación: `cargo test --workspace` (98 en `aivt-database`, antes 94;
31 en `aivt-service-chat`, antes 27 — los 8 tests nuevos: crear con
transcripción y titular, título genérico sin mensaje de usuario, fijar
antepone, fijar dos deja el más nuevo arriba, reordenar aplica el orden
dado, desfijar saca del grupo), `cargo build` limpio, `npm run lint` +
`npx vitest run` (209, sin cambios — ningún test de frontend nuevo
cubre `ConversationList`/`CallPage` directamente, la fijación y el
arrastre se probaron a mano). Confirmado en vivo tras recompilar y
relanzar.

**Nota anterior**: **rediseño de la pantalla de Llamada** (2026-09-16),
pedido por el usuario después de terminar el rebrandeo a Project Ciel.
Antes: `CallPage.tsx` mostraba una card de configuración (micrófono,
modo de respuesta, frase de activación, sensibilidad del VAD, tiempo de
silencio) siempre visible al lado de la transcripción — la
configuración competía visualmente con la llamada en sí. Pedido
explícito: que se sintiera como un chat, con un botón central para
iniciar/colgar y toda la configuración detrás de un ícono de engranaje.

- **Header** igual que el de Chat: avatar + nombre del personaje +
  estado de la conexión + `MoodBadge`, y un `IconButton` de engranaje
  (`Settings`, lucide-react) a la derecha que abre `CallSettingsDialog`
  — los mismos campos que antes estaban siempre visibles, ahora en un
  `Dialog` que se autoguarda campo por campo igual que antes (sin botón
  "Guardar" explícito, solo "Cerrar").
- **Transcripción como chat**: `TranscriptBubble` reutiliza la forma y
  el alineado de `MessageBubble` (Chat) — burbujas a la derecha/
  izquierda según quién habla, con la hora. A diferencia de
  `MessageBubble`, sin Markdown/adjuntos/botón de reproducir: una línea
  de transcripción nunca es contenido enriquecido generado por IA, solo
  lo que se escuchó o se dijo.
- **Botón central**: círculo grande (64px) — verde/acento con ícono de
  teléfono para iniciar cuando no hay llamada activa, rojo con
  `PhoneOff` para colgar durante la llamada. El silenciar micrófono
  pasa a un `IconButton` más chico al lado, ya no comparte protagonismo
  con colgar. Arriba del botón, un indicador de una sola línea (punto +
  texto) reemplaza los dos recuadros "Tú"/"Personaje" que había antes:
  un solo punto que pulsa según `CallState` (escuchando/pensando/
  hablando) es más minimalista que dos indicadores separados y dice lo
  mismo.
- **Estado sin personaje activo**: `CallPage` no manejaba este caso
  antes (dependía de que `CallError::NoActiveProfile` apareciera recién
  al intentar generar una respuesta). Ahora sigue el mismo patrón que
  `AvatarPage`/`VoicePage` — `EmptyState` con `call.emptyTitle`/
  `call.emptyDescription` (claves nuevas, en los 6 idiomas) si no hay
  personaje activo, antes de siquiera mostrar el header.
- Claves nuevas en los 6 idiomas: `call.emptyTitle`, `call.emptyDescription`,
  `call.settingsTitle`, y `common.close` (reutilizable para cualquier
  diálogo futuro que solo necesite cerrarse, sin acción de guardar
  explícita). `call.indicators.*` quedaron sin uso (no se borraron —
  bajo riesgo, cero costo en runtime).
- Sin cambios de backend ni de `callStore.ts` — el store ya exponía
  todo lo necesario (`feed`, `callState`, `status`, `isMuted`, etc.),
  el rediseño es puramente de `CallPage.tsx` + las claves de i18n.

Verificación: `npm run lint` (tsc) limpio, 209 tests de frontend en
verde (ningún test cubre `CallPage.tsx` directamente — no había
tests previos de ese componente). Revisado en el navegador vía el
propio servidor de Vite (`localhost:1420`, sin IPC de Tauri disponible
ahí): confirmado que el estado vacío ("Todavía no hay un personaje
configurado") renderiza bien con el texto nuevo y sin errores de
consola propios del componente. La experiencia completa (con Ciel
activo, en llamada real, burbujas de transcripción, diálogo de ajustes)
no se pudo verificar visualmente desde acá — no hay forma de capturar
la ventana nativa de Tauri — así que quedó pendiente de que el usuario
la confirme en la app ya corriendo (el build de depuración usa el
servidor de Vite con HMR, así que el cambio de frontend se refleja solo,
sin recompilar Rust).

**Nota anterior**: **documentación retroactiva de Fase 6 (Llamada IA),
decisiones de alcance y limpieza de texto en español** (2026-09-16). Tres
cosas separadas, pedidas juntas en la misma sesión:

- **Fase 6 — Llamada IA nunca tuvo su propia sección** en este documento
  (ver "Nota posterior" más abajo, dentro de la sección de Fase 9) —
  quedó como deuda de documentación desde que se construyó. Escrita
  ahora, retroactivamente, a partir del código real
  (`crates/services/call`, `src/features/call/CallPage.tsx`): ver
  "## Fase 6 — Llamada IA" más abajo, entre Fase 7 y Fase 5.
- **Decisiones de alcance confirmadas por el usuario** (ver
  "## Próxima fase" al final del documento para el detalle):
  Fase 12 (investigación mobile) se pospone explícitamente hasta tener
  la app ya exportada/empaquetada para distribuirla; la decisión
  Avatar 2D vs 3D (Live2D/VRM) se pospone hasta tener la app
  "bien finalizada". Ninguna de las dos estaba bloqueando nada — ambas
  quedan explícitamente fuera de alcance por ahora, documentado para
  que no se vuelvan a proponer antes de tiempo en una sesión futura.
  Se evaluó además integrar MCP (Model Context Protocol) como cliente
  genérico de herramientas para el personaje (inspirado en la app
  "OpChat") — el usuario decidió explícitamente no construirlo.
- **El nombre de la app cambió a "Project Ciel"** en algún punto de esta
  sesión larga (el usuario lo mencionó de pasada, como contexto de por
  qué el ícono sigue siendo un placeholder — sigue diseñando uno nuevo).
  Encontrado hardcodeado como "AIVT Companion" en el locale español
  (`app.name`, y tres descripciones que lo mencionaban por nombre) —
  corregido ahí. **Sigue diciendo "AIVT Companion" en**: los otros 5
  locales (en/pt/ja/zh/ko), `src-tauri/tauri.conf.json`
  (`productName`/el título de la ventana principal),
  `src-tauri/src/commands/avatar.rs` (título de la ventana del avatar)
  y un log de arranque en `src-tauri/src/lib.rs` — deliberadamente sin
  tocar, ya que ninguno de ellos es texto en español y el usuario no
  pidió el rebrandeo completo, solo dio contexto sobre el ícono. El
  identificador de la app (`"identifier": "com.aivtcompanion.app"` en
  `tauri.conf.json`) tampoco se tocó — cambiarlo movería la carpeta de
  datos del usuario (`AppData/Roaming/com.aivtcompanion.app/`, donde
  vive `aivt.db`) a una ruta nueva, perdiendo acceso a los datos
  existentes sin una migración explícita.

**Actualización dentro de la misma sesión (más tarde el 2026-09-16)**:
el usuario confirmó que quería el rebrandeo **completo**, incluyendo el
`identifier` de Tauri (no solo lo cosmético descrito arriba). Antes de
tocar nada, se lanzó la app para que el usuario exportara a mano el
perfil de Ciel (`.aivtprofile`) como red de seguridad. Hecho eso:
- `tauri.conf.json`: `identifier` `"com.aivtcompanion.app"` →
  `"com.projectciel.app"`, `productName` y el `title` de la ventana
  principal → `"Project Ciel"`.
- `src-tauri/src/commands/avatar.rs`: título de la ventana del avatar
  → `"Project Ciel — Avatar"`.
- `src-tauri/src/lib.rs`: el mensaje de log de arranque (visible en la
  pestaña Actividad) → `"Project Ciel se inició correctamente"`.
- Los 5 locales restantes (en/pt/ja/zh/ko): mismas 4 claves que ya se
  habían corregido en español (`app.name` + 3 descripciones),
  manteniendo "Project Ciel" sin traducir en todos (es un nombre
  propio).
- **Migración de datos**: antes de recompilar, se copió (no se movió)
  toda la carpeta `AppData/Roaming/com.aivtcompanion.app/` a
  `AppData/Roaming/com.projectciel.app/` — no solo `aivt.db`, también
  `attachments/`, `avatars/`, `logs/` y `models/` (los modelos STT/
  embeddings ya descargados). La carpeta vieja se dejó intacta como
  respaldo, no se borró. Confirmado en vivo tras relanzar: la app
  escribe en la carpeta nueva (el WAL de `aivt.db` se actualiza ahí) y
  el perfil de Ciel sigue presente sin haber necesitado el
  `.aivtprofile` exportado — ese export queda como respaldo aparte, no
  hizo falta usarlo.
- **Confirmado que esto NO afecta las API keys guardadas**: el
  `SecretStore` (Windows Credential Manager vía la crate `keyring`)
  usa nombres de servicio fijos y ajenos al identifier de Tauro
  (`"aivt.llm_provider"`, `"aivt.tts_provider"`, `"aivt.twitch"`,
  `"aivt.discord"`, `"aivt.web_search"`, en
  `crates/services/settings/src/{llm,tts,twitch,discord,web_search}.rs`) — cambiar el identifier
  no mueve ni invalida ninguna credencial guardada.
- **Deliberadamente sin tocar**: el nombre del paquete de Cargo
  (`src-tauri/Cargo.toml`: `name = "aivt-companion"`) — el binario
  compilado se sigue llamando `aivt-companion.exe`. Cambiarlo es un
  paso aparte (renombra el `.exe`, afecta cualquier script/documento
  que lo referencie por nombre) que no se pidió en esta pasada.
  Tampoco se tocaron los comentarios de documentación interna en Rust
  que mencionan "AIVT Companion" en prosa (`crates/database/src/
  lib.rs`, `crates/eventbus/src/event.rs`,
  `crates/services/twitch/src/lib.rs`, `crates/core/src/lib.rs`) — son
  solo para quien lee el código, no texto de cara al usuario.
- Verificación: `cargo test --workspace` en verde (sin cambios de
  lógica, solo strings/config), `cargo build` limpio, y confirmación en
  vivo de que la app arranca con el nombre nuevo y los datos intactos.

De paso, pedido en la misma sesión: revisión de **todo el texto en
español de la app** (`src/lib/i18n/locales/es/common.json`) para
neutralizarlo (sin regionalismos) y sintetizar los textos más densos.
Encontrado y corregido voseo rioplatense colado en 6 strings —
`activity.emptyDescription` ("Acá" → "Aquí"),
`settings.appearance.themeDescription` ("Elegí"/"dejalo"/"personalizá"
→ "Elige"/"déjalo"/"personaliza"), `settings.webSearch.apiKeyHint`
("Conseguila" → "Consíguela"), `settings.webSearch.apiKeyPlaceholder`
("Pegá" → "Pega"), `settings.memory.description` ("preguntás" →
"preguntas") y `settings.memory.fusionHint` ("vos decidís" → "tú
decides") — el resto del archivo ya usaba consistentemente la
conjugación "tú", estos eran los únicos residuos. Sintetizados también
los hints más densos que no perdían nada al acortarse:
`twitch.targetChannelHint` (el más largo del archivo, una mini-guía de
varias frases para armar un bot dedicado), `twitch.fields.
ownAccountIgnoredNote`, `settings.privacy.chatAudio.hint` y
`avatar.liveHint`. El resto del archivo ya estaba en un largo
razonable — no se tocó por tocar. Cambio de solo texto, sin tests
nuevos (ninguno depende del texto exacto de estas claves, confirmado
por grep antes de tocarlas); `node -e "JSON.parse(...)"` confirma que
el archivo sigue siendo JSON válido.

**Nota anterior**: **validación en vivo y endurecimiento de la fusión
de memorias** (2026-09-15, mismo día que se construyó — el usuario la
probó de inmediato con datos de prueba reales). La primera pasada
funcionaba en los tests pero fallaba en vivo: con el modelo local que el
usuario tenía cargado en ese momento, `propose_fusion` devolvía siempre
0 grupos. Encontrado y corregido con varias vueltas de prueba directa
contra el servidor local (`curl`, no solo `cargo test`) mientras el
usuario alternaba entre distintos modelos:

- **Detección del modelo cargado**: `AiProvidersSettings.tsx`'s botón
  "Probar conexión" ya llamaba a `/v1/models` y mostraba un desplegable,
  pero solo autocompletaba el modelo si el campo estaba vacío — si el
  guardado ya no coincidía con lo que el servidor tenía cargado (pasó en
  este caso: config decía `qwen3-vl-8b`, el servidor tenía cargado
  `qwen3-1.7b`), el desplegable quedaba en blanco sin avisar. Ahora, si
  el modelo guardado no está entre los detectados, lo corrige
  automáticamente y avisa con un toast.
- **Presupuesto de tokens insuficiente para modelos "thinking"**:
  `FUSION_MAX_TOKENS` (1024) alcanzaba para el JSON de respuesta pero no
  para el razonamiento oculto (`reasoning_content`) de un modelo Qwen3 en
  modo thinking — se gastaba el presupuesto entero pensando y nunca
  llegaba a escribir la respuesta (`finish_reason: "length"`, contenido
  vacío, indistinguible desde `propose_fusion` de "no hay nada que
  fusionar"). Se probó primero apagar el razonamiento con el sufijo
  `/no_think` (convención de facto de Qwen3) — funcionó para el modelo
  chico, pero confirmado en vivo que rompe un modelo *distinto*
  (`qwen3-vl-8b`, no-thinking): con el sufijo, el modelo devolvía
  `{"groups": []}` en ~6 tokens sin comparar nada; sin él, agrupaba bien
  los duplicados reales. Se revirtió el sufijo y en su lugar se subió
  `FUSION_MAX_TOKENS` a 4096 — mismo criterio que `TwitchService`/
  `DiscordService` ya aplican a sus propias respuestas de chat por el
  mismo motivo.
- **Instrucción "sé conservador" insuficiente con varias memorias
  parecidas**: con varios "café" en la lista (algunos genuinamente
  duplicados, otros solo relacionados), un modelo mediano se volvía
  demasiado cauteloso y dejaba de proponer *cualquier* grupo, incluido el
  duplicado real. Se reemplazó esa instrucción por un ejemplo concreto
  positivo/negativo en el prompt (`fusion.rs`), que le da al modelo algo
  contra qué calibrar en vez de una regla abstracta.
- **Salvaguarda nueva e independiente del modelo**: un modelo más chico
  (`qwen2.5-1.5b-instruct`) llegó a proponer — y el usuario llegó a
  aceptar en la revisión — fusionar un `fact` ("Trabaja como diseñadora")
  con una `preference` ("Senderismo de montaña") sin relación real entre
  sí. `propose_fusion` ahora descarta cualquier grupo cuyas memorias
  originales no compartan la misma categoría, sin importar qué proponga
  el modelo — un hecho y una preferencia nunca son "genuinamente lo
  mismo".
- **Validado en vivo con éxito** con `qwen3-14b` cargado: fusionó
  correctamente el único par realmente duplicado de la lista de prueba y
  dejó las demás memorias (relacionadas pero distintas) intactas.

Conclusión práctica: la fusión necesita un modelo de banda alta
(~7-8B en adelante, confirmado con varios modelos locales reales) para
ser confiable — por debajo de eso falla en direcciones opuestas
(demasiado conservador o demasiado permisivo) sin importar el ajuste del
prompt. La salvaguarda de categoría reduce el daño de un modelo débil
pero no sustituye tener uno capaz.

**Nota anterior**: **fusión de memorias asistida por IA** (pedido
directo del usuario, 2026-09-15, ya no ligado a la revisión de UX
anterior). `docs/DEVELOPMENT_STATUS.md`'s Fase 4 documentaba esto como un
gap deliberado ("no algo que un uso normal alcance a corto plazo") — se
construyó igual, antes de que hiciera falta de verdad. Dos decisiones
confirmadas por el usuario antes de programar: **disparador manual**
(botón "Fusionar memorias" en Configuración > Memoria, junto a "Rellenar
memorias antiguas" — nada automático) y **con revisión antes de
aplicar** (la IA solo propone, el usuario ve cada grupo y confirma o
descarta antes de que se borre nada — mismo criterio de seguridad que ya
se usó al agregar confirmación para borrar una sola memoria).

- **Backend** (`crates/memory/src/fusion.rs`, nuevo): mismo patrón
  exacto que `extraction.rs` — prompt dedicado, parseo tolerante a
  prosa (objeto limpio → substring `{...}` → `None`, nunca un error),
  `temperature` forzada baja. Dos métodos nuevos en `MemoryManager`:
  `propose_fusion` (de solo lectura — carga las memorias del perfil, le
  pide al modelo grupos de fusión, valida cada uno contra memorias
  reales, nunca escribe nada) y `apply_fusion` (crea la memoria
  fusionada con `source: "fusion"` y borra los originales, por grupo
  aceptado). Menos de 4 memorias no llega a llamar al modelo. Dos
  comandos Tauri nuevos (`memory_propose_fusion`/`memory_apply_fusion`),
  reutilizando `resolve_llm_connection` (antes privada a
  `commands/chat.rs`, ahora `pub(crate)` para que `commands/memory.rs`
  la use también — mismo criterio ya aplicado a `commands::memory::map_err`
  para `commands::mood`).
- **Frontend**: `MemoryFusionDialog.tsx` (nuevo) — al abrirse pide la
  propuesta y la muestra (memorias originales tachadas → memoria
  combinada propuesta), con un interruptor por grupo para descartarlo
  puntualmente; el botón de confirmar solo aplica los grupos que siguen
  tildados. Card nueva en `MemorySettings.tsx` para abrirlo, mismo lugar
  que "Rellenar memorias antiguas".
- **Límite aceptado, no resuelto**: sin límite de tamaño de lote — se
  manda la lista completa de memorias del perfil en un solo prompt.
  Aceptable porque, como ya documentaba la idea original, esto no es
  algo que un uso normal alcance a corto plazo.

6 tests nuevos en `fusion.rs` + 8 en `service.rs` (Rust) y 5 en
`MemoryFusionDialog.test.tsx` (frontend). `cargo test --workspace` en
verde, lint limpio, 209 tests de frontend pasan (subieron de 204).

**Nota anterior**: **command palette (Ctrl+K)** (última idea de la
misma revisión de UX, 2026-09-15). La app no tenía ningún atajo de
teclado ni búsqueda global — con 9 pestañas en el sidebar + 8
sub-pestañas de Configuración, saltar entre secciones lejanas exigía
varios clicks. Construido a mano sobre `@radix-ui/react-dialog` (ya
dependencia, base de `Dialog.tsx`) — no se sumó ninguna librería nueva
tipo `cmdk`.

- `src/app/CommandPalette.tsx` (nuevo): `Ctrl+K`/`Cmd+K` global (listener
  a nivel `window`, funciona en cualquier pestaña incluido modo
  inmersivo) abre un buscador anclado cerca del tope de la pantalla (no
  el `DialogContent` centrado compartido, pensado para diálogos de
  confirmación, no para un palette). Filtro por substring simple sobre
  la etiqueta ya traducida de cada comando, navegación con flechas +
  Enter, click en cualquier fila, Escape cierra (nativo de Radix).
- **Sin datos duplicados**: la lista de comandos sale de reexportar lo
  que ya existía — `NAV_GROUPS` (`Sidebar.tsx`) para las 9 pestañas del
  sidebar, `SECTIONS`/`SECTION_ICONS` (`SettingsPage.tsx`, antes
  privados al archivo) para las 8 de Configuración. Ninguna traducción
  nueva para los nombres de los comandos — solo 2 claves nuevas
  (`commandPalette.placeholder`/`noResults`) en los 6 idiomas.
- **Saltar directo a una sub-pestaña de Configuración desde afuera**:
  `SettingsPage.tsx` guardaba `activeSection` en un `useState` puramente
  local, sin forma de fijarlo desde otro componente. `navigationStore.ts`
  gana `pendingSettingsSection`/`openSettingsSection`/
  `clearPendingSettingsSection`; `SettingsPage` lee ese campo en un
  `useEffect` y salta ahí una sola vez, sin quedar "pegado" a esa
  sección en visitas futuras.
- Límite aceptado, no resuelto: si el palette se abre con otro diálogo
  ya abierto (ej. una confirmación de borrado), queda apilado encima en
  vez de bloquearse — caso borde poco probable, no valía la complejidad
  de prevenirlo.

Nuevo `src/app/__tests__/CommandPalette.test.tsx` (8 tests: abre/cierra
con Ctrl+K, filtra al escribir, "sin resultados", click y Enter navegan,
una sección de Configuración deja `pendingSettingsSection` seteado,
Escape cierra). Lint limpio, 204 tests de frontend pasan (subieron de
196). Cambio puro de frontend.

Con esto se completó toda la lista de mejoras de calidad de vida
identificada en la revisión de UX de esta sesión (2026-09-14/15).

**Nota anterior**: **timestamps por mensaje** (misma revisión de UX,
2026-09-15). Ningún feed (Chat, Twitch, Discord, Llamada) mostraba cuándo
se envió cada mensaje. El dato ya existía en los tres stores de
plataforma (`TwitchFeedMessage`/`DiscordFeedMessage`/`CallFeedMessage`
ya tenían un campo `at: string`, cargado al recibir el evento) y en
`Message.createdAt` del lado de Chat — era puramente un tema de que no se
renderizaba, no faltaba plumbing.

- Nuevo `formatTime(isoString, locale)` en `src/lib/formatDate.ts` — solo
  hora ("14:32"), sin fecha, para no repetir el día completo en cada fila
  de un feed en vivo (a diferencia de `formatDateTime`, ya usado en
  Memoria). Devuelve `""` en vez del string crudo si el valor no
  parsea, ya que esto se renderiza inline junto a cada mensaje.
- `MessageBubble.tsx`: hora chica en la esquina de cada burbuja, en la
  misma fila que el botón de "hablar" cuando está presente.
- `TwitchPage.tsx`/`DiscordPage.tsx` (`ChatFeedCard`) y `CallPage.tsx`
  (`TranscriptCard`): hora chica antes del nombre en cada línea del feed.

2 tests nuevos en `formatDate.test.ts`. Lint limpio, 196 tests de
frontend pasan (subieron de 194). Cambio puro de frontend.

**Nota anterior**: **"Apariencia" se mudó al sidebar principal** (misma
revisión de UX, 2026-09-15). El tema visual por personaje (ADR-009) vivía
en Configuración > Apariencia, pero es config *por personaje* — la misma
categoría que Identidad/Voz/Avatar, que ya viven en el grupo "Personaje"
del sidebar — no un ajuste global de la app como el resto de
Configuración. El propio comentario en el código decía "movido acá desde
Identidad", así que este es en el fondo el mismo movimiento, terminado.

- Archivos movidos de `src/features/settings/` a un `src/features/
  appearance/` nuevo: `AppearanceSettings.tsx` → `AppearancePage.tsx`
  (renombrado para seguir la misma convención que `IdentityPage`/
  `VoicePage`/`AvatarPage`) y `ThemePresetPicker.tsx` (sin cambios de
  lógica). `AppearancePage` ya se auto-alcanzaba el perfil activo desde
  `useProfileStore` (no recibía `profile` como prop) — se le sumó el
  mismo chequeo de `isLoading` que ya tienen sus nuevos hermanos, para no
  mostrar "no hay personaje" de más mientras los perfiles todavía están
  cargando al arrancar la app.
- `navigationStore.ts` (`SIDEBAR_TABS`), `Sidebar.tsx` (nueva entrada en
  el grupo "Personaje", ícono `Palette`) y `App.tsx` (`PAGES`) actualizados
  para que "Apariencia" sea una pestaña de primer nivel.
- `SettingsPage.tsx` pierde la sección "appearance" — vuelve a 8
  sub-pestañas.
- i18n: nueva `sidebar.appearance` (mismo texto que la vieja
  `settings.tabs.appearance`, que se eliminó) + `settings.appearance.
  subtitle`/`loading` nuevas, en los 6 idiomas.

Lint limpio, 194 tests de frontend pasan. Cambio puro de frontend, sin
tocar Rust.

**Nota anterior**: **mood visible durante la conversación** (misma
revisión de UX, 2026-09-15). `MoodBadge` (el indicador de "cómo está" el
personaje ahora mismo, distinto de su personalidad fija) solo vivía en
Identidad > Personalidad — para verlo mientras se hablaba de verdad con el
personaje había que salir de la conversación. Cambio puramente de
ubicación, sin tocar `MoodBadge` ni `moodStore` (ya se autoactualiza vía
`AppEvent::MoodUpdated`, así que aparece solo en los lugares nuevos sin
plumbing extra):
- `ChatWindow.tsx`: el badge aparece en el header, junto al nombre del
  personaje.
- `CallPage.tsx`: el badge aparece junto al badge de estado del servicio
  en `PolicyCard`, leyendo el perfil activo de `useProfileStore` (esta
  pantalla no recibía `profile` como prop hasta ahora).

Lint limpio, 194 tests de frontend pasan (sin cambios de cantidad —
reutiliza un componente ya probado).

**Nota anterior**: **botón "Probar" en Configuración > TTS** (misma
revisión de UX, 2026-09-15). A diferencia de Proveedores IA, TTS no tenía
ninguna forma de confirmar que una conexión funciona antes de guardarla —
había que guardar a ciegas e ir a la pestaña Voz de un personaje a probar
ahí. Complicación real encontrada al implementarlo: el motor genérico HTTP
no tiene ningún endpoint de "solo verificar conexión" como sí lo tiene
LM Studio (`/v1/models`) — la única forma real de probarlo es pedirle que
sintetice algo, y eso reproduce audio de verdad por los parlantes. Se le
preguntó al usuario y eligió esa opción (reproducir audio de prueba)
en vez de dejar ese backend sin ninguna verificación.

- **Backend**: nuevo comando `settings_test_tts_connection`
  (`src-tauri/src/commands/settings.rs`), mismo contrato "prueba lo
  tipeado, no lo guardado" que `settings_test_llm_connection`. Para
  ElevenLabs llama a `state.tts.list_voices(&TtsBackend::ElevenLabs {..})`
  (el `GET /v1/voices` real, ya existente, reutilizado tal cual — sin
  crear un motor nuevo) y devuelve la cantidad de voces encontradas. Para
  HTTP arma un `SpeakRequest` con una frase de prueba (mandada desde el
  frontend ya traducida) y llama a `state.tts.speak(...)` — reproduce el
  audio de verdad por el `AudioPlayer` compartido. Nativo no tiene botón
  de prueba (nada que probar).
- **Frontend**: `TtsSettings.tsx` gana un botón "Probar conexión" (visible
  para ElevenLabs/HTTP, oculto para Nativo) + un Badge Conectado/Sin
  conexión en el header, mismo patrón visual que `AiProvidersSettings.tsx`.
  Tras un test exitoso muestra "N voces encontradas" (ElevenLabs) o un
  aviso de que deberías haber escuchado un audio (HTTP), para que reproducir
  sonido sin aviso previo no se sienta como un bug.

Compila (`cargo build`/`cargo test --workspace` en verde, sin tests nuevos
de Rust — el comando es un passthrough sin lógica propia que no esté ya
cubierta por `TTSProvider`/`AudioPlayer`), lint limpio, 194 tests de
frontend pasan.

**Nota anterior**: dos ajustes chicos de UX, ambos de la misma revisión
de calidad de vida (2026-09-15):

- **Reordenar Twitch/Discord**: en las dos pestañas, la tarjeta de
  "Conexión" (conectar la cuenta) aparecía DEBAJO de la de "Política"
  (comportamiento + iniciar/detener) — alguien abriendo la pestaña por
  primera vez, sin cuenta conectada, veía primero ajustes que ni podía
  usar todavía. `TwitchPage.tsx`/`DiscordPage.tsx`: se invirtió el orden
  (Conexión → Probar respuesta → Política) sin tocar el contenido de
  ninguna tarjeta.
- **Confirmación al borrar una memoria**: borrar desde `MemoryPage.tsx`
  pasaba directo, sin ningún paso intermedio, a diferencia de Chat
  (conversaciones) y Actividad (limpiar), que ya piden confirmación por
  el mismo riesgo de pérdida de datos. Ahora el ícono de borrar abre un
  `ConfirmDialog` (mismo componente ya usado en Identidad/Chat/
  Actividad) antes de llamar a `remove()`. 2 tests nuevos en
  `MemoryPage.test.tsx` (mismo patrón que los de
  `ConversationList.test.tsx`: no borra hasta confirmar, confirmar sí
  borra) + claves i18n nuevas en los 6 idiomas.

Lint limpio, 194 tests de frontend pasan (subieron de 192).

**Nota anterior**: **unificación del guardado hacia autoguardado**
(post-Fase 11, pedido directo del usuario tras una revisión de UX de toda
la interfaz, 2026-09-14). La app mezclaba dos convenciones de guardado sin
criterio (botón "Guardar" explícito + estado "sucio" en Identidad, Voz,
Proveedores IA y TTS, vs. autoguardado silencioso por campo en el resto:
Llamada, Twitch, Discord, Apariencia) — riesgo real de que alguien
acostumbrado al autoguardado de otra pestaña diera por guardado un cambio
en Identidad/Voz y lo perdiera al navegar. Se extendió el patrón de
autoguardado (ya mayoritario) a las 4 pantallas restantes en vez de al
revés. `WebSearchSettings.tsx` no necesitó ningún cambio — ya seguía la
forma correcta.

- Nuevo hook compartido `src/lib/useAutosave.ts`: autoguarda un valor
  700ms después de que deja de cambiar (cubre tanto texto libre como
  sliders, que disparan `onValueChange` de forma continua mientras se
  arrastran), sin guardar nunca el valor recién cargado del backend
  (al montar, o cuando `enabled` pasa de `false` a `true` tras un fetch
  async), y con un `flush()` que se dispara solo, sin llamado manual,
  al desmontar el componente — cubre switchear de pestaña o de personaje
  a mitad de una edición sin perderla. Sin cambios de backend: las 4
  funciones de guardado ya existentes siempre recibían y persistían el
  objeto completo, nunca un patch parcial.
- **Identidad y Voz**: `IdentityEditor.tsx`/`VoiceEditor.tsx` pierden el
  botón "Guardar" + estado "sucio" del header, reemplazado por un
  indicador chico "Guardando…" transitorio. Ambos ya estaban keyeados por
  `profile.id` (`IdentityPage.tsx`/`VoicePage.tsx`), así que cambiar de
  personaje ya remonta el componente entero — el `flush()` de
  desmontaje cubre ese caso solo, sin lógica adicional.
- **Configuración > Proveedores IA y TTS**: los campos no-secretos
  (URL, modelo, temperatura, tokens, visión / backend) autoguardan; la
  API key mantiene su propio guardado explícito pero con un botón
  "Guardar" chico junto al campo en vez del botón grande de la página —
  mismo patrón que ya usaba Twitch para "Canal objetivo" — porque un
  valor vacío ahí significa "no tocar la key guardada" y autoguardar
  cada tecla escribiría claves parciales al secret store del SO mientras
  se está escribiendo. "Probar" (conexión) sigue siendo una acción manual
  aparte en ambas, sin cambios.
- Nuevo `src/lib/__tests__/useAutosave.test.ts` (5 tests: no guarda el
  valor inicial, debounce agrupa cambios seguidos, `flush()` guarda ya
  mismo y cancela el timer pendiente, no guarda el valor recién cargado
  tras un `enabled` que pasa a `true`, un error de guardado llega a
  `onError` sin romper el hook). Lint limpio, 192 tests de frontend
  pasan (subieron de 187).

**Nota anterior**: **limpieza de pestañas de Configuración** (post-Fase
11, pedido directo del usuario, 2026-09-14). Se sacaron de la barra
lateral las 4 pestañas que eran placeholders sin ninguna implementación
detrás ("Integraciones", "Almacenamiento", "Avanzado", "Desarrollador") —
se agregará una pestaña nueva si hace falta en el futuro, no antes.
Cambio puramente de frontend: `SettingsPage.tsx` simplificado (ya no
necesita el mecanismo `REAL_SECTIONS`/fallback — todas las secciones
restantes tienen componente real, así que `SECTION_COMPONENTS` pasó de
`Partial<Record<...>>` a `Record<...>`), y los dos componentes que
quedaron sin ningún uso al sacar esas pestañas
(`PlaceholderSettingsSection`, `PlaceholderPage`) se eliminaron junto con
sus claves i18n en los 6 idiomas. Lint limpio, 187 tests de frontend
pasan.

**Nota anterior**: **Fase 11 — Polish**, ahora completa. Además del tab
de Actividad y el `ThemeEngine` por perfil (los dos ítems ya anotados de
entrada para esta fase), sobre la marcha de la misma sesión el usuario
pidió tres arreglos de UX relacionados: confirmación al borrar una
conversación de Chat (con extracción opcional de memorias antes de
borrar), renderizado de Markdown en las burbujas de Chat, y que la voz ya
no lea Markdown/URLs en voz alta. Se encontró y corrigió además, de paso,
un bug de la animación de los diálogos de confirmación (se veían
brevemente descentrados al abrirse). Todo implementado y verificado en
vivo en la misma sesión — `cargo test --workspace` en verde y 107 tests
de frontend pasan. Detalle completo en la sección de esta fase más abajo.

**Nota anterior**: **Fase 10b — Búsqueda web / tool-calling** (nuevo
mecanismo genérico de tool-calling en `aivt-orchestrator` +
`aivt-provider-llm`, con dos herramientas reales: `web_search` (Brave
Search API) y `get_current_time`). Aplica a los cuatro canales: Chat,
Twitch, Discord y Llamada. **Verificado en vivo en los cuatro canales**
(Chat: fecha, hora en otro país, clima; Llamada: cambio de moneda,
búsqueda de una persona; Twitch: hora de atardecer en otra ciudad;
Discord: varias búsquedas de persona) y con la búsqueda desactivada (el
toggle de opt-in saca `web_search` de las herramientas ofrecidas sin
tocar nada más), tras resolver varios problemas reales encontrados en
sesiones de prueba, incluyendo dos bugs de reconexión — uno en Discord,
uno en Twitch — donde una respuesta de tool-calling lenta bloqueaba el
loop de escucha lo suficiente como para que la plataforma cerrara la
conexión, y el código trataba ese cierre como fatal en vez de recuperarse
solo. De paso se encontró y corrigió un hallazgo de seguridad no
relacionado (una API key quedaba en texto plano en el log). Ver la
sección de esta fase más abajo para el detalle completo.

**Nota anterior**: **Fase 10 — Discord con voz, abandonada** (Milestone A
—unirse/salir de un canal con `!join`/`!leave`— se completó y se verificó
en vivo, pero Milestone B quedó bloqueado por un bug abierto y sin
resolver en `songbird` combinado con el cifrado DAVE que Discord exige
desde marzo de 2026 para toda conexión de voz; se decidió con el usuario
abandonar la función en vez de parchear un problema ajeno sin ETA. Todo el
código de voz fue removido — Discord vuelve a texto-only. Compila, 270
tests de backend + 93 de frontend pasan. Ver la sección de esta fase más
abajo para el detalle completo).

**Nota anterior**: trabajo posterior a Fase 9 — TTS ElevenLabs,
guardado/reproducción de audio en Chat, y Client ID único de Twitch
(compila, 270 tests de backend + 93 de frontend pasan; **verificado en
vivo por el usuario** — ver la sección "Mejoras posteriores a Fase 9" más
abajo).

**Nota anterior**: Fase 8b — Twitch, cuenta de bot propia (compila, 257
tests de backend + 93 de frontend pasan; **verificada en vivo por el
usuario** con una segunda cuenta de Twitch real hecha moderadora en el
canal principal).

**Nota anterior**: Fase 9 — Discord (texto) (compila, 247 tests de backend
+ 93 de frontend pasan; **verificada en vivo por el usuario** con un bot
real respondiendo en un servidor de Discord de prueba — ver "Problemas
encontrados y corregidos" para el recorrido de bugs que hizo falta
resolver).

**Nota posterior**: Fase 6 (Llamada IA) y Fase 7 (Avatar) se construyeron y
se verificaron en vivo con el usuario después de esto — deuda de
documentación resuelta el 2026-09-16, ver "## Fase 6 — Llamada IA" y
"## Fase 7 — Avatar" más abajo. La transcripción de voz (STT) de Fase 6 arrancó
hablando por HTTP con un servidor `speaches` en Docker; se reemplazó por
completo por whisper.cpp embebido (`aivt-provider-stt`, vía `whisper-rs`,
CPU-only) — decisión explícita del usuario para vender la app sin pedirle
Docker al cliente final (ver docs/architecture.md §6). Ya no existe ningún
concepto de servidor/API key para STT: el modelo GGML se descarga una vez
desde Configuración > Transcripción de voz y todo corre sin conexión.

## Proveedores multi-servicio, voz, dictado y ajustes de interfaz (2026-09/10)

Trabajo posterior a Fase 11, pedido y probado en vivo por el usuario a lo largo
de varias sesiones. Las decisiones de diseño están en
[ADR-010](ADR/010-multi-provider-and-failover.md) (proveedores y cambio
automático), [ADR-011](ADR/011-voice-dictation.md) (dictado por voz) y
[ADR-012](ADR/012-profile-archive-format.md) (formato `.aivtprofile`) y
[ADR-013](ADR/013-memory-scope-and-shared-channels.md) (alcance de la memoria entre
canales) y
[ADR-014](ADR/014-full-app-backup.md) (copia de seguridad completa) y
[ADR-015](ADR/015-diagnostics-center.md) (centro de diagnóstico). Todo lo de abajo está confirmado por el usuario salvo lo
que figura en "Pendiente / sin verificar".

### Proveedores de IA (LLM) — varios a la vez, con cambio automático

- **Modelo de datos** (`crates/services/settings/src/llm.rs`): lista de
  proveedores (`llm_providers_v1`) con `enabled` ("activar"), `pinned`
  ("fijar") y un puntero `current_provider_id`; una key por proveedor
  (`api_key.{id}`). Migra sola desde la vieja conexión única. Tipos: OpenAI,
  Anthropic, Gemini, Groq, OpenRouter, Cerebras, Mistral, DeepSeek, Together,
  xAI, SambaNova y "Servidor local" (genérico, con URL a mano). Los que hablan
  el protocolo de OpenAI llevan su URL fija (`LlmProviderKind::openai_compatible_base_url`),
  sin adaptador propio; Anthropic y Gemini sí tienen adaptador
  (`crates/providers/llm/src/{anthropic,gemini}.rs`).
- **Router** (`crates/providers/llm/src/failover.rs`, `FailoverLLMProvider`):
  pool = proveedor en uso primero, luego los activos y fijados que puedan
  responder (`is_usable_fallback`: con modelo y, si son de nube, con key). Solo
  mira el **primer** fragmento: nunca cambia a mitad de una respuesta.
  - El proveedor **en uso** cede el turno si `ProviderError::should_fail_over()`:
    sin crédito, límite de solicitudes, servidor caído, timeout, error 5xx, o
    sin primer fragmento en 45 s (`DEFAULT_FIRST_CHUNK_TIMEOUT`). Una key
    inválida o un modelo inexistente se muestran tal cual.
  - Los **de reserva** se saltan ante cualquier fallo.
  - Si nadie responde se muestra el error del proveedor en uso, y el "en uso"
    no se mueve. El aviso `AppEvent::LlmProviderSwitched` sale una sola vez,
    cuando un reemplazo de verdad respondió.
  - **Preferido y recuperación (2026-10-02):** el cambio automático mueve solo el
    proveedor *en uso*; el *preferido* (el que el usuario eligió) se conserva y se
    prueba primero en cada petición, así que se vuelve a él solo cuando responde de
    nuevo. Un proveedor que falló pasa a pausa un rato (de 1 a 10 min según el
    motivo) para no esperarlo en cada mensaje. El aviso lleva el motivo
    (`FailoverReason`) y la interfaz muestra un aviso claro; el preferido se marca
    con "Preferido" cuando hay uno temporal en uso. Ver ADR-010.
  - Cada llamada al LLM dentro de una misma respuesta (p. ej. la consolidación de
    memoria) vuelve a probar primero el proveedor en uso del momento en que se
    armó el pool: un error rápido de más, inofensivo; el aviso ya no se repite.
- Los 6 sitios que armaban el proveedor a mano (chat y memoria, llamada, Discord,
  Twitch ×2 —respuestas y subtítulos de emotes—, proactivo) usan `SettingsService::build_llm_provider`.
- "Probar conexión" usa la key guardada si el campo está vacío
  (`settings_test_llm_provider` recibe el `id`).

### Motor de voz (TTS) — varios a la vez, sin rotación automática

- Mismo patrón (`tts_providers_v1`), con "Nativo" siempre presente. Tipos:
  Nativo, ElevenLabs, OpenAI, Google Cloud, Azure Speech, Fish Audio, Deepgram,
  Groq, Together y servidor genérico.
- Adaptadores nuevos: `google.rs`, `azure.rs`, `fish.rs`, `deepgram.rs`;
  Groq/Together reutilizan `HttpEngine::synthesize_preset` (URL, modelo, voz,
  formato y tope por petición propios). Groq (Orpheus) acepta 200 caracteres
  por petición y solo WAV: `chunking.rs` parte el texto por frases y une los
  WAV. Cada entrada puede llevar un `model` (Fish Audio, Groq, Together); Fish
  Audio usa por defecto el modelo gratuito `s2.1-pro-free` (hasta 2026-11-30,
  política de uso justo, puede usar las peticiones para mejorar su modelo).
- Fish Audio: selector con voces propias + 50 populares, y buscador por nombre
  (`voice_search_voices`). Las demás voces se pegan por ID.
- Parar la voz cancela también una síntesis remota en curso (`epoch` en
  `AivtTtsProvider`) y siempre intenta parar la nativa y el audio remoto. En el
  chat, el botón del altavoz es "stop" mientras el motor suena o genera, aunque
  haya una copia guardada del audio.

### Llamada: historial de la sesión

- Antes cada turno de la llamada llegaba al modelo como si fuera el primero: el
  personaje no recordaba lo dicho diez segundos antes. Ahora `CallService` guarda
  en memoria los últimos 20 mensajes (del usuario y del personaje) de la llamada en
  curso (`crates/services/call/src/history.rs`, `CallHistory`) y los envía en cada
  turno; se vacía al empezar y al colgar, y no se guarda en la base de datos. Lo
  dicho por el usuario cuenta aunque su turno se interrumpiera.

### Memoria compartida entre canales, con alcance (2026-10)

- Antes solo el Chat leía la memoria y el estado de ánimo; Llamada, Twitch y Discord
  respondían sin ningún recuerdo. Ahora cada memoria tiene un `scope`
  (`private`/`public`, migración `0019`) y los canales leen según su privacidad. Ver
  ADR-013.
- **Llamada**: lee toda la memoria y el estado de ánimo (es una charla con el dueño).
  **Twitch y Discord**: solo memorias públicas, sin estado de ánimo.
- Toda memoria nace privada (también las fusionadas, salvo que todas las originales
  fueran públicas). Pasarla a pública es una decisión explícita: botón globo/candado en
  la pestaña Memoria, con confirmación al hacerla pública. Comando
  `memory_set_scope`; `MemoryManager::search_public` filtra en la propia consulta SQL.
- Pruebas: 111 en `aivt-database` (+5 de alcance), 65 en `aivt-memory` (+4), 2 nuevas del
  historial de la Llamada, 234 de frontend (+5). Workspace completo en verde.
- **Sin verificar en vivo**: que la Llamada recuerde de verdad (decir un dato en el Chat y
  preguntarlo por voz), y que Twitch/Discord solo usen lo marcado como público.

### Copia de seguridad completa (2026-10)

- Configuración > Copia de seguridad: crea un `.aivtbackup` (ZIP con la base, adjuntos y
  avatares) y restaura desde uno. Crate nuevo `aivt-service-backup`; comandos
  `backup_create`, `backup_prepare_restore`, `backup_cancel_restore`, `backup_restart_app`.
  Ver ADR-014.
- La base se copia con `VACUUM INTO` y se comprueba `PRAGMA integrity_check` antes de
  empaquetar; el ZIP escrito se relee. **No incluye API keys ni tokens** (almacén seguro
  del sistema): al restaurar en otro equipo hay que volver a escribirlos; la pantalla lo avisa.
- Restaurar es en dos pasos: validar y preparar, y aplicar **al reiniciar**, antes de abrir
  la base. Lo anterior se mueve a `pre-restore/<fecha>/`, nunca se borra. Se rechazan copias
  de una app más nueva, ZIP ajenos (p. ej. un `.aivtprofile`), rutas peligrosas y bases dañadas.
- Pruebas: 12 en el crate de copia, 8 de frontend. Verificado con una copia de los datos reales
  del usuario en carpeta temporal (53 MB, 49 archivos; mismas filas tras restaurar).
- **Sin verificar**: restaurar de punta a punta en la app (reinicio incluido), y desde un
  instalador real.

### Centro de diagnóstico (2026-10)

- Configuración > Diagnóstico: estado general, comprobaciones explicadas (base de datos, proveedores
  de IA y su pausa, voz, servicios, modelos, errores recientes, espacio recuperable), detalle por
  sección y botones para volver a comprobar, copiar o guardar el informe como texto. Crate
  `aivt-service-diagnostics`; comandos `diagnostics_run`, `diagnostics_text`, `diagnostics_save`. Ver ADR-015.
- El informe nunca lleva secretos (solo "tiene key: sí/no") y el texto pasa por `redact_secrets`.
- Las comprobaciones son códigos que la interfaz traduce a los 6 idiomas.
- Pruebas: 16 en el crate, 7 de frontend; workspace Rust 638 en verde, frontend 249.
- Aprovechando, se corrigió un fallo intermitente de la restauración de copias: en Windows un
  archivo recién cerrado puede seguir bloqueado un instante y el renombrado fallaba (sharing violation);
  ahora reintenta unos segundos antes de rendirse (y deshace si no puede).
- **Sin verificar**: ver la pantalla con datos reales en la app.

### Dictado por voz en el chat

- Botón de micrófono junto al de enviar (que ahora es solo el icono): mantener
  pulsado graba; deslizar hacia arriba lo fija; un toque corto también; "Detener"
  transcribe y deja el texto en el cuadro (o lo envía solo si
  Configuración > Transcripción de voz > "Enviar automáticamente lo dictado").
  Escape cancela. Mientras se graba se ve el texto provisional, que se
  re-transcribe cada ~1,5 s (`dictation_partial`, solo los últimos 25 s); la
  pasada final usa todo el audio.
- Backend en `CallService` (`dictation_start/stop/cancel`): reutiliza el
  micrófono y Whisper de Llamada; rechaza si hay una llamada en curso o no hay un
  modelo descargado. Tope de 5 min. No transcribe audio casi en silencio y
  quita las etiquetas entre corchetes que Whisper inventa (`[Música]`).
- Frontend: `src/features/chat/useDictation.ts`.

### Perfiles: exportar / importar completos

- `.aivtprofile` es ahora un ZIP (perfil, avatares, chats y adjuntos opcionales);
  los `.aivtprofile` viejos en JSON plano siguen importando.
- **Endurecimiento de la importación (2026-10-02)**: una revisión externa pidió auditar
  *Zip Slip* y bombas zip, y al hacerlo se encontró un fallo real: `hash` y
  `extension` de cada adjunto salen del `chats.json` del archivo importado y se
  usaban tal cual para armar la ruta en disco, así que un `.aivtprofile` malicioso
  podía escribir un archivo con contenido y extensión a elección fuera de la
  carpeta de adjuntos (`../`). Corregido en `export.rs` (`is_safe_attachment_name`:
  hash hexadecimal, extensión alfanumérica corta; si no, el archivo se rechaza
  entero) y, como defensa en profundidad, otra vez en
  `ProfileService::write_attachment_file_if_missing`. Además cada entrada del ZIP
  tiene un tope de 64 MB y el total descomprimido de 1 GB. Con pruebas del archivo
  malicioso. Las rutas de las entradas del ZIP nunca se usan para escribir: solo
  se buscan por nombre.
- Pendiente: ícono propio para `.aivtprofile` (hace falta una plantilla NSIS
  propia y una build instalada para probarlo) y el doble clic para importar,
  que solo funciona con la app instalada.

### Interfaz

- Acento de fábrica **#96fff3** (`globals.css`, `DEFAULT_THEME_TOKENS`,
  `DEFAULT_ACCENT_HEX`; texto oscuro encima). Los mensajes de la IA llevan el
  acento y los del usuario la superficie neutra; los enlaces de la IA usan el
  color de texto del acento.
- Los botones de adjuntar, micrófono y enviar usan el radio del tema
  (`rounded-lg`) y siguen el ajuste de redondez.
- Los desplegables (`Select`, `DropdownMenu`, `Popover`) se limitan al espacio
  libre de la ventana y hacen scroll por dentro.
- Clic derecho > Pegar dentro de un diálogo ya no cierra el diálogo.
- Textos de todo lo anterior en los 6 idiomas (es, en, pt, ja, ko, zh).

### Pantalla de bienvenida, Créditos y selector de color (no documentados antes)

- **Pantalla de bienvenida** (`src/features/onboarding/`): asistente de 8 pasos
  (`intro`, idioma, apariencia, proveedor de IA, transcripción de voz, motor de
  voz, personaje y final) que se muestra mientras `onboardingCompleted` sea
  `false` (`settingsStore`, `settings_get_onboarding_completed`). Los pasos de
  apariencia, IA, STT y TTS reutilizan tal cual los componentes de
  Configuración (autoguardan); el paso de personaje se engancha al botón
  "Continuar" (`registerSubmit`) para que no se pueda avanzar sin guardarlo. Se
  ve como la ventana de Configuración, con transición lateral entre pasos y
  "Omitir todo". El ítem temporal del clic derecho con Shift ("Ver pantalla de
  bienvenida (test)") la fuerza solo en memoria.
- **Configuración > Créditos** (`CreditsSettings.tsx`): nombre y versión de la
  app (API de Tauri) y el autor con un enlace externo (`openExternalUrl`).
- **Selector de color propio** (`src/design-system/ColorPicker.tsx`): HSV con
  cuadro de saturación/brillo, tono y campo hexadecimal, dentro de un `Popover`;
  reemplaza al `<input type="color">` nativo. Se corrigió un bucle infinito real
  (un efecto secundario dentro de un `setState` mientras se arrastraba).

### Comandos y claves reemplazados (para leer las fases anteriores)

Las secciones de Fase 2, 3 y 5 hablan de la conexión única; hoy:

| Antes | Ahora |
|---|---|
| `settings_get/set/test_llm_connection`, clave `llm_connection` | `settings_list/add/update/remove_llm_provider`, `settings_set_llm_provider_enabled/pinned`, `settings_set_current_llm_provider`, `settings_test_llm_provider`; clave `llm_providers_v1` |
| `settings_get/set/test_tts_connection`, clave `tts_connection` | `settings_list/add/update/remove_tts_provider`, `settings_set_tts_provider_enabled`, `settings_set_current_tts_provider`, `settings_test_tts_provider`; clave `tts_providers_v1` |
| Secreto único `api_key` / `elevenlabs_api_key` | Un secreto por proveedor: `api_key.{id}` |
| `llm_connection()`/`llm_api_key()` | Siguen existiendo como vista de solo lectura del proveedor en uso |

Las claves viejas quedan sin borrar tras migrar. Otras claves nuevas:
`dictation_settings` (envío automático del dictado).

### Pendiente / sin verificar

- Sin probar con claves reales: el cambio automático con Anthropic y Gemini, y
  Together, xAI, SambaNova, Mistral, DeepSeek, Deepgram y la voz de Groq/Together.
  Las URLs de SambaNova, Mistral y DeepSeek vienen de conocimiento previo, no de
  la documentación consultada.
- Probado en vivo con servicios reales: LM Studio apagado → salta a Groq.
- Instalador: personalización (imágenes lateral y de cabecera, idiomas, plantilla
  NSIS) y el ícono de `.aivtprofile`, aplazados por el usuario.
- Quitar el ítem temporal "Ver pantalla de bienvenida (test)" de
  `src/app/AppContextMenu.tsx` cuando el usuario lo indique.
- La carpeta de compilación (`CARGO_TARGET_DIR=C:/cgt-aivt`) llega a ~85-100 GB.

## Fase 11 — Polish (completa)

Pedido del usuario tras cerrar Fase 10b: seguir con el roadmap
(`docs/architecture.md` §7), que marca **Fase 11 Polish** como lo
siguiente. De los dos ítems ya anotados para esta fase en la
documentación (tab de Actividad, `ThemeEngine` por perfil), el usuario
eligió el tab de Actividad primero; sobre la marcha, en la misma sesión,
pidió además tres arreglos de UX relacionados (confirmación al borrar en
Chat, Markdown en el Chat, que la voz no lea Markdown/URLs), todos
implementados y verificados en vivo junto con un bug encontrado de paso
(animación de diálogos descentrada).

### Tab de Actividad — completo

El backend (`aivt-service-activity`, tabla `activity_events`) llevaba
desde Fase 0 recibiendo eventos reales vía el puente `tracing` →
`ActivityLayer` (cualquier `tracing::info!/warn!` con `target: "activity"`
en Chat/Twitch/Discord/LLM/Memoria), con `ActivityRepository::recent`/
`ActivityService::recent` ya implementados y probados — nadie los leía
todavía (`AppState._activity` sin usar, `ActivityPage` un placeholder).
Trabajo puramente de exposición, sin lógica de negocio nueva:

- `AppState._activity` → `AppState.activity` (ya no es dead code).
- `ActivityEventRow` gana `#[serde(rename_all = "camelCase")]` (era el
  único DTO del proyecto sin esa convención).
- Nuevo comando `activity_recent` (`src-tauri/src/commands/activity.rs`),
  mismo formato que los de `web_search.rs` — sin parámetros, devuelve los
  200 eventos más recientes.
- Nuevos `src/lib/ipc/activity.ts` (schema zod) y
  `src/stores/activityStore.ts` (zustand, mismo patrón que
  `memoryStore.ts`).
- `ActivityPage.tsx` reescrito: filtros de nivel y categoría (categoría
  armada dinámicamente desde los eventos cargados, nunca hardcodeada —
  si aparece una categoría nueva en el backend no hace falta tocar el
  frontend), una `Card` por evento con badge de nivel/categoría,
  duración, y detalle expandible.
- **Decisión de diseño — polling, no push**: se evaluó un
  `AppEvent::ActivityRecorded` para actualizar en vivo (patrón que ya
  usan los feeds de Twitch/Discord/Chat), pero se descartó para esta
  pasada — `aivt-service-activity` ni depende de `aivt-eventbus` hoy, y
  `ActivityService::spawn` se llama en `lib.rs` *antes* de que exista el
  `EventBus`; cablear push tocaría esa firma (rompiendo sus 3 tests) y el
  orden de arranque. Para una vista de diagnóstico, un poll cada 5s
  mientras el tab está montado es un costo aceptable a cambio de cero
  riesgo sobre código de backend ya probado. Si más adelante se quiere
  que sea instantáneo, agregar el push es un follow-up chico y aislado.

Compila, 308 tests de backend (sin tests nuevos de Rust — el rename y el
`serde(rename_all)` no cambian lógica ya cubierta, y el comando es un
passthrough sin ramas) + 97 de frontend (2 archivos nuevos) pasan.
Verificado visualmente (filtros, estado vacío, traducciones ES) contra el
servidor de Vite; falta la confirmación en vivo dentro de la app real de
que un evento nuevo (ej. una respuesta de Chat, categoría `LLM`) aparece
solo dentro de un ciclo de poll.

### Confirmación al borrar conversaciones de Chat — completo

Borrar una conversación pasaba directo, sin ningún paso intermedio — un
riesgo real de pérdida de datos que el usuario señaló directamente.
`ConversationList.tsx` ahora abre un `ConfirmDialog` con un interruptor
opcional "extraer memorias antes de eliminar"; la extracción corre
primero, best-effort (su propio toast si falla), y el borrado siempre
sigue adelante después sin importar el resultado de la extracción.

### Markdown en el Chat — completo

Disparador: una respuesta de `web_search` cuyos links/negritas/listas se
mostraban como sintaxis literal (`[texto](url)`, `**texto**`) en vez de
renderizarse. `MessageBubble.tsx` ahora usa `react-markdown` v9 +
`remark-gfm` + `remark-breaks`. Los links abren en el navegador real del
sistema operativo gratis vía `tauri_plugin_opener` (ya registrado, sin
plugin nuevo). Una imagen remota (`![img](url)`) se renderiza
deliberadamente como link de texto, no como `<img>` — una URL de imagen
inventada por el modelo filtraría la IP/user-agent del usuario a un
tercero al renderizarse.

### La voz ya no lee Markdown/URLs en voz alta — completo

Nuevo `markdown::strip_markdown_for_speech` en `aivt-provider-tts`
(regex ordenada a mano, mismo estilo que `aivt_core::redact_secrets`),
aplicado una sola vez en `AivtTtsProvider::speak_and_capture` — cubre los
cuatro canales más el botón manual "probar voz" en un solo lugar. Dos
problemas reales encontrados armando la regex: el crate `regex` de Rust
no soporta backreferences/lookaround (un patrón de línea horizontal con
backreference hacía panic al inicializar el `LazyLock`, envenenando el
lock y arrastrando fallos en cascada por todo el binario de tests — una
pista falsa, no un bug aparte); y la ambigüedad entre cursiva con guion
bajo (`_texto_`) y nombres de archivo/identificadores reales
(`mi_archivo_final.txt`, `__init__.py`) — resuelto exigiendo que el
primer/último carácter capturado no sea guion bajo ni espacio, y sacando
la regla de `__negrita__` por completo (los modelos de chat acá solo
producen `**negrita**` en la práctica).

### Bug encontrado de paso: animación de diálogos descentrada — corregido

Reportado por el usuario justo después de que salieran los ítems de
arriba: los diálogos de confirmación se veían brevemente descentrados al
abrirse, "saltando" al centro real recién al terminar la animación.
Causa: las keyframes compartidas `.animate-pop-in` solo animan
`transform: scale(...)`, y como `animation` reemplaza la propiedad
`transform` entera mientras corre, el centrado propio del diálogo
(`translate(-50%,-50%)`, aplicado con clases de Tailwind) quedaba
anulado durante los 120ms de la animación. Corregido con una keyframe
dedicada (`dialog-pop-in`/`.animate-dialog-pop-in`) que incluye ese mismo
`translate` en los dos extremos de la animación, usada solo por
`DialogContent` — la base de todo diálogo de confirmación de la app
(Actividad, Chat, Identidad, Memoria, Privacidad), así que el fix los
cubre a todos.

**`ThemeEngine` por perfil — implementado en dos rondas** (ver
ADR-008/ADR-009): primero acento dinámico (`accentColor` → 3 variables
CSS derivadas), después 15 presets de tema completo (paleta de 9 colores
+ 3 radios) elegibles por personaje desde Identidad, con "Personalizado"
cayendo al comportamiento del acento solo. Ya no queda pendiente de esta
fase.

Con los seis ítems de arriba, Fase 11 queda completa — sin verificación
pendiente conocida. `cargo test --workspace` en verde (sin fallos) y 107
tests de frontend pasan (subieron de 98 antes de esta fase). Verificado
en vivo por el usuario: cambio de voz sin freezes ("funciona bien y sin
problemas") y centrado de diálogos correcto ("funciona bien y se ve
bastante bien").

## Mejoras posteriores a Fase 11 — limpieza de pestañas de Configuración

Pedido directo del usuario (2026-09-14): sacar de Configuración las
pestañas que eran placeholders sin ninguna implementación real detrás
("Integraciones", "Almacenamiento", "Avanzado", "Desarrollador") — se
suma una pestaña nueva si hace falta más adelante, no antes de que haya
algo real que mostrar en ella.

- `SettingsPage.tsx`: `SECTIONS` pasó de 13 a 9 entradas (general,
  aiProviders, stt, tts, webSearch, memory, appearance, privacy,
  activity). Como ya no queda ninguna sección sin componente real, se
  simplificó el mecanismo entero de `REAL_SECTIONS`/fallback:
  `SECTION_COMPONENTS` pasó de `Partial<Record<Section, ...>>` a
  `Record<Section, ...>`, y el render ya no necesita el ternario
  `ActiveContent ? <ActiveContent /> : <PlaceholderSettingsSection />`.
- `PlaceholderSettingsSection.tsx` y `src/app/PlaceholderPage.tsx`
  quedaron sin ningún otro uso en toda la app tras sacar esas 4 pestañas
  — se eliminaron los dos archivos en vez de dejarlos como dead code.
- Limpieza de las claves i18n correspondientes (`settings.tabs.
  integrations/storage/advanced/developer`, `placeholder.
  settingsSection`) en los 6 idiomas.

Compila (lint de TypeScript limpio) y 187 tests de frontend pasan. No fue
necesario recompilar el backend Rust — cambio puro de frontend.
Verificado relanzando la app real.

## Fase 10b — Búsqueda web / tool-calling

Pedido confirmado del usuario (idea planteada 2026-08-29, formalizada en
el roadmap 2026-09-05): que el personaje pueda responder cosas como el
clima, la hora en otro país, y consultas generales — "para complementar
todas las respuestas que sean necesarias". `docs/ADR/003-provider-
architecture.md` ya anticipaba un flag `tool_calling` en
`ModelCapabilities`, pero no existía ningún mecanismo real de
tool-calling — este trabajo lo construye desde cero.

**Decisiones de diseño**:
- **Backend de búsqueda: Brave Search API** — se investigó también
  SearXNG (sin costo pero con fricción de setup) y DuckDuckGo (descartado:
  su API oficial "Instant Answer" no es un buscador general, y scrapear su
  HTML viola sus términos de servicio).
- **Activo en los cuatro canales desde el arranque, incluida Llamada** —
  aceptando que una búsqueda puede agregar 1-3s de silencio antes de que
  el personaje responda en una llamada de voz en vivo.
- **Opt-in, apagado por defecto** — mismo criterio que Visión (Fase 3) y
  coherente con `docs/security.md` (la app es local-first); activar la
  búsqueda es la señal explícita del usuario de que esos canales pueden
  necesitar red.
- **Una sola herramienta genérica (`web_search`)**, no una por tipo de
  consulta — el modelo decide qué buscar.

**Completado**:
- **`aivt-provider-tools`** (nuevo crate): trait `Tool` (nombre,
  descripción, JSON Schema de parámetros, `execute`) + `BraveSearchTool`,
  su primera implementación real.
- **`aivt-provider-llm`**: `ChatRequest` gana `tools`; `ChatMessage`/
  `MessageRole` ganan lo necesario para representar una llamada a
  herramienta y su resultado (`ToolCall`, rol `Tool`); `ChatChunk` gana
  `tool_calls`. `openai_compatible.rs` acumula los fragmentos de
  `tool_calls` que llegan repartidos entre varios eventos del streaming
  (igual que ya hacía con el texto) y entrega la llamada completa en el
  chunk final.
- **`aivt-orchestrator`**: `AIOrchestrator::with_tools(...)` (builder,
  sin tocar la firma de `new` para no romper los ~25 sitios de test que ya
  la llamaban) + el loop de tool-calling dentro de `stream_respond`: llama
  al proveedor, si pide una herramienta la ejecuta y reenvía el resultado,
  hasta un tope de 3 idas y vueltas (la última siempre sin ofrecer
  herramientas, forzando una respuesta directa). Mismo patrón
  `tokio::spawn` + canal que ya usaba `OpenAiCompatibleProvider` — no uno
  nuevo — así que cancelar el stream (barge-in de Llamada) ya funciona
  sin código adicional.
- **`aivt_service_settings::WebSearchSettings`**: activado/desactivado +
  API key de Brave (mismo patrón `None`-mantiene/`Some("")`-borra que
  ElevenLabs), comandos Tauri (`web_search_get/set_settings`), pestaña
  nueva "Búsqueda web" en Configuración.
- Los cuatro canales (`ChatService` vía `commands/chat.rs`,
  `TwitchService`, `DiscordService`, `CallService`) leen el ajuste antes
  de generar una respuesta y encadenan `.with_tools(...)` si está
  activado y hay una key guardada — sin ningún otro cambio en esos
  servicios, ya que el loop entero vive en `stream_respond`.

**Verificado en vivo (parcial, Chat)**: tras una sesión larga de debugging
en vivo, confirmado que el personaje llama a `get_current_time` con la
zona horaria correcta y responde con la hora real (probado con Chile y
España en una conversación nueva). Dos hallazgos de esa sesión, ya
corregidos:
- **El historial largo de la conversación diluye la instrucción de usar
  herramientas** — con la instrucción solo en el prompt de sistema (al
  principio), un modelo local pequeño (8B) dejaba de intentar llamar a la
  herramienta después de ~20 mensajes de charla casual, aunque la
  herramienta siguiera ofrecida en cada request. Se confirmó comparando
  el mismo prompt exacto con y sin historial simulado contra el servidor
  real, antes de aplicar el fix. **Corregido** repitiendo la instrucción
  en un segundo mensaje de sistema, agregado *después* del historial (más
  cerca de la generación, donde el modelo le presta más atención) en vez
  de confiar solo en la copia del principio.
- **Una conversación que ya tiene varias respuestas incorrectas de este
  tipo en su propio historial sigue fallando** incluso con el fix — el
  modelo repite el patrón de "contestar directo" que ve en sus propios
  turnos anteriores dentro de esa charla puntual. No es un bug pendiente:
  es el modelo aprendiendo del propio contexto. Una conversación nueva
  (o que no haya visto ya el error repetido) funciona bien de entrada.
- Se encontró y corrigió además, de paso, que el modelo a veces alucina el
  identificador de zona horaria (ej. `Chile/Temuco`, que no existe) — el
  loop de reintento ya previsto en el diseño (herramienta rechaza el valor
  inválido, el error vuelve al modelo, reintenta) resuelve esto solo,
  confirmado en vivo.

**Dos hallazgos más, en sesiones de prueba posteriores — el patrón en
ambos: sin lugar mencionado, el modelo no tiene de dónde sacar "dónde está
el usuario" y termina inventando o buscando algo sin sentido**:
- **Una fecha/hora bare ("¿qué fecha es hoy?", sin ningún país) nunca
  disparaba ninguna herramienta** — la descripción de `get_current_time`
  la escopa a *otro* país/ciudad, así que el modelo la daba por no
  aplicable y contestaba de memoria de entrenamiento (llegó a decir "12 de
  junio de 2024", confirmado en el log: `tool_calls_pedidas=0` para esa
  respuesta). **Corregido de raíz, no con una herramienta**: la fecha/hora
  real (la del sistema, `chrono::Local::now()`) ahora se inyecta como dato
  ya dado en el prompt de sistema — al principio y repetido después del
  historial (mismo mecanismo que la instrucción de herramientas) — así
  que no depende de que el modelo decida pedirla. Cero cambio de
  comportamiento si no hay tools configuradas más que esta corrección.
- **Un "clima" bare (sin ciudad) hizo que el modelo buscara literalmente
  "clima en mi ubicación"** — una consulta sin ningún ancla geográfica —
  y Brave le devolvió el pronóstico de Buenos Aires, que el modelo
  presentó como si fuera el del usuario (que vive en Chile). **Corregido**
  agregando la zona horaria IANA real del sistema (vía el crate
  `iana-time-zone`, ej. `America/Santiago`) al mismo mensaje de fecha/hora
  ya inyectado, con instrucción explícita de usarla como pista de
  ubicación cuando no se menciona ciudad, y de nunca inventar una ciudad
  ni buscar "mi ubicación" tal cual. Es 100% dinámico — lee la zona
  horaria configurada en el sistema operativo en cada request, no hay
  ningún país/ciudad hardcodeado; en la máquina de otro usuario esto
  resuelve solo a la suya. Verificado en vivo: preguntando "clima" a
  secas en un chat nuevo, ubicó correctamente el país/región del usuario.

**Verificado en vivo también en Llamada** (sin buscarlo a propósito —
encontrado revisando el log de una sesión de uso real): preguntas sobre el
tipo de cambio de moneda y sobre el perfil de redes sociales de una
persona dispararon `web_search` de verdad (6 búsquedas reales contra
Brave, no alucinadas — confirmado cruzando el log con la tabla
`conversations`, que no tiene ningún registro de Chat en ese horario, y
solo guarda conversaciones con `source = 'chat'` — Llamada no persiste su
historial ahí). Dos límites de calidad de búsqueda encontrados, ninguno es
un bug del mecanismo:
- **"peso a dollar actual" (sin decir de qué país) devolvió el tipo de
  cambio del peso mexicano**, no el chileno — misma clase de ambigüedad
  que el clima sin ciudad, pero para monedas; una consulta anterior en la
  misma llamada sí había especificado "Chilean peso" y funcionó bien.
- **La búsqueda de una persona por su nombre de usuario chocó con una
  organización sin relación que comparte parte del nombre** (una clínica
  de neurofeedback llamada igual que uno de los apodos buscados) — Brave
  devolvió el Instagram de esa organización en vez del de la persona
  real. La primera búsqueda de esa serie sí había encontrado un perfil que
  parece ser el correcto (itch.io), pero el modelo no se quedó con ese
  resultado y siguió buscando variantes.

**Verificado en vivo en Discord**: varias búsquedas reales confirmadas en
el log (Bogotá y su clima, el perfil de una persona en varias
plataformas, hasta una búsqueda de "dirección de Vulito" que no encontró
nada real — recordatorio de que `web_search` busca literalmente lo que le
pidan, incluyendo datos del usuario si alguien en un servidor los pide).
Durante estas pruebas apareció un problema real, no relacionado con
búsqueda en sí:

- **El Gateway de Discord cerró la conexión a mitad de una prueba y el
  bot dejó de escuchar mensajes para siempre**, sin que reconectar fuera
  posible sin reiniciar. Causa raíz: una respuesta con tool-calling
  (búsqueda + varias idas y vueltas al LLM) tardó ~25s bloqueando el loop
  que lee el Gateway — lo suficiente como para que Discord considerara la
  conexión un "zombie" (sin heartbeat) y la cerrara. El código
  (`aivt_service_discord::gateway::DiscordGateway::next_message`) trataba
  ese cierre (`Event::GatewayClose`) como fatal, matando el loop entero.
  **Corregido**: confirmado contra el código fuente de `twilight_gateway`
  que un cierre de este tipo es recuperable — el `Shard` reconecta y
  resume la sesión solo con que se lo siga consultando (`next_event`).
  Ahora ese cierre (y errores transitorios de recepción) se loguean y se
  continúa el loop en vez de abortar; solo se sigue tratando como fatal el
  caso genuinamente irrecuperable (`next_event` devolviendo `None`, que
  según el código fuente de la librería solo pasa con el shard en estado
  `FatallyClosed` — token inválido o intents mal configurados).

**Verificado en vivo en Twitch**: una búsqueda real confirmada ("sunset
time Shanghai China", resultado correcto de timeanddate.com). Se encontró
el mismo tipo de bug que en Discord, pero con una causa técnica distinta:

- **La conexión EventSub se cerró a mitad de una prueba** (mismo patrón:
  una respuesta lenta de tool-calling bloqueando el loop) y
  `TwitchService::run_message_loop` trataba ese cierre como fatal también.
  A diferencia de Discord, el WebSocket de Twitch (`EventSubConnection`,
  hecho a mano sobre `tokio-tungstenite`) no tiene ningún mecanismo de
  auto-reconexión interno — una vez cerrado, está realmente muerto.
  **Corregido**: en vez de rendirse, ahora reconecta activamente
  (`establish_connection()` de nuevo: nueva sesión EventSub + resuscribirse
  vía Helix, con el mismo refresh de token que ya existía) y sigue
  escuchando — mismo patrón que el código ya usaba para cuando Twitch
  mismo pide una reconexión por protocolo (`EventSubMessage::Reconnect`).
  Si la reconexión en sí falla, ahí sí se da por vencido.

Con esto, las cuatro herramientas/canales de Fase 10b quedan verificados
en vivo — no queda ninguna verificación pendiente conocida. Ideas para
más adelante, no implementadas: si la ambigüedad de moneda ("peso" sin
país devolvió el tipo de cambio mexicano en vez del chileno una vez) se
repite seguido, se podría extender la misma pista de zona horaria para
sugerir la moneda local, igual que ya se hace para ciudades.

**Hallazgo de seguridad no relacionado, encontrado revisando el log
durante esta misma sesión de debugging**: `TtsBackend` (`aivt-provider-
tts`) derivaba `Debug` automáticamente, y un `tracing::info!` en
`aivt-service-chat` (`backend = ?tts.backend`) imprimía esa estructura
completa — incluyendo la API key de ElevenLabs en texto plano — al log en
disco cada vez que se reproducía audio. **Corregido**: `Debug` manual en
`TtsBackend` que oculta la key (`<redacted>`). De paso se revisó el resto
del código por el mismo patrón (un tipo con una API key/token que también
deriva `Debug`) y se encontraron cuatro casos más en riesgo latente (no
se habían llegado a loguear todavía): `LlmConnectionInput`,
`TtsConnectionInput`, `WebSearchInput` y `TokenPair` (Twitch) — a estos se
les quitó `Debug` directamente en vez de ocultar el campo, para que un
futuro intento de loguearlos no compile en vez de arriesgarse a filtrar la
key. Se recomendó al usuario rotar su API key de ElevenLabs, ya que la
anterior quedó expuesta en un log local antes de este fix.

## Fase 10 — Discord con voz — abandonada

Se intentó (Milestone A completo y verificado en vivo: `!join`/`!leave`
conectaban visiblemente al bot a un canal de voz de Discord, usando
`songbird` 0.6.0 sobre el mismo `twilight_gateway::Shard` ya abierto para
texto). Milestone B (que el personaje escuche y transcriba lo que se dice)
quedó bloqueado por un bug real, no un error propio:

- Discord exige DAVE (su cifrado extremo a extremo de voz) para **toda**
  conexión de voz desde el 2 de marzo de 2026 — un cliente que no lo
  declare en el `Identify` es rechazado directamente por el gateway de voz
  con el código de cierre 4017 (confirmado probando a mano: no es opcional,
  no hay downgrade posible sin que Discord corte la conexión).
- `songbird` 0.6.0 sí implementa DAVE, pero tiene un bug abierto y sin
  resolver en su ruta de recepción
  ([serenity-rs/songbird#310](https://github.com/serenity-rs/songbird/issues/310)):
  tras completar el handshake MLS y reportarse "listo", el audio recibido
  queda permanentemente sin decodificar (`decoded_voice` siempre `None`),
  sin ningún log de error — el paquete se descarta en silencio dentro de
  songbird. Se confirmó en vivo, con diagnóstico extendido (ventana de
  hasta 30s observando cada tick), que esto no es una demora transitoria:
  nunca se recupera.
- Sin mantenedores respondiendo el issue y sin ningún workaround viable
  desde fuera de la librería (deshabilitar DAVE no es una opción — ver el
  primer punto), se decidió con el usuario abandonar la función en vez de
  invertir en parches probabilísticos (reintentos automáticos `!leave`+
  `!join`) sobre un bug ajeno sin ETA de resolución.

Todo el código de voz (dependencia `songbird`/`webrtc-vad`, `!join`/
`!leave`, `DiscordVoicePolicy`, el segmentador VAD, los comandos Tauri y la
tarjeta de ajustes en el frontend) fue removido — Discord vuelve a ser
texto-only, igual que al cierre de la Fase 9. `crates/services/discord`
compila sin `songbird` en su árbol de dependencias.

## Mejoras posteriores a Fase 9 — TTS ElevenLabs, guardado de audio, y Client ID único de Twitch

Tres piezas de trabajo pedidas directamente por el usuario después de Fase
9, no parte de la numeración original de fases — construidas y verificadas
en vivo en la misma sesión.

### Completado

**TTS ElevenLabs (`crates/providers/tts/src/elevenlabs.rs`, nuevo)**
- Tercer backend de TTS, de primera clase (no "otro HTTP genérico") —
  orden pedido explícitamente: Nativo, ElevenLabs, Servidor HTTP. Solo
  necesita una API key, sin URL que configurar (endpoint fijo).
- `ElevenLabsEngine::synthesize` habla contra `POST /v1/text-to-speech/{voice_id}`
  (modelo fijo `eleven_multilingual_v2`, `speed` acotado a 0.7–1.2 a
  diferencia del rango más ancho de OpenAI) y `list_voices` contra
  `GET /v1/voices` — es el único backend externo con listado real de
  voces (la pestaña Voz muestra un desplegable, no texto libre, igual que
  con voces nativas).
- Clave de API en su propio slot de `SecretStore`
  (`elevenlabs_api_key`), separado del que usa `Http`, para que cambiar
  de backend en el selector no borre la clave del otro.
- `TtsBackendKind::ElevenLabs` necesitó `#[serde(rename = "elevenlabs")]`
  explícito — `rename_all = "snake_case"` por sí solo produce
  `eleven_labs` (separa en la mayúscula), que no coincidía con lo que el
  frontend ya tenía hardcodeado en todos lados. Bug real que llegó a
  producción (el usuario lo reportó al guardar la conexión) — quedó
  fijado con un test dedicado que congela el formato de wire.
- Error 402 (`payment_required`/`paid_plan_required`) al usar una voz de
  la Voice Library en un plan gratuito de ElevenLabs — no es un bug de la
  app, es una restricción documentada de ElevenLabs (esas voces solo se
  pueden usar desde su web, no vía API, sin plan de pago). Se tradujo a
  un mensaje claro en español en vez de mostrar el JSON crudo del error.

**Guardado y reproducción de audio en Chat (`ChatAudioSettings`, nueva
tabla de adjuntos reutilizada con `kind: "audio"`)**
- Nueva sección en Configuración > Privacidad: interruptor "Guardar audio
  de los mensajes" + selector de retención (1 día / 1 semana / 1, 2, 3 o 6
  meses — ampliado a pedido del usuario para quien quiera limpiar más
  seguido).
- Alcance deliberadamente limitado al Chat directo — Twitch y Llamada no
  persisten mensajes en absoluto hoy, así que no hay nada ahí para
  adjuntarles audio (ver Fase 8/Fase 6 más abajo).
- Solo funciona con los backends `Http`/`ElevenLabs` — la voz nativa habla
  directo por el sistema operativo (`tts::Tts`) sin pasar por un buffer de
  bytes, así que no hay nada capturable que guardar. `TTSProvider` ganó
  `speak_and_capture` (nuevo método requerido, `speak()` pasó a tener una
  implementación por defecto en términos de él) — devuelve
  `Some(bytes)` para los backends que sí producen un buffer, `None` para
  nativo.
- El botón de "reproducir" que ya existía en cada mensaje ahora reproduce
  el archivo guardado al instante (sin llamar a la API de nuevo) cuando
  existe, con fallback a la síntesis en vivo de siempre cuando no
  (voz nativa, guardado desactivado, o falla la lectura del archivo).
- Limpieza automática al arrancar la app (`ChatService::prune_old_audio`)
  — borra audios más viejos que la retención configurada, no un temporizador
  en vivo (la app se reinicia seguido, un barrido por arranque alcanza).

**Client ID único de Twitch (`aivt_service_twitch::CLIENT_ID`)**
- El diseño "un solo Client ID para todo el producto" (elegido por el
  usuario en una ronda anterior) quedó implementado: el usuario creó su
  propia aplicación de Twitch (tipo Público) y proporcionó el Client ID,
  que ahora vive hardcodeado como constante en el crate de Twitch — un
  Client ID de OAuth es un identificador público por diseño (viaja con la
  app cliente, no es un secreto), así que no necesita `SecretStore` ni
  configuración por usuario.
- Se eliminó por completo el campo "Client ID" que cada usuario tenía que
  pegar manualmente: `TwitchConnectionInput`, `set_twitch_client_id`
  (settings), el comando `twitch_set_client_id`, el campo en el frontend
  (`TwitchPage.tsx`), y las claves i18n asociadas.

### Decisiones de alcance (documentadas, no accidentales)
- El guardado de audio no cubre Twitch ni Llamada — construir eso primero
  exigiría persistencia de mensajes que esas integraciones no tienen hoy
  en absoluto (decisión explícita del usuario al plantear el alcance).
- Sin botón de "borrar todos los audios guardados ahora" — solo el
  barrido automático por retención al arrancar; no se pidió explícitamente
  y se puede sumar después sin romper nada.

### Problemas encontrados y corregidos
El guardado de audio pasó por dos rondas de diagnóstico en vivo antes de
funcionar:
1. **El audio nunca se guardaba en absoluto** — causa: el mensaje del
   asistente que ve el frontend se arma solo a partir de los eventos de
   streaming (`AiResponseStarted`/`Token`/`Finished`), que nunca cargan
   `attachmentId`. Se agregó un evento nuevo, `AppEvent::ChatAudioSaved`
   (publicado por `maybe_speak` justo después de guardar el archivo), para
   que el frontend se entere sin tener que volver a pedir toda la
   conversación.
2. **El evento se publicaba correctamente (confirmado por log:
   `auto-play: audio guardado, publicando ChatAudioSaved`) pero el botón
   de reproducir seguía sin encontrar el archivo guardado** — causa real:
   `App.tsx` tiene un enrutador central de eventos que reenvía cada tipo a
   la store que le corresponde mediante una cadena de `if` explícita por
   prefijo/nombre de evento; `chat_audio_saved` no estaba en ninguna de
   esas ramas, así que el evento se publicaba y llegaba al frontend pero
   se descartaba en silencio antes de llegar a `chatStore`. Arreglado
   agregando la rama que faltaba. Ambos pasos se diagnosticaron leyendo el
   log de la app en vivo, no adivinando — el primer síntoma reportado
   ("sigue pidiendo una voz nueva") era idéntico en ambos casos, así que
   sin el log habría sido fácil arreglar solo la mitad del problema.

### Verificación en vivo
El usuario desconectó y volvió a conectar su cuenta de Twitch desde cero
para confirmar que el flujo OAuth completo funciona con el Client ID
único fijo (no solo con una cuenta ya conectada antes de este cambio, cuyos
tokens ya emitidos no habrían probado nada). `begin_oauth`/`complete_oauth`/
`disconnect` no tienen ninguna línea de `tracing` propia, así que el log de
la app no sirve para confirmar esto — se verificó leyendo directamente
`app_settings.twitch_connection` en `aivt.db`: tras la reconexión,
`hasTokens: true` y `channelLogin` con la cuenta de bot correcta
(`ciel_iabot`), con `targetChannelLogin`/`targetBroadcasterId` (Fase 8b)
intactos como se esperaba, ya que `disconnect_twitch` no los toca.

### Problemas conocidos
Ninguno pendiente — las tres piezas de esta sección quedaron verificadas
en vivo por el usuario.

## Fase 8b — Twitch, cuenta de bot propia

### Completado

Pedido explícito del usuario: Fase 8 conectaba la propia cuenta de Twitch
del streamer (el personaje respondía con su login) — se pidió que Twitch
se pareciera más a Discord (Fase 9), con una cuenta de bot dedicada
respondiendo con su propio nombre, para que fuera más sencillo de montar
para cualquier usuario final.

A diferencia de Discord, Twitch no tiene un concepto de "Application +
Bot" — un bot ahí es simplemente otra cuenta normal de Twitch. Investigado
contra la documentación oficial: leer el chat de cualquier canal vía
EventSub solo necesita `user:read:chat` en el token de quien lee (sin
permiso del streamer); publicar en nombre de una cuenta en el canal de
*otra* persona necesita que esa cuenta sea moderadora ahí (o que
`broadcaster_id == sender_id`). Esto significa que **no hace falta una
segunda autorización OAuth del streamer** — el mismo Device Code Grant de
siempre alcanza, con los mismos scopes.

**Backend**
- Nuevo concepto: "canal al que se une" (`target_channel_login`/
  `target_broadcaster_id` en `TwitchConnectionSettings`), separado de "qué
  cuenta está conectada". Vacío por defecto → el comportamiento es
  exactamente el de Fase 8 (compatible con toda cuenta ya conectada sin
  que el usuario haga nada). Con un valor → la cuenta conectada (ahora
  normalmente un bot dedicado) lee y publica en ESE canal.
- `crates/services/twitch/src/service.rs`: nueva función pura
  `effective_broadcaster_id` (con sus propios tests unitarios, sin
  depender de red/DB) decide qué id usar. `establish_connection`
  construye la condición de EventSub con `broadcaster_user_id` = canal
  destino y `user_id` = cuenta conectada (antes ambos eran el mismo
  valor). `respond_to` separa `target_broadcaster_id` de `sender_id`.
  Nuevo `TwitchService::set_target_channel(channel_login)`: vacío limpia
  el destino (vuelve a "responde en su propio canal") sin llamar a Twitch;
  si no, resuelve el login vía Helix y guarda el par resuelto.
- `crates/services/twitch/src/helix.rs`: nuevo `get_user_by_login` (`GET
  /helix/users?login=`) — cualquier token válido puede resolver cualquier
  nombre de canal, no hace falta que le pertenezca.
- **Doble señal de mención**: `TwitchChatMessage` ahora tiene
  `mentions_broadcaster` (mención del canal donde llegó el mensaje, sin
  cambios) **y** `mentions_bot` (mención de la cuenta conectada
  específicamente, nuevo) — ambas cuentan para "Solo cuando lo mencionan".
  Deliberadamente no se reemplazó una por la otra: cuando el bot publica
  en un canal ajeno, un espectador puede seguir mencionando al streamer
  esperando que el personaje reaccione, o mencionar directamente al bot —
  las dos son razonables. En modo "propio canal" (sin destino) ambas
  señales siempre coinciden, así que no hay regresión.
- `EventSubConnection::connect`/`connect_to` ahora reciben el id de la
  cuenta conectada (mismo patrón que `DiscordGateway::connect` en Fase 9)
  para poder calcular `mentions_bot`.

**Frontend**
- `TwitchPage.tsx`'s `ConnectionCard`: nuevo campo "Canal al que se une"
  (solo visible con la cuenta ya conectada), con instrucciones inline
  (crear una segunda cuenta de Twitch, conectarla, escribir
  `/mod <nombre_del_bot>` una vez en el canal principal — y aviso de que
  eso también da poderes completos de moderador, no solo permiso para
  hablar). Deshabilitado mientras la escucha esté activa (la suscripción
  de EventSub no se puede mutar en un socket ya abierto — cambiar de
  canal exige detener y volver a iniciar, igual que cambiar de cuenta).
- Limpieza de paso: se borraron 4 claves i18n muertas
  (`twitch.clientSecret*`, resto de una implementación de Authorization
  Code Grant abandonada en Fase 8, confirmado sin referencias) y se
  corrigió `clientIdHint` (mencionaba una Redirect URL que Device Code
  Grant nunca necesitó).

### Decisiones de alcance (documentadas, no accidentales)
- **Sin segunda autorización OAuth del streamer** (`channel:bot` otorgado
  por el broadcaster) — solo el camino de "cuenta de bot como moderadora".
  Más simple, un único Device Code Grant, sin ceremonia de dos partes.
- **Sin chequeo previo de "¿ya es moderador?"** — Twitch no expone una
  forma simple de comprobarlo con el token del propio bot. Si falla al
  publicar, el log de advertencia genérico que ya existía en `respond_to`
  es suficiente; la instrucción de moderar se explica por adelantado en la
  UI en vez de reaccionar al error.
- **`disconnect_twitch` conserva el canal destino guardado** (igual que ya
  conservaba `client_id`) — no es un dato de identidad que haga falta
  re-verificar; reconectar la misma cuenta de bot no debería obligar a
  re-escribir el canal.

### Cómo probarlo en vivo (pendiente al momento de escribir esto)
1. Con una cuenta de Twitch ya conectada en modo Fase 8 (sin canal
   destino) — confirmar que sigue funcionando exactamente igual que antes
   (sin regresión).
2. Crear una segunda cuenta de Twitch para el bot, conectarla en la
   pestaña Twitch (usar una ventana de incógnito si ya había sesión
   iniciada con la cuenta principal).
3. Escribir el nombre del canal principal en "Canal al que se une" y
   guardar.
4. Desde el chat del canal principal (con la cuenta del streamer),
   ejecutar `/mod <nombre_del_bot>`.
5. Iniciar la escucha — confirmar que el personaje responde en el chat del
   canal principal usando el nombre del bot, no el del streamer.
6. Probar tanto una mención al bot (`@nombre_del_bot`) como una mención al
   streamer (`@streamer`) con "Solo cuando lo mencionan" activo —
   confirmar que ambas disparan una respuesta.

### Problemas conocidos
- Sin verificar en vivo todavía — ver la sección anterior.

## Fase 9 — Discord (texto)

### Completado

**Backend**
- `crates/services/discord` (antes vacío, sección 9 del brief) —
  `DiscordService`, segundo consumidor real de `ManagedService`/
  `ServiceManager` tras Twitch, pero con una cuenta de **bot propia** en
  vez de la cuenta personal del usuario (pedido explícito: "quiero que lo
  de discord sea un bot con su cuenta propia para que pueda responder a
  cualquier persona").
  - **Sin OAuth en absoluto**: a diferencia del Device Code Grant de
    Twitch, un bot de Discord solo necesita un token generado una vez en
    el Developer Portal. `DiscordService::connect_bot` valida el token con
    `GET /users/@me` (`twilight-http`) antes de guardarlo — un token
    inválido falla ahí mismo, no en silencio dentro de `start()`.
  - **Gateway** (`gateway.rs`): `twilight-gateway`'s `Shard`, no un
    WebSocket hecho a mano como Twitch — Discord exige heartbeating y
    resume/reconexión que `Shard` maneja internamente; el bucle de
    `DiscordService` nunca necesita un caso `Reconnect` explícito.
  - **Publicar respuestas** (`http.rs`): `twilight-http`, `POST` de mensaje
    vía `create_message(channel_id).content(text)`, tope de 2000
    caracteres (el límite real de Discord, no un margen reducido como el
    de Twitch).
  - **Política** (`policy.rs`): `should_respond` — descarta *cualquier*
    mensaje de una cuenta de bot (no solo el propio, a diferencia de
    Twitch, ya que un servidor de Discord puede tener otros bots).
    `mentioned_only` acepta una @mención real de Discord
    (`message.mentions`, una señal estructurada de verdad) *o* el nombre/
    apodo del personaje en texto plano — Twitch solo tiene la segunda
    opción porque un espectador no puede @mencionar oficialmente al
    personaje ahí.
  - **Sin voz, nunca**: `generate_reply` es puramente texto — a
    diferencia de Twitch/Chat/Llamada, Discord nunca llama a
    `TTSProvider::speak` en absoluto (ver "Problemas encontrados" para por
    qué se llegó a esta decisión tras dos iteraciones).
- **Mute global de voz** (`crates/providers/tts`): `AivtTtsProvider` ahora
  guarda un `AtomicBool` de silencio, comprobado al inicio de `speak()` —
  cubre Chat/Twitch/Llamada con un solo interruptor (botón en el Sidebar,
  junto a "Mostrar ventana flotante"), independiente del `voice.enabled`
  de cada perfil. Apagado por defecto (no cambia nada de lo que ya
  funcionaba); silenciar también detiene de inmediato lo que esté sonando
  (`voice_set_muted` llama a `tts.stop()`), no solo las respuestas
  futuras. Persistido en `SettingsService::voice_muted`, releído al
  arrancar para sembrar el `AtomicBool`. Discord no lo usa — ver punto
  anterior.
- `AppEvent::DiscordMessageReceived`/`DiscordMessageSent` — mismo patrón
  que los eventos de Twitch, para el feed en vivo de la pestaña Discord.
- Comandos Tauri (`commands/discord.rs`): `discord_get_connection`,
  `discord_get/set_policy`, `discord_connect_bot` (un solo paso, sin
  navegador), `discord_disconnect_account`, `discord_start`/`discord_stop`,
  `discord_get_status`, `discord_set_active_profile`,
  `discord_send_test_message`. Más `commands::voice::voice_get/set_muted`
  para el mute global.
- `crates/core/src/cooldown.rs` (nuevo): `CooldownTracker` se movió aquí
  desde `crates/services/twitch/src/policy.rs` — Discord lo necesitaba
  igual, mismo criterio que ya se usó para `contains_word`.

**Frontend**
- `src/features/discord/DiscordPage.tsx` (reemplaza el placeholder):
  mismo patrón que `TwitchPage.tsx` — tarjeta de conexión (un solo campo
  para el token del bot), tarjeta de política, tarjeta "Probar respuesta",
  feed en vivo.
- `src/app/Sidebar.tsx`: botón de mute global (`Volume2`/`VolumeX`) junto
  a los controles del avatar, en vez de vivir dentro de una pestaña
  concreta — pedido explícito del usuario para que quede alcanzable sin
  importar qué pestaña esté abierta.
- `src/stores/discordStore.ts`, `src/stores/voiceStore.ts` (ampliado con
  `muted`/`loadMuted`/`setMuted`), `src/lib/ipc/discord.ts`,
  `src/lib/ipc/voice.ts` (ampliado).

### Decisiones de alcance (documentadas, no accidentales)
- **Registro de personas para Discord** — agregado después del cierre de
  esta fase (pedido explícito del usuario), reusando
  `PeopleRepository::upsert_by_external_key` tal como ya se preveía acá:
  `PeopleService::observe_discord_author` (espejo de
  `observe_twitch_chatter`, `external_key = "discord:<author_id>"`,
  `role = "discord_chatter"`) se llama por cada mensaje de un autor humano
  real. Interruptor propio e independiente del de Twitch,
  `PeoplePolicy.discord_auto_learn_enabled` (on por defecto), en
  Configuración > Privacidad.
- **Sin cooldown/política por servidor** — una sola política global,
  igual de simple que Twitch hoy, aunque el bot puede estar en varios
  servidores a la vez.
- **Discord nunca habla, sin excepción** — no es un ajuste desactivable
  desde la pestaña Discord; es una decisión de diseño (el crate ni
  siquiera depende de `aivt-provider-tts`). Si más adelante se quiere voz
  ahí, hace falta una decisión explícita, no solo activar un interruptor.

### Problemas encontrados y corregidos
Verificar esta fase en vivo (bot real, servidor de prueba) llevó a un
malentendido de diseño en tres pasos, cada uno visible solo una vez
resuelto el anterior:
1. **Primer intento**: Discord obtuvo su propio `DiscordPolicy.voice_enabled`
   (apagado por defecto) para no sorprender al usuario con voz en un canal
   de texto. El usuario aclaró que quería el botón **junto a "Mostrar
   ventana flotante"** en el Sidebar, no dentro de la pestaña Discord.
2. Tras mover el botón, el usuario probó silenciar la voz **desde la
   pestaña Chat** y no pasó nada — reveló que el pedido real era un
   **mute global** (Chat/Twitch/Discord/Llamada a la vez), no un ajuste
   por integración. Se reemplazó `DiscordPolicy.voice_enabled` por un
   único `AtomicBool` en `AivtTtsProvider` (elegido sobre una bandera en
   `SettingsService` comprobada en cada llamada, para no meter una lectura
   async a la base de datos en un camino con sensibilidad a la latencia),
   con métodos `set_muted`/`is_muted` añadidos al trait `TTSProvider` con
   implementación por defecto (no-op) para que `MockTTSProvider` no
   necesitara tocarse.
3. Con el mute global funcionando, el usuario probó Discord y **seguía
   sonando** — el mute global (por diseño) no estaba pensado para silenciar
   Discord específicamente, solo para ser un interruptor maestro. La
   aclaración final: Discord no debía hablar **nunca**, con voz activada o
   no. Se eliminó `TTSProvider` de `aivt-service-discord` por completo
   (incluida la dependencia en `Cargo.toml`) en vez de agregar un tercer
   mecanismo de silencio — la integración es de texto puro por diseño,
   no "voz apagada por defecto".

### Problemas conocidos
- Sin cooldown por servidor — ver "Decisiones de alcance".
- Sin voz por diseño — ver arriba.

## Fase 8 — Twitch

### Completado

**Backend**
- `crates/services/settings`: `twitch_connection` (client_id, channel_login,
  broadcaster_id, has_tokens — nunca los tokens en sí) y `twitch_policy`
  (enabled, respond_to_all, mentioned_only, cooldown_seconds), mismo patrón
  que `llm_connection`/`tts_connection`. Los tokens OAuth viven en
  `SecretStore` bajo `service="aivt.twitch"` — nunca en SQLite ni en logs
  (docs/security.md ya lo anticipaba, aunque como blob de settings + secret
  store en vez de una tabla `twitch_settings` dedicada, siguiendo el patrón
  que ya funciona para LLM/TTS en vez del boceto original de Fase 0).
- `crates/services/twitch` (antes vacío, sección 8 del brief) —
  `TwitchService`, primer consumidor real de `ManagedService`/
  `ServiceManager` (Fase 0): `start()` conecta EventSub y crea la
  suscripción de verdad antes de devolver `Ok` (un token vencido falla ahí,
  no en silencio dentro de una tarea ya "Running"); `stop()` cancela el
  bucle de mensajes.
  - **OAuth** (`auth.rs`): **Device Code Grant**, no Authorization Code —
    ver "Problemas encontrados y corregidos" para por qué. Sin redirect
    URI, sin listener HTTP local, sin client secret (los apps de Twitch
    tipo "Público" no lo tienen en absoluto). `begin_oauth` pide un
    `device_code`/`user_code` a `POST /oauth2/device`; el comando Tauri
    abre `verification_uri` (ya trae el código prellenado) en el
    navegador; `complete_oauth` hace polling a `POST /oauth2/token` con
    `grant_type=urn:ietf:params:oauth:grant-type:device_code` hasta que
    el usuario aprueba, lo rechaza, o se agota el tiempo.
  - **Chat** (`eventsub.rs`): WebSocket a EventSub, no IRC — maneja
    `session_welcome`/`session_keepalive`/`session_reconnect`/
    `notification`, y usa el evento `channel.chat.message` tanto para leer
    el chat como (en el futuro) redenciones, sobre la misma conexión.
  - **Publicar respuestas** (`helix.rs`): `POST /chat/messages`, con
    refresco automático del token de acceso si Helix devuelve 401.
  - **Política** (`policy.rs`): `should_respond` — ignora sus propios
    mensajes, mensajes vacíos y comandos `!` de otros bots; `respond_to_all`
    tiene prioridad sobre `mentioned_only` si ambos están activos.
    `CooldownTracker` aparte, para no responder más seguido que
    `cooldown_seconds` sin importar cuántos mensajes califiquen mientras
    tanto.
  - **"Mencionado" es por nombre, no por @mention de Twitch**: un
    espectador no puede @mencionar al personaje de verdad — en Twitch solo
    existe la cuenta del streamer, el personaje no tiene una propia. Así
    que `mentioned_only` se dispara buscando el `name`/`nickname` del
    perfil activo como palabra suelta en el texto (`contains_word`,
    insensible a mayúsculas, tolera puntuación alrededor:
    "@Ciel"/"Ciel,"/"oye Ciel!" cuentan igual) — el @mention oficial del
    canal del streamer sigue contando también, por si algún espectador lo
    usa así.
  - **Respuesta del personaje**: reutiliza `AIOrchestrator::stream_respond`
    (el mismo que Chat) con un `max_tokens` más bajo (150) — los mensajes
    de Twitch tienen un límite de 500 caracteres, así que no tiene sentido
    generar (ni pagar) una respuesta larga solo para truncarla.
  - **Voz por IA**: tras publicar una respuesta (o en modo prueba, ver
    abajo), `spawn_speak` reproduce el texto con el mismo `TTSProvider` y
    los mismos ajustes de voz del perfil (Identidad > Voz) que usa Chat —
    "¿tiene voz este personaje?" sigue siendo un único ajuste compartido
    por todos los canales, no un interruptor aparte para Twitch. Corre
    como tarea aparte (`tokio::spawn`) para no bloquear el siguiente
    mensaje del bucle de EventSub mientras habla.
  - **"Probar respuesta"** (`TwitchService::send_test_message`): corre la
    misma generación + voz que una respuesta real, pero sin tocar Helix
    — no hace falta un token de acceso ni estar "escuchando". Existe
    porque la cuenta conectada es la misma que usa el personaje para
    publicar (ver más abajo), así que probar en solitario desde tu propio
    chat de Twitch es imposible; esta es la vía soportada para ver/oír una
    respuesta sin una segunda cuenta.
  - **Qué personaje responde**: el backend no tenía ninguna noción de
    "perfil activo" (vive solo en `localStorage` del frontend, ver
    `profileStore.ts`) — `twitch_set_active_profile` lo sincroniza en vivo
    cada vez que cambia, igual que `avatarStore` hace con la ventana
    flotante.
  - **Diagnóstico**: cada vez que un mensaje se ignora (cuenta propia
    conectada, sin personaje activo, sin proveedor de IA, en cooldown)
    queda anotado con `tracing::info!`/`warn!` (visible en el log sin
    `RUST_LOG=debug`) — antes esos casos fallaban en silencio y eran
    indistinguibles de "todo funciona pero nadie calificó".
- `AppEvent::TwitchMessageReceived`/`TwitchMessageSent` — para el feed de
  chat en vivo; `ServiceStatusChanged` (ya existía desde Fase 0) ahora
  tiene su primer productor real.
- Comandos Tauri (`commands/twitch.rs`): `twitch_get/set_connection`,
  `twitch_get/set_policy`, `twitch_connect_account` (corre todo el ida y
  vuelta de OAuth y no devuelve hasta terminar — abre el navegador vía
  `tauri-plugin-opener`, nuevo en esta fase), `twitch_disconnect_account`,
  `twitch_start`/`twitch_stop` (delegan a `ServiceManager`),
  `twitch_get_status`, `twitch_set_active_profile`,
  `twitch_send_test_message`.

**Frontend**
- `src/features/twitch/TwitchPage.tsx` (reemplaza el placeholder):
  tarjeta de cuenta (Client ID, conectar/desconectar), tarjeta de política
  (activar respuestas, modo mencionado/todos, cooldown, iniciar/detener
  escucha con su badge de estado, nota fija explicando que la cuenta
  propia nunca dispara una respuesta), tarjeta "Probar respuesta" (texto +
  botón, solo visible con la cuenta conectada), y un feed de chat en vivo.
- `src/stores/twitchStore.ts`: estado de conexión/política/status +
  feed acotado a 200 mensajes (`AppEvent::TwitchMessage*`) +
  `sendTestMessage`/`isSendingTestMessage`.
- `App.tsx`: sincroniza `twitch_set_active_profile` cada vez que cambia el
  perfil activo, igual que ya hace con `avatarStore`.
- `src/lib/errorMessage.ts` (nuevo, usado en toda la app): Tauri rechaza
  los comandos fallidos con el `AppError { module, code, message }` crudo,
  no una `Error` de JS — el helper duplicado en 8 archivos
  (`err instanceof Error ? err.message : String(err)`) producía
  `"[object Object]"` en vez del mensaje real. Ver "Problemas encontrados
  y corregidos".

### Decisiones de alcance (documentadas, no accidentales)
- **Redenciones de puntos de canal: fuera de esta fase.** Requieren un
  scope adicional y una UI de reglas propia; EventSub ya deja la puerta
  abierta (misma conexión, otro tipo de suscripción) para cuando se
  aborde. Pedido explícito del usuario al arrancar la fase.
- **Los mensajes de Twitch no se guardan como conversaciones ni generan
  memorias automáticas.** El personaje responde con su misma identidad/
  personalidad de siempre, pero no hay historial persistente de Twitch ni
  extracción de memoria desde ahí todavía — se puede sumar después sin
  romper nada, ya que reutiliza el mismo `AIOrchestrator` que Chat.
- **Sin reconexión automática con backoff** ante una caída inesperada de
  EventSub — sí se maneja `session_reconnect` (la migración de sesión que
  Twitch pide explícitamente), pero una desconexión genuina deja el
  servicio en estado `Error` para que el usuario reconecte manualmente
  desde la pestaña Twitch, en vez de reintentar en silencio indefinidamente.
- El personaje conectado responde en **su propio** canal (la cuenta que
  autoriza la app es la misma que transmite) — no hay flujo separado para
  "bot moderador añadido a otro canal" todavía. Consecuencia directa: la
  cuenta conectada también es la que el personaje usa para publicar sus
  respuestas, así que `run_message_loop` ignora cualquier mensaje cuyo
  `chatter_login` coincida con ella (si no, el personaje podría
  responderse a sí mismo en bucle). Eso hace imposible probar las
  respuestas automáticas escribiendo en el propio chat desde la cuenta
  conectada — de ahí "Probar respuesta" (arriba) y la nota fija en la
  pestaña Twitch que lo explica.

### Problemas encontrados y corregidos
Verificar esta fase en vivo (cuenta de Twitch real, luego una cuenta
secundaria para probar la respuesta automática de verdad) sacó a la luz
tres problemas en cadena, cada uno solo visible una vez resuelto el
anterior:
1. **`"[object Object]"` en vez del error real.** El primer intento de
   conectar la cuenta fallaba con ese texto inútil en el toast. Causa: los
   comandos de Tauri fallidos rechazan con el `AppError` crudo, no una
   `Error` de JS, y el helper `err instanceof Error ? err.message :
   String(err)` (duplicado en 8 archivos) no contemplaba esa forma.
   Arreglado de una vez para toda la app con `src/lib/errorMessage.ts`.
2. **El flujo de OAuth entero era el equivocado.** Una vez visible, el
   error real era `{"status":400,"message":"Invalid client credentials"}`
   — parecía "falta el client secret", así que se construyó soporte para
   guardarlo (ajuste, UI, `exchange_code`/`refresh_tokens` actualizados).
   Pero una captura de pantalla del usuario mostrando su app de Twitch
   registrada como **"Público"** reveló que ese tipo de app **no tiene
   client secret en absoluto** en la consola de Twitch — el problema no
   era un parámetro faltante, era que Authorization Code Grant nunca fue
   el flujo correcto para un cliente Público. Se revirtió todo el trabajo
   de client secret y se reescribió `auth.rs` para usar Device Code Grant
   (sin redirect URI, sin listener local, sin secret en ningún punto,
   tampoco al refrescar tokens).
3. **"Responder a todos los mensajes" no hacía nada, en silencio.**
   Después de conectar con Device Code Grant, escribir en el propio chat
   no generaba ninguna respuesta ni había pista de por qué. Causa: el
   filtro de "no responder a mis propios mensajes" (necesario para que el
   personaje no se responda a sí mismo) también descarta cualquier
   mensaje escrito por la cuenta conectada — que es la misma que usa
   quien prueba en solitario. Arreglado con "Probar respuesta" (evita el
   filtro por completo, sin tocar Helix) más registro de diagnóstico en
   cada punto donde un mensaje se descarta, para que un futuro silencio
   similar sea legible en el log sin `RUST_LOG=debug`. Confirmado
   funcionando con una cuenta de Twitch secundaria real.

### Problemas conocidos
- No hay backoff/reintentos en la conexión EventSub — ver "Decisiones de
  alcance".
- La ventana flotante del avatar y Twitch todavía no están conectados
  entre sí (el avatar no reacciona a "está escuchando el chat" como
  estado propio) — no era parte del pedido de esta fase.

## Fase 7 — Avatar

**Nota de esta reescritura (2026-09-22)**: esta sección describía un diseño
que fue **reemplazado por completo** en una sesión anterior (el cambio en
sí no se documentó en el momento en que se hizo, quedó como deuda de
documentación — mismo caso que la nota de reconstrucción de Fase 6, más
abajo). Reescrita ahora a partir del código real (`crates/services/avatar`,
`src/features/avatar/AvatarEditor.tsx`, `src-tauri/src/commands/avatar.rs`),
no de un registro en vivo de la sesión que hizo el cambio — por eso no hay
una sección nueva de "Problemas encontrados probando en vivo" con la misma
narrativa cronológica que el resto del documento. El diseño viejo (ventana
nativa de Tauri) y sus bugs quedan documentados, condensados, en "Diseño
anterior" al final de esta sección, porque siguen siendo relevantes como
motivación del rediseño y como advertencia si alguna vez se considera volver
a un enfoque de ventana nativa para otra cosa.

### Completado

**Backend — almacenamiento de las imágenes (sin cambios desde el diseño original)**
- Migración `0009_avatar_states.sql`: tres columnas nuevas en `profiles`
  (`avatar_idle_path`, `avatar_thinking_path`, `avatar_talking_path`) — no
  una tabla hija normalizada, porque el conjunto de estados es fijo (lo que
  el propio ciclo de vida de eventos de la app puede impulsar), igual que
  los 19 sliders de personalidad son un conjunto fijo de columnas.
- `crates/database/src/repositories/profiles.rs`: enum `AvatarState`
  (Idle/Thinking/Talking), `ProfilesRepository::set_avatar_state_path`.
  `ProfileService::set_avatar_state` reutiliza el mismo patrón de
  reemplazar-el-archivo-anterior que `set_avatar`, guardando en
  `avatars/<id>/<estado>.<ext>`.
- **Deliberadamente sin `AvatarProvider`/`AvatarService` en Rust** para esta
  parte — el stub de Fase 0 anticipaba un trait (como
  `TTSProvider`/`STTProvider`), pero "qué imagen tiene guardada este perfil
  para este estado" no habla con ningún servicio externo, es solo
  almacenamiento de archivos. El servidor que sí se agregó en este rediseño
  (`aivt-service-avatar`, abajo) es la pieza de servicio real — llegó
  cuando hizo falta, no antes.
- `ProfileExport` (`.aivtprofile`) sigue sin incluir estas rutas — mismo
  criterio que el avatar estático: no tiene sentido portar una ruta de
  archivo local en un export JSON.
- `src-tauri/src/commands/avatar.rs`: `avatar_set_state_image`/
  `avatar_get_state_image` siguen igual (mismo patrón base64 que
  `profiles_set_avatar`). Lo que sí desapareció de este archivo:
  `avatar_window_show`/`avatar_window_hide`/`avatar_window_set_always_on_top`
  — ver "Diseño anterior".

**Backend — servidor OBS (`crates/services/avatar`, crate `aivt-service-avatar`)**
El avatar reactivo ahora se sirve como una **fuente de navegador (Browser
Source) de OBS auto-hosteada**, en vez de una ventana nativa capturada por
OBS. Un pequeño servidor Axum local expone tres rutas:
- `GET /overlay` — la página HTML/CSS/JS completa que OBS carga en su
  Browser Source (ver `overlay_html.rs`).
- `GET /avatar/{profile_id}/{state}` — bytes crudos de la imagen de un
  estado (`idle`/`thinking`/`talking`), con *fallback* a la foto de perfil
  normal si ese estado no tiene imagen propia subida. Esta resolución en
  dos pasos antes solo existía en el frontend (`useAvatarStateImage.ts`,
  porque el comando Tauri `avatar_get_state_image` deliberadamente
  devolvía `None` y dejaba decidir al frontend); una página de OBS no tiene
  ningún frontend Tauri al que delegarle esa decisión, así que
  `routes.rs::avatar_image` es un port desde cero de esa misma resolución,
  del lado del servidor. El `Content-Type` se infiere de la extensión del
  archivo (png/jpg/gif/webp).
- `GET /ws` — WebSocket que empuja un mensaje `{"state": "idle"|"thinking"|"talking"}`
  cada vez que el estado visual cambia para el `profileId` de esa conexión
  (query param), y uno más al conectar para que la página muestre algo
  antes de que llegue el primer evento real.
- **Bindeado solo a `127.0.0.1`** (`service.rs::start`) — OBS siempre corre
  en la misma máquina, así que quedarse fuera de `0.0.0.0` evita tanto
  exponer el servidor a la LAN como disparar el prompt de "permitir esta
  app en el Firewall de Windows" que un bind público hubiera activado.
- `crates/services/avatar/src/reducer.rs`: un **port a Rust de la máquina
  de estados que antes vivía solo en `avatarStore.ts`** (frontend) —
  `Reducer::apply` toma un `AppEvent` y devuelve `Some(nuevo_estado)` solo
  en una transición real. Reglas idénticas a las que ya tenía el store:
  `AiResponseStarted` → Thinking (correlacionando el fin de la generación
  por su propio `request_id`, porque esos eventos no llevan `profile_id`,
  del mismo modo que evitaba antes reaccionar a la generación de otro
  perfil), `VoicePlaybackStarted/Finished/Error` → Talking/Idle,
  `CallStateChanged` → Thinking/Talking/Idle según la fase de la llamada
  (sin guarda de perfil, porque solo hay una llamada activa a la vez).
  Cada conexión WebSocket tiene su propia instancia de `Reducer`
  (`routes.rs::handle_socket`) en vez de compartir una capa de estado
  derivado entre conexiones — más simple, y a la escala de "OBS tiene una
  fuente de navegador abierta" no cuesta nada.
- `VisualState` (en `reducer.rs`) es un tipo **deliberadamente separado**
  de `aivt_database::AvatarState` — ese último nombra *columnas de
  almacenamiento de archivos* (qué PNG subió el perfil para qué estado),
  mientras que `VisualState` es el estado *en vivo* que un espectador
  debería estar viendo ahora mismo. Mantenerlos separados evita que este
  crate dependa para nada de `aivt-database`.
- `overlay_html.rs`: la página que sirve `GET /overlay` es HTML/CSS/JS
  plano, sin framework ni build step — no es parte del bundle Vite/React,
  se sirve directo desde Rust como un string embebido. Precarga las tres
  imágenes de estado al cargar la página (mismo motivo que
  `useAvatarStateImages` del lado React: pedir la imagen recién al cambiar
  de estado pierde la carrera contra frases de prueba cortas) y alterna
  cuál es visible con opacidad. El WebSocket se reconecta solo con un
  backoff fijo de 2s si se corta, porque OBS mantiene una fuente de
  navegador cargada indefinidamente entre reinicios de la app — sin
  reconexión, un reinicio de Project Ciel dejaría el avatar congelado en
  OBS hasta que alguien recargara la fuente a mano.
- Registrado en `ServiceManager` como `AvatarServerService` (implementa
  `ManagedService`, igual que Twitch/Discord/Call/Proactivo) — `start()`
  levanta el listener y guarda un `CancellationToken` para poder apagarlo
  con `stop()` sin matar el proceso.

**Backend — ajuste opt-in con auto-reanudado**
- `AvatarServerSettings` (`crates/services/settings/src/preferences.rs`):
  `{ enabled: bool, port: u16 }`, `enabled: false` y `port: 47823` por
  default. El puerto no es configurable desde la UI todavía (sin comando
  para cambiarlo) — queda fijo en 47823 salvo que se edite el ajuste
  guardado directamente.
- Opt-in como Twitch/Discord/Call/Llamada (nunca bindea un puerto sin que
  el usuario lo pida explícitamente desde la pestaña Avatar), pero **a
  diferencia de esos, se auto-reanuda en cada lanzamiento futuro** una vez
  activado una vez — `src-tauri/src/lib.rs`'s `setup()` lee
  `avatar_server_settings()` una sola vez al arrancar (con `block_on`,
  antes de `app.manage(...)`) y llama `service_manager.start("avatar_server")`
  si `enabled` es `true`. Decisión deliberada: a diferencia de Twitch/
  Discord/Llamada (que el usuario inicia a mano cada vez que quiere
  transmitir), la fuente de OBS típicamente se deja siempre activa como
  parte de la escena, así que pedirle al usuario que la reactive a mano en
  cada arranque de la app sería fricción sin beneficio.
- `avatar_server_get_status`/`avatar_server_set_enabled`
  (`src-tauri/src/commands/avatar.rs`): status combina el ajuste persistido
  (`enabled`, `port`) con el estado en vivo del `ServiceManager`
  (`stopped`/`starting`/`running`/`error`/`stopping`), para que la pestaña
  Avatar sepa distinguir "apagado a propósito" de "activado pero todavía
  arrancando o en error" sin asumir nada sobre lo que dejó la sesión
  anterior. `set_enabled` persiste el flag y arranca/para el servicio en
  la misma llamada.

**Frontend**
- `src/stores/avatarStore.ts` **ya no controla el avatar que ve OBS** — ese
  rol ahora lo cumple enteramente `reducer.rs` del lado del servidor,
  porque una pestaña de navegador plana dentro de OBS/CEF no tiene puente
  IPC de Tauri al que este store pudiera escuchar. El store sigue
  existiendo, pero reducido a alimentar la insignia "En vivo: X" de la
  vista previa **dentro de la app** (`AvatarEditor.tsx`) — misma lógica de
  reducción que antes (duplicada a propósito entre TS y Rust; ver el
  comentario en `reducer.rs` sobre por qué), pero ya no es la fuente de
  verdad de lo que se transmite.
- `src/features/avatar/AvatarEditor.tsx`: además de la subida de imagen por
  estado y la vista previa manual (ya existían), agrega una sección nueva
  con un `Switch` ("Activar fuente OBS") que llama a
  `setAvatarServerEnabled` y, cuando está activo, un `Input` de solo
  lectura con la URL completa (`http://127.0.0.1:{port}/overlay?profileId={id}`)
  más un botón "Copiar" — esa URL es literalmente lo único que hay que
  pegar en OBS como Browser Source. El estado inicial se lee al montar con
  `getAvatarServerStatus()`.
- `src/lib/ipc/avatar.ts`: agrega `getAvatarServerStatus`/
  `setAvatarServerEnabled` sobre los comandos Tauri nuevos; `AvatarServerStatus`
  valida con Zod igual que el resto de las respuestas de IPC.
- **Eliminados por completo**: `src/windows/avatar/AvatarOverlay.tsx`, el
  split de `main.tsx` que decidía entre montar `<App/>` o esa ventana
  mínima según `?window=avatar` en la URL, `src-tauri/capabilities/avatar.json`,
  los tres comandos `avatar_window_*`, y el acceso rápido de
  Mostrar/Ocultar/Siempre-visible que tenía `src/app/Sidebar.tsx` para la
  ventana flotante — ninguno de estos tiene sentido con una fuente de
  navegador (no hay ventana de Tauri que mostrar/ocultar/fijar). La pestaña
  Avatar del sidebar se mantiene, apuntando ahora al switch + URL descritos
  arriba.
- `tauri.conf.json`: `"windows"` volvió a tener una sola entrada (`"main"`)
  — la ventana `"avatar"` se borró junto con el resto. `macOSPrivateApi:
  true` (agregado en el diseño original para la transparencia de esa
  ventana) **quedó en el archivo, sin tocar** — configuración muerta ahora
  que no hay ninguna ventana transparente, pero inofensiva, así que no se
  tocó como parte de este rediseño.

### Decisiones de diseño (documentadas, no accidentales)
- **Un `Reducer` por conexión WebSocket, sin estado compartido entre
  fuentes** — si alguna vez hay dos fuentes de OBS abiertas para el mismo
  perfil (dos escenas, por ejemplo), cada una calcula su propio estado de
  forma independiente en vez de compartir una única suscripción — más
  simple de razonar, sin ahorro real de recursos a esta escala.
- **La URL de la fuente lleva el `profileId` como query param fijo** — si
  se cambia el perfil activo en la app, la fuente de OBS ya agregada
  **no** sigue sola al nuevo perfil (igual que la limitación equivalente
  del diseño anterior). Hay que copiar la URL del perfil nuevo y
  actualizar (o agregar) la fuente en OBS a mano. Limitación aceptada, no
  un bug — mismo criterio que antes.
- **Sin backoff configurable ni límite de reintentos en la reconexión del
  overlay** (`overlay_html.rs`) — un backoff fijo de 2s es suficiente
  porque el único cliente real es OBS, que no tiene prisa ni penaliza
  reintentos frecuentes, a diferencia de una integración con un servicio
  externo con rate limits (comparar con la falta de backoff en EventSub,
  ver "Problemas conocidos" de Fase 9).

### Diseño anterior (reemplazado por completo)
El diseño original era una **segunda ventana de Tauri**, transparente y
sin bordes (`src/windows/avatar/AvatarOverlay.tsx`), pensada para
capturarse en OBS como fuente de "Captura de ventana". Funcionaba, pero
con varios bugs encontrados probando en vivo y corregidos en su momento:
click-through en los píxeles transparentes de Windows/WebView2 (mitigado
con una opacidad casi-cero, `bg-black/[0.02]`); el arrastre de la ventana
roto porque `core:window:allow-start-dragging` no está en el
permission-set `default` del plugin window de Tauri; doble clic
maximizando la ventana por el script de arrastre interno de Tauri
(corregido con `.maximizable(false)`); y un primer intento de logs de
diagnóstico con `RUST_LOG=debug` que saturó el archivo de log por el
volumen de logs de crates de terceros (wry/tao/hyper). Estructuralmente,
además, estaba **atada a un solo perfil a la vez** y dependía de que
`avatar_window_hide` ocultara la ventana en vez de cerrarla (pedido
explícito del usuario en su momento) para que la fuente de "Captura de
ventana" en OBS no se quedara en negro y hubiera que volver a
seleccionarla cada vez. Ninguno de estos problemas es exclusivo de una
mala implementación — son limitaciones estructurales de capturar una
ventana del SO desde OBS. Migrar a una fuente de navegador con URL fija
(el diseño actual, arriba) elimina esa categoría entera de problemas: OBS
ya sabe cargar una Browser Source de forma nativa, sin depender de qué
ventana esté abierta ni de trucos de click-through/arrastre/permisos de
plugin.

### Decisiones de alcance (siguen vigentes)
- Sin Live2D/VRM todavía (fuera de alcance explícito de "reactive PNG
  first" — sección 53 del brief).

## Fase 6 — Llamada IA

**Nota de esta reconstrucción (2026-09-16)**: esta sección no se escribió
en el momento en que se construyó la fase — quedó como deuda de
documentación (ver "Nota posterior" en Fase 9, más arriba). Reconstruida
ahora a partir del código real (`crates/services/call`,
`src/features/call/CallPage.tsx`), no de un registro en vivo de la
sesión original. Los dos bugs listados abajo sí están confirmados con
certeza — quedaron documentados como comentarios en el propio código
fuente (`service.rs`/`segmenter.rs`), citando explícitamente esta
sección por nombre.

### Completado

**Backend** (`crates/services/call`) — pipeline en tiempo real:
micrófono → detección de voz (VAD) → transcripción (STT) → IA → voz
(TTS), corriendo en loop mientras la llamada esté activa.
- `CallService` implementa `ManagedService` (mismo trait que
  `TwitchService`/`DiscordService`) — `start()` abre el micrófono y
  lanza el pipeline completo, `stop()` lo cierra todo y corta cualquier
  voz que esté sonando en ese momento.
- `capture.rs` (`MicCapture`): dueño de un stream de entrada `cpal` en
  su propio hilo de SO (los handles de audio de la plataforma no son
  `Send` de forma confiable). Siempre captura el formato nativo que
  reporta el dispositivo por defecto y hace la conversión a mono 16kHz
  i16 PCM por software (downmix + resample lineal) — la mayoría de
  dispositivos/drivers solo soportan su propio formato de mezcla nativo
  (comúnmente f32 a 44.1/48kHz), así que pedirle un formato específico
  al dispositivo es pedir algo que puede simplemente fallar.
  `list_input_devices()` alimenta el selector de micrófono en la UI.
- `segmenter.rs` (`Segmenter`): convierte el stream continuo de PCM en
  utterances discretas vía `webrtc-vad`, con debounce de arranque
  (~90ms de voz sostenida para confirmar que alguien empezó a hablar) y
  un mínimo de contenido realmente hablado (300ms) antes de considerar
  la utterance completa "digna de transcribirse" — ver el bug del ruido
  de fondo más abajo. Emite tres eventos: `OnsetDetected` (para mostrar
  "Escuchando" cuanto antes), `SpeechConfirmed` (para el barge-in, ver
  abajo) y `UtteranceComplete` (la utterance completa, lista para STT).
- `Inner::transcribe`: transcribe con `WhisperCppProvider` (el mismo
  whisper.cpp embebido de Configuración > Transcripción de voz), con un
  `transcription_prompt()` que sesga el reconocimiento hacia el
  nombre/apodo del personaje activo y las personas ya registradas
  (`PeopleRepository::top_for_prompt`) — ver el bug "si él" más abajo.
- `Inner::should_respond_to`: mismo criterio que el `mentioned_only` de
  Twitch — responde a todo, o solo cuando se lo llama por nombre/apodo o
  una frase personalizada (`contains_word`, compartido con Twitch).
- `Inner::generate_reply`: misma forma que el `generate_reply` de
  Twitch (vía `AIOrchestrator`, con `CurrentTimeTool` siempre disponible
  y `BraveSearchTool` si la búsqueda web está activada — Fase 10b), pero
  sin el límite de longitud de Twitch (una respuesta de llamada no se
  publica en ningún lado con límite de caracteres) y cancelable a mitad
  de generación para que el barge-in pueda cortarla.
- `Inner::speak_and_wait`: a diferencia del `spawn_speak` de Twitch
  (fire-and-forget), espera a que la voz realmente termine de sonar (o
  sea interrumpida) antes de devolver el control — el loop de la
  llamada necesita saber cuándo volver a escuchar el siguiente turno.
- `Inner::run_orchestrator`/`handle_utterance`: la máquina de estados
  que publica `CallState` (Idle/Listening/Transcribing/Thinking/
  Speaking/Error vía `EventBus`) y resuelve el **barge-in**
  (interrumpir al personaje a mitad de frase): en modo "responder a
  todo", cualquier voz confirmada cancela de inmediato lo que el
  personaje esté diciendo; en modo frase-de-activación, espera a
  confirmar que la nueva utterance de verdad merece respuesta antes de
  cortar el turno anterior — así no se queda en silencio por una
  conversación de fondo que no lo llamaba a él.
- Silenciar el micrófono se implementa en un único punto (el callback
  de captura revisa un `AtomicBool` antes de reenviar las muestras) —
  sin un camino de pausa/reanudación separado que mantener sincronizado.
- `CallPolicy`: dispositivo de micrófono, modo de respuesta (todo vs.
  frase de activación personalizada), sensibilidad del VAD, y el
  tiempo de silencio (ms) que da por terminada una utterance.

**Frontend** (`src/features/call/CallPage.tsx`, `callStore`):
- `PolicyCard`: iniciar/colgar la llamada, silenciar micrófono, y **dos
  indicadores siempre visibles** (tú/personaje) en vez de una sola
  insignia que va cambiando de texto — pedido del usuario para tener
  una respuesta clara a "¿me está escuchando ahora mismo?" separada de
  "¿qué está haciendo el personaje?", ya que una sola insignia
  cambiante es fácil de perderse a mitad de conversación.
- `TranscriptCard`: transcripción en vivo (líneas de usuario/personaje/
  error con hora), mismo patrón que los feeds de Twitch/Discord.
- Selector de micrófono, modo de respuesta, campo de frase de
  activación (solo visible en ese modo), sensibilidad del VAD y tiempo
  de silencio — todo en `PolicyCard`.

### Problemas encontrados y corregidos
- **"Ciel" se transcribía como "si él"**: sin contexto, Whisper
  reconocía el nombre del personaje como la frase común más cercana que
  sí conocía. Corregido con `transcription_prompt()`: le da a Whisper
  una frase de contexto mencionando el nombre/apodo del personaje activo
  y, además, una lista corta de personas ya registradas
  (`PeopleRepository`) — pedido explícito del usuario de que funcionara
  "de forma general" para cualquier nombre, no solo el del propio
  personaje, incluyendo nombres poco comunes o japoneses.
- **Alucinaciones tipo "¡Suscríbete!" con un micrófono ruidoso**:
  Whisper es conocido por inventar texto plausible (frases al estilo
  YouTube) cuando se le da un clip que es mayormente silencio/ruido de
  fondo. El debounce de arranque por sí solo (~90ms) no bastaba para
  filtrar esto. Corregido con `MIN_VOICED_FRAMES` (300ms de voz
  realmente detectada, sin contar el silencio final): una utterance que
  no llega a ese mínimo se descarta antes de llegar a STT, en vez de
  transcribirse y volver como texto inventado con apariencia creíble.

### Decisiones de alcance (documentadas, no accidentales)
- **STT vía whisper.cpp embebido, no un servidor HTTP/Docker** — ver la
  "Nota posterior" de la sección de Fase 9 más arriba para el porqué
  (decisión explícita del usuario, pensando en vender la app sin
  pedirle Docker al cliente final).
- **Sin estado de ánimo (mood) en la Llamada** — mood es exclusivo de
  Chat por ahora (ver `docs/ADR/006-mood.md`).
- **El barge-in "eager" (cortar apenas se confirma voz) solo aplica en
  modo "responder a todo"** — en modo frase-de-activación, cortar al
  personaje antes de saber si la utterance nueva lo menciona arriesga
  silenciarlo sobre una conversación de fondo que no era para él.
- **El micrófono se guarda por nombre, no por un id estable** — mismo
  criterio que el avatar reactivo con las imágenes por estado: usar el
  id opaco que expone `cpal` (`id()`/`description()`) habría significado
  guardar ese id en ajustes *y* mantener una búsqueda aparte solo para
  mostrar el nombre legible en la UI; el nombre por sí solo es preciso
  para el caso común de unos pocos dispositivos con nombres distintos.

## Fase 5 — Voz

### Completado

**Backend**
- Migración `0008_voice.sql`: tabla `profile_voice` (1:1 con `profiles`,
  mismo patrón que `profile_identity`/`profile_personality` — `enabled`,
  `voice_id`, `rate`, `pitch`, `volume`), con backfill para perfiles creados
  antes de esta fase.
- `crates/database/src/repositories/profiles.rs`: `ProfileVoice` +
  `ProfileVoiceUpdate`, incluidos en `Profile`, insertados/leídos junto con
  identity/personality en `create`/`get`/`duplicate`; `set_voice` aparte
  (se edita desde su propia pestaña Voz, no junto con Identidad).
- `crates/providers/tts` (antes vacío, sección 3 del brief): trait
  `TTSProvider` (`list_voices`/`speak`/`stop`) + `AivtTtsProvider`, la única
  implementación que construye la app. Enruta cada llamada según
  `SpeakRequest::backend`:
  - **Nativo** (`NativeEngine`, vía crate `tts` — WinRT en Windows,
    AVFoundation en macOS, Speech Dispatcher en Linux): sin configuración,
    funciona de inmediato. Un `Mutex<tts::Tts>` propio evita depender de
    los `unsafe impl Send/Sync` que el propio crate declara sobre un `Rc`
    interno. Eventos de inicio/fin de habla vía `on_utterance_end`/
    `on_utterance_stop`.
  - **HTTP** (`HttpEngine`): contrato `/audio/speech` compatible con OpenAI
    (igual que `OpenAiCompatibleProvider` trata LM Studio como "este mismo
    contrato, apuntado en local") — para un servidor Piper/XTTS/otro que el
    usuario ya tenga corriendo, o un servicio en la nube. Reproduce el
    audio recibido con `rodio`, en un hilo de audio dedicado
    (`AudioPlayer`) para no depender de si `cpal::Stream` es `Send`.
  - Sin "listar voces" para HTTP (no hay endpoint estándar) — el campo
    `voice_id` es texto libre en ese caso.
- `AppEvent::VoicePlaybackStarted/Finished/Error` — igual que
  `AiResponse*`, para que la UI sepa qué mensaje está sonando ahora mismo.
- `aivt_service_settings::{TtsBackendKind, TtsConnectionSettings,
  TtsConnectionInput}` — conexión TTS **global** (qué motor usa toda la
  app), separada de la voz **por perfil**: mismo patrón que
  `llm_connection`, pero por defecto ya resuelve a "nativo" sin que el
  usuario tenga que guardar nada primero.
- `ChatService`: `GenerationContext` gana `tts: Option<TtsInvocation>`;
  `maybe_speak` (detached, igual que `maybe_consolidate`) dispara la
  reproducción automática tras cada respuesta completa si el perfil tiene
  la voz activada — nunca bloquea ni retrasa el evento `AiResponseFinished`.
- Comandos Tauri: `voice_list_voices`, `voice_get_settings`,
  `voice_set_settings`, `voice_speak` (reproducción manual — repetir un
  mensaje del chat, o "Probar voz" con la configuración *sin guardar* vía
  `voice_override`, igual que `settings_test_llm_connection` prueba una
  conexión aún no guardada), `voice_stop`; `settings_get_tts_connection` /
  `settings_set_tts_connection`.
- `MockTTSProvider` (`crates/testing`) para probar el hook de auto-play sin
  un motor de voz real.

**Frontend**
- `src/features/voice/VoicePage.tsx` + `VoiceEditor.tsx` (reemplaza el
  placeholder): activar/desactivar lectura automática, elegir voz
  (desplegable si el backend es nativo, texto libre si es HTTP),
  velocidad/tono/volumen, "Probar voz" con texto editable y botón Detener.
- `src/features/settings/TtsSettings.tsx` (nueva pestaña real
  Configuración > TTS): elegir motor (nativo / servidor HTTP), URL y API
  key cuando es HTTP.
- `MessageBubble.tsx`: botón de altavoz en cada respuesta del personaje
  para reproducirla manualmente (independiente del auto-play), que cambia
  a botón Detener mientras esa respuesta concreta está sonando.
- `voiceStore.ts` (nuevo): estado global de "qué se está reproduciendo
  ahora" a partir de `AppEvent::VoicePlayback*` — lo consulta tanto
  `MessageBubble` (cualquier mensaje) como `VoiceEditor` (la vista previa),
  suscrito una sola vez en `App.tsx` como memoria/chat.
- `Profile`/`ProfileExport` (Rust y TS) ganan el campo `voice` — también se
  incluye en exportar/importar personajes (`.aivtprofile`), con
  `#[serde(default)]` para que un archivo exportado antes de esta fase
  siga importando sin el campo.

### Decisiones de alcance (documentadas, no accidentales)
- **STT (que el personaje entienda audio/mensajes de voz en el chat):
  deliberadamente fuera de esta fase**, decisión explícita del usuario tras
  explicarle el motivo: la transcripción nativa de Windows para apps de
  escritorio sin empaquetar tiene fallos de permisos conocidos (a
  diferencia de la síntesis nativa, que sí funciona sin fricción — ver
  `NativeEngine` arriba), y el camino confiable —un servidor local
  compatible tipo Whisper— necesita algo real contra qué probarlo en vivo,
  que el usuario no tiene todavía. Mismo criterio que dejó STT fuera de
  Fase 3. Queda pendiente para cuando haya un servidor STT confirmado.
- El backend HTTP de TTS se construyó completo (con tests) porque el
  usuario pidió explícitamente tener la capacidad de usarlo más adelante,
  pero **no se ha probado en vivo contra un servidor real** — a diferencia
  de LM Studio en Fase 2, aquí no hay un servidor ya confirmado disponible.
  El usuario probó las voces nativas y confirmó que se escuchan
  correctamente; el HTTP queda para cuando decida configurar un servicio.
- `rate`/`pitch`/`volume` del formulario Voz son los mismos para cualquier
  backend, pero el HTTP (contrato OpenAI `/audio/speech`) solo tiene
  equivalente para `rate` (como `speed`) — `pitch`/`volume` se ignoran ahí,
  con una nota visible en el formulario en vez de ocultarlos.

### Problemas conocidos
- La llamada síncrona de `tts::Tts::speak` en Windows (WinRT) puede
  bloquear brevemente el hilo async que la invoca mientras sintetiza el
  audio (normalmente bajo un segundo para una respuesta de chat típica) —
  aceptado como no es una ruta caliente (una vez por respuesta, en una
  tarea ya desacoplada); si se nota en uso real, la solución sería mover
  esa llamada a un hilo dedicado, igual que ya se hizo para el backend
  HTTP con `AudioPlayer`.

## Fase 4 — Memoria

### Completado

**Backend**
- Migración `0006_memory.sql`: tabla `memories` (content, summary, category,
  importance, confidence, source, source_id, is_pinned, timestamps de
  creación/actualización/último acceso, access_count) + `memories_fts`
  (FTS5 sobre contenido externo, sincronizada con triggers AFTER
  INSERT/UPDATE/DELETE — el patrón estándar de SQLite para no duplicar el
  texto). Añade también `conversations.memory_consolidated_through` —
  cuántos mensajes ya se consideraron para extracción, para no
  reprocesar los mismos mensajes en cada pasada automática (sección 39 del
  brief).
- `MemoriesRepository` (`crates/database`) — CRUD + `search` (FTS5
  `MATCH`, con cada término del usuario escapado entre comillas y unido con
  `OR`, para que comillas/operadores en la búsqueda nunca rompan la
  sintaxis de FTS5) + `list_for_conversation`-equivalente para conversación
  (`list_for_conversation` en `AttachmentsRepository`, reutilizado aquí como
  patrón) + `touch_access` (bookkeeping best-effort).
- `aivt-memory` (crate que existía como stub desde Fase 0, ahora real) —
  trait `MemoryManager` (`list_for_profile`, `search`, `create`,
  `set_pinned`, `delete`, `extract_and_store`) + `MemoryService`. La
  extracción reutiliza el mismo `LLMProvider`/`stream_chat` que ya usa el
  chat (sección "Decisiones de alcance" abajo) con un prompt dedicado
  (`extraction.rs`) que pide un array JSON de memorias candidatas;
  `temperature` forzada a 0.2 para esa llamada específica (análisis, no
  charla) sin importar lo que el usuario tenga configurado. El parseo
  intenta JSON directo y, si el modelo añadió texto alrededor, extrae el
  primer `[...]` — nunca falla la operación completa, una respuesta
  ininterpretable simplemente no genera memorias.
- `AIOrchestrator::stream_respond` gana un cuarto parámetro
  `relevant_memories: &[Memory]`, renderizado como una sección "Recuerdos
  relevantes..." al final del system prompt cuando no está vacío — el
  orchestrator solo renderiza lo que le pasan, nunca consulta memoria por
  sí mismo (`aivt-orchestrator` sigue sin depender de `aivt-memory`,
  exactamente como ya anticipaba el comentario de `PromptComposer::compose`
  desde Fase 1).
- `ChatService`: antes de cada generación, busca memorias relevantes
  (`memory.search(profile_id, último_mensaje_del_usuario, 5)`) y las pasa al
  orchestrator — una fallo de retrieval nunca bloquea la respuesta del chat
  (se registra y se continúa con la lista vacía, sección 81 del brief:
  fallo de una integración no debe afectar a las demás). Tras una
  generación completada con éxito, si se acumularon
  `CONSOLIDATION_BATCH_SIZE = 6` mensajes nuevos desde la última
  consolidación, lanza la extracción como una tarea totalmente
  independiente (`tokio::spawn` separado, nunca `.await`eada dentro de
  `run_generation`) — así una extracción lenta nunca mantiene ocupado el
  slot de "generación en curso" de la conversación ni retrasa el evento
  `AiResponseFinished` que el usuario ya vio. Nuevo método
  `extract_memories_now` (mismo mecanismo, sin el umbral, para el disparo
  manual).
- Comandos nuevos: `chat_extract_memories` (extracción manual inmediata,
  ignora el umbral automático) y `memory_list` / `memory_search` /
  `memory_create` / `memory_set_pinned` / `memory_delete` — estos últimos
  van directo contra `AppState.memory` (la misma instancia que usa
  `ChatService` internamente, compartida vía `Arc<dyn MemoryManager>`), no
  a través de `ChatService`.
- `aivt-testing` gana `MockMemoryManager` (mismo patrón que
  `MockLLMProvider`/`MockSecretStore`) — registra cada llamada a
  `extract_and_store` para que los tests de `ChatService` puedan afirmar
  que la consolidación automática se disparó (o no) sin necesitar un LLM
  real.

**Frontend**
- Pestaña Memoria completamente real (antes placeholder): buscador (FTS del
  backend), tarjetas de memoria (contenido, categoría, importancia, fijar/
  desfijar, eliminar), diálogo "Nueva memoria" para crear una a mano.
- `memoryStore` (zustand) — mismo patrón que `profileStore`: estado por
  perfil, `search()` cae a `list()` cuando la búsqueda está vacía.
- Menú de cada conversación en Chat gana "Extraer memorias" — dispara
  `chat_extract_memories` de inmediato (sin esperar al umbral de 6
  mensajes), útil tanto para el usuario como para verificar la feature sin
  tener que escribir varios mensajes primero.
- `DropdownMenuItem` (design system) gana soporte para `disabled` — Radix
  ya lo soportaba en la primitiva, solo faltaba exponerlo; necesario para
  deshabilitar "Extraer memorias" mientras una extracción está en curso.

### Tests
- Rust: **127/127 en verde** — 36 tests nuevos: 8 en `MemoriesRepository`
  (incluye búsqueda con comillas/operadores FTS5 sin romper, scoping por
  perfil, orden fijado+importancia) + 2 más para `update` (edita
  contenido/categoría/importancia sin tocar `source`/`confidence`, y el
  cambio se refleja en la búsqueda FTS) + 3 más para el filtrado de
  palabras vacías de `to_fts_query` (descrito abajo), 1 en
  `ConversationsRepository` (marcador de consolidación), 13 en
  `aivt-memory` (7 de parseo de extracción — JSON limpio, con prosa
  alrededor, categoría desconocida, scores fuera de rango — y 6 del
  servicio contra `MockLLMProvider`), 2 en el orchestrator (memoria
  inyectada en el prompt / ausente cuando no hay memorias), 7 en
  `ChatService` (retrieval inyectado en el request real, consolidación
  automática disparada exactamente al llegar al umbral, NO disparada por
  debajo, extracción manual ignora el umbral, extracción manual sobre
  conversación inexistente falla, un fallo de retrieval no bloquea la
  respuesta).
- Frontend: 22 tests nuevos — 9 de `memoryStore` (load/search con y sin
  query/create/pin/remove/update/`handleAppEvent` refresca solo si el
  perfil ya se había cargado, e ignora eventos ajenos) + 2 de `MemoryPage`
  montado de verdad (Testing Library), incluyendo el mismo tipo de
  regresión que ya mordió en Fase 2 (selector de Zustand sin entrada
  todavía para el perfil activo) + 7 de `computeFitDimensions` (redondeo,
  cuadrada/vertical/horizontal, nunca agranda, sin dimensión en cero) + 3
  de `formatDateTime`. **Total frontend: 50/50 en verde.**
- Rust, ronda del rediseño título/resumen/detalle: 8 tests más — `update`
  ahora también edita `title` sin tocar procedencia, búsqueda por título,
  4 sobre `resolved_title`/`resolved_summary` (usa el del modelo, cae a
  contenido truncado si falta o viene en blanco, resumen ausente vs.
  presente), 1 confirmando que el prompt de extracción menciona las tres
  partes, 1 en el orchestrator confirmando que un resumen presente se usa
  en vez del contenido más largo.
- Rust, ronda de ajustes tras probar el rediseño en vivo: 1 test más —
  `memory_section` reescrito para incluir tanto el resumen como el detalle
  (antes solo mandaba uno u otro), con la instrucción de "detalle
  adicional" mencionada solo cuando al menos una memoria del lote
  realmente la tiene (si no, un test lo detectó: el texto de instrucciones
  mencionaba "detalle adicional" aunque ninguna memoria tuviera uno).
- Rust, ronda del arreglo de la ventana de retrieval: 3 tests más —
  `retrieval_query` usa una ventana de mensajes en vez de solo el último,
  nunca mira más atrás de la ventana configurada, y un test de extremo a
  extremo que reproduce el escenario reportado (tema mencionado →
  seguimiento vago → la consulta de búsqueda registrada sigue conteniendo
  la palabra del tema). **Total backend: 140/140 en verde.**

### Verificación realizada en este entorno
- `cargo check --workspace` y `cargo test --workspace` — limpio, **140/140**
  (incluye todos los arreglos/mejoras que salieron de probar en vivo,
  descritos abajo, más el rediseño de memorias y sus ajustes posteriores).
- `npx tsc -b` — limpio.
- `npx vitest run` — **50/50 en verde**.
- `npm run tauri dev` — **probado en vivo por el usuario** en varias
  rondas, incluyendo el rediseño título/resumen/detalle (migración
  `0007_memory_title`): migración aplicada sin problemas, extracción
  manual y automática funcionando, memoria manual usada correctamente por
  el personaje sin habérselo mencionado en el chat, búsqueda FTS
  funcionando, edición de memorias, redimensionado de imágenes del chat,
  orden cronológico + fecha en la Memoria tab. El usuario probó los tres
  ajustes de esa ronda (recorte del borde de foco, detalle disponible para
  la IA bajo petición, ejemplo en el prompt de extracción) y encontró que
  la IA seguía sin poder recuperar el detalle — llevó primero al arreglo de
  la ventana de retrieval (el resumen ya se lee bien tras eso), y después
  a la decisión de abandonar el envío condicional del detalle por completo
  (ver "Problemas encontrados y corregidos" arriba). **Ese último cambio
  (revertir `memory_section` a solo `summary`) es lo único de esta fase que
  compila y pasa todos los tests pero todavía no se ha vuelto a probar en
  vivo** — pendiente.

### Decisiones de alcance (documentadas, no accidentales)
- **La extracción reutiliza el `LLMProvider` del chat, no un backend de
  resumen aparte.** Ninguna dependencia externa nueva, y es la conexión que
  el usuario ya tiene configurada y probada — coherente con la misma
  decisión tomada en Fase 3 para el escaneo de scope de audio/STT.
- **`people`/`memory_people` (seguimiento explícito de personas) quedan
  fuera de esta fase**, aunque estaban en el diseño original del esquema.
  Nada las consume todavía (no hay UI de "personas" ni resolución de quién-
  es-quién entre canales) — crear esas tablas ahora sería exactamente el
  "esquema muerto" que este proyecto ha evitado deliberadamente en cada
  fase anterior (el mismo criterio que ya quitó `transcript`/`duration_ms`
  de `attachments` en Fase 3). Se retoma cuando Twitch/Discord realmente
  necesiten desambiguar usuarios.
- **Umbral de consolidación fijo (6 mensajes), no configurable por el
  usuario todavía.** Es un valor razonable para probar la feature sin
  esperar demasiado ni disparar extracción en cada mensaje; exponerlo como
  ajuste es un cambio aislado y de bajo riesgo si hace falta más adelante.
- **Fusión de memorias — implementada después del cierre de esta fase**
  (pedido explícito del usuario, 2026-09-15, ver "Mejoras posteriores a
  Fase 11" más arriba para el detalle completo). Botón manual en
  Configuración > Memoria; la IA solo propone grupos a fusionar, el
  usuario revisa y confirma antes de que se borre ninguna memoria
  original.

### Problemas encontrados y corregidos (probando en vivo)
- **La pestaña Memoria no se actualizaba sola** cuando la consolidación
  automática (o el botón "Extraer memorias") creaba memorias nuevas
  mientras el usuario no estaba en esa pestaña — solo aparecían al volver a
  cargarla por otra razón. Causa: la consolidación automática corre en una
  tarea de fondo totalmente independiente (`tokio::spawn` separado — ver
  arriba) sin ninguna forma de avisarle al frontend que había datos nuevos.
  Corregido añadiendo `AppEvent::MemoriesUpdated { profile_id }` — se
  publica tanto tras la extracción automática (solo si de verdad se creó
  algo) como tras la manual, y `memoryStore.handleAppEvent` (suscrito a
  nivel de `App.tsx`, igual que `chatStore`) refresca esa pestaña **solo
  si ya se había cargado antes** para ese perfil, para no hacer fetches de
  datos que nadie está mirando.
- **No se podían editar memorias ya guardadas**, solo fijar/desfijar y
  eliminar — pedido explícito del usuario tras la primera prueba en vivo
  ("por si algo está mal"). Añadido `MemoriesRepository::update` +
  `MemoryManager::update` + comando `memory_update`, y en el frontend el
  diálogo "Nueva memoria" se generalizó a un formulario compartido
  (`MemoryFormDialog`) que sirve tanto para crear como editar, con un botón
  de lápiz nuevo en cada tarjeta.
- **El personaje no reconocía memorias editadas/relevantes en preguntas
  formuladas con naturalidad** — el usuario tuvo que preguntar varias
  veces y finalmente citar casi textualmente lo que había modificado para
  que el personaje "encontrara" esa memoria. Causa real: `to_fts_query`
  (usada tanto por la búsqueda de la pestaña Memoria como por el retrieval
  automático del chat, que pasa el mensaje del usuario tal cual como
  consulta) unía TODAS las palabras del mensaje con `OR` sin distinguir
  relevancia — una pregunta natural como "¿qué me dijiste sobre...?" pesaba
  igual "qué"/"me"/"dijiste"/"sobre" que la palabra de contenido real,
  diluyendo el ranking BM25 de FTS5 lo suficiente para que la memoria
  correcta no quedara entre los primeros `MEMORY_RETRIEVAL_LIMIT = 5`
  resultados. Corregido filtrando una lista de palabras vacías (artículos,
  preposiciones, pronombres, verbos auxiliares comunes en es/en) antes de
  construir la consulta FTS5 — con reserva: si una consulta resulta ser
  *solo* palabras vacías, se usan todas sin filtrar en vez de no
  buscar nada. Cubierto con tests unitarios puros sobre `to_fts_query`
  (no depende del ranking real de FTS5, que no es determinista de probar
  con precisión en un corpus de prueba pequeño).
- **Las imágenes adjuntas en el chat se enviaban a tamaño completo** —
  pedido del usuario para reducir lo que se manda a LM Studio/servicios de
  IA. Añadido `resizeImageToFit` (`src/lib/files.ts`): redimensiona en el
  cliente (canvas) antes de leerla como base64, tope de 1500px en el lado
  más largo (funciona igual para imágenes cuadradas, verticales u
  horizontales — escala proporcional, nunca agranda una imagen ya más
  pequeña), y deja los GIF animados intactos sin tocar (dibujarlos en un
  canvas los aplanaría a su primer frame). La lógica de cálculo de
  dimensiones (`computeFitDimensions`) se separó como función pura,
  testeable sin un motor de renderizado real — jsdom no implementa
  `createImageBitmap`/canvas, así que el redimensionado real solo se pudo
  verificar en la app de verdad.
- **La pestaña Memoria no mostraba fecha/hora y ordenaba por importancia**
  — pedido del usuario tras usarla: quería ver cuándo se creó cada memoria
  y verlas de más nueva a más antigua (arriba las más recientes).
  `MemoriesRepository::list_for_profile` cambió su `ORDER BY` de
  `is_pinned DESC, importance DESC, created_at DESC` a
  `is_pinned DESC, created_at DESC` — fijado sigue ganando (es una
  decisión explícita del usuario de mantener algo visible), pero la
  importancia ya no interviene en el orden, solo se sigue mostrando como
  dato en la tarjeta. En el frontend, cada tarjeta ahora muestra
  `createdAt` formateado con `Intl.DateTimeFormat` (nuevo
  `src/lib/formatDate.ts`, respeta el idioma activo de la app).
- **Rediseño de memorias: título + resumen + detalle** — pedido del usuario
  tras usar la Memoria tab en vivo: quería poder organizar mejor las
  memorias con un título corto, y separar "un contexto resumido" de "una
  memoria más extendida y detallada pero no demasiado extensa". El esquema
  ya tenía `summary` desde el diseño original de Fase 4 pero nunca se
  usaba — encajó perfecto para el "resumen"; se añadió `title` como columna
  nueva (migración `0007_memory_title.sql`, que también recrea
  `memories_fts` porque FTS5 no admite `ALTER … ADD COLUMN` sobre una tabla
  virtual existente). `content` pasa a ser el "detalle" — el prompt de
  extracción ahora pide explícitamente las tres partes con un límite suave
  de ~500 caracteres para `content` ("denso, no pesado"), con reserva: si
  el modelo no manda `title`, se deriva truncando `content` a límite de
  palabra (mismo patrón que `title_from_first_message` de Fase 2, duplicado
  a propósito en vez de compartido entre crates — es un puñado de líneas
  muy específicas de cada contexto). Formulario y tarjeta del frontend
  rediseñados para las tres partes, con topes de longitud en el formulario
  (80/200/600 caracteres) para que "denso" no se convierta en "extenso" por
  accidente.

### Problemas encontrados y corregidos (probando el rediseño en vivo)
- **El borde de foco de los campos de texto se recortaba por la
  izquierda** en el formulario de memorias. Causa: el contenedor con
  scroll (`overflow-y-auto`) solo tenía padding a la derecha (`pr-1`) — por
  la spec de CSS overflow, un eje en `auto` fuerza al otro eje (que estaba
  implícito en `visible`) a comportarse también como `auto`, así que el
  contenedor recortaba horizontalmente también, y el anillo de foco (que
  se dibuja como `box-shadow`, hacia afuera del elemento) no tenía espacio
  reservado a la izquierda. Corregido con padding simétrico
  (`-mx-1 px-1`).
- **La IA no podía usar el detalle de una memoria, ni cuando el usuario
  pedía explícitamente más información** — `AIOrchestrator`'s memory
  section solo mandaba el resumen (o el detalle como *reemplazo*, nunca
  ambos). Sin infraestructura de tool-calling todavía (sección 107 del
  brief, deliberadamente no construida), la única forma de que el detalle
  esté "disponible bajo petición" es incluirlo en el contexto con una
  instrucción de uso selectivo. Reescrito `memory_section` para mandar
  siempre la línea de resumen (recuerdo rápido) y, cuando existe un
  detalle realmente distinto, añadirlo como "(detalle adicional: …)" con
  una instrucción explícita de usarlo solo si el usuario pide más
  contexto — nunca como reemplazo del resumen.
- **El prompt de extracción no garantizaba que el modelo rellenara bien
  las tres partes** (título con nombre real, resumen autocontenido y
  fácil de recordar, detalle suficientemente completo) — pedido explícito
  del usuario tras ver el comportamiento en vivo. Reforzado el prompt:
  el resumen ahora se pide explícitamente "autocontenido" (debe
  entenderse sin haber leído la conversación original, ya que es
  literalmente lo que se le recuerda al personaje después), y se añadió
  un ejemplo completo trabajado (frase de entrada → JSON esperado) — una
  técnica conocida para mejorar el cumplimiento de formato en modelos
  locales más pequeños, que no siempre siguen instrucciones de formato al
  pie de la letra sin un ejemplo concreto.
- **La IA seguía sin poder recuperar el detalle ni con las mejoras
  anteriores — se inventaba información en vez de encontrarla.** Causa
  real, más profunda que la del punto anterior: `run_generation` construía
  la consulta de retrieval únicamente con `history.last()` (el último
  mensaje). Un seguimiento vago como "cuéntame más sobre eso" no comparte
  ninguna palabra con la memoria real — el tema se mencionó uno o dos
  mensajes antes, no en ese turno — así que la búsqueda no encontraba
  *nada*, `memory_section` nunca se añadía al prompt, y el modelo, sin
  ningún recuerdo real en el contexto, inventaba una respuesta plausible.
  Corregido construyendo la consulta con una ventana de los últimos
  `RETRIEVAL_QUERY_WINDOW = 4` mensajes en vez de solo el más reciente, así
  las palabras clave del tema siguen alimentando la búsqueda durante varios
  turnos después de haberse mencionado. Cubierto con un test de extremo a
  extremo que reproduce exactamente el escenario reportado (mensaje con el
  tema → seguimiento vago → la consulta de búsqueda registrada todavía
  contiene la palabra del tema).
- **Con el arreglo de retrieval, el resumen ya se lee correctamente, pero
  el detalle ("(detalle adicional: …)") seguía sin funcionar de forma
  fiable.** Decisión del usuario tras esa prueba: en vez de seguir
  intentando que el modelo use el detalle condicionalmente (pedirle en
  texto que "solo lo use si el usuario pide más" es, en la práctica, pedir
  atención condicional a un modelo sin herramientas reales para eso —
  nunca iba a ser fiable), **se abandona esa idea por completo**. `content`
  vuelve a ser puramente organizativo — contexto extendido para que el
  propio usuario ordene mejor sus memorias en la Memoria tab — y
  `memory_section` vuelve a mandar solo `summary` (con `content` como
  respaldo únicamente cuando no hay resumen en absoluto, para memorias
  creadas antes del rediseño). Los dos tests que cubrían el envío
  condicional del detalle se reemplazaron por dos que confirman lo
  contrario: `content` nunca llega al modelo cuando hay resumen, y sí se
  usa como respaldo cuando no lo hay.

### Problemas conocidos
- La ventana de retrieval (Fase 4, corregido más arriba) ayuda con
  seguimientos inmediatos pero sigue siendo una heurística de palabras
  clave, no comprensión real — una referencia indirecta varios turnos
  después de mencionado el tema todavía puede no encontrar la memoria.
  Mejorarlo de verdad (embeddings semánticos en vez de solo FTS léxico) es
  una mejora
  post-MVP ya prevista en el brief original, no un bug de esta fase.
- `touch_access` (contador de veces que se usó una memoria) existe en el
  repositorio pero no está conectado a ningún flujo todavía — ninguna
  llamada real lo invoca aún. Queda listo para cuando se quiera mostrar
  "memorias más usadas" en la UI.
- El redimensionado de imágenes de Fase 3 solo se aplica al adjunto del
  chat (`ChatWindow`) — el avatar de perfil (Fase 1) sigue subiéndose a
  tamaño completo, a propósito: es una imagen pequeña de UI, no algo que
  viaje en cada request al LLM, así que el mismo problema no aplica ahí.

## Fase 3 — Multimodal

### Completado

**Backend**
- Migración `0005_attachments.sql`: tabla `attachments` (id, message_id,
  kind, path, mime_type, size_bytes, hash, created_at) — `kind` deja espacio
  para `'audio'` cuando llegue con su propia migración más adelante, en vez
  de crear columnas muertas (`transcript`/`duration_ms`) para una feature
  que todavía no existe.
- `AttachmentsRepository` (`crates/database`) — CRUD mínimo +
  `list_for_conversation` (un solo `JOIN` con `messages`, para poder
  enriquecer una conversación completa sin N+1 queries).
- `aivt-provider-llm`: `ChatMessage` ahora lleva `images: Vec<String>`
  (data URLs). `OpenAiCompatibleProvider` serializa `content` como string
  plano cuando el mensaje no tiene imágenes (compatibilidad máxima con
  servidores que no entienden el formato array) y como el array de partes
  tipadas de OpenAI (`type: text | image_url`) en cuanto hay al menos una.
- `AIOrchestrator::stream_respond` recibe un tercer parámetro
  `latest_user_image: Option<&str>`, que se adjunta solo al **último**
  mensaje del historial (el mensaje de usuario recién enviado). **Decisión
  de alcance documentada**: las imágenes de turnos anteriores no se
  reenvían al modelo en cada mensaje siguiente — se quedan guardadas y
  visibles en la UI, pero replayarlas en cada turno dispararía el uso de
  tokens sin necesidad real.
- `ChatService`: `send_message` acepta un `Option<NewAttachmentInput>`;
  cuando hay imagen, la guarda en disco con nombre direccionado por
  contenido (`<data_dir>/attachments/<hash[0..2]>/<hash>.<ext>`, dedup
  gratis si el mismo archivo se envía dos veces — sección 75 del brief:
  nunca base64 en SQLite), inserta la fila en `attachments`, y pasa el data
  URL original al `AIOrchestrator` para esa generación. `get_messages`
  ahora hace `JOIN` en memoria con los adjuntos de la conversación y
  devuelve `MessageView` (mensaje + `attachmentId`/`attachmentMimeType`
  opcionales) en vez de `Message` a secas — la capa de repositorio se
  mantiene SQL-puro; el enriquecido es responsabilidad del servicio (mismo
  criterio que separa `StoredLlmConnection` de `LlmConnectionSettings` en
  Fase 2). Nuevo método `get_attachment` (lee el archivo, lo devuelve como
  bytes — el comando lo pasa a base64, mismo patrón que
  `ProfileService::read_avatar`).
- `SettingsService`: `LlmConnectionSettings`/`Input` ganan `vision_enabled`
  — **override manual, no detección automática**. Un servidor
  OpenAI-compatible genérico no expone de forma fiable si el modelo
  cargado acepta imágenes, así que se le pregunta al usuario directamente
  (sección 84-85 del brief: "nunca se asume, se puede corregir") en vez de
  adivinar por el nombre del modelo. `#[serde(default)]` en el campo
  guardado para que las conexiones guardadas antes de esta fase sigan
  cargando sin el campo.
- Comandos nuevos/cambiados: `chat_send_message` acepta `image: Option<{
  base64, mimeType, extension }>` y **rechaza explícitamente** con
  `CHAT_VISION_NOT_ENABLED` si llega una imagen sin `vision_enabled` activo
  — defensa en profundidad; el composer ya oculta/deshabilita el botón,
  pero un IPC directo o un estado de UI obsoleto no deben poder colar una
  imagen en silencio. Nuevo `chat_get_attachment_base64`.

**Frontend**
- Composer del chat: botón de adjuntar imagen (`<input type="file"
  accept="image/*">`), deshabilitado con explicación (`title`) cuando
  `vision_enabled` es falso — nunca un botón que aparente funcionar sin
  hacerlo (sección 118 del brief). Miniatura de previsualización con opción
  de quitarla antes de enviar.
- `MessageBubble` muestra la imagen adjunta vía un nuevo hook
  `useAttachmentImage` (mismo patrón que `useProfileAvatar`: pide el
  archivo por IPC, lo cachea como `data:` URI local al componente).
- `ProfileAvatarBadge` se reutiliza sin cambios adicionales (ya extraído en
  el cierre de Fase 2).
- Configuración > Proveedores IA: switch "Visión (imágenes)" con texto
  explicando que es manual, no detectado.
- i18n (es/en) completo para todo lo anterior.

### Tests
- Rust: **91/91 en verde** (80 de Fase 2 + 11 nuevos: 4 en
  `AttachmentsRepository`, 1 en el orchestrator — verifica que la imagen se
  adjunta solo al último mensaje —, 3 en la serialización de
  `OpenAiCompatibleProvider` — string plano vs. array de partes —, 1 en
  `ChatService` que cubre el flujo completo adjuntar→guardar en
  disco→incluir en el request→aparecer en `get_messages`, y 2 en
  `SettingsService` para `vision_enabled`, incluyendo que una conexión
  guardada antes de esta fase sigue cargando).
- Frontend: 2 tests nuevos — `ConversationList` (heredado de Fase 2, sigue
  en verde) y un test nuevo de `chatStore.send` que verifica que la imagen
  llega tal cual a la capa IPC y que el adjunto devuelto por el backend
  queda guardado en el mensaje correspondiente. **Total frontend: 28/28 en
  verde.**

### Verificación realizada en este entorno
- `cargo check --workspace` y `cargo test --workspace` — limpio, **91/91**.
- `npx tsc -b` — sin salida.
- `npx vitest run` — **28/28 en verde**.
- `npm run tauri dev` — **probado en vivo por el usuario**: migración
  `0005_attachments` aplicada sin problemas, chat con imagen adjunta
  funcionando de extremo a extremo.

### Decisiones de alcance (documentadas, no accidentales)
- **Audio/mensajes de voz: deliberadamente fuera de esta fase.** El brief
  original agrupaba "Multimodal" como imagen + audio, pero construir
  transcripción de voz de verdad implica un `STTProvider` real — y a
  diferencia de LM Studio (que el usuario ya tiene configurado y probado),
  no hay todavía ningún servidor STT local confirmado disponible para
  probarlo de verdad. Meter un STT a medias, sin forma de verificarlo en
  vivo, es exactamente el "half-built feature" que el usuario pidió evitar.
  Queda para cuando Voz (Fase 5) o Llamada (Fase 6) construyan ese
  `STTProvider` real — la tabla `attachments` ya tiene `kind` listo para
  `'audio'` sin cambios de esquema.
- Solo **una** imagen por mensaje — no hay adjuntar varias a la vez. El
  esquema (`message_id` en `attachments`, no un array) sí soportaría varias
  sin cambios, pero la UI del composer no lo expone todavía; ampliarlo es
  un cambio de frontend aislado si hace falta más adelante.
- Sin validación de tamaño máximo de imagen — coherente con el mismo riesgo
  ya aceptado para el avatar de perfil desde Fase 1.

### Problemas conocidos
- Igual que en Fase 2: sin caché de cliente HTTP entre mensajes.
- La detección de si el modelo acepta imágenes es 100% manual — un modelo
  sin soporte real con `vision_enabled` activado simplemente fallará al
  generar (error del proveedor), no hay heurística que lo prevenga.

## Fase 2 — Chat

### Completado

**Backend**
- Migración `0004_chat.sql`: tablas `conversations` (id, profile_id, title,
  source, model, is_archived, timestamps) y `messages` (id, conversation_id,
  role, content, status, provider_id, model, input/output_tokens,
  created_at), con sus índices.
- `ConversationsRepository` / `MessagesRepository` (`crates/database`) —
  `MessageRole`/`MessageStatus` como enums Rust con `as_str()`/`parse()`
  (nunca strings sueltos comparados a mano), `NewMessage`, y `finalize()`
  para pasar un mensaje `streaming → complete` con su contenido y tokens
  finales.
- `aivt-provider-llm` (nueva crate) — trait `LLMProvider` (`test_connection`,
  `list_models`, `capabilities`, `stream_chat`) y `OpenAiCompatibleProvider`,
  la implementación concreta que sirve tanto para LM Studio como para
  cualquier servidor compatible con la API de chat completions de OpenAI
  (mismo provider, distinta URL base — sección 3 del brief). Parseo SSE
  manual (`sse.rs`, función pura `drain_sse_data_lines`, muy testeada) sobre
  `Response::chunk()` de `reqwest` en vez de `.bytes_stream()`, para no
  necesitar la feature `stream`. Mapea códigos HTTP (401/403, 404, 429) y
  errores de `reqwest` (timeout, conexión) a variantes específicas de
  `ProviderError`, nunca un error genérico.
- `aivt-orchestrator` — `AIOrchestrator::stream_respond(profile, history)`,
  ahora real: arma el prompt vía `PromptComposer` (Fase 1), recorta el
  historial a `MAX_HISTORY_MESSAGES = 20` y delega el streaming al
  `LLMProvider` activo. Es el único lugar del backend donde "el personaje
  responde" — Chat es el primer canal que lo demuestra; Call/Twitch/Discord
  reutilizarán exactamente este mismo método en sus fases.
- `aivt-service-chat` (nueva crate) — `ChatService`: `send_message` persiste
  el mensaje del usuario y devuelve el control de inmediato, lanzando la
  generación en un `tokio::spawn` en segundo plano (el comando Tauri nunca
  bloquea esperando la respuesta completa del LLM). Política de
  concurrencia elegida explícitamente (sección 88 del brief): **Reject**, no
  Queue ni Interrupt — un segundo `send_message` sobre una conversación con
  generación activa devuelve `ChatError::GenerationInProgress`. Cancelación
  vía `tokio_util::sync::CancellationToken` por generación en curso,
  corriendo en `tokio::select!` contra el stream de tokens. Título de
  conversación autogenerado del primer mensaje (`title.rs`, corta en límite
  de palabra, máx. 48 caracteres).
- `AppEvent` extendido con `AiResponseStarted/Token/Finished/Cancelled/
  Error` (`crates/eventbus`) — reutiliza el EventBus construido en Fase 0
  (hasta ahora solo probado con un heartbeat). `#[serde(tag = "type",
  rename_all = "snake_case", rename_all_fields = "camelCase")]`: el tag va
  en snake_case pero los campos de cada variante en camelCase, para que el
  frontend no necesite transformar nada.
- `SettingsService` extendido con la configuración de conexión LLM
  (`LlmConnectionSettings`/`LlmConnectionInput`) — decisión de alcance
  documentada: en vez de una tabla `providers` completa con CRUD
  multi-proveedor (que el brief prevé pero que no hace falta todavía con un
  solo canal activo), se persiste **una única conexión activa** como JSON en
  `app_settings` (reutilizando `get_json`/`set_json`), con solo la API key
  enrutada a través de `SecretStore`. La tabla `providers` completa queda
  para cuando de verdad haga falta más de un proveedor simultáneo.
- Comandos Tauri nuevos: `chat_send_message`, `chat_cancel_generation`,
  `chat_list_conversations`, `chat_list_messages`, `chat_create_conversation`,
  `settings_get_llm_connection`, `settings_set_llm_connection`,
  `settings_test_llm_connection`.

**Frontend**
- Design system: `Field` (extraído de un componente local duplicado que
  vivía dentro de `IdentityEditor`), `Badge`.
- `chatStore` (zustand) — el store más complejo hasta ahora:
  `requestIndex: Record<requestId, {conversationId, messageId}>` para poder
  enrutar los eventos de streaming (que solo traen `requestId`) al mensaje y
  conversación correctos; `handleAppEvent` es un reducer puro que procesa
  los 5 eventos `ai_response_*` (placeholder de streaming al recibir
  `started`, acumulación de tokens, cierre a `complete/error/cancelled`).
- Reescritura completa de la feature Chat (antes placeholder de Fase 0):
  `ConversationList`, `ChatWindow` (composer con Enter=enviar/
  Shift+Enter=nueva línea, botón Detener durante generación), `MessageBubble`.
- `AiProvidersSettings` — nueva sección de Configuración para apuntar a LM
  Studio/servidor OpenAI-compatible (URL, modelo, API key opcional,
  temperatura, max tokens), con test de conexión real y aviso de que hay
  una API key ya guardada sin mostrarla nunca en claro.
- `App.tsx`: el listener central de `useAppEvent` reenvía cualquier evento
  `ai_response_*` a `chatStore.handleAppEvent`, sin acoplar el bridge de
  eventos a la lógica de streaming.
- i18n (es/en): secciones `chat.*` y `settings.aiProviders.*` completas; se
  retiraron las claves de placeholder de Chat (`placeholder.chat.title/
  description`) porque Chat ya no es un placeholder — solo queda
  `placeholder.chat.noProfileTitle/Description` para el caso "sin perfil".
- `ErrorBoundary` (`src/app/ErrorBoundary.tsx`) — último recurso alrededor
  de `<App />`: sin él, un error de render no capturado desmonta todo el
  árbol de React en silencio (la ventana queda en negro, sin ninguna pista).
  Ahora muestra el mensaje, stack y component stack directamente en la
  ventana. Añadido durante la verificación manual de esta fase — ver
  "Problemas encontrados y corregidos".
- `ProfileAvatarBadge` (antes vivía solo dentro de `ProfileSwitcher.tsx`)
  se extrajo a `src/design-system/ProfileAvatarBadge.tsx` para reutilizarla
  también en la cabecera de `ChatWindow`.

### Tests
- Rust: suite completa del workspace en verde tras esta fase — incluye
  parsing SSE (`aivt-provider-llm`), el proveedor OpenAI-compatible contra
  un servidor mock HTTP, y `aivt-service-chat` (9 tests: envío,
  persistencia, cancelación con `tokio::select!` real, rechazo de
  generación concurrente). **80/80 en verde.**
- Frontend: 7 tests nuevos — 6 de `chatStore.handleAppEvent`
  (start/token/finished/error/cancelled/evento con requestId desconocido
  ignorado sin lanzar) + 1 de `ConversationList` que monta el componente
  real con Testing Library. **Total frontend: 27/27 en verde.**

### Verificación realizada en este entorno
- `cargo test --workspace` — limpio, **80/80**.
- `npx tsc -b` — sin salida (build limpio).
- `npx vitest run` — **27/27 en verde** (8 archivos).
- `npm run tauri dev` — **probado en vivo por el usuario**: creación de
  conversación, envío de mensaje, avatar en la cabecera del chat. Dos bugs
  reales encontrados y corregidos en el camino (ver abajo).

### Decisiones de arquitectura relevantes
- Streaming de tokens al frontend reutiliza el EventBus de Fase 0 en vez de
  inventar un mecanismo nuevo — confirma que el diseño original (propuesta
  técnica, sección de EventBus) ya anticipaba correctamente esta necesidad.
- El comando `chat_send_message` construye un `OpenAiCompatibleProvider` +
  `AIOrchestrator` nuevos en cada llamada, a partir de la configuración
  actual — no hay cacheo de conexión todavía. Aceptable mientras solo existe
  un proveedor activo a la vez; se revisará si el coste de reconectar por
  mensaje resulta perceptible en uso real.
- Mapeo de errores de cada crate a `AppError` sigue haciéndose explícito en
  la capa de comandos (nunca `From` entre crates), igual que en fases
  anteriores.

### Problemas encontrados y corregidos
- **Bucle infinito de render en `ConversationList`/`ChatWindow`** (bug real,
  encontrado por el usuario probando la app — la ventana se quedaba
  totalmente en negro sin ningún indicio del motivo). Causa: dos selectores
  de Zustand con la forma `useChatStore((s) => s.algoPorId[id] ?? [])` —
  cuando `id` todavía no tiene entrada en el store, ese `?? []` crea un
  array **nuevo** en cada render; Zustand compara por referencia, ve un
  valor "distinto" cada vez y vuelve a renderizar, que vuelve a crear otro
  array nuevo — bucle infinito hasta que React lo aborta con "Maximum
  update depth exceeded". Como no había ningún `ErrorBoundary`, ese error
  no capturado desmontaba silenciosamente toda la interfaz, dejando la
  ventana en negro sin pista alguna — por eso el primer paso del arreglo
  fue añadir el `ErrorBoundary` (ver arriba), que reveló el error real de
  inmediato. Corregido reemplazando el `?? []` inline por una constante
  módulo-level estable (`EMPTY_CONVERSATIONS` / `EMPTY_MESSAGES`) en ambos
  archivos. Se añadió un test de regresión
  (`ConversationList.test.tsx`) que monta el componente real con una
  entrada inexistente en el store — confirmado que falla contra el código
  sin arreglar y pasa con el arreglo. Lección para fases futuras: cualquier
  selector de Zustand que necesite un valor por defecto para una colección
  debe usar una constante compartida, nunca un literal `[]`/`{}` inline.
- **Avatar no aparecía en la cabecera del chat**: `ChatWindow` solo dibujaba
  un círculo con la inicial del nombre, igual que el bug de `ProfileSwitcher`
  ya corregido en Fase 1, pero sin usar `useProfileAvatar`. Corregido
  extrayendo `ProfileAvatarBadge` (antes privado a `ProfileSwitcher.tsx`) a
  `src/design-system/` y reutilizándolo en la cabecera de `ChatWindow`.

### Problemas conocidos
- Sin caché de conexión/cliente HTTP entre mensajes (ver arriba).
- No hay reintentos automáticos ante error de red transitorio — un error de
  proveedor termina la generación (`AiResponseError`) y el usuario debe
  reenviar. Aceptable para esta fase; reintentos con backoff quedan como
  mejora posible, no bloqueante.
- El título de conversación se genera una sola vez, del primer mensaje —
  no se recalcula si el usuario edita o borra ese primer mensaje (no hay
  edición/borrado de mensajes todavía, así que hoy no es alcanzable).
- El primer arranque de `npm run tauri dev` tarda unos segundos en pintar
  contenido (WebView2 inicializando su perfil + build sin optimizar) —
  esperado en modo desarrollo, no reproducido como un problema en sí; una
  build de release (`tauri build`) arranca sensiblemente más rápido.
- Envío de imágenes y mensajes de voz en el chat: **todavía no
  implementado a propósito** — es el contenido de Fase 3 (Multimodal), no
  de esta fase.

## Fase 1 — Profiles / Identity

### Completado

**Backend**
- Migración `0003_profiles.sql`: tablas `profiles`, `profile_identity`,
  `profile_personality` (19 sliders — sección 11 del brief).
- `ProfilesRepository` (`crates/database`) — CRUD completo + `duplicate`,
  todo en transacciones donde toca (create/update tocan 3 tablas a la vez).
- `aivt-service-profiles` (nuevo crate) — `ProfileService`: CRUD, avatar
  (guardado en `<data_dir>/avatars/<id>/`, nunca base64 en SQLite —
  sección 75), export/import `.aivtprofile` (JSON; nunca incluye secretos,
  memorias ni voz/avatar porque esas features no existen todavía).
- `aivt-orchestrator` (contenido real por primera vez) —
  `PersonalityPromptBuilder` (19 sliders → 6 párrafos de instrucción en
  lenguaje natural, agrupados por tema en vez de un bullet por trait) y
  `PromptComposer` (identidad + lore + relación + personalidad → system
  prompt). Ver docs/architecture.md §5 y ADR-003.
- 10 comandos Tauri nuevos (`profiles_list/create/update/duplicate/delete/
  set_avatar/get_avatar/export_to_path/import_from_path/preview_prompt`).
- Plugin `tauri-plugin-dialog` (solo para elegir ruta de export/import — la
  lectura/escritura real la hace Rust directo con `tokio::fs`, no el plugin
  `fs`, para no necesitar configurar su scope de permisos).
- Todos los tipos que cruzan IPC (`Profile`, `ProfileIdentity`,
  `ProfilePersonality`, `NewProfile`, `ProfileUpdate`, `ProfileExport`) usan
  `#[serde(rename_all = "camelCase")]` para que el JSON en el wire ya sea
  camelCase — el frontend no necesita una capa de transformación.

**Frontend**
- Design system: `Input`, `Textarea`, `Slider`, `DropdownMenu`, `Dialog` +
  `ConfirmDialog`, `LoadingState`.
- `ProfileSwitcher` (en el header del sidebar) — selector global de
  personaje activo (sección 105 del brief), con avatar real por perfil
  (`ProfileAvatarBadge`, fetch propio por ítem), crear e importar.
- Pestaña Identidad completa: Información, Lore y relación, Personalidad
  (19 sliders + descripción libre), y una pestaña "Vista previa del prompt"
  que llama a `profiles_preview_prompt` sobre el *borrador* (sin guardar)
  con debounce de 300ms — feedback inmediato de cómo los sliders se
  traducen a instrucciones reales.
- Acciones: Duplicar, Exportar (diálogo nativo de guardado), Eliminar
  (con `ConfirmDialog`, no `window.confirm`).
- `activeProfileId` persistido en `localStorage` (sección 105: la selección
  de personaje sobrevive a reinicios) vía el middleware `persist` de
  zustand — el resto del estado del store siempre se recarga del backend.
- Chat ahora reacciona a si existe algún perfil: sin perfiles, apunta a
  crear uno en Identidad; con perfiles, deja claro que el chat en sí es la
  siguiente pieza por construir (Fase 2) — ya no dice nada de "Fase 1".

### Tests
- Rust: 15 tests nuevos (`aivt-database::profiles` 7, `aivt-orchestrator`
  10 — ver desglose abajo, `aivt-service-profiles` 6). **Total del
  workspace: 46/46 en verde.**
- Frontend: 6 tests nuevos (`profileStore`: load/create/update/remove con
  IPC mockeada). **Total: 20/20 en verde.**

### Verificación realizada en este entorno
- `cargo check --workspace` y `cargo test --workspace` — limpio, **46/46
  tests en verde**, 0 warnings.
- `npx tsc -b`, `npx vitest run` (**20/20**), `npx vite build` — todos
  limpios.
- `npm run tauri dev` — app real, probada a mano por el usuario: crear
  personaje, editar los 4 sub-tabs de Identidad, subir avatar, guardar,
  duplicar, exportar. Dos bugs de UI reales encontrados y corregidos en el
  camino (ver "Problemas encontrados y corregidos").

### Problemas encontrados y corregidos
- **Bind order equivocado en el INSERT de `profile_personality`**: la
  columna `profile_id` estaba primero en la lista de columnas del SQL pero
  el binder compartido con `update` (que sí la pone al final, por ser un
  `WHERE`) la bindeaba último — el resultado era que `profile_id` recibía
  el valor de `free_description` y viceversa, causando
  `FOREIGN KEY constraint failed` en *todo* `create()`. Detectado por los
  tests (`cargo test`), no en la app — exactamente el tipo de bug que la
  suite existe para atrapar. Corregido reordenando las columnas del INSERT
  para que coincidan con el orden de bind ya usado por `update`.
- **Avatar no aparecía en el `ProfileSwitcher`**: encontrado por el usuario
  probando la app. El switcher solo mostraba un círculo con la inicial —
  nunca llamaba a `profiles_get_avatar`. Corregido con un hook compartido
  `useProfileAvatar` (usado también por `IdentityEditor`, eliminando
  duplicación) y un componente `ProfileAvatarBadge` que lo usa tanto en el
  trigger como en cada ítem de la lista desplegable (el primer arreglo solo
  cubrió el trigger; el usuario encontró que la lista seguía sin avatares,
  segundo arreglo).

### Problemas conocidos
- El export `.aivtprofile` no incluye la imagen de avatar (solo texto/
  sliders) — consistente con la sección 109 del brief, que no la exige
  todavía; se puede añadir como campo opcional más adelante sin romper el
  formato.
- La vista previa del prompt no incluye memorias, contexto de conversación
  ni contexto de canal (Twitch/Discord) — llegan con sus fases respectivas;
  hoy solo compone la parte "de personaje", tal como está documentado en
  `PromptComposer`.
- `ProfileService::delete` borra el directorio de avatar en "best effort"
  (si falla, no revierte el borrado del perfil) — aceptable porque perder
  unos KB de imagen huérfana no es un problema de correctness.

## Fase 0 — Foundation

### Completado

**Scaffolding**
- Cargo workspace con crates por módulo de dominio (`src-tauri` +
  `crates/`), reflejando docs/architecture.md §4.
- Frontend Vite + React 18 + TypeScript strict (`tsc -b` con project
  references: `tsconfig.app.json` + `tsconfig.node.json`).
- `.claude/launch.json` para previsualizar el frontend solo (sin backend
  Tauri) durante desarrollo de UI.

**Design system y shell**
- Componentes: `Button`, `IconButton`, `Card`, `Tooltip`, `Switch`,
  `Select`, `Tabs`, `EmptyState`, `Toast`/`Toaster` — construidos sobre
  primitivas Radix UI (accesibilidad gratis) + Tailwind.
- `ThemeEngine`: variables CSS en `src/globals.css` (dark mode + 15
  presets de tema completo elegibles por perfil desde Identidad, ver
  ADR-008/ADR-009) — ya no es solo acento violeta fijo.
- Sidebar con las 10 secciones del brief, navegación vía `navigationStore`
  (zustand).
- Cada pestaña sin backend renderiza un `PlaceholderPage` honesto
  ("Disponible próximamente" + explicación) — sección 118 del brief:
  ningún control aparenta funcionar sin hacerlo.

**Base de datos**
- `sqlx::SqlitePool` en WAL mode + `sqlx::migrate!`.
- Migraciones: `0001_foundation` (`app_settings`), `0002_activity`
  (`activity_events`).

**Logging + Activity**
- `tracing` + `tracing-appender` (archivo rotado diario en
  `<app_data>/logs/`).
- `ActivityLayer` (tracing_subscriber `Layer` custom, filtra por
  `target: "activity"`) → canal async → `ActivityService` → tabla
  `activity_events`.
- `aivt_core::redact_secrets` — redacción de API keys/bearer tokens/tokens
  de bot vía regex. Ver docs/security.md.

**Error handling**
- `AppError` (`crates/core`) — `{ module, code, message }`, mapeado
  explícitamente en la capa de comandos (sin `From` entre crates).

**EventBus**
- `AppEvent` (`ServiceStatusChanged`, `Heartbeat`) sobre
  `tokio::sync::broadcast`.
- Bridge `EventBus → app_handle.emit("app://event", ...)`, con manejo
  explícito de `RecvError::Lagged` (continúa) vs `Closed` (termina).
- Hook `useAppEvent` en el frontend, con captura de la promesa de
  `listen()` para no dejar rechazos sin manejar.

**SecretStore**
- Trait + `KeyringSecretStore` (crate `keyring`, backends nativos por SO).
  Sin ningún provider real consumiéndolo todavía.

**ServiceManager**
- `ManagedService` trait + `ServiceManager` (`Stopped/Starting/Running/
  Error/Stopping`, publica `AppEvent::ServiceStatusChanged`). No wired en
  `src-tauri` todavía — no hay servicios reales que registrar hasta
  Fase 2/8/9/10.

**Configuración > General**
- Idioma (es/en, i18next) persistido en `app_settings`.
- Iniciar con sistema vía `tauri-plugin-autostart` (fuente de verdad = el
  registro del SO, no duplicado en `app_settings`).

**i18n**
- `i18next` + `react-i18next`, namespace `common`.

### Verificación realizada en este entorno
- Rust/Cargo no estaban instalados; se instalaron con permiso del usuario
  (`rustup` + Visual Studio 2022 Build Tools vía `winget`).
- `cargo check --workspace` / `cargo test --workspace` — limpio, 21/21
  tests en verde.
- `npm install`, `npx tsc -b`, `npx vitest run` (14/14), `npx vite build` —
  todos limpios.
- `npm run tauri dev` — la app arrancó correctamente, confirmado
  visualmente por el usuario.

### Problemas encontrados y corregidos
- `vitest` requería alinearse a v3 para compartir la misma instancia de
  Vite que `@vitejs/plugin-react` (v2 solo soporta Vite 5 como peer, y el
  proyecto usa Vite 6).
- `useAppEvent` dejaba sin capturar el rechazo de la promesa de `listen()`
  cuando corre fuera de un WebView de Tauri real.
- `ServiceManagerError::{StartFailed,StopFailed}` tenían un campo llamado
  `source: String` — `thiserror` trata cualquier campo llamado `source`
  como la fuente de error y exige que implemente `std::error::Error`.
  Renombrado a `reason`.
- `crates/database/src/lib.rs` no reexportaba los tipos de
  `repositories::*` en la raíz del crate.
- `tauri::generate_handler!` necesita la ruta exacta donde se define cada
  `#[tauri::command]`, no un re-export.

### Problemas conocidos
- El icono de la app es un placeholder generado programáticamente (fondo
  violeta + "A") — reemplazar con `npm run tauri icon <ruta-a-un-png>`
  cuando exista arte real. La app se renombró a "Project Ciel" durante
  esta sesión larga; el usuario sigue diseñando un ícono nuevo acorde
  (2026-09-16) — hasta entonces el placeholder queda como está.
- `tauri-plugin-autostart`, `tauri-plugin-single-instance` y
  `tauri-plugin-dialog` están como dependencias incondicionales en
  `src-tauri/Cargo.toml`. Cuando arranque la Fase 12 (investigación mobile)
  hay que moverlos a
  `[target.'cfg(not(any(target_os = "android", target_os = "ios")))'.dependencies]`.

## Cómo probarlo manualmente

```bash
npm install
cargo test --workspace   # backend — 172 tests
npm run test             # frontend — 75 tests
npm run tauri dev        # app completa
```

En la app: crea un personaje desde el selector del sidebar (o usa uno ya
creado en Fase 1), completa su Identidad. Luego ve a Configuración > AI
Providers y apunta a un servidor LM Studio (o cualquier servidor
OpenAI-compatible) corriendo localmente — URL, modelo, prueba la conexión.
Con eso configurado, ve a Chat, crea una conversación y escribe: la
respuesta debe llegar en streaming token a token, con un botón "Detener"
mientras genera. Para probar imágenes: activa "Visión (imágenes)" en
Configuración > AI Providers (solo si el modelo cargado de verdad las
acepta), vuelve a Chat, adjunta una imagen desde el clip del composer y
envíala — debe aparecer como miniatura en tu mensaje y el modelo debe
poder describirla. Para probar memoria: chatea contando algo sobre ti
(p. ej. "me encanta el café"), y desde el menú de esa conversación
("⋯" en la lista de conversaciones) elige "Extraer memorias" para verla
aparecer de inmediato en la pestaña Memoria — sin ese botón, la extracción
también ocurre sola cada 6 mensajes nuevos, y la pestaña Memoria se
actualiza sola aunque no estés mirándola en ese momento. En la pestaña
Memoria puedes buscar, fijar, editar (lápiz), eliminar o añadir una
memoria a mano — cada una con título, resumen (lo que el personaje
recuerda en cada conversación) y detalle opcional (disponible tras pulsar
"Ver detalle" en la tarjeta). Envía otro mensaje que toque el mismo tema y
deberías ver "Recuerdos relevantes..." reflejado en cómo responde el
personaje. Para probar voz: ve a la pestaña Voz del personaje activo,
activa "Leer respuestas en voz alta", elige una voz del sistema (o escribe
un id si cambiaste el motor a HTTP en Configuración > TTS) y pulsa "Probar
voz" — debería escucharse de inmediato con la velocidad/tono/volumen que
elijas. Envía un mensaje en Chat y la respuesta debería reproducirse sola
al terminar de generarse; cada respuesta también tiene su propio botón de
altavoz para repetirla. Para probar la Llamada: ve a la pestaña Llamada
del personaje activo, elige el micrófono (o deja el predeterminado del
sistema), el modo de respuesta ("Siempre que detecte que hablas" o
"Solo cuando lo llames" con una frase propia) y pulsa "Empezar llamada"
— vas a ver dos indicadores separados (tú/personaje) reflejando en vivo
si te está escuchando, transcribiendo, pensando o hablando, además de
la transcripción de lo dicho en el panel de la derecha. Con voz
activada en el personaje, debería responder por audio igual que en
Chat; hablar mientras está respondiendo debería cortarlo (barge-in) si
el modo de respuesta es "a todo", o esperar a que la nueva frase lo
mencione si el modo es por frase de activación. El botón de silenciar
micrófono corta la escucha sin cortar la llamada. Para probar el
avatar: en la pestaña Avatar del personaje activo, sube una imagen para
cada estado (reposo/pensando/hablando) y activa el switch "Activar fuente
OBS" — aparece una URL (`http://127.0.0.1:47823/overlay?profileId=...`,
salvo que hayas cambiado el puerto en el ajuste guardado). Pégala como
fuente "Navegador" (Browser Source) en OBS, o simplemente ábrela en una
pestaña de cualquier navegador normal para confirmar que funciona sin
necesitar OBS instalado. Apenas cargue la página deberías ver la imagen de
reposo; mientras esa pestaña siga abierta, chatea o usa "Probar voz" desde
la app: el avatar debería cambiar solo entre reposo/pensando/hablando
según lo que esté pasando. A diferencia del diseño anterior, no hay que
"mostrar"/"ocultar" nada — la fuente vive en esa URL mientras el switch
esté activado, así que se puede cerrar y reabrir la pestaña (o reiniciar
OBS) sin perder nada. Para probar Twitch: crea una
aplicación en dev.twitch.tv/console/apps (tipo "Público" — no hace falta
ninguna Redirect URL para Device Code Grant), copia el Client ID, pégalo
en la pestaña Twitch y guárdalo. Pulsa "Conectar cuenta" — se abre tu
navegador en una página de Twitch con un código ya prellenado, solo hace
falta aprobar; al volver a la app debería mostrar "Conectado como
<tu_canal>". Configura la política (activar respuestas, modo mencionado/
todos, cooldown) y pulsa "Empezar a escuchar". Para ver y oír una
respuesta sin depender de otra cuenta, usa la tarjeta "Probar respuesta":
escribe cualquier texto y pulsa "Probar" — la respuesta (y su voz, si el
personaje la tiene activada) llega igual que una real, sin publicarse en
Twitch. Para probar el flujo real de punta a punta hace falta que **otra**
cuenta escriba en tu chat (la propia cuenta conectada se ignora a
propósito — ver "Decisiones de alcance" de esta fase): con "Solo cuando lo
mencionan" activo, que mencione el nombre o apodo del personaje; con "A
todos los mensajes" activo, cualquier mensaje suyo califica. Debería
aparecer en el feed en vivo de la pestaña y el personaje debería
responder ahí mismo. Para probar Discord, ver la sección "Fase 9 —
Discord (texto)" más arriba.

**Proveedores de IA con cambio automático**: en Configuración > Proveedores IA,
agrega dos proveedores, actívalos y fíjalos, y deja uno "en uso". Apaga el
servidor del que está en uso (o usa uno con el límite agotado) y envía un
mensaje en el Chat: debe responder el otro y la insignia "En uso" debe pasar
sola. Una key mala no debe cambiar de proveedor, solo mostrar el error. Si
necesitas simularlo sin gastar créditos, apunta dos proveedores "Servidor local"
a servidores falsos locales (uno que responda 429 o no responda nunca).

**Dictado por voz**: descarga un modelo en Configuración > Transcripción de voz;
en el Chat, mantén pulsado el micrófono y habla (se ve el texto en vivo), suelta
para transcribir, o desliza hacia arriba para fijar y usa el botón rojo de stop.
Prueba también el interruptor de envío automático.

**Motor de voz**: en Configuración > Motor de voz agrega el servicio, pulsa
"Probar conexión" (sintetiza y reproduce una frase) y elige la voz en la pestaña
Voz. Con Fish Audio también hay buscador por nombre.

Solo frontend, sin backend (más rápido para iterar UI, `invoke()`/eventos
fallan de forma controlada):

```bash
npm run dev
```

## Próxima fase

Fase 8 — Twitch, Fase 9 — Discord (texto), Fase 6 — Llamada IA y Fase 7 —
Avatar quedaron completas y verificadas en vivo (ver la nota posterior al
inicio de este documento). STT pasó de HTTP/Docker a whisper.cpp embebido.
Sigue abierto:
- **Fase 10 — Discord voz**: intentada y abandonada — ver la sección
  "Fase 10 — Discord con voz — abandonada" más arriba para el motivo
  (bug abierto en `songbird` combinado con el cifrado DAVE que Discord
  exige desde 2026).
- **Fase 12 — investigación mobile**: pospuesta explícitamente por el
  usuario (2026-09-16) hasta tener la app ya exportada/empaquetada para
  distribuirla — no antes. Cuando arranque, ver "Problemas conocidos"
  más abajo para la tarea técnica ya anotada (mover
  `tauri-plugin-autostart`/`tauri-plugin-single-instance`/
  `tauri-plugin-dialog` a dependencias condicionales).
- **Avatar 2D vs 3D (Live2D/VRM)**: pospuesta explícitamente por el
  usuario (2026-09-16) hasta tener la app "bien finalizada". Hasta
  entonces el avatar sigue siendo "reactive PNG" (Fase 7) — ver sus
  "Decisiones de alcance".
- **Pruebas con claves reales** (pendiente): cambio automático con Anthropic y
  Gemini; Together, xAI, SambaNova, Mistral, DeepSeek, Deepgram y la voz de
  Groq/Together. Ver "Pendiente / sin verificar" en la sección de proveedores
  multi-servicio.
- **Instalador**: personalizar imágenes, idiomas y plantilla NSIS, y el ícono
  propio de `.aivtprofile` — aplazado por el usuario (2026-10-02).
- **MCP (Model Context Protocol) como cliente de herramientas**:
  evaluado el 2026-09-16 (inspirado en features de la app "OpChat") y
  descartado explícitamente por el usuario — no construir, no volver a
  proponerlo salvo que el usuario lo pida de nuevo.

## Rendimiento de imágenes (2026-10-02)

Los avatares y adjuntos se pedían por IPC en base64 una vez por componente (un avatar de 1,6 MB repetido en cada burbuja del chat), lo que cargaba la CPU. Ahora hay una caché compartida (`src/lib/imageCache.ts`, `useCachedImage`) con un `blob:` URL por imagen, y las subidas se reducen a 512 px (perfil/usuario) o 1024 px (estados del avatar). Confirmado en vivo por el usuario.

## Ajustes de interfaz y filtro de teclado (2026-10-06)

- **Diálogos que no se adaptaban a la ventana**: `DialogContent` (`src/design-system/Dialog.tsx`) no tenía altura máxima, así que en ventanas bajas el diálogo de ajustes de Llamada se cortaba por arriba y por abajo. Ahora es una columna flex con `max-h-[calc(100vh-2rem)]` (afecta a todos los diálogos). En los ajustes de Llamada y de Discord el cuerpo se desplaza (`min-h-0 flex-1 overflow-y-auto`, con `-mx-1 px-1` para no recortar el anillo de foco) y el título y el botón Cerrar quedan fijos. Confirmado en vivo por el usuario.
- **Campos de texto que crecen solos**: `Textarea` gana la opción `autoGrow` (altura ajustada al contenido, nunca menor que `rows`, sin asa de arrastre, se reajusta si cambia el ancho). Aplicada a todos los campos de Identidad (descripción, historia, relación, rol del usuario, estilo de habla, instrucciones especiales, descripción libre de personalidad), a Resumen y Contenido de Memoria y a la descripción de crear personaje. Confirmado en vivo.
- **Importar personaje en Identidad**: el botón se movió del selector de perfil de la barra lateral al encabezado de Identidad (junto a Duplicar, Exportar y Eliminar) y también aparece en la pantalla vacía de Identidad. La lógica vive en `src/features/identity/useImportProfile.ts`; etiqueta `identity.actions.import` en los 6 idiomas. El doble clic sobre un `.aivtprofile` usa otro camino (`App.tsx`) y no cambió. Confirmado en vivo.
- **Filtro básico de golpes de teclado en la Llamada — QUITADO el mismo día** (`crates/services/call/src/segmenter.rs`; el segmentador volvió a su comportamiento anterior, `cargo test -p aivt-service-call` 32 pasan). Lo que hacía: antes del VAD, si la energía de un trozo de 30 ms se concentra en una ventana de 5 ms (≥ 60 %, y energía media mínima 2 500), se descarta como golpe (tecla/clic) y se ignoran 2 tramos más (~60 ms). Solo actúa cuando no se está grabando una frase, para no cortarla. Prueba unitaria sintética (`a_key_click_is_impulsive_but_sustained_tone_is_not`); `cargo test -p aivt-service-call` 33 pasan. **Resultado real: el tecleo ("toc toc") sigue reconociéndose como voz en pruebas con micrófono.** El usuario decidió dejarlo así (2026-10-06): lo importante es que se reconozcan bien los nombres, y eso lo cubre el registro de nombres. Opciones si se retoma: subir la sensibilidad del VAD a "Muy alta", o una supresión de ruido real (crate nativo más pesado).

### Icono de archivos y doble clic en copias de seguridad (2026-10-06)

- **Icono propio de `.aivtprofile` y `.aivtbackup`**: el usuario envió un moño con icono de persona (`src-tauri/branding/file-icon-source.png`), convertido a `src-tauri/icons/file-icon.ico` (16–256 px). Tauri v2 no tiene un campo de icono por asociación de archivo (`FileAssociation` solo trae `ext`, `name`, `description`, `role`, `mimeType`…), así que `src-tauri/installer-hooks.nsh` (`NSIS_HOOK_POSTINSTALL`, enlazado en `bundle.windows.nsis.installerHooks`) escribe a mano `DefaultIcon` de las clases `ProjectCielProfile` y `ProjectCielBackup` y avisa al Explorador (`SHChangeNotify`). El `.ico` se instala junto a la app vía `bundle.resources`. Se eliminó el icono provisional `profile-icon.ico`. **Pendiente de verlo en el Explorador con el instalador real** (no se activa en `tauri dev`).
- **Doble clic en un `.aivtbackup`** (app cerrada o ya abierta): `find_profile_file_arg` (`src-tauri/src/lib.rs`) acepta ambas extensiones y reutiliza el camino ya existente (`pending_import_path` / `AppEvent::ProfileImportRequested`, cuyo nombre quedó histórico). `App.tsx` decide por la extensión: `.aivtprofile` → confirmación de importar; `.aivtbackup` → el flujo de restauración en dos pasos de Configuración > Copia de seguridad (valida y prepara, muestra fecha/archivos/tamaño, y solo al confirmar reinicia y restaura; si cancela, `cancelRestore`). Ese flujo se extrajo a `src/features/settings/useRestoreFlow.tsx`, compartido por `BackupSettings` y `App`. `tsc` y 338 tests pasan; pendiente de probar con el instalador.
- **Verificado con el instalador real (2026-10-06)**: los iconos de ambos tipos de archivo se ven en el Explorador y el doble clic en `.aivtprofile` y `.aivtbackup`, con la app abierta y cerrada, muestra su diálogo de confirmación.
- **Bug encontrado y corregido: "Restaurar y reiniciar" no reabría la app.** `backup_restart_app` usaba `app.restart()`, que lanza el proceso nuevo antes de que el actual termine; `tauri-plugin-single-instance` lo trata como segunda instancia y lo cierra. Ahora (`src-tauri/src/commands/backup.rs`, solo Windows) se lanza un `cmd` independiente que espera ~3 s (`ping -n 4`) y abre el ejecutable sin argumentos (así tampoco se repite la confirmación si la restauración vino de un doble clic) y la app se cierra con `app.exit(0)`. Pendiente de confirmar con un instalador nuevo. En `tauri dev` la reapertura no funciona (falta el servidor de la interfaz).
- **Segunda corrección del reinicio (2026-10-06)**: el primer arreglo mostraba una ventana de consola con la cuenta atrás y luego el error "Windows no encuentra el archivo \\". Causas: `DETACHED_PROCESS` + `CREATE_NO_WINDOW` juntos (Windows ignora el segundo y abre consola; ahora solo `CREATE_NO_WINDOW`) y `current_exe()` devuelve una ruta con prefijo `\?\` que `cmd` no entiende (ahora se quita antes de lanzar). La línea de comandos de reapertura se probó con el Bloc de notas; pendiente de confirmar con la app real en un instalador nuevo.
- **Tercera corrección del reinicio (2026-10-06)**: tras quitar la consola, seguía el error "Windows no encuentra el archivo". Causa real, reproducida con un programa de prueba: `Command::args` escapa las comillas como `\"` y `cmd /C` no lo entiende (`start \"\"` buscaba un archivo llamado `\`). Ahora se usa `CommandExt::raw_arg` con la línea de `cmd` tal cual. Verificado con el Bloc de notas lanzado desde Rust (con `args` no abre, con `raw_arg` sí); pendiente de confirmar con la app real.
- **Reinicio tras restaurar: confirmado en vivo (2026-10-06)** con el instalador real — la app se cierra y se vuelve a abrir sola.
- **Flash blanco al arrancar (2026-10-06)**: la ventana principal ahora tiene `backgroundColor: "#0d0d11"` (`tauri.conf.json`) y `index.html` fija el mismo fondo oscuro con un `<style>` en línea y `<meta name="color-scheme" content="dark">`, el valor de `--color-background` de `src/globals.css`. Es siempre oscuro, también en modo Claro. Pendiente de confirmar con el instalador.

### Pantalla de Créditos rediseñada (2026-10-06)

`src/features/settings/CreditsSettings.tsx` se rehízo como una sola tarjeta con tres secciones separadas por `border-t`: (1) cabecera con el icono de la app (`/app-icon.png`) centrado con brillo del acento, nombre, versión (`getVersion`), descripción, "Creado con" (Tauri 2, Rust, React, TypeScript) y "Desarrollado con la ayuda de Claude, de Anthropic"; (2) autor Vulito con los enlaces `vulito.carrd.co` (principal, destacado), `@VulitoVT` y `@VulitoVRC` (X), `@vulitovt` (Instagram) y `VuloB1` (GitHub), todos con `openExternalUrl`; (3) agradecimientos a Shumdrador (`x.com/Shum_vrc`) con la leyenda de que verificó la traducción al coreano. Textos nuevos en `settings.credits.*` en los 6 idiomas (se quitaron `appInfoTitle` y `visitLink`). Revisado visualmente en el navegador con una página temporal (ya borrada); falta verlo dentro de la app. Ko-fi: `https://ko-fi.com/vulito` (constante `KOFI_URL`; si se vacía, el bloque de apoyo se oculta).
- Créditos: el nombre correcto es **Shumdrador** (se corrigió) y su foto de perfil es `public/shumdrador.webp` (96×96 px, WebP calidad 80, ~3 KB; se muestra a 40 px, con margen para pantallas de alta densidad).

### Instalador personalizado (2026-10-07)

`bundle.windows.nsis` (`src-tauri/tauri.conf.json`): icono del instalador `icons/icon.ico` (el moño), imagen lateral `installer/sidebar.bmp` (164×314, la ilustración enviada por el usuario, recortada por abajo), cabecera `installer/header.bmp` (150×57, moño + "Project Ciel" sobre el fondo oscuro de la app), 6 idiomas (`Spanish`, `English`, `PortugueseBR`, `Japanese`, `Korean`, `SimpChinese`) con selector, y el paso `installer-hooks.nsh` que pone icono a `.aivtprofile`/`.aivtbackup`. Las imágenes se generan a partir de los originales (los `.bmp` van versionados). Compila sin errores; pendiente de verlo instalado. Sin pantalla de licencia todavía (decisión del usuario: cobrará por el programa, hace falta un acuerdo de uso comercial). Créditos: foto del autor `public/vulito.webp` (160 px, ~5 KB, mostrada a 64 px).

Licencias (revisión de dependencias): 837 crates y 35 paquetes npm directos, sin GPL/AGPL; MIT/Apache dominan, MPL-2.0 (cssparser/selectors/symphonia…, conservar avisos) y fuentes OFL-1.1 (`@fontsource/*`). Obligatorio incluir los avisos de terceros. Plan comercial: licencia propietaria de pago; pendientes: archivo de licencias de terceros, acuerdo de uso en 6 idiomas, firma de código, derechos de las imágenes, búsqueda de marca "Project Ciel", plataforma de venta y modelo de licencia.
- Instalador, comprobado instalado (2026-10-07): las imágenes lateral y de cabecera se ven bien. El selector de idioma no apareció porque `HKCU\Software\projectciel\Project Ciel\Installer Language` ya valía `1033` por instalaciones anteriores solo en inglés (NSIS reutiliza el idioma guardado); en una primera instalación sí se muestra. Se mantiene el selector porque el español de Latinoamérica no coincide con el `Spanish` de NSIS (solo es-ES). El texto de la cabecera (`installer/header.bmp`) se centró midiendo la tinta de las líneas, no la caja de la fuente.


### Acuerdo de uso y licencias de terceros (2026-10-07)

El usuario venderá la app: licencia propietaria de pago, país Chile, firma como «Vulito» (sin nombre legal). Se agregó:
- `legal/EULA.{es,en,pt,ja,ko,zh}.txt`: acuerdo de uso comercial en 6 idiomas (licencia personal intransferible, uso con fines comerciales del contenido propio, sin reventa/redistribución/ingeniería inversa, datos locales, servicios de terceros, contenido de IA, sin garantía, actualizaciones de la misma versión mayor incluidas, reembolsos según la plataforma de venta, ley de Chile, contacto = `https://vulito.carrd.co/`). **Borrador no revisado por un abogado.** Supuestos que el usuario no ha confirmado: instalación en «tus propios equipos» (sin límite fijo de dispositivos) y actualizaciones de la misma versión mayor incluidas.
- `legal/THIRD_PARTY_LICENSES.txt`, generado por `scripts/generate-third-party-licenses.py` (688 paquetes Rust+npm, ~1 MB, textos idénticos agrupados, los paquetes sin archivo de licencia llevan autor y enlace al código, más los modelos que se descargan: Whisper MIT, MiniLM Apache-2.0). Se regenera antes de cada release.
- Pantalla de licencia del instalador: `bundle.licenseFile = installer/license.txt` (generado por `scripts/build-installer-license.py`: español + inglés en UTF-16 LE con BOM, porque Tauri solo admite un archivo).
- Los 7 documentos viajan como recursos (`bundle.resources` → `legal/`) y Configuración > Créditos tiene una sección «Legal» con dos botones (acuerdo de uso en el idioma activo y licencias de terceros) que los abren con el programa predeterminado (comando `open_legal_document`, con lista fija de documentos/idiomas).
- Pendiente para vender: firma de código, derechos de las imágenes, búsqueda de marca «Project Ciel», plataforma de venta, política de reembolsos, mecanismo de clave de licencia y revisión legal del acuerdo.
- Decisiones del usuario (2026-10-07): sin límite fijo de dispositivos y actualizaciones incluidas (queda como está en el acuerdo); **sin certificado de firma de código por ahora** (falta de presupuesto: SmartScreen advertirá al instalar); las imágenes son en su mayoría generadas con IA desde cuentas propias (conviene revisar los términos de uso comercial de cada herramienta); sin coincidencias exactas de «Project Ciel» en su búsqueda informal (falta consulta formal, p. ej. INAPI); **primera plataforma de venta: itch.io**.

- Ajustes (2026-10-07): la pestaña «Créditos» pasó a llamarse **«Acerca de»** (en: About, pt: Sobre, ja: アプリについて, ko: 정보, zh: 关于) porque contiene información de la app, autor, agradecimientos, apoyo y documentos legales; la clave interna sigue siendo `credits`. Las licencias de terceros ahora existen **en los 6 idiomas** (`legal/THIRD_PARTY_LICENSES.{es,en,pt,ja,ko,zh}.txt`, mismos textos de licencia originales con encabezados y etiquetas traducidos) y el botón abre la del idioma activo; antes abría siempre el mismo archivo bilingüe es/en. El generador (`scripts/generate-third-party-licenses.py`) produce los 6 archivos. Los acuerdos de uso referencian la nueva ruta «Configuración > Acerca de» en cada idioma.
- Corrección (2026-10-07): la pantalla de licencia del instalador mostraba caracteres basura. Tauri antepone un BOM de UTF-8 al archivo de licencia al copiarlo, y yo lo había guardado en UTF-16: quedaba con dos BOM. Ahora `installer/license.txt` se genera en UTF-8 sin BOM (`scripts/build-installer-license.py`).
- Confirmado por el usuario con el instalador real (2026-10-07): la pantalla de licencia se ve bien (UTF-8 sin BOM), «Acerca de» y las licencias por idioma.

### Publicación en itch.io (2026-10-07)

Página: https://vulito.itch.io/projectciel (de pago, «En desarrollo», categoría Herramienta). Versión 0.1.0 subida con butler: canal `windows` (zip portable, build #2081169) y `windows-installer` (instalador, build #2081170). Textos y recomendaciones en `docs/itch/page-description.md`; portada `src-tauri/branding/itch-cover-630x500.{png,jpg}`. Para cada versión nueva: recompilar, regenerar `legal/` (`scripts/generate-third-party-licenses.py`, `scripts/build-installer-license.py`), rehacer el zip y subir con `butler push <archivo> vulito/projectciel:<canal> --userversion X.Y.Z`. Sin firma de código (SmartScreen avisará salvo desde la app de itch.io).
- Fondo de la página de itch.io (2026-10-07): `src-tauri/branding/itch-background*.jpg`; la versión `itch-background-seamless.jpg` (1920×1400) se repite sin costuras en ambos ejes (dibujada en 3×3 baldosas y recortada), para páginas más altas que la imagen.
