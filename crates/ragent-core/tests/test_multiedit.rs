#![allow(missing_docs, unused_variables, unused_imports, dead_code, unused_mut)]

use ragent_core::event::EventBus;
use ragent_core::tool::multiedit::MultiEditTool;
use ragent_core::tool::{Tool, ToolContext};
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;

fn make_ctx(dir: PathBuf) -> ToolContext {
    ToolContext {
        session_id: "test".to_string(),
        working_dir: dir,
        event_bus: Arc::new(EventBus::new(16)),
        storage: None,
        task_manager: None,
        lsp_manager: None,
        active_model: None,
        team_context: None,
        team_manager: None,
    }
}

fn tool() -> MultiEditTool {
    MultiEditTool
}

// ── Basic functionality ──────────────────────────────────────────

#[tokio::test]
async fn test_multiedit_single_file_single_edit() {
    let dir = std::env::temp_dir().join("ragent_multiedit_1");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("hello.txt");
    std::fs::write(&path, "Hello World\n").unwrap();

    let ctx = make_ctx(dir.clone());
    let result = tool()
        .execute(
            json!({
                "edits": [{
                    "path": "hello.txt",
                    "old_str": "World",
                    "new_str": "Rust"
                }]
            }),
            &ctx,
        )
        .await
        .unwrap();

    let content = std::fs::read_to_string(&path).unwrap();
    assert_eq!(content, "Hello Rust\n");
    assert!(result.content.contains("1 edit"));
    assert!(result.content.contains("1 file"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn test_multiedit_single_file_multiple_edits() {
    let dir = std::env::temp_dir().join("ragent_multiedit_2");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("code.rs");
    std::fs::write(&path, "fn foo() {}\nfn bar() {}\n").unwrap();

    let ctx = make_ctx(dir.clone());
    let result = tool()
        .execute(
            json!({
                "edits": [
                    { "path": "code.rs", "old_str": "fn foo()", "new_str": "fn foo_renamed()" },
                    { "path": "code.rs", "old_str": "fn bar()", "new_str": "fn bar_renamed()" }
                ]
            }),
            &ctx,
        )
        .await
        .unwrap();

    let content = std::fs::read_to_string(&path).unwrap();
    assert_eq!(content, "fn foo_renamed() {}\nfn bar_renamed() {}\n");
    assert!(result.content.contains("2 edits"));
    assert!(result.content.contains("1 file"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn test_multiedit_multiple_files() {
    let dir = std::env::temp_dir().join("ragent_multiedit_3");
    let _ = std::fs::create_dir_all(&dir);
    std::fs::write(dir.join("a.txt"), "aaa\n").unwrap();
    std::fs::write(dir.join("b.txt"), "bbb\n").unwrap();

    let ctx = make_ctx(dir.clone());
    let result = tool()
        .execute(
            json!({
                "edits": [
                    { "path": "a.txt", "old_str": "aaa", "new_str": "AAA" },
                    { "path": "b.txt", "old_str": "bbb", "new_str": "BBB" }
                ]
            }),
            &ctx,
        )
        .await
        .unwrap();

    assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap(), "AAA\n");
    assert_eq!(std::fs::read_to_string(dir.join("b.txt")).unwrap(), "BBB\n");
    assert!(result.content.contains("2 edits"));
    assert!(result.content.contains("2 files"));
    let _ = std::fs::remove_dir_all(&dir);
}

// ── Atomicity ────────────────────────────────────────────────────

#[tokio::test]
async fn test_multiedit_atomic_rollback_on_missing_match() {
    let dir = std::env::temp_dir().join("ragent_multiedit_4");
    let _ = std::fs::create_dir_all(&dir);
    std::fs::write(dir.join("a.txt"), "original\n").unwrap();
    std::fs::write(dir.join("b.txt"), "keep\n").unwrap();

    let ctx = make_ctx(dir.clone());
    let result = tool()
        .execute(
            json!({
                "edits": [
                    { "path": "a.txt", "old_str": "original", "new_str": "changed" },
                    { "path": "b.txt", "old_str": "DOES_NOT_EXIST", "new_str": "whatever" }
                ]
            }),
            &ctx,
        )
        .await;

    assert!(result.is_err());
    // Both files should be unchanged
    assert_eq!(
        std::fs::read_to_string(dir.join("a.txt")).unwrap(),
        "original\n"
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("b.txt")).unwrap(),
        "keep\n"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn test_multiedit_atomic_rollback_on_duplicate_match() {
    let dir = std::env::temp_dir().join("ragent_multiedit_5");
    let _ = std::fs::create_dir_all(&dir);
    std::fs::write(dir.join("a.txt"), "foo foo foo\n").unwrap();

    let ctx = make_ctx(dir.clone());
    let result = tool()
        .execute(
            json!({
                "edits": [
                    { "path": "a.txt", "old_str": "foo", "new_str": "bar" }
                ]
            }),
            &ctx,
        )
        .await;

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("3 times"));
    // File unchanged
    assert_eq!(
        std::fs::read_to_string(dir.join("a.txt")).unwrap(),
        "foo foo foo\n"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// ── Error cases ──────────────────────────────────────────────────

#[tokio::test]
async fn test_multiedit_empty_edits_array() {
    let dir = std::env::temp_dir().join("ragent_multiedit_6");
    let _ = std::fs::create_dir_all(&dir);
    let ctx = make_ctx(dir.clone());
    let result = tool().execute(json!({ "edits": [] }), &ctx).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("empty"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn test_multiedit_missing_edits_param() {
    let dir = std::env::temp_dir().join("ragent_multiedit_7");
    let _ = std::fs::create_dir_all(&dir);
    let ctx = make_ctx(dir.clone());
    let result = tool().execute(json!({}), &ctx).await;
    assert!(result.is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn test_multiedit_nonexistent_file() {
    let dir = std::env::temp_dir().join("ragent_multiedit_8");
    let _ = std::fs::create_dir_all(&dir);
    let ctx = make_ctx(dir.clone());
    let result = tool()
        .execute(
            json!({
                "edits": [{
                    "path": "nonexistent.txt",
                    "old_str": "a",
                    "new_str": "b"
                }]
            }),
            &ctx,
        )
        .await;
    assert!(result.is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

// ── Metadata ─────────────────────────────────────────────────────

#[tokio::test]
async fn test_multiedit_metadata() {
    let dir = std::env::temp_dir().join("ragent_multiedit_9");
    let _ = std::fs::create_dir_all(&dir);
    std::fs::write(dir.join("a.txt"), "hello\n").unwrap();
    std::fs::write(dir.join("b.txt"), "world\n").unwrap();

    let ctx = make_ctx(dir.clone());
    let result = tool()
        .execute(
            json!({
                "edits": [
                    { "path": "a.txt", "old_str": "hello", "new_str": "hi" },
                    { "path": "b.txt", "old_str": "world", "new_str": "earth" }
                ]
            }),
            &ctx,
        )
        .await
        .unwrap();

    let meta = result.metadata.unwrap();
    assert_eq!(meta["files"], 2);
    assert_eq!(meta["edits"], 2);
    let _ = std::fs::remove_dir_all(&dir);
}

// ── Tool trait ───────────────────────────────────────────────────

#[test]
fn test_multiedit_name_and_permission() {
    let t = tool();
    assert_eq!(t.name(), "multiedit");
    assert_eq!(t.permission_category(), "file:write");
}

#[test]
fn test_multiedit_schema_has_edits_array() {
    let schema = tool().parameters_schema();
    let edits = &schema["properties"]["edits"];
    assert_eq!(edits["type"], "array");
    let items = &edits["items"];
    assert_eq!(items["type"], "object");
    let required: Vec<&str> = items["required"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(required.contains(&"path"));
    assert!(required.contains(&"old_str"));
    assert!(required.contains(&"new_str"));
}

// ── Absolute paths ───────────────────────────────────────────────

#[tokio::test]
async fn test_multiedit_absolute_path() {
    let dir = std::env::temp_dir().join("ragent_multiedit_10");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("abs.txt");
    std::fs::write(&path, "before\n").unwrap();

    let ctx = make_ctx(dir.clone());
    let result = tool()
        .execute(
            json!({
                "edits": [{
                    "path": path.to_str().unwrap(),
                    "old_str": "before",
                    "new_str": "after"
                }]
            }),
            &ctx,
        )
        .await
        .unwrap();

    assert_eq!(std::fs::read_to_string(&path).unwrap(), "after\n");
    assert!(result.content.contains("1 edit"));
    let _ = std::fs::remove_dir_all(&dir);
}
