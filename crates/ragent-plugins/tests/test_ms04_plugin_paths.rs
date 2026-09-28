//! MS-04 (SECTASKS T-062) plugin path guards: the marketplace wrapper-directory
//! join must refuse a traversal-derived name.

use ragent_plugins::marketplace::materialize_at;
use std::path::PathBuf;

#[test]
fn materialize_at_refuses_a_parent_component_name() {
    let root = tempfile::tempdir().unwrap();
    let staged = root.path().join("staged");
    std::fs::create_dir_all(&staged).unwrap();
    std::fs::write(staged.join("README.md"), "x").unwrap();

    // A source ending in `:..` derives the name `..`; the join used to point at
    // the staging parent.
    let staging_root = root.path().to_path_buf();
    let out = materialize_at(staging_root.clone(), "..", Some("deadbeef"));
    assert_eq!(out, staging_root);

    // Nothing was materialised above the staging directory.
    assert!(!out.join(".claude-plugin").exists());
}

#[test]
fn materialize_at_refuses_absolute_and_multi_segment_names() {
    let root = tempfile::tempdir().unwrap();
    let staging_root = root.path().to_path_buf();
    for name in ["/etc", "a/b", "."] {
        let out = materialize_at(staging_root.clone(), name, Some("deadbeef"));
        assert_eq!(out, staging_root, "name {name:?} must be refused");
    }
}

#[test]
fn materialize_at_without_a_key_is_a_no_op() {
    let root: PathBuf = tempfile::tempdir().unwrap().path().to_path_buf();
    let out = materialize_at(root.clone(), "plugin", None);
    assert_eq!(out, root);
}
