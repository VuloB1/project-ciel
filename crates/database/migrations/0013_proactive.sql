-- Modo proactivo: permite que el personaje inicie una conversación por su
-- cuenta (ver docs/ADR/007-proactive-mode.md). Fila ausente = desactivado,
-- mismo criterio que profile_moods/memory_embeddings — no hace falta
-- sembrar una fila al crear el perfil.
CREATE TABLE profile_proactive (
    profile_id TEXT PRIMARY KEY REFERENCES profiles(id) ON DELETE CASCADE,
    enabled INTEGER NOT NULL DEFAULT 0,
    last_triggered_at TEXT
);
