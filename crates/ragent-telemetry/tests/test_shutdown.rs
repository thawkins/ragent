//! External tests for `tests` from `crates/ragent-telemetry/src/shutdown.rs`
//!
//! Relocated from the inline `#[cfg(test)]` module.

use ragent_telemetry::shutdown::{ShutdownGuard, flush_on_signal_arc};
use ragent_telemetry::{OtelConfig, TelemetrySubsystem};
use std::sync::Arc;

/// A `ShutdownGuard` wrapping a disabled subsystem does not panic on
/// Drop.
#[test]
fn test_shutdown_guard_disabled_noop() {
    let sub = TelemetrySubsystem::disabled();
    let guard = ShutdownGuard::new(sub);
    // Drop the guard — should not panic even with a disabled subsystem.
    drop(guard);
}

/// A `ShutdownGuard` wrapping a subsystem constructed from config does
/// not panic on Drop.
#[test]
fn test_shutdown_guard_from_config_noop() {
    let config = OtelConfig::default();
    let sub = TelemetrySubsystem::new(config).expect("disabled subsystem");
    let guard = ShutdownGuard::new(sub);
    drop(guard);
}

/// The `subsystem()` accessor returns a reference to the wrapped
/// subsystem.
#[test]
fn test_shutdown_guard_subsystem_accessor() {
    let sub = TelemetrySubsystem::disabled();
    let guard = ShutdownGuard::new(sub);
    assert!(!guard.subsystem().is_enabled());
}

/// The `flush()` method on the guard delegates to the subsystem and
/// succeeds for a disabled subsystem.
#[test]
fn test_shutdown_guard_flush_disabled() {
    let sub = TelemetrySubsystem::disabled();
    let guard = ShutdownGuard::new(sub);
    assert!(
        guard.flush().is_ok(),
        "flush on disabled subsystem should succeed"
    );
}

/// `into_inner()` releases the guard without flushing, returning the
/// subsystem.
#[test]
fn test_shutdown_guard_into_inner() {
    let sub = TelemetrySubsystem::disabled();
    let guard = ShutdownGuard::new(sub);
    let recovered = guard.into_inner();
    assert!(!recovered.is_enabled());
    // recovered should still be usable.
    assert!(recovered.flush().is_ok());
}

/// The `Debug` impl includes the subsystem's state.
#[test]
fn test_shutdown_guard_debug() {
    let sub = TelemetrySubsystem::disabled();
    let guard = ShutdownGuard::new(sub);
    let debug = format!("{guard:?}");
    assert!(
        debug.contains("ShutdownGuard"),
        "debug should contain struct name: {debug}"
    );
}

/// An enabled subsystem wrapped in a `ShutdownGuard` does not panic on
/// Drop (even though flush/shutdown may fail without a collector).
#[cfg(feature = "telemetry")]
#[test]
fn test_shutdown_guard_enabled_no_panic_on_drop() {
    let config = OtelConfig {
        enabled: true,
        endpoint: "http://localhost:4318".to_string(),
        ..Default::default()
    };

    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let sub = rt.block_on(async { TelemetrySubsystem::new(config).expect("enabled subsystem") });
    let guard = ShutdownGuard::new(sub);
    // Drop the guard — flush+shutdown may fail (no collector) but must
    // not panic (FR-031, FR-033).
    drop(guard);
}

/// An enabled subsystem with an `Arc` can be used with
/// `flush_on_signal_arc` without panicking at construction time.
#[cfg(feature = "telemetry")]
#[tokio::test]
async fn test_flush_on_signal_arc_constructs() {
    let config = OtelConfig {
        enabled: true,
        endpoint: "http://localhost:4318".to_string(),
        ..Default::default()
    };

    let sub = Arc::new(TelemetrySubsystem::new(config).expect("enabled subsystem"));

    // Spawn the signal handler — it should construct without error.
    let handle = flush_on_signal_arc(sub).expect("signal handler should install");
    // Abort the task to clean up (we don't want it lingering).
    handle.abort();
}

/// `flush_on_signal_arc` works with a disabled subsystem.
#[tokio::test]
async fn test_flush_on_signal_arc_disabled() {
    let sub = Arc::new(TelemetrySubsystem::disabled());
    let handle = flush_on_signal_arc(sub).expect("signal handler should install");
    handle.abort();
}
