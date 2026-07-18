//! Telemetry integration for the ragent agent crate.
//!
//! This module re-exports the public telemetry API from `ragent-telemetry` so
//! that `ragent-agent` can record metrics without depending on a Cargo feature
//! flag. The actual OTEL implementation is enabled by default in
//! `ragent-telemetry`; when disabled at compile time, all recorders are
//! zero-overhead no-ops.

pub use ragent_telemetry::shutdown::{ShutdownGuard, flush_on_signal_arc};
pub use ragent_telemetry::{
    InstrumentRegistry, LlmRecorder, OtelConfig, OtelProtocol, SessionRecorder, TelemetryConfig,
    TelemetryState, TelemetrySubsystem, ToolRecorder,
};
