//! Tests for Milestone T8 — Persistent Agent Memory.

use std::fs;
use std::path::PathBuf;

use ragent_core::agent::custom::load_custom_agents;
use ragent_core::team::config::{MemoryScope, resolve_memory_dir};

/// Helper: create a temp dir with a `.ragent/agents/` subdir containing the
/// given files, and return the temp dir guard.
fn setup_agents_dir(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::Builder::new()
        .prefix("ragent-t8-")
        .tempdir()
        .expect("create temp dir");
    let agents_dir = dir.path().join(".ragent").join("agents");
    fs::create_dir_all(&agents_dir).expect("create .ragent/agents/");
    for (name, content) in files {
        fs::write(agents_dir.join(name), content).expect("write agent file");
    }
    dir
}

// ── T8.1: Memory directory resolution ─────────────────────────────────────────

#[test]
fn test_resolve_memory_dir_none() {
    let dir = resolve_memory_dir(MemoryScope::None, "agent-a", &PathBuf::from("/project"));
    assert!(dir.is_none(), "None scope should resolve to None");
}

#[test]
fn test_resolve_memory_dir_project() {
    let wd = PathBuf::from("/tmp/my-project");
    let dir = resolve_memory_dir(MemoryScope::Project, "doc-writer", &wd).unwrap();
    assert_eq!(
        dir,
        PathBuf::from("/tmp/my-project/.ragent/agent-memory/doc-writer")
    );
}

#[test]
fn test_resolve_memory_dir_user() {
    let wd = PathBuf::from("/tmp/project");
    let dir = resolve_memory_dir(MemoryScope::User, "sec-rev", &wd).unwrap();
    // Should be under home directory, not working dir.
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    assert_eq!(dir, home.join(".ragent/agent-memory/sec-rev"));
}

// ── T8.1: MemoryScope serde round-trip ────────────────────────────────────────

#[test]
fn test_memory_scope_serde_roundtrip() {
    for scope in [MemoryScope::None, MemoryScope::User, MemoryScope::Project] {
        let json = serde_json::to_string(&scope).unwrap();
        let deserialized: MemoryScope = serde_json::from_str(&json).unwrap();
        assert_eq!(scope, deserialized, "round-trip failed for {json}");
    }
}

#[test]
fn test_memory_scope_deserialize_strings() {
    let cases = [
        ("\"none\"", MemoryScope::None),
        ("\"user\"", MemoryScope::User),
        ("\"project\"", MemoryScope::Project),
    ];
    for (input, expected) in cases {
        let result: MemoryScope = serde_json::from_str(input).unwrap();
        assert_eq!(result, expected, "failed for input {input}");
    }
}

// ── T8.1: TeamMember has memory_scope ─────────────────────────────────────────

#[test]
fn test_team_member_default_memory_scope() {
    let member = ragent_core::team::TeamMember::new("test", "tm-001", "general");
    assert_eq!(member.memory_scope, MemoryScope::None);
}

#[test]
fn test_team_member_memory_scope_serde() {
    let mut member = ragent_core::team::TeamMember::new("test", "tm-001", "general");
    member.memory_scope = MemoryScope::Project;
    let json = serde_json::to_string(&member).unwrap();
    assert!(json.contains("\"memory_scope\":\"project\""));
    let deserialized: ragent_core::team::TeamMember = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.memory_scope, MemoryScope::Project);
}

// ── T8.2: Memory injection (MEMORY.md loading) ───────────────────────────────

#[test]
fn test_memory_injection_no_file() {
    let tmp = tempfile::tempdir().unwrap();
    let mem_dir = tmp.path().join(".ragent/agent-memory/test-agent");
    std::fs::create_dir_all(&mem_dir).unwrap();

    // No MEMORY.md — should still produce a prompt block.
    let block = load_memory_block_for_test(&mem_dir);
    assert!(block.contains("Persistent Memory"));
    assert!(block.contains("first session"));
    assert!(block.contains(&mem_dir.display().to_string()));
}

