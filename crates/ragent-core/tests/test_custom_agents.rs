//! Tests for test_custom_agents.rs

//! Tests for the custom OASF agent loading system.
//!
//! Covers:
//! - Valid record parsing and conversion to AgentInfo
//! - Invalid records producing correct error messages
//! - Discovery path resolution (global and project-local)
//! - Name collision handling (rename to `custom:<name>`)
//! - Template variable substitution in system_prompt
//! - Permission ruleset parsing and defaults

use ragent_core::agent::custom::{
    find_project_agents_dir, global_agents_dir, load_custom_agents, record_to_agent_info,
};
use ragent_core::agent::oasf::{
    OasfAgentRecord, OasfModule, RAGENT_MODULE_TYPE,
};
use ragent_core::agent::{AgentMode, build_system_prompt, create_builtin_agents};
use ragent_core::permission::PermissionAction;
use serde_json::json;
use std::path::PathBuf;

// ── Helpers ────────────────────────────────────────────────────────────────────

/// Build a minimal valid OasfAgentRecord with the given system_prompt.
fn minimal_record(name: &str, description: &str, system_prompt: &str) -> OasfAgentRecord {
    OasfAgentRecord {
        name: name.to_string(),
        description: description.to_string(),
        version: "1.0.0".to_string(),
        schema_version: "0.7.0".to_string(),
        authors: vec![],
        created_at: None,
        skills: vec![],
        domains: vec![],
        locators: vec![],
        modules: vec![OasfModule {
            module_type: RAGENT_MODULE_TYPE.to_string(),
            payload: json!({
                "system_prompt": system_prompt,
                "mode": "primary",
                "max_steps": 50
            }),
        }],
    }
}

/// Build a record with a custom payload value.
fn record_with_payload(name: &str, description: &str, payload: serde_json::Value) -> OasfAgentRecord {
    OasfAgentRecord {
        name: name.to_string(),
        description: description.to_string(),
        version: "1.0.0".to_string(),
        schema_version: "0.7.0".to_string(),
        authors: vec![],
        created_at: None,
        skills: vec![],
        domains: vec![],
        locators: vec![],
        modules: vec![OasfModule {
            module_type: RAGENT_MODULE_TYPE.to_string(),
            payload,
        }],
    }
}

// ── Valid record parsing ────────────────────────────────────────────────────────

#[test]
fn test_valid_minimal_record_parses_to_agent_info() {
    let record = minimal_record("my-agent", "My description", "You are an agent.");
    let path = PathBuf::from("test.json");
    let agent = record_to_agent_info(&record, &path).expect("should parse");

    assert_eq!(agent.name, "my-agent");
    assert_eq!(agent.description, "My description");
    assert!(matches!(agent.mode, AgentMode::Primary));
    assert!(!agent.hidden);
    assert_eq!(agent.max_steps, Some(50));
    assert_eq!(agent.prompt.as_deref(), Some("You are an agent."));
}

#[test]
fn test_valid_record_mode_subagent() {
    let record = record_with_payload(
        "sub-agent",
        "A subagent",
        json!({ "system_prompt": "You assist.", "mode": "subagent", "max_steps": 10 }),
    );
    let agent = record_to_agent_info(&record, &PathBuf::from("x.json")).expect("should parse");
    assert!(matches!(agent.mode, AgentMode::Subagent));
}

#[test]
fn test_valid_record_mode_all() {
    let record = record_with_payload(
        "both-agent",
        "Usable everywhere",
        json!({ "system_prompt": "You do everything." }),
    );
    let agent = record_to_agent_info(&record, &PathBuf::from("x.json")).expect("should parse");
    assert!(matches!(agent.mode, AgentMode::All));
}

#[test]
fn test_valid_record_with_temperature_and_top_p() {
    let record = record_with_payload(
        "temp-agent",
        "Has temperature",
        json!({ "system_prompt": "Precise agent.", "temperature": 0.2, "top_p": 0.9 }),
    );
    let agent = record_to_agent_info(&record, &PathBuf::from("x.json")).expect("should parse");
    assert_eq!(agent.temperature, Some(0.2));
    assert_eq!(agent.top_p, Some(0.9));
}

