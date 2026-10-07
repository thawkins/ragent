//! Inline tests for `panic_hook.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;

#[test]
fn test_panic_log_path_format() {
    let dir = PathBuf::from("target/temp/test-log");
    let path = panic_log_path(&dir);
    let name = path.file_name().unwrap().to_string_lossy();
    assert!(
        name.starts_with("panic-"),
        "expected panic- prefix, got {name}"
    );
    assert!(name.ends_with(".log"), "expected .log suffix, got {name}");
}

#[test]
fn test_panics_dir_under_cwd() {
    let dir = panics_dir();
    assert!(dir.ends_with("panics"));
}
