use std::sync::Arc;

use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use aivt_eventbus::EventBus;
use aivt_service_manager::ManagedService;
use aivt_service_profiles::ProfileService;
use aivt_service_settings::SettingsService;

use crate::routes::{router, ServerState};

/// Cancelling this stops the `axum::serve` task gracefully (see `start`) —
/// mirrors the `CancellationToken` shutdown pattern already used by
/// `CallService`/`ProactiveService` for their own background async tasks.
struct Session {
    cancellation: CancellationToken,
}

struct Inner {
    profiles: Arc<ProfileService>,
    event_bus: EventBus,
    settings: SettingsService,
    session: Mutex<Option<Session>>,
}

/// The `ManagedService` that owns the OBS Browser Source HTTP/WebSocket
/// server's lifecycle — registered with `ServiceManager` like Twitch/Discord/
/// Call, but (unlike those) auto-resumed at boot if the user previously
/// turned it on, see `src-tauri/src/lib.rs`'s `setup()`.
#[derive(Clone)]
pub struct AvatarServerService(Arc<Inner>);

impl AvatarServerService {
    pub fn new(profiles: Arc<ProfileService>, event_bus: EventBus, settings: SettingsService) -> Self {
        Self(Arc::new(Inner { profiles, event_bus, settings, session: Mutex::new(None) }))
    }
}

#[async_trait::async_trait]
impl ManagedService for AvatarServerService {
    fn name(&self) -> &str {
        "avatar_server"
    }

    /// Binds loopback-only (`127.0.0.1`) — OBS always runs on the same
    /// machine, and staying off `0.0.0.0` avoids both exposing anything to
    /// the LAN and triggering a Windows Firewall inbound-connection prompt.
    async fn start(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let port = self.0.settings.avatar_server_settings().await?.port;
        let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
        let listener = tokio::net::TcpListener::bind(addr).await?;

        let state = ServerState { profiles: self.0.profiles.clone(), event_bus: self.0.event_bus.clone() };
        let token = CancellationToken::new();
        let shutdown = token.clone();
        tokio::spawn(async move {
            let result = axum::serve(listener, router(state))
                .with_graceful_shutdown(async move { shutdown.cancelled().await })
                .await;
            if let Err(err) = result {
                tracing::warn!(target: "system", "avatar_server: el servidor terminó con un error: {err}");
            }
        });

        *self.0.session.lock().await = Some(Session { cancellation: token });
        Ok(())
    }

    async fn stop(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(session) = self.0.session.lock().await.take() {
            session.cancellation.cancel();
        }
        Ok(())
    }
}
