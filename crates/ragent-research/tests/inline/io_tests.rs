//! Inline tests for `io.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;
use crate::status::ResearchStatus;
use tempfile::TempDir;

#[tokio::test]
async fn atomic_write_then_read_round_trips() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("note.md");
    ResearchIo::atomic_write(&path, "hello").await.unwrap();
    let read = ResearchIo::read_file(&path).await.unwrap();
    assert_eq!(read, "hello");
}

#[tokio::test]
async fn atomic_write_creates_parent_dirs() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("nested/deep/file.md");
    ResearchIo::atomic_write(&path, "x").await.unwrap();
    assert!(path.is_file());
}

#[test]
fn item_dir_uses_name_as_dir_name() {
    let name = ResearchName::new("rust-async").unwrap();
    let path = ResearchIo::item_dir(Path::new("/data"), &name);
    assert_eq!(path, PathBuf::from("/data/rust-async"));
}

#[test]
fn research_md_path_appends_filename() {
    let name = ResearchName::new("rust-async").unwrap();
    let path = ResearchIo::research_md_path(Path::new("/data"), &name);
    assert_eq!(path, PathBuf::from("/data/rust-async/RESEARCH.md"));
}

#[test]
fn sources_dir_sits_inside_item_dir() {
    let name = ResearchName::new("rust-async").unwrap();
    let path = ResearchIo::sources_dir(Path::new("/data"), &name);
    assert_eq!(path, PathBuf::from("/data/rust-async/sources"));
}

#[test]
fn source_body_path_uses_two_digit_index() {
    let name = ResearchName::new("rust-async").unwrap();
    assert_eq!(
        ResearchIo::source_body_path(Path::new("/data"), &name, "web", 1),
        PathBuf::from("/data/rust-async/sources/web-01.md"),
    );
    assert_eq!(
        ResearchIo::source_body_path(Path::new("/data"), &name, "local", 12),
        PathBuf::from("/data/rust-async/sources/local-12.md"),
    );
}

#[test]
fn template_path_sits_under_templates_dir() {
    assert_eq!(
        ResearchIo::template_path(Path::new("/data"), "deepdive"),
        Some(PathBuf::from("/data/_templates/deepdive.md")),
    );
}

/// SEC-ragent-research-001 (SECTASKS T-014): a traversal, absolute, or
/// separator-bearing template name is rejected (returns `None`) instead of
/// being joined into a path.
#[test]
fn template_path_rejects_traversal_and_absolute_names() {
    for bad in [
        "../secrets",
        "../../etc/passwd",
        "/home/u/notes",
        "a/b",
        ".",
        "..",
        "",
    ] {
        assert_eq!(
            ResearchIo::template_path(Path::new("/data"), bad),
            None,
            "template name {bad:?} must be rejected"
        );
    }
}

#[test]
fn split_frontmatter_extracts_yaml_block() {
    let content = "---\nname: foo\n---\n\n# Title\nbody\n";
    let (fm, body) = ResearchIo::split_frontmatter(content);
    assert_eq!(fm, "name: foo");
    assert_eq!(body, "# Title\nbody\n");
}

#[test]
fn split_frontmatter_handles_missing_block() {
    let content = "# No frontmatter\nbody\n";
    let (fm, body) = ResearchIo::split_frontmatter(content);
    assert_eq!(fm, "");
    assert_eq!(body, content);
}

#[test]
fn references_index_includes_placeholder_when_empty() {
    let idx = ResearchIo::render_references_index(&[], Utc::now(), false);
    assert!(idx.contains("No sources captured"));
}

#[test]
fn references_index_numbers_sources_sequentially() {
    let sources = vec![
        Source::Other {
            label: "first".into(),
            captured_at: Utc::now(),
            body_path: PathBuf::from("sources/other-01.md"),

            body: String::new(),
        },
        Source::Other {
            label: "second".into(),
            captured_at: Utc::now(),
            body_path: PathBuf::from("sources/other-02.md"),

            body: String::new(),
        },
    ];
    let idx = ResearchIo::render_references_index(&sources, Utc::now(), false);
    assert!(idx.contains("| 1 | other"));
    assert!(idx.contains("| 2 | other"));
}

