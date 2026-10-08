# Seguridad y privacidad

## Secretos

API keys, tokens OAuth de Twitch, tokens de bot de Discord y credenciales
similares **nunca** se guardan en texto plano — ni en SQLite, ni en archivos
de configuración, ni en logs.

- Abstracción: trait `SecretStore` (`crates/secrets`), con una única
  implementación real hoy, `KeyringSecretStore`, sobre la crate `keyring`:
  - Windows → Credential Manager.
  - macOS → Keychain.
  - Linux → Secret Service síncrono (D-Bus).
- Cada proveedor de IA o de voz guarda su API key en su propio slot de
  `SecretStore` (`api_key.{id}`); la base de datos solo guarda si hay una
  (`has_api_key`), nunca el valor. El `Debug` de `TtsBackend` redacta las
  keys a mano (una key real se filtró una vez a un log) y "Probar conexión"
  usa la key ya guardada sin devolverla al frontend.
- Las tablas que referencian un secreto (`providers`, `twitch_settings`,
  `discord_settings`, cuando existan) guardan un `secret_ref` — una clave
  que apunta a una entrada en `SecretStore` — nunca el valor.
- **Riesgo conocido:** en Linux headless o distros mínimas sin un backend de
  Secret Service en ejecución, `SecretStore::get/set` retorna
  `SecretError::Unavailable` con un mensaje explicable en la UI. No existe
  (ni existirá) un fallback silencioso a almacenamiento sin cifrar — si el
  keyring del sistema no está disponible, la función que lo necesita falla
  de forma visible en vez de degradar seguridad en silencio.

## Redacción de logs

`aivt_core::redact_secrets` (regex-based, ver `crates/core/src/redact.rs`)
enmascara patrones que parecen API keys, bearer tokens o tokens de bot antes
de que un texto llegue a un log o a `activity_events`. Esto es defensa en
profundidad — la garantía real es que el código nunca debe pasar un valor
de `SecretStore` a un campo que se loguea. Si `redact_secrets` alguna vez
deja pasar un secreto, es un bug a corregir, no evidencia de que la función
sobra.

`redact_secrets` se creó en la Fase 0, antes de que existiera ningún proveedor
real. Hoy hay muchos secretos en juego (API keys de cada proveedor de IA y de
voz, tokens de Twitch y Discord, la key de búsqueda web), todos en `SecretStore`;
el `Debug` de `TtsBackend` redacta las keys a mano porque una key real se filtró
una vez a un log. La redacción general se prueba con los tests unitarios de
`redact_secrets`.

## Privacidad de datos


- **Qué recibe cada proveedor de IA** (ADR-019): cada proveedor tiene un modo de privacidad —
  Completo, Solo recuerdos públicos o Mínimo— que decide si recibe tus recuerdos privados,
  tu estado de ánimo y las imágenes que adjuntas. Se aplica en el propio proveedor, así que
  vale también cuando el cambio automático pasa a otro. La conversación siempre llega.

- La app es local-first: Chat, Memoria, TTS/STT locales, Avatar y Llamada
  local deben funcionar sin conexión a Internet (Twitch/Discord
  naturalmente la requieren).
- Antes de configurar un proveedor cloud (LLM/TTS/STT), la UI debe dejar
  claro que los datos saldrán del equipo — este requisito se implementa a
  partir de Fase 2 (providers reales), documentado aquí como recordatorio
  de diseño.
- Con proveedores en la nube, lo que se escribe o dice sale del equipo hacia
  ese servicio, y con el cambio automático puede ir a cualquiera de los
  proveedores fijados. Algunos servicios usan las peticiones para mejorar sus
  modelos: es el caso del modelo gratuito `s2.1-pro-free` de Fish Audio, que
  es el que se usa por defecto con ese motor — la ayuda del campo "Modelo" lo
  avisa. El dictado por voz transcribe localmente (whisper.cpp): el audio no
  sale del equipo.
- Sin telemetría remota por defecto. Las métricas de observabilidad
  (sección "Developer Mode" del brief original) son locales.
- El centro de diagnóstico (Configuración > Diagnóstico) genera un informe que **nunca incluye secretos**: de cada proveedor solo dice si tiene API key, y los textos de actividad pasan por `redact_secrets` (dos veces: al reunirlos y sobre el informe de texto completo). Es seguro compartirlo para pedir ayuda. Ver ADR-015.
- Las memorias se extraen de charlas privadas con el dueño, así que cada una lleva
  un alcance (`scope`): **privada** por defecto (solo Chat y Llamada la usan) o
  **pública** (también Twitch y Discord, canales abiertos a terceros). Pasar a
  pública es siempre una decisión explícita del usuario, con confirmación, y el
  filtro se aplica dentro de la consulta a la base de datos. Twitch y Discord
  tampoco reciben el estado de ánimo. Ver ADR-013.
- La copia de seguridad (Configuración > Copia de seguridad) **no incluye secretos**: las
  API keys y tokens se quedan en el almacén seguro del sistema, así que perder o compartir
  un `.aivtbackup` no filtra credenciales (sí contiene los chats y memorias: tratarlo como
  un dato privado). Al restaurar se validan el formato, la versión y la integridad, y se
  rechazan rutas peligrosas. Ver ADR-014.
- El registro de personas (Configuración > Privacidad) guarda localmente
  nombres visibles de personas mencionadas en conversaciones y del chat de
  Twitch conectado — nunca se transmite a ningún servidor. El aprendizaje
  automático desde Twitch puede desactivarse desde esa misma pestaña, y
  cualquier entrada puede eliminarse individualmente en cualquier momento.

## Diagnóstico y exports

Cualquier función futura de "copiar diagnóstico" o "exportar logs" debe
pasar el contenido por `redact_secrets` antes de escribirlo a disco o al
portapapeles. Los exports de perfil (`.aivtprofile`, Fase 1) nunca incluyen
API keys ni tokens, incluso si el usuario marca "incluir memorias".
