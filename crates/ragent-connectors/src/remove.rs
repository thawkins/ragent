//! `/connectors remove` store operation (spec `connectors` T-009; FR-030).
//!
//! Uninstalls a connector by deleting its directory from the store it was
//! discovered in, then clearing its row from that store's `_state.json` ledger
//! so a later re-add starts from a clean slate. The connector's MCP
//! configuration is removed with its manifest, so no `/connectors enable` will
//! ever start a bridged server for it again.
//!
//! Guards (both refuse and change nothing):
//!
//! - an unknown connector id ([`RemoveError::UnknownConnector`]);
//! - a connector currently **enabled** ([`RemoveError::Enabled`] - "disable it
//!   first", FR-030). A removal while enabled would deregister a live server's
//!   tools out from under the session, so the caller refuses rather than
//!   silently unloading it.
//!
//! A disabled connector is inert (FR-018), so removing one touches only its
//! files.
//!
//! Modelled on `ragent_plugins::remove` so the two store-remove paths read the
//! same way.

use std::path::PathBuf;

use crate::error::ConnectorError;
use crate::store::{StoreDirs, StoreLedger, scan_dirs};

/// The outcome of a successful [`remove`].
#[derive(Debug)]
pub struct RemoveOutcome {
    /// The connector id that was removed.
    pub id: String,
    /// The connector directory that was deleted.
    pub dir: PathBuf,
    /// The store directory the connector lived in.
    pub store: PathBuf,
}

/// A refusal or failure from [`remove`].
///
/// Every variant is a contained, reportable condition; none panics and a
/// refusal changes no file (FR-030).
#[derive(Debug, thiserror::Error)]
pub enum RemoveError {
    /// No store contains a connector with this id.
    #[error("connector remove: unknown connector id {0}")]
    UnknownConnector(String),

    /// The connector is enabled; it must be disabled before removal (FR-030).
    #[error("connector remove: connector {0} is enabled; run `/connectors disable {0}` first")]
    Enabled(String),

    /// An I/O failure while deleting the connector directory or the ledger row.
    /// Uses [`ConnectorError`] so the underlying error's source chain survives
    /// (ANTIPAT M15).
    #[error("connector remove: {0}")]
    Io(#[from] ConnectorError),
}

/// Uninstall a connector from its store (FR-030).
///
/// Finds the connector across the configured stores (project wins on id
/// collision), refuses when it is enabled, then deletes its directory and
/// clears its ledger row.
///
/// # Errors
///
/// Returns [`RemoveError::UnknownConnector`] when no store holds `id`,
/// [`RemoveError::Enabled`] when the connector is enabled, or
/// [`RemoveError::Io`] when the directory cannot be deleted.
pub fn remove(dirs: &StoreDirs, id: &str) -> Result<RemoveOutcome, RemoveError> {
    let found = scan_dirs(dirs.clone())
        .into_iter()
        .find(|c| c.id == id)
        .ok_or_else(|| RemoveError::UnknownConnector(id.to_string()))?;

    if found.enabled {
        return Err(RemoveError::Enabled(id.to_string()));
    }

    std::fs::remove_dir_all(&found.dir).map_err(ConnectorError::io)?;

    // Drop the ledger row so a re-add starts clean; only rewrite the ledger when
    // the row was actually present, so an absent `_state.json` is not created as
    // a side effect of a removal. A ledger-write failure does not undo the
    // removal (the directory is already gone); it is logged instead.
    let mut ledger = StoreLedger::load(&found.store);
    if ledger.connectors.remove(id).is_some()
        && let Err(error) = ledger.save(&found.store)
    {
        tracing::warn!(
            connector = id,
            error = %error,
            "connector ledger row could not be cleared after remove"
        );
    }

    Ok(RemoveOutcome {
        id: id.to_string(),
        dir: found.dir,
        store: found.store,
    })
}
