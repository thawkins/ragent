//! HTTP/SSE server for ragent.
//!
//! Provides an Axum-based REST API and Server-Sent Events (SSE) stream for
//! managing agent sessions, sending messages, and receiving real-time updates
//! from the ragent core runtime.

#![deny(unsafe_code)]
// Prevent blocking sync primitives in async code.
#![deny(clippy::await_holding_lock)]
// Deep async-handler futures exceed the default type-recursion limit during
// trait resolution; raise it so the `Handler` impl for every route computes.
#![recursion_limit = "256"]

pub mod routes;
pub mod sse;

pub use routes::start_server;

/// Default address `ragent serve` binds to when `--addr` is not given.
///
/// The single source of truth for the CLI flag default, the TUI's initial
/// `webapi_addr`, and the CORS allowlist that must cover it.
pub const DEFAULT_LISTEN_ADDR: &str = "127.0.0.1:3000";
