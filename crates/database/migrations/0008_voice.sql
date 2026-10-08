-- Pestaña "Voz" (Fase 5). Ajustes de voz por perfil — el motor real
-- (nativo del SO u HTTP externo) se elige globalmente en Configuración >
-- AI Providers, pero cada personaje tiene su propia voz/velocidad/tono.
CREATE TABLE profile_voice (
    profile_id TEXT PRIMARY KEY REFERENCES profiles(id) ON DELETE CASCADE,
    enabled INTEGER NOT NULL DEFAULT 0,   -- auto-reproducir cada respuesta del chat
    voice_id TEXT NOT NULL DEFAULT '',
    rate REAL NOT NULL DEFAULT 1.0,
    pitch REAL NOT NULL DEFAULT 1.0,
    volume REAL NOT NULL DEFAULT 1.0
);

-- Perfiles creados antes de esta migración no tienen fila todavía.
INSERT INTO profile_voice (profile_id, enabled, voice_id, rate, pitch, volume)
SELECT id, 0, '', 1.0, 1.0, 1.0 FROM profiles;
