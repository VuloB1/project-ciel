use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;

use crate::{DatabaseError, DbPool};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileIdentity {
    pub lore: String,
    pub relationship_to_user: String,
    pub user_role: String,
    pub speaking_style: String,
    pub special_instructions: String,
}

impl Default for ProfileIdentity {
    fn default() -> Self {
        Self {
            lore: String::new(),
            relationship_to_user: String::new(),
            user_role: String::new(),
            speaking_style: String::new(),
            special_instructions: String::new(),
        }
    }
}

/// The 19 personality sliders from sección 11 del brief, plus the free-text
/// description. Values are 0-100; translated into prompt instructions by
/// `aivt_orchestrator::PersonalityPromptBuilder`, never sent to the model
/// as raw numbers.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfilePersonality {
    pub free_description: String,
    pub warmth: i64,
    pub empathy: i64,
    pub humor: i64,
    pub sarcasm: i64,
    pub formality: i64,
    pub curiosity: i64,
    pub energy: i64,
    pub spontaneity: i64,
    pub initiative: i64,
    pub affection: i64,
    pub shyness: i64,
    pub confidence: i64,
    pub expressiveness: i64,
    pub verbosity: i64,
    pub flirtiness: i64,
    pub emotional_intensity: i64,
    pub teasing_tendency: i64,
    pub question_tendency: i64,
    pub initiate_conversation_tendency: i64,
}

impl Default for ProfilePersonality {
    fn default() -> Self {
        Self {
            free_description: String::new(),
            warmth: 50,
            empathy: 50,
            humor: 50,
            sarcasm: 20,
            formality: 30,
            curiosity: 50,
            energy: 50,
            spontaneity: 50,
            initiative: 40,
            affection: 50,
            shyness: 30,
            confidence: 50,
            expressiveness: 50,
            verbosity: 50,
            flirtiness: 0,
            emotional_intensity: 50,
            teasing_tendency: 30,
            question_tendency: 40,
            initiate_conversation_tendency: 30,
        }
    }
}

/// Fase 5 — sliders y voz elegida para este personaje. `voice_id` se
/// interpreta según el backend TTS activo globalmente (nativo del SO o
/// HTTP): un id de voz del sistema en el primer caso, lo que el servidor
/// espere como `voice` en el segundo.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileVoice {
    pub enabled: bool,
    pub voice_id: String,
    pub rate: f64,
    pub pitch: f64,
    pub volume: f64,
}

impl Default for ProfileVoice {
    fn default() -> Self {
        Self { enabled: false, voice_id: String::new(), rate: 1.0, pitch: 1.0, volume: 1.0 }
    }
}

/// A `profiles.avatar_*_path` column — Fase 7's "reactive PNG first" set.
/// Kept as a fixed 3-way enum (not a normalized child table) because the
/// state set itself is fixed by what the app's own event lifecycle can
/// drive (see `AppEvent::AiResponse*`/`VoicePlayback*`), not something the
/// user defines — same reasoning as the fixed 19 personality sliders.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AvatarState {
    Idle,
    Thinking,
    Talking,
}

impl AvatarState {
    fn column(self) -> &'static str {
        match self {
            AvatarState::Idle => "avatar_idle_path",
            AvatarState::Thinking => "avatar_thinking_path",
            AvatarState::Talking => "avatar_talking_path",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub nickname: Option<String>,
    pub avatar_path: Option<String>,
    pub description: Option<String>,
    pub age_fictional: Option<String>,
    pub pronouns: Option<String>,
    pub occupation: Option<String>,
    pub species: Option<String>,
    pub accent_color: Option<String>,
    pub theme: Option<String>,
    pub is_archived: bool,
    pub created_at: String,
    pub updated_at: String,
    pub identity: ProfileIdentity,
    pub personality: ProfilePersonality,
    pub voice: ProfileVoice,
    pub avatar_idle_path: Option<String>,
    pub avatar_thinking_path: Option<String>,
    pub avatar_talking_path: Option<String>,
}

/// What the "Crear personaje" action needs — just a name. Everything else
/// starts at its default and is filled in later on the Identidad page.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewProfile {
    pub name: String,
    pub nickname: Option<String>,
    pub description: Option<String>,
}

