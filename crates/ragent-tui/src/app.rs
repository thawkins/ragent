//! Application state and event handling for the TUI.
//!
//! This module is the entry point for the TUI application logic.  The
//! [`App`] struct and its methods are now split across several submodules
//! (see REMPLAN.md M5).

mod state;
pub use self::state::*;

mod helpers;
pub(crate) use helpers::short_id;
pub use helpers::{hard_break_lines, image_dimensions_or_placeholder, sanitize_for_display};
#[doc(hidden)]
pub use helpers::{set_log_root_override, use_scratch_log_root_for_tests};

mod backend_panel;
pub use self::backend_panel::{BackendPanelRow, BackendPanelState};

mod bench;
mod compress;
pub mod cron;
mod event_handler;
mod init;
mod input_handler;
mod loop_dialog;
pub mod md_worker;
pub use self::md_worker::MdWorker;

mod connector;
mod models;
mod newproj;
mod plugin;
pub(crate) use self::plugin::toggle_plugin_enabled;
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
mod launch;
mod mcp_bridge;
mod spawn;
mod subagent_events;
mod swarm;
pub mod toolchain;
mod windows;

/// Why the Agents window refuses to open with nothing to show (spec
/// `agentssetup` FR-042). Re-exported so the panel's refusal text has one
/// definition, reachable from the TUI tests.
pub use self::windows::AGENTS_WINDOW_EMPTY_REFUSAL;

/// Why the Teams window refuses to open with no active team (spec `agentssetup`
/// FR-042), re-exported alongside its Agents counterpart.
pub use self::windows::TEAMS_WINDOW_EMPTY_REFUSAL;

/// Test hook: drive one pass of the shared sub-agent launch poll
/// (`tests/test_slash_commands.rs`, `tests/test_misc_modules_deep.rs`) without
/// widening the production surface.
#[doc(hidden)]
pub fn poll_spawn_result_for_tests(app: &mut App) {
    app.poll_subagent_launch_results();
}

/// Test hook: seed one completed launch outcome on the shared queue, as the
/// launch worker would, so a test can drive the poll's reporting arms without
/// running a real sub-agent (spec `agentssetup` D3).
#[doc(hidden)]
pub fn seed_spawn_outcome_for_tests(app: &mut App, outcome: Result<String, String>) {
    let launch = app
        .begin_subagent_launch(LaunchOrigin::SpawnCommand, "explore")
        .unwrap_or_else(|_| panic!("test hook: launch set must have room"));
    let mut queue = crate::app::session_ops::recover_poisoned(
        app.subagent_launch_results.lock(),
        "subagent_launch_results",
    );
    queue.push(SubagentLaunchOutcome { launch, outcome });
}

/// Test hook: how many launches are currently registered as in flight (D3).
#[doc(hidden)]
pub fn pending_spawn_launches_for_tests(app: &App) -> usize {
    app.subagent_launches.len()
}

/// Test hook: register one launch without running it, so a test can occupy the
/// bounded launch set and assert the refusal (spec `agentssetup` D3).
#[doc(hidden)]
pub fn register_spawn_launch_for_tests(app: &mut App, agent: &str) {
    // INTENTIONAL: a refusal here is the very thing the test asserts on.
    let _ = app.begin_subagent_launch(LaunchOrigin::SpawnCommand, agent);
}

/// Test hook: drive the `SubagentComplete` transition (including the snapshot
/// into the completed history) without a live sub-agent run
/// (`tests/test_agents_window_refusals.rs`, spec `agentssetup` FR-028).
#[doc(hidden)]
pub fn note_subagent_completed_for_tests(
    app: &mut App,
    task_id: &str,
    finish_reason: &str,
) -> Option<ragent_agent::task::ReportStatus> {
    app.note_subagent_completed(task_id, finish_reason)
}

/// Test hook: drive the `SubagentCancelled` transition (including the snapshot
/// into the completed history) without a live sub-agent run
/// (`tests/test_agents_window_refusals.rs`, spec `agentssetup` FR-032).
#[doc(hidden)]
pub fn note_subagent_cancelled_for_tests(app: &mut App, task_id: &str) -> Option<String> {
    app.note_subagent_cancelled(task_id)
}

/// Test hook: the bridged MCP servers as `(id, origin label, defining file)`
/// rows, so the integration tests can assert that `/mcp discover` offers a
/// plugin- or connector-bridged server with its owner and the manifest that
/// declares it.
#[doc(hidden)]
pub fn bridged_discover_rows_for_tests<S: std::hash::BuildHasher>(
    working_dir: &std::path::Path,
    configured: &std::collections::HashMap<String, ragent_agent::McpServerConfig, S>,
) -> Vec<(String, &'static str, Option<std::path::PathBuf>)> {
    mcp_bridge::bridged_discover_rows(working_dir, configured)
        .into_iter()
        .map(|server| {
            let label = match &server.source {
                ragent_agent::mcp::McpDiscoverySource::Plugin { .. } => "plugin",
                ragent_agent::mcp::McpDiscoverySource::Connector { .. } => "connector",
                ragent_agent::mcp::McpDiscoverySource::SystemPath
                | ragent_agent::mcp::McpDiscoverySource::NpmGlobal { .. }
                | ragent_agent::mcp::McpDiscoverySource::McpRegistry { .. } => "discovered",
            };
            (server.id, label, server.definition_file)
        })
        .collect()
}

/// Test hook: the `/mcp remove` refusal text for a bridged id whose owner is
/// `owner`, as a plugin or a connector.
#[doc(hidden)]
pub fn bridged_removal_refusal_for_tests(server_id: &str, owner: &str, is_plugin: bool) -> String {
    let origin = if is_plugin {
        mcp_bridge::BridgeOrigin::Plugin {
            plugin_id: owner.to_string(),
        }
    } else {
        mcp_bridge::BridgeOrigin::Connector {
            connector_id: owner.to_string(),
        }
    };
    let server = mcp_bridge::BridgedMcpServer {
        id: server_id.to_string(),
        config: ragent_agent::McpServerConfig::default(),
        source_file: None,
        origin,
    };
    mcp_bridge::bridged_removal_refusal(server_id, &server)
}

/// Test hook: render the `/mcp config` report from discovery results, without
/// widening the private slash-command module.
#[doc(hidden)]
pub fn render_mcp_config_for_tests(
    locations: &[ragent_agent::mcp::McpDiscoveryLocation],
) -> String {
    slash::render_mcp_config(locations)
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

/// Test hook: drive the `/reverse` text grammar end to end (the production
/// `/spec reverse` path calls [`App::run_spec_reverse`] with pre-parsed args,
/// so this is the only way to exercise `parse_reverse_args` against a live
/// `App`). Kept reachable for the integration tests without widening the
/// production slash-command surface.
#[doc(hidden)]
pub fn reverse_command_for_tests(app: &mut App, args: &str) {
    app.handle_reverse_command(args);
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
