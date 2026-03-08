//! HTTP/SSE server for ragent.
//!
//! Provides an Axum-based REST API and Server-Sent Events (SSE) stream for
//! managing agent sessions, sending messages, and receiving real-time updates
//! from the ragent core runtime.

pub mod routes;
pub mod sse;

pub use routes::start_server;
