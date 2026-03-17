use ragent_core::event::EventBus;
use ragent_core::tool::patch::PatchTool;
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
    }
}

fn tool() -> PatchTool {
    PatchTool
}

// ── Basic single-hunk patch ─────────────────────────────────────

#[tokio::test]
async fn test_patch_single_hunk() {
    let dir = std::env::temp_dir().join("ragent_patch_1");
    let _ = std::fs::create_dir_all(&dir);
    std::fs::write(
        dir.join("hello.txt"),
        "line1\nline2\nline3\nline4\nline5\n",
    )
    .unwrap();

    let patch = "\
--- a/hello.txt
+++ b/hello.txt
@@ -2,3 +2,3 @@
 line2
-line3
+line3_modified
 line4
";

    let ctx = make_ctx(dir.clone());
    let result = tool()
        .execute(json!({ "patch": patch }), &ctx)
        .await
        .unwrap();

    let content = std::fs::read_to_string(dir.join("hello.txt")).unwrap();
    assert_eq!(content, "line1\nline2\nline3_modified\nline4\nline5\n");
    assert!(result.content.contains("1 hunk"));
    assert!(result.content.contains("1 file"));
    let _ = std::fs::remove_dir_all(&dir);
}

// ── Multi-hunk single-file patch ────────────────────────────────

#[tokio::test]
async fn test_patch_multi_hunk() {
    let dir = std::env::temp_dir().join("ragent_patch_2");
    let _ = std::fs::create_dir_all(&dir);
    std::fs::write(
        dir.join("code.rs"),
        "fn alpha() {}\nfn beta() {}\nfn gamma() {}\nfn delta() {}\nfn epsilon() {}\n",
    )
    .unwrap();

    let patch = "\
--- a/code.rs
+++ b/code.rs
@@ -1,2 +1,2 @@
-fn alpha() {}
+fn alpha_renamed() {}
 fn beta() {}
@@ -4,2 +4,2 @@
-fn delta() {}
+fn delta_renamed() {}
 fn epsilon() {}
";

    let ctx = make_ctx(dir.clone());
    let result = tool()
        .execute(json!({ "patch": patch }), &ctx)
        .await
        .unwrap();

    let content = std::fs::read_to_string(dir.join("code.rs")).unwrap();
    assert!(content.contains("fn alpha_renamed() {}"));
    assert!(content.contains("fn delta_renamed() {}"));
    assert!(content.contains("fn beta() {}"));
    assert!(result.content.contains("2 hunks"));
    let _ = std::fs::remove_dir_all(&dir);
}

// ── Multi-file patch ────────────────────────────────────────────

#[tokio::test]
async fn test_patch_multi_file() {
    let dir = std::env::temp_dir().join("ragent_patch_3");
    let _ = std::fs::create_dir_all(&dir);
    std::fs::write(dir.join("a.txt"), "aaa\nbbb\nccc\n").unwrap();
    std::fs::write(dir.join("b.txt"), "xxx\nyyy\nzzz\n").unwrap();

    let patch = "\
--- a/a.txt
+++ b/a.txt
@@ -1,3 +1,3 @@
 aaa
-bbb
+BBB
 ccc
--- a/b.txt
+++ b/b.txt
@@ -1,3 +1,3 @@
 xxx
-yyy
+YYY
 zzz
";

    let ctx = make_ctx(dir.clone());
    let result = tool()
        .execute(json!({ "patch": patch }), &ctx)
        .await
        .unwrap();

    assert_eq!(
        std::fs::read_to_string(dir.join("a.txt")).unwrap(),
        "aaa\nBBB\nccc\n"
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("b.txt")).unwrap(),
        "xxx\nYYY\nzzz\n"
    );
    assert!(result.content.contains("2 files"));
    let _ = std::fs::remove_dir_all(&dir);
}

// ── Path override ───────────────────────────────────────────────

