//! Tests for test_tool_registry.rs

use ragent_core::tool::*;
use serde_json::json;

use std::sync::Arc;

// ── Default registry ─────────────────────────────────────────────

#[test]
fn test_default_registry_has_all_tools() {
    let registry = create_default_registry();
    let tools = registry.list();

    // Core tools that must always be present (not exhaustive — aliases and
    // new tools are allowed without updating this list).
    let required = [
        "bash",
        "bash_reset",
        "cancel_task",
        "create",
        "edit",
        "glob",
        "grep",
        "github_close_issue",
        "github_comment_issue",
        "github_create_issue",
        "github_create_pr",
        "github_get_issue",
        "github_get_pr",
        "github_list_issues",
        "github_list_prs",
        "github_merge_pr",
        "github_review_pr",
        "libre_info",
        "libre_read",
        "libre_write",
        "list",
        "list_tasks",
        "lsp_definition",
        "lsp_diagnostics",
        "lsp_hover",
        "lsp_references",
        "lsp_symbols",
        "memory_read",
        "memory_write",
        "multiedit",
        "new_task",
        "office_info",
        "office_read",
        "office_write",
        "patch",
        "pdf_read",
        "pdf_write",
        "plan_enter",
        "plan_exit",
        "question",
        "read",
        "rm",
        "task_complete",
        "journal_write",
        "journal_search",
        "journal_read",
        "memory_write",
        "memory_read",
        "memory_replace",
        "memory_migrate",
        "team_approve_plan",
        "team_assign_task",
        "team_broadcast",
        "team_cleanup",
        "team_create",
        "team_idle",
        "team_memory_read",
        "team_memory_write",
        "team_message",
        "team_read_messages",
        "team_shutdown_ack",
        "team_shutdown_teammate",
        "team_spawn",
        "team_status",
        "team_submit_plan",
        "team_task_claim",
        "team_task_complete",
        "team_task_create",
        "team_task_list",
        "team_wait",
        "think",
        "todo_read",
        "todo_write",
        "wait_tasks",
        "webfetch",
        "websearch",
        "write",
    ];
    for name in &required {
        assert!(tools.contains(&name.to_string()), "Missing tool: {}", name);
    }
    // Ensure the registry has at least the required tools (may include aliases).
    assert!(
        tools.len() >= required.len(),
        "Registry has {} tools but expected at least {}",
        tools.len(),
        required.len()
    );
}

#[test]
fn test_registry_list_alphabetically_sorted() {
    let registry = create_default_registry();
    let tools = registry.list();

    let mut sorted = tools.clone();
    sorted.sort();
    assert_eq!(tools, sorted, "Tool list should be alphabetically sorted");
}

#[test]
fn test_registry_get_existing_tool() {
    let registry = create_default_registry();

    for name in &[
        "read",
        "write",
        "edit",
        "multiedit",
        "patch",
        "webfetch",
        "bash",
        "grep",
        "glob",
        "list",
        "question",
        "office_read",
        "office_write",
        "office_info",
    ] {
        let tool = registry.get(name);
        assert!(
            tool.is_some(),
            "Tool '{}' should be found in registry",
            name
        );
        assert_eq!(tool.unwrap().name(), *name);
    }
}

#[test]
fn test_registry_get_nonexistent() {
    let registry = create_default_registry();
    assert!(registry.get("nonexistent").is_none());
}

// ── Tool definitions ─────────────────────────────────────────────

#[test]
fn test_tool_definitions_have_required_fields() {
    let registry = create_default_registry();
    let defs = registry.definitions();

    // At least the 70 original core tools; may grow as new tools and aliases are added.
    assert!(
        defs.len() >= 70,
        "Expected at least 70 tool definitions, got {}",
        defs.len()
    );

    for def in &defs {
        assert!(
            !def.name.is_empty(),
            "Tool definition name should not be empty"
        );
        assert!(
            !def.description.is_empty(),
            "Tool '{}' definition should have a description",
            def.name
        );
        assert!(
            def.parameters.is_object(),
            "Tool '{}' parameters should be a JSON object",
            def.name
        );
    }
}

#[test]
fn test_read_tool_schema_is_strict() {
    let registry = create_default_registry();
    let tool = registry.get("read").unwrap();
    let schema = tool.parameters_schema();

    assert_eq!(schema["additionalProperties"], false);
    assert_eq!(schema["properties"]["start_line"]["minimum"], 1);
    assert_eq!(schema["properties"]["end_line"]["minimum"], 1);
}

#[test]
fn test_tool_definitions_alphabetically_sorted() {
    let registry = create_default_registry();
    let defs = registry.definitions();

    let names: Vec<&str> = defs.iter().map(|d| d.name.as_str()).collect();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    assert_eq!(names, sorted);
}

