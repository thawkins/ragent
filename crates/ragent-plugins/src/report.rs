//! Report rendering for the `/plugins add` and `/plugins remove` store
//! operations (spec `plugins` T-011; FR-007, FR-010).
//!
//! The store operations live in [`crate::add`](mod@crate::add) and
//! [`crate::remove`](mod@crate::remove); this
//! module turns their outcomes and refusals into the message text the command
//! surfaces print, so the TUI (`/plugins` slash family) and the CLI
//! (`ragent plugins`, T-017) share one wording. Every report carries the
//! `From: /plugins <subcommand>` attribution and an `[ok]`/`[err]` marker in
//! the house style (mirroring `SpecCommand::build_*_message`).

use crate::add::{AddError, AddOutcome};
use crate::help::attribution;
use crate::remove::{RemoveError, RemoveOutcome};

/// Render the success report for `/plugins add` (FR-007): the installed id,
/// dialect, and version, plus the reminder that the plugin is disabled until
/// explicitly enabled.
#[must_use]
pub fn add_report(outcome: &AddOutcome) -> String {
    let d = &outcome.parsed.descriptor;
    format!(
        "{}\n\n[ok] Installed plugin `{id}` (dialect: {dialect}, version: {version}).\n\
         Installed to `{dir}`.\n\
         The plugin is **disabled** until you run `/plugins enable {id}`.",
        attribution("add"),
        id = d.id,
        dialect = d.dialect,
        version = d.version,
        dir = outcome.installed_dir.display(),
    )
}

/// Render the `[err]` report for a refused `/plugins add` (FR-010, FR-026).
#[must_use]
pub fn add_error_report(err: &AddError) -> String {
    format!("{}\n\n[err] {err}", attribution("add"))
}

/// Render the success report for `/plugins remove` (FR-010).
#[must_use]
pub fn remove_report(outcome: &RemoveOutcome) -> String {
    format!(
        "{}\n\n[ok] Removed plugin `{id}` from `{store}`.\n\
         Deleted `{dir}`.",
        attribution("remove"),
        id = outcome.id,
        store = outcome.store.display(),
        dir = outcome.dir.display(),
    )
}

/// Render the `[err]` report for a refused `/plugins remove` (FR-010, FR-026).
#[must_use]
pub fn remove_error_report(err: &RemoveError) -> String {
    format!("{}\n\n[err] {err}", attribution("remove"))
}
