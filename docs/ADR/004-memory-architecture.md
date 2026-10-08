# ADR-004: Arquitectura de memoria por niveles

## Estado

Aceptado (diseño); implementación planificada para Fase 4. Documentado ahora
porque el esquema de `memories` y la forma en que `AIOrchestrator`
consumirá memoria afectan decisiones tomadas antes (`InteractionContext`
tiene `allow_memory`, por ejemplo).

## Contexto

No se puede enviar todo el historial de conversación al modelo
indefinidamente (coste, latencia, límite de contexto), pero el personaje
debe "recordar" cosas importantes de forma persistente y coherente entre
canales (sección 28 del brief: memoria como una de las features
fundamentales).

## Decisión

Memoria en niveles, con responsabilidades separadas:

1. **Working memory** — últimos N mensajes de la conversación actual,
   acotados por presupuesto de tokens del modelo/provider activo. Vive en
   el propio historial de la conversación, no es una tabla aparte.
2. **Session memory** — resumen generado al cerrar una sesión o tras cierto
   número de mensajes (`MemoryConsolidator`), no transcripciones completas.
3. **Semantic memory** (`memories` table) — hechos individuales
   (preferencias, nombres, eventos) con `importance`, `confidence`,
   `source`, `category`; recuperados por `MemoryRetriever` vía SQLite FTS5
   primero, embeddings como mejora posterior (sección 36 del brief).
4. **Person memory** (`people` / `memory_people`) — entidades mencionadas,
   vinculables a memorias.
5. **Pinned memory** — cualquier memoria marcada `is_pinned`, siempre
   priorizada en retrieval independientemente de su score.

`MemoryConsolidator` corre tras N mensajes, al cerrar sesión, en idle, o
manualmente — nunca de forma continua, para mantener bajo el consumo de
recursos (sección 39 del brief, y sección 9 de docs/architecture.md).

## Razones

- Separar "qué se recuerda" (semantic memory, con importancia y
  deduplicación) de "qué se manda al modelo ahora" (working memory acotada
  por tokens) es lo que evita tanto el desbordamiento de contexto como la
  pérdida de continuidad entre sesiones.
- FTS5 antes que embeddings: cumple el requisito de recuperación relevante
  desde el primer milestone de memoria sin forzar una dependencia de
  embeddings (modelo/proveedor adicional) antes de que sea necesaria.

## Consecuencias

- El esquema completo (`memories`, `memories_fts`, `people`,
  `memory_people`) se crea en la migración de Fase 4, no antes — Fase 0 no
  incluye estas tablas (ver [database.md](../database.md)).
- `InteractionContext.allow_memory` (ya definido en `crates/core` desde
  Fase 0) es el punto de control por el que un canal puede desactivar
  lectura/escritura de memoria para una interacción puntual sin tocar
  `MemoryManager`.
