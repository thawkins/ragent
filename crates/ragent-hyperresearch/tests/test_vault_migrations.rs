//! FR-021: the vault schema ledger.
//!
//! Migrations run before any state is read, so the two behaviours that matter
//! are: a vault with no ledger is the version-1 case (an un-started vault must
//! not be rewritten), and a vault written by a *newer* build is refused rather
//! than silently downgraded. `MIGRATIONS` is empty today (the format is at
//! version 1), so `apply_pending` on a fresh vault writes the ledger once and
//! is idempotent afterwards. These tests pin the ledger file format too, since
//! `read_version` is what every later run parses.
//!
//! `apply_pending` deliberately writes the ledger only when a step actually
//! runs (the loop body owns the write), so with an empty `MIGRATIONS` list a
//! fresh vault stays ledger-less and reads back as version 1 either way. These
//! tests pin that, and pin the write the loop performs by replaying it. If a
//! future migration is added the "settled vault" assertions below should be
//! revisited to assert the ledger file directly.

use ragent_hyperresearch::core::Error;
use ragent_hyperresearch::core::migrations::{
    MIGRATIONS, MIGRATIONS_FILE, apply_pending, current_version, read_version,
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
fn test_replayed_step_writes_the_ledger_in_the_readable_format() {
    // The migration loop writes `{version}\n` to the ledger for each step it
    // applies. With no step to apply today, replay that one line by hand so a
    // change to the ledger format (which `read_version` parses on every run)
    // fails here rather than on a user's vault after an upgrade.
    let root = scratch("replayed-step");
    for version in [2u32, 3] {
        std::fs::write(root.join(MIGRATIONS_FILE), format!("{version}\n")).expect("step write");
        assert_eq!(
            std::fs::read_to_string(root.join(MIGRATIONS_FILE)).expect("ledger"),
            format!("{version}\n"),
            "the ledger is the bare version number and a newline"
        );
        assert_eq!(read_version(&root).expect("ledger parses"), version);
    }
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
