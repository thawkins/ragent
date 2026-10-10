//! `ragent plugins` CLI parity surface (spec `plugins` T-017; FR-021).
//!
//! Mirrors the TUI `/plugins` slash family so the plugin store can be managed
//! from a shell without launching the TUI: `ragent plugins <sub> [args...]` for
//! the eight subcommands (`list`, `add`, `remove`, `enable`, `disable`, `test`,
//! `stores`, `help`). The parse-and-run logic lives entirely in the
//! `ragent-plugins` crate ([`ragent_plugins::run_store_command`],
//! [`ragent_plugins::run_control_command`], [`ragent_plugins::run_test_command`],
//! [`ragent_plugins::render_stores_report`], [`render_help`]); this module
//! supplies only the CLI-side glue:
//!
//! - the argument vector is joined and split into a subcommand token and the
//!   remaining text exactly as the TUI dispatch arm does;
//! - `list`/`enable`/`disable` drive an ephemeral [`PluginSession`] against a
//!   [`CliPluginSurface`] seeded from the built-in tool registry and the
//!   `SLASH_COMMANDS` triggers so collision rejection (FR-024) matches the live
//!   TUI surface;
//! - reports print to stdout, with the `From: /plugins ...` TUI attribution
//!   header replaced by a plain `ragent plugins ...` line and the `## /plugins`
//!   usage title rewritten to `## ragent plugins` (FR-021: plain text for the
//!   non-TUI surface).
//!
//! ## `!Send` sandbox contexts
//!
//! As on the TUI side, the plugin manager owns `rquickjs` contexts which are
//! `!Send`. The ephemeral [`PluginSession`] is created, used, and dropped
//! entirely inside one synchronous call in [`run_cli`]; it never crosses an
//! `.await` (the handler is invoked before the async dispatch reaches any
//! await, and is itself fully synchronous).

use anyhow::Result;

use ragent_plugins::{PLUGIN_SUBCOMMANDS, render_help, run_plugin_subcommand, subcommand_of};

/// Render the `ragent plugins` help block from the shared
/// [`ragent_plugins::render_help`] body.
///
/// Delegates to the `ragent-plugins` crate rather than hand-copying the usage
/// table (see `ANTIPAT.md` M3.10), then rewrites the TUI attribution/heading
/// for the non-TUI surface.
#[must_use]
fn cli_usage() -> String {
    cli_body(&render_help("help"))
}

/// Entry point for `ragent plugins <args...>`, invoked from the CLI dispatcher.
///
/// `args` is everything after the `plugins` verb (may be empty). Prints the
/// report for the requested subcommand to stdout and returns `Ok(())`; refusal
/// and usage cases render `[err]`/usage text rather than returning an error
/// (matching the TUI surface, which never propagates `/plugins` failures).
///
/// # Errors
///
/// Returns an error only when the current working directory cannot be resolved.
pub fn run_cli(args: &[String]) -> Result<()> {
    let joined = args.join(" ");
    let text = joined.trim();

    // A bare `ragent plugins` renders usage (FR-014 parity).
    if text.is_empty() {
        println!("{}", cli_usage());
        return Ok(());
    }

    let sub = subcommand_of(text);
    // Strip the typed prefix rather than slicing by `sub.len()`: the two agree
    // today, but a byte slice indexed by `sub` would panic if they ever diverged.
    // Mirrors the connectors CLI dispatch.
    let rest = text.strip_prefix(sub).map(str::trim_start).unwrap_or("");

    let known = PLUGIN_SUBCOMMANDS.contains(&sub);
    // `help` and any unrecognised subcommand both render the usage block.
    if sub == "help" || !known {
        println!("{}", cli_usage());
        return Ok(());
    }

    let workdir = std::env::current_dir()?;

    // `/plugins stores --check` performs the store-index fetch on the calling
    // thread. `fetch_index` builds a `reqwest::blocking::Client`, which spins up
    // and *drops* its own internal tokio runtime; dropping a runtime from inside
    // an async context panics ("Cannot drop a runtime in a context where blocking
    // is not allowed"), which is exactly what happened when this ran inside
    // `async_main`. Hand the whole subcommand off to a dedicated OS thread so the
    // blocking client lives and dies outside any async runtime. The other
    // subcommands are cheap local operations and run inline.
    let report = if sub == "stores" && ragent_plugins::stores_check_requested(rest) {
        let workdir = workdir.clone();
        let sub_owned = sub.to_string();
        let rest_owned = rest.to_string();
        std::thread::spawn(move || run_plugin_subcommand_offline(&workdir, &sub_owned, &rest_owned))
            .join()
            .map_err(|_| anyhow::anyhow!("plugin store probe thread panicked"))?
            .unwrap_or_else(|| render_help(sub))
    } else {
        run_plugin_subcommand_offline(&workdir, sub, rest).unwrap_or_else(|| render_help(sub))
    };
    print!("{}", cli_body(&report));
    Ok(())
}

/// Run the shared plugin dispatch ladder for the CLI surface.
///
/// Seeds the collision surface (FR-024) from the built-in tool registry and the
/// `SLASH_COMMANDS` triggers so it matches the live TUI surface. No MCP client is
/// connected for a one-shot CLI invocation, so a plugin's MCP tool counts stay
/// `?` (unknown, not zero): the count is only known from a live connection.
///
/// Returns `None` for subcommands the shared ladder does not own, so the caller
/// can render the CLI usage block.
#[must_use]
fn run_plugin_subcommand_offline(
    workdir: &std::path::Path,
    sub: &str,
    rest: &str,
) -> Option<String> {
    let registry = ragent_agent::tool::create_default_registry();
    run_plugin_subcommand(
        workdir,
        sub,
        rest,
        registry.list().into_iter().collect(),
        ragent_tui::app::SLASH_COMMANDS
            .iter()
            .map(|c| c.trigger.to_string())
            .collect(),
        &std::collections::BTreeMap::new(),
    )
}

/// Rewrite a shared `/plugins` report for the CLI surface (FR-021): the TUI
/// `From: /plugins ...` attribution becomes `ragent plugins ...`, the
/// `## /plugins` usage heading becomes `## ragent plugins`, and the usage-table
/// rows swap the `/plugins` trigger for `ragent plugins`. The body is
/// otherwise printed verbatim so both surfaces stay in step.
#[must_use]
fn cli_body(report: &str) -> String {
    crate::cli_surface::rewrite_report_body(report, "plugins")
}
