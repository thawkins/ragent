//! Application state and event handling for the TUI.
//!
//! This module is the entry point for the TUI application logic.  The
//! [`App`] struct and its methods are now split across several submodules
//! (see REMPLAN.md M5).

mod state;
pub use self::state::*;

mod helpers;

mod init;
mod compress;
mod bench;
mod swarm;
mod research;
mod models;
mod slash;
mod input_handler;
mod event_handler;
mod session_ops;

// Re-export status types from theme for use in app
pub use crate::theme::{StatusCategory, StatusHistory, StatusMessage};

#[cfg(test)]
mod tests;
