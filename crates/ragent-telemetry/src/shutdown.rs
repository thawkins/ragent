//! Graceful shutdown flush for the telemetry subsystem (T-009, FR-019).
//!
//! FR-019: "When the process receives a shutdown signal (SIGINT/SIGTERM), the
//! system shall flush all pending metric exports to the configured endpoint
//! before terminating."
//!
//! This module provides two complementary mechanisms:
//!
//! 1. [`ShutdownGuard`] — an RAII guard that calls
//!    [`TelemetrySubsystem::flush`] and [`TelemetrySubsystem::shutdown`] on
//!    `Drop`. This ensures metrics are flushed on **every** exit path:
//!    normal return, early return, `?` propagation, and even panics. The
//!    guard is the primary mechanism and requires no signal handler wiring.
//!
//! 2. [`flush_on_signal_arc`] — an async helper that spawns a background
//!    task listening for SIGINT/SIGTERM (or Ctrl+C on Windows) and calls
//!    [`TelemetrySubsystem::flush`] when a signal is received. This gives
//!    the process a chance to flush **before** the main loop exits, so
//!    metrics are delivered even if the `ShutdownGuard`'s `Drop` runs during
//!    a forceful exit where the runtime may already be tearing down.
//!    Accepts an [`Arc<TelemetrySubsystem>`](std::sync::Arc) since the
//!    spawned task requires `'static`.
//!
//! # Recommended usage
//!
//! ```no_run
//! use ragent_telemetry::{OtelConfig, TelemetrySubsystem, shutdown::ShutdownGuard};
//!
//! let config = OtelConfig::default(); // or from ragent.json
//! let subsystem = TelemetrySubsystem::new(config).expect("telemetry subsystem");
//!
//! // Install the guard — it will flush+shutdown on Drop.
//! let _guard = ShutdownGuard::new(subsystem);
//!
//! // ... run the agent loop ...
//! // When the function returns (or panics), _guard drops and flushes.
//! ```

use crate::subsystem::TelemetrySubsystem;

/// RAII guard that flushes and shuts down the [`TelemetrySubsystem`] on `Drop`.
///
/// When the guard is dropped (whether by normal scope exit, `?` propagation,
/// or panic), it calls [`TelemetrySubsystem::flush`] followed by
/// [`TelemetrySubsystem::shutdown`], ensuring all pending metric exports
/// are delivered to the configured OTLP endpoint before the process
/// terminates (FR-019).
///
/// The guard is infallible on `Drop`: flush/shutdown errors are logged at
/// `warn` level but never panic (FR-031, FR-033). This is critical because
/// `Drop` runs during stack unwinding and must not itself fail.
///
/// # Examples
///
/// ```
/// use ragent_telemetry::{OtelConfig, TelemetrySubsystem, shutdown::ShutdownGuard};
///
/// let config = OtelConfig::default(); // disabled
/// let subsystem = TelemetrySubsystem::new(config).expect("subsystem");
/// let _guard = ShutdownGuard::new(subsystem);
/// // ... do work ...
/// // _guard drops here, flushing and shutting down.
/// ```
pub struct ShutdownGuard {
    subsystem: TelemetrySubsystem,
}

impl ShutdownGuard {
    /// Create a new guard that will flush+shutdown the given subsystem on
    /// `Drop`.
    ///
    /// The subsystem is moved into the guard. Use [`Self::subsystem`] to
    /// get a reference for recording metrics while the guard is alive.
    #[must_use]
    pub const fn new(subsystem: TelemetrySubsystem) -> Self {
        Self { subsystem }
    }

    /// Returns a reference to the wrapped [`TelemetrySubsystem`].
    ///
    /// Use this to obtain instruments and record metrics while the guard is
    /// alive.
    #[must_use]
    pub const fn subsystem(&self) -> &TelemetrySubsystem {
        &self.subsystem
    }

    /// Returns a mutable reference to the wrapped [`TelemetrySubsystem`].
    #[must_use]
    pub const fn subsystem_mut(&mut self) -> &mut TelemetrySubsystem {
        &mut self.subsystem
    }

    /// Manually trigger a flush without dropping the guard (FR-006).
    ///
    /// This is useful for periodic checkpointing or before a known
    /// long-running operation where you want to ensure metrics are
    /// delivered. The guard will still flush+shutdown on `Drop`.
    ///
    /// # Errors
    ///
    /// Returns [`crate::TelemetryError`] if the flush fails. The caller
    /// should log the error but must not propagate it in a way that would
    /// crash the agent loop (FR-031, FR-033).
    pub fn flush(&self) -> crate::Result<()> {
        self.subsystem.flush()
    }

    /// Release the guard without flushing, returning the wrapped subsystem.
    ///
    /// This is useful when you want to take manual control of the shutdown
    /// sequence. The caller becomes responsible for calling
    /// [`TelemetrySubsystem::flush`] and [`TelemetrySubsystem::shutdown`].
    #[must_use]
    pub fn into_inner(self) -> TelemetrySubsystem {
        // Manually destructure to avoid running Drop on self.

        std::mem::take(&mut std::mem::ManuallyDrop::new(self).subsystem)
    }
}