/// Full replace of the editable fields, submitted together by the
/// Identidad page's save action.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileUpdate {
    pub name: String,
    pub nickname: Option<String>,
    pub description: Option<String>,
    pub age_fictional: Option<String>,
    pub pronouns: Option<String>,
    pub occupation: Option<String>,
    pub species: Option<String>,
    pub accent_color: Option<String>,
    pub theme: Option<String>,
    pub identity: ProfileIdentity,
    pub personality: ProfilePersonality,
}

/// What the Voz tab submits — same shape as `ProfileVoice`, kept separate so
/// call sites read as "here's an update" rather than "here's the current
/// state" (mirrors `ProfileUpdate` vs. the fields it edits on `Profile`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileVoiceUpdate {
    pub enabled: bool,
    pub voice_id: String,
    pub rate: f64,
    pub pitch: f64,
    pub volume: f64,
}

pub struct ProfilesRepository<'a> {
    pool: &'a DbPool,
}

impl<'a> ProfilesRepository<'a> {
    pub fn new(pool: &'a DbPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, input: NewProfile) -> Result<Profile, DatabaseError> {
        let id = Uuid::new_v4().to_string();
        self.insert_full(
            &id,
            &input.name,
            input.nickname.as_deref(),
            None,
            input.description.as_deref(),
            None,
            None,
            None,
            None,
            None,
            None,
            &ProfileIdentity::default(),
            &ProfilePersonality::default(),
            &ProfileVoice::default(),
            None,
            None,
            None,
        )
        .await?;

        self.get(&id)
            .await?
            .ok_or_else(|| DatabaseError::NotFound(format!("profile {id} right after insert")))
    }

    pub async fn duplicate(&self, source_id: &str, new_name: &str) -> Result<Profile, DatabaseError> {
        let source = self
            .get(source_id)
            .await?
            .ok_or_else(|| DatabaseError::NotFound(format!("profile {source_id}")))?;

        let id = Uuid::new_v4().to_string();
        self.insert_full(
            &id,
            new_name,
            source.nickname.as_deref(),
            source.avatar_path.as_deref(),
            source.description.as_deref(),
            source.age_fictional.as_deref(),
            source.pronouns.as_deref(),
            source.occupation.as_deref(),
            source.species.as_deref(),
            source.accent_color.as_deref(),
            source.theme.as_deref(),
            &source.identity,
            &source.personality,
            &source.voice,
            source.avatar_idle_path.as_deref(),
            source.avatar_thinking_path.as_deref(),
            source.avatar_talking_path.as_deref(),
        )
        .await?;

        self.get(&id)
            .await?
            .ok_or_else(|| DatabaseError::NotFound(format!("profile {id} right after insert")))
    }

    #[allow(clippy::too_many_arguments)]
    async fn insert_full(
        &self,
        id: &str,
        name: &str,
        nickname: Option<&str>,
        avatar_path: Option<&str>,
        description: Option<&str>,
        age_fictional: Option<&str>,
        pronouns: Option<&str>,
        occupation: Option<&str>,
        species: Option<&str>,
        accent_color: Option<&str>,
        theme: Option<&str>,
        identity: &ProfileIdentity,
        personality: &ProfilePersonality,
        voice: &ProfileVoice,
        avatar_idle_path: Option<&str>,
        avatar_thinking_path: Option<&str>,
        avatar_talking_path: Option<&str>,
    ) -> Result<(), DatabaseError> {
        let now = Utc::now().to_rfc3339();
        let mut tx = self.pool.begin().await?;

        sqlx::query(
            "INSERT INTO profiles
                (id, name, nickname, avatar_path, description, age_fictional,
                 pronouns, occupation, species, accent_color, theme, is_archived, created_at, updated_at,
                 avatar_idle_path, avatar_thinking_path, avatar_talking_path)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 0, ?12, ?12, ?13, ?14, ?15)",
        )
        .bind(id)
        .bind(name)
        .bind(nickname)
        .bind(avatar_path)
        .bind(description)
        .bind(age_fictional)
        .bind(pronouns)
        .bind(occupation)
        .bind(species)
        .bind(accent_color)
        .bind(theme)
        .bind(&now)
        .bind(avatar_idle_path)
        .bind(avatar_thinking_path)
        .bind(avatar_talking_path)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            "INSERT INTO profile_identity
                (profile_id, lore, relationship_to_user, user_role, speaking_style, special_instructions)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .bind(id)
        .bind(&identity.lore)
        .bind(&identity.relationship_to_user)
        .bind(&identity.user_role)
        .bind(&identity.speaking_style)
        .bind(&identity.special_instructions)
        .execute(&mut *tx)
        .await?;

