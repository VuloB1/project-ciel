# ADR-015: Centro de diagnóstico

## Estado

Aceptado e implementado (2026-10). Probado con pruebas automáticas; **pendiente de ver
en la app con datos reales**.

## Contexto

Cuando algo fallaba (el chat no responde, la voz no suena, un servicio "no hace nada")
la única forma de entender por qué era mirar la pestaña Actividad o el archivo de logs.
No había un lugar que respondiera "¿está todo bien?" ni algo que el usuario pudiera
mandar para pedir ayuda sin copiar logs a mano (con el riesgo de pegar una credencial).

## Decisión

### Un informe, no una pantalla de logs

Configuración > Diagnóstico muestra el estado de la app en una sola pantalla:

- **Comprobaciones** con severidad (`ok`, `info`, `warning`, `error`) y una explicación en
  lenguaje normal de qué pasa y qué hacer.
- **Detalle**: servicios (Twitch, Discord, Llamada, proactivo, avatar para OBS), proveedores
  de IA y de voz, modelos locales, base de datos (integridad, versión de esquema, tamaño,
  número de filas), almacenamiento por carpeta, sistema, y los **problemas recientes**
  sacados de la actividad (avisos y errores).

### Qué se comprueba

| Código | Severidad | Cuándo |
|---|---|---|
| `database_integrity` | error | `PRAGMA integrity_check` ≠ ok |
| `database_schema` | warning | el esquema aplicado no es el que espera la app |
| `llm_not_ready` / `llm_current_not_ready` / `llm_no_current` | warning | ningún proveedor listo, el de uso a medio configurar, o ninguno en uso |
| `llm_cooling_down` | warning | el proveedor en uso está en pausa tras fallar (con los segundos que faltan) |
| `llm_no_fallback` | info | solo hay un proveedor listo: el cambio automático no tiene a dónde pasar |
| `tts_missing_key` | warning | el motor de voz en uso es de nube y no tiene key |
| `service_error` | error | un servicio está en error |
| `stt_model_missing` / `embedding_missing` | info | modelos locales sin descargar (opcionales) |
| `recent_errors` | warning | hay errores en la actividad reciente |
| `pre_restore_leftover` | info | datos de antes de una restauración que se pueden borrar |

Lo `info` nunca vuelve el estado general un problema: son notas, no fallos.

### Los códigos se traducen en la interfaz

El backend devuelve **código + parámetros**, no frases. La interfaz los traduce a los 6
idiomas, y un código que aún no tenga traducción se muestra con un texto genérico en lugar
de una clave cruda: añadir una comprobación nueva en Rust no rompe la pantalla.

### Nunca secretos

- De cada proveedor solo se informa si **tiene** API key (sí/no), nunca su valor.
- El texto de los problemas pasa por `redact_secrets`, y el informe de texto completo vuelve a
  pasar por ella como última barrera. Hay pruebas de ambas cosas.
- Por eso se puede **copiar** o **guardar como `.txt`** y compartir para pedir ayuda
  (`docs/security.md`, "Diagnóstico y exports").

### Estructura

- Crate `aivt-service-diagnostics`: no depende de los servicios. Mide lo que puede (base de
  datos, almacenamiento) y recibe ya simples los demás datos; calcula las comprobaciones y
  genera el texto. Así la lógica se prueba sin levantar la app.
- `src-tauri/src/commands/diagnostics.rs` reúne los datos (servicios, proveedores, modelos,
  actividad) y llama al crate. Comandos: `diagnostics_run`, `diagnostics_text`,
  `diagnostics_save`.
- `SettingsService` expone `LlmProviderEntry::is_ready()` (misma regla que el cambio
  automático) y `llm_provider_cooldown_remaining_secs()`, solo lectura.

## Razones

- Un informe estructurado responde a "¿qué falla y por qué?" mejor que una lista cronológica.
- Reutilizar la regla de "proveedor listo" del cambio automático garantiza que el diagnóstico
  diga lo mismo que hace la app, en vez de una segunda definición que se desincronice.
- Devolver códigos permite traducir sin duplicar la lógica en el frontend.

## Alternativas descartadas

- **Pruebas activas** (mandar un mensaje a cada proveedor, abrir el micrófono): cuestan
  créditos o tienen efectos; ya existen los botones "Probar" de cada proveedor.
- **Mostrar los logs en bruto:** ya está la pestaña Actividad, y los logs de archivo pueden
  contener datos que no conviene compartir.
- **Espacio libre en disco:** necesita una dependencia nueva por una sola cifra; queda para
  cuando se pida.

## Consecuencias

- Es una foto: no se actualiza sola. Hay un botón "Volver a comprobar".
- Los "problemas recientes" salen solo de eventos de actividad (target `activity`); los
  errores que solo van al archivo de log no aparecen.
- La comprobación de integridad recorre toda la base: con datos muy grandes la pantalla
  tarda más en abrir.
