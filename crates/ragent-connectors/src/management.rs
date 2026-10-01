//! `/connectors list` and `/connectors search` report wording (spec `connectors`
//! T-011; FR-009, FR-010, FR-025, FR-039, FR-041).
//!
//! The pure renderers live here so the TUI family and the `ragent connectors`
//! CLI parity (T-014) print one wording. No function in this module performs
//! network access, opens an MCP connection, or writes any file:
//!
//! - [`render_list`] prints one row per discovered connector (id, name, category,
//!   lifecycle state, auth state, and the bridged server and advertised-tool
//!   counts), the unsupported-capability notices recorded during validation
//!   (FR-025), the active category filter with its match count (FR-039, FR-041),
//!   and a totals summary line (FR-009). A connector absent from the lifecycle
//!   snapshot (a store scan performed without a live session) is shown with its
//!   store enable state as `disabled`/`enabled` and no live tool counts.
//! - [`render_search`] prints the matching catalogue entries with id, name,
//!   category, tags, and authentication requirement (FR-010). An empty result set
//!   is a message, never an error.
//!
//! Both resolve their header through the shared browser predicate
//! ([`resolve_filter`] over [`crate::browse::CategoryFilterState`]), so
//! `--category <name>` restricts the rows to one declared category (FR-041),
//! `ALL` clears any filter (FR-040), and an unknown category renders the
//! family's `[err]` row and changes no state. The browser and these text
//! reports therefore can never disagree about which connectors a category
//! shows.
//!
//! A discovery-time parse failure is reported as its own `[err]` row so a broken
//! manifest is visible instead of silently absent.

use std::collections::BTreeMap;

use crate::auth::AuthRequirement;
use crate::browse::{CategoryFilter, CategoryFilterState, build_categories};
use crate::descriptor::ConnectorDescriptor;
use crate::help::attribution;
use crate::lifecycle::{ConnectorLifecycleState, ConnectorStatus, ServerState};
use crate::store::{ScanFailure, StoreDirs, scan_dirs};

/// How many tools a bridged server advertises.
///
/// `Known(n)` is a live count adopted from the session's MCP client;
/// `Unknown` renders as `?` so a discovery-only render (no live session) is
/// never mistaken for a server that contributed nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolCount {
    /// The live count of tools the server advertises.
    Known(usize),
    /// No live session was available to count the server's tools.
    Unknown,
}

impl ToolCount {
    /// The count's rendering: the number, or `?` when unknown.
    #[must_use]
    pub fn render(self) -> String {
        match self {
            Self::Known(count) => count.to_string(),
            Self::Unknown => "?".to_string(),
        }
    }
}

/// One `/connectors list` row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListRow {
    /// The connector id.
    pub id: String,
    /// The display name (the id when the manifest failed to parse).
    pub name: String,
    /// The declared category (empty when none).
    pub category: String,
    /// The lifecycle state the row prints (FR-009).
    pub state: ConnectorLifecycleState,
    /// The authentication state label (FR-014, FR-022).
    pub auth: String,
    /// Number of MCP servers the connector declares.
    pub server_count: usize,
    /// Live tool count per bridged server id (`<connector-id>.<server>`).
    pub tool_counts: BTreeMap<String, ToolCount>,
    /// Unsupported-capability labels recorded during validation (FR-025).
    pub unsupported: Vec<String>,
    /// The failure cause when the state is `errored`.
    pub error: Option<String>,
}

impl ListRow {
    /// The total advertised tool count across every bridged server, or `None`
    /// when no live count is available.
    #[must_use]
    pub fn tools_total(&self) -> Option<usize> {
        if self.tool_counts.is_empty() {
            return None;
        }
        let mut total = 0usize;
        for count in self.tool_counts.values() {
            match count {
                ToolCount::Known(n) => total += n,
                ToolCount::Unknown => return None,
            }
        }
        Some(total)
    }
}

/// What the `/connectors list` renderer iterates: the discovered store scan plus
/// the session's live lifecycle snapshot, when one is available.
///
/// The store scan alone yields every installed connector with its persisted
/// enable flag; the snapshot overlays the live state (`connected` / `errored`)
/// and the per-server tool counts when a session has loaded the connectors.
#[derive(Debug, Clone, Copy)]
pub struct ListInput<'a> {
    /// The store directories to scan (FR-001).
    pub dirs: &'a StoreDirs,
    /// The live lifecycle snapshot, empty for a discovery-only render.
    pub statuses: &'a [ConnectorStatus],
}

