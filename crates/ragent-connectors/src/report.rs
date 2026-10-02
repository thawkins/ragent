//! Report rendering for the `/connectors` command family (spec `connectors`
//! T-010; FR-006, FR-021, FR-023).
//!
//! The store operations live in [`crate::add`](mod@crate::add) and
//! [`crate::remove`](mod@crate::remove); this module turns their outcomes and
//! refusals into the message text the command surfaces print, so the TUI
//! (`/connectors` slash family) and the `ragent connectors` CLI parity (T-014)
//! share one wording. Every report carries the `From: /connectors <subcommand>`
//! attribution and an `[ok]`/`[err]` marker in the house style, mirroring
//! `ragent_plugins::report`.
//!
//! Secret values never appear in a report (FR-005, FR-014): an `add` report
//! names the connector's credential *requirement* (the shape, the credential or
//! environment-variable name, and any OAuth scopes; FR-023) but never a value.

use crate::add::AddError;
use crate::auth::AuthRequirement;
use crate::help::attribution;
use crate::manifest::StagedConnector;
use crate::remove::{RemoveError, RemoveOutcome};

/// Render the success report for `/connectors add` (FR-006, FR-011, FR-023).
///
/// Names the installed id, its display name and category, the directory it was
/// written to, and the enabled state (nothing connects until the next session
/// start or an explicit `/connectors connect`). The connector's declared
/// credential requirement is surfaced so the requirement is visible without
/// contacting the catalogue again (FR-023); the requirement describes the shape
/// and the credential name, never a secret.
#[must_use]
pub fn add_report(outcome: &StagedConnector) -> String {
    let d = &outcome.descriptor;
    let requirement = AuthRequirement::for_descriptor(d);
    let mut lines = vec![
        format!("{}\n", attribution("add")),
        format!(
            "[ok] Installed connector `{}` ({}, category: {}).",
            d.id,
            d.name,
            if d.category.is_empty() {
                "(none)"
            } else {
                d.category.as_str()
            }
        ),
        format!("Installed to `{}`.", outcome.installed_dir.display()),
        "The connector is recorded **enabled**; run `/connectors enable \
         <id>` after installing while a session is already running to connect \
         its servers now."
            .to_string(),
    ];
    if requirement.requires_credential() {
        lines.push(format!(
            "Credential requirement: {}",
            requirement.describe()
        ));
    }
    if !d.unsupported.is_empty() {
        lines.push(format!(
            "Unsupported capabilities recorded: {}",
            d.unsupported.join("; ")
        ));
    }
    if !d.source.is_empty() {
        lines.push(format!("Source: {}", d.source));
    }
    lines.join("\n")
}

/// Render the `[err]` report for a refused `/connectors add` (FR-006, FR-027).
#[must_use]
pub fn add_error_report(err: &AddError) -> String {
    err_report("add", err)
}

/// Render the success report for `/connectors remove` (FR-006).
#[must_use]
pub fn remove_report(outcome: &RemoveOutcome) -> String {
    format!(
        "{}\n\n[ok] Removed connector `{id}` from `{store}`.\n\
         Deleted `{dir}`.",
        attribution("remove"),
        id = outcome.id,
        store = outcome.store.display(),
        dir = outcome.dir.display(),
    )
}

/// Render the `[err]` report for a refused `/connectors remove`
/// (FR-006, FR-030).
#[must_use]
pub fn remove_error_report(err: &RemoveError) -> String {
    err_report("remove", err)
}

/// The shared `[err]` report shape: the subcommand attribution, a blank line, and
/// the error text.
fn err_report(sub: &str, err: &dyn std::fmt::Display) -> String {
    format!("{}\n\n[err] {err}", attribution(sub))
}

/// The `[err]` report shown by any non-`help` `/connectors` subcommand while the
/// master switch `connectors.enabled` is false (FR-021). No discovery, catalogue
/// fetch, or connection runs.
#[must_use]
pub fn disabled_subsystem_report(sub: &str) -> String {
    format!(
        "{}\n\n[err] Connector subsystem is disabled \
         (connectors.enabled = false); no connectors were discovered.",
        attribution(sub)
    )
}
