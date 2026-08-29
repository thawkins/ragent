//! Application state and event handling for the TUI.
//!
//! This module is the entry point for the TUI application logic.  The
//! [`App`] struct and its methods are now split across several submodules
//! (see REMPLAN.md M5).

mod state;
pub use self::state::*;

mod helpers;
pub use helpers::{image_dimensions_or_placeholder, sanitize_for_display};

mod bench;
mod compress;
pub mod cron;
mod event_handler;
mod init;
mod input_handler;
mod md_worker;
pub use self::md_worker::MdWorker;

mod models;
pub use models::model_part_from_selected_model;
mod status_bar_cache;
pub use self::status_bar_cache::StatusBarCache;

mod model_picker_cache;
pub use self::model_picker_cache::ModelPickerRowsCache;

mod research;
mod reverse;
mod session_ops;
pub mod skillgen;
mod slash;
mod swarm;

// Re-export status types from theme for use in app
pub use crate::theme::{StatusCategory, StatusHistory, StatusMessage};

#[cfg(test)]
mod tests;
