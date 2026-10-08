-- Memoria del personaje (sección 29-41 del brief, Fase 4). FTS5 sobre
-- contenido externo (content='memories'), sincronizada con triggers — el
-- patrón estándar de SQLite para no duplicar el texto dos veces en disco.
CREATE TABLE memories (
    id TEXT PRIMARY KEY,
    profile_id TEXT NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
    content TEXT NOT NULL,
    summary TEXT,
    category TEXT NOT NULL,             -- person | preference | event | fact | other
    importance INTEGER NOT NULL,        -- 0-100
    confidence INTEGER NOT NULL,        -- 0-100
    source TEXT NOT NULL,               -- chat | manual (más fuentes cuando Twitch/Discord/Call las produzcan)
    source_id TEXT,                     -- p.ej. conversation_id, cuando aplica
    is_pinned INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    last_accessed_at TEXT,
    access_count INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX idx_memories_profile_id ON memories(profile_id, importance DESC);

CREATE VIRTUAL TABLE memories_fts USING fts5(content, summary, content='memories', content_rowid='rowid');

CREATE TRIGGER memories_ai AFTER INSERT ON memories BEGIN
    INSERT INTO memories_fts(rowid, content, summary) VALUES (new.rowid, new.content, new.summary);
END;
CREATE TRIGGER memories_ad AFTER DELETE ON memories BEGIN
    INSERT INTO memories_fts(memories_fts, rowid, content, summary) VALUES('delete', old.rowid, old.content, old.summary);
END;
CREATE TRIGGER memories_au AFTER UPDATE ON memories BEGIN
    INSERT INTO memories_fts(memories_fts, rowid, content, summary) VALUES('delete', old.rowid, old.content, old.summary);
    INSERT INTO memories_fts(rowid, content, summary) VALUES (new.rowid, new.content, new.summary);
END;

-- Cuántos mensajes de la conversación ya se consideraron para extracción de
-- memoria — evita reprocesar los mismos mensajes en cada consolidación
-- automática (sección 39 del brief: nunca en cada mensaje).
ALTER TABLE conversations ADD COLUMN memory_consolidated_through INTEGER NOT NULL DEFAULT 0;