// ── Custom tool registration ─────────────────────────────────────

struct TestTool;

#[async_trait::async_trait]
impl Tool for TestTool {
    fn name(&self) -> &'static str {
        "test_tool"
    }
    fn description(&self) -> &'static str {
        "A test tool"
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json!({"type": "object", "properties": {}})
    }
    fn permission_category(&self) -> &'static str {
        "test:execute"
    }
    async fn execute(
        &self,
        _input: serde_json::Value,
        _ctx: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        Ok(ToolOutput {
            content: "test output".to_string(),
            metadata: None,
        })
    }
}

#[test]
fn test_register_custom_tool() {
    let registry = ToolRegistry::new();
    assert!(registry.list().is_empty());

    registry.register(Arc::new(TestTool));

    assert_eq!(registry.list().len(), 1);
    assert_eq!(registry.list()[0], "test_tool");

    let tool = registry.get("test_tool").unwrap();
    assert_eq!(tool.description(), "A test tool");
    assert_eq!(tool.permission_category(), "test:execute");
}

// ── ToolOutput ───────────────────────────────────────────────────

#[test]
fn test_tool_output_default() {
    let output = ToolOutput::default();
    assert_eq!(output.content, "");
    assert!(output.metadata.is_none());
}

#[test]
fn test_tool_output_serde() {
    let output = ToolOutput {
        content: "some result".to_string(),
        metadata: Some(json!({"exit_code": 0})),
    };
    let json = serde_json::to_string(&output).unwrap();
    let deserialized: ToolOutput = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.content, "some result");
    assert_eq!(deserialized.metadata, Some(json!({"exit_code": 0})));
}

// ── Tool execution: ReadTool ─────────────────────────────────────

