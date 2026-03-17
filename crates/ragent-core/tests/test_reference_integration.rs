//! External integration tests for the @ file references feature.

use ragent_core::reference::fuzzy::{collect_project_files, fuzzy_match};
use ragent_core::reference::parse::{parse_refs, FileRef};
use ragent_core::reference::resolve::resolve_all_refs;
use std::fs;
use std::path::PathBuf;

fn make_temp_dir(suffix: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "ragent_test_integ_{suffix}_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn cleanup(dir: &PathBuf) {
    let _ = fs::remove_dir_all(dir);
}

/// End-to-end: parse → fuzzy match → resolve for a real project file.
#[tokio::test]
async fn test_end_to_end_file_reference() {
    let dir = make_temp_dir("e2e");
    let src = dir.join("src");
    fs::create_dir(&src).expect("mkdir");
    fs::write(src.join("main.rs"), "fn main() {}").expect("write");

    let input = format!("review @{}", src.join("main.rs").display());
    let refs = parse_refs(&input);
    assert_eq!(refs.len(), 1);

    let (resolved, resolved_refs) = resolve_all_refs(&input, &dir).await.expect("resolve");

    assert!(!resolved_refs.is_empty());
    assert!(resolved.contains("fn main()"));
    cleanup(&dir);
}

/// Fuzzy reference triggers file collection and matching.
#[test]
fn test_fuzzy_file_discovery_and_match() {
    let dir = make_temp_dir("fuzzy");
    fs::write(dir.join("config.toml"), "[package]").expect("write");
    fs::write(dir.join("README.md"), "# Hello").expect("write");

    let files = collect_project_files(&dir, 100);
    assert!(files.len() >= 2, "should find at least 2 files");

    let matches = fuzzy_match("config", &files);
    assert!(!matches.is_empty());
    let top = &matches[0];
    assert!(top.path.to_string_lossy().contains("config"));
    cleanup(&dir);
}

/// Multiple ref types in one prompt.
#[test]
fn test_mixed_ref_types() {
    let input = "check @src/main.rs and @docs/ and @https://example.com and @parser";
    let refs = parse_refs(input);
    assert_eq!(refs.len(), 4);

    assert!(matches!(refs[0].kind, FileRef::File(_)));
    assert!(matches!(refs[1].kind, FileRef::Directory(_)));
    assert!(matches!(refs[2].kind, FileRef::Url(_)));
    assert!(matches!(refs[3].kind, FileRef::Fuzzy(_)));
}

/// Email addresses and @-mentions in code should not trigger.
#[test]
fn test_no_false_positives() {
    // Email
    let refs = parse_refs("contact admin@company.com for help");
    assert!(refs.is_empty());

    // Normal text without @
    let refs = parse_refs("fix the bug in main.rs");
    assert!(refs.is_empty());
}

/// Resolve preserves original message text.
#[tokio::test]
async fn test_resolve_preserves_original_text() {
    let dir = make_temp_dir("preserve");
    let input = "just a normal message without refs";
    let (resolved, refs) = resolve_all_refs(input, &dir).await.expect("resolve");

    assert_eq!(resolved, input);
    assert!(refs.is_empty());
    cleanup(&dir);
}

/// Directory reference resolves to a listing.
#[tokio::test]
async fn test_directory_listing_resolution() {
    let dir = make_temp_dir("dirlist");
    let sub = dir.join("mydir");
    fs::create_dir(&sub).expect("mkdir");
    fs::write(sub.join("a.txt"), "aaa").expect("write");
    fs::write(sub.join("b.txt"), "bbb").expect("write");

    let input = format!("list @{}/", sub.display());
    let (resolved, refs) = resolve_all_refs(&input, &dir).await.expect("resolve");

    assert!(!refs.is_empty());
    assert!(resolved.contains("<referenced_files>"));
    cleanup(&dir);
}
