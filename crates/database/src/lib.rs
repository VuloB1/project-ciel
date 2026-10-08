//! SQLite access layer for AIVT Companion.
//!
//! Deliberately built on plain `sqlx` (no ORM) with a `Repository`-per-table
//! pattern — see docs/ADR/002-sqlite-storage.md for why. Queries use the
//! dynamic `sqlx::query`/`query_as` API rather than the `query!` compile-time
//! macros: the latter require a live database (or a checked-in `.sqlx`
//! offline cache) at `cargo build` time, which this workspace does not
//! assume. Once CI provisions a dev database, switching hot-path queries to
//! `query!` is a mechanical, low-risk follow-up.
//!
//! Schema changes only ever happen through a new file in `migrations/`,
//! applied via [`migrate`]. Never edit an already-released migration.

mod error;
pub mod pool;
pub mod repositories;

pub use error::DatabaseError;
pub use pool::{applied_schema_version, connect, inspect_backup_file, integrity_check, latest_migration_version, migrate, row_count, vacuum_into, DbPool};
pub use repositories::{
    ActivityEventRow, ActivityLevel, ActivityRepository, AppSettingsRepository, Attachment,
    AttachmentsRepository, CachedEmote, Conversation, ConversationsRepository, Memory, MemoriesRepository,
    MemoryEmbeddingsRepository, MemoryUpdate, Message, MessageRole, MessageStatus, MessagesRepository,
    MoodRepository, NewActivityEvent, AvatarState, NewAttachment, NewEmote, NewMemory, NewMessage, NewPerson,
    NewProfile, PeopleRepository, Person, Profile, ProfileIdentity, ProfileMood, ProfilePersonality,
    ProfileUpdate, ProfileVoice, ProfileVoiceUpdate, ProfilesRepository, MAX_ALIASES_PER_PERSON,
    LlmUsageRepository, LlmUsageSample, LlmUsageTotals, ProactiveSettings, ProactiveSettingsRepository, TwitchEmotesRepository, MEMORY_SCOPE_PRIVATE, MEMORY_SCOPE_PUBLIC,
};