#[test]
fn test_memory_injection_with_file() {
    let tmp = tempfile::tempdir().unwrap();
    let mem_dir = tmp.path().join(".ragent/agent-memory/test-agent");
    std::fs::create_dir_all(&mem_dir).unwrap();
    std::fs::write(
        mem_dir.join("MEMORY.md"),
        "# Previous notes\n\nFoo found a bug in bar.rs\n",
    )
    .unwrap();

    let block = load_memory_block_for_test(&mem_dir);
    assert!(block.contains("Prior Memory"));
    assert!(block.contains("Foo found a bug in bar.rs"));
}

#[test]
fn test_memory_injection_truncates_long_file() {
    let tmp = tempfile::tempdir().unwrap();
    let mem_dir = tmp.path().join(".ragent/agent-memory/test-agent");
    std::fs::create_dir_all(&mem_dir).unwrap();

    // Write 300 lines (exceeds 200-line limit).
    let long_content: String = (0..300)
        .map(|i| format!("Line {i}: some content here"))
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(mem_dir.join("MEMORY.md"), &long_content).unwrap();

    let block = load_memory_block_for_test(&mem_dir);
    // Should contain early lines but not all 300.
    assert!(block.contains("Line 0: some content here"));
    assert!(block.contains("Line 199: some content here"));
    assert!(!block.contains("Line 299: some content here"));
}

// ── T8.3: Memory tools ──────────────────────────────────────────────────────

#[tokio::test]
async fn test_memory_write_and_read() {
    use ragent_core::event::EventBus;
    use ragent_core::team::{MemoryScope, TeamMember, TeamStore};
    use ragent_core::tool::team_memory_read::TeamMemoryReadTool;
    use ragent_core::tool::team_memory_write::TeamMemoryWriteTool;
    use ragent_core::tool::{Tool, ToolContext};
    use std::sync::Arc;

    let tmp = tempfile::tempdir().unwrap();
    let wd = tmp.path().to_path_buf();

    // Create .ragent dir so TeamStore::create can find it.
    fs::create_dir_all(wd.join(".ragent")).unwrap();

    let mut store = TeamStore::create("test-team", "lead-session", &wd, true).unwrap();
    let mut member = TeamMember::new("writer", "tm-001", "general");
    member.memory_scope = MemoryScope::Project;
    store.add_member(member).unwrap();
    store.save().unwrap();

    let ctx = ToolContext {
        session_id: "child-sess".to_string(),
        working_dir: wd.clone(),
        event_bus: Arc::new(EventBus::new(128)),
        storage: None,
        task_manager: None,
        lsp_manager: None,
        active_model: None,
        team_context: Some(Arc::new(ragent_core::tool::TeamContext {
            team_name: "test-team".to_string(),
            agent_id: "tm-001".to_string(),
            is_lead: false,
        })),
        team_manager: None,
    };

    // Write to memory.
    let write_tool = TeamMemoryWriteTool;
    let result = write_tool
        .execute(
            serde_json::json!({
                "team_name": "test-team",
                "content": "# Notes\n\nFound issue in auth.rs\n",
                "mode": "overwrite"
            }),
            &ctx,
        )
        .await
        .unwrap();
    assert!(
        result.content.contains("Wrote"),
        "Write should succeed: {}",
        result.content
    );

    // Read back.
    let read_tool = TeamMemoryReadTool;
    let result = read_tool
        .execute(serde_json::json!({ "team_name": "test-team" }), &ctx)
        .await
        .unwrap();
    assert!(result.content.contains("Found issue in auth.rs"));

    // Append.
    let result = write_tool
        .execute(
            serde_json::json!({
                "team_name": "test-team",
                "content": "\nAlso check session.rs\n"
            }),
            &ctx,
        )
        .await
        .unwrap();
    assert!(result.content.contains("Wrote"));

    // Read back after append.
    let result = read_tool
        .execute(serde_json::json!({ "team_name": "test-team" }), &ctx)
        .await
        .unwrap();
    assert!(result.content.contains("Found issue in auth.rs"));
    assert!(result.content.contains("Also check session.rs"));
}

