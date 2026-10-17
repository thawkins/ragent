//! Coverage-focused happy-path tests for the simple file-operation tools that
//! are otherwise only exercised on their rejection branches: `create`,
//! `write`, `append_to_file`, `copy_file`, `move_file`, `rm`, `make_directory`,
//! `file_info`, `diff_files`, `calculator` and the `xlsx` writer.
//!
//! Every assertion is concrete (real files under `target/temp`, real byte
//! counts, real type strings) so a regression in the success path fails loudly.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use ragent_tools_core::append_file::AppendFileTool;
use ragent_tools_core::calculator::CalculatorTool;
use ragent_tools_core::copy_file::CopyFileTool;
use ragent_tools_core::create::CreateTool;
use ragent_tools_core::diff::DiffFilesTool;
use ragent_tools_core::file_info::FileInfoTool;
use ragent_tools_core::mkdir::MakeDirTool;
use ragent_tools_core::move_file::MoveFileTool;
use ragent_tools_core::rm::RmTool;
use ragent_tools_core::write::WriteTool;
use ragent_tools_core::{Tool, ToolContext, ToolOutput};
use ragent_types::event::EventBus;
use serde_json::json;

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::current_dir()
        .unwrap()
        .join("target")
        .join("temp")
        .join(format!(
            "fileops_{tag}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn ctx(dir: &Path) -> ToolContext {
    ToolContext {
        session_id: "test".to_string(),
        working_dir: dir.to_path_buf(),
        event_bus: Arc::new(EventBus::new(32)),
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        canonical_cache: Arc::new(ragent_tools_core::CanonicalPathCache::new()),
        allowed_roots: vec![dir.to_path_buf()],
    }
}

async fn run<T: Tool>(tool: &T, input: serde_json::Value, dir: &Path) -> ToolOutput {
    tool.execute(input, &ctx(dir))
        .await
        .unwrap_or_else(|e| panic!("{} failed: {e}", tool.name()))
}

#[tokio::test]
async fn test_create_then_overwrite_reports_both_labels() {
    let dir = temp_dir("create");
    let out = run(
        &CreateTool,
        json!({"path": "nested/a.txt", "content": "hello\nworld\n"}),
        &dir,
    )
    .await;
    assert!(out.content.starts_with("Created "), "{}", out.content);
    assert!(
        out.content.contains("12 bytes (2 lines)"),
        "{}",
        out.content
    );
    let meta = out.metadata.unwrap();
    assert_eq!(meta["byte_count"], 12);
    assert_eq!(meta["line_count"], 2);
    assert_eq!(meta["file_count"], 1);

    // Overwriting an existing file is labelled differently and truncates.
    let out = run(
        &CreateTool,
        json!({"path": "nested/a.txt", "content": "x"}),
        &dir,
    )
    .await;
    assert!(out.content.starts_with("Overwrote "), "{}", out.content);
    assert_eq!(
        std::fs::read_to_string(dir.join("nested/a.txt")).unwrap(),
        "x"
    );
}

#[tokio::test]
async fn test_write_reports_byte_and_line_counts() {
    let dir = temp_dir("write");
    let out = run(
        &WriteTool,
        json!({"path": "w.txt", "content": "alpha\nbeta"}),
        &dir,
    )
    .await;
    assert!(
        out.content.contains("Wrote 10 bytes (2 lines)"),
        "{}",
        out.content
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("w.txt")).unwrap(),
        "alpha\nbeta"
    );
    let meta = out.metadata.unwrap();
    assert_eq!(meta["byte_count"], 10);
    assert_eq!(meta["path"], dir.join("w.txt").display().to_string());
}

#[tokio::test]
async fn test_append_creates_then_appends_and_is_visible() {
    let dir = temp_dir("append");
    // create_new path: append creates the file.
    let out = run(
        &AppendFileTool,
        json!({"path": "log.txt", "content": "one\n"}),
        &dir,
    )
    .await;
    assert!(out.content.starts_with("Appended "), "{}", out.content);
    // Real, immediately-visible content (FUNC-061 flush guarantee).
    assert_eq!(
        std::fs::read_to_string(dir.join("log.txt")).unwrap(),
        "one\n"
    );

    run(
        &AppendFileTool,
        json!({"path": "log.txt", "content": "two\n"}),
        &dir,
    )
    .await;
    assert_eq!(
        std::fs::read_to_string(dir.join("log.txt")).unwrap(),
        "one\ntwo\n"
    );
}

