-- AI character profiles (sección 9-11 del brief). No default/official
-- character ships with the app — every row here is user-created.
CREATE TABLE profiles (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    nickname TEXT,
    avatar_path TEXT,
    description TEXT,
    age_fictional TEXT,
    pronouns TEXT,
    occupation TEXT,
    species TEXT,
    accent_color TEXT,
    is_archived INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- Pestaña "Identidad" > Lore + Relación con el usuario.
CREATE TABLE profile_identity (
    profile_id TEXT PRIMARY KEY REFERENCES profiles(id) ON DELETE CASCADE,
    lore TEXT NOT NULL DEFAULT '',
    relationship_to_user TEXT NOT NULL DEFAULT '',  -- "¿quién eres para el usuario?"
    user_role TEXT NOT NULL DEFAULT '',              -- "¿quién es el usuario para ti?"
    speaking_style TEXT NOT NULL DEFAULT '',
    special_instructions TEXT NOT NULL DEFAULT ''
);

-- Pestaña "Identidad" > Personalidad: descripción libre + 19 sliders 0-100.
CREATE TABLE profile_personality (
    profile_id TEXT PRIMARY KEY REFERENCES profiles(id) ON DELETE CASCADE,
    free_description TEXT NOT NULL DEFAULT '',
    warmth INTEGER NOT NULL DEFAULT 50,
    empathy INTEGER NOT NULL DEFAULT 50,
    humor INTEGER NOT NULL DEFAULT 50,
    sarcasm INTEGER NOT NULL DEFAULT 20,
    formality INTEGER NOT NULL DEFAULT 30,
    curiosity INTEGER NOT NULL DEFAULT 50,
    energy INTEGER NOT NULL DEFAULT 50,
    spontaneity INTEGER NOT NULL DEFAULT 50,
    initiative INTEGER NOT NULL DEFAULT 40,
    affection INTEGER NOT NULL DEFAULT 50,
    shyness INTEGER NOT NULL DEFAULT 30,
    confidence INTEGER NOT NULL DEFAULT 50,
    expressiveness INTEGER NOT NULL DEFAULT 50,
    verbosity INTEGER NOT NULL DEFAULT 50,
    flirtiness INTEGER NOT NULL DEFAULT 0,
    emotional_intensity INTEGER NOT NULL DEFAULT 50,
    teasing_tendency INTEGER NOT NULL DEFAULT 30,
    question_tendency INTEGER NOT NULL DEFAULT 40,
    initiate_conversation_tendency INTEGER NOT NULL DEFAULT 30
);
