//! Custom [`tracing_subscriber::Layer`] that captures log records and forwards
//! them to the TUI log panel via a channel instead of writing to stdout/stderr.
//!
//! This prevents tracing output from corrupting the ratatui alternate-screen
//! rendering during an interactive TUI session. Records are buffered in a
//! [`std::sync::mpsc`] channel and drained by the TUI event loop on each frame.

use std::sync::mpsc;

use tracing::Level;
use tracing_subscriber::Layer;

/// A log record forwarded from the tracing layer to the TUI.
#[derive(Debug)]
pub struct TuiLogRecord {
    /// Severity level of the record.
    pub level: Level,
    /// Formatted log message.
    pub message: String,
}

/// Receiving end of the tracing → TUI channel.
pub type TuiLogReceiver = mpsc::Receiver<TuiLogRecord>;

/// Sending end of the tracing → TUI channel.
type TuiLogSender = mpsc::SyncSender<TuiLogRecord>;

/// Create a matched (sender, receiver) pair for the TUI tracing layer.
///
/// The `capacity` controls how many records can be buffered before senders
/// block. 512 is enough for any realistic burst.
pub fn tui_log_channel(capacity: usize) -> (TuiLogSender, TuiLogReceiver) {
    mpsc::sync_channel(capacity)
}

/// A [`tracing_subscriber::Layer`] that sends log records to the TUI.
///
/// Install this layer via [`tracing_subscriber::registry()`] instead of the
/// default `fmt()` subscriber when the TUI is active, so that all tracing
/// output is routed to [`app::push_log`] rather than stdout.
pub struct TuiTracingLayer {
    tx: TuiLogSender,
}

impl TuiTracingLayer {
    /// Create a new layer that forwards records over `tx`.
    pub fn new(tx: TuiLogSender) -> Self {
        Self { tx }
    }
}

/// Visitor that extracts the `message` field and endpoint-related metadata
/// from a tracing event.
#[derive(Default)]
struct MessageVisitor {
    message: String,
    endpoint: Option<String>,
}

impl tracing::field::Visit for MessageVisitor {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        match field.name() {
            "message" => self.message = value.to_string(),
            "endpoint" | "url" | "api_base" | "base_url" => self.endpoint = Some(value.to_string()),
            _ => {}
        }
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        match field.name() {
            "message" => self.message = format!("{:?}", value),
            "endpoint" | "url" | "api_base" | "base_url" => {
                self.endpoint = Some(format!("{:?}", value));
            }
            _ => {}
        }
    }
}

impl<S> Layer<S> for TuiTracingLayer
where
    S: tracing::Subscriber,
{
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let level = *event.metadata().level();
        let mut visitor = MessageVisitor::default();
        event.record(&mut visitor);

        // Include the target (module path) for context when it adds value.
        let target = event.metadata().target();
        let mut message = if target.starts_with("ragent") || visitor.message.is_empty() {
            visitor.message
        } else {
            format!("[{}] {}", target, visitor.message)
        };

        // Append endpoint info if present — this is how we show the full
        // Azure AI Foundry URL in the TUI log panel.
        if let Some(endpoint) = visitor.endpoint {
            message.push_str(&format!(" → {}", endpoint));
        }

        // Non-blocking send — drop the record if the channel is full rather
        // than stalling the async runtime.
        let _ = self.tx.try_send(TuiLogRecord { level, message });
    }
}
