//! Inline tests for `mod.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::strip_code_fences;

#[test]
fn test_strip_code_fences_removes_bare_language_prefix() {
    assert_eq!(
        strip_code_fences("rust\npub fn answer() -> i32 {\n    42\n}"),
        "pub fn answer() -> i32 {\n    42\n}"
    );
}

#[test]
fn test_strip_code_fences_removes_fenced_language_prefix() {
    assert_eq!(
        strip_code_fences("```rust\npub fn answer() -> i32 {\n    42\n}\n```"),
        "pub fn answer() -> i32 {\n    42\n}"
    );
}
