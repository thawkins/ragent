//! External tests for `ragent_core::reference::parse`.

use ragent_core::reference::parse::{FileRef, parse_refs};

#[test]
fn test_parse_simple_file_ref() {
    let refs = parse_refs("check @src/main.rs please");
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].raw, "src/main.rs");
    assert!(matches!(refs[0].kind, FileRef::File(_)));
}

#[test]
fn test_parse_directory_ref() {
    let refs = parse_refs("look at @src/");
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].raw, "src/");
    assert!(matches!(refs[0].kind, FileRef::Directory(_)));
}

#[test]
fn test_parse_url_ref() {
    let refs = parse_refs("see @https://example.com/page");
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].raw, "https://example.com/page");
    assert!(matches!(refs[0].kind, FileRef::Url(_)));
}

#[test]
fn test_parse_fuzzy_ref() {
    let refs = parse_refs("check @main");
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].raw, "main");
    assert!(matches!(refs[0].kind, FileRef::Fuzzy(_)));
}

#[test]
fn test_parse_email_excluded() {
    let refs = parse_refs("email user@example.com");
    assert!(refs.is_empty(), "emails should not be parsed as refs");
}

#[test]
fn test_parse_multiple_refs() {
    let refs = parse_refs("compare @src/main.rs with @Cargo.toml");
    assert_eq!(refs.len(), 2);
    assert_eq!(refs[0].raw, "src/main.rs");
    assert_eq!(refs[1].raw, "Cargo.toml");
}

#[test]
fn test_parse_no_refs() {
    let refs = parse_refs("no references here");
    assert!(refs.is_empty());
}

#[test]
fn test_parse_ref_at_start() {
    let refs = parse_refs("@README.md is the file");
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].raw, "README.md");
}

#[test]
fn test_parse_ref_at_end() {
    let refs = parse_refs("read @Cargo.toml");
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].raw, "Cargo.toml");
}

#[test]
fn test_parse_ref_with_tilde() {
    let refs = parse_refs("check @~/config.toml");
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].raw, "~/config.toml");
    assert!(matches!(refs[0].kind, FileRef::File(_)));
}

#[test]
fn test_parse_ref_span_positions() {
    let input = "look at @src/lib.rs end";
    let refs = parse_refs(input);
    assert_eq!(refs.len(), 1);
    // The span should cover @src/lib.rs
    let span_text = &input[refs[0].span.clone()];
    assert_eq!(span_text, "@src/lib.rs");
}

#[test]
fn test_parse_http_url_ref() {
    let refs = parse_refs("see @http://localhost:8080/api");
    assert_eq!(refs.len(), 1);
    assert!(matches!(refs[0].kind, FileRef::Url(_)));
}

#[test]
fn test_parse_dot_preceded_at_excluded() {
    let refs = parse_refs("file.ext@other");
    // dot-preceded '@' should be excluded (email-like)
    assert!(refs.is_empty());
}

#[test]
fn test_parse_consecutive_refs() {
    let refs = parse_refs("@foo @bar @baz");
    assert_eq!(refs.len(), 3);
}

#[test]
fn test_parse_empty_input() {
    let refs = parse_refs("");
    assert!(refs.is_empty());
}

#[test]
fn test_parse_at_alone() {
    let refs = parse_refs("@ ");
    assert!(
        refs.is_empty(),
        "bare @ with no text after should produce no refs"
    );
}
