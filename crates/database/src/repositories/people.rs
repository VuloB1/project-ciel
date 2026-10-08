use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;

use crate::{DatabaseError, DbPool};

/// How many aliases a single person accumulates before the oldest gets
/// dropped to make room — enough to track a name's realistic variants (a
/// couple of nicknames, a display-name rename or two) without becoming an
/// unbounded list nobody reads. Shared across every caller of
/// `upsert_by_name`/`observe_twitch_chatter` (conversation extraction,
/// profile creation, Twitch) so a person's alias list behaves the same
/// regardless of which source is adding to it.
pub const MAX_ALIASES_PER_PERSON: usize = 5;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Person {
    pub id: String,
    pub canonical_name: String,
    pub aliases: Vec<String>,
    pub role: String,
    pub source: String,
    pub external_key: Option<String>,
    pub mention_count: i64,
    pub last_mentioned_at: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct NewPerson {
    pub canonical_name: String,
    pub role: String,
}

pub struct PeopleRepository<'a> {
    pool: &'a DbPool,
}

impl<'a> PeopleRepository<'a> {
    pub fn new(pool: &'a DbPool) -> Self {
        Self { pool }
    }

    /// Manual add from Configuración > Privacidad — always `source =
    /// "manual"`, no aliases yet.
    pub async fn create(&self, input: NewPerson) -> Result<Person, DatabaseError> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO people (id, canonical_name, aliases, role, source, external_key, mention_count, last_mentioned_at, created_at, updated_at)
             VALUES (?1, ?2, '[]', ?3, 'manual', NULL, 1, ?4, ?4, ?4)",
        )
        .bind(&id)
        .bind(&input.canonical_name)
        .bind(&input.role)
        .bind(&now)
        .execute(self.pool)
        .await?;
        self.get(&id).await?.ok_or_else(|| DatabaseError::NotFound(format!("person {id} right after insert")))
    }

    pub async fn get(&self, id: &str) -> Result<Option<Person>, DatabaseError> {
        let row = sqlx::query("SELECT * FROM people WHERE id = ?1").bind(id).fetch_optional(self.pool).await?;
        Ok(row.map(row_to_person))
    }

    /// Backs the management list in Configuración > Privacidad — most
    /// recently mentioned first, so "did it actually learn the friend I
    /// just mentioned" is immediately visible at the top.
    pub async fn list_all(&self) -> Result<Vec<Person>, DatabaseError> {
        let rows = sqlx::query("SELECT * FROM people ORDER BY last_mentioned_at DESC").fetch_all(self.pool).await?;
        Ok(rows.into_iter().map(row_to_person).collect())
    }

    pub async fn delete(&self, id: &str) -> Result<(), DatabaseError> {
        let result = sqlx::query("DELETE FROM people WHERE id = ?1").bind(id).execute(self.pool).await?;
        require_row(result.rows_affected(), id)
    }

    /// Curated list for `CallService::transcription_prompt` — self/nickname/
    /// ai always outrank friend/twitch_chatter, and a `twitch_chatter`
    /// needs at least 2 sightings to qualify (filters out one-off drive-by
    /// viewers; the other roles have no such floor since they're low-volume
    /// by nature and still worth the boost from a single mention). `ai`
    /// ranks right after the user's own name/nickname — recognizing other
    /// AI characters' names by voice matters as much as the user's own,
    /// and unlike the *active* profile (already primed unconditionally,
    /// see `CallService::transcription_prompt`), this is what covers a
    /// character the user created but isn't currently talking *as*.
    pub async fn top_for_prompt(&self, limit: i64) -> Result<Vec<Person>, DatabaseError> {
        let rows = sqlx::query(
            "SELECT * FROM people
             WHERE role NOT IN ('twitch_chatter', 'discord_chatter') OR mention_count >= 2
             ORDER BY CASE role WHEN 'self' THEN 0 WHEN 'nickname' THEN 1 WHEN 'ai' THEN 2 WHEN 'friend' THEN 3 ELSE 4 END,
                      last_mentioned_at DESC
             LIMIT ?1",
        )
        .bind(limit)
        .fetch_all(self.pool)
        .await?;
        Ok(rows.into_iter().map(row_to_person).collect())
    }

    /// Upserts a person matched by case-insensitive name — used by
    /// conversation extraction (`crates/memory`, `source = "chat"`, `role`
    /// guessed by the model) and by profile creation
    /// (`crates/services/profiles`, `source = "profile"`, `role = "ai"`
    /// always). Matches an existing row among rows that aren't
    /// Twitch-derived (`external_key IS NULL`) — a Twitch chatter and a
    /// same-named person mentioned elsewhere are kept as separate rows
    /// deliberately (see the migration's comment on why there's no unique
    /// constraint on `canonical_name`). On match: bumps `mention_count`/
    /// `last_mentioned_at`, merges in `alias` if new (capped at
    /// `max_aliases`, drops the oldest to make room), and — only if the
    /// existing row's `role` wasn't already trusted (i.e. its `source`
    /// isn't the lowest-confidence `"chat"`) — lets a newer, more
    /// authoritative `role` correct an earlier guess. A manually-curated
    /// or profile-derived entry's `role` is never silently overwritten by
    /// a later chat-extraction guess, or by a same-named collision from a
    /// different source.
    pub async fn upsert_by_name(
        &self,
        name: &str,
        alias: Option<&str>,
        role: &str,
        source: &str,
        max_aliases: usize,
    ) -> Result<Person, DatabaseError> {
        let existing = sqlx::query("SELECT * FROM people WHERE external_key IS NULL AND LOWER(canonical_name) = LOWER(?1)")
            .bind(name)
            .fetch_optional(self.pool)
            .await?
            .map(row_to_person);

        let now = Utc::now().to_rfc3339();
        match existing {
            Some(person) => {
                let aliases = merge_alias(&person.aliases, alias, max_aliases);
                let next_role = if person.source != "chat" { person.role.clone() } else { role.to_string() };
                sqlx::query(
                    "UPDATE people SET aliases = ?1, role = ?2, mention_count = mention_count + 1, last_mentioned_at = ?3, updated_at = ?3 WHERE id = ?4",
                )
                .bind(serde_json::to_string(&aliases).expect("aliases are always serializable"))
                .bind(&next_role)
                .bind(&now)
                .bind(&person.id)
                .execute(self.pool)
                .await?;
                self.get(&person.id).await?.ok_or_else(|| DatabaseError::NotFound(format!("person {}", person.id)))
            }
            None => {
                let id = Uuid::new_v4().to_string();
                let aliases = merge_alias(&[], alias, max_aliases);
                sqlx::query(
                    "INSERT INTO people (id, canonical_name, aliases, role, source, external_key, mention_count, last_mentioned_at, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, NULL, 1, ?6, ?6, ?6)",
                )
                .bind(&id)
                .bind(name)
                .bind(serde_json::to_string(&aliases).expect("aliases are always serializable"))
                .bind(role)
                .bind(source)
                .bind(&now)
                .execute(self.pool)
                .await?;
                self.get(&id).await?.ok_or_else(|| DatabaseError::NotFound(format!("person {id} right after insert")))
            }
        }
    }

    /// Upserts a person matched by a *stable* external identity — for
    /// entities that have one, unlike the free-text names
    /// `upsert_by_name` matches on. Two callers: `TwitchService`
    /// (`external_key = "twitch:<chatter_user_id>"`, keyed on the numeric
    /// id rather than login/display name so a rename doesn't orphan the
    /// row — see `observe_twitch_chatter`) and `ProfileService`
    /// (`external_key = "profile:<profile_id>"`, so editing a character's
    /// name — e.g. correcting the "Nuevo personaje" placeholder a
    /// freshly-created profile starts with — updates the *same* row
    /// instead of leaving that placeholder stuck in the registry forever
    /// while a second row appears under the real name; a live-testing
    /// checkpoint hit exactly this). On match: updates `canonical_name`/
    /// `role` unconditionally (both callers always pass the same value
    /// for their own identity's whole lifetime, so there's no
    /// cross-source collision to protect against the way `upsert_by_name`
    /// has to guard for), merges in `alias` if new (capped at
    /// `max_aliases`), bumps `mention_count`/`last_mentioned_at`. Because
    /// external-key rows are never matched by `upsert_by_name` (which only
    /// looks at `external_key IS NULL` rows), a Twitch chatter or AI
    /// character never gets silently merged into a same-named
    /// conversation-extracted "friend" row, or vice versa.
    pub async fn upsert_by_external_key(
        &self,
        external_key: &str,
        canonical_name: &str,
        alias: Option<&str>,
        role: &str,
        source: &str,
        max_aliases: usize,
    ) -> Result<Person, DatabaseError> {
        let existing = sqlx::query("SELECT * FROM people WHERE external_key = ?1")
            .bind(external_key)
            .fetch_optional(self.pool)
            .await?
            .map(row_to_person);

        let now = Utc::now().to_rfc3339();
        match existing {
            Some(person) => {
                let aliases = merge_alias(&person.aliases, alias, max_aliases);
                sqlx::query(
                    "UPDATE people SET canonical_name = ?1, aliases = ?2, role = ?3, mention_count = mention_count + 1, last_mentioned_at = ?4, updated_at = ?4 WHERE id = ?5",
                )
                .bind(canonical_name)
                .bind(serde_json::to_string(&aliases).expect("aliases are always serializable"))
                .bind(role)
                .bind(&now)
                .bind(&person.id)
                .execute(self.pool)
                .await?;
                self.get(&person.id).await?.ok_or_else(|| DatabaseError::NotFound(format!("person {}", person.id)))
            }
            None => {
                let id = Uuid::new_v4().to_string();
                let aliases = merge_alias(&[], alias, max_aliases);
                sqlx::query(
                    "INSERT INTO people (id, canonical_name, aliases, role, source, external_key, mention_count, last_mentioned_at, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, ?7, ?7, ?7)",
                )
                .bind(&id)
                .bind(canonical_name)
                .bind(serde_json::to_string(&aliases).expect("aliases are always serializable"))
                .bind(role)
                .bind(source)
                .bind(external_key)
                .bind(&now)
                .execute(self.pool)
                .await?;
                self.get(&id).await?.ok_or_else(|| DatabaseError::NotFound(format!("person {id} right after insert")))
            }
        }
    }

    /// Called from `TwitchService` for every real chat message. Dedups by
    /// `external_key` (the chatter's numeric user id, not their login/
    /// display name, which can change) — if their display name has
    /// drifted since last seen (capitalization, a rename), the old name is
    /// kept as an alias rather than lost (a Twitch-specific courtesy, not
    /// part of `upsert_by_external_key` itself — a profile rename, by
    /// contrast, usually corrects a meaningless placeholder and shouldn't
    /// keep it around as a "known alias").
    pub async fn observe_twitch_chatter(&self, user_id: &str, display_name: &str, max_aliases: usize) -> Result<(), DatabaseError> {
        let external_key = format!("twitch:{user_id}");
        let previous_name: Option<String> =
            sqlx::query_scalar("SELECT canonical_name FROM people WHERE external_key = ?1").bind(&external_key).fetch_optional(self.pool).await?;
        let alias = previous_name.as_deref().filter(|old| *old != display_name);

        self.upsert_by_external_key(&external_key, display_name, alias, "twitch_chatter", "twitch", max_aliases).await?;
        Ok(())
    }

    /// Called from `DiscordService` for every real message from a human
    /// author (bot messages, including this bot's own replies, are already
    /// filtered out before this is reached). Mirrors `observe_twitch_chatter`
    /// exactly, keyed by the author's stable Discord snowflake id instead of
    /// a Twitch numeric user id — same rename-as-alias courtesy applies.
    pub async fn observe_discord_author(&self, author_id: &str, display_name: &str, max_aliases: usize) -> Result<(), DatabaseError> {
        let external_key = format!("discord:{author_id}");
        let previous_name: Option<String> =
            sqlx::query_scalar("SELECT canonical_name FROM people WHERE external_key = ?1").bind(&external_key).fetch_optional(self.pool).await?;
        let alias = previous_name.as_deref().filter(|old| *old != display_name);

        self.upsert_by_external_key(&external_key, display_name, alias, "discord_chatter", "discord", max_aliases).await?;
        Ok(())
    }

    /// Best-effort cleanup when the owning entity is deleted (a Twitch
    /// disconnect currently never calls this — chatter history staying
    /// around after a disconnect is reasonable; `ProfileService::delete`
    /// does). A no-op, not an error, when nothing matches — the person row
    /// might never have been created (e.g. the initial registration
    /// failed) or the user already deleted it by hand.
    pub async fn delete_by_external_key(&self, external_key: &str) -> Result<(), DatabaseError> {
        sqlx::query("DELETE FROM people WHERE external_key = ?1").bind(external_key).execute(self.pool).await?;
        Ok(())
    }
}

