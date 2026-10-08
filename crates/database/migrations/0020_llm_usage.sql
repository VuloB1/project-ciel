-- Uso de cada proveedor de IA, agregado por día (Configuración > Proveedores de IA).
-- Solo cifras: nunca se guarda el texto de las peticiones ni de las respuestas.
CREATE TABLE llm_usage (
    provider_id TEXT NOT NULL,
    day TEXT NOT NULL,
    requests INTEGER NOT NULL DEFAULT 0,
    errors INTEGER NOT NULL DEFAULT 0,
    -- Caracteres enviados y recibidos; los tokens se estiman a partir de ellos.
    input_chars INTEGER NOT NULL DEFAULT 0,
    output_chars INTEGER NOT NULL DEFAULT 0,
    last_used_at TEXT NOT NULL,
    last_error TEXT,
    PRIMARY KEY (provider_id, day)
);