#[tokio::test]
async fn test_patch_path_override() {
    let dir = std::env::temp_dir().join("ragent_patch_4");
    let _ = std::fs::create_dir_all(&dir);
    std::fs::write(dir.join("target.txt"), "old line\n").unwrap();

    let patch = "\
--- a/wrong_name.txt
+++ b/wrong_name.txt
@@ -1,1 +1,1 @@
-old line
+new line
";

    let ctx = make_ctx(dir.clone());
    let result = tool()
        .execute(
            json!({ "patch": patch, "path": "target.txt" }),
            &ctx,
        )
        .await
        .unwrap();

    assert_eq!(
        std::fs::read_to_string(dir.join("target.txt")).unwrap(),
        "new line\n"
    );
    assert!(result.content.contains("1 hunk"));
    let _ = std::fs::remove_dir_all(&dir);
}

// ── Fuzz matching ───────────────────────────────────────────────

#[tokio::test]
async fn test_patch_with_fuzz() {
    let dir = std::env::temp_dir().join("ragent_patch_5");
    let _ = std::fs::create_dir_all(&dir);
    // File has slightly different context than the patch expects
    std::fs::write(
        dir.join("fuzz.txt"),
        "context_changed\nkeep\ntarget_line\nkeep\ncontext_changed\n",
    )
    .unwrap();

    // Patch has context lines that don't match the actual file
    let patch = "\
--- a/fuzz.txt
+++ b/fuzz.txt
@@ -1,5 +1,5 @@
 original_context
 keep
-target_line
+modified_line
 keep
 original_context
";

    let ctx = make_ctx(dir.clone());
    // Without fuzz, this should fail
    let result = tool()
        .execute(json!({ "patch": patch, "fuzz": 0 }), &ctx)
        .await;
    assert!(result.is_err());

    // With fuzz=1, dropping outer context lines should work
    let result = tool()
        .execute(json!({ "patch": patch, "fuzz": 1 }), &ctx)
        .await
        .unwrap();

    let content = std::fs::read_to_string(dir.join("fuzz.txt")).unwrap();
    assert!(content.contains("modified_line"));
    assert!(!content.contains("target_line"));
    let _ = std::fs::remove_dir_all(&dir);
}

// ── Add-only patch (new lines) ──────────────────────────────────

#[tokio::test]
async fn test_patch_add_lines() {
    let dir = std::env::temp_dir().join("ragent_patch_6");
    let _ = std::fs::create_dir_all(&dir);
    std::fs::write(dir.join("add.txt"), "line1\nline2\nline3\n").unwrap();

    let patch = "\
--- a/add.txt
+++ b/add.txt
@@ -1,3 +1,5 @@
 line1
+inserted_a
+inserted_b
 line2
 line3
";

    let ctx = make_ctx(dir.clone());
    let result = tool()
        .execute(json!({ "patch": patch }), &ctx)
        .await
        .unwrap();

    let content = std::fs::read_to_string(dir.join("add.txt")).unwrap();
    assert_eq!(content, "line1\ninserted_a\ninserted_b\nline2\nline3\n");
    assert!(result.content.contains("1 hunk"));
    let _ = std::fs::remove_dir_all(&dir);
}

// ── Remove-only patch ───────────────────────────────────────────

#[tokio::test]
async fn test_patch_remove_lines() {
    let dir = std::env::temp_dir().join("ragent_patch_7");
    let _ = std::fs::create_dir_all(&dir);
    std::fs::write(
        dir.join("remove.txt"),
        "keep1\nremove_me\nremove_me_too\nkeep2\n",
    )
    .unwrap();

    let patch = "\
--- a/remove.txt
+++ b/remove.txt
@@ -1,4 +1,2 @@
 keep1
-remove_me
-remove_me_too
 keep2
";

    let ctx = make_ctx(dir.clone());
    let result = tool()
        .execute(json!({ "patch": patch }), &ctx)
        .await
        .unwrap();

    let content = std::fs::read_to_string(dir.join("remove.txt")).unwrap();
    assert_eq!(content, "keep1\nkeep2\n");
    let _ = std::fs::remove_dir_all(&dir);
}

// ── Error: no hunks ─────────────────────────────────────────────

