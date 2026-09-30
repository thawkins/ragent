//! MS-04 (SECTASKS T-066) TUI source guards.
//!
//! The `/alog` run-id validation and the owner-only log-window spool live in
//! `src/app/` and are not reachable from a public test surface, so this guard
//! asserts on the source text the same way `test_block_in_place_guard.rs` does.

use std::path::PathBuf;

fn read(rel: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

#[test]
fn alog_run_id_is_character_validated() {
    let source = read("src/app/helpers.rs");
    assert!(
        source.contains("Invalid run id"),
        "parse_alog_run_id_yes must reject a run id that is not [A-Za-z0-9_-]+"
    );
}

#[test]
fn log_window_spool_is_owner_only_on_unix() {
    // ANTIPAT M3.15 / M6.9: the spool writers share
    // `app::helpers::open_owner_only`, which owns the 0600 create-and-tighten
    // logic, via the thin `open_log_spool` wrapper. Assert that the shared
    // helper re-asserts 0600, that `open_log_spool` delegates to it, and that
    // every spool writer routes through the wrapper rather than opening the
    // file directly.
    let helper = read("src/app/helpers.rs");
    assert!(
        helper.contains("Permissions::from_mode(0o600)") && helper.contains("OpenOptionsExt"),
        "the shared owner-only helper must set and re-assert 0600"
    );
    assert!(
        helper.contains("fn open_log_spool") && helper.contains("open_owner_only(path, false)"),
        "open_log_spool must delegate to open_owner_only"
    );
    let source = read("src/app/session_ops.rs");
    let calls = source.matches("open_log_spool").count();
    assert!(
        calls >= 2,
        "both spool writers must route through open_log_spool, found {calls}"
    );
}
