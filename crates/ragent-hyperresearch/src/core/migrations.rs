//! Schema migrations (FR-021).
//!
//! Migrations run before any state is read or written, so a vault written by a
//! newer build is detected rather than silently misread. The ledger is a small
//! TOML-free `key = value` file so it needs no schema of its own.

use std::path::Path;

use crate::core::error::{Error, Result};

/// File recording the applied schema version, relative to the vault root.
pub const MIGRATIONS_FILE: &str = ".schema-version";

/// Ordered migration steps; index + 1 is the schema version they produce.
///
/// Empty today: the vault format is at version 1. Adding a step here is how the
/// format advances, and the ledger makes an un-started vault the version-1 case.
pub const MIGRATIONS: &[&str] = &[];

/// Current schema version this build understands.
pub fn current_version() -> u32 {
    u32::try_from(MIGRATIONS.len()).unwrap_or(u32::MAX) + 1
}

/// Read the schema version recorded in the vault, defaulting to 1.
///
/// # Errors
///
/// Returns [`Error::Io`] when the ledger exists but cannot be read, or
/// [`Error::Config`] when it does not hold a number.
pub fn read_version(vault_root: &Path) -> Result<u32> {
    let path = vault_root.join(MIGRATIONS_FILE);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(1),
        Err(err) => return Err(Error::io(&path, err)),
    };

    text.trim().parse::<u32>().map_err(|_| {
        Error::Config(format!(
            "{} does not hold a schema version: {:?}",
            path.display(),
            text.trim()
        ))
    })
}

/// Bring the vault up to [`current_version`].
///
/// # Errors
///
/// Returns [`Error::Unsupported`] when the vault was written by a newer build —
/// downgrading state silently would corrupt it — and propagates I/O failures.
pub fn apply_pending(vault_root: &Path) -> Result<u32> {
    let mut version = read_version(vault_root)?;
    let target = current_version();

    if version > target {
        return Err(Error::Unsupported(format!(
            "vault schema version {version} is newer than this build understands ({target})"
        )));
    }

    while version < target {
        // Each step is applied in order; a step that fails leaves the ledger at
        // the last version that completed, so a retry resumes rather than
        // replays.
        version += 1;
        let path = vault_root.join(MIGRATIONS_FILE);
        std::fs::write(&path, format!("{version}\n")).map_err(|err| Error::io(&path, err))?;
    }

    Ok(version)
}
