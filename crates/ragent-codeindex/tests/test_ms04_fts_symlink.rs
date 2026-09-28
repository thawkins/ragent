//! MS-04 (SECTASKS T-060) codeindex guard: the FTS recovery path must not
//! delete files through a symlinked index directory.

#[cfg(unix)]
#[test]
fn fts_open_refuses_a_symlinked_index_directory() {
    use ragent_codeindex::search::FtsIndex;
    use std::os::unix::fs::symlink;

    let root = tempfile::tempdir().unwrap();
    let victim = root.path().join("victim");
    std::fs::create_dir_all(&victim).unwrap();
    std::fs::write(victim.join("keep.txt"), "important").unwrap();

    let index_dir = root.path().join("fts");
    symlink(&victim, &index_dir).unwrap();

    let result = FtsIndex::open(&index_dir);
    assert!(result.is_err(), "a symlinked FTS dir must be refused");

    // The link target was untouched.
    assert!(victim.join("keep.txt").exists());
}