#[test]
fn references_index_escapes_pipes_in_titles() {
    let sources = vec![Source::Other {
        label: "a|b".into(),
        captured_at: Utc::now(),
        body_path: PathBuf::from("sources/other-01.md"),
        body: String::new(),
    }];
    let idx = ResearchIo::render_references_index(&sources, Utc::now(), false);
    assert!(idx.contains(r"a\|b"), "pipe must be escaped: {idx}");
}

#[test]
fn references_index_includes_published_column_for_web_sources() {
    use chrono::TimeZone;
    let published = Utc.with_ymd_and_hms(2024, 3, 22, 0, 0, 0).unwrap();
    let sources = vec![
        Source::Web {
            url: "https://dated.example".into(),
            title: "Dated".into(),
            captured_at: Utc::now(),
            published_at: Some(published),
            body_path: PathBuf::from("sources/web-01.md"),
            relevance: String::new(),

            body: String::new(),
            search_tool: String::new(),
            search_engine: String::new(),
            content_type: None,
            page_type: None,
            media_type: "page".into(),
            language: None,
            oa_recovery: None,
            author: None,
        },
        Source::Web {
            url: "https://undated.example".into(),
            title: "Undated".into(),
            captured_at: Utc::now(),
            published_at: None,
            body_path: PathBuf::from("sources/web-02.md"),
            relevance: String::new(),

            body: String::new(),
            search_tool: String::new(),
            search_engine: String::new(),
            content_type: None,
            page_type: None,
            media_type: "page".into(),
            language: None,
            oa_recovery: None,
            author: None,
        },
    ];
    let idx = ResearchIo::render_references_index(&sources, Utc::now(), false);
    assert!(
        idx.contains("Published"),
        "header row must include Published column: {idx}"
    );
    assert!(
        idx.contains("2024-03-22"),
        "dated web source should show its publication date: {idx}"
    );
    // The undated row should render an em-dash placeholder for Published.
    let undated_row = idx
        .lines()
        .find(|l| l.contains("https://undated.example"))
        .unwrap_or_default();
    assert!(
        undated_row.contains("| - |"),
        "undated web source should show '-' for Published: {idx}"
    );
}

#[test]
fn index_renders_empty_placeholder_when_no_items() {
    let out = ResearchIo::render_index(&[]);
    assert!(out.contains("No research items yet"));
}

#[test]
fn index_includes_one_row_per_item() {
    let now = Utc::now();
    let items = vec![
        IndexEntry {
            name: "alpha".into(),
            title: "Alpha research".into(),
            status: ResearchStatus::Complete,
            created_at: now,
            modified_at: now,
        },
        IndexEntry {
            name: "beta".into(),
            title: "Beta research".into(),
            status: ResearchStatus::Draft,
            created_at: now,
            modified_at: now,
        },
    ];
    let out = ResearchIo::render_index(&items);
    assert!(out.contains("| alpha | Alpha research | complete |"));
    assert!(out.contains("| beta | Beta research | draft |"));
    assert!(out.contains("2 items"));
}

#[tokio::test]
async fn remove_item_returns_not_found_for_missing_dir() {
    let tmp = TempDir::new().unwrap();
    let name = ResearchName::new("rust-async").unwrap();
    let err = ResearchIo::remove_item(tmp.path(), &name)
        .await
        .unwrap_err();
    assert!(matches!(err, ResearchIoError::NotFound(_)));
}

#[tokio::test]
async fn remove_item_deletes_existing_dir() {
    let tmp = TempDir::new().unwrap();
    let name = ResearchName::new("rust-async").unwrap();
    let dir = ResearchIo::item_dir(tmp.path(), &name);
    tokio::fs::create_dir_all(&dir).await.unwrap();
    ResearchIo::remove_item(tmp.path(), &name).await.unwrap();
    assert!(!dir.exists());
}

#[tokio::test]
async fn create_item_dirs_makes_sources_subdir() {
    let tmp = TempDir::new().unwrap();
    let name = ResearchName::new("rust-async").unwrap();
    ResearchIo::create_item_dirs(tmp.path(), &name)
        .await
        .unwrap();
    assert!(ResearchIo::sources_dir(tmp.path(), &name).is_dir());
}
