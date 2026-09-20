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
    PLUGIN_SUBCOMMANDS, PluginSession, ScratchSurface, render_help, run_control_command,
    run_store_command, run_test_command, store_and_config, subcommand_of,
};

use crate::app::helpers::current_working_dir;

/// Dispatch a `/plugins <args>` invocation, returning the report to print in
/// the message window. `args` is the text after the `/plugins` trigger.
///
/// `app` is borrowed only to read the live tool-name set for collision
/// rejection (FR-024); nothing is stored on it.
///
/// A bare `/plugins`, `/plugins help`, and an unrecognised subcommand all
/// render the usage block and create no files (FR-014).
#[must_use]
pub(super) fn handle_plugins_command(app: &crate::app::App, args: &str) -> String {
    let args = args.trim();
    // A bare `/plugins` renders usage (FR-014).
    if args.is_empty() {
        return render_help("");
    }

    let sub = subcommand_of(args);
    let rest = args[sub.len()..].trim_start();

    let known = PLUGIN_SUBCOMMANDS.contains(&sub);
    // `help` and any unrecognised subcommand both render the usage block
    // (FR-014); a bare `/plugins` is handled above.
    if sub == "help" || !known {
        return render_help(sub);
    }

    let (dirs, config) = store_and_config(&current_working_dir());

    // Store subcommands (add / remove) never need a live session.
    if let Some(report) = run_store_command(&config, &dirs, &current_working_dir(), sub, rest) {
        return report;
    }

    // The isolated test harness must not touch any live session (FR-013).
    if let Some(report) = run_test_command(dirs.clone(), &config, sub, rest) {
        return report;
    }

    // Control subcommands (list / enable / disable) drive a live session for
    // the duration of this call; the sandbox contexts are dropped on return.
    // Seed the collision surface from the live session: built-in tool names
    // plus the `SLASH_COMMANDS` triggers (FR-024).
    let mut surface = ScratchSurface::seeded(
        app.session_processor
            .tool_registry
            .list()
            .into_iter()
            .collect(),
        crate::app::state::SLASH_COMMANDS
            .iter()
            .map(|c| c.trigger.to_string())
            .collect(),
    );
    let mut session = PluginSession::start(dirs, config, &mut surface);
    run_control_command(&mut session, &mut surface, sub, rest).unwrap_or_else(|| render_help(sub))
}
