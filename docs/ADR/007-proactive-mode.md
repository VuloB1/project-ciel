# ADR-007: Modo proactivo — el personaje inicia conversaciones por su cuenta

## Estado

Aceptado e implementado.

## Contexto

Última pieza pendiente de una ronda de recomendaciones evaluadas junto
con el usuario (búsqueda semántica de memoria, moods, modo proactivo) —
las dos primeras ya en producción (ver ADR-005 y ADR-006). El pedido
explícito fue combinar los 4 disparadores discutidos —inactividad,
estado de ánimo, seguimiento de memorias y rutina (hora del día)— en vez
de elegir uno solo.

Hoy el personaje solo responde a mensajes del usuario: no existía ningún
camino, público o privado, para generar un mensaje de "asistente" sin un
mensaje de usuario previo (`ChatService::send_message` siempre inserta el
mensaje de usuario antes de disparar la generación). Modo proactivo es,
en esencia, el primer contenido que no viene de una acción del usuario.

## Decisión

- **Un único motor de decisión, no cuatro mecanismos separados.** Cada
  cierto intervalo, `ProactiveService` (nuevo crate `aivt-service-proactive`)
  le hace **una sola llamada al LLM** con todo el contexto disponible:
  horas de inactividad, mood actual, fecha/hora, memorias recientes o
  fijadas, y la tendencia a iniciar charla ya existente en la
  personalidad (`ProfilePersonality.initiate_conversation_tendency`,
  0-100). El modelo decide si corresponde hablar y, si es así, redacta el
  mensaje en el momento (`{"should_initiate": bool, "message": string}`).
  Esto evita: (a) agregar un campo de "fecha de evento" nuevo al schema de
  `Memory` — el modelo ya razona sobre fechas mencionadas en el texto sin
  que nadie las etiquete a mano; (b) multiplicar llamadas al LLM — es una
  sola, igual de barata que la que ya hace extracción de memoria/mood.
- **Primer scheduler real del proyecto.** A diferencia de mood (que decae
  perezosamente, sin timer) o memoria semántica (que embebe bajo demanda),
  "proactivo" por definición necesita actuar sin que haya una interacción
  — exige un `tokio::spawn` con un loop periódico (15 minutos), el primer
  productor real desde que el heartbeat de `lib.rs` quedó marcado como
  scaffolding descartable. Al escribirse este ADR no hacía falta bandeja del
  sistema ni notificaciones de Windows: la app no tenía tray icon ni
  interceptaba el cierre de ventana, así que el motor simplemente corre mientras
  el proceso vive, igual que cualquier otro `ManagedService`. *(Actualización: más
  tarde se añadieron la minimización a la bandeja (opt-in) y una notificación de
  escritorio para los mensajes proactivos — ver `docs/DEVELOPMENT_STATUS.md`.)*
- **`ProactiveService` arranca solo al iniciar la app**, a diferencia de
  Twitch/Discord/Llamada (que esperan una conexión explícita del
  usuario) — el on/off real vive en `profile_proactive.enabled` por
  perfil, no en si el `ManagedService` está corriendo.
- **Núcleo de decisión separado de la resolución de conexión LLM**
  (`Inner::evaluate_with` vs `Inner::evaluate_once`) — mismo motivo que
  `MemoryManager::extract_and_store` recibe `provider`/`model` como
  parámetros en vez de resolverlos internamente: permite testear toda la
  lógica de gating (habilitado, cooldown, inactividad, sin conversación)
  con un `MockLLMProvider`, sin nunca necesitar una conexión real.
- **Reusa el pipeline de eventos existente para el contenido del
  mensaje.** `ChatService::send_proactive_message` (nuevo método público)
  inserta el mensaje ya completo y publica los mismos tres eventos que una
  respuesta normal (`AiResponseStarted`/`Token`/`Finished`) — se ve, anima
  el avatar y dispara auto-speak igual que cualquier respuesta, sin un
  camino de UI paralelo. Un cuarto evento, `AppEvent::ProactiveMessageSent`,
  existe solo para que el frontend marque "no leído" en el Sidebar aunque
  el usuario esté en otra pestaña.
- **v1 no abre conversaciones en frío** — si el perfil nunca chateó, el
  motor no hace nada; solo continúa relaciones ya existentes.
- **Trait extendido en vez de servicio nuevo para el estado**: el on/off
  y el cooldown (`ProactiveSettings`, tabla `profile_proactive` — mismo
  molde que `profile_voice`) se agregan como tres métodos más a
  `MemoryManager`, porque `ProactiveService` ya necesita `Arc<dyn MemoryManager>`
  para leer mood y memorias — mismo criterio que `get_mood`/`reset_mood`.
- **Solo Chat participa en v1** — mismo alcance que memoria semántica y
  mood tuvieron al principio.

## Razones

- Combinar las 4 señales en un solo prompt es más simple de construir,
  mantener y razonar que cuatro subsistemas — y deja que el modelo, no
  reglas hardcodeadas, decida cómo pesarlas entre sí.
- Deltas/decisión pedidos al mismo tipo de llamada que ya usa
  memoria/mood (JSON estructurado, temperatura baja) reusa un patrón ya
  probado en vez de inventar uno nuevo.
- Separar `evaluate_with` de `evaluate_once` es lo que hace testeable un
  motor que de otra forma dependería de un servidor LLM real corriendo —
  las 6 pruebas de gating (deshabilitado, cooldown, inactividad, sin
  conversación, el modelo declina, camino feliz) corren con un mock, en
  milisegundos.
- No abrir conversaciones en frío evita un "hola, soy nuevo" forzado y
  mantiene el alcance de v1 acotado — extenderlo es una condición menos,
  cuando haya una razón concreta.

## Consecuencias

- El personaje puede escribir mientras el usuario está en cualquier
  pestaña de la app (no cerrada) — el punto de "no leído" en el Sidebar
  era la única señal de que pasó cuando se escribió este ADR (sin notificaciones
  nativas de Windows); hoy además hay una notificación de escritorio.
- Cerrar la app detiene el motor como cualquier otro servicio — un
  mensaje proactivo nunca llega con la ventana cerrada.
- `CHECK_INTERVAL` (15 min), `MIN_INACTIVITY` (90 min) y `MIN_COOLDOWN`
  (4 horas) son constantes de código en v1, no configurables desde
  Configuración — ajustarlas es un cambio de una línea si hace falta.
- Twitch/Discord/Llamada no ganan comportamiento nuevo — el modo
  proactivo es exclusivo de Chat.
- Modo proactivo → avatar (expresión especial al recibir un mensaje
  proactivo) queda afuera de v1, deliberadamente — extensión natural, no
  bloqueante.
