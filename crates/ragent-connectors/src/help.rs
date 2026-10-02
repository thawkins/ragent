//! `/connectors` usage text and command-surface metadata (spec `connectors`
//! T-010, T-012; FR-004, FR-006, FR-017).
//!
//! This module owns the attribution line every report carries (FR-006), the
//! usage block the surfaces render for `help`, a bare `/connectors`, or an
//! unrecognised subcommand (FR-017), the subcommand list the autocomplete menu
//! is seeded from (FR-004), and the autocomplete token list itself. Keeping it
//! here means the TUI and the `ragent connectors` CLI parity (T-014) share one
//! wording and one exit condition, mirroring `ragent_plugins::help`.
//!
//! Rendering help is a pure string function with no store access and creates no
//! files (FR-017).

/// The `/connectors` subcommand tokens, in display order (FR-004). Used to seed
/// the autocomplete menu, to recognise a known subcommand, and to document the
/// family in the usage block.
pub const CONNECTOR_SUBCOMMANDS: [&str; 12] = [
    "list",
    "claude",
    "add",
    "remove",
    "enable",
    "disable",
    "connect",
    "disconnect",
    "auth",
    "test",
    "stores",
    "help",
];

/// The flag tokens the `/connectors` family accepts, in display order: the
/// filters (`--verbose`, `--category`), the install guard (`--force`), the
/// store probe (`--check`), and the browser re-fetch (`--refresh`) (FR-004,
/// FR-041).
pub const CONNECTOR_FLAGS: [&str; 5] =
    ["--verbose", "--category", "--force", "--check", "--refresh"];

/// The autocomplete-menu token list for `/connectors`: every subcommand
/// followed by every flag (FR-004).
///
/// Seeded from [`CONNECTOR_SUBCOMMANDS`] and [`CONNECTOR_FLAGS`] rather than a
/// hand-maintained list, so the menu cannot drift from the usage block:
/// `CONNECTOR_SUBCOMMANDS` lists exactly the subcommands `render_help`
/// documents, so documenting a new subcommand automatically seeds it here.
#[must_use]
pub fn autocomplete_tokens() -> Vec<String> {
    CONNECTOR_SUBCOMMANDS
        .iter()
        .chain(CONNECTOR_FLAGS.iter())
        .map(|token| (*token).to_string())
        .collect()
}

/// The usage block body (everything after the `From:` attribution line).
///
/// Documents every subcommand, its arguments, and the accepted `<source>` forms
/// (FR-004, FR-017). ASCII only. A bare `/connectors`, `/connectors help`, and
/// an unrecognised subcommand all fall through to this block.
const USAGE_BODY: &str = "\
## /connectors command reference

Manage connectors: named integrations (Google Drive, Slack, GitHub, ...) that
reach an external system through one or more MCP servers.

| Command | Arguments | Description |
|---|---|---|
| `/connectors list [--verbose] [--category <name>]` | optional | List installed connectors with state, auth state, category, and bridged server and tool counts. `--category` filters by category. |
| `/connectors claude [query] [--category <name>] [--refresh]` | optional | Open the interactive Claude connector-catalogue browser (filter, browse, and install with one key). An optional `query` pre-fills the search field, `--category` pre-filters by category, and `--refresh` bypasses the catalogue cache. |
| `/connectors add <id|source> [--force]` | required `id|source` | Install a connector. It is recorded enabled; `/connectors enable` connects it now in a running session. |
| `/connectors remove <id>` | required `id` | Uninstall a connector. Refused while the connector is enabled. |
| `/connectors enable <id>` | required `id` | Enable a connector and connect its servers now. |
| `/connectors disable <id>` | required `id` | Disable a connector and disconnect its servers. |
| `/connectors connect <id>` | required `id` | Connect an enabled connector's servers without a restart. |
| `/connectors disconnect <id>` | required `id` | Disconnect a connector's servers, leaving it enabled. |
| `/connectors auth <id>` | required `id` | Manage a connector's credential and report its auth state. |
| `/connectors test <id>` | required `id` | Connect in isolation, invoke one tool, and report per-step results. |
| `/connectors stores [--check]` | optional `--check` | Report each catalogue endpoint and its source; `--check` probes it. |
| `/connectors help` | none | Show this usage block. |

### Sources accepted by `/connectors add`

- a catalogue connector id;
- a local directory holding a connector manifest;
- a local `.zip` or `.tar.gz` package;
- an `https://` URL naming a `.zip`/`.tar.gz` package.

A newly installed connector is recorded enabled; run `/connectors enable <id>` to
connect its servers now in a session that is already running.";

/// The `From: /connectors <subcommand>` attribution line every report renders
/// (FR-006).
///
/// Centralised so the header format is defined once across the store, control,
/// and stores report builders; `sub` is the invoked subcommand (or empty for a
/// bare `/connectors`).
#[must_use]
pub fn attribution(sub: &str) -> String {
    let sub = sub.trim();
    if sub.is_empty() {
        "From: /connectors".to_string()
    } else {
        format!("From: /connectors {sub}")
    }
}

/// Render the `/connectors` usage block (FR-017). `sub` is the invoked
/// subcommand (or nothing for a bare `/connectors`). Rendering performs no I/O
/// and creates no files.
#[must_use]
pub fn render_help(sub: &str) -> String {
    format!("{}\n\n{USAGE_BODY}", attribution(sub))
}

/// The subcommand token from a `/connectors <args>` invocation: the first
/// whitespace-delimited word of `args`, or `""` when none is present.
///
/// A bare `/connectors` is intercepted before this is called (the caller treats
/// empty args as usage, FR-017); the dispatcher renders usage when the token is
/// `help` or anything it does not recognise.
#[must_use]
pub fn subcommand_of(args: &str) -> &str {
    args.split_whitespace().next().unwrap_or("")
}
