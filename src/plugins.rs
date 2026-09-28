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
//! - reports print to stdout, with the `From: /plugins …` TUI attribution
//!   header replaced by a plain `ragent plugins …` line and the `## /plugins`
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

/// The CLI usage block. Structurally mirrors the shared
/// [`render_help`] body (T-014, FR-014) but uses the non-TUI surface spelling
/// and the `## ragent plugins` title (FR-021).
const CLI_USAGE: &str = "\
## ragent plugins command reference

Manage third-party Codex and Claude Code/Desktop plugins loaded in a sandboxed
JavaScript runtime.

| Command | Arguments | Description |
|---|---|---|
| `ragent plugins list [--verbose]` | optional `--verbose` | List discovered plugins with state, contributions, and (with `--verbose`) telemetry counters. The `MCP` and `MCP Tools` columns give each plugin's MCP server count and the total tools those servers advertise (`?` until the server connects). |
| `ragent plugins add <source> [--force]` | required `source`, optional `--force` | Install a plugin and validate its manifest. It is enabled and loads at the next session start. |
| `ragent plugins remove <pluginid>` | required `pluginid` | Uninstall a plugin from the store. Refused while the plugin is enabled. |
| `ragent plugins enable <pluginid>` | required `pluginid` | Mark a plugin enabled, load it now, and register its tools and commands. |
| `ragent plugins disable <pluginid>` | required `pluginid` | Unload a plugin and deregister its tools and commands without deleting files. |
| `ragent plugins test <pluginid>` | required `pluginid` | Load a plugin in an isolated harness, invoke each contributed tool once, and report per-step results. |
| `ragent plugins stores` | optional `--check` | Report each store's effective endpoint and its source; add `--check` to also contact each store and report availability and plugin count. |
| `ragent plugins help` | none | Show this usage block. |

### Sources accepted by `ragent plugins add`

- a local directory containing a plugin manifest;
- a local `.zip` or `.tar.gz` package file;
- an `https://` URL pointing at a `.zip`/`.tar.gz` package (non-`https` URLs are refused).

Existing plugins with the same id are not overwritten unless `--force` is given.

The same operations are available in the TUI as the `/plugins` slash command.";

/// Entry point for `ragent plugins <args…>`, invoked from the CLI dispatcher.
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
        println!("{CLI_USAGE}");
        return Ok(());
    }

    let sub = subcommand_of(text);
    let rest = text[sub.len()..].trim_start();

    let known = PLUGIN_SUBCOMMANDS.contains(&sub);
    // `help` and any unrecognised subcommand both render the usage block.
    if sub == "help" || !known {
        println!("{CLI_USAGE}");
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
        let sub_for_fallback = sub_owned.clone();
        std::thread::spawn(move || run_plugin_subcommand_offline(&workdir, &sub_owned, &rest_owned))
            .join()
            .map_err(|_| anyhow::anyhow!("plugin store probe thread panicked"))?
            .unwrap_or_else(|| render_help(&sub_for_fallback))
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
/// `From: /plugins …` attribution becomes `ragent plugins …`, and the
/// `## /plugins` usage heading becomes `## ragent plugins`. The body is
/// otherwise printed verbatim so both surfaces stay in step.
#[must_use]
fn cli_body(report: &str) -> String {
    let rewritten = report
        .strip_prefix("From: /plugins")
        .map(|tail| format!("ragent plugins{tail}"))
        .unwrap_or_else(|| report.to_string());
    let rewritten = rewritten.replace("## /plugins", "## ragent plugins");
    format!("{rewritten}\n")
}
