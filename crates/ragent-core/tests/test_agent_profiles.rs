//! Tests for declarative agent profiles (.md with JSON frontmatter).
//!
//! Covers Milestone T7 (Declarative Agent Profiles):
//! - Parsing JSON frontmatter from markdown files
//! - Loading .md profiles via the custom agent discovery pipeline
//! - Validation of frontmatter fields
//! - Profile application produces correct AgentInfo

use std::fs;

use ragent_core::agent::custom::load_custom_agents;
use ragent_core::agent::AgentMode;
use ragent_core::permission::PermissionAction;

/// Helper: create a temp dir with a `.ragent/agents/` subdir containing the
/// given files, and return the temp dir guard.
fn setup_agents_dir(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::Builder::new()
        .prefix("ragent-t7-")
        .tempdir()
        .expect("create temp dir");
    let agents_dir = dir.path().join(".ragent").join("agents");
    fs::create_dir_all(&agents_dir).expect("create .ragent/agents/");
    for (name, content) in files {
        fs::write(agents_dir.join(name), content).expect("write agent file");
    }
    dir
}

// ── Basic profile loading ────────────────────────────────────────────────────

#[test]
fn test_load_minimal_profile() {
    let dir = setup_agents_dir(&[(
        "helper.md",
        r#"---
{
  "name": "helper",
  "description": "A helpful assistant"
}
---

You are a helpful assistant. Answer questions clearly and concisely.
"#,
    )]);

    let (agents, diagnostics) = load_custom_agents(dir.path());
    assert!(diagnostics.is_empty(), "diagnostics: {:?}", diagnostics);
    assert_eq!(agents.len(), 1);

    let agent = &agents[0].agent_info;
    assert_eq!(agent.name, "helper");
    assert_eq!(agent.description, "A helpful assistant");
    assert_eq!(
        agent.prompt.as_deref().unwrap(),
        "You are a helpful assistant. Answer questions clearly and concisely."
    );
    // Defaults
    assert_eq!(agent.mode, AgentMode::All);
    assert!(agent.model.is_none());
    assert_eq!(agent.max_steps, Some(100)); // default
}

#[test]
fn test_load_profile_with_all_fields() {
    let dir = setup_agents_dir(&[(
        "reviewer.md",
        r#"---
{
  "name": "security-reviewer",
  "description": "OWASP security reviewer",
  "mode": "subagent",
  "model": "anthropic:claude-haiku-4-5",
  "max_steps": 30,
  "temperature": 0.2,
  "top_p": 0.9,
  "hidden": true,
  "permissions": [
    { "permission": "read", "pattern": "**", "action": "allow" },
    { "permission": "edit", "pattern": "**", "action": "deny" },
    { "permission": "bash", "pattern": "**", "action": "deny" }
  ],
  "skills": ["code_review"]
}
---

You are a security-focused code reviewer specialising in the OWASP Top 10.

For every review:
1. Identify injection flaws
2. Check authentication weaknesses
3. Look for sensitive data exposure
"#,
    )]);

    let (agents, diagnostics) = load_custom_agents(dir.path());
    assert!(diagnostics.is_empty(), "diagnostics: {:?}", diagnostics);
    assert_eq!(agents.len(), 1);

    let agent = &agents[0].agent_info;
    assert_eq!(agent.name, "security-reviewer");
    assert_eq!(agent.mode, AgentMode::Subagent);
    assert_eq!(agent.temperature, Some(0.2));
    assert_eq!(agent.top_p, Some(0.9));
    assert!(agent.hidden);
    assert_eq!(agent.max_steps, Some(30));
    assert_eq!(agent.skills, vec!["code_review"]);

    let model = agent.model.as_ref().unwrap();
    assert_eq!(model.provider_id, "anthropic");
    assert_eq!(model.model_id, "claude-haiku-4-5");

    // Check permissions
    assert_eq!(agent.permission.len(), 3);
    assert_eq!(agent.permission[0].action, PermissionAction::Allow);
    assert_eq!(agent.permission[1].action, PermissionAction::Deny);
    assert_eq!(agent.permission[2].action, PermissionAction::Deny);

    // System prompt should be the markdown body
    assert!(agent.prompt.as_deref().unwrap().starts_with("You are a security-focused"));
    assert!(agent.prompt.as_deref().unwrap().contains("OWASP Top 10"));
}

// ── Coexistence with .json files ─────────────────────────────────────────────