#[tokio::test]
async fn test_patch_empty_patch() {
    let dir = std::env::temp_dir().join("ragent_patch_8");
    let _ = std::fs::create_dir_all(&dir);
    let ctx = make_ctx(dir.clone());
    let result = tool()
        .execute(json!({ "patch": "just some text\n" }), &ctx)
        .await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("No valid diff hunks"));
    let _ = std::fs::remove_dir_all(&dir);
}

// ── Error: hunk fails to match ──────────────────────────────────

#[tokio::test]
async fn test_patch_hunk_mismatch() {
    let dir = std::env::temp_dir().join("ragent_patch_9");
    let _ = std::fs::create_dir_all(&dir);
    std::fs::write(dir.join("mismatch.txt"), "actual content\n").unwrap();

    let patch = "\
--- a/mismatch.txt
+++ b/mismatch.txt
@@ -1,1 +1,1 @@
-wrong content
+new content
";

    let ctx = make_ctx(dir.clone());
    let result = tool()
        .execute(json!({ "patch": patch }), &ctx)
        .await;
    assert!(result.is_err());
    // File should be unchanged
    assert_eq!(
        std::fs::read_to_string(dir.join("mismatch.txt")).unwrap(),
        "actual content\n"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// ── Error: nonexistent file ─────────────────────────────────────

#[tokio::test]
async fn test_patch_nonexistent_file() {
    let dir = std::env::temp_dir().join("ragent_patch_10");
    let _ = std::fs::create_dir_all(&dir);
    let ctx = make_ctx(dir.clone());

    let patch = "\
--- a/missing.txt
+++ b/missing.txt
@@ -1,1 +1,1 @@
-old
+new
";

    let result = tool()
        .execute(json!({ "patch": patch }), &ctx)
        .await;
    assert!(result.is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

// ── Metadata ─────────────────────────────────────────────────────

#[tokio::test]
async fn test_patch_metadata() {
    let dir = std::env::temp_dir().join("ragent_patch_11");
    let _ = std::fs::create_dir_all(&dir);
    std::fs::write(dir.join("meta.txt"), "aaa\nbbb\nccc\n").unwrap();

    let patch = "\
--- a/meta.txt
+++ b/meta.txt
@@ -1,3 +1,3 @@
 aaa
-bbb
+BBB
 ccc
";

    let ctx = make_ctx(dir.clone());
    let result = tool()
        .execute(json!({ "patch": patch }), &ctx)
        .await
        .unwrap();

    let meta = result.metadata.unwrap();
    assert_eq!(meta["files"], 1);
    assert_eq!(meta["hunks"], 1);
    let _ = std::fs::remove_dir_all(&dir);
}

// ── Tool trait ───────────────────────────────────────────────────

#[test]
fn test_patch_name_and_permission() {
    let t = tool();
    assert_eq!(t.name(), "patch");
    assert_eq!(t.permission_category(), "file:write");
}

#[test]
fn test_patch_schema() {
    let schema = tool().parameters_schema();
    let props = &schema["properties"];
    assert!(props["patch"].is_object());
    assert!(props["path"].is_object());
    assert!(props["fuzz"].is_object());
    let required: Vec<&str> = schema["required"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(required.contains(&"patch"));
    assert!(!required.contains(&"path"));
}

// ── Git-style diff with a/ b/ prefixes ──────────────────────────

#[tokio::test]
async fn test_patch_git_style_paths() {
    let dir = std::env::temp_dir().join("ragent_patch_12");
    let _ = std::fs::create_dir_all(&dir);
    std::fs::write(dir.join("file.txt"), "before\n").unwrap();

    let patch = "\
diff --git a/file.txt b/file.txt
--- a/file.txt
+++ b/file.txt
@@ -1 +1 @@
-before
+after
";

    let ctx = make_ctx(dir.clone());
    let result = tool()
        .execute(json!({ "patch": patch }), &ctx)
        .await
        .unwrap();

    assert_eq!(
        std::fs::read_to_string(dir.join("file.txt")).unwrap(),
        "after\n"
    );
    assert!(result.content.contains("1 hunk"));
    let _ = std::fs::remove_dir_all(&dir);
}
