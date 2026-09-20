//! `/plugins help` usage text and command-surface metadata (spec `plugins`
//! T-014; FR-006, FR-014).
//!
//! This module owns the word-for-word usage block the surfaces render for
//! `help`, a bare `/plugins`, or an unrecognised subcommand (FR-014), plus the
//! subcommand list the autocomplete menu is seeded from (FR-006). Keeping it
//! here means the TUI (T-014) and the `ragent plugins` CLI parity (T-017) share
//! one help text and one exit condition, mirroring the parse-and-run glue in
//! [`crate::commands`] and [`crate::control`].
//!
//! Rendering help creates or modifies no files (FR-014): it is a pure string
//! function with no store access.

/// The `/plugins` subcommand tokens, in display order: the five management
/// subcommands, the `test` harness, and `help` (FR-006). Used to seed the
/// autocomplete menu and asserted against the usage block.
pub const PLUGIN_SUBCOMMANDS: [&str; 7] =
    ["list", "add", "remove", "enable", "disable", "test", "help"];

/// The usage block body (everything after the `From:` attribution line).
///
/// Documents every subcommand, its arguments, and the accepted `<source>`
/// forms (FR-014). ASCII only.
const USAGE_BODY: &str = "\
## /plugins command reference

Manage third-party Codex and Claude Code/Desktop plugins loaded in a sandboxed
JavaScript runtime.

| Command | Arguments | Description |
|---|---|---|
| `/plugins list [--verbose]` | optional `--verbose` | List discovered plugins with state, contributions, and (with `--verbose`) telemetry counters. |
| `/plugins add <source> [--force]` | required `source`, optional `--force` | Install a plugin and validate its manifest. It stays disabled until enabled. |
| `/plugins remove <pluginid>` | required `pluginid` | Uninstall a plugin from the store. Refused while the plugin is enabled. |
| `/plugins enable <pluginid>` | required `pluginid` | Mark a plugin enabled, load it now, and register its tools and commands. |
| `/plugins disable <pluginid>` | required `pluginid` | Unload a plugin and deregister its tools and commands without deleting files. |
| `/plugins test <pluginid>` | required `pluginid` | Load a plugin in an isolated harness, invoke each contributed tool once, and report per-step results. |
| `/plugins help` | none | Show this usage block. |

### Sources accepted by `/plugins add`

- a local directory containing a plugin manifest;
- a local `.zip` or `.tar.gz` package file;
- an `https://` URL pointing at a `.zip`/`.tar.gz` package (non-`https` URLs are refused).

Existing plugins with the same id are not overwritten unless `--force` is given.";

/// The `From: /plugins <subcommand>` attribution line every report renders
/// (FR-006, FR-014).
///
/// Centralised so the header format is defined once across the store, control,
/// and harness report builders; `sub` is the invoked subcommand (or empty for a
/// bare `/plugins`).
#[must_use]
pub fn attribution(sub: &str) -> String {
    let sub = sub.trim();
    if sub.is_empty() {
        "From: /plugins".to_string()
    } else {
        format!("From: /plugins {sub}")
    }
}

/// Render the `/plugins` usage block (FR-014). `sub` is the invoked subcommand
/// (or nothing for a bare `/plugins`). Rendering performs no I/O and creates no
/// files.
#[must_use]
pub fn render_help(sub: &str) -> String {
    format!("{}\n\n{USAGE_BODY}", attribution(sub))
}

/// The subcommand token from a `/plugins <args>` invocation: the first
/// whitespace-delimited word of `args`, or `""` when none is present.
///
/// A bare `/plugins` is intercepted before this is called (the caller treats
/// empty args as usage, FR-014); the dispatch arm renders usage when the token
/// is `help` or anything it does not recognise.
#[must_use]
pub fn subcommand_of(args: &str) -> &str {
    args.split_whitespace().next().unwrap_or("")
}
