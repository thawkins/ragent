//! Application state and event handling for the TUI.
//!
//! This module is the entry point for the TUI application logic.  The
//! [`App`] struct and its methods are now split across several submodules
//! (see REMPLAN.md M5).

mod state;
pub use self::state::*;

mod helpers;
pub use helpers::{hard_break_lines, image_dimensions_or_placeholder, sanitize_for_display};

mod bench;
mod compress;
pub mod cron;
mod event_handler;
mod init;
mod input_handler;
mod loop_dialog;
pub mod md_worker;
pub use self::md_worker::MdWorker;

mod models;
mod newproj;
mod plugin;
pub use models::model_part_from_selected_model;
mod status_bar_cache;
pub use self::status_bar_cache::StatusBarCache;

mod model_picker_cache;
pub use self::model_picker_cache::ModelPickerRowsCache;

pub mod prompt;
mod research;
mod reverse;
mod session_ops;
pub mod skillgen;
mod slash;

/// Test hook: expose the `/mcp` display-list builder to the integration test
/// suite (`tests/test_slash_commands.rs`) without widening the production
/// surface. `crate::app::slash::*` is not visible outside the `app` module.
#[doc(hidden)]
pub fn mcp_display_servers_for_tests<S, H>(
    previous: &[ragent_agent::mcp::McpServer],
    configured: &std::collections::HashMap<String, ragent_agent::McpServerConfig, S>,
    working_dir: &std::path::Path,
    live: &std::collections::HashMap<String, ragent_agent::mcp::McpStatus, H>,
) -> Vec<ragent_agent::mcp::McpServer>
where
    S: std::hash::BuildHasher,
    H: std::hash::BuildHasher,
{
    slash::mcp_display_servers(previous, configured, working_dir, live)
}
mod spawn;
mod swarm;
pub mod toolchain;

/// Test hook: expose the private `/spawn` poll to the integration test suite
/// (`tests/test_slash_commands.rs`) without widening the production surface.
#[doc(hidden)]
pub fn poll_spawn_result_for_tests(app: &mut App) {
    app.poll_spawn_result();
}

/// Test hook: re-render a parsed `/spec reverse` invocation into the
/// `/reverse` argument string, so the integration tests can assert the
/// tokenizer round-trip without widening the production surface.
#[doc(hidden)]
pub fn spec_reverse_args_for_tests(
    repo: String,
    create: Option<String>,
    depth: Option<String>,
    scaffold: Option<ragent_tools_extended::project_scaffold::ScaffoldRequest>,
    folder: Option<String>,
) -> String {
    reverse::render_spec_reverse_args_for_tests(&reverse::build_spec_reverse_args(
        repo, create, depth, scaffold, folder,
    ))
}

pub use self::loop_dialog::{
    LoopOverrides, LoopSetupField, LoopSetupState, apply_loop_overrides, build_spec_from_state,
    handle_loop_setup_key, open_loop_setup, parse_comma_list, parse_loop_flags, parse_optional_u64,
    show_loop_help,
};

/// Shared helpers for listing installed team blueprints.
pub mod blueprints;

// Re-export status types from theme for use in app
pub use crate::theme::{StatusCategory, StatusHistory, StatusMessage};

#[cfg(test)]
mod tests;