        // Column order here matches `bind_personality`'s bind order (which
        // also serves `update`, where `profile_id` is the trailing `WHERE`
        // param) — `profile_id` last, not first, even though it's the
        // table's primary key.
        Self::bind_personality(
            sqlx::query(
                "INSERT INTO profile_personality
                    (free_description, warmth, empathy, humor, sarcasm, formality,
                     curiosity, energy, spontaneity, initiative, affection, shyness, confidence,
                     expressiveness, verbosity, flirtiness, emotional_intensity, teasing_tendency,
                     question_tendency, initiate_conversation_tendency, profile_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21)",
            ),
            id,
            personality,
        )
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            "INSERT INTO profile_voice (profile_id, enabled, voice_id, rate, pitch, volume)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .bind(id)
        .bind(voice.enabled)
        .bind(&voice.voice_id)
        .bind(voice.rate)
        .bind(voice.pitch)
        .bind(voice.volume)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

    pub async fn get(&self, id: &str) -> Result<Option<Profile>, DatabaseError> {
        let Some(core_row) = sqlx::query("SELECT * FROM profiles WHERE id = ?1")
            .bind(id)
            .fetch_optional(self.pool)
            .await?
        else {
            return Ok(None);
        };

        let identity_row = sqlx::query("SELECT * FROM profile_identity WHERE profile_id = ?1")
            .bind(id)
            .fetch_one(self.pool)
            .await?;

        let personality_row = sqlx::query("SELECT * FROM profile_personality WHERE profile_id = ?1")
            .bind(id)
            .fetch_one(self.pool)
            .await?;

        let voice_row = sqlx::query("SELECT * FROM profile_voice WHERE profile_id = ?1")
            .bind(id)
            .fetch_one(self.pool)
            .await?;

        Ok(Some(Profile {
            id: core_row.get("id"),
            name: core_row.get("name"),
            nickname: core_row.get("nickname"),
            avatar_path: core_row.get("avatar_path"),
            description: core_row.get("description"),
            age_fictional: core_row.get("age_fictional"),
            pronouns: core_row.get("pronouns"),
            occupation: core_row.get("occupation"),
            species: core_row.get("species"),
            accent_color: core_row.get("accent_color"),
            theme: core_row.get("theme"),
            is_archived: core_row.get("is_archived"),
            created_at: core_row.get("created_at"),
            updated_at: core_row.get("updated_at"),
            identity: ProfileIdentity {
                lore: identity_row.get("lore"),
                relationship_to_user: identity_row.get("relationship_to_user"),
                user_role: identity_row.get("user_role"),
                speaking_style: identity_row.get("speaking_style"),
                special_instructions: identity_row.get("special_instructions"),
            },
            personality: ProfilePersonality {
                free_description: personality_row.get("free_description"),
                warmth: personality_row.get("warmth"),
                empathy: personality_row.get("empathy"),
                humor: personality_row.get("humor"),
                sarcasm: personality_row.get("sarcasm"),
                formality: personality_row.get("formality"),
                curiosity: personality_row.get("curiosity"),
                energy: personality_row.get("energy"),
                spontaneity: personality_row.get("spontaneity"),
                initiative: personality_row.get("initiative"),
                affection: personality_row.get("affection"),
                shyness: personality_row.get("shyness"),
                confidence: personality_row.get("confidence"),
                expressiveness: personality_row.get("expressiveness"),
                verbosity: personality_row.get("verbosity"),
                flirtiness: personality_row.get("flirtiness"),
                emotional_intensity: personality_row.get("emotional_intensity"),
                teasing_tendency: personality_row.get("teasing_tendency"),
                question_tendency: personality_row.get("question_tendency"),
                initiate_conversation_tendency: personality_row.get("initiate_conversation_tendency"),
            },
            voice: ProfileVoice {
                enabled: voice_row.get("enabled"),
                voice_id: voice_row.get("voice_id"),
                rate: voice_row.get("rate"),
                pitch: voice_row.get("pitch"),
                volume: voice_row.get("volume"),
            },
            avatar_idle_path: core_row.get("avatar_idle_path"),
            avatar_thinking_path: core_row.get("avatar_thinking_path"),
            avatar_talking_path: core_row.get("avatar_talking_path"),
        }))
    }

