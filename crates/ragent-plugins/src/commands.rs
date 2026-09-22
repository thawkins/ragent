//! `/plugins add` and `/plugins remove` command glue (spec `plugins` T-011;
//! FR-007, FR-010).
//!
//! This module is the single entry point the command surfaces (the TUI
//! `/plugins` slash family and, later, `ragent plugins` CLI parity in T-017)
//! call for the two store-mutating subcommands. It parses the raw argument
//! text, invokes the store operation in [`crate::add`](mod@crate::add) /
//! [`crate::remove`](mod@crate::remove), and
//! returns the report string to print via [`crate::report`]. Keeping the
//! parse-and-run here means the two surfaces share one wording and one set of
//! guards.
//!
//! It also owns [`render_stores_report`], the `/plugins stores` report (spec
//! `pluginstores` T-017; FR-031): a pure config read that lists each store's
//! effective endpoint and whether it came from configuration or the compiled
//! default. The opt-in `--check` flag ([`probe_stores`]) additionally contacts
//! each store endpoint and reports whether it is available and how many plugins
//! it advertises; without the flag the report stays a pure config read.

use std::path::Path;

use crate::add::add;
use crate::help::attribution;
use crate::remove::remove;
use crate::report::{add_error_report, add_report, remove_error_report, remove_report};
use crate::store::StoreDirs;
use crate::store_fetch::FetchLimits;
use crate::store_index::StoreKind;
use crate::store_seam::StoreIndexFetcher;

/// A parsed store subcommand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreCommand {
    /// `add <source> [--force]`.
    Add {
        /// The source path or `https://` URL to install from.
        source: String,
        /// Overwrite an existing plugin with the same id.
        force: bool,
    },
    /// `remove <pluginid>`.
    Remove {
        /// The plugin id to uninstall.
        plugin_id: String,
    },
}

/// Why a store subcommand could not be parsed (malformed arguments — reported
/// as an `[err]` row that changes no state, per the SPEC error policy).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoreArgError {
    /// `add` was given no source.
    MissingAddSource,
    /// `remove` was given no plugin id.
    MissingRemoveId,
}

impl StoreArgError {
    /// Render the usage error for the offending subcommand.
    #[must_use]
    pub fn report(self, sub: &str) -> String {
        match self {
            Self::MissingAddSource => format!(
                "{}\n\n[err] Missing <source>.\n\n\
                 Usage: `/plugins add <source> [--force]`\n\
                 Source forms: a local directory, a local `.zip`/`.tar.gz` file, or an \
                 `https://` URL ending in `.zip`/`.tar.gz`.",
                attribution(sub)
            ),
            Self::MissingRemoveId => format!(
                "{}\n\n[err] Missing <pluginid>.\n\n\
                 Usage: `/plugins {sub} <pluginid>`",
                attribution(sub)
            ),
        }
    }
}

/// Parse `/plugins <sub> <args>` for the two store subcommands.
///
/// Returns `None` when `sub` is neither `add` nor `remove` (the caller falls
/// through to the other subcommands), `Some(Err(_))` for malformed arguments,
/// and `Some(Ok(_))` for a valid command. `--force` is accepted anywhere in the
/// `add` argument list; the remaining tokens joined by a single space form the
/// source (so a quoted path with spaces still resolves).
#[must_use]
pub fn parse_store_command(sub: &str, args: &str) -> Option<Result<StoreCommand, StoreArgError>> {
    match sub {
        "add" => {
            let mut force = false;
            let mut source_tokens: Vec<&str> = Vec::new();
            for token in args.split_whitespace() {
                if token == "--force" {
                    force = true;
                } else {
                    source_tokens.push(token);
                }
            }
            if source_tokens.is_empty() {
                return Some(Err(StoreArgError::MissingAddSource));
            }
            Some(Ok(StoreCommand::Add {
                source: source_tokens.join(" "),
                force,
            }))
        }
        "remove" => {
            let plugin_id = args.split_whitespace().next();
            match plugin_id {
                Some(id) => Some(Ok(StoreCommand::Remove {
                    plugin_id: id.to_string(),
                })),
                None => Some(Err(StoreArgError::MissingRemoveId)),
            }
        }
        _ => None,
    }
}

/// Run the `add`/`remove` store subcommand and return the report string to
/// print, or `None` when `sub` is not a store subcommand.
///
/// `dirs` are the resolved plugin store legs and `workdir` the working
/// directory used to resolve a relative `add` source. When the master switch
/// `plugins.enabled` is false the install/uninstall is refused and the disabled
/// subsystem is reported instead (SPEC configuration schema).
#[must_use]
pub fn run_store_command(
    config: &ragent_config::PluginsConfig,
    dirs: &StoreDirs,
    workdir: &Path,
    sub: &str,
    args: &str,
) -> Option<String> {
    let parsed = parse_store_command(sub, args)?;
    if !config.is_enabled() {
        return Some(crate::control::disabled_subsystem_report(sub));
    }
    let report = match parsed {
        Ok(StoreCommand::Add { source, force }) => match add(dirs, workdir, &source, force) {
            Ok(outcome) => add_report(&outcome),
            Err(err) => add_error_report(&err),
        },
        Ok(StoreCommand::Remove { plugin_id }) => match remove(dirs, &plugin_id) {
            Ok(outcome) => remove_report(&outcome),
            Err(err) => remove_error_report(&err),
        },
        Err(arg_err) => arg_err.report(sub),
    };
    Some(report)
}