#[tokio::test]
async fn test_memory_read_disabled() {
    use ragent_core::event::EventBus;
    use ragent_core::team::{TeamMember, TeamStore};
    use ragent_core::tool::team_memory_read::TeamMemoryReadTool;
    use ragent_core::tool::{Tool, ToolContext};
    use std::sync::Arc;

    let tmp = tempfile::tempdir().unwrap();
    let wd = tmp.path().to_path_buf();

    fs::create_dir_all(wd.join(".ragent")).unwrap();
    let mut store = TeamStore::create("test-team", "lead-session", &wd, true).unwrap();
    // Default memory_scope = None.
    let member = TeamMember::new("reader", "tm-002", "general");
    store.add_member(member).unwrap();
    store.save().unwrap();

    let ctx = ToolContext {
        session_id: "child-sess".to_string(),
        working_dir: wd,
        event_bus: Arc::new(EventBus::new(128)),
        storage: None,
        task_manager: None,
        lsp_manager: None,
        active_model: None,
        team_context: Some(Arc::new(ragent_core::tool::TeamContext {
            team_name: "test-team".to_string(),
            agent_id: "tm-002".to_string(),
            is_lead: false,
        })),
        team_manager: None,
    };

    let tool = TeamMemoryReadTool;
    let result = tool
        .execute(serde_json::json!({ "team_name": "test-team" }), &ctx)
        .await
        .unwrap();
    assert!(
        result.content.contains("not enabled"),
        "Should report disabled: {}",
        result.content
    );
}

// ── T8.4: MemoryScope in agent profiles ──────────────────────────────────────

#[test]
fn test_profile_with_memory_scope() {
    let dir = setup_agents_dir(&[(
        "mem-agent.md",
        r#"---
{
  "name": "mem-agent",
  "description": "Agent with memory",
  "memory": "project"
}
---

You are a memory-enabled agent.
"#,
    )]);

    let (agents, diags) = load_custom_agents(dir.path());
    assert!(diags.is_empty(), "No diagnostics expected: {:?}", diags);
    assert_eq!(agents.len(), 1);
    assert_eq!(agents[0].agent_info.memory, MemoryScope::Project);
}

#[test]
fn test_profile_without_memory_defaults_to_none() {
    let dir = setup_agents_dir(&[(
        "plain-agent.md",
        r#"---
{
  "name": "plain-agent",
  "description": "No memory"
}
---

You are a plain agent.
"#,
    )]);

    let (agents, diags) = load_custom_agents(dir.path());
    assert!(diags.is_empty(), "No diagnostics expected: {:?}", diags);
    assert_eq!(agents.len(), 1);
    assert_eq!(agents[0].agent_info.memory, MemoryScope::None);
}

#[test]
fn test_profile_invalid_memory_scope_rejected() {
    let dir = setup_agents_dir(&[(
        "bad-mem.md",
        r#"---
{
  "name": "bad-mem",
  "description": "Invalid memory",
  "memory": "galaxy"
}
---

You are an agent with bad memory scope.
"#,
    )]);

    let (agents, diags) = load_custom_agents(dir.path());
    assert_eq!(agents.len(), 0, "Invalid memory scope should be rejected");
    assert!(!diags.is_empty(), "Should have a diagnostic for bad scope");
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Load the memory block for testing (mirrors the private function in manager.rs).
fn load_memory_block_for_test(mem_dir: &std::path::Path) -> String {
    let memory_file = mem_dir.join("MEMORY.md");
    let max_lines: usize = 200;
    let max_bytes: usize = 25 * 1024;

    let content = if memory_file.is_file() {
        match std::fs::read_to_string(&memory_file) {
            Ok(raw) => {
                let mut taken = 0usize;
                raw.lines()
                    .take(max_lines)
                    .take_while(|line| {
                        taken += line.len() + 1;
                        taken <= max_bytes
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            }
            Err(_) => String::new(),
        }
    } else {
        String::new()
    };

    let dir_display = mem_dir.display();
    let preamble = format!(
        "\n\n## Persistent Memory\n\
         \n\
         Your memory directory: `{dir_display}`\n\
         Use `team_memory_read` to read files and `team_memory_write` to write files in this directory.\n\
         Write important findings, decisions, and context to `MEMORY.md` so you can recall them in future sessions.\n"
    );
    if content.is_empty() {
        format!(
            "{preamble}\n\
             _No prior memory found — this is your first session.  Start writing notes to `MEMORY.md`._\n"
        )
    } else {
        format!(
            "{preamble}\n\
             ### Prior Memory (MEMORY.md)\n\
             \n\
             {content}\n"
        )
    }
}
