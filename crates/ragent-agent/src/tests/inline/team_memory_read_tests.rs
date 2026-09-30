//! Inline tests for `team_memory_read.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::path_tag;

#[test]
fn test_path_tag_normalisation() {
    assert_eq!(path_tag("MEMORY.md"), "path-memory-md");
    assert_eq!(path_tag("Notes / Decisions"), "path-notes-decisions");
    assert_eq!(path_tag("--weird__path!!"), "path-weird-path");
}
