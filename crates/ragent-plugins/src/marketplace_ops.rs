//! `/plugins marketplace <store> add|remove` source resolution and the durable
//! marketplace ledger (spec `pluginstores` FR-048).
//!
//! This module owns the one piece the `marketplace` command family needs beyond
//! the pure report rendering in [`crate::commands`]: turning a user-typed
//! marketplace source into a display `name` and (when fetchable) an `https`
//! marketplace-index URL. Four source forms are accepted, mirroring the Claude
//! Code `/plugin marketplace add` prompt:
//!
//! - `owner/repo` (GitHub shorthand) - resolved to the store's raw marketplace
//!   document on branch `main`;
//! - `git@github.com:owner/repo.git` (SSH) - resolved the same way;
//! - `https://.../marketplace.json` (or a GitHub repo URL) - used directly;
//! - `./path/to/marketplace` (a local directory) - recorded as a local,
//!   non-fetchable marketplace (its `name` is read from the manifest when it is
//!   readable).
//!
//! The ledger edits themselves ([`add_marketplace`] / [`remove_marketplace`])
//! are the shared implementation in [`ragent_surface::marketplace_ledger`],
//! parameterised by the `plugins` root key; the `/connectors` family calls the
//! same functions with its own key. No network access happens here: a source is
//! resolved syntactically and the URL is recorded for the browser to fetch
//! later.

use std::path::Path;

use crate::store_index::StoreKind;

// `MarketplaceSource` and `MarketplaceSourceError` are shared with the
// `/connectors` marketplace family; re-export the single definition.
pub use ragent_surface::marketplace::{MarketplaceSource, MarketplaceSourceError};

/// The JSON root key this subsystem's ledger lives under.
const ROOT_KEY: &str = "plugins";

/// The store-specific marketplace document path (relative to a repository root).
#[must_use]
pub const fn marketplace_doc_path(store: StoreKind) -> &'static str {
    match store {
        StoreKind::Codex => ".agents/plugins/marketplace.json",
        StoreKind::Claude => ".claude-plugin/marketplace.json",
    }
}

/// Resolve a user-typed marketplace `source` for `store` (spec `pluginstores`
/// FR-048).
///
/// Accepts `owner/repo`, `git@github.com:owner/repo.git`, an `https://` URL, or a
/// local directory path. Returns a [`MarketplaceSourceError`] for an empty or
/// unrecognisable source. Performs no network access; the URL is derived
/// syntactically and anchored to branch `main`.
pub fn resolve_source(
    store: StoreKind,
    source: &str,
    cwd: &Path,
) -> Result<MarketplaceSource, MarketplaceSourceError> {
    ragent_surface::marketplace::resolve_source(source, cwd, marketplace_doc_path(store))
}

/// Add (or replace) a marketplace entry in the user-global
/// `plugins.stores.marketplaces` array (spec `pluginstores` FR-048).
///
/// Delegates to [`ragent_surface::marketplace_ledger::add_marketplace`].
///
/// # Errors
///
/// Returns a short message when the global config cannot be read, parsed, or
/// written.
pub fn add_marketplace(
    store_token: &str,
    name: &str,
    url: Option<&str>,
    source: &str,
    force: bool,
) -> Result<bool, String> {
    ragent_surface::marketplace_ledger::add_marketplace(
        ROOT_KEY,
        store_token,
        name,
        url,
        source,
        force,
    )
}

/// Remove the marketplace entry named `name` for `store_token` from the
/// user-global config (spec `pluginstores` FR-048).
///
/// Delegates to [`ragent_surface::marketplace_ledger::remove_marketplace`].
///
/// # Errors
///
/// Returns a short message when the global config cannot be read, parsed, or
/// written.
pub fn remove_marketplace(store_token: &str, name: &str) -> Result<bool, String> {
    ragent_surface::marketplace_ledger::remove_marketplace(ROOT_KEY, store_token, name)
}
