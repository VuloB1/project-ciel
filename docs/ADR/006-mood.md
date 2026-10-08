# ADR-006: Estado emocional evolutivo por personaje (mood)

## Estado

Aceptado e implementado.

## Contexto

La app ya tiene personalidades detalladas por personaje
(`ProfilePersonality`, 18 sliders 0-100 + descripción libre), pero son
**configuración estática** — el mismo perfil siempre renderiza
exactamente el mismo texto en el prompt (determinismo verificado por
`PersonalityPromptBuilder::build_is_deterministic`, aprovechado además
por la "vista previa del prompt" de la pestaña Identidad). Faltaba un
**estado que evolucione** con la conversación y con el tiempo real
transcurrido — "cómo está" el personaje en este momento, distinto de
"cómo es".

El pedido surgió de una ronda de recomendaciones evaluadas junto con el
usuario (búsqueda semántica de memoria, modo proactivo, moods) — de esas
tres, mood es la que más se conecta con lo ya construido (personalidades
profundas ya existentes) y la más acotada en alcance.

## Decisión

- **Dos ejes numéricos, `valence` y `energy`, 0-100 cada uno**, no un enum
  de emociones fijas. `valence`: negativo↔positivo. `energy`:
  bajo/tranquilo↔alto/enérgico. Baseline neutral = 50/50.
- **El modelo devuelve un delta por eje** (-15..15, clampeado
  server-side) en la **misma llamada al LLM que ya extrae memorias**
  cada 6 mensajes (`crates/memory/src/extraction.rs`), no un request
  aparte ni una heurística sin LLM — costo extra cero (mismo JSON, un
  campo más).
- **Decae hacia 50/50 con el tiempo real transcurrido**, calculado
  perezosamente (`MoodRepository::apply_delta` compara `updated_at`
  contra `Utc::now()` en cada aplicación) — sin ningún timer corriendo.
  Pasa siempre que corre la extracción, incluso si el modelo no manda un
  delta explícito.
- **Ausencia de fila en `profile_moods` = neutral (50/50)** — no hace
  falta sembrar una fila al crear el perfil.
- **Vive dentro de `MemoryManager`** (`crates/memory`), sin trait ni
  crate nuevo — `extract_and_store` ya escribía memorias y personas
  desde la misma respuesta del modelo sin devolverlas por separado; mood
  se suma como un tercer efecto secundario de la misma llamada.
  `get_mood`/`reset_mood` se agregaron como dos métodos más al trait
  `MemoryManager` (que ya había ganado `backfill_embeddings` antes por
  una razón análoga).
- **Solo Chat participa en v1** — Twitch/Discord/Llamada reciben `None`
  en el nuevo parámetro `mood` de `AIOrchestrator::stream_respond`, mismo
  alcance que tuvo la memoria semántica al principio.

## Razones

- Un delta pedido al mismo modelo que ya analiza la conversación (en vez
  de una heurística de palabras clave) entiende sarcasmo/contexto real,
  sin costo adicional real — es el mismo request, un campo más en el
  JSON.
- Deltas, no valores absolutos: el modelo juzga "cuánto cambió esto",
  nunca tiene que inventar un baseline falso que probablemente esté mal.
- Decadencia perezosa (sin timer) evita introducir el primer scheduler
  por-servicio del proyecto solo para esto — el único precedente
  existente (el heartbeat de `lib.rs`) está marcado explícitamente como
  scaffolding descartable.
- Reusar `MemoryManager` en vez de un servicio nuevo evita todo el
  wiring de bootstrap (`AppState`, construcción en `lib.rs`,
  registro de comandos) que un `Arc<dyn XManager>` nuevo exigiría, para
  dos métodos que son passthrough directo a un repositorio.
- Alcance Chat-only en v1: evita expandir el parámetro `mood` a fetches
  nuevos en tres crates de servicio más antes de saber si el resultado
  vale la pena en producción — extenderlo después es una línea por canal.

## Consecuencias

- *(Actualizado 2026-10, ADR-013: la Llamada ya **lee** el estado de ánimo; sigue sin
  escribirlo. Twitch y Discord siguen sin usarlo, a propósito: es un canal público.)*
- El personaje puede sonar distinto en Chat que en Twitch/Discord/Llamada
  para la misma sesión de uso, ya que solo Chat lee y escribe mood — una
  inconsistencia conocida y aceptada, no un bug.
- Ningún canal fuera de Chat necesita cambios para seguir funcionando
  igual que antes — pasan `None` explícito, cero comportamiento nuevo.
- Un botón de "Reiniciar" manual (Identidad > Personalidad) da una
  válvula de escape si un mood queda en un estado raro por una mala
  extracción — no hace falta diálogo de confirmación, no es una acción
  destructiva de datos.
- Modulación de voz por mood (el `stability` de ElevenLabs, hoy
  hardcodeado a 0.5) y expresión del avatar por mood quedan afuera de
  v1, deliberadamente — son extensiones naturales, no bloqueantes.
