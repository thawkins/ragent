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
use crate::store_index::{EndpointSource, StoreKind};
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

/// Why a store subcommand could not be parsed (malformed arguments - reported
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

/// Whether a `/plugins marketplace <args>` invocation is the `show` report
/// (`/plugins marketplace show`). The sub-subcommand is the first
/// whitespace-delimited token of the argument text (matching the
/// `subcommand_of` convention); a bare `/plugins marketplace` is not `show`
/// and falls through to the usage block.
#[must_use]
pub fn marketplace_show_requested(args: &str) -> bool {
    args.split_whitespace().next() == Some("show")
}

/// Whether a `/plugins marketplace <args>` invocation is a recognised marketplace
/// subcommand: the `show` report or a `<store> add|remove|list` operation
/// (spec `pluginstores` FR-047, FR-048). Anything else (including a bare
/// `marketplace`, an unknown `<store>`, or an unknown verb) falls through to the
/// usage block.
///
/// A recognised operation with malformed arguments (`<store> add` with no source,
/// `<store> remove` with no name) IS recognised here, so the dispatch ladder can
/// render its specific `[err]` usage report rather than the generic usage block.
#[must_use]
pub fn marketplace_command_requested(args: &str) -> bool {
    parse_marketplace_command(args).is_some()
}

/// A parsed `/plugins marketplace` sub-subcommand (spec `pluginstores` FR-047,
/// FR-048).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarketplaceCommand {
    /// `show` - list the configured marketplaces for both stores.
    Show,
    /// `<store> list` - list one store's configured marketplaces.
    List {
        /// The store whose marketplaces are listed.
        kind: StoreKind,
    },
    /// `<store> add <source> [--force]` - add an additional marketplace.
    Add {
        /// The store the marketplace extends.
        kind: StoreKind,
        /// The user-typed source.
        source: String,
        /// Replace an existing marketplace with the same resolved name.
        force: bool,
    },
    /// `<store> remove <name>` - drop an additional marketplace.
    Remove {
        /// The store the marketplace belongs to.
        kind: StoreKind,
        /// The marketplace display name to remove.
        name: String,
    },
}

/// Why a `/plugins marketplace <store> ...` invocation could not be parsed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarketplaceArgError {
    /// `add` was given no source.
    MissingAddSource,
    /// `remove` was given no marketplace name.
    MissingRemoveName,
}

impl MarketplaceArgError {
    /// Render the usage error for the offending sub-subcommand.
    #[must_use]
    pub fn report(self) -> String {
        format!(
            "{}\n\n[err] {}\n\nUsage: `/plugins marketplace <codex|claude> add <source> [--force]` \
             or `/plugins marketplace <codex|claude> remove <name>`",
            attribution("marketplace"),
            match self {
                Self::MissingAddSource => "Missing <source>.",
                Self::MissingRemoveName => "Missing <name>.",
            }
        )
    }
}

/// Parse `/plugins marketplace <args>` into a [`MarketplaceCommand`].
///
/// Returns `None` when the invocation is neither `show` nor a `<store>
/// add|remove|list` operation (the caller renders usage), `Some(Err(_))` for a
/// malformed `<store>` operation, and `Some(Ok(_))` otherwise. `--force` is
/// accepted anywhere in an `add` argument list; the remaining tokens joined by a
/// single space form the source (so a URL or path with no spaces still resolves).
#[must_use]
pub fn parse_marketplace_command(
    args: &str,
) -> Option<Result<MarketplaceCommand, MarketplaceArgError>> {
    let mut tokens = args.split_whitespace();
    let first = tokens.next()?;
    if first == "show" {
        return Some(Ok(MarketplaceCommand::Show));
    }
    let kind = StoreKind::from_token(first)?;
    let verb = tokens.next()?;
    match verb {
        "list" => Some(Ok(MarketplaceCommand::List { kind })),
        "add" => {
            let mut force = false;
            let mut source_tokens: Vec<&str> = Vec::new();
            for token in tokens {
                if token == "--force" {
                    force = true;
                } else {
                    source_tokens.push(token);
                }
            }
            if source_tokens.is_empty() {
                return Some(Err(MarketplaceArgError::MissingAddSource));
            }
            Some(Ok(MarketplaceCommand::Add {
                kind,
                source: source_tokens.join(" "),
                force,
            }))
        }
        "remove" => match tokens.next() {
            Some(name) => Some(Ok(MarketplaceCommand::Remove {
                kind,
                name: name.to_string(),
            })),
            None => Some(Err(MarketplaceArgError::MissingRemoveName)),
        },
        _ => None,
    }
}