/// Render the `/plugins stores` report: each store's effective endpoint and
/// whether it came from configuration or the compiled default (spec
/// `pluginstores` T-017; FR-031).
///
/// Both stores are always listed, even when only one is overridden, so the user
/// can confirm the out-of-the-box defaults (FR-027). A store whose endpoint is
/// refused by the `https` guard (FR-024, FR-029) is reported with an `[err]`
/// line naming the refusal rather than being omitted, so a misconfigured
/// non-`https` URL is visible. ASCII only, consistent with the other `/plugins`
/// reports.
#[must_use]
pub fn render_stores_report(stores: &ragent_config::PluginStoresConfig) -> String {
    render_stores_report_with_probes(stores, None)
}

/// The result of contacting one store endpoint (`/plugins stores --check`).
///
/// `Ok(count)` means the endpoint answered with a parseable index advertising
/// `count` plugins; `Err(detail)` means it could not be reached or parsed and
/// carries a short, ASCII cause to print on the report line. It never holds a
/// raw [`crate::store_fetch::StoreError`] so the report stays a plain string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreProbe {
    /// The store is available and advertises this many plugins.
    Available {
        /// Number of accepted catalogue entries.
        count: usize,
    },
    /// The store could not be reached or parsed; `detail` names the cause.
    Unavailable {
        /// Short cause, e.g. `store fetch timed out after 10000 ms`.
        detail: String,
    },
}

impl StoreProbe {
    /// Render the probe suffix appended to a store's report line.
    #[must_use]
    fn render(&self) -> String {
        match self {
            Self::Available { count } => format!(" [ok] available, {count} plugins"),
            Self::Unavailable { detail } => format!(" [err] unavailable: {detail}"),
        }
    }
}

/// Whether a `/plugins stores` invocation carries the `--check` flag, which
/// additionally contacts each store endpoint (`/plugins stores --check`).
///
/// The flag is accepted anywhere in the argument text (matching the
/// `--refresh`/`--force` convention of the other `/plugins` subcommands); an
/// unrecognised argument is ignored so a stray token still renders the plain
/// config report rather than failing.
#[must_use]
pub fn stores_check_requested(args: &str) -> bool {
    args.split_whitespace().any(|token| token == "--check")
}

/// Contact each store's effective endpoint and summarise availability and
/// catalogue size (`/plugins stores --check`).
///
/// Resolves each store's endpoint through the same provenance resolver the
/// report uses (a non-empty `plugins.stores.<name>.url` wins, otherwise the
/// compiled default; FR-027, FR-028), then fetches it once through `fetcher`
/// under `stores`' byte/time budget. A store whose endpoint is refused by the
/// `https` guard, or whose fetch fails, yields a contained
/// [`StoreProbe::Unavailable`]; no path panics. The returned slice is aligned
/// with [`StoreKind::ALL`].
///
/// Blocking: call this off the event loop on the TUI surface (the CLI is a
/// one-shot process and may call it synchronously).
#[must_use]
pub fn probe_stores(
    stores: &ragent_config::PluginStoresConfig,
    fetcher: &dyn StoreIndexFetcher,
) -> Vec<StoreProbe> {
    let limits = FetchLimits::from(stores);
    StoreKind::ALL
        .into_iter()
        .map(|kind| match kind.effective_endpoint(stores) {
            Ok(endpoint) => match fetcher.fetch_index(kind, &endpoint, &limits) {
                Ok(index) => StoreProbe::Available {
                    count: index.entries.len(),
                },
                Err(err) => StoreProbe::Unavailable {
                    detail: err.to_string(),
                },
            },
            Err(err) => StoreProbe::Unavailable {
                detail: err.to_string(),
            },
        })
        .collect()
}

/// [`render_stores_report`] with an optional per-store availability probe
/// appended to each line (`/plugins stores --check`).
///
/// `probes`, when present, is aligned with [`StoreKind::ALL`]; a missing or
/// short slice leaves the remaining lines as the plain config report.
#[must_use]
pub fn render_stores_report_with_probes(
    stores: &ragent_config::PluginStoresConfig,
    probes: Option<&[StoreProbe]>,
) -> String {
    let mut lines = vec![attribution("stores"), String::new()];
    for (i, kind) in StoreKind::ALL.into_iter().enumerate() {
        let probe = probes
            .and_then(|p| p.get(i))
            .map(StoreProbe::render)
            .unwrap_or_default();
        match kind.effective_endpoint_with_source(stores) {
            Ok((endpoint, source)) => lines.push(format!(
                "- {token}: [{tag}] {endpoint}{probe}",
                token = kind.token(),
                tag = source.tag(),
                endpoint = endpoint.as_str(),
            )),
            Err(err) => lines.push(format!(
                "- {token}: [err] {err}{probe}",
                token = kind.token()
            )),
        }
    }
    lines.join("\n")
}
