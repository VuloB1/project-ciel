-- Conversaciones y mensajes (sección 13-14 del brief). `source` distingue
-- de qué canal vino la conversación — hoy solo "chat" existe; "call",
-- "twitch", "discord" se usan a partir de sus fases respectivas sin
-- necesitar cambios de esquema.
CREATE TABLE conversations (
    id TEXT PRIMARY KEY,
    profile_id TEXT NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    source TEXT NOT NULL DEFAULT 'chat',
    model TEXT,
    is_archived INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX idx_conversations_profile_id ON conversations(profile_id, updated_at DESC);

CREATE TABLE messages (
    id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    role TEXT NOT NULL,                        -- user | assistant | system
    content TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'complete',    -- streaming | complete | error | cancelled
    provider_id TEXT,
    model TEXT,
    input_tokens INTEGER,
    output_tokens INTEGER,
    created_at TEXT NOT NULL
);
CREATE INDEX idx_messages_conversation_id ON messages(conversation_id, created_at ASC);
