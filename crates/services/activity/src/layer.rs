use tokio::sync::mpsc::UnboundedSender;
use tracing::field::{Field, Visit};
use tracing::Level;
use tracing_subscriber::layer::Context;
use tracing_subscriber::Layer;

use aivt_database::{ActivityLevel, NewActivityEvent};

/// Bridges `tracing` events into `activity_events`.
///
/// Only events emitted with `target: "activity"` are forwarded — ordinary
/// `tracing::debug!`/`info!` calls elsewhere in the codebase keep going to
/// the rotating log file (via the regular `fmt` layer) without also
/// flooding the structured activity table. By convention, an activity-target
/// call site should include a `summary` field; everything else
/// (`category`, `module`, `action`, `status`, id fields, `duration_ms`) is
/// optional and falls back to a sane default.
pub struct ActivityLayer {
    sender: UnboundedSender<NewActivityEvent>,
}

impl ActivityLayer {
    pub fn new(sender: UnboundedSender<NewActivityEvent>) -> Self {
        Self { sender }
    }
}

impl<S> Layer<S> for ActivityLayer
where
    S: tracing::Subscriber,
{
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        let metadata = event.metadata();
        if metadata.target() != "activity" {
            return;
        }

        let mut collector = FieldCollector::default();
        event.record(&mut collector);

        let level = *metadata.level();
        let activity_level = if level == Level::ERROR {
            ActivityLevel::Error
        } else if level == Level::WARN {
            ActivityLevel::Warning
        } else if level == Level::INFO {
            ActivityLevel::Info
        } else {
            ActivityLevel::Debug
        };

        let new_event = NewActivityEvent {
            level: activity_level,
            category: collector.category.unwrap_or_else(|| "SYSTEM".to_string()),
            module: collector.module.unwrap_or_else(|| metadata.target().to_string()),
            action: collector.action.unwrap_or_else(|| metadata.name().to_string()),
            profile_id: collector.profile_id,
            conversation_id: collector.conversation_id,
            request_id: collector.request_id,
            duration_ms: collector.duration_ms,
            status: collector.status.unwrap_or_else(|| "info".to_string()),
            summary: collector.summary.unwrap_or_default(),
            details: None,
        };

        let _ = self.sender.send(new_event);
    }
}

#[derive(Default)]
struct FieldCollector {
    category: Option<String>,
    module: Option<String>,
    action: Option<String>,
    summary: Option<String>,
    status: Option<String>,
    profile_id: Option<String>,
    conversation_id: Option<String>,
    request_id: Option<String>,
    duration_ms: Option<i64>,
}

impl FieldCollector {
    fn set(&mut self, name: &str, value: String) {
        match name {
            "category" => self.category = Some(value),
            "module" => self.module = Some(value),
            "action" => self.action = Some(value),
            "summary" => self.summary = Some(value),
            "status" => self.status = Some(value),
            "profile_id" => self.profile_id = Some(value),
            "conversation_id" => self.conversation_id = Some(value),
            "request_id" => self.request_id = Some(value),
            // Unrecognized fields (including the implicit `message`) are
            // ignored rather than guessed at — call sites are expected to
            // set `summary` explicitly.
            _ => {}
        }
    }
}

impl Visit for FieldCollector {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.set(field.name(), format!("{value:?}"));
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.set(field.name(), value.to_string());
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        if field.name() == "duration_ms" {
            self.duration_ms = Some(value);
        }
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        if field.name() == "duration_ms" {
            self.duration_ms = Some(value as i64);
        }
    }
}
