-- Preferencias de la ventana flotante del avatar (glow, opacidad, tamaño
-- recordado) — distinto de las imágenes por estado (avatar_idle_path,
-- etc., columnas de `profiles`). Fila ausente = valores por defecto
-- (sin glow, opacidad completa, 360x480), mismo criterio que
-- profile_moods/profile_proactive.
CREATE TABLE profile_avatar_window (
    profile_id TEXT PRIMARY KEY REFERENCES profiles(id) ON DELETE CASCADE,
    glow_enabled INTEGER NOT NULL DEFAULT 0,
    opacity REAL NOT NULL DEFAULT 1.0,
    width REAL NOT NULL DEFAULT 360,
    height REAL NOT NULL DEFAULT 480
);