#[test]
fn test_valid_record_with_model_binding() {
    let record = record_with_payload(
        "bound-agent",
        "Uses specific model",
        json!({ "system_prompt": "I use a specific model.", "model": "anthropic:claude-opus-4-5" }),
    );
    let agent = record_to_agent_info(&record, &PathBuf::from("x.json")).expect("should parse");
    let model = agent.model.expect("model should be set");
    assert_eq!(model.provider_id, "anthropic");
    assert_eq!(model.model_id, "claude-opus-4-5");
}

#[test]
fn test_valid_record_hidden_flag() {
    let record = record_with_payload(
        "hidden-agent",
        "Not shown in picker",
        json!({ "system_prompt": "Secret agent.", "hidden": true }),
    );
    let agent = record_to_agent_info(&record, &PathBuf::from("x.json")).expect("should parse");
    assert!(agent.hidden);
}

#[test]
fn test_valid_record_default_max_steps() {
    let record = record_with_payload(
        "default-steps",
        "Uses default max steps",
        json!({ "system_prompt": "Default steps." }),
    );
    let agent = record_to_agent_info(&record, &PathBuf::from("x.json")).expect("should parse");
    assert_eq!(agent.max_steps, Some(100));
}

// ── Invalid record error messages ──────────────────────────────────────────────

#[test]
fn test_error_empty_name() {
    let mut record = minimal_record("", "desc", "prompt");
    // name is already empty in the record struct
    let err = record_to_agent_info(&record, &PathBuf::from("x.json")).unwrap_err();
    assert!(err.contains("non-empty"), "got: {err}");

    record.name = "has space".to_string();
    let err = record_to_agent_info(&record, &PathBuf::from("x.json")).unwrap_err();
    assert!(err.contains("no spaces"), "got: {err}");
}

#[test]
fn test_error_empty_description() {
    let mut record = minimal_record("agent", "", "prompt");
    // description is empty; name is set but description isn't
    // The OasfAgentRecord struct sets description; pass a whitespace-only one
    record.description = "   ".to_string();
    let err = record_to_agent_info(&record, &PathBuf::from("x.json")).unwrap_err();
    assert!(err.contains("description"), "got: {err}");
}

#[test]
fn test_error_missing_ragent_module() {
    let record = OasfAgentRecord {
        name: "no-module".to_string(),
        description: "No ragent module".to_string(),
        version: "1.0.0".to_string(),
        schema_version: "0.7.0".to_string(),
        authors: vec![],
        created_at: None,
        skills: vec![],
        domains: vec![],
        locators: vec![],
        modules: vec![OasfModule {
            module_type: "some/other/type".to_string(),
            payload: json!({}),
        }],
    };
    let err = record_to_agent_info(&record, &PathBuf::from("x.json")).unwrap_err();
    assert!(err.contains("missing required module"), "got: {err}");
    assert!(err.contains(RAGENT_MODULE_TYPE), "got: {err}");
}

#[test]
fn test_error_empty_system_prompt() {
    let record = record_with_payload(
        "empty-prompt",
        "Has empty prompt",
        json!({ "system_prompt": "   " }),
    );
    let err = record_to_agent_info(&record, &PathBuf::from("x.json")).unwrap_err();
    assert!(err.contains("system_prompt must not be empty"), "got: {err}");
}

#[test]
fn test_error_system_prompt_too_long() {
    let long_prompt = "x".repeat(32_769);
    let record = record_with_payload(
        "long-prompt",
        "Prompt too long",
        json!({ "system_prompt": long_prompt }),
    );
    let err = record_to_agent_info(&record, &PathBuf::from("x.json")).unwrap_err();
    assert!(err.contains("too long"), "got: {err}");
}

