use tokio::sync::broadcast;

use crate::event::AppEvent;

const CHANNEL_CAPACITY: usize = 256;

/// Thin wrapper around a Tokio broadcast channel. Cloning an `EventBus`
/// clones the sender handle (cheap, `Arc`-backed internally); every clone
/// publishes to the same set of subscribers.
#[derive(Clone)]
pub struct EventBus {
    tx: broadcast::Sender<AppEvent>,
}

impl EventBus {
    pub fn new() -> Self {
        let (tx, _rx) = broadcast::channel(CHANNEL_CAPACITY);
        Self { tx }
    }

    /// Publishes an event to every current subscriber. Having zero
    /// subscribers (e.g. during early startup) is normal, not an error.
    pub fn publish(&self, event: AppEvent) {
        let _ = self.tx.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<AppEvent> {
        self.tx.subscribe()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::ServiceStatus;

    #[tokio::test]
    async fn subscriber_receives_published_event() {
        let bus = EventBus::new();
        let mut rx = bus.subscribe();

        bus.publish(AppEvent::ServiceStatusChanged {
            service: "twitch".into(),
            status: ServiceStatus::Starting,
        });

        match rx.recv().await.unwrap() {
            AppEvent::ServiceStatusChanged { service, status } => {
                assert_eq!(service, "twitch");
                assert_eq!(status, ServiceStatus::Starting);
            }
            other => panic!("unexpected event variant: {other:?}"),
        }
    }

    #[tokio::test]
    async fn publish_without_subscribers_does_not_panic() {
        let bus = EventBus::new();
        bus.publish(AppEvent::Heartbeat { timestamp: "now".into() });
    }

    #[tokio::test]
    async fn multiple_subscribers_each_get_the_event() {
        let bus = EventBus::new();
        let mut rx1 = bus.subscribe();
        let mut rx2 = bus.subscribe();

        bus.publish(AppEvent::Heartbeat { timestamp: "t".into() });

        assert!(matches!(rx1.recv().await.unwrap(), AppEvent::Heartbeat { .. }));
        assert!(matches!(rx2.recv().await.unwrap(), AppEvent::Heartbeat { .. }));
    }
}
