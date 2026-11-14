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
    u32::try_from(step_count()).unwrap_or(u32::MAX) + 1
}

/// Process-wide stand-in for the pending step count, set only by tests.
///
/// `MIGRATIONS` is a `&'static` list, so a test cannot add a step to make
/// [`apply_pending`] enter its migration loop the way a real migration would -
/// and that loop body is the only writer of the ledger. This override lets the
/// tests drive the write against a real vault instead of replaying it by hand.
static STEP_COUNT_OVERRIDE: std::sync::OnceLock<Option<usize>> = std::sync::OnceLock::new();

/// The step count [`apply_pending`] walks to: the override when a test installed
/// one, otherwise the real [`MIGRATIONS`] length.
fn step_count() -> usize {
    STEP_COUNT_OVERRIDE
        .get()
        .copied()
        .flatten()
        .unwrap_or(MIGRATIONS.len())
}

/// Lock serialising tests that call [`set_step_count_override`], so two of them
/// cannot interleave an override with an `apply_pending` run.
///
/// Held for the whole test rather than just the setter: the value has to stay
/// pinned while the migration run reads it.
#[doc(hidden)]
#[must_use]
pub fn step_count_override_mutex() -> &'static std::sync::Mutex<()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    &LOCK
}

/// Set (or with `None` clear) the pending step count [`apply_pending`] walks to.
///
/// Test-only: it exists so the migration loop - and with it the production
/// ledger write - can be exercised while [`MIGRATIONS`] is still empty. Callers
/// must hold [`step_count_override_mutex`] for as long as the value matters.
///
/// The override is write-once in practice: the first value installed stands, so
/// a second setter cannot change what an in-flight run sees.
#[doc(hidden)]
pub fn set_step_count_override(count: Option<usize>) {
    let _ = STEP_COUNT_OVERRIDE.set(count); // INTENTIONAL: write-once; a repeat keeps the first value
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