/// Adds `new_alias` to `existing` if it's non-empty and not already present
/// (case-insensitively), dropping the oldest alias first if that would
/// exceed `max_aliases` — a name shouldn't accumulate an unbounded list of
/// half-remembered spelling variants.
fn merge_alias(existing: &[String], new_alias: Option<&str>, max_aliases: usize) -> Vec<String> {
    let mut aliases = existing.to_vec();
    if let Some(alias) = new_alias.map(str::trim).filter(|a| !a.is_empty()) {
        if !aliases.iter().any(|a| a.eq_ignore_ascii_case(alias)) {
            aliases.push(alias.to_string());
            if aliases.len() > max_aliases {
                aliases.remove(0);
            }
        }
    }
    aliases
}

fn require_row(rows_affected: u64, id: &str) -> Result<(), DatabaseError> {
    if rows_affected == 0 {
        Err(DatabaseError::NotFound(format!("person {id}")))
    } else {
        Ok(())
    }
}

fn row_to_person(row: sqlx::sqlite::SqliteRow) -> Person {
    let aliases_json: String = row.get("aliases");
    Person {
        id: row.get("id"),
        canonical_name: row.get("canonical_name"),
        aliases: serde_json::from_str(&aliases_json).unwrap_or_default(),
        role: row.get("role"),
        source: row.get("source"),
        external_key: row.get("external_key"),
        mention_count: row.get("mention_count"),
        last_mentioned_at: row.get("last_mentioned_at"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
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

    #[tokio::test]
    async fn create_then_get_round_trips() {
        let pool = test_pool().await;
        let repo = PeopleRepository::new(&pool);

        let created = repo.create(NewPerson { canonical_name: "Kenji".to_string(), role: "friend".to_string() }).await.unwrap();
        assert_eq!(created.canonical_name, "Kenji");
        assert_eq!(created.role, "friend");
        assert_eq!(created.source, "manual");
        assert_eq!(created.mention_count, 1);
        assert!(created.aliases.is_empty());

        let fetched = repo.get(&created.id).await.unwrap().unwrap();
        assert_eq!(fetched.id, created.id);
    }

    #[tokio::test]
    async fn upsert_by_name_creates_a_new_person_on_first_mention() {
        let pool = test_pool().await;
        let repo = PeopleRepository::new(&pool);

        let person = repo.upsert_by_name("Ryūji", None, "friend", "chat", 5).await.unwrap();
        assert_eq!(person.canonical_name, "Ryūji");
        assert_eq!(person.source, "chat");
        assert_eq!(person.mention_count, 1);
    }

    #[tokio::test]
    async fn upsert_by_name_matches_case_insensitively_and_bumps_mentions() {
        let pool = test_pool().await;
        let repo = PeopleRepository::new(&pool);

        repo.upsert_by_name("Kenji", None, "friend", "chat", 5).await.unwrap();
        let second = repo.upsert_by_name("kenji", Some("Ken"), "friend", "chat", 5).await.unwrap();

        let all = repo.list_all().await.unwrap();
        assert_eq!(all.len(), 1, "should not create a second row for the same name");
        assert_eq!(second.mention_count, 2);
        assert_eq!(second.aliases, vec!["Ken".to_string()]);
    }

    #[tokio::test]
    async fn upsert_by_name_never_overwrites_a_manual_entrys_role() {
        let pool = test_pool().await;
        let repo = PeopleRepository::new(&pool);

        repo.create(NewPerson { canonical_name: "María".to_string(), role: "friend".to_string() }).await.unwrap();
        let updated = repo.upsert_by_name("María", None, "self", "chat", 5).await.unwrap();

        assert_eq!(updated.role, "friend", "extraction guessed 'self' but the manual entry's role must be preserved");
    }

    #[tokio::test]
    async fn upsert_by_name_lets_a_later_call_correct_a_chat_sourced_role() {
        let pool = test_pool().await;
        let repo = PeopleRepository::new(&pool);

        repo.upsert_by_name("Ana", None, "friend", "chat", 5).await.unwrap();
        let corrected = repo.upsert_by_name("Ana", None, "self", "chat", 5).await.unwrap();

        assert_eq!(corrected.role, "self", "a chat-sourced entry's role can be corrected by later extraction");
    }

    #[tokio::test]
    async fn profile_registration_creates_an_ai_person_that_ranks_above_friends() {
        let pool = test_pool().await;
        let repo = PeopleRepository::new(&pool);

        let ai = repo.upsert_by_external_key("profile:1", "Ciel", Some("Luciel"), "ai", "profile", 5).await.unwrap();
        assert_eq!(ai.role, "ai");
        assert_eq!(ai.source, "profile");
        assert_eq!(ai.aliases, vec!["Luciel".to_string()]);

        repo.upsert_by_name("Amiga", None, "friend", "chat", 5).await.unwrap();
        let top = repo.top_for_prompt(10).await.unwrap();
        assert_eq!(top.iter().map(|p| p.canonical_name.as_str()).collect::<Vec<_>>(), vec!["Ciel", "Amiga"]);
    }

    #[tokio::test]
    async fn renaming_a_profile_updates_the_same_person_instead_of_leaving_a_stale_row() {
        // Regression: a freshly-created profile starts out named "Nuevo
        // personaje" and gets renamed once the user actually fills in the
        // form — the placeholder must not linger as an orphaned entry
        // once the real name is saved (pedido del usuario, encontrado en
        // vivo: "se guarda directamente el nombre al momento de crear el
        // perfil... debería guardar cuando se guarda el perfil").
        let pool = test_pool().await;
        let repo = PeopleRepository::new(&pool);

        let created = repo.upsert_by_external_key("profile:1", "Nuevo personaje", None, "ai", "profile", 5).await.unwrap();
        let renamed = repo.upsert_by_external_key("profile:1", "Ciel", Some("Luciel"), "ai", "profile", 5).await.unwrap();

        assert_eq!(renamed.id, created.id, "same underlying row, not a new one");
        assert_eq!(renamed.canonical_name, "Ciel");
        let all = repo.list_all().await.unwrap();
        assert_eq!(all.len(), 1, "the 'Nuevo personaje' placeholder must not linger as a separate row");
    }

    #[tokio::test]
    async fn an_ai_person_and_a_same_named_chat_mention_stay_separate_rows() {
        let pool = test_pool().await;
        let repo = PeopleRepository::new(&pool);

        repo.upsert_by_external_key("profile:1", "Ciel", None, "ai", "profile", 5).await.unwrap();
        repo.upsert_by_name("Ciel", None, "friend", "chat", 5).await.unwrap();

        let all = repo.list_all().await.unwrap();
        assert_eq!(all.len(), 2, "an AI character's registered name and an unrelated same-named chat mention are different entities");
    }

    #[tokio::test]
    async fn delete_by_external_key_removes_the_matching_person() {
        let pool = test_pool().await;
        let repo = PeopleRepository::new(&pool);
        repo.upsert_by_external_key("profile:1", "Ciel", None, "ai", "profile", 5).await.unwrap();

        repo.delete_by_external_key("profile:1").await.unwrap();

        assert!(repo.list_all().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn delete_by_external_key_is_a_no_op_when_nothing_matches() {
        let pool = test_pool().await;
        let repo = PeopleRepository::new(&pool);
        assert!(repo.delete_by_external_key("profile:missing").await.is_ok());
    }

    #[tokio::test]
    async fn observe_twitch_chatter_creates_a_new_row_keyed_by_user_id() {
        let pool = test_pool().await;
        let repo = PeopleRepository::new(&pool);

        repo.observe_twitch_chatter("123", "SomeViewer", 5).await.unwrap();

        let all = repo.list_all().await.unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].canonical_name, "SomeViewer");
        assert_eq!(all[0].role, "twitch_chatter");
        assert_eq!(all[0].source, "twitch");
        assert_eq!(all[0].external_key.as_deref(), Some("twitch:123"));
    }

    #[tokio::test]
    async fn observe_twitch_chatter_bumps_mentions_without_duplicating() {
        let pool = test_pool().await;
        let repo = PeopleRepository::new(&pool);

        repo.observe_twitch_chatter("123", "SomeViewer", 5).await.unwrap();
        repo.observe_twitch_chatter("123", "SomeViewer", 5).await.unwrap();
        repo.observe_twitch_chatter("123", "SomeViewer", 5).await.unwrap();

        let all = repo.list_all().await.unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].mention_count, 3);
    }

    #[tokio::test]
    async fn observe_twitch_chatter_tracks_a_rename_as_an_alias() {
        let pool = test_pool().await;
        let repo = PeopleRepository::new(&pool);

        repo.observe_twitch_chatter("123", "OldName", 5).await.unwrap();
        repo.observe_twitch_chatter("123", "NewName", 5).await.unwrap();

        let all = repo.list_all().await.unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].canonical_name, "NewName");
        assert_eq!(all[0].aliases, vec!["OldName".to_string()]);
    }

    #[tokio::test]
    async fn observe_discord_author_creates_a_new_row_keyed_by_author_id() {
        let pool = test_pool().await;
        let repo = PeopleRepository::new(&pool);

        repo.observe_discord_author("456", "SomeMember", 5).await.unwrap();

        let all = repo.list_all().await.unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].canonical_name, "SomeMember");
        assert_eq!(all[0].role, "discord_chatter");
        assert_eq!(all[0].source, "discord");
        assert_eq!(all[0].external_key.as_deref(), Some("discord:456"));
    }

    #[tokio::test]
    async fn observe_discord_author_bumps_mentions_without_duplicating() {
        let pool = test_pool().await;
        let repo = PeopleRepository::new(&pool);

        repo.observe_discord_author("456", "SomeMember", 5).await.unwrap();
        repo.observe_discord_author("456", "SomeMember", 5).await.unwrap();
        repo.observe_discord_author("456", "SomeMember", 5).await.unwrap();

        let all = repo.list_all().await.unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].mention_count, 3);
    }

    #[tokio::test]
    async fn observe_discord_author_tracks_a_rename_as_an_alias() {
        let pool = test_pool().await;
        let repo = PeopleRepository::new(&pool);

        repo.observe_discord_author("456", "OldName", 5).await.unwrap();
        repo.observe_discord_author("456", "NewName", 5).await.unwrap();

        let all = repo.list_all().await.unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].canonical_name, "NewName");
        assert_eq!(all[0].aliases, vec!["OldName".to_string()]);
    }

    #[tokio::test]
    async fn a_discord_chatter_and_a_twitch_chatter_with_the_same_name_stay_separate() {
        let pool = test_pool().await;
        let repo = PeopleRepository::new(&pool);

        repo.observe_discord_author("456", "Ale", 5).await.unwrap();
        repo.observe_twitch_chatter("123", "Ale", 5).await.unwrap();

        let all = repo.list_all().await.unwrap();
        assert_eq!(all.len(), 2, "a discord chatter and a twitch chatter are different external keys even with the same display name");
    }

    #[tokio::test]
    async fn top_for_prompt_excludes_a_discord_chatter_seen_only_once() {
        let pool = test_pool().await;
        let repo = PeopleRepository::new(&pool);

        repo.observe_discord_author("456", "DriveBy", 5).await.unwrap();

        let top = repo.top_for_prompt(10).await.unwrap();
        assert!(top.is_empty(), "a one-off discord author shouldn't qualify for the prompt, same rule as a one-off twitch chatter");
    }

    #[tokio::test]
    async fn a_twitch_chatter_and_a_chat_mentioned_person_with_the_same_name_stay_separate() {
        let pool = test_pool().await;
        let repo = PeopleRepository::new(&pool);

        repo.observe_twitch_chatter("123", "Ale", 5).await.unwrap();
        repo.upsert_by_name("Ale", None, "friend", "chat", 5).await.unwrap();

        let all = repo.list_all().await.unwrap();
        assert_eq!(all.len(), 2, "a twitch chatter and a same-named person mentioned in chat may be different people");
    }

    #[tokio::test]
    async fn top_for_prompt_ranks_self_and_friend_above_twitch_chatters() {
        let pool = test_pool().await;
        let repo = PeopleRepository::new(&pool);

        repo.observe_twitch_chatter("1", "Chatter", 5).await.unwrap();
        repo.observe_twitch_chatter("1", "Chatter", 5).await.unwrap(); // 2 mentions, qualifies
        repo.upsert_by_name("Amiga", None, "friend", "chat", 5).await.unwrap();
        repo.upsert_by_name("YoMismo", None, "self", "chat", 5).await.unwrap();

        let top = repo.top_for_prompt(10).await.unwrap();
        assert_eq!(top.iter().map(|p| p.canonical_name.as_str()).collect::<Vec<_>>(), vec!["YoMismo", "Amiga", "Chatter"]);
    }

    #[tokio::test]
    async fn top_for_prompt_excludes_a_twitch_chatter_seen_only_once() {
        let pool = test_pool().await;
        let repo = PeopleRepository::new(&pool);

        repo.observe_twitch_chatter("1", "DriveBy", 5).await.unwrap();

        let top = repo.top_for_prompt(10).await.unwrap();
        assert!(top.is_empty(), "a one-off chatter shouldn't qualify for the prompt");
    }

    #[tokio::test]
    async fn top_for_prompt_respects_the_limit() {
        let pool = test_pool().await;
        let repo = PeopleRepository::new(&pool);
        for i in 0..5 {
            repo.upsert_by_name(&format!("Persona{i}"), None, "friend", "chat", 5).await.unwrap();
        }

        let top = repo.top_for_prompt(3).await.unwrap();
        assert_eq!(top.len(), 3);
    }

    #[tokio::test]
    async fn merge_alias_drops_the_oldest_once_the_cap_is_reached() {
        let pool = test_pool().await;
        let repo = PeopleRepository::new(&pool);
        repo.upsert_by_name("Diego", Some("D"), "friend", "chat", 2).await.unwrap();
        repo.upsert_by_name("Diego", Some("Dieguito"), "friend", "chat", 2).await.unwrap();
        let updated = repo.upsert_by_name("Diego", Some("Dieguín"), "friend", "chat", 2).await.unwrap();

        assert_eq!(updated.aliases.len(), 2, "capped at max_aliases");
        assert_eq!(updated.aliases, vec!["Dieguito".to_string(), "Dieguín".to_string()], "oldest alias dropped first");
    }

    #[tokio::test]
    async fn delete_removes_the_person() {
        let pool = test_pool().await;
        let repo = PeopleRepository::new(&pool);
        let created = repo.create(NewPerson { canonical_name: "Temporal".to_string(), role: "friend".to_string() }).await.unwrap();

        repo.delete(&created.id).await.unwrap();

        assert!(repo.get(&created.id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn delete_on_a_missing_person_errors() {
        let pool = test_pool().await;
        let repo = PeopleRepository::new(&pool);
        assert!(matches!(repo.delete("missing").await, Err(DatabaseError::NotFound(_))));
    }
}
