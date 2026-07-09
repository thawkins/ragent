//! Integration tests for `ragent-tools-core` multiedit helpers.
//!
//! Relocated from the inline `#[cfg(test)]` module in `src/multiedit.rs`
//! (T-008 of the testconsolidate spec). The tests exercise `pub(crate)`
//! helpers (`resolve_batch_edit`) and the public `FindError` enum. The source
//! module is re-imported via `#[path]` (FR-008); `pub(crate)` items are
//! visible because the `#[path]` module becomes part of the test crate. Shims
//! are provided for `super::replace` and `super::{Tool,...}` references in
//! `multiedit.rs`.

use ragent_tools_core::{Tool, ToolContext, ToolOutput};

mod replace {
    pub use ragent_tools_core::replace::{
        FindDiag, FindError, find_batch_normalized_replacement_range, find_exact_replacement_range,
        format_match_failure,
    };
}

mod file_lock {
    pub use ragent_tools_core::file_lock::lock_file;
}

mod path_util {
    pub use ragent_tools_core::path_util::resolve_path;
}

#[path = "../src/multiedit.rs"]
mod multiedit;

use multiedit::resolve_batch_edit;
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
fn resolve_batch_edit_exact_match() {
    let content = "fn a() { 1 }\nfn b() { 2 }\n";
    let (s, e, effective) = resolve_batch_edit(content, "fn a() { 1 }", "fn a() { 10 }").unwrap();
    assert_eq!(&content[s..e], "fn a() { 1 }");
    assert_eq!(effective, "fn a() { 10 }");
}

#[test]
fn resolve_batch_edit_normalizes_crlf_and_trailing_space() {
    let content = "fn a() {  \r\n    bar  \r\n}\r\n";
    let needle = "fn a() {\n    bar\n}\n";
    let (s, e, effective) = resolve_batch_edit(content, needle, "fn a() {\n    baz\n}\n").unwrap();
    assert_eq!(&content[s..e], "fn a() {  \r\n    bar  \r\n}\r\n");
    assert_eq!(effective, "fn a() {\n    baz\n}\n");
}

#[test]
fn resolve_batch_edit_not_found_returns_diag() {
    let content = "fn a() { 1 }\n";
    let diag = resolve_batch_edit(content, "nonexistent", "x").unwrap_err();
    assert!(matches!(
        diag.kind,
        ragent_tools_core::replace::FindDiagKind::NotFound
    ));
    assert_eq!(diag.pass, "batch-normalized");
}

#[test]
fn resolve_batch_edit_multiple_matches_returns_diag() {
    let content = "dup\nmid\ndup\n";
    let diag = resolve_batch_edit(content, "dup", "DUP").unwrap_err();
    assert!(matches!(
        diag.kind,
        ragent_tools_core::replace::FindDiagKind::MultipleMatches(2)
    ));
}