#[tokio::test]
async fn test_copy_file_copies_content() {
    let dir = temp_dir("copy");
    std::fs::write(dir.join("src.txt"), "payload").unwrap();
    let out = run(
        &CopyFileTool,
        json!({"source": "src.txt", "destination": "dst/copy.txt"}),
        &dir,
    )
    .await;
    assert!(out.content.contains("Copied"), "{}", out.content);
    assert_eq!(
        std::fs::read_to_string(dir.join("dst/copy.txt")).unwrap(),
        "payload"
    );
    // Source is left intact.
    assert_eq!(
        std::fs::read_to_string(dir.join("src.txt")).unwrap(),
        "payload"
    );
}

#[tokio::test]
async fn test_copy_missing_source_is_error() {
    let dir = temp_dir("copy_missing");
    let err = CopyFileTool
        .execute(
            json!({"source": "nope.txt", "destination": "x.txt"}),
            &ctx(&dir),
        )
        .await
        .unwrap_err();
    assert!(
        err.to_string().to_lowercase().contains("canonicalize")
            || err.to_string().to_lowercase().contains("no such file"),
        "{err}"
    );
}

#[tokio::test]
async fn test_move_file_renames_and_creates_parents() {
    let dir = temp_dir("move");
    std::fs::write(dir.join("a.txt"), "moved").unwrap();
    let out = run(
        &MoveFileTool,
        json!({"source": "a.txt", "destination": "deep/er/b.txt"}),
        &dir,
    )
    .await;
    assert!(out.content.starts_with("Moved '"), "{}", out.content);
    assert!(!dir.join("a.txt").exists());
    assert_eq!(
        std::fs::read_to_string(dir.join("deep/er/b.txt")).unwrap(),
        "moved"
    );
    assert_eq!(
        out.metadata.unwrap()["destination"],
        dir.join("deep/er/b.txt").display().to_string()
    );
}

#[tokio::test]
async fn test_move_missing_source_is_error() {
    let dir = temp_dir("move_missing");
    let err = MoveFileTool
        .execute(
            json!({"source": "ghost.txt", "destination": "b.txt"}),
            &ctx(&dir),
        )
        .await
        .unwrap_err();
    assert!(err.to_string().contains("Source not found"), "{err}");
    // No orphan destination directory is created on failure.
    assert!(!dir.join("b.txt").exists());
}

#[tokio::test]
async fn test_make_directory_then_rm_file() {
    let dir = temp_dir("mkdir_rm");
    let out = run(&MakeDirTool, json!({"path": "a/b/c"}), &dir).await;
    assert!(
        out.content.contains("Created directory:"),
        "{}",
        out.content
    );
    assert!(dir.join("a/b/c").is_dir());
    assert_eq!(
        out.metadata.unwrap()["path"],
        dir.join("a/b/c").display().to_string()
    );

    std::fs::write(dir.join("a/b/c/gone.txt"), "x").unwrap();
    let out = run(&RmTool, json!({"path": "a/b/c/gone.txt"}), &dir).await;
    assert!(out.content.starts_with("Deleted "), "{}", out.content);
    assert!(!dir.join("a/b/c/gone.txt").exists());
    assert_eq!(out.metadata.unwrap()["deleted"], true);
}

#[tokio::test]
async fn test_rm_missing_file_is_error() {
    let dir = temp_dir("rm_missing");
    let err = RmTool
        .execute(json!({"path": "no.txt"}), &ctx(&dir))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("File not found"), "{err}");
}

#[tokio::test]
async fn test_rm_directory_is_error() {
    let dir = temp_dir("rm_dir");
    std::fs::create_dir_all(dir.join("adir")).unwrap();
    let err = RmTool
        .execute(json!({"path": "adir"}), &ctx(&dir))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("is a directory"), "{err}");
}

