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
    let source = read("src/app/session_ops.rs");
    let mentions = source.matches("Permissions::from_mode(0o600)").count();
    assert!(
        mentions >= 2,
        "both spool writers must re-assert 0600, found {mentions}"
    );
}
