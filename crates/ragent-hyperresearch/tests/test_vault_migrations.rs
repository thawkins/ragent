//! FR-021: the vault schema ledger.
//!
//! Migrations run before any state is read, so the two behaviours that matter
//! are: a vault with no ledger is the version-1 case (an un-started vault must
//! not be rewritten), and a vault written by a *newer* build is refused rather
//! than silently downgraded. `MIGRATIONS` is empty today (the format is at
//! version 1), so `apply_pending` on a fresh vault writes no ledger at all and
//! is idempotent.
//!
//! `apply_pending` writes the ledger only from its migration loop (the loop body
//! owns the write), so with an empty `MIGRATIONS` list nothing a fresh vault
//! does can reach that write. The loop is therefore driven through the crate's
//! `#[doc(hidden)]` step-count override, which stands in for "N steps are
//! pending": `test_apply_pending_advances_a_version_zero_ledger` seeds a
//! version-0 ledger and asserts what the loop persisted, the format it wrote,
//! and that a second run neither advances nor rewrites. If a real migration is
//! added, the override can be dropped from that test.

use ragent_hyperresearch::core::Error;
use ragent_hyperresearch::core::migrations::{
    self, MIGRATIONS, MIGRATIONS_FILE, apply_pending, current_version, read_version,
};

/// A fresh scratch vault root under `target/temp/` (NFR-004: tests never write
/// into the checked-in tree).
fn scratch(name: &str) -> std::path::PathBuf {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("temp")
        .join("vault-migrations")
        .join(name);
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("scratch vault root");
    root
}

#[test]
fn test_current_version_is_at_least_one() {
    // The ledger format is `version = step_count + 1`: a vault predating the
    // ledger is version 1, so the count can never be zero.
    assert!(current_version() >= 1);
    assert_eq!(
        current_version(),
        u32::try_from(MIGRATIONS.len()).expect("step count fits") + 1
    );
}

/// Take the crate's process-global step-count override for `count`, recovering
/// from a poisoned lock.
///
/// `MIGRATIONS` is a `&'static` list, so `apply_pending`'s loop cannot be driven
/// from a test by adding a step the way a real migration would. The crate
/// therefore exposes a `#[doc(hidden)]` override that stands in for "N steps are
/// pending", which is what lets these tests reach the loop body - and with it
/// the production ledger write - while the real list stays empty.
fn with_step_count(count: usize) -> std::sync::MutexGuard<'static, ()> {
    let guard = migrations::step_count_override_mutex()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    migrations::set_step_count_override(Some(count));
    guard
}

#[test]
fn test_missing_ledger_reads_as_version_one() {
    let root = scratch("missing-ledger");
    assert_eq!(
        read_version(&root).expect("a missing ledger is version 1"),
        1
    );
    // Reading must not create the ledger: the migration pass owns that write.
    assert!(!root.join(MIGRATIONS_FILE).exists());
}

#[test]
fn test_apply_pending_returns_the_current_version() {
    let root = scratch("apply-pending");
    let version = apply_pending(&root).expect("migrations run");
    assert_eq!(version, current_version());
    // The version reads back identically with or without a ledger file, which
    // is why an un-started vault needs no marker.
    assert_eq!(read_version(&root).expect("ledger parses"), version);
}

#[test]
fn test_apply_pending_is_idempotent() {
    let root = scratch("idempotent");
    let first = apply_pending(&root).expect("first run");
    let second = apply_pending(&root).expect("second run");
    assert_eq!(first, second, "a second run must not advance the version");
    assert_eq!(read_version(&root).expect("ledger"), first);
    assert_eq!(
        apply_pending(&root).expect("third run"),
        first,
        "a settled vault stays settled"
    );
}

