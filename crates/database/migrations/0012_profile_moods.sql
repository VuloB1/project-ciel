-- Estado emocional evolutivo por personaje, distinto de
-- `profile_personality` (que es configuración estática — ver
-- docs/ADR/006-mood.md). Fila ausente = neutral (50/50), sin necesidad
-- de sembrar una fila al crear el perfil.
CREATE TABLE profile_moods (
    profile_id TEXT PRIMARY KEY REFERENCES profiles(id) ON DELETE CASCADE,
    valence INTEGER NOT NULL, -- 0-100, negativo..positivo
    energy INTEGER NOT NULL,  -- 0-100, bajo/tranquilo..alto/enérgico
    updated_at TEXT NOT NULL
);