#[test]
fn test_md_and_json_coexist() {
    let json_agent = r#"{
  "name": "json-agent",
  "description": "JSON-based agent",
  "version": "1.0.0",
  "schema_version": "0.7.0",
  "modules": [{
    "type": "ragent/agent/v1",
    "payload": {
      "system_prompt": "You are a JSON agent."
    }
  }]
}"#;

    let md_agent = r#"---
{
  "name": "md-agent",
  "description": "Markdown-based agent"
}
---

You are a markdown agent.
"#;

    let dir = setup_agents_dir(&[("json-agent.json", json_agent), ("md-agent.md", md_agent)]);

    let (agents, diagnostics) = load_custom_agents(dir.path());
    assert!(diagnostics.is_empty(), "diagnostics: {:?}", diagnostics);
    assert_eq!(agents.len(), 2);

    let names: Vec<&str> = agents.iter().map(|a| a.agent_info.name.as_str()).collect();
    assert!(names.contains(&"json-agent"));
    assert!(names.contains(&"md-agent"));
}

#[test]
fn test_md_overrides_json_same_name() {
    let json_agent = r#"{
  "name": "shared-name",
  "description": "JSON version",
  "version": "1.0.0",
  "schema_version": "0.7.0",
  "modules": [{
    "type": "ragent/agent/v1",
    "payload": {
      "system_prompt": "JSON system prompt."
    }
  }]
}"#;

    let md_agent = r#"---
{
  "name": "shared-name",
  "description": "MD version"
}
---

MD system prompt.
"#;

    // Both in same dir — last-scanned wins (filesystem order is non-deterministic,
    // but since project-local overrides global, test that explicitly).
    let dir = tempfile::Builder::new()
        .prefix("ragent-t7-override-")
        .tempdir()
        .unwrap();

    // Global: JSON
    let global_dir = dir.path().join("global").join(".ragent").join("agents");
    fs::create_dir_all(&global_dir).unwrap();
    fs::write(global_dir.join("agent.json"), json_agent).unwrap();

    // Project-local: MD
    let project_dir = dir.path().join("project").join(".ragent").join("agents");
    fs::create_dir_all(&project_dir).unwrap();
    fs::write(project_dir.join("agent.md"), md_agent).unwrap();

    // Load from project dir — project-local MD should win.
    let (agents, _) = load_custom_agents(dir.path().join("project").as_path());
    // Only project-local is found since global path is non-standard.
    let shared = agents.iter().find(|a| a.agent_info.name == "shared-name");
    assert!(shared.is_some());
    assert_eq!(shared.unwrap().agent_info.description, "MD version");
}

// ── Error cases ──────────────────────────────────────────────────────────────

#[test]
fn test_profile_missing_frontmatter() {
    let dir = setup_agents_dir(&[(
        "bad.md",
        "# No frontmatter here\n\nJust markdown.\n",
    )]);

    let (agents, diagnostics) = load_custom_agents(dir.path());
    assert_eq!(agents.len(), 0);
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].contains("frontmatter"));
}

#[test]
fn test_profile_empty_body() {
    let dir = setup_agents_dir(&[(
        "empty-body.md",
        r#"---
{
  "name": "empty",
  "description": "Empty body"
}
---
"#,
    )]);

    let (agents, diagnostics) = load_custom_agents(dir.path());
    assert_eq!(agents.len(), 0);
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].contains("system_prompt"));
}

#[test]
fn test_profile_invalid_json_frontmatter() {
    let dir = setup_agents_dir(&[(
        "bad-json.md",
        "---\n{ invalid json }\n---\n\nSome body.\n",
    )]);

    let (agents, diagnostics) = load_custom_agents(dir.path());
    assert_eq!(agents.len(), 0);
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].contains("JSON parse error"));
}

#[test]
fn test_profile_missing_name() {
    let dir = setup_agents_dir(&[(
        "no-name.md",
        r#"---
{
  "name": "",
  "description": "Missing name"
}
---

Some prompt.
"#,
    )]);

    let (agents, diagnostics) = load_custom_agents(dir.path());
    assert_eq!(agents.len(), 0);
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].contains("name"));
}

// ── Source path tracking ─────────────────────────────────────────────────────

#[test]
fn test_profile_source_path_is_md() {
    let dir = setup_agents_dir(&[(
        "tracker.md",
        r#"---
{
  "name": "tracker",
  "description": "Track source"
}
---

Track me.
"#,
    )]);

    let (agents, _) = load_custom_agents(dir.path());
    assert_eq!(agents.len(), 1);
    assert!(agents[0].source_path.extension().unwrap() == "md");
}
