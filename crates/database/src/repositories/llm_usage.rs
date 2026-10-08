use chrono::Utc;
use sqlx::Row;

use crate::{DatabaseError, DbPool};

/// Una petición ya terminada, lista para sumarse al día de su proveedor.
#[derive(Debug, Clone, Default)]
pub struct LlmUsageSample {
    pub input_chars: i64,
    pub output_chars: i64,
    /// `Some` si la petición falló (texto breve del error).
    pub error: Option<String>,
}

/// Lo acumulado de un proveedor en un periodo.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LlmUsageTotals {
    pub requests: i64,
    pub errors: i64,
    pub input_chars: i64,
    pub output_chars: i64,
    pub last_used_at: Option<String>,
    pub last_error: Option<String>,
}

/// Uso por proveedor y día. Solo cifras, nunca el contenido de los mensajes.
pub struct LlmUsageRepository<'a> {
    pool: &'a DbPool,
}

impl<'a> LlmUsageRepository<'a> {
    pub fn new(pool: &'a DbPool) -> Self {
        Self { pool }
    }

    pub async fn record(&self, provider_id: &str, sample: &LlmUsageSample) -> Result<(), DatabaseError> {
        let now = Utc::now();
        self.record_at(provider_id, &now.format("%Y-%m-%d").to_string(), &now.to_rfc3339(), sample).await
    }

    pub async fn record_at(&self, provider_id: &str, day: &str, at: &str, sample: &LlmUsageSample) -> Result<(), DatabaseError> {
        sqlx::query(
            "INSERT INTO llm_usage (provider_id, day, requests, errors, input_chars, output_chars, last_used_at, last_error)
             VALUES (?1, ?2, 1, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(provider_id, day) DO UPDATE SET
                requests = requests + 1,
                errors = errors + excluded.errors,
                input_chars = input_chars + excluded.input_chars,
                output_chars = output_chars + excluded.output_chars,
                last_used_at = excluded.last_used_at,
                last_error = COALESCE(excluded.last_error, llm_usage.last_error)",
        )
        .bind(provider_id)
        .bind(day)
        .bind(i64::from(sample.error.is_some()))
        .bind(sample.input_chars)
        .bind(sample.output_chars)
        .bind(at)
        .bind(sample.error.as_deref())
        .execute(self.pool)
        .await?;
        Ok(())
    }

    /// Suma de los días desde `since_day` (inclusive, `YYYY-MM-DD`) hasta hoy; `None` = desde siempre.
    /// Un proveedor sin uso no aparece.
    pub async fn totals_since(&self, since_day: Option<&str>) -> Result<Vec<(String, LlmUsageTotals)>, DatabaseError> {
        let rows = sqlx::query(
            "SELECT provider_id,
                    SUM(requests) AS requests, SUM(errors) AS errors,
                    SUM(input_chars) AS input_chars, SUM(output_chars) AS output_chars,
                    MAX(last_used_at) AS last_used_at
             FROM llm_usage
             WHERE (?1 IS NULL OR day >= ?1)
             GROUP BY provider_id
             ORDER BY provider_id",
        )
        .bind(since_day)
        .fetch_all(self.pool)
        .await?;

        let mut totals = Vec::with_capacity(rows.len());
        for row in rows {
            let provider_id: String = row.get("provider_id");
            // El último error es el del día más reciente que tuvo alguno.
            let last_error: Option<String> = sqlx::query_scalar(
                "SELECT last_error FROM llm_usage WHERE provider_id = ?1 AND last_error IS NOT NULL AND (?2 IS NULL OR day >= ?2)
                 ORDER BY day DESC LIMIT 1",
            )
            .bind(&provider_id)
            .bind(since_day)
            .fetch_optional(self.pool)
            .await?;
            totals.push((
                provider_id,
                LlmUsageTotals {
                    requests: row.get("requests"),
                    errors: row.get("errors"),
                    input_chars: row.get("input_chars"),
                    output_chars: row.get("output_chars"),
                    last_used_at: row.get("last_used_at"),
                    last_error,
                },
            ));
        }
        Ok(totals)
    }

    pub async fn delete_for_provider(&self, provider_id: &str) -> Result<(), DatabaseError> {
        sqlx::query("DELETE FROM llm_usage WHERE provider_id = ?1").bind(provider_id).execute(self.pool).await?;
        Ok(())
    }

    pub async fn clear(&self) -> Result<(), DatabaseError> {
        sqlx::query("DELETE FROM llm_usage").execute(self.pool).await?;
        Ok(())
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

    fn ok(input: i64, output: i64) -> LlmUsageSample {
        LlmUsageSample { input_chars: input, output_chars: output, error: None }
    }

    #[tokio::test]
    async fn requests_of_the_same_day_add_up() {
        let pool = test_pool().await;
        let repo = LlmUsageRepository::new(&pool);
        repo.record_at("a", "2026-10-01", "2026-10-01T10:00:00Z", &ok(100, 40)).await.unwrap();
        repo.record_at("a", "2026-10-01", "2026-10-01T11:00:00Z", &ok(50, 10)).await.unwrap();

        let totals = repo.totals_since(None).await.unwrap();
        assert_eq!(totals.len(), 1);
        let (id, t) = &totals[0];
        assert_eq!(id, "a");
        assert_eq!((t.requests, t.errors, t.input_chars, t.output_chars), (2, 0, 150, 50));
        assert_eq!(t.last_used_at.as_deref(), Some("2026-10-01T11:00:00Z"));
    }

    #[tokio::test]
    async fn errors_are_counted_and_the_last_message_survives_later_successes() {
        let pool = test_pool().await;
        let repo = LlmUsageRepository::new(&pool);
        repo.record_at("a", "2026-10-01", "t1", &LlmUsageSample { input_chars: 10, output_chars: 0, error: Some("sin crédito".into()) }).await.unwrap();
        repo.record_at("a", "2026-10-01", "t2", &ok(10, 20)).await.unwrap();

        let t = &repo.totals_since(None).await.unwrap()[0].1;
        assert_eq!((t.requests, t.errors), (2, 1));
        assert_eq!(t.last_error.as_deref(), Some("sin crédito"));
    }

    #[tokio::test]
    async fn the_period_filter_leaves_out_older_days_and_providers_are_separate() {
        let pool = test_pool().await;
        let repo = LlmUsageRepository::new(&pool);
        repo.record_at("a", "2026-09-01", "t", &ok(1000, 1000)).await.unwrap();
        repo.record_at("a", "2026-10-02", "t", &ok(5, 5)).await.unwrap();
        repo.record_at("b", "2026-10-02", "t", &ok(7, 7)).await.unwrap();

        let recent = repo.totals_since(Some("2026-10-01")).await.unwrap();
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].1.input_chars, 5);
        assert_eq!(recent[1].1.input_chars, 7);
        assert_eq!(repo.totals_since(None).await.unwrap()[0].1.input_chars, 1005);
    }

    #[tokio::test]
    async fn deleting_a_provider_or_clearing_removes_its_numbers() {
        let pool = test_pool().await;
        let repo = LlmUsageRepository::new(&pool);
        repo.record_at("a", "2026-10-02", "t", &ok(1, 1)).await.unwrap();
        repo.record_at("b", "2026-10-02", "t", &ok(1, 1)).await.unwrap();
        repo.delete_for_provider("a").await.unwrap();
        assert_eq!(repo.totals_since(None).await.unwrap().len(), 1);
        repo.clear().await.unwrap();
        assert!(repo.totals_since(None).await.unwrap().is_empty());
    }
}
