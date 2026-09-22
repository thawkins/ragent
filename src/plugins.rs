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
| `ragent plugins list [--verbose]` | optional `--verbose` | List discovered plugins with state, contributions, and (with `--verbose`) telemetry counters. |
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
    // Seed the collision surface (FR-024) from the built-in tool registry and the
    // `SLASH_COMMANDS` triggers so it matches the live TUI surface, then run the
    // shared dispatch ladder (store / test / control).
    let registry = ragent_agent::tool::create_default_registry();
    let report = run_plugin_subcommand(
        &workdir,
        sub,
        rest,
        registry.list().into_iter().collect(),
        ragent_tui::app::SLASH_COMMANDS
            .iter()
            .map(|c| c.trigger.to_string())
            .collect(),
    )
    .unwrap_or_else(|| render_help(sub));
    print!("{}", cli_body(&report));
    Ok(())
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