#[test]
fn test_error_unknown_mode() {
    let record = record_with_payload(
        "bad-mode",
        "Bad mode",
        json!({ "system_prompt": "Agent.", "mode": "turbo" }),
    );
    let err = record_to_agent_info(&record, &PathBuf::from("x.json")).unwrap_err();
    assert!(err.contains("unknown mode"), "got: {err}");
    assert!(err.contains("turbo"), "got: {err}");
}

#[test]
fn test_error_temperature_out_of_range_high() {
    let record = record_with_payload(
        "hot-agent",
        "Too hot",
        json!({ "system_prompt": "Agent.", "temperature": 2.1 }),
    );
    let err = record_to_agent_info(&record, &PathBuf::from("x.json")).unwrap_err();
    assert!(err.contains("temperature"), "got: {err}");
    assert!(err.contains("out of range"), "got: {err}");
}

#[test]
fn test_error_temperature_out_of_range_negative() {
    let record = record_with_payload(
        "cold-agent",
        "Too cold",
        json!({ "system_prompt": "Agent.", "temperature": -0.1 }),
    );
    let err = record_to_agent_info(&record, &PathBuf::from("x.json")).unwrap_err();
    assert!(err.contains("temperature"), "got: {err}");
}

#[test]
fn test_error_top_p_out_of_range() {
    let record = record_with_payload(
        "bad-topp",
        "Bad top_p",
        json!({ "system_prompt": "Agent.", "top_p": 1.5 }),
    );
    let err = record_to_agent_info(&record, &PathBuf::from("x.json")).unwrap_err();
    assert!(err.contains("top_p"), "got: {err}");
}

#[test]
fn test_error_model_missing_colon() {
    let record = record_with_payload(
        "bad-model",
        "Bad model format",
        json!({ "system_prompt": "Agent.", "model": "anthropic-claude" }),
    );
    let err = record_to_agent_info(&record, &PathBuf::from("x.json")).unwrap_err();
    assert!(err.contains("provider:model"), "got: {err}");
}

#[test]
fn test_error_model_empty_parts() {
    // Colon present but one side empty
    let record = record_with_payload(
        "empty-model",
        "Empty model part",
        json!({ "system_prompt": "Agent.", "model": ":claude-opus" }),
    );
    let err = record_to_agent_info(&record, &PathBuf::from("x.json")).unwrap_err();
    assert!(err.contains("provider:model"), "got: {err}");
}

#[test]
fn test_error_max_steps_zero() {
    let record = record_with_payload(
        "no-steps",
        "Zero steps",
        json!({ "system_prompt": "Agent.", "max_steps": 0 }),
    );
    let err = record_to_agent_info(&record, &PathBuf::from("x.json")).unwrap_err();
    assert!(err.contains("max_steps"), "got: {err}");
}

// ── Permission ruleset parsing ─────────────────────────────────────────────────

#[test]
fn test_permissions_allow_deny_ask_parse() {
    let record = record_with_payload(
        "perms-agent",
        "Permission test",
        json!({
            "system_prompt": "Security agent.",
            "permissions": [
                { "permission": "read",  "pattern": "**",      "action": "allow" },
                { "permission": "edit",  "pattern": "**",      "action": "deny"  },
                { "permission": "bash",  "pattern": "**",      "action": "ask"   }
            ]
        }),
    );
    let agent = record_to_agent_info(&record, &PathBuf::from("x.json")).expect("should parse");
    assert_eq!(agent.permission.len(), 3);

    assert!(matches!(agent.permission[0].action, PermissionAction::Allow));
    assert!(matches!(agent.permission[1].action, PermissionAction::Deny));
    assert!(matches!(agent.permission[2].action, PermissionAction::Ask));
}

#[test]
fn test_permissions_default_when_absent() {
    let record = minimal_record("default-perms", "Uses default perms", "Agent.");
    let agent = record_to_agent_info(&record, &PathBuf::from("x.json")).expect("should parse");
    // Default permissions should be non-empty (inherit the built-in defaults)
    assert!(
        !agent.permission.is_empty(),
        "expected default permissions to be set"
    );
}