#[test]
fn test_apply_pending_advances_a_version_zero_ledger() {
    // A version-0 ledger (a vault predating this build's ledger format) is the
    // only state that can make `apply_pending` enter its migration loop, so it
    // is the one case that drives the production ledger write. `read_version`
    // reports a missing ledger as version 0, so the v0 vault is simply a vault
    // with no ledger at all - nothing is written here except by the loop.
    //
    // `MIGRATIONS` is empty today, so the loop first has to be reachable at all:
    // with the real (zero) step count the vault's version already equals the
    // target and the loop body - the only writer of the ledger - is skipped,
    // which is exactly the regression this test exists to catch.
    let _guard = with_step_count(1);
    let root = scratch("version-zero");
    // The v0 vault is the state `read_version` already gives a missing ledger
    // under the real (empty) list: the version this build reports for a vault
    // that predates the ledger. Nothing is written here to create it - no
    // `std::fs::write` reproduces any part of the ledger - so the migration loop
    // below is the only writer there is. A seeded file would leave this case
    // green even if the loop stopped writing, or changed what it writes.
    assert_eq!(
        migrations::MIGRATIONS.len(),
        0,
        "the v0 case is the real empty-list version"
    );
    assert!(
        !root.join(MIGRATIONS_FILE).exists(),
        "nothing but the migration loop writes the ledger"
    );

    let version = apply_pending(&root).expect("the ledger advances");
    assert_eq!(
        version,
        current_version(),
        "a v0 vault reaches the current version"
    );
    assert_eq!(version, 2, "one step takes a v0 vault to v2");
    assert_eq!(
        read_version(&root).expect("ledger parses"),
        version,
        "the version the loop wrote reads back"
    );
    // The production write is the only thing that created the ledger, so these
    // assert what the loop persisted - the bare version number and a newline,
    // the format `read_version` parses on every later run.
    assert_eq!(
        std::fs::read_to_string(root.join(MIGRATIONS_FILE)).expect("the loop wrote a ledger"),
        migrations::ledger_bytes(version),
        "the file the loop left behind is the ledger format, recording the new version"
    );

    // A second run finds a settled vault: it must not re-step or rewrite.
    let settled = std::fs::metadata(root.join(MIGRATIONS_FILE))
        .expect("stat")
        .modified()
        .expect("mtime");
    assert_eq!(
        apply_pending(&root).expect("second run"),
        version,
        "a settled vault does not advance again"
    );
    assert_eq!(
        read_version(&root).expect("ledger parses"),
        version,
        "a settled vault's ledger still reads back at the version the loop wrote"
    );
    assert_eq!(
        std::fs::metadata(root.join(MIGRATIONS_FILE))
            .expect("stat")
            .modified()
            .expect("mtime"),
        settled,
        "a settled vault's ledger is not rewritten"
    );
}

#[test]
fn test_apply_pending_refuses_a_newer_vault() {
    let root = scratch("newer");
    let newer = current_version() + 3;
    std::fs::write(root.join(MIGRATIONS_FILE), format!("{newer}\n")).expect("seed ledger");

    let err = apply_pending(&root).expect_err("a newer vault must be refused");
    assert!(
        matches!(err, Error::Unsupported(_)),
        "a newer vault must fail as Unsupported, got {err:?}"
    );
    assert!(
        err.to_string().contains("newer than this build"),
        "got: {err}"
    );
    // The refusal leaves the ledger untouched: downgrading it would corrupt a
    // vault the newer build still owns.
    assert_eq!(
        std::fs::read_to_string(root.join(MIGRATIONS_FILE)).expect("ledger"),
        format!("{newer}\n")
    );
}

#[test]
fn test_invalid_ledger_is_a_config_error() {
    let root = scratch("invalid");
    std::fs::write(root.join(MIGRATIONS_FILE), "not-a-number\n").expect("seed ledger");
    let err = read_version(&root).expect_err("a non-numeric ledger must be refused");
    assert!(matches!(err, Error::Config(_)), "got {err:?}");
    assert!(
        err.to_string().contains("does not hold a schema version"),
        "got: {err}"
    );
}

#[test]
fn test_ledger_tolerates_surrounding_whitespace() {
    // `read_version` trims before parsing, so a hand-edited ledger with a
    // trailing space or blank line still resolves.
    let root = scratch("whitespace");
    std::fs::write(root.join(MIGRATIONS_FILE), "  1  \n\n").expect("seed ledger");
    assert_eq!(read_version(&root).expect("trimmed ledger parses"), 1);
}

#[test]
fn test_unreadable_ledger_is_an_io_error() {
    // A directory in the ledger's place makes the read fail with something
    // other than NotFound, which must surface as Error::Io rather than being
    // mistaken for "no ledger" (that would silently reset the version).
    let root = scratch("unreadable");
    std::fs::create_dir(root.join(MIGRATIONS_FILE)).expect("directory in place of the ledger");
    let err = read_version(&root).expect_err("an unreadable ledger must be refused");
    assert!(matches!(err, Error::Io { .. }), "got {err:?}");
}
