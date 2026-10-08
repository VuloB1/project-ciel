# ADR-010: Varios proveedores de IA y de voz a la vez, con cambio automático para IA

## Estado

Aceptado e implementado (2026-09/10). Probado en vivo con servicios reales
solo en el caso LM Studio apagado → Groq; el resto, con servidores falsos y
pruebas unitarias (ver "Pendiente / sin verificar" en
`docs/DEVELOPMENT_STATUS.md`).

## Contexto

ADR-003 fijó el contrato `LLMProvider`/`TTSProvider`, pero la app arrancó con
**una sola conexión** por capacidad (un servidor OpenAI-compatible para el
LLM; un motor de voz global). El usuario pidió poder tener varios servicios
configurados a la vez (OpenAI, Anthropic, Gemini, Groq, OpenRouter, ...), con
una experiencia de alta tipo "pega tu API key" para los conocidos, y que la
app "nunca deje de funcionar": si el proveedor en uso se queda sin crédito, llega
a su límite del plan gratuito o se cae, debe pasar solo a otro.

## Decisión

### Almacenamiento

- Cada capacidad guarda una **lista** de proveedores bajo una clave versionada
  de `app_settings` (`llm_providers_v1`, `tts_providers_v1`) más un puntero
  `current_provider_id`. No se mutó la clave vieja (`llm_connection`,
  `tts_connection`): se migra una sola vez, en silencio, y queda huérfana sin
  borrar — mismo criterio que `appearance_settings_v2`.
- Cada entrada tiene `enabled` ("activar": se puede elegir a mano) y, solo en
  IA, `pinned` ("fijar": además entra en el cambio automático).
- Un secreto **por proveedor** (`api_key.{id}`) en `SecretStore`; la base solo
  guarda `has_api_key`. Cambiar de servicio nunca pisa la key de otro.
- El tipo (`LlmProviderKind`, `TtsBackendKind`) decide qué pide la UI: los
  servicios conocidos traen su URL fija y solo piden la key; "servidor local" /
  "genérico" piden la URL. Un solo proveedor de cada tipo conocido; ilimitados
  genéricos. Los que hablan el protocolo de OpenAI no necesitan adaptador: solo
  su URL (`LlmProviderKind::openai_compatible_base_url`,
  `HttpEngine::synthesize_preset` en voz).
- Anthropic y Gemini tienen adaptador propio (formato, autenticación y SSE
  distintos).
- La construcción del proveedor activo vive en un solo sitio:
  `SettingsService::build_llm_provider` / `build_tts_backend`. Los canales (Chat,
  Llamada, Discord, Twitch, mensajes espontáneos) nunca arman uno a mano.

### Cambio automático (solo IA)

`FailoverLLMProvider` envuelve un pool ordenado: el proveedor en uso primero, y
luego los activos y fijados **que puedan responder** (con modelo y, si son de
nube, con API key — un fijado a medio configurar no entra).

- Solo mira el **primer fragmento** de cada intento: nunca cambia de proveedor a
  mitad de una respuesta ya empezada.
- Cada intento sustituye el `model` de la petición por el de esa entrada (los
  nombres de modelo no son intercambiables entre servicios).
- El proveedor **en uso** cede el turno si `ProviderError::should_fail_over()`: sin
  crédito, límite de solicitudes, servidor caído, timeout, error 5xx, o sin
  primer fragmento en 45 s. Una key inválida o un modelo inexistente se
  muestran tal cual: cambiar de proveedor no los arregla y esconderlos
  confundiría.
- Los proveedores **de reserva** se saltan ante cualquier fallo.
- Si nadie responde se muestra el error del proveedor en uso y el "en uso" no se
  mueve.
- El cambio se avisa **una sola vez** y solo cuando un reemplazo de verdad
  respondió, con un motivo tipado (`FailoverReason`: sin crédito, límite de uso,
  no responde, tarda demasiado, error del servidor, o recuperado) que la interfaz
  traduce a un aviso claro ("Cambié a Groq: LM Studio no responde").

### Proveedor preferido vs. proveedor en uso (recuperación automática)

