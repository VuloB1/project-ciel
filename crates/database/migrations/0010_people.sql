-- Registro de personas conocidas (retoma el tier 4 "Person memory" de
-- ADR-004, diferido en Fase 4 — ver docs/DEVELOPMENT_STATUS.md, sección de
-- "Decisiones de alcance" de esa fase, y docs/ADR/004-memory-architecture.md).
-- Se retoma ahora porque ya hay un consumidor concreto: el prompt de
-- Whisper (`CallService::Inner::transcription_prompt`) necesita nombres
-- reales para no confundirlos con palabras parecidas al hablar.
--
-- Deliberadamente UNA sola tabla, no el `people` + `memory_people` (tabla
-- de enlace) del diseño original de ADR-004: `memory_people` existía para
-- vincular personas a memorias concretas (un grafo de entidades) — nada
-- consume ese vínculo hoy, sería exactamente el mismo "esquema muerto" que
-- ya se evitó una vez. Si una feature concreta lo llega a necesitar, se
-- añade entonces, con el mismo criterio que justifica esta tabla ahora.
--
-- Global, no por perfil (sin FK a profiles): son personas reales de la
-- vida/comunidad del usuario, relevantes sin importar qué personaje de IA
-- esté activo.
CREATE TABLE people (
    id TEXT PRIMARY KEY,
    canonical_name TEXT NOT NULL,
    -- JSON array de cadenas — apodos u otras formas del mismo nombre.
    -- Nunca se consulta por su contenido en SQL, solo se lee/escribe
    -- entero desde Rust — igual que los blobs JSON de app_settings.
    aliases TEXT NOT NULL DEFAULT '[]',
    role TEXT NOT NULL,                 -- self | nickname | friend | twitch_chatter
    source TEXT NOT NULL,               -- manual | chat | twitch
    -- Clave de deduplicación estable para personas de un canal externo con
    -- ID propio — hoy solo Twitch: "twitch:<chatter_user_id>" (el ID
    -- numérico, no el login: el login puede cambiar, el ID no). NULL para
    -- personas manuales o extraídas del chat.
    external_key TEXT,
    mention_count INTEGER NOT NULL DEFAULT 1,
    last_mentioned_at TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- Solo para el camino de Twitch: garantiza que el mismo chatter nunca
-- produce dos filas.
CREATE UNIQUE INDEX idx_people_external_key ON people(external_key) WHERE external_key IS NOT NULL;

-- Respalda `PeopleRepository::top_for_prompt` — prioriza por rol y luego
-- por mención más reciente.
CREATE INDEX idx_people_role_last_mentioned ON people(role, last_mentioned_at DESC);
