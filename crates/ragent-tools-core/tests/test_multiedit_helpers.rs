//! Integration tests for `ragent-tools-core` multiedit helpers.
//!
//! Relocated from the inline `#[cfg(test)]` module in `src/multiedit.rs`
//! (T-008 of the testconsolidate spec). The tests exercise `pub(crate)`
//! helpers (`resolve_path`, `format_strict_error`) and the public
//! `FindError` enum. The source module is re-imported via `#[path]`
//! (FR-008); `pub(crate)` items are visible because the `#[path]` module
//! becomes part of the test crate. Shims are provided for `super::replace`
//! and `super::{Tool,...}` references in `multiedit.rs`.

use ragent_tools_core::{Tool, ToolContext, ToolOutput};

mod replace {
    pub use ragent_tools_core::replace::{FindError, find_exact_replacement_range};
}

mod file_lock {
    pub use ragent_tools_core::file_lock::lock_file;
}

#[path = "../src/multiedit.rs"]
mod multiedit;

use multiedit::{format_strict_error, resolve_path};
use ragent_tools_core::replace::FindError;
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
fn format_strict_error_not_found_includes_edit_and_path() {
    let err = FindError::NotFound;
    let msg = format_strict_error(&err, 2, Path::new("/tmp/foo.rs"));
    assert!(
        msg.contains("Edit 2"),
        "msg should name the edit index: {msg}"
    );
    assert!(
        msg.contains("/tmp/foo.rs"),
        "msg should name the file: {msg}"
    );
    assert!(msg.contains("not found"), "msg should say not found: {msg}");
}

#[test]
fn format_strict_error_multiple_includes_count() {
    let err = FindError::MultipleMatches(3);
    let msg = format_strict_error(&err, 1, Path::new("/tmp/bar.rs"));
    assert!(
        msg.contains("Edit 1"),
        "msg should name the edit index: {msg}"
    );
    assert!(msg.contains("3 times"), "msg should name the count: {msg}");
    assert!(
        msg.contains("/tmp/bar.rs"),
        "msg should name the file: {msg}"
    );
}