El almacenamiento distingue dos punteros: `preferred_provider_id` (el que el
usuario eligió con "usar ahora") y `current_provider_id` (el que está
respondiendo, que puede ser uno temporal). Un cambio automático mueve **solo** el
segundo; una elección manual mueve los dos. En cada petición el pool empieza por
el preferido, luego el que esté en uso y luego las reservas, así que cuando el
preferido vuelve a responder se recupera solo y se avisa (`restored`).

Para que un preferido caído no cueste una espera en cada mensaje, un proveedor
que falló pasa a **pausa** un rato (`cooldown_for`: 1 min apagado, 90 s límite de
uso, 2 min error 5xx, 5 min sin responder, 10 min sin crédito) y se prueba al final
del pool hasta que vence. La pausa vive en memoria (un `static` del crate de ajustes,
porque hay instancias de `SettingsService` que se crean al vuelo) y se borra al elegir
el proveedor a mano o cuando responde. Los datos guardados antes de existir el
campo tratan el que esté en uso como preferido.

### Capas

`aivt-provider-llm` no depende de eventbus ni de settings (ADR-003): el router
avisa el cambio por un canal `mpsc` genérico, y `build_llm_provider` lo convierte
en `AppEvent::LlmProviderSwitched` y actualiza `current_provider_id`. El
frontend refresca la lista al recibir el evento.

### Voz: sin rotación

El motor de voz también admite varios proveedores, pero **sin** cambio
automático: el usuario elige a mano ("usar ahora"). Cambiar de voz a mitad de una
frase sería audible y raro, y no hay un "sin crédito" equivalente que justifique
el riesgo. "Nativo" es un singleton siempre presente que no se puede borrar.

## Razones

- Una lista versionada + puntero es el modelo mínimo que sirve tanto para elegir
  a mano como para rotar, y migra sin perder la configuración existente.
- Mirar solo el primer fragmento hace el cambio invisible y seguro: o la
  respuesta empieza en un proveedor, o no empieza.
- Distinguir "no puede atender ahora" de "está mal configurado" evita el peor
  fallo posible de un failover: ocultar un error de configuración detrás de un
  cambio silencioso.
- El plan gratuito de Groq responde con límites de solicitudes o de tokens, no con
  "sin crédito": por eso el criterio inicial (solo `QuotaExceeded`) fue
  insuficiente y se amplió (ver Consecuencias).

## Alternativas descartadas

- **Reintentar a mitad de respuesta:** duplicaría o cortaría texto ya mostrado.
- **Rotar ante cualquier error:** una key mala saltaría en silencio a otro
  servicio, gastando su plan sin que el usuario lo sepa.
- **Rotación automática en voz:** ver arriba.
- **Un adaptador por servicio compatible con OpenAI:** código repetido; basta la
  URL.

## Consecuencias

- El primer criterio de rotación (solo "sin crédito") se probó en la práctica y no
  cubría el caso real del usuario (apagar LM Studio, o el límite de Groq): se
  amplió a `should_fail_over()` y se añadió el tiempo de espera de 45 s, que es
  holgado a propósito porque un modelo local grande puede tardar en emitir su
  primer token. Un modelo local legítimamente más lento que eso saltaría al
  respaldo.
- Con la recuperación, una pausa corta significa que un preferido que sigue caído se
  vuelve a intentar cada minuto o más; si está *colgado* (no apagado) cada intento
  cuesta hasta 45 s, por eso esa pausa es de 5 minutos.
- Cada llamada al LLM dentro de una misma respuesta (p. ej. la consolidación de
  memoria o la segunda pasada tras una herramienta) vuelve a probar primero el
  proveedor en uso del momento en que se armó el pool: un error rápido de más,
  inofensivo. El aviso de cambio no se repite: si el proveedor en uso ya es el que
  respondió, no se publica otra vez.
- Con la rotación, el texto del usuario puede acabar en cualquiera de los
  proveedores fijados (ver `docs/security.md`).
- Pendiente de verificar con claves reales: Anthropic y Gemini (los adaptadores y
  el cambio solo se probaron con pruebas unitarias y servidores falsos).