/// Resolve the requested category filter against the categories a surface knows
/// (FR-039, FR-040, FR-041).
///
/// The shared browser state ([`CategoryFilterState::select_when_known`]) is the
/// one place a category is accepted or refused, so the text reports and the
/// browser agree: `ALL` (any case) and a blank value clear the filter, a value
/// matching a known category is applied case-insensitively, and a value naming
/// no known category is refused and changes no state.
///
/// Returns [`CategoryError::Unknown`] for a refused value so the caller can
/// render the family's `[err]` row.
///
/// # Errors
///
/// Returns [`CategoryError::Unknown`] when `requested` names a category the
/// surface does not know.
pub fn resolve_filter<'a>(
    requested: &CategoryFilter,
    known: impl IntoIterator<Item = &'a str>,
) -> Result<CategoryFilter, CategoryError> {
    let mut state = CategoryFilterState::new();
    for category in known {
        state.add_category(category);
    }
    state.sort_categories();
    if state.select_when_known(requested.label()) {
        Ok(state.filter().clone())
    } else {
        Err(CategoryError::Unknown(requested.label().to_string()))
    }
}

/// Why a `--category <name>` value could not be applied (FR-041).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CategoryError {
    /// The value names no category the surface knows.
    #[error("unknown category {0}")]
    Unknown(String),
}

impl CategoryError {
    /// Render the family's `[err]` row for this refusal.
    ///
    /// The report opens with the standard `From: /connectors <sub>` attribution
    /// and names the offending category exactly as given.
    #[must_use]
    pub fn report(&self, sub: &str) -> String {
        match self {
            Self::Unknown(category) => format!(
                "{}\n\n[err] Unknown category `{category}`.",
                attribution(sub)
            ),
        }
    }
}

/// Render the `/connectors list` report (FR-009, FR-025, FR-039, FR-041).
///
/// `requested` restricts the rows to one declared category; an empty match set
/// after filtering is a message, not an error. `verbose` appends the per-connector
/// bridged-server tool counts. Rendering scans manifests only (no MCP connection,
/// no network access, no ledger write) and its output is ASCII only and prefixed
/// `From: /connectors list` (FR-006).
///
/// # Errors
///
/// Returns [`CategoryError::Unknown`] when `requested` names no category that the
/// connector browser would offer for this scan (FR-041).
#[must_use]
pub fn render_list(
    input: &ListInput<'_>,
    requested: &CategoryFilter,
    verbose: bool,
) -> Result<String, CategoryError> {
    let statuses: BTreeMap<&str, &ConnectorStatus> = input
        .statuses
        .iter()
        .map(|status| (status.id.as_str(), status))
        .collect();

    let mut rows: Vec<ListRow> = Vec::new();
    let mut failures: Vec<(String, String)> = Vec::new();
    let scanned = scan_dirs(input.dirs.clone());
    let total = scanned.len();
    let known: Vec<String> = build_categories(
        scanned
            .iter()
            .filter_map(|connector| connector.outcome.as_ref().ok()),
        std::iter::empty(),
    );
    let filter = resolve_filter(requested, known.iter().map(std::string::String::as_str))?;
    for connector in scanned {
        match &connector.outcome {
            Ok(descriptor) => {
                if !filter.matches(descriptor) {
                    continue;
                }
                rows.push(row_for(
                    descriptor,
                    connector.enabled,
                    statuses.get(descriptor.id.as_str()).copied(),
                ));
            }
            Err(ScanFailure { error }) => {
                // On a parse failure the identity key is the directory name, so
                // the scan already computed the label the failure row needs.
                failures.push((connector.id, error.to_string()));
            }
        }
    }

    Ok(finish_list(rows, failures, &filter, total, verbose))
}

/// The shared `category <label> (matched of total)` header both the list report
/// and the browser render (FR-039, FR-040).
///
/// `CategoryFilterState::summary` delegates here so the report wording and the
/// browser wording cannot drift.
#[must_use]
pub fn category_header(filter: &CategoryFilter, matched: usize, total: usize) -> String {
    format!("category {} ({matched} of {total})", filter.label())
}

