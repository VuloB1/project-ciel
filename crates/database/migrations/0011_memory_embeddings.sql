-- Embeddings locales para búsqueda semántica de memorias — capa opcional
-- sobre FTS5 (memories_fts), no un reemplazo. Ver
-- docs/ADR/004-memory-architecture.md y (cuando se escriba, Etapa 3)
-- docs/ADR/005-local-embedding-search.md.
--
-- Tabla satélite, no una columna en `memories`: la ausencia de fila
-- significa "todavía no embebida" (modelo no descargado, o memoria
-- creada antes de descargarlo) sin necesitar una columna nullable en la
-- tabla más caliente del esquema.
CREATE TABLE memory_embeddings (
    memory_id TEXT PRIMARY KEY REFERENCES memories(id) ON DELETE CASCADE,
    embedding BLOB NOT NULL,     -- Vec<f32> little-endian crudo, sin cabecera
    model_version TEXT NOT NULL, -- aivt_provider_embedding::MODEL_VERSION
    updated_at TEXT NOT NULL
);