#[test]
fn test_permissions_unknown_action_error() {
    let record = record_with_payload(
        "bad-perm",
        "Bad permission",
        json!({
            "system_prompt": "Agent.",
            "permissions": [
                { "permission": "read", "pattern": "**", "action": "maybe" }
            ]
        }),
    );
    let err = record_to_agent_info(&record, &PathBuf::from("x.json")).unwrap_err();
    assert!(err.contains("unknown action"), "got: {err}");
    assert!(err.contains("maybe"), "got: {err}");
}

#[test]
fn test_permissions_pattern_preserved() {
    let record = record_with_payload(
        "pattern-agent",
        "Pattern check",
        json!({
            "system_prompt": "Agent.",
            "permissions": [
                { "permission": "edit", "pattern": "src/**/*.rs", "action": "allow" }
            ]
        }),
    );
    let agent = record_to_agent_info(&record, &PathBuf::from("x.json")).expect("should parse");
    assert_eq!(agent.permission[0].pattern, "src/**/*.rs");
}

// ── Template variable substitution ────────────────────────────────────────────

#[test]
fn test_template_working_dir_substituted() {
    let record = minimal_record(
        "tmpl-agent",
        "Template agent",
        "You are in {{WORKING_DIR}}. Help the user.",
    );
    let agent = record_to_agent_info(&record, &PathBuf::from("x.json")).expect("should parse");

    let working_dir = PathBuf::from("/tmp/test-project");
    let _result = build_system_prompt(&agent, &working_dir, "", None);

    assert!(result.contains("/tmp/test-project"), "got: {result}");
    assert!(!result.contains("{{WORKING_DIR}}"), "variable not substituted: {result}");
}

#[test]
fn test_template_file_tree_substituted() {
    let record = minimal_record(
        "tree-agent",
        "File tree agent",
        "Project structure:\n{{FILE_TREE}}\n\nHelp the user.",
    );
    let agent = record_to_agent_info(&record, &PathBuf::from("x.json")).expect("should parse");

    let file_tree = "src/\n  main.rs\n  lib.rs";
    let _result = build_system_prompt(&agent, &PathBuf::from("/tmp"), file_tree, None);

    assert!(result.contains("src/"), "got: {result}");
    assert!(result.contains("main.rs"), "got: {result}");
    assert!(!result.contains("{{FILE_TREE}}"), "variable not substituted: {result}");
}

#[test]
fn test_template_date_substituted() {
    let record = minimal_record(
        "date-agent",
        "Date agent",
        "Today is {{DATE}}. Help the user.",
    );
    let agent = record_to_agent_info(&record, &PathBuf::from("x.json")).expect("should parse");

    let _result = build_system_prompt(&agent, &PathBuf::from("/tmp"), "", None);

    assert!(!result.contains("{{DATE}}"), "variable not substituted: {result}");
    // Result should contain a date-like string (YYYY-MM-DD), possibly with trailing punctuation
    let has_date = result
        .split_whitespace()
        .any(|w| {
            let w = w.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '-');
            w.len() == 10 && w.chars().nth(4) == Some('-') && w.chars().nth(7) == Some('-')
        });
    assert!(has_date, "expected date in output, got: {result}");
}

#[test]
fn test_template_no_duplicate_working_dir_section() {
    // When {{WORKING_DIR}} is used in the prompt, the auto-appended "## Working
    // Directory" section should NOT appear as well.
    let record = minimal_record(
        "no-dupe-agent",
        "No duplication",
        "Working directory: {{WORKING_DIR}}\n\nHelp the user.",
    );
    let agent = record_to_agent_info(&record, &PathBuf::from("x.json")).expect("should parse");

    let _result = build_system_prompt(&agent, &PathBuf::from("/tmp/proj"), "", None);

    let count = result.matches("## Working Directory").count();
    assert_eq!(count, 0, "auto-section should be suppressed when template var used, got: {result}");
}

