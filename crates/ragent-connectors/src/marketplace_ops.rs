//! `/connectors marketplace <store> add|remove` source resolution and the
//! durable marketplace ledger (spec `connectors` FR-043).
//!
//! Ported from `ragent_plugins::marketplace_ops` (spec `pluginstores` FR-048).
//! This module keeps only what differs between the two subsystems - the
//! catalogue's marketplace document path - and calls the shared ledger in
//! [`ragent_surface::marketplace_ledger`] with the `connectors` root key.
//!
//! No network access happens here: a source is resolved syntactically and the
//! URL is recorded for the browser to fetch later.

use std::path::Path;

// `MarketplaceSource` and `MarketplaceSourceError` are shared with the
// `/plugins` marketplace family; re-export the single definition.
pub use ragent_surface::marketplace::{MarketplaceSource, MarketplaceSourceError};

/// The JSON root key this subsystem's ledger lives under.
const ROOT_KEY: &str = "connectors";

/// The Claude catalogue's marketplace document path (relative to a repository
/// root) - the `.claude-plugin/marketplace.json` shape the Claude plugin
/// marketplaces publish, reused so one repo layout serves both subsystems (spec
/// `pluginstores` FR-048).
pub const MARKETPLACE_DOC_PATH: &str = ".claude-plugin/marketplace.json";

/// Resolve a user-typed marketplace `source` (spec `connectors` FR-043).
///
/// Accepts `owner/repo`, `git@github.com:owner/repo.git`, an `https://` URL, or
/// a local directory path. Returns a [`MarketplaceSourceError`] for an empty or
/// unrecognisable source. Performs no network access; the URL is derived
/// syntactically and anchored to branch `main`.
pub fn resolve_source(
    source: &str,
    cwd: &Path,
) -> Result<MarketplaceSource, MarketplaceSourceError> {
    ragent_surface::marketplace::resolve_source(source, cwd, MARKETPLACE_DOC_PATH)
}

/// Add (or replace) a marketplace entry in the user-global
/// `connectors.stores.marketplaces` array (spec `connectors` FR-043).
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
/// user-global config (spec `connectors` FR-043).
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
