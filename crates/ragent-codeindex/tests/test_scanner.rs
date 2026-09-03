#![allow(clippy::assert_is_empty)]
//! External integration tests for the code-index scanner utilities.

use ragent_codeindex::scanner::{count_lines, detect_language, hash_content, is_binary};
use std::path::Path;

#[test]
fn test_detect_language_rust() {
    assert_eq!(
        detect_language(Path::new("src/main.rs")),
        Some("rust".to_string())
    );
}

#[test]
fn test_detect_language_python() {
    assert_eq!(
        detect_language(Path::new("app.py")),
        Some("python".to_string())
    );
}

#[test]
fn test_detect_language_unknown() {
    assert_eq!(detect_language(Path::new("README")), None);
}

#[test]
fn test_hash_content_deterministic() {
    let h1 = hash_content(b"hello world");
    let h2 = hash_content(b"hello world");
    assert_eq!(h1, h2);
    assert_ne!(h1, hash_content(b"different content"));
}

#[test]
fn test_is_binary() {
    assert!(is_binary(b"hello\x00world"));
    assert!(!is_binary(b"hello world\n"));
}

#[test]
fn test_count_lines() {
    assert_eq!(count_lines(b"line1\nline2\nline3\n"), 3);
    assert_eq!(count_lines(b"no newline"), 0);
}