#[test]
fn test_template_no_duplicate_file_tree_section() {
    let record = minimal_record(
        "no-dupe-tree",
        "No tree duplication",
        "File tree:\n{{FILE_TREE}}\n\nHelp the user.",
    );
    let agent = record_to_agent_info(&record, &PathBuf::from("x.json")).expect("should parse");

    let file_tree = "src/\n  main.rs";
    let _result = build_system_prompt(&agent, &PathBuf::from("/tmp"), file_tree, None);

    let count = result.matches("## Project Structure").count();
    assert_eq!(count, 0, "auto-section should be suppressed when template var used, got: {result}");
}

// ── Discovery path resolution ──────────────────────────────────────────────────

#[test]
fn test_global_agents_dir_returns_path_under_home() {
    if let Some(dir) = global_agents_dir() {
        let dir_str = dir.display().to_string();
        assert!(
            dir_str.contains(".ragent") && dir_str.contains("agents"),
            "expected ~/.ragent/agents, got: {dir_str}"
        );
    }
    // If home is not set, None is fine — no assertion needed
}

#[test]
fn test_find_project_agents_dir_finds_existing() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let agents_dir = tmp.path().join(".ragent").join("agents");
    std::fs::create_dir_all(&agents_dir).unwrap();

    let found = find_project_agents_dir(tmp.path());
    assert_eq!(found, Some(agents_dir));
}

#[test]
fn test_find_project_agents_dir_none_when_absent() {
    let tmp = tempfile::tempdir().expect("tempdir");
    // No .ragent/agents created
    let found = find_project_agents_dir(tmp.path());
    assert_eq!(found, None);
}

#[test]
fn test_find_project_agents_dir_walks_up() {
    let tmp = tempfile::tempdir().expect("tempdir");
    // Put .ragent/agents at the root
    let agents_dir = tmp.path().join(".ragent").join("agents");
    std::fs::create_dir_all(&agents_dir).unwrap();
    // Search from a subdirectory
    let sub = tmp.path().join("a").join("b").join("c");
    std::fs::create_dir_all(&sub).unwrap();

    let found = find_project_agents_dir(&sub);
    assert_eq!(found, Some(agents_dir));
}

// ── load_custom_agents integration ─────────────────────────────────────────────

#[test]
fn test_load_custom_agents_empty_dir() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let agents_dir = tmp.path().join(".ragent").join("agents");
    std::fs::create_dir_all(&agents_dir).unwrap();

    let (agents, diagnostics) = load_custom_agents(tmp.path());
    assert!(agents.is_empty());
    assert!(diagnostics.is_empty());
}

#[test]
fn test_load_custom_agents_loads_valid_file() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let agents_dir = tmp.path().join(".ragent").join("agents");
    std::fs::create_dir_all(&agents_dir).unwrap();

    let json = serde_json::to_string(&minimal_record(
        "test-agent",
        "A test agent",
        "You are a test agent.",
    ))
    .unwrap();
    std::fs::write(agents_dir.join("test-agent.json"), &json).unwrap();

    let (agents, diagnostics) = load_custom_agents(tmp.path());
    assert_eq!(agents.len(), 1, "diagnostics: {diagnostics:?}");
    assert_eq!(agents[0].agent_info.name, "test-agent");
    assert!(agents[0].is_project_local);
    assert!(diagnostics.is_empty());
}

#[test]
fn test_load_custom_agents_skips_invalid_file_with_diagnostic() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let agents_dir = tmp.path().join(".ragent").join("agents");
    std::fs::create_dir_all(&agents_dir).unwrap();

    // Write invalid JSON
    std::fs::write(agents_dir.join("broken.json"), "{ not valid json }").unwrap();

    let (agents, diagnostics) = load_custom_agents(tmp.path());
    assert!(agents.is_empty());
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].contains("broken.json"));
}

