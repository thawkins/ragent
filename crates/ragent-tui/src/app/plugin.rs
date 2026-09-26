//! TUI `/plugins` command surface (spec `plugins` T-014; FR-006, FR-014).
//!
//! `/plugins <sub> [args...]` is dispatched here. The heavy lifting lives in the
//! `ragent-plugins` crate: [`run_store_command`] (add/remove),
//! [`run_control_command`] (list/enable/disable), [`run_test_command`] (test),
//! and [`render_help`] (help, a bare `/plugins`, or an unrecognised
//! subcommand). This module adds the TUI-side glue: [`handle_plugins_command`]
//! parses the subcommand, renders usage for the help/bare/unknown cases, and
//! forwards the rest to the shared entry points with an ephemeral
//! [`PluginSession`].
//!
//! ## Scope
//!
//! This module owns the `/plugins` family only (FR-006, FR-014). Registering
//! plugin-*contributed* tools and commands into the live session registries is
//! the session-start integration (T-016), deliberately not done here.
//!
//! ## `!Send` sandbox contexts
//!
//! The plugin manager owns `rquickjs` sandbox contexts, which are `!Send` and
//! `!Sync`. The [`PluginSession`] is therefore created, used, and dropped
//! entirely inside one synchronous [`handle_plugins_command`] call — it is
//! never stored on [`App`](crate::app::App) and never crosses an `.await`, so
//! `App` stays `Send + Sync` for the async event loop.

use ragent_plugins::{
    PLUGIN_SUBCOMMANDS, StoreKind, render_help, run_plugin_subcommand, store_and_config,
    subcommand_of,
};

use crate::app::helpers::current_working_dir;

/// Dispatch a `/plugins <args>` invocation.
///
/// Returns `Some(report)` for a management report to print in the message
/// window, or `None` when the invocation opened the plugin-store browse panel
/// (`/plugins codex`, `/plugins claude`), which owns the screen and prints
/// nothing.
///
/// `app` is borrowed to read the live tool-name set for collision rejection
/// (FR-024) and to open the browse panel (FR-007); nothing is stored on it.
///
/// A bare `/plugins`, `/plugins help`, and an unrecognised subcommand all
/// render the usage block and create no files (FR-014).
#[must_use]
pub(super) fn handle_plugins_command(app: &mut crate::app::App, args: &str) -> Option<String> {
    let args = args.trim();
    // A bare `/plugins` renders usage (FR-014).
    if args.is_empty() {
        return Some(render_help(""));
    }

    let sub = subcommand_of(args);
    let rest = args.strip_prefix(sub).map(str::trim_start).unwrap_or("");

    // `/plugins codex` and `/plugins claude` open the browse panel (FR-002,
    // FR-007). The launch resolves the effective endpoint for the store at this
    // moment (NFR-002): a non-empty `plugins.stores.<name>.url` wins, otherwise
    // the compiled default applies. A config with no `plugins.stores` block
    // therefore still opens the panel and starts the default-endpoint fetch
    // rather than reporting a configuration error (FR-027, FR-029, FR-030).
    if let Some(kind) = StoreKind::from_token(sub) {
        // A disabled subsystem is inert: report and open no panel (SPEC
        // configuration schema).
        if !store_and_config(&app.cwd_path).1.is_enabled() {
            return Some(ragent_plugins::disabled_subsystem_report(sub));
        }
        let (query, refresh) = parse_store_launch(rest);
        app.open_plugin_store(kind, &query, refresh);
        return None;
    }

    // `/plugins stores --check` contacts each store endpoint to report
    // availability and catalogue size. The fetch must not block the UI thread,
    // so it is spawned off-loop and the finished report is appended by
    // `poll_plugin_store_probe_result` on a later frame; the acknowledgement
    // below renders immediately. A plain `/plugins stores` stays a pure config
    // read and falls through to the shared dispatch ladder unchanged.
    if sub == "stores" && ragent_plugins::stores_check_requested(rest) {
        app.begin_plugin_store_probe();
        return Some("From: /plugins stores\n\nChecking store availability...".to_string());
    }

    let known = PLUGIN_SUBCOMMANDS.contains(&sub);
    // `help` and any unrecognised subcommand both render the usage block
    // (FR-014); a bare `/plugins` is handled above.
    if sub == "help" || !known {
        return Some(render_help(sub));
    }

    let workdir = current_working_dir();
    // A plugin's `mcpServers` entry is a separate process, so the number of tools
    // it exposes is only known after it connects — never derivable from the
    // manifest. `--mcp` is meaningful on `list` only: gate on the subcommand
    // so `/plugins remove foo --mcp` still executes the removal rather than
    // silently printing the MCP inventory.
    if sub == "list" && parses_mcp_flag(rest) {
        return Some(render_mcp_tool_report(app));
    }
    // Seed the collision surface (FR-024) from the live session: built-in tool
    // names plus the `SLASH_COMMANDS` triggers, then run the shared dispatch
    // ladder (store / test / control). The control subcommands drive an ephemeral
    // session whose sandbox contexts are dropped on return.
    let report = run_plugin_subcommand(
        &workdir,
        sub,
        rest,
        app.session_processor
            .tool_registry
            .list()
            .into_iter()
            .collect(),
        crate::app::state::SLASH_COMMANDS
            .iter()
            .map(|c| c.trigger.to_string())
            .collect(),
        &live_mcp_tool_counts(app),
    )
    .unwrap_or_else(|| render_help(sub));
    Some(report)
}