/// Run a recognised `/plugins marketplace` sub-subcommand and return the report
/// string, or `None` when the invocation is not recognised (the caller renders
/// usage).
///
/// `show` and `<store> list` are pure config reads (FR-047). `add` and `remove`
/// edit the **user-global** `plugins.stores.marketplaces` array (FR-048) and
/// report the outcome; a malformed argument renders an `[err]` usage report that
/// changes no state. No network access happens here - the added URL is recorded
/// for the browser to fetch later.
#[must_use]
pub fn run_marketplace_command(args: &str) -> Option<String> {
    let parsed = match parse_marketplace_command(args)? {
        Ok(command) => command,
        Err(err) => return Some(err.report()),
    };
    match parsed {
        MarketplaceCommand::Show => {
            let stores = load_stores();
            Some(render_marketplace_show_report(&stores))
        }
        MarketplaceCommand::List { kind } => {
            let stores = load_stores();
            Some(render_marketplace_list_report(kind, &stores))
        }
        MarketplaceCommand::Add {
            kind,
            source,
            force,
        } => Some(run_marketplace_add(kind, &source, force)),
        MarketplaceCommand::Remove { kind, name } => Some(run_marketplace_remove(kind, &name)),
    }
}

/// Load the effective `plugins.stores` block from the live environment, falling
/// back to the compiled defaults when the config cannot be read.
fn load_stores() -> ragent_config::PluginStoresConfig {
    match ragent_config::Config::load() {
        Ok(config) => config.plugins.unwrap_or_default().stores_or_default(),
        Err(e) => {
            tracing::warn!(error = %e, "plugin config could not be loaded; using compiled defaults");
            ragent_config::PluginStoresConfig::default()
        }
    }
}

/// Render the `<store> list` report: the base endpoint plus every added
/// marketplace for one store (spec `pluginstores` FR-048).
#[must_use]
pub fn render_marketplace_list_report(
    kind: StoreKind,
    stores: &ragent_config::PluginStoresConfig,
) -> String {
    let mut out = format!(
        "{}\n\n## {} marketplace\n\n",
        attribution("marketplace"),
        kind.label()
    );
    out.push_str("| Name | Url | Description |\n|---|---|---|\n");
    let (endpoints, refused) = kind.effective_endpoints_with_source(stores);
    for (index, (endpoint, source)) in endpoints.iter().enumerate() {
        let name = if index == 0 && *source != EndpointSource::Added {
            kind.token().to_string()
        } else {
            marketplace_name_for(stores, kind, endpoint).unwrap_or_else(|| kind.token().to_string())
        };
        out.push_str(&format!(
            "| {name} | {} | {} plugin marketplace ({source}) |\n",
            endpoint.as_str(),
            kind.label(),
            source = source.tag(),
        ));
    }
    if let Some(err) = refused {
        out.push_str(&format!(
            "| {} | [err] {} | {} plugin marketplace (invalid) |\n",
            kind.token(),
            err,
            kind.label(),
        ));
    }
    out
}

/// Resolve and record one added marketplace, returning the report (FR-048).
fn run_marketplace_add(kind: StoreKind, source: &str, force: bool) -> String {
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let resolved = match crate::marketplace_ops::resolve_source(kind, source, &cwd) {
        Ok(resolved) => resolved,
        Err(err) => return format!("{}\n\n[err] {err}", attribution("marketplace")),
    };
    match crate::marketplace_ops::add_marketplace(
        kind.token(),
        &resolved.name,
        resolved.url.as_deref(),
        source,
        force,
    ) {
        Ok(true) => format!(
            "{}\n\n[ok] added {label} marketplace `{name}`{url}\n\
             It is written to your user-global config and used by the next `/plugins {token}` browse.",
            attribution("marketplace"),
            label = kind.label(),
            name = resolved.name,
            token = kind.token(),
            url = resolved
                .url
                .as_deref()
                .map(|u| format!("\nUrl: {u}"))
                .unwrap_or_else(|| "\n(local marketplace - not fetched remotely)".to_string()),
        ),
        Ok(false) => format!(
            "{}\n\n[err] a {label} marketplace named `{name}` already exists; pass `--force` to replace it.",
            attribution("marketplace"),
            label = kind.label(),
            name = resolved.name,
        ),
        Err(detail) => format!("{}\n\n[err] {detail}", attribution("marketplace")),
    }
}