impl Drop for ShutdownGuard {
    fn drop(&mut self) {
        // FR-019: flush all pending metric exports on process termination.
        // FR-031/FR-033: exporter errors must be logged but never panic.
        if let Err(e) = self.subsystem.flush() {
            tracing::warn!(error = %e, "Telemetry flush on shutdown failed");
        }
        if let Err(e) = self.subsystem.shutdown() {
            tracing::warn!(error = %e, "Telemetry shutdown failed");
        }
    }
}

impl std::fmt::Debug for ShutdownGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShutdownGuard")
            .field("subsystem", &self.subsystem)
            .finish()
    }
}

/// Spawn a background task that listens for SIGINT/SIGTERM (or Ctrl+C on
/// Windows) and calls [`TelemetrySubsystem::flush`] when a signal is
/// received (FR-019).
///
/// This provides an **early** flush — before the main loop exits and before
/// the [`ShutdownGuard`] runs its `Drop`. The early flush ensures metrics
/// are delivered even when the process is terminated forcefully (e.g.
/// `kill -9` after the first SIGINT).
///
/// The task runs until either a signal is received or the runtime is shut
/// down. It is fire-and-forget: the returned [`tokio::task::JoinHandle`] can
/// be awaited (to know when the signal fired) or simply dropped.
///
/// Accepts an [`Arc<TelemetrySubsystem>`](std::sync::Arc) since the spawned
/// task requires `'static`.
///
/// # Platform support
///
/// - **Unix** (Linux, macOS): listens for `SIGINT` and `SIGTERM` via
///   `tokio::signal::unix`.
/// - **Windows**: listens for Ctrl+C via `tokio::signal::ctrl_c`.
///
/// # Errors
///
/// Returns an error if the signal handler cannot be installed (e.g. the
/// tokio runtime is not available). The signal listener itself never
/// panics — if the signal stream ends, the task simply exits.
///
/// # Examples
///
/// ```no_run
/// use std::sync::Arc;
/// use ragent_telemetry::{OtelConfig, TelemetrySubsystem, shutdown::flush_on_signal_arc};
///
/// # tokio::runtime::Runtime::new().unwrap().block_on(async {
/// let config = OtelConfig::default();
/// let subsystem = Arc::new(TelemetrySubsystem::new(config).expect("subsystem"));
///
/// // Install the signal handler (fire-and-forget).
/// let _handle = flush_on_signal_arc(subsystem.clone()).expect("signal handler");
///
/// // ... use `subsystem` in the main loop ...
/// # });
/// ```
pub fn flush_on_signal_arc(
    subsystem: std::sync::Arc<TelemetrySubsystem>,
) -> std::io::Result<tokio::task::JoinHandle<()>> {
    let handle = tokio::spawn(async move {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{SignalKind, signal};
            let sigint = signal(SignalKind::interrupt());
            let sigterm = signal(SignalKind::terminate());

            match (sigint, sigterm) {
                (Ok(mut sigint), Ok(mut sigterm)) => {
                    tokio::select! {
                        _ = sigint.recv() => {
                            tracing::info!("SIGINT received, flushing telemetry metrics (FR-019)");
                        }
                        _ = sigterm.recv() => {
                            tracing::info!("SIGTERM received, flushing telemetry metrics (FR-019)");
                        }
                    }
                }
                (Ok(mut sigint), Err(e)) => {
                    tracing::warn!(error = %e, "Failed to install SIGTERM handler, listening for SIGINT only");
                    sigint.recv().await;
                    tracing::info!("SIGINT received, flushing telemetry metrics (FR-019)");
                }
                (Err(e), Ok(mut sigterm)) => {
                    tracing::warn!(error = %e, "Failed to install SIGINT handler, listening for SIGTERM only");
                    sigterm.recv().await;
                    tracing::info!("SIGTERM received, flushing telemetry metrics (FR-019)");
                }
                (Err(e), Err(e2)) => {
                    tracing::warn!(
                        error_sigint = %e,
                        error_sigterm = %e2,
                        "Failed to install signal handlers, telemetry will not flush on signal"
                    );
                    return;
                }
            }
        }
        #[cfg(windows)]
        {
            match tokio::signal::ctrl_c().await {
                Ok(()) => {
                    tracing::info!("Ctrl+C received, flushing telemetry metrics (FR-019)");
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to listen for Ctrl+C, telemetry will not flush on signal");
                    return;
                }
            }
        }
        #[cfg(not(any(unix, windows)))]
        {
            tracing::warn!(
                "Signal handlers not supported on this platform, telemetry will not flush on signal"
            );
            return;
        }

        // FR-019: flush all pending metric exports on shutdown signal.
        // FR-031/FR-033: exporter errors logged but never panic.
        if let Err(e) = subsystem.flush() {
            tracing::warn!(error = %e, "Telemetry flush on signal failed (FR-033)");
        } else {
            tracing::info!("Telemetry metrics flushed on signal (FR-019)");
        }
    });

    Ok(handle)
}

// ── Tests ─────────────────────────────────────────────────────────────────
