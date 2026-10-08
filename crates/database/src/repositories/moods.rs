use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::Row;

use crate::{DatabaseError, DbPool};

/// A character's evolving emotional state — distinct from
/// `ProfilePersonality`, which is static configuration (same input,
/// same rendered prompt, every time). `valence`/`energy` are both 0-100
/// on the same scale/convention as personality's sliders; 50/50 is the
/// neutral baseline. See `docs/ADR/006-mood.md`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileMood {
    pub profile_id: String,
    pub valence: i64,
    pub energy: i64,
    pub updated_at: String,
}

impl ProfileMood {
    pub fn baseline(profile_id: &str) -> Self {
        Self { profile_id: profile_id.to_string(), valence: 50, energy: 50, updated_at: Utc::now().to_rfc3339() }
    }
}

pub struct MoodRepository<'a> {
    pool: &'a DbPool,
}

impl<'a> MoodRepository<'a> {
    pub fn new(pool: &'a DbPool) -> Self {
        Self { pool }
    }

    /// A missing row means "never updated" — returns the neutral
    /// baseline rather than requiring a row to be seeded when a profile
    /// is created.
    pub async fn get(&self, profile_id: &str) -> Result<ProfileMood, DatabaseError> {
        let row = sqlx::query("SELECT profile_id, valence, energy, updated_at FROM profile_moods WHERE profile_id = ?1")
            .bind(profile_id)
            .fetch_optional(self.pool)
            .await?;
        Ok(match row {
            Some(row) => ProfileMood {
                profile_id: row.get("profile_id"),
                valence: row.get("valence"),
                energy: row.get("energy"),
                updated_at: row.get("updated_at"),
            },
            None => ProfileMood::baseline(profile_id),
        })
    }

    /// Applies decay toward the 50/50 baseline based on real time elapsed
    /// since the last update, *then* adds the delta — so even a "nothing
    /// notable happened" delta of (0, 0) still lets the mood settle
    /// toward neutral over time, without needing a running timer
    /// anywhere: elapsed time is computed lazily from `updated_at` each
    /// time this is called.
    pub async fn apply_delta(&self, profile_id: &str, valence_delta: i64, energy_delta: i64) -> Result<ProfileMood, DatabaseError> {
        let current = self.get(profile_id).await?;
        let hours = hours_since(&current.updated_at);
        let valence = (decay_toward_baseline(current.valence, hours) + valence_delta).clamp(0, 100);
        let energy = (decay_toward_baseline(current.energy, hours) + energy_delta).clamp(0, 100);
        let updated_at = Utc::now().to_rfc3339();

        sqlx::query(
            "INSERT INTO profile_moods (profile_id, valence, energy, updated_at) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(profile_id) DO UPDATE SET valence = excluded.valence, energy = excluded.energy, updated_at = excluded.updated_at",
        )
        .bind(profile_id)
        .bind(valence)
        .bind(energy)
        .bind(&updated_at)
        .execute(self.pool)
        .await?;

        Ok(ProfileMood { profile_id: profile_id.to_string(), valence, energy, updated_at })
    }

    /// Manual "reset" — deletes the row rather than writing an explicit
    /// 50/50, so it stays consistent with "an absent row is neutral".
    pub async fn reset(&self, profile_id: &str) -> Result<ProfileMood, DatabaseError> {
        sqlx::query("DELETE FROM profile_moods WHERE profile_id = ?1").bind(profile_id).execute(self.pool).await?;
        Ok(ProfileMood::baseline(profile_id))
    }
}

/// ~2 points/hour — an extreme mood (0 or 100) fully settles back to
/// baseline in about a day of no interaction. Tunable.
const DECAY_PER_HOUR: f64 = 2.0;
const BASELINE: i64 = 50;

fn decay_toward_baseline(value: i64, hours_elapsed: f64) -> i64 {
    if hours_elapsed <= 0.0 || value == BASELINE {
        return value;
    }
    let max_pull = (value - BASELINE).unsigned_abs() as f64;
    let pull = (hours_elapsed * DECAY_PER_HOUR).min(max_pull);
    if value > BASELINE {
        (value as f64 - pull).round() as i64
    } else {
        (value as f64 + pull).round() as i64
    }
}

