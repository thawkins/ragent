//! Coverage-focused tests for `read.rs` structural section detection, the
//! large-file summary path, line-range selection, and the output cap.
//!
//! Each test writes a file (>100 lines, so the summary path is taken when no
//! line range is supplied) into `target/temp` and drives the public
//! [`ReadTool`] so the private `detect_*_sections` helpers are exercised
//! through their real caller.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ragent_tools_core::read::ReadTool;
use ragent_tools_core::{Tool, ToolContext, ToolOutput};
use ragent_types::event::EventBus;
use serde_json::json;

fn make_ctx(dir: &Path) -> ToolContext {
    ToolContext {
        session_id: "test".to_string(),
        working_dir: dir.to_path_buf(),
        event_bus: Arc::new(EventBus::new(1024)),
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        canonical_cache: Arc::new(ragent_tools_core::CanonicalPathCache::new()),
        allowed_roots: vec![dir.to_path_buf()],
    }
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::current_dir()
        .unwrap()
        .join("target")
        .join("temp")
        .join(format!(
            "read_sections_{tag}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Write `body` to `name` inside `dir`, padding with filler comment lines so
/// the file comfortably exceeds the 100-line large-file threshold.
fn write_large(dir: &Path, name: &str, marker_lines: &[&str]) -> PathBuf {
    let path = dir.join(name);
    let mut f = std::fs::File::create(&path).unwrap();
    for line in marker_lines {
        writeln!(f, "{line}").unwrap();
    }
    for i in 0..150 {
        writeln!(f, "// filler line {i}").unwrap();
    }
    path
}

async fn read_no_range(path: &Path, dir: &Path) -> ToolOutput {
    ReadTool
        .execute(json!({ "path": path.to_str().unwrap() }), &make_ctx(dir))
        .await
        .unwrap()
}

/// Rust markers: fn/struct/enum/impl/trait/mod/macro_rules.
#[tokio::test]
async fn test_read_sections_rust() {
    let dir = temp_dir("rust");
    let path = write_large(
        &dir,
        "sample.rs",
        &[
            "pub fn alpha() {",
            "fn beta() {",
            "pub(crate) async fn gamma() {",
            "struct Widget;",
            "pub enum Colour {",
            "impl Widget {",
            "pub trait Behaviour {",
            "pub mod inner;",
            "macro_rules! my_macro {",
        ],
    );
    let out = read_no_range(&path, &dir).await;
    assert!(out.content.contains("--- Section Map ---"));
    assert!(out.content.contains("pub fn alpha()"));
    assert!(out.content.contains("fn beta()"));
    assert!(out.content.contains("struct Widget"));
    assert!(out.content.contains("impl Widget"));
    assert!(out.content.contains("pub trait Behaviour"));
    assert!(out.content.contains("macro_rules! my_macro"));
    let meta = out.metadata.unwrap();
    assert_eq!(meta["summarised"], true);
    assert_eq!(meta["start_line"], 1);
}

/// Markdown markers: lines starting with '#'.
#[tokio::test]
async fn test_read_sections_markdown() {
    let dir = temp_dir("md");
    let path = write_large(&dir, "doc.md", &["# Title", "## Section One", "### Deep"]);
    let out = read_no_range(&path, &dir).await;
    assert!(out.content.contains("# Title"));
    assert!(out.content.contains("## Section One"));
}

/// Python markers: def / async def / class.
#[tokio::test]
async fn test_read_sections_python() {
    let dir = temp_dir("py");
    let path = write_large(
        &dir,
        "mod.py",
        &["def compute(x):", "async def stream():", "class Handler:"],
    );
    let out = read_no_range(&path, &dir).await;
    assert!(out.content.contains("def compute(x)"));
    assert!(out.content.contains("async def stream()"));
    assert!(out.content.contains("class Handler"));
}

/// TypeScript/JavaScript markers: functions, classes, exported consts.
#[tokio::test]
async fn test_read_sections_javascript() {
    let dir = temp_dir("ts");
    let path = write_large(
        &dir,
        "app.ts",
        &[
            "export function run() {",
            "async function fetchIt() {",
            "export default class Service {",
            "export const CONFIG = 1;",
            "export interface Shape {",
            "export type Alias = number;",
        ],
    );
    let out = read_no_range(&path, &dir).await;
    assert!(out.content.contains("export function run()"));
    assert!(out.content.contains("export default class Service"));
    assert!(out.content.contains("export interface Shape"));
}

/// TOML markers: bracketed section headers.
#[tokio::test]
async fn test_read_sections_toml() {
    let dir = temp_dir("toml");
    let path = write_large(
        &dir,
        "conf.toml",
        &["[package]", "[dependencies]", "[profile]"],
    );
    let out = read_no_range(&path, &dir).await;
    assert!(out.content.contains("[package]"));
    assert!(out.content.contains("[profile]"));
}

/// YAML markers: unindented, non-comment, non-list top-level keys.
#[tokio::test]
async fn test_read_sections_yaml() {
    let dir = temp_dir("yaml");
    let path = write_large(
        &dir,
        "conf.yaml",
        &["key: value", "nested_block:", "# a comment", "- list item"],
    );
    let out = read_no_range(&path, &dir).await;
    assert!(out.content.contains("key: value"));
    assert!(out.content.contains("nested_block:"));
}

/// INI/CFG markers reuse the bracket-header format.
#[tokio::test]
async fn test_read_sections_ini() {
    let dir = temp_dir("ini");
    let path = write_large(&dir, "app.cfg", &["[server]", "[logging]"]);
    let out = read_no_range(&path, &dir).await;
    assert!(out.content.contains("[server]"));
    assert!(out.content.contains("[logging]"));
}

/// C/C++ markers: function-definition heuristics and typedef struct/enum.
#[tokio::test]
async fn test_read_sections_c() {
    let dir = temp_dir("c");
    let path = write_large(
        &dir,
        "lib.c",
        &[
            "int compute(int x) {",
            "void helper(void)",
            "typedef struct {",
            "struct point {",
            "if (x) {",
            "// a comment",
        ],
    );
    let out = read_no_range(&path, &dir).await;
    assert!(out.content.contains("int compute(int x)"));
    assert!(out.content.contains("void helper(void)"));
    assert!(out.content.contains("typedef struct"));
}

/// Java/Kotlin markers: class/interface declarations and method signatures.
#[tokio::test]
async fn test_read_sections_java() {
    let dir = temp_dir("java");
    let path = write_large(
        &dir,
        "Main.java",
        &[
            "public class Main {",
            "interface Greeter {",
            "public void run() {",
            "private int value()",
        ],
    );
    let out = read_no_range(&path, &dir).await;
    assert!(out.content.contains("public class Main"));
    assert!(out.content.contains("interface Greeter"));
    assert!(out.content.contains("public void run()"));
}

/// Go markers: func and type declarations.
#[tokio::test]
async fn test_read_sections_go() {
    let dir = temp_dir("go");
    let path = write_large(
        &dir,
        "main.go",
        &[
            "func main() {",
            "type Server struct {",
            "func (s *Server) Run() {",
        ],
    );
    let out = read_no_range(&path, &dir).await;
    assert!(out.content.contains("func main()"));
    assert!(out.content.contains("type Server struct"));
}

/// Ruby markers: def / class / module.
#[tokio::test]
async fn test_read_sections_ruby() {
    let dir = temp_dir("rb");
    let path = write_large(
        &dir,
        "app.rb",
        &["def greet(name)", "class Greeter", "module Helpers"],
    );
    let out = read_no_range(&path, &dir).await;
    assert!(out.content.contains("def greet(name)"));
    assert!(out.content.contains("class Greeter"));
    assert!(out.content.contains("module Helpers"));
}

/// CSS markers: unindented selectors containing '{'.
#[tokio::test]
async fn test_read_sections_css() {
    let dir = temp_dir("css");
    let path = write_large(
        &dir,
        "style.css",
        &[".header {", "#main {", "  .indented {"],
    );
    let out = read_no_range(&path, &dir).await;
    assert!(out.content.contains(".header"));
    assert!(out.content.contains("#main"));
}

/// An unknown extension with >100 lines yields no structural sections and the
/// "No structural sections detected" summary.
#[tokio::test]
async fn test_read_sections_unknown_extension_has_no_sections() {
    let dir = temp_dir("unknown");
    let path = write_large(&dir, "data.xyz", &["alpha", "beta"]);
    let out = read_no_range(&path, &dir).await;
    assert!(out.content.contains("--- Section Map ---"));
    assert!(out.content.contains("No structural sections detected"));
    assert_eq!(out.metadata.unwrap()["summarised"], true);
}

/// A large file that is all filler (no markers) for a *known* extension also
/// reports no structural sections.
#[tokio::test]
async fn test_read_sections_known_extension_without_markers() {
    let dir = temp_dir("norsrust");
    let path = write_large(&dir, "plain.rs", &[]);
    let out = read_no_range(&path, &dir).await;
    assert!(out.content.contains("No structural sections detected"));
}

/// `start_line` only returns from the start line to end of file.
#[tokio::test]
async fn test_read_start_line_only_to_eof() {
    let dir = temp_dir("startonly");
    let path = dir.join("small.txt");
    {
        let mut f = std::fs::File::create(&path).unwrap();
        for i in 1..=10 {
            writeln!(f, "Line {i}").unwrap();
        }
    }
    let out = ReadTool
        .execute(
            json!({ "path": path.to_str().unwrap(), "start_line": 7 }),
            &make_ctx(&dir),
        )
        .await
        .unwrap();
    assert!(out.content.contains("Line 7"));
    assert!(out.content.contains("Line 10"));
    assert!(!out.content.contains("Line 6"));
    let meta = out.metadata.unwrap();
    assert_eq!(meta["start_line"], 7);
    assert_eq!(meta["end_line"], 10);
    assert_eq!(meta["line_count"], 4);
}

/// `end_line` larger than the file length is clamped to the file length.
#[tokio::test]
async fn test_read_end_line_clamped_to_file_length() {
    let dir = temp_dir("clampend");
    let path = dir.join("small.txt");
    {
        let mut f = std::fs::File::create(&path).unwrap();
        for i in 1..=5 {
            writeln!(f, "Line {i}").unwrap();
        }
    }
    let out = ReadTool
        .execute(
            json!({ "path": path.to_str().unwrap(), "start_line": 2, "end_line": 999 }),
            &make_ctx(&dir),
        )
        .await
        .unwrap();
    assert!(out.content.contains("Line 5"));
    let meta = out.metadata.unwrap();
    assert_eq!(meta["end_line"], 5);
}

/// `start_line` beyond the file length is an error.
#[tokio::test]
async fn test_read_start_line_beyond_file_is_error() {
    let dir = temp_dir("beyond");
    let path = dir.join("small.txt");
    std::fs::write(&path, "a\nb\nc\n").unwrap();
    let err = ReadTool
        .execute(
            json!({ "path": path.to_str().unwrap(), "start_line": 50 }),
            &make_ctx(&dir),
        )
        .await
        .expect_err("start_line past EOF must fail");
    let msg = err.to_string();
    assert!(msg.contains("exceeds file length"), "unexpected: {msg}");
}

/// Zero `start_line` is rejected.
#[tokio::test]
async fn test_read_start_line_zero_is_error() {
    let dir = temp_dir("startzero");
    let path = dir.join("small.txt");
    std::fs::write(&path, "a\nb\n").unwrap();
    let err = ReadTool
        .execute(
            json!({ "path": path.to_str().unwrap(), "start_line": 0 }),
            &make_ctx(&dir),
        )
        .await
        .expect_err("start_line=0 must fail");
    assert!(err.to_string().contains("start_line"));
}

/// A missing required `path` parameter is rejected.
#[tokio::test]
async fn test_read_missing_path_is_error() {
    let dir = temp_dir("nopath");
    let err = ReadTool
        .execute(json!({}), &make_ctx(&dir))
        .await
        .expect_err("missing path must fail");
    assert!(err.to_string().contains("path"));
}

/// Reading a directory path is rejected with an actionable message.
#[tokio::test]
async fn test_read_directory_is_error() {
    let dir = temp_dir("isdir");
    let sub = dir.join("subdir");
    std::fs::create_dir_all(&sub).unwrap();
    let err = ReadTool
        .execute(json!({ "path": sub.to_str().unwrap() }), &make_ctx(&dir))
        .await
        .expect_err("directory read must fail");
    assert!(err.to_string().contains("is a directory"));
}

/// Reading a non-existent file surfaces the contextual read error.
#[tokio::test]
async fn test_read_missing_file_is_error() {
    let dir = temp_dir("nofile");
    let missing = dir.join("does_not_exist.txt");
    let err = ReadTool
        .execute(
            json!({ "path": missing.to_str().unwrap() }),
            &make_ctx(&dir),
        )
        .await
        .expect_err("missing file must fail");
    assert!(err.to_string().contains("Cannot read file"));
}

/// A whole-file read whose formatted output exceeds the cap is truncated at a
/// UTF-8 boundary and marked in metadata.
#[tokio::test]
async fn test_read_whole_file_huge_single_line_is_capped() {
    let dir = temp_dir("huge");
    let path = dir.join("huge.txt");
    {
        let mut f = std::fs::File::create(&path).unwrap();
        // One >200k-char line (with multi-byte chars near the cut boundary) plus
        // a couple of short lines so it is not the "large file" summary path.
        let filler = "\u{e9}".repeat(200_000);
        writeln!(f, "{filler}").unwrap();
        writeln!(f, "tail").unwrap();
    }
    let out = ReadTool
        .execute(json!({ "path": path.to_str().unwrap() }), &make_ctx(&dir))
        .await
        .unwrap();
    assert!(out.content.contains("output truncated"));
    let meta = out.metadata.unwrap();
    assert_eq!(meta["truncated"], true);
    assert_eq!(meta["max_output_chars"], 200_000);
}