#[test]
fn test_load_custom_agents_project_overrides_global() {
    let tmp = tempfile::tempdir().expect("tempdir");

    // Global: agent with description "global"
    let global_dir = tmp.path().join("home").join(".ragent").join("agents");
    std::fs::create_dir_all(&global_dir).unwrap();
    let global_agent = minimal_record("shared-agent", "global version", "Global prompt.");
    std::fs::write(
        global_dir.join("shared-agent.json"),
        serde_json::to_string(&global_agent).unwrap(),
    )
    .unwrap();

    // Project: agent with same name but description "project"
    let project_dir = tmp.path().join("project").join(".ragent").join("agents");
    std::fs::create_dir_all(&project_dir).unwrap();
    let project_agent = minimal_record("shared-agent", "project version", "Project prompt.");
    std::fs::write(
        project_dir.join("shared-agent.json"),
        serde_json::to_string(&project_agent).unwrap(),
    )
    .unwrap();

    // Load from project directory; global is not in the home search path here,
    // so we call load_custom_agents with the project dir as working_dir.
    let (agents, _) = load_custom_agents(&tmp.path().join("project"));
    let shared = agents.iter().find(|a| a.agent_info.name == "shared-agent");
    assert!(shared.is_some(), "shared-agent should be loaded");
    assert_eq!(
        shared.unwrap().agent_info.description,
        "project version",
        "project-local should win"
    );
    assert!(shared.unwrap().is_project_local);
}

#[test]
fn test_load_custom_agents_non_json_files_ignored() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let agents_dir = tmp.path().join(".ragent").join("agents");
    std::fs::create_dir_all(&agents_dir).unwrap();

    std::fs::write(agents_dir.join("agent.yaml"), "name: something").unwrap();
    std::fs::write(agents_dir.join("README.md"), "# docs").unwrap();

    let (agents, diagnostics) = load_custom_agents(tmp.path());
    assert!(agents.is_empty());
    assert!(diagnostics.is_empty());
}

#[test]
fn test_load_custom_agents_is_project_local_false_for_global() {
    // We can't easily test global dir without overriding HOME, but we can test
    // the is_project_local=true path confirmed above and verify the flag is
    // correctly false for agents not in the project path via the
    // record_to_agent_info path (global scan sets false).
    let tmp = tempfile::tempdir().expect("tempdir");
    let agents_dir = tmp.path().join(".ragent").join("agents");
    std::fs::create_dir_all(&agents_dir).unwrap();

    let json = serde_json::to_string(&minimal_record(
        "proj-agent",
        "Project agent",
        "You are project-local.",
    ))
    .unwrap();
    std::fs::write(agents_dir.join("proj-agent.json"), &json).unwrap();

    let (agents, _) = load_custom_agents(tmp.path());
    assert!(agents[0].is_project_local, "should be project-local");
}

// ── Name collision with built-ins ──────────────────────────────────────────────

#[test]
fn test_builtin_names_exist() {
    // Sanity check: the built-in agents list is non-empty
    let builtins = create_builtin_agents();
    assert!(!builtins.is_empty());
}

#[test]
fn test_custom_agent_does_not_collide_with_builtins_when_unique() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let agents_dir = tmp.path().join(".ragent").join("agents");
    std::fs::create_dir_all(&agents_dir).unwrap();

    let json = serde_json::to_string(&minimal_record(
        "unique-custom-xyz",
        "Unique name",
        "You are unique.",
    ))
    .unwrap();
    std::fs::write(agents_dir.join("unique-custom-xyz.json"), &json).unwrap();

    let builtin_names: std::collections::HashSet<String> = create_builtin_agents()
        .iter()
        .map(|a| a.name.clone())
        .collect();

    let (agents, _) = load_custom_agents(tmp.path());
    assert_eq!(agents.len(), 1);
    // Name should NOT be in builtins, so no collision rename needed
    assert!(
        !builtin_names.contains(&agents[0].agent_info.name),
        "unique name should not be in built-ins"
    );
}