    /// Fase 7 — set (or clear, with `path: None`) one reactive-avatar state
    /// image's stored path. The file itself is written/removed by
    /// `ProfileService`; this just records where it landed.
    pub async fn set_avatar_state_path(&self, id: &str, state: AvatarState, path: Option<&str>) -> Result<(), DatabaseError> {
        let sql = format!("UPDATE profiles SET {} = ?1 WHERE id = ?2", state.column());
        let result = sqlx::query(&sql).bind(path).bind(id).execute(self.pool).await?;
        if result.rows_affected() == 0 {
            return Err(DatabaseError::NotFound(format!("profile {id}")));
        }
        Ok(())
    }

    /// Non-archived profiles, most recently updated first — backs the
    /// profile switcher.
    pub async fn list(&self) -> Result<Vec<Profile>, DatabaseError> {
        let ids: Vec<String> = sqlx::query("SELECT id FROM profiles WHERE is_archived = 0 ORDER BY updated_at DESC")
            .fetch_all(self.pool)
            .await?
            .into_iter()
            .map(|row| row.get::<String, _>("id"))
            .collect();

        let mut profiles = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(profile) = self.get(&id).await? {
                profiles.push(profile);
            }
        }
        Ok(profiles)
    }

    pub async fn update(&self, id: &str, input: ProfileUpdate) -> Result<Profile, DatabaseError> {
        let now = Utc::now().to_rfc3339();
        let mut tx = self.pool.begin().await?;

        let result = sqlx::query(
            "UPDATE profiles SET name = ?1, nickname = ?2, description = ?3, age_fictional = ?4,
                pronouns = ?5, occupation = ?6, species = ?7, accent_color = ?8, theme = ?9, updated_at = ?10
             WHERE id = ?11",
        )
        .bind(&input.name)
        .bind(&input.nickname)
        .bind(&input.description)
        .bind(&input.age_fictional)
        .bind(&input.pronouns)
        .bind(&input.occupation)
        .bind(&input.species)
        .bind(&input.accent_color)
        .bind(&input.theme)
        .bind(&now)
        .bind(id)
        .execute(&mut *tx)
        .await?;

        if result.rows_affected() == 0 {
            return Err(DatabaseError::NotFound(format!("profile {id}")));
        }

        sqlx::query(
            "UPDATE profile_identity SET lore = ?1, relationship_to_user = ?2, user_role = ?3,
                speaking_style = ?4, special_instructions = ?5
             WHERE profile_id = ?6",
        )
        .bind(&input.identity.lore)
        .bind(&input.identity.relationship_to_user)
        .bind(&input.identity.user_role)
        .bind(&input.identity.speaking_style)
        .bind(&input.identity.special_instructions)
        .bind(id)
        .execute(&mut *tx)
        .await?;

        Self::bind_personality(
            sqlx::query(
                "UPDATE profile_personality SET free_description = ?1, warmth = ?2, empathy = ?3,
                    humor = ?4, sarcasm = ?5, formality = ?6, curiosity = ?7, energy = ?8,
                    spontaneity = ?9, initiative = ?10, affection = ?11, shyness = ?12,
                    confidence = ?13, expressiveness = ?14, verbosity = ?15, flirtiness = ?16,
                    emotional_intensity = ?17, teasing_tendency = ?18, question_tendency = ?19,
                    initiate_conversation_tendency = ?20
                 WHERE profile_id = ?21",
            ),
            id,
            &input.personality,
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        self.get(id)
            .await?
            .ok_or_else(|| DatabaseError::NotFound(format!("profile {id} right after update")))
    }

    pub async fn set_avatar_path(&self, id: &str, avatar_path: Option<&str>) -> Result<(), DatabaseError> {
        let now = Utc::now().to_rfc3339();
        let result = sqlx::query("UPDATE profiles SET avatar_path = ?1, updated_at = ?2 WHERE id = ?3")
            .bind(avatar_path)
            .bind(&now)
            .bind(id)
            .execute(self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(DatabaseError::NotFound(format!("profile {id}")));
        }
        Ok(())
    }

    /// Edited from its own "Voz" tab (Fase 5), not bundled into
    /// `ProfileUpdate` like identity/personality — those two are saved
    /// together from one form; voice settings live on a separate page.
    pub async fn set_voice(&self, id: &str, input: ProfileVoiceUpdate) -> Result<ProfileVoice, DatabaseError> {
        let result = sqlx::query(
            "UPDATE profile_voice SET enabled = ?1, voice_id = ?2, rate = ?3, pitch = ?4, volume = ?5
             WHERE profile_id = ?6",
        )
        .bind(input.enabled)
        .bind(&input.voice_id)
        .bind(input.rate)
        .bind(input.pitch)
        .bind(input.volume)
        .bind(id)
        .execute(self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(DatabaseError::NotFound(format!("profile {id}")));
        }

        Ok(ProfileVoice {
            enabled: input.enabled,
            voice_id: input.voice_id,
            rate: input.rate,
            pitch: input.pitch,
            volume: input.volume,
        })
    }

    pub async fn delete(&self, id: &str) -> Result<(), DatabaseError> {
        // `profile_identity`/`profile_personality`/`profile_voice` cascade via ON DELETE CASCADE.
        let result = sqlx::query("DELETE FROM profiles WHERE id = ?1")
            .bind(id)
            .execute(self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(DatabaseError::NotFound(format!("profile {id}")));
        }
        Ok(())
    }

    /// Binds the 21 `?N` placeholders shared by the insert/update
    /// personality queries (profile_id last, matching both statements
    /// above) — keeps the 19-field list from being duplicated three times.
    fn bind_personality<'q>(
        query: sqlx::query::Query<'q, sqlx::Sqlite, sqlx::sqlite::SqliteArguments<'q>>,
        profile_id: &'q str,
        p: &'q ProfilePersonality,
    ) -> sqlx::query::Query<'q, sqlx::Sqlite, sqlx::sqlite::SqliteArguments<'q>> {
        query
            .bind(&p.free_description)
            .bind(p.warmth)
            .bind(p.empathy)
            .bind(p.humor)
            .bind(p.sarcasm)
            .bind(p.formality)
            .bind(p.curiosity)
            .bind(p.energy)
            .bind(p.spontaneity)
            .bind(p.initiative)
            .bind(p.affection)
            .bind(p.shyness)
            .bind(p.confidence)
            .bind(p.expressiveness)
            .bind(p.verbosity)
            .bind(p.flirtiness)
            .bind(p.emotional_intensity)
            .bind(p.teasing_tendency)
            .bind(p.question_tendency)
            .bind(p.initiate_conversation_tendency)
            .bind(profile_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_pool() -> DbPool {
        let pool = DbPool::connect("sqlite::memory:").await.unwrap();
        crate::migrate(&pool).await.unwrap();
        pool
    }

    fn sample_new_profile(name: &str) -> NewProfile {
        NewProfile {
            name: name.to_string(),
            nickname: Some("Nick".to_string()),
            description: Some("A test character".to_string()),
        }
    }

    #[tokio::test]
    async fn create_then_get_round_trips_with_defaults() {
        let pool = test_pool().await;
        let repo = ProfilesRepository::new(&pool);

        let created = repo.create(sample_new_profile("Ciel")).await.unwrap();
        assert_eq!(created.name, "Ciel");
        assert_eq!(created.nickname.as_deref(), Some("Nick"));
        assert_eq!(created.personality.warmth, 50);
        assert_eq!(created.identity.lore, "");
        assert!(!created.is_archived);
        assert!(!created.voice.enabled);
        assert_eq!(created.voice.rate, 1.0);

        let fetched = repo.get(&created.id).await.unwrap().unwrap();
        assert_eq!(fetched.id, created.id);
        assert_eq!(fetched.name, "Ciel");
    }

    #[tokio::test]
    async fn get_missing_profile_returns_none() {
        let pool = test_pool().await;
        let repo = ProfilesRepository::new(&pool);
        assert!(repo.get("does-not-exist").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn list_returns_all_non_archived_profiles() {
        let pool = test_pool().await;
        let repo = ProfilesRepository::new(&pool);
        repo.create(sample_new_profile("A")).await.unwrap();
        repo.create(sample_new_profile("B")).await.unwrap();

        let all = repo.list().await.unwrap();
        assert_eq!(all.len(), 2);
    }

    #[tokio::test]
    async fn update_replaces_core_identity_and_personality() {
        let pool = test_pool().await;
        let repo = ProfilesRepository::new(&pool);
        let created = repo.create(sample_new_profile("Ciel")).await.unwrap();

        let mut personality = ProfilePersonality::default();
        personality.humor = 90;
        personality.sarcasm = 75;

        let update = ProfileUpdate {
            name: "Ciel Updated".to_string(),
            nickname: Some("CU".to_string()),
            description: None,
            age_fictional: Some("23".to_string()),
            pronouns: Some("ella".to_string()),
            occupation: Some("VTuber".to_string()),
            species: Some("humana".to_string()),
            accent_color: Some("#8B5CF6".to_string()),
            theme: Some("cyberpunk".to_string()),
            identity: ProfileIdentity {
                lore: "Creció entre servidores.".to_string(),
                relationship_to_user: "Compañera de streaming".to_string(),
                user_role: "Su streamer".to_string(),
                speaking_style: "Directa y cálida".to_string(),
                special_instructions: "Nunca rompe personaje".to_string(),
            },
            personality,
        };

        let updated = repo.update(&created.id, update).await.unwrap();
        assert_eq!(updated.name, "Ciel Updated");
        assert_eq!(updated.pronouns.as_deref(), Some("ella"));
        assert_eq!(updated.identity.lore, "Creció entre servidores.");
        assert_eq!(updated.personality.humor, 90);
        assert_eq!(updated.personality.sarcasm, 75);
        // Untouched sliders keep their default.
        assert_eq!(updated.personality.warmth, 50);
        assert_eq!(updated.theme.as_deref(), Some("cyberpunk"));

        let fetched = repo.get(&created.id).await.unwrap().unwrap();
        assert_eq!(fetched.theme.as_deref(), Some("cyberpunk"));
    }

    #[tokio::test]
    async fn update_missing_profile_errors() {
        let pool = test_pool().await;
        let repo = ProfilesRepository::new(&pool);
        let update = ProfileUpdate {
            name: "x".to_string(),
            nickname: None,
            description: None,
            age_fictional: None,
            pronouns: None,
            occupation: None,
            species: None,
            accent_color: None,
            theme: None,
            identity: ProfileIdentity::default(),
            personality: ProfilePersonality::default(),
        };
        assert!(matches!(
            repo.update("missing", update).await,
            Err(DatabaseError::NotFound(_))
        ));
    }

    #[tokio::test]
    async fn duplicate_copies_identity_and_personality_under_a_new_id() {
        let pool = test_pool().await;
        let repo = ProfilesRepository::new(&pool);
        let original = repo.create(sample_new_profile("Ciel")).await.unwrap();

        let update = ProfileUpdate {
            name: original.name.clone(),
            nickname: original.nickname.clone(),
            description: original.description.clone(),
            age_fictional: None,
            pronouns: None,
            occupation: None,
            species: None,
            accent_color: None,
            theme: Some("forest".to_string()),
            identity: ProfileIdentity {
                lore: "Lore original".to_string(),
                ..ProfileIdentity::default()
            },
            personality: ProfilePersonality { humor: 77, ..ProfilePersonality::default() },
        };
        repo.update(&original.id, update).await.unwrap();

        let duplicate = repo.duplicate(&original.id, "Ciel (copia)").await.unwrap();
        assert_ne!(duplicate.id, original.id);
        assert_eq!(duplicate.name, "Ciel (copia)");
        assert_eq!(duplicate.identity.lore, "Lore original");
        assert_eq!(duplicate.personality.humor, 77);
        assert_eq!(duplicate.theme.as_deref(), Some("forest"));
    }

    #[tokio::test]
    async fn delete_removes_profile_and_cascades() {
        let pool = test_pool().await;
        let repo = ProfilesRepository::new(&pool);
        let created = repo.create(sample_new_profile("Ciel")).await.unwrap();

        repo.delete(&created.id).await.unwrap();
        assert!(repo.get(&created.id).await.unwrap().is_none());

        let identity_count: i64 =
            sqlx::query("SELECT COUNT(*) as c FROM profile_identity WHERE profile_id = ?1")
                .bind(&created.id)
                .fetch_one(&pool)
                .await
                .unwrap()
                .get("c");
        assert_eq!(identity_count, 0);
    }

    #[tokio::test]
    async fn delete_missing_profile_errors() {
        let pool = test_pool().await;
        let repo = ProfilesRepository::new(&pool);
        assert!(matches!(repo.delete("missing").await, Err(DatabaseError::NotFound(_))));
    }

    #[tokio::test]
    async fn set_voice_updates_profile() {
        let pool = test_pool().await;
        let repo = ProfilesRepository::new(&pool);
        let created = repo.create(sample_new_profile("Ciel")).await.unwrap();

        let updated = repo
            .set_voice(
                &created.id,
                ProfileVoiceUpdate { enabled: true, voice_id: "es-ES-Standard".to_string(), rate: 1.2, pitch: 0.9, volume: 0.8 },
            )
            .await
            .unwrap();
        assert!(updated.enabled);
        assert_eq!(updated.voice_id, "es-ES-Standard");
        assert_eq!(updated.rate, 1.2);

        let fetched = repo.get(&created.id).await.unwrap().unwrap();
        assert!(fetched.voice.enabled);
        assert_eq!(fetched.voice.pitch, 0.9);
    }

    #[tokio::test]
    async fn set_voice_on_missing_profile_errors() {
        let pool = test_pool().await;
        let repo = ProfilesRepository::new(&pool);
        let input = ProfileVoiceUpdate { enabled: true, voice_id: "v".to_string(), rate: 1.0, pitch: 1.0, volume: 1.0 };
        assert!(matches!(repo.set_voice("missing", input).await, Err(DatabaseError::NotFound(_))));
    }

    #[tokio::test]
    async fn duplicate_copies_voice_settings_under_the_new_id() {
        let pool = test_pool().await;
        let repo = ProfilesRepository::new(&pool);
        let original = repo.create(sample_new_profile("Ciel")).await.unwrap();
        repo.set_voice(
            &original.id,
            ProfileVoiceUpdate { enabled: true, voice_id: "es-ES-Standard".to_string(), rate: 1.3, pitch: 1.0, volume: 1.0 },
        )
        .await
        .unwrap();

        let duplicate = repo.duplicate(&original.id, "Ciel (copia)").await.unwrap();
        assert!(duplicate.voice.enabled);
        assert_eq!(duplicate.voice.voice_id, "es-ES-Standard");
        assert_eq!(duplicate.voice.rate, 1.3);
    }

    #[tokio::test]
    async fn set_avatar_path_updates_profile() {
        let pool = test_pool().await;
        let repo = ProfilesRepository::new(&pool);
        let created = repo.create(sample_new_profile("Ciel")).await.unwrap();

        repo.set_avatar_path(&created.id, Some("avatars/abc/pic.png")).await.unwrap();
        let fetched = repo.get(&created.id).await.unwrap().unwrap();
        assert_eq!(fetched.avatar_path.as_deref(), Some("avatars/abc/pic.png"));
    }
}