/// The live MCP tool count per bridged server id, for the `/plugins list`
/// renderer's `mcp_tool_counts` map (FR-030).
///
/// The count can only come from the live client: a plugin manifest declares
/// which servers to start, never how many tools they advertise. `App::mcp_servers`
/// is the TUI's copy of that client state, keyed by the bridged id
/// (`<plugin-id>.<server>`) — the same key the renderer reports, so no lookup by
/// declared server name is needed. A server with no tools yet is absent, and the
/// renderer prints `?` (unknown), not `0`.
fn live_mcp_tool_counts(app: &crate::app::App) -> std::collections::BTreeMap<String, usize> {
    app.mcp_servers
        .iter()
        .filter(|server| !server.tools.is_empty())
        .map(|server| (server.id.clone(), server.tools.len()))
        .collect()
}

/// The `--mcp` flag, accepted anywhere in the argument tail.
fn parses_mcp_flag(args: &str) -> bool {
    args.split_whitespace().any(|token| token == "--mcp")
}

/// Render the live MCP tool inventory a `/plugins list --mcp` invocation prints.
///
/// The tool list comes from the session's live MCP client (`App::mcp_servers`,
/// kept current by `adopt_mcp_client_state`), keyed by bridged server id, so it
/// covers plugin-contributed servers (`<plugin-id>.<server>`) and configured
/// servers alike.
fn render_mcp_tool_report(app: &crate::app::App) -> String {
    let mut out = String::from("From: /plugins list --mcp\nMCP tool inventory:\n\n");
    if app.mcp_servers.is_empty() {
        out.push_str("  (no MCP servers configured)\n");
        out.push_str(
            "  Enabled plugins that declare an 'mcpServers' section are bridged at startup.\n",
        );
        return out;
    }
    for server in &app.mcp_servers {
        out.push_str(&format!(
            "  {:<24} {:<12} {} tool(s)\n",
            server.id,
            server.status,
            server.tools.len()
        ));
        for tool in &server.tools {
            out.push_str(&format!(
                "      {} -> {}\n",
                tool.name,
                crate::app::slash::mcp_ragent_tool_name(&server.id, &tool.name)
            ));
        }
    }
    let connected = app
        .mcp_servers
        .iter()
        .filter(|s| s.status == ragent_agent::mcp::McpStatus::Connected)
        .count();
    out.push_str(&format!(
        "\n{}/{} server(s) connected\n",
        connected,
        app.mcp_servers.len()
    ));
    out
}

/// Parse the optional trailing arguments of a store-browser launch
/// (`/plugins codex [query] [--refresh]`).
///
/// Returns the pre-filled search query (FR-020) and whether a cache-bypassing
/// re-fetch was requested (FR-021). `--refresh` is accepted anywhere and is
/// removed from the query tokens, so a pre-filled query may contain spaces.
#[must_use]
fn parse_store_launch(rest: &str) -> (String, bool) {
    let mut refresh = false;
    let mut query_tokens: Vec<&str> = Vec::new();
    for token in rest.split_whitespace() {
        if token == "--refresh" {
            refresh = true;
        } else {
            query_tokens.push(token);
        }
    }
    (query_tokens.join(" "), refresh)
}
