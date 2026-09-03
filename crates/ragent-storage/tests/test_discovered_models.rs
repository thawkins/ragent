#![allow(clippy::assert_is_empty)]
//! Integration tests for `ragent-storage` discovered-models cache.
//!
//! Relocated from the inline `#[cfg(test)]` module in `src/storage.rs`
//! (T-004 of the testconsolidate spec, FR-004 — this crate previously had no
//! `tests/` directory).

use ragent_storage::Storage;

#[test]
fn test_discovered_models_round_trip() {
    let storage = Storage::open_in_memory().expect("in-memory storage");
    storage
        .set_discovered_models("gemini", r#"[{"id":"gemini-2.5-pro"}]"#)
        .expect("store models");

    let cached = storage
        .get_discovered_models("gemini")
        .expect("load models");
    assert_eq!(cached.as_deref(), Some(r#"[{"id":"gemini-2.5-pro"}]"#));
}

#[test]
fn test_delete_discovered_models_removes_entry() {
    let storage = Storage::open_in_memory().expect("in-memory storage");
    storage
        .set_discovered_models("copilot", r"[]")
        .expect("store models");
    storage
        .delete_discovered_models("copilot")
        .expect("delete models");

    assert!(
        storage
            .get_discovered_models("copilot")
            .expect("load models")
            .is_none()
    );
}
