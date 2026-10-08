-- Memorias más densas y organizables (pedido del usuario tras usar Fase 4
-- en vivo): un título corto además del contenido, y `summary` — ya existía
-- en el esquema desde el diseño original pero nunca se usaba — pasa a ser
-- el "contexto resumido" que se muestra por defecto, con `content` como el
-- detalle más extenso disponible bajo demanda.
ALTER TABLE memories ADD COLUMN title TEXT NOT NULL DEFAULT '';

-- FTS5 no admite ALTER para añadir una columna a una tabla virtual
-- existente — se recrea con `title` incluido y se repuebla desde la tabla
-- base. Software aún no lanzado, sin datos de producción que preservar con
-- cuidado especial más allá de una migración limpia.
DROP TRIGGER memories_ai;
DROP TRIGGER memories_ad;
DROP TRIGGER memories_au;
DROP TABLE memories_fts;

CREATE VIRTUAL TABLE memories_fts USING fts5(title, content, summary, content='memories', content_rowid='rowid');
INSERT INTO memories_fts(rowid, title, content, summary) SELECT rowid, title, content, summary FROM memories;

CREATE TRIGGER memories_ai AFTER INSERT ON memories BEGIN
    INSERT INTO memories_fts(rowid, title, content, summary) VALUES (new.rowid, new.title, new.content, new.summary);
END;
CREATE TRIGGER memories_ad AFTER DELETE ON memories BEGIN
    INSERT INTO memories_fts(memories_fts, rowid, title, content, summary) VALUES('delete', old.rowid, old.title, old.content, old.summary);
END;
CREATE TRIGGER memories_au AFTER UPDATE ON memories BEGIN
    INSERT INTO memories_fts(memories_fts, rowid, title, content, summary) VALUES('delete', old.rowid, old.title, old.content, old.summary);
    INSERT INTO memories_fts(rowid, title, content, summary) VALUES (new.rowid, new.title, new.content, new.summary);
END;
