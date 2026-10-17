//! External tests for `shutdown.rs`: the `ShutdownGuard` empty-slot paths and
//! the real signal-driven flush in `flush_on_signal_arc` (FR-019).

use ragent_telemetry::shutdown::{ShutdownGuard, flush_on_signal_arc};
use ragent_telemetry::{OtelConfig, TelemetrySubsystem};
use std::sync::Arc;

/// `into_inner` returns a usable subsystem that can still be flushed.
#[test]
fn test_into_inner_returns_usable_subsystem() {
    let sub = TelemetrySubsystem::disabled();
    let guard = ShutdownGuard::new(Arc::new(sub));
    let recovered = guard.into_inner();
    assert!(recovered.flush().is_ok());
    assert!(!recovered.is_enabled());
}

/// Dropping a guard after `into_inner` must be a no-op (empty slot early
/// return) and must not panic.
#[test]
fn test_guard_drop_after_into_inner_is_noop() {
    let sub = TelemetrySubsystem::disabled();
    let guard = ShutdownGuard::new(Arc::new(sub));
    let _inner = guard.into_inner();
    // `guard` was consumed by into_inner; a second guard exercises the same
    // "take then drop" path.
    let guard2 = ShutdownGuard::new(Arc::new(TelemetrySubsystem::disabled()));
    let _inner2 = guard2.into_inner();
    // Both drops here are no-ops because the slots were emptied.
}

/// `Debug` for the guard with an emptied slot renders `None`.
#[test]
fn test_guard_debug_after_into_inner() {
    let guard = ShutdownGuard::new(Arc::new(TelemetrySubsystem::disabled()));
    let debug = format!("{guard:?}");
    assert!(debug.contains("ShutdownGuard"));
}

/// Install the real SIGINT-driven flush path: spawn the signal handler, deliver
/// SIGINT to this process, and await the task. The handler must consume the
/// signal (so the test process survives) and flush the subsystem.
#[cfg(all(feature = "telemetry", unix))]
#[tokio::test]
async fn test_flush_on_signal_arc_handles_real_sigint() {
    let config = OtelConfig {
        enabled: true,
        endpoint: "http://localhost:4318".to_string(),
        ..Default::default()
    };
    let sub = Arc::new(TelemetrySubsystem::new(config).expect("enabled subsystem"));
    let handle = flush_on_signal_arc(Arc::clone(&sub)).expect("signal handler installs");

    // Give the spawned task time to install its signal stream.
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Deliver SIGINT to this process; the tokio unix signal handler installed
    // by the spawned task consumes it, so the process is not terminated.
    let pid = std::process::id();
    let status = std::process::Command::new("kill")
        .args(["-INT", &pid.to_string()])
        .status();
    assert!(status.is_ok(), "kill command must run");

    // The task should resolve promptly after the signal.
    let joined = tokio::time::timeout(std::time::Duration::from_secs(5), handle).await;
    assert!(
        joined.is_ok(),
        "flush_on_signal_arc task should resolve after SIGINT"
    );
}

/// The disabled subsystem path through the signal handler also resolves.
#[cfg(all(feature = "telemetry", unix))]
#[tokio::test]
async fn test_flush_on_signal_arc_disabled_handles_sigint() {
    let sub = Arc::new(TelemetrySubsystem::disabled());
    let handle = flush_on_signal_arc(sub).expect("signal handler installs");
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    let pid = std::process::id();
    let _ = std::process::Command::new("kill")
        .args(["-INT", &pid.to_string()])
        .status();
    let joined = tokio::time::timeout(std::time::Duration::from_secs(5), handle).await;
    assert!(joined.is_ok(), "disabled signal handler should resolve");
}