#[tokio::test]
async fn test_file_info_reports_file_dir_symlink_and_missing() {
    let dir = temp_dir("file_info");
    std::fs::write(dir.join("f.txt"), "12345").unwrap();
    std::fs::create_dir_all(dir.join("d")).unwrap();

    let out = run(&FileInfoTool, json!({"path": "f.txt"}), &dir).await;
    assert!(out.content.contains("Type:        file"), "{}", out.content);
    assert!(
        out.content.contains("Size:        5 bytes"),
        "{}",
        out.content
    );
    assert!(out.content.contains("Permissions:"), "{}", out.content);
    let meta = out.metadata.unwrap();
    assert_eq!(meta["exists"], true);
    assert_eq!(meta["type"], "file");
    assert_eq!(meta["size"], 5);

    let out = run(&FileInfoTool, json!({"path": "d"}), &dir).await;
    assert!(
        out.content.contains("Type:        directory"),
        "{}",
        out.content
    );

    let out = run(&FileInfoTool, json!({"path": "absent.txt"}), &dir).await;
    assert!(
        out.content.contains("Path does not exist"),
        "{}",
        out.content
    );
    assert_eq!(out.metadata.unwrap()["exists"], false);

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink("f.txt", dir.join("link")).unwrap();
        let out = run(&FileInfoTool, json!({"path": "link"}), &dir).await;
        assert!(
            out.content.contains("Type:        symlink"),
            "{}",
            out.content
        );
    }
}

#[tokio::test]
async fn test_diff_files_identical_and_changed() {
    let dir = temp_dir("diff");
    std::fs::write(dir.join("left.txt"), "a\nb\nc\n").unwrap();
    std::fs::write(dir.join("right.txt"), "a\nB\nc\n").unwrap();

    let out = run(
        &DiffFilesTool,
        json!({"path_a": "left.txt", "path_b": "right.txt"}),
        &dir,
    )
    .await;
    assert!(out.content.contains("@@"), "{}", out.content);
    assert!(out.content.contains("-b"), "{}", out.content);
    assert!(out.content.contains("+B"), "{}", out.content);
    assert!(out.metadata.unwrap()["changes"].as_u64().unwrap() >= 2);

    // Identical text via the inline text_* parameters.
    let out = run(
        &DiffFilesTool,
        json!({"text_a": "same", "text_b": "same"}),
        &dir,
    )
    .await;
    assert_eq!(out.content, "Files are identical (no differences)");
    assert_eq!(out.metadata.unwrap()["changes"], 0);
}

#[tokio::test]
async fn test_diff_missing_input_is_error() {
    let dir = temp_dir("diff_missing");
    let err = DiffFilesTool
        .execute(json!({"text_b": "x"}), &ctx(&dir))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("path_a"), "{err}");
    let err = DiffFilesTool
        .execute(json!({"text_a": "x"}), &ctx(&dir))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("path_b"), "{err}");
}

#[tokio::test]
async fn test_calculator_returns_result_and_reports_errors() {
    let dir = temp_dir("calc");
    let out = run(&CalculatorTool, json!({"expression": "2 + 3 * 4"}), &dir).await;
    assert_eq!(out.content, "2 + 3 * 4 = 14");
    assert_eq!(out.metadata.unwrap()["result"], "14");

    let out = run(&CalculatorTool, json!({"expression": "10 / 4"}), &dir).await;
    assert!(out.content.ends_with("= 2.5"), "{}", out.content);

    // Missing parameter and evaluation errors surface as Err.
    assert!(CalculatorTool.execute(json!({}), &ctx(&dir)).await.is_err());
    let err = CalculatorTool
        .execute(json!({"expression": "1 +"}), &ctx(&dir))
        .await
        .unwrap_err();
    assert_ne!(err.to_string(), "");
}

#[test]
fn test_xlsx_writer_edge_shapes() {
    use ragent_tools_core::xlsx::write_xlsx;
    let dir = temp_dir("xlsx");
    // Missing sheets array is an error.
    assert!(write_xlsx(&dir.join("bad.xlsx"), &json!({})).is_err());

    // A sheet with no rows still writes a valid workbook.
    let path = dir.join("empty.xlsx");
    write_xlsx(&path, &json!({"sheets": [{"name": "Only"}]})).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    assert_eq!(&bytes[..4], b"PK\x03\x04");

    // Default sheet name when `name` is absent; null cells are skipped.
    let path2 = dir.join("def.xlsx");
    write_xlsx(
        &path2,
        &json!({"sheets": [{"rows": [[1, true, "s", null]]}]}),
    )
    .unwrap();
    assert_eq!(&std::fs::read(&path2).unwrap()[..4], b"PK\x03\x04");
}