/// Remove one added marketplace, returning the report (FR-048).
fn run_marketplace_remove(kind: StoreKind, name: &str) -> String {
    match crate::marketplace_ops::remove_marketplace(kind.token(), name) {
        Ok(true) => format!(
            "{}\n\n[ok] removed {label} marketplace `{name}`.",
            attribution("marketplace"),
            label = kind.label(),
        ),
        Ok(false) => format!(
            "{}\n\n[err] no {label} marketplace named `{name}` is configured.",
            attribution("marketplace"),
            label = kind.label(),
        ),
        Err(detail) => format!("{}\n\n[err] {detail}", attribution("marketplace")),
    }
}

/// Whether a `/plugins marketplace` invocation carries no sub-subcommand, so
/// the surfaces render usage for it as they do for a bare `/plugins`.
#[must_use]
pub fn marketplace_bare(args: &str) -> bool {
    args.trim().is_empty()
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

/// Render the `/plugins marketplace show` report: one section per store
/// (Codex, then Claude), each carrying a markdown table with one row per
/// marketplace endpoint the store is configured with (name, url, description).
///
/// It is a pure config read - no network, no session, no store access - so a
/// missing `plugins.stores` block still lists the compiled default marketplace
/// for each store, and a disabled subsystem does not block the report
/// (informational, like `/plugins stores` FR-031). Every additional marketplace
/// added through `/plugins marketplace <store> add` (FR-048) is listed after the
/// base endpoint, tagged `added`. A configured endpoint that is refused by the
/// `https` guard (FR-024, FR-029) renders an `[err]` row naming the refusal rather
/// than being omitted. ASCII only beyond the markdown table pipes, consistent with
/// the other `/plugins` reports.
#[must_use]
pub fn render_marketplace_show_report(stores: &ragent_config::PluginStoresConfig) -> String {
    let mut out = attribution("marketplace show");
    for kind in StoreKind::ALL {
        out.push_str(&format!("\n\n## {} marketplace\n\n", kind.label()));
        out.push_str("| Name | Url | Description |\n|---|---|---|\n");
        let (endpoints, refused) = kind.effective_endpoints_with_source(stores);
        // The first endpoint is the store's base (config override or compiled
        // default); any later ones are added marketplaces (FR-048).
        for (index, (endpoint, source)) in endpoints.iter().enumerate() {
            let (name, description) = if index == 0 && *source != EndpointSource::Added {
                (
                    kind.token().to_string(),
                    format!("{} plugin marketplace", kind.label()),
                )
            } else {
                (
                    marketplace_name_for(stores, kind, endpoint)
                        .unwrap_or_else(|| kind.token().to_string()),
                    format!("{} plugin marketplace", kind.label()),
                )
            };
            out.push_str(&format!(
                "| {name} | {} | {description} ({source}) |\n",
                endpoint.as_str(),
                source = source.tag(),
            ));
        }
        if let Some(err) = refused {
            out.push_str(&format!(
                "| {} | [err] {} | {} plugin marketplace (invalid) |\n",
                kind.token(),
                err,
                kind.label(),
            ));
        }
    }
    out
}

/// The display name recorded for the marketplace whose resolved URL is
/// `endpoint`, so the `show` table names an added marketplace the way the user
/// added it (FR-048).
fn marketplace_name_for(
    stores: &ragent_config::PluginStoresConfig,
    kind: StoreKind,
    endpoint: &crate::store_index::StoreEndpoint,
) -> Option<String> {
    stores
        .marketplaces
        .iter()
        .find(|entry| {
            entry.store.eq_ignore_ascii_case(kind.token())
                && entry.url.as_deref() == Some(endpoint.as_str())
        })
        .map(|entry| entry.name.clone())
}