/// Render the body of the `/connectors list` report once the filter is resolved
/// (FR-009, FR-039, FR-041).
fn finish_list(
    rows: Vec<ListRow>,
    failures: Vec<(String, String)>,
    filter: &CategoryFilter,
    total: usize,
    verbose: bool,
) -> String {
    let mut lines = vec![attribution("list"), String::new()];
    lines.push(category_header(filter, rows.len(), total));
    if rows.is_empty() {
        lines.push(String::new());
        lines.push(empty_message(filter, total));
    } else {
        lines.push(String::new());
        for row in &rows {
            lines.push(row_line(row, verbose));
        }
    }

    let with_unsupported: Vec<&ListRow> = rows
        .iter()
        .filter(|row| !row.unsupported.is_empty())
        .collect();
    if !with_unsupported.is_empty() {
        lines.push(String::new());
        lines.push("Unsupported capabilities:".to_string());
        for row in with_unsupported {
            lines.push(format!("- {}: {}", row.id, row.unsupported.join("; ")));
        }
    }

    let with_errors: Vec<&ListRow> = rows.iter().filter(|row| row.error.is_some()).collect();
    if !with_errors.is_empty() {
        lines.push(String::new());
        lines.push("Errors:".to_string());
        for row in with_errors {
            lines.push(format!(
                "- {}: {}",
                row.id,
                row.error.as_deref().unwrap_or_default()
            ));
        }
    }

    if !failures.is_empty() {
        lines.push(String::new());
        lines.push("Unreadable connector directories:".to_string());
        for (dir, cause) in &failures {
            lines.push(format!("- {dir}: [err] {cause}"));
        }
    }

    lines.push(String::new());
    lines.push(summary_line(&rows, total));
    lines.join("\n")
}

/// Build one list row from a parsed descriptor, its persisted enable flag, and
/// the live status when the connector is tracked (FR-009).
fn row_for(
    descriptor: &ConnectorDescriptor,
    enabled: bool,
    status: Option<&ConnectorStatus>,
) -> ListRow {
    let (state, auth, error, tool_counts) = match status {
        Some(status) => {
            let auth = status.auth.label().to_string();
            let counts = status
                .servers
                .iter()
                .map(|server| {
                    (
                        server.server_id.clone(),
                        match server.state {
                            ServerState::Connected => ToolCount::Known(server.tools.len()),
                            ServerState::Disconnected | ServerState::Errored => ToolCount::Unknown,
                        },
                    )
                })
                .collect();
            (status.state, auth, status.error.clone(), counts)
        }
        None => {
            let state = if enabled {
                ConnectorLifecycleState::Enabled
            } else {
                ConnectorLifecycleState::Disabled
            };
            let auth = AuthRequirement::for_descriptor(descriptor);
            let auth = if auth.requires_credential() {
                "needs auth".to_string()
            } else {
                "none".to_string()
            };
            (state, auth, None, BTreeMap::new())
        }
    };
    ListRow {
        id: descriptor.id.as_str().to_string(),
        name: descriptor.name.clone(),
        category: descriptor.category.clone(),
        state,
        auth,
        server_count: descriptor.servers.len(),
        tool_counts,
        unsupported: descriptor.unsupported.clone(),
        error,
    }
}

/// One rendered list row: `- <id>: <name>; state <state>; auth <auth>; ...`.
fn row_line(row: &ListRow, verbose: bool) -> String {
    let category = if row.category.is_empty() {
        "(none)"
    } else {
        row.category.as_str()
    };
    let mut line = format!(
        "- {id}: {name}; category {category}; state {state}; auth {auth}; \
         {servers} server(s), {tools} tool(s)",
        id = row.id,
        name = if row.name.is_empty() {
            row.id.as_str()
        } else {
            row.name.as_str()
        },
        state = row.state.label(),
        auth = row.auth,
        servers = row.server_count,
        tools = match row.tools_total() {
            Some(total) => total.to_string(),
            None => "?".to_string(),
        },
    );
    if verbose && !row.tool_counts.is_empty() {
        let servers: Vec<String> = row
            .tool_counts
            .iter()
            .map(|(server_id, count)| format!("{server_id} ({})", count.render()))
            .collect();
        line.push_str(&format!(" [{}]", servers.join(", ")));
    }
    line
}