fn hours_since(rfc3339: &str) -> f64 {
    chrono::DateTime::parse_from_rfc3339(rfc3339)
        .map(|then| (Utc::now() - then.with_timezone(&Utc)).num_seconds() as f64 / 3600.0)
        .unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_pool() -> DbPool {
        let pool = DbPool::connect("sqlite::memory:").await.unwrap();
        crate::migrate(&pool).await.unwrap();
        pool
    }

    async fn seed_profile(pool: &DbPool) -> String {
        let profiles = crate::ProfilesRepository::new(pool);
        let profile = profiles
            .create(crate::NewProfile { name: "Ciel".to_string(), nickname: None, description: None })
            .await
            .unwrap();
        profile.id
    }

    #[test]
    fn decay_toward_baseline_does_nothing_with_no_time_elapsed() {
        assert_eq!(decay_toward_baseline(80, 0.0), 80);
        assert_eq!(decay_toward_baseline(20, 0.0), 20);
    }

    #[test]
    fn decay_toward_baseline_reaches_exactly_fifty_without_overshooting() {
        assert_eq!(decay_toward_baseline(80, 1000.0), 50);
        assert_eq!(decay_toward_baseline(20, 1000.0), 50);
    }

    #[test]
    fn decay_toward_baseline_is_symmetric_above_and_below_fifty() {
        // 5 hours * 2.0/hour = 10 points pulled toward 50.
        assert_eq!(decay_toward_baseline(80, 5.0), 70);
        assert_eq!(decay_toward_baseline(20, 5.0), 30);
    }

    #[test]
    fn decay_toward_baseline_leaves_the_baseline_alone() {
        assert_eq!(decay_toward_baseline(50, 100.0), 50);
    }

    #[tokio::test]
    async fn get_returns_the_neutral_baseline_when_no_row_exists() {
        let pool = test_pool().await;
        let profile_id = seed_profile(&pool).await;
        let mood = MoodRepository::new(&pool).get(&profile_id).await.unwrap();
        assert_eq!(mood.valence, 50);
        assert_eq!(mood.energy, 50);
    }

    #[tokio::test]
    async fn apply_delta_clamps_to_the_zero_to_hundred_range() {
        let pool = test_pool().await;
        let profile_id = seed_profile(&pool).await;
        let repo = MoodRepository::new(&pool);

        let mood = repo.apply_delta(&profile_id, 1000, -1000).await.unwrap();

        assert_eq!(mood.valence, 100);
        assert_eq!(mood.energy, 0);
    }

    #[tokio::test]
    async fn apply_delta_applies_decay_before_the_new_delta() {
        let pool = test_pool().await;
        let profile_id = seed_profile(&pool).await;

        // Insert a row by hand with an `updated_at` far in the past —
        // full control over elapsed time without mocking the clock.
        let old_timestamp = (Utc::now() - chrono::Duration::hours(1000)).to_rfc3339();
        sqlx::query("INSERT INTO profile_moods (profile_id, valence, energy, updated_at) VALUES (?1, 90, 90, ?2)")
            .bind(&profile_id)
            .bind(&old_timestamp)
            .execute(&pool)
            .await
            .unwrap();

        // Delta of 0,0 — any change observed is purely from decay.
        let mood = MoodRepository::new(&pool).apply_delta(&profile_id, 0, 0).await.unwrap();

        assert_eq!(mood.valence, 50, "1000 horas es más que de sobra para asentarse del todo");
        assert_eq!(mood.energy, 50);
    }

    #[tokio::test]
    async fn reset_removes_the_row_and_returns_baseline() {
        let pool = test_pool().await;
        let profile_id = seed_profile(&pool).await;
        let repo = MoodRepository::new(&pool);
        repo.apply_delta(&profile_id, 20, 20).await.unwrap();

        let mood = repo.reset(&profile_id).await.unwrap();

        assert_eq!(mood.valence, 50);
        assert_eq!(mood.energy, 50);
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM profile_moods WHERE profile_id = ?1")
            .bind(&profile_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0);
    }
}
