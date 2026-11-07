//! NFR-001: a note id is a file name, so any id that could address another
//! directory must be refused before it reaches a `join`.
//!
//! `validate_id` in `core::vault` is private, so these drive it through the
//! public surface that calls it - `Vault::note_path` (and its two siblings
//! `note_assets_dir` / `ensure_available`) - which is exactly the path a caller
//! takes. Every case asserts the specific rejection reason too, so a future
//! edit that widened the check (or narrowed it to the wrong branch) fails here
//! rather than silently accepting a traversal.

use ragent_hyperresearch::core::vault::Vault;

/// Build a vault under `target/temp/`, per NFR-004 (tests never write into the
/// checked-in tree).
fn vault(name: &str) -> Vault {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("temp")
        .join("vault-id")
        .join(name);
    let _ = std::fs::remove_dir_all(&root);
    Vault::open(&root).expect("vault must open")
}

/// Assert `note_path` refuses `id` and names `reason_fragment`.
fn assert_refused(v: &Vault, id: &str, reason_fragment: &str) {
    let err = v
        .note_path(id)
        .expect_err(&format!("id {id:?} must be refused"));
    let message = err.to_string();
    assert!(
        matches!(err, ragent_hyperresearch::core::Error::InvalidNoteId { .. }),
        "id {id:?} must fail as InvalidNoteId, got {err:?}"
    );
    assert!(
        message.contains(reason_fragment),
        "id {id:?} must be refused for {reason_fragment:?}, got {message:?}"
    );
}

#[test]
fn test_empty_id_is_refused() {
    let v = vault("empty");
    assert_refused(&v, "", "must not be empty");
}

#[test]
fn test_parent_directory_component_is_refused() {
    let v = vault("dotdot");
    // `..` trips the leading-dot rule before the `..` rule; either rejection is
    // correct, so accept whichever names it (the id must never be joined).
    let err = v.note_path("..").expect_err("`..` must be refused");
    assert!(
        matches!(err, ragent_hyperresearch::core::Error::InvalidNoteId { .. }),
        "`..` must fail as InvalidNoteId, got {err:?}"
    );
    let message = err.to_string();
    assert!(
        message.contains("must not start with a dot") || message.contains("must not contain `..`"),
        "`..` must be refused for a dot reason, got {message:?}"
    );
}

#[test]
fn test_id_with_embedded_dotdot_is_refused() {
    let v = vault("embedded-dotdot");
    // `..` in the middle is as dangerous as a bare `..`: `a/../../../etc/passwd`
    // style ids would escape if the separator check ever let them through.
    assert_refused(&v, "a..b", "must not contain `..`");
}

#[test]
fn test_forward_slash_is_refused() {
    let v = vault("slash");
    assert_refused(&v, "sub/note", "must not contain a path separator");
}

#[test]
fn test_backslash_is_refused() {
    let v = vault("backslash");
    assert_refused(&v, "sub\\note", "must not contain a path separator");
}

#[test]
fn test_absolute_path_is_refused() {
    let v = vault("absolute");
    // An absolute path contains a separator, so the separator rule catches it;
    // the point is that it can never be joined onto the root.
    assert_refused(&v, "/etc/passwd", "must not contain a path separator");
}

#[test]
fn test_leading_dot_is_refused() {
    let v = vault("leading-dot");
    assert_refused(&v, ".hidden", "must not start with a dot");
}

#[test]
fn test_over_long_id_is_refused() {
    let v = vault("long");
    let id = "a".repeat(129);
    assert_refused(&v, &id, "at most 128 characters");
}

#[test]
fn test_non_ascii_id_is_refused() {
    let v = vault("unicode");
    // Homoglyph and normalization tricks (U+FF0F FULLWIDTH SOLIDUS, U+2215
    // DIVISION SLASH) look like separators to a human and to some filesystems.
    assert_refused(&v, "note\u{2215}escape", "ASCII alphanumeric");
    assert_refused(&v, "caf\u{e9}", "ASCII alphanumeric");
}

#[test]
fn test_nul_and_control_characters_are_refused() {
    let v = vault("control");
    assert_refused(&v, "note\0id", "ASCII alphanumeric");
    assert_refused(&v, "note\nid", "ASCII alphanumeric");
}

#[test]
fn test_valid_id_is_accepted_and_stays_inside_the_root() {
    let v = vault("valid");
    let path = v.note_path("alpha-1_beta.v2").expect("id must be accepted");
    assert_eq!(
        path.file_name().and_then(|n| n.to_str()),
        Some("alpha-1_beta.v2.md")
    );
    assert!(
        path.starts_with(v.root()),
        "note path {} must stay inside {}",
        path.display(),
        v.root().display()
    );
    assert_eq!(
        path.parent(),
        Some(v.root()),
        "a valid id must join directly onto the root"
    );
}

#[test]
fn test_id_at_the_length_limit_is_accepted() {
    let v = vault("at-limit");
    let id = "a".repeat(128);
    let path = v
        .note_path(&id)
        .expect("128 characters is the documented limit");
    assert_eq!(path.parent(), Some(v.root()));
}

#[test]
fn test_asset_and_conflict_paths_apply_the_same_rule() {
    let v = vault("siblings");
    // Every public entry point that builds a path from an id must refuse the
    // same ids; a single one that forgot `validate_id` would be the hole.
    assert!(v.note_assets_dir("../escape").is_err());
    assert!(v.ensure_note_assets_dir("../escape").is_err());
    assert!(v.ensure_available("../escape").is_err());
    assert!(v.note_assets_dir("ok-id").is_ok());
}
