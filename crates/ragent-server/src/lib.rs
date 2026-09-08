//! HTTP/SSE server for ragent.
//!
//! Provides an Axum-based REST API and Server-Sent Events (SSE) stream for
//! managing agent sessions, sending messages, and receiving real-time updates
//! from the ragent core runtime.

// Prevent blocking sync primitives in async code.
#![deny(clippy::await_holding_lock)]
// Deep async-handler futures exceed the default type-recursion limit during
// trait resolution; raise it so the `Handler` impl for every route computes.
#![recursion_limit = "256"]

pub mod routes;
pub mod sse;

pub use routes::start_server;
