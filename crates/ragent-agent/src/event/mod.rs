//! Event streaming infrastructure for ragent sessions.
//!
//! The [`EventBus`] broadcasts [`Event`] values to any number of subscribers
//! using a Tokio broadcast channel. Events cover the full lifecycle of a
//! session: creation, message streaming, tool calls, permission gates,
//! agent switches, errors, and token usage.

pub use ragent_types::event::{Event, EventBus, FinishReason};