/// The message an unmatched or empty list renders: never an error (FR-009,
/// FR-041).
fn empty_message(filter: &CategoryFilter, total: usize) -> String {
    if total == 0 {
        "No connectors installed. Run `/connectors search <query>` then \
         `/connectors add <id>` to install one."
            .to_string()
    } else {
        format!("No connectors in {}.", category_header(filter, 0, total))
    }
}

/// The totals line (FR-009).
fn summary_line(rows: &[ListRow], total: usize) -> String {
    let count =
        |state: ConnectorLifecycleState| rows.iter().filter(|row| row.state == state).count();
    format!(
        "Total: {} connector(s) - {} connected, {} enabled, {} disabled, {} errored \
         ({} discovered).",
        rows.len(),
        count(ConnectorLifecycleState::Connected),
        count(ConnectorLifecycleState::Enabled),
        count(ConnectorLifecycleState::Disabled),
        count(ConnectorLifecycleState::Errored),
        total,
    )
}

/// Render the `/connectors search <query>` report (FR-010, FR-025, FR-041).
///
/// `catalogue` is the already-fetched catalogue (the caller owns the fetch and
/// the configured endpoint resolver); this renderer performs no I/O and no
/// network access, so the search report is exactly as deterministic as the
/// catalogue it was handed. Matching is a case-insensitive substring test across
/// the id, name, category, and tags; `requested` additionally restricts the
/// result to one declared category (FR-041). An empty result set is a message,
/// never an error (FR-010).
///
/// # Errors
///
/// Returns [`CategoryError::Unknown`] when `requested` names no category the
/// fetched catalogue declares (FR-041).
#[must_use]
pub fn render_search(
    query: &str,
    catalogue: &[ConnectorDescriptor],
    requested: &CategoryFilter,
) -> Result<String, CategoryError> {
    let filter = resolve_filter(
        requested,
        catalogue
            .iter()
            .map(|descriptor| descriptor.category.as_str()),
    )?;
    let needle = query.trim().to_lowercase();
    let mut lines = vec![attribution("search"), String::new()];
    lines.push(format!(
        "Query: \"{query}\"; category {} ({} catalogue entries).",
        filter.label(),
        catalogue.len()
    ));

    let matches: Vec<&ConnectorDescriptor> = catalogue
        .iter()
        .filter(|descriptor| filter.matches(descriptor))
        .filter(|descriptor| matches_query(descriptor, &needle))
        .collect();

    if matches.is_empty() {
        lines.push(String::new());
        lines.push(format!(
            "No catalogue connector matches \"{query}\" in category {}.",
            filter.label()
        ));
        return Ok(lines.join("\n"));
    }

    lines.push(String::new());
    for descriptor in &matches {
        lines.push(search_row(descriptor));
    }
    lines.push(String::new());
    lines.push(format!(
        "Total: {} matching connector(s) in category {}.",
        matches.len(),
        filter.label()
    ));
    Ok(lines.join("\n"))
}

/// One rendered search row: id, name, category, tags, and auth requirement
/// (FR-010). A descriptor with an unexpressible server carries its recorded
/// unsupported label so a browsable-but-uninstallable entry is visible (FR-025).
fn search_row(descriptor: &ConnectorDescriptor) -> String {
    let requirement = AuthRequirement::for_descriptor(descriptor);
    let tags = if descriptor.tags.is_empty() {
        "(none)".to_string()
    } else {
        descriptor.tags.join(", ")
    };
    let category = if descriptor.category.is_empty() {
        "(none)"
    } else {
        descriptor.category.as_str()
    };
    let mut line = format!(
        "- {id}: {name}; category {category}; tags {tags}; auth {auth}",
        id = descriptor.id.as_str(),
        name = if descriptor.name.is_empty() {
            descriptor.id.as_str()
        } else {
            descriptor.name.as_str()
        },
        auth = requirement.describe(),
    );
    if !descriptor.unsupported.is_empty() {
        line.push_str(&format!(
            "; unsupported {}",
            descriptor.unsupported.join("; ")
        ));
    }
    line
}

/// Whether a descriptor matches a search needle (FR-010).
///
/// The needle is already lowercased; the comparison is a case-insensitive
/// substring test across the id, name, category, and tags (the description is
/// not searched here). Shares one implementation with the browse panel's
/// matcher so the two cannot drift.
fn matches_query(descriptor: &ConnectorDescriptor, needle: &str) -> bool {
    crate::browse::descriptor_matches(descriptor, needle, false)
}
