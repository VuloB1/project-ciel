-- Adjuntos de mensajes (sección 3 del brief, Fase 3: Multimodal). Solo
-- imágenes por ahora — `kind` deja espacio para 'audio' cuando llegue con su
-- propia migración (Fase 5/6), en vez de crear columnas muertas hoy
-- (transcript/duration_ms) para una feature que todavía no existe.
CREATE TABLE attachments (
    id TEXT PRIMARY KEY,
    message_id TEXT NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    kind TEXT NOT NULL DEFAULT 'image',
    path TEXT NOT NULL,                 -- relativo a <data_dir>/attachments/, nunca base64 en SQLite
    mime_type TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,
    hash TEXT NOT NULL,
    created_at TEXT NOT NULL
);
CREATE INDEX idx_attachments_message_id ON attachments(message_id);
