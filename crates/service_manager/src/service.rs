use async_trait::async_trait;

/// A background integration the app can start and stop independently.
///
/// Fase 0 registers no real implementations yet — Chat/Twitch/Discord/Call
/// each implement this in their own phase. `ServiceManager` only ever talks
/// to this trait object, never to a concrete service type, so one
/// integration failing can't take another down (sección 81 del brief).
#[async_trait]
pub trait ManagedService: Send + Sync {
    /// Stable identifier used in `AppEvent::ServiceStatusChanged` and in
    /// activity logs (e.g. "twitch", "discord", "call").
    fn name(&self) -> &str;

    async fn start(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    async fn stop(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}
