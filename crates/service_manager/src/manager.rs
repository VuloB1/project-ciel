use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;

use aivt_eventbus::{AppEvent, EventBus, ServiceStatus};

use crate::{ManagedService, ServiceManagerError};

/// Central registry of `ManagedService`s. Owns the source of truth for each
/// service's `ServiceStatus` and publishes `AppEvent::ServiceStatusChanged`
/// on every transition, so the UI (via the EventBus -> Tauri `emit` bridge)
/// always reflects reality without polling.
pub struct ServiceManager {
    event_bus: EventBus,
    services: RwLock<HashMap<String, Arc<dyn ManagedService>>>,
    statuses: RwLock<HashMap<String, ServiceStatus>>,
}

impl ServiceManager {
    pub fn new(event_bus: EventBus) -> Self {
        Self {
            event_bus,
            services: RwLock::new(HashMap::new()),
            statuses: RwLock::new(HashMap::new()),
        }
    }

    /// Registers a service in the `Stopped` state. Does not start it —
    /// starting is a separate, explicit call driven by Settings (sección 80).
    pub async fn register(&self, service: Arc<dyn ManagedService>) {
        let name = service.name().to_string();
        self.services.write().await.insert(name.clone(), service);
        self.set_status(&name, ServiceStatus::Stopped).await;
    }

    pub async fn start(&self, name: &str) -> Result<(), ServiceManagerError> {
        let service = self.get(name).await?;
        self.set_status(name, ServiceStatus::Starting).await;

        match service.start().await {
            Ok(()) => {
                self.set_status(name, ServiceStatus::Running).await;
                Ok(())
            }
            Err(err) => {
                self.set_status(name, ServiceStatus::Error).await;
                Err(ServiceManagerError::StartFailed {
                    service: name.to_string(),
                    reason: err.to_string(),
                })
            }
        }
    }

    pub async fn stop(&self, name: &str) -> Result<(), ServiceManagerError> {
        let service = self.get(name).await?;
        self.set_status(name, ServiceStatus::Stopping).await;

        match service.stop().await {
            Ok(()) => {
                self.set_status(name, ServiceStatus::Stopped).await;
                Ok(())
            }
            Err(err) => {
                self.set_status(name, ServiceStatus::Error).await;
                Err(ServiceManagerError::StopFailed {
                    service: name.to_string(),
                    reason: err.to_string(),
                })
            }
        }
    }

    pub async fn status(&self, name: &str) -> Option<ServiceStatus> {
        self.statuses.read().await.get(name).copied()
    }

    pub async fn statuses(&self) -> HashMap<String, ServiceStatus> {
        self.statuses.read().await.clone()
    }

    async fn get(&self, name: &str) -> Result<Arc<dyn ManagedService>, ServiceManagerError> {
        self.services
            .read()
            .await
            .get(name)
            .cloned()
            .ok_or_else(|| ServiceManagerError::UnknownService(name.to_string()))
    }

    async fn set_status(&self, name: &str, status: ServiceStatus) {
        self.statuses.write().await.insert(name.to_string(), status);
        self.event_bus.publish(AppEvent::ServiceStatusChanged {
            service: name.to_string(),
            status,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicBool, Ordering};

    struct FakeService {
        name: &'static str,
        fail: AtomicBool,
    }

    impl FakeService {
        fn new(name: &'static str) -> Self {
            Self { name, fail: AtomicBool::new(false) }
        }

        fn failing(name: &'static str) -> Self {
            Self { name, fail: AtomicBool::new(true) }
        }
    }

    #[async_trait]
    impl ManagedService for FakeService {
        fn name(&self) -> &str {
            self.name
        }

        async fn start(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            if self.fail.load(Ordering::SeqCst) {
                Err("simulated start failure".into())
            } else {
                Ok(())
            }
        }

        async fn stop(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn newly_registered_service_is_stopped() {
        let manager = ServiceManager::new(EventBus::new());
        manager.register(Arc::new(FakeService::new("twitch"))).await;

        assert_eq!(manager.status("twitch").await, Some(ServiceStatus::Stopped));
    }

    #[tokio::test]
    async fn start_transitions_to_running_and_publishes_events() {
        let bus = EventBus::new();
        let mut rx = bus.subscribe();
        let manager = ServiceManager::new(bus);
        manager.register(Arc::new(FakeService::new("discord"))).await;

        manager.start("discord").await.unwrap();
        assert_eq!(manager.status("discord").await, Some(ServiceStatus::Running));

        // register -> Stopped, start -> Starting, start -> Running.
        let mut seen = Vec::new();
        for _ in 0..3 {
            if let AppEvent::ServiceStatusChanged { status, .. } = rx.recv().await.unwrap() {
                seen.push(status);
            }
        }
        assert_eq!(
            seen,
            vec![ServiceStatus::Stopped, ServiceStatus::Starting, ServiceStatus::Running]
        );
    }

    #[tokio::test]
    async fn failing_start_leaves_service_in_error_state() {
        let manager = ServiceManager::new(EventBus::new());
        manager.register(Arc::new(FakeService::failing("call"))).await;

        let result = manager.start("call").await;
        assert!(result.is_err());
        assert_eq!(manager.status("call").await, Some(ServiceStatus::Error));
    }

    #[tokio::test]
    async fn stop_transitions_running_service_back_to_stopped() {
        let manager = ServiceManager::new(EventBus::new());
        manager.register(Arc::new(FakeService::new("avatar"))).await;
        manager.start("avatar").await.unwrap();

        manager.stop("avatar").await.unwrap();
        assert_eq!(manager.status("avatar").await, Some(ServiceStatus::Stopped));
    }

    #[tokio::test]
    async fn starting_unknown_service_errors() {
        let manager = ServiceManager::new(EventBus::new());
        let result = manager.start("does-not-exist").await;
        assert!(matches!(result, Err(ServiceManagerError::UnknownService(_))));
    }
}
