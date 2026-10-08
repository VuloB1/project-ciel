# ADR-002: SQLite + sqlx (sin ORM completo)

## Estado

Aceptado.

## Contexto

Necesitamos persistencia local (perfiles, conversaciones, memoria, activity
log, configuración de integraciones) que sea local-first, sin servidor
externo, con migraciones versionadas, y que soporte búsqueda de texto (FTS5)
para la recuperación de memoria semántica (Fase 4).

## Decisión

**SQLite** como motor, accedido vía **`sqlx`** (async, sin capa ORM
completa encima) con un patrón Repository explícito por tabla.

## Alternativas evaluadas

| Opción | Por qué no |
|---|---|
| **Diesel** | Sólido y maduro, pero síncrono — requiere un pool bloqueante conviviendo con el runtime async de Tokio/Tauri (`spawn_blocking` en cada query). Su DSL de macros es pesado y encaja mal con FTS5 y con queries de scoring dinámicas para retrieval de memoria. |
| **SeaORM** | Async y construido sobre `sqlx`, pero es un ORM "active record" completo — más abstracción y magia de la que el brief pide ("ligera y mantenible"). |
| **rusqlite** | Muy ligero, pero síncrono puro y sin sistema de migraciones versionado de fábrica — habría que construir esa pieza a mano. |

## Razones para sqlx

1. **Async nativo** — encaja con Tokio/Tauri sin pools bloqueantes.
2. **Migraciones de fábrica** (`sqlx::migrate!`) — requisito explícito de
   "nunca modificar el esquema sin una migration".
3. **Verificación de queries en tiempo de compilación** disponible
   (`query!`/`query_as!`) cuando el entorno lo soporte, sin necesidad de
   generar un DSL propio encima.
4. **Control total del SQL** — necesario para FTS5 (memoria), UPSERTs de
   deduplicación de memoria, y queries de retrieval con scoring que un ORM
   completo dificultaría o requeriría "raw SQL escape hatches" de todas
   formas.

## Consecuencias

- Fase 0 usa la API dinámica de sqlx (`sqlx::query`/`query_as`) en vez de
  las macros verificadas en compilación, porque estas últimas requieren una
  base de datos accesible (o un caché `.sqlx` versionado) en el momento de
  `cargo build`, y no queríamos acoplar el build del proyecto a eso todavía.
  Es un follow-up de bajo riesgo migrar las queries de mayor tráfico a
  `query!` una vez exista un flujo de CI/dev-db.
- Cada tabla nueva requiere escribir su propio repositorio (más código
  explícito que un ORM generativo) — trade-off aceptado a cambio de
  legibilidad y control.
