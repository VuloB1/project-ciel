use tokio::sync::mpsc;

use aivt_database::{ActivityEventRow, ActivityRepository, DatabaseError, DbPool, NewActivityEvent};

use crate::layer::ActivityLayer;

/// Handle for writing to and reading from `activity_events`.
///
/// Writes are fire-and-forget from the caller's perspective: `record` just
/// pushes onto an unbounded channel and returns immediately. A single
/// background task owns the `DbPool` writer side and drains the channel,
/// so callers never block on (or fail because of) a slow disk write.
pub struct ActivityService {
    sender: mpsc::UnboundedSender<NewActivityEvent>,
    pool: DbPool,
}

impl ActivityService {
    /// Spawns the background writer task and returns the service handle
    /// alongside the `tracing_subscriber::Layer` that feeds it. Install the
    /// layer once at startup, e.g.:
    ///
    /// ```ignore
    /// let (activity, layer) = ActivityService::spawn(pool);
    /// tracing_subscriber::registry().with(layer).with(fmt_layer).init();
    /// ```
    pub fn spawn(pool: DbPool) -> (Self, ActivityLayer) {
        let (tx, mut rx) = mpsc::unbounded_channel::<NewActivityEvent>();

        let writer_pool = pool.clone();
        tokio::spawn(async move {
            let repo = ActivityRepository::new(&writer_pool);
            while let Some(event) = rx.recv().await {
                if let Err(err) = repo.record(event).await {
                    // The activity log itself failing to write must never
                    // take the app down or recurse into itself; fall back to
                    // stderr via `tracing` on a different target.
                    tracing::warn!(target: "system", "failed to persist activity event: {err}");
                }
            }
        });

        let layer = ActivityLayer::new(tx.clone());
        (Self { sender: tx, pool }, layer)
    }

    pub fn record(&self, event: NewActivityEvent) {
        // A closed receiver only happens if the writer task panicked; there
        // is nothing useful to do about that from the call site.
        let _ = self.sender.send(event);
    }

    pub async fn recent(&self, limit: i64) -> Result<Vec<ActivityEventRow>, DatabaseError> {
        ActivityRepository::new(&self.pool).recent(limit).await
    }

    pub async fn clear(&self) -> Result<(), DatabaseError> {
        ActivityRepository::new(&self.pool).clear().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aivt_database::ActivityLevel;
    use std::time::Duration;

    async fn test_pool() -> DbPool {
        let pool = DbPool::connect("sqlite::memory:").await.unwrap();
        aivt_database::migrate(&pool).await.unwrap();
        pool
    }

    fn sample_event(summary: &str) -> NewActivityEvent {
        NewActivityEvent {
            level: ActivityLevel::Info,
            category: "SYSTEM".into(),
            module: "test".into(),
            action: "sample".into(),
            profile_id: None,
            conversation_id: None,
            request_id: None,
            duration_ms: None,
            status: "success".into(),
            summary: summary.to_string(),
            details: None,
        }
    }

    async fn wait_for(service: &ActivityService, summary: &str) -> bool {
        for _ in 0..50 {
            let recent = service.recent(20).await.unwrap();
            if recent.iter().any(|e| e.summary == summary) {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        false
    }

    #[tokio::test]
    async fn record_is_eventually_persisted() {
        let pool = test_pool().await;
        let (service, _layer) = ActivityService::spawn(pool);

        service.record(sample_event("hello from record()"));

        assert!(wait_for(&service, "hello from record()").await);
    }

    #[tokio::test]
    async fn tracing_event_with_activity_target_is_persisted() {
        use tracing_subscriber::layer::SubscriberExt;

        let pool = test_pool().await;
        let (service, layer) = ActivityService::spawn(pool);
        let subscriber = tracing_subscriber::registry().with(layer);

        tracing::subscriber::with_default(subscriber, || {
            tracing::info!(
                target: "activity",
                category = "SYSTEM",
                module = "test",
                action = "sample",
                summary = "hello from tracing",
            );
        });

        assert!(wait_for(&service, "hello from tracing").await);
    }

    #[tokio::test]
    async fn tracing_event_without_activity_target_is_ignored() {
        use tracing_subscriber::layer::SubscriberExt;

        let pool = test_pool().await;
        let (service, layer) = ActivityService::spawn(pool);
        let subscriber = tracing_subscriber::registry().with(layer);

        tracing::subscriber::with_default(subscriber, || {
            tracing::info!(target: "some_other_target", "should not be persisted");
        });

        // Give the (non-existent) write every chance to land before asserting absence.
        tokio::time::sleep(Duration::from_millis(50)).await;
        let recent = service.recent(20).await.unwrap();
        assert!(recent.is_empty());
    }

    #[tokio::test]
    async fn clear_empties_the_feed() {
        let pool = test_pool().await;
        let (service, _layer) = ActivityService::spawn(pool);
        service.record(sample_event("hello from record()"));
        assert!(wait_for(&service, "hello from record()").await);

        service.clear().await.unwrap();

        assert_eq!(service.recent(10).await.unwrap().len(), 0);
    }
}
