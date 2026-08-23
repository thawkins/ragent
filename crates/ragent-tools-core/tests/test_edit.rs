//! Integration tests for `ragent-tools-core` edit helpers.
//!
//! Relocated from the inline `#[cfg(test)]` module in `src/edit.rs`
//! (T-008 of the testconsolidate spec). The tests exercise `pub(crate)`
//! helpers (`byte_offset_to_line`, `build_snippet`).
//! The source module is re-imported via `#[path]` (FR-008); `pub(crate)`
//! items are visible because the `#[path]` module becomes part of the
//! test crate. Shims are provided for `super::replace` and `super::{Tool,...}`
//! references in `edit.rs`.

use ragent_tools_core::{
    CanonicalPathCache, Tool, ToolContext, ToolOutput, check_path_within_root,
    check_path_within_root_cached,
};

mod replace {
    pub(crate) use ragent_tools_core::replace::{
        CascadeFail, CascadeMatch, FindDiag, FindError, MatchLane, decode_escapes,
        disambiguation_hint, find_flexible_replacement_range, find_replacement_cascade,
        format_match_failure, length_note, not_found_hint,
    };
}

mod file_lock {
    pub(crate) use ragent_tools_core::file_lock::lock_file;
}

mod path_util {
    pub(crate) use ragent_tools_core::path_util::resolve_path;
}

#[path = "../src/edit_common.rs"]
mod edit_common;

mod edit_log {
    pub(crate) use ragent_tools_core::edit_log::{
        EntryExtras, log_edit_operation, log_edit_operation_ex,
    };
}

#[path = "../src/edit.rs"]
#[allow(unreachable_pub)] // public items are reachable from the lib target, not this test target
mod edit;

use edit::{build_snippet, byte_offset_to_line};
use ragent_tools_core::path_util::resolve_path;
use std::path::{Path, PathBuf};

#[test]
fn resolve_path_relative() {
    let p = resolve_path(Path::new("/work"), "src/main.rs");
    assert_eq!(p, PathBuf::from("/work/src/main.rs"));
}

#[test]
fn resolve_path_absolute() {
    let p = resolve_path(Path::new("/work"), "/etc/hosts");
    assert_eq!(p, PathBuf::from("/etc/hosts"));
}

#[test]
fn byte_offset_to_line_basic() {
    let content = "line1\nline2\nline3\n";
    assert_eq!(byte_offset_to_line(content, 0), 1);
    assert_eq!(byte_offset_to_line(content, 6), 2);
    assert_eq!(byte_offset_to_line(content, 12), 3);
}

#[test]
fn build_snippet_includes_context_and_marker() {
    let content = "l1\nl2\nl3\nl4\nl5\nl6\nl7\nl8\nl9\nl10\n";
    let snippet = build_snippet(content, 12, 14);
    assert!(
        snippet.contains("   1  l1"),
        "should include context before: {snippet}"
    );
    assert!(
        snippet.contains("   5> l5"),
        "should mark the changed line: {snippet}"
    );
    assert!(
        snippet.contains("   9  l9"),
        "should include context after: {snippet}"
    );
    assert!(
        !snippet.contains("l10"),
        "should clamp after context: {snippet}"
    );
}

#[test]
fn build_snippet_clamps_to_file_start() {
    let content = "l1\nl2\nl3\nl4\nl5\n";
    let snippet = build_snippet(content, 0, 2);
    assert!(
        snippet.contains("   1> l1"),
        "should mark line 1: {snippet}"
    );
    assert!(
        snippet.contains("   5  l5"),
        "should include up to line 5: {snippet}"
    );
}