#[tokio::test]
async fn test_read_tool_execute() {
    let dir = std::env::temp_dir().join("ragent_test_read_tool");
    std::fs::create_dir_all(&dir).unwrap();
    let file_path = dir.join("test_read.txt");
    std::fs::write(&file_path, "line one\nline two\nline three\n").unwrap();

    let registry = create_default_registry();
    let tool = registry.get("read").unwrap();

    let ctx = ToolContext {
        session_id: "s1".to_string(),
        working_dir: dir.clone(),
        event_bus: Arc::new(ragent_core::event::EventBus::new(16)),
        storage: None,
        task_manager: None,
        lsp_manager: None,
        active_model: None,
        team_context: None,
        team_manager: None,
        code_index: None,
    };

    let result = tool
        .execute(json!({"path": "test_read.txt"}), &ctx)
        .await
        .unwrap();

    assert!(result.content.contains("line one"));
    assert!(result.content.contains("line two"));
    assert!(result.content.contains("line three"));

    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn test_read_tool_line_range() {
    let dir = std::env::temp_dir().join("ragent_test_read_range");
    std::fs::create_dir_all(&dir).unwrap();
    let file_path = dir.join("test_range.txt");
    std::fs::write(&file_path, "a\nb\nc\nd\ne\n").unwrap();

    let registry = create_default_registry();
    let tool = registry.get("read").unwrap();

    let ctx = ToolContext {
        session_id: "s1".to_string(),
        working_dir: dir.clone(),
        event_bus: Arc::new(ragent_core::event::EventBus::new(16)),
        storage: None,
        task_manager: None,
        lsp_manager: None,
        active_model: None,
        team_context: None,
        team_manager: None,
        code_index: None,
    };

    let result = tool
        .execute(
            json!({"path": "test_range.txt", "start_line": 2, "end_line": 4}),
            &ctx,
        )
        .await
        .unwrap();

    assert!(result.content.contains('b'));
    assert!(result.content.contains('c'));
    assert!(result.content.contains('d'));
    assert!(
        !result.content.contains("   1  a"),
        "Should not include line 1"
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn test_read_tool_rejects_inverted_line_range() {
    let dir = std::env::temp_dir().join("ragent_test_read_bad_range");
    std::fs::create_dir_all(&dir).unwrap();
    let file_path = dir.join("test_range.txt");
    std::fs::write(&file_path, "a\nb\nc\nd\ne\n").unwrap();

    let registry = create_default_registry();
    let tool = registry.get("read").unwrap();

    let ctx = ToolContext {
        session_id: "s1".to_string(),
        working_dir: dir.clone(),
        event_bus: Arc::new(ragent_core::event::EventBus::new(16)),
        storage: None,
        task_manager: None,
        lsp_manager: None,
        active_model: None,
        team_context: None,
        team_manager: None,
        code_index: None,
    };

    let result = tool
        .execute(
            json!({"path": "test_range.txt", "start_line": 10, "end_line": 3}),
            &ctx,
        )
        .await;

    assert!(result.is_err(), "inverted line range should fail");
    let err = result.err().unwrap().to_string();
    assert!(
        err.contains("start_line") && err.contains("end_line"),
        "unexpected error: {}",
        err
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn test_read_tool_missing_file() {
    let dir = std::env::temp_dir().join("ragent_test_read_missing");
    std::fs::create_dir_all(&dir).unwrap();

    let registry = create_default_registry();
    let tool = registry.get("read").unwrap();

    let ctx = ToolContext {
        session_id: "s1".to_string(),
        working_dir: dir.clone(),
        event_bus: Arc::new(ragent_core::event::EventBus::new(16)),
        storage: None,
        task_manager: None,
        lsp_manager: None,
        active_model: None,
        team_context: None,
        team_manager: None,
        code_index: None,
    };

    let result = tool.execute(json!({"path": "nonexistent.txt"}), &ctx).await;
    assert!(result.is_err());

    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn test_read_tool_large_file_section_map() {
    let dir = std::env::temp_dir().join("ragent_test_read_large");
    std::fs::create_dir_all(&dir).unwrap();

    // Create a Rust file with >100 lines containing structs/fns
    let mut content = String::new();
    content.push_str("use std::io;\n\n");
    content.push_str("pub struct Foo {\n    x: i32,\n}\n\n");
    // Pad to exceed 100 lines
    for i in 0..50 {
        content.push_str(&format!("// comment line {}\n", i));
    }
    content.push_str("impl Foo {\n");
    content.push_str("    pub fn new() -> Self {\n        Foo { x: 0 }\n    }\n}\n\n");
    for i in 50..80 {
        content.push_str(&format!("// comment line {}\n", i));
    }
    content.push_str("pub fn helper() -> i32 {\n    42\n}\n");
    for i in 80..100 {
        content.push_str(&format!("// filler {}\n", i));
    }

    let file_path = dir.join("big.rs");
    std::fs::write(&file_path, &content).unwrap();

    let total_lines = content.lines().count();
    assert!(
        total_lines > 100,
        "test file must exceed 100 lines, got {}",
        total_lines
    );

    let registry = create_default_registry();
    let tool = registry.get("read").unwrap();

    let ctx = ToolContext {
        session_id: "s1".to_string(),
        working_dir: dir.clone(),
        event_bus: Arc::new(ragent_core::event::EventBus::new(16)),
        storage: None,
        task_manager: None,
        lsp_manager: None,
        active_model: None,
        team_context: None,
        team_manager: None,
        code_index: None,
    };

    // Read without line range → should get summary, not the whole file
    let result = tool.execute(json!({"path": "big.rs"}), &ctx).await.unwrap();

    assert!(
        result.content.contains("Section Map"),
        "should contain section map"
    );
    assert!(
        result.content.contains("pub struct Foo"),
        "should detect struct Foo"
    );
    assert!(
        result.content.contains("impl Foo"),
        "should detect impl Foo"
    );
    assert!(
        result.content.contains("pub fn helper"),
        "should detect fn helper"
    );

    let meta = result.metadata.unwrap();
    assert_eq!(meta["summarised"], true);
    assert_eq!(meta["end_line"], 100);
    assert!(meta["total_lines"].as_u64().unwrap() > 100);

    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn test_read_tool_large_file_with_range_returns_full() {
    let dir = std::env::temp_dir().join("ragent_test_read_large_range");
    std::fs::create_dir_all(&dir).unwrap();

    // Create a file with >100 lines
    let content: String = (1..=150).map(|i| format!("line {}\n", i)).collect();
    let file_path = dir.join("big.txt");
    std::fs::write(&file_path, &content).unwrap();

    let registry = create_default_registry();
    let tool = registry.get("read").unwrap();

    let ctx = ToolContext {
        session_id: "s1".to_string(),
        working_dir: dir.clone(),
        event_bus: Arc::new(ragent_core::event::EventBus::new(16)),
        storage: None,
        task_manager: None,
        lsp_manager: None,
        active_model: None,
        team_context: None,
        team_manager: None,
        code_index: None,
    };

    // Read WITH a line range → should return those exact lines, no summary
    let result = tool
        .execute(
            json!({"path": "big.txt", "start_line": 50, "end_line": 60}),
            &ctx,
        )
        .await
        .unwrap();

    assert!(result.content.contains("line 50"));
    assert!(result.content.contains("line 60"));
    assert!(
        !result.content.contains("Section Map"),
        "should not contain section map when range specified"
    );

    let meta = result.metadata.unwrap();
    assert!(meta.get("summarised").is_none());

    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn test_read_tool_small_file_no_summary() {
    let dir = std::env::temp_dir().join("ragent_test_read_small_no_sum");
    std::fs::create_dir_all(&dir).unwrap();

    // 5-line file — should return everything, no summary
    let file_path = dir.join("small.rs");
    std::fs::write(&file_path, "fn main() {\n    println!(\"hi\");\n}\n").unwrap();

    let registry = create_default_registry();
    let tool = registry.get("read").unwrap();

    let ctx = ToolContext {
        session_id: "s1".to_string(),
        working_dir: dir.clone(),
        event_bus: Arc::new(ragent_core::event::EventBus::new(16)),
        storage: None,
        task_manager: None,
        lsp_manager: None,
        active_model: None,
        team_context: None,
        team_manager: None,
        code_index: None,
    };

    let result = tool
        .execute(json!({"path": "small.rs"}), &ctx)
        .await
        .unwrap();

    assert!(result.content.contains("fn main()"));
    assert!(!result.content.contains("Section Map"));

    let meta = result.metadata.unwrap();
    assert!(meta.get("summarised").is_none());

    std::fs::remove_dir_all(&dir).ok();
}

// ── Tool execution: WriteTool ────────────────────────────────────

#[tokio::test]
async fn test_write_tool_execute() {
    let dir = std::env::temp_dir().join("ragent_test_write_tool");
    std::fs::create_dir_all(&dir).unwrap();

    let registry = create_default_registry();
    let tool = registry.get("write").unwrap();

    let ctx = ToolContext {
        session_id: "s1".to_string(),
        working_dir: dir.clone(),
        event_bus: Arc::new(ragent_core::event::EventBus::new(16)),
        storage: None,
        task_manager: None,
        lsp_manager: None,
        active_model: None,
        team_context: None,
        team_manager: None,
        code_index: None,
    };

    let result = tool
        .execute(
            json!({"path": "output.txt", "content": "hello world"}),
            &ctx,
        )
        .await
        .unwrap();

    assert!(!result.content.is_empty());

    let content = std::fs::read_to_string(dir.join("output.txt")).unwrap();
    assert_eq!(content, "hello world");

    std::fs::remove_dir_all(&dir).ok();
}

// ── Tool execution: ListTool ─────────────────────────────────────

#[tokio::test]
async fn test_list_tool_execute() {
    let dir = std::env::temp_dir().join("ragent_test_list_tool");
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    std::fs::write(dir.join("file1.txt"), "").unwrap();
    std::fs::write(dir.join("file2.rs"), "").unwrap();
    std::fs::write(dir.join("sub").join("nested.txt"), "").unwrap();

    let registry = create_default_registry();
    let tool = registry.get("list").unwrap();

    let ctx = ToolContext {
        session_id: "s1".to_string(),
        working_dir: dir.clone(),
        event_bus: Arc::new(ragent_core::event::EventBus::new(16)),
        storage: None,
        task_manager: None,
        lsp_manager: None,
        active_model: None,
        team_context: None,
        team_manager: None,
        code_index: None,
    };

    let result = tool.execute(json!({"path": "."}), &ctx).await.unwrap();

    assert!(result.content.contains("file1.txt"));
    assert!(result.content.contains("file2.rs"));
    assert!(result.content.contains("sub"));

    std::fs::remove_dir_all(&dir).ok();
}

// ── Tool execution: GlobTool ─────────────────────────────────────

#[tokio::test]
async fn test_glob_tool_execute() {
    let dir = std::env::temp_dir().join("ragent_test_glob_tool");
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(dir.join("src").join("main.rs"), "fn main() {}").unwrap();
    std::fs::write(dir.join("src").join("lib.rs"), "// lib").unwrap();
    std::fs::write(dir.join("README.md"), "# readme").unwrap();

    let registry = create_default_registry();
    let tool = registry.get("glob").unwrap();

    let ctx = ToolContext {
        session_id: "s1".to_string(),
        working_dir: dir.clone(),
        event_bus: Arc::new(ragent_core::event::EventBus::new(16)),
        storage: None,
        task_manager: None,
        lsp_manager: None,
        active_model: None,
        team_context: None,
        team_manager: None,
        code_index: None,
    };

    let result = tool
        .execute(json!({"pattern": "**/*.rs"}), &ctx)
        .await
        .unwrap();

    assert!(result.content.contains("main.rs"));
    assert!(result.content.contains("lib.rs"));
    assert!(!result.content.contains("README.md"));

    std::fs::remove_dir_all(&dir).ok();
}

// ── Empty registry ───────────────────────────────────────────────

#[test]
fn test_empty_registry() {
    let registry = ToolRegistry::new();
    assert!(registry.list().is_empty());
    assert!(registry.definitions().is_empty());
    assert!(registry.get("anything").is_none());
}
