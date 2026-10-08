-- Structured event log backing the future Actividad tab (Fase 11) and, from
-- day one, the `tracing` layer that feeds it. See docs/architecture.md.
CREATE TABLE activity_events (
    id TEXT PRIMARY KEY,
    timestamp TEXT NOT NULL,
    level TEXT NOT NULL,               -- DEBUG|INFO|WARNING|ERROR|CRITICAL
    category TEXT NOT NULL,            -- CHAT|LLM|STT|TTS|MEMORY|TWITCH|DISCORD|AVATAR|CALL|SYSTEM|DATABASE|NETWORK
    module TEXT NOT NULL,
    action TEXT NOT NULL,
    profile_id TEXT,
    conversation_id TEXT,
    request_id TEXT,
    duration_ms INTEGER,
    status TEXT NOT NULL,
    summary TEXT NOT NULL,
    details_json TEXT
);

CREATE INDEX idx_activity_events_timestamp ON activity_events(timestamp);
CREATE INDEX idx_activity_events_category ON activity_events(category);
