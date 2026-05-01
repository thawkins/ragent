//! Tests for tool visibility configuration.

use ragent_agent::config::{Config, ToolVisibilityConfig, tool_family_names};

#[test]
fn test_default_tool_visibility_config() {
    let config = ToolVisibilityConfig::default();
    assert!(!config.office);
    assert!(!config.github);
    assert!(!config.gitlab);
    assert!(!config.teams);
    assert!(!config.agents);
    assert!(!config.plan);
    assert!(config.codeindex);
}

#[test]
fn test_tool_family_names_office() {
    let names = tool_family_names("office").unwrap();
    assert!(names.contains(&"office_read"));
    assert!(names.contains(&"office_write"));
    assert!(names.contains(&"office_info"));
    assert!(names.contains(&"libre_read"));
    assert!(names.contains(&"libre_write"));
    assert!(names.contains(&"libre_info"));
    assert!(names.contains(&"pdf_read"));
    assert!(names.contains(&"pdf_write"));
}

#[test]
fn test_tool_family_names_github() {
    let names = tool_family_names("github").unwrap();
    assert_eq!(names.len(), 10);
    assert!(names.contains(&"github_list_issues"));
    assert!(names.contains(&"github_review_pr"));
}

#[test]
fn test_tool_family_names_gitlab() {
    let names = tool_family_names("gitlab").unwrap();
    assert_eq!(names.len(), 19);
    assert!(names.contains(&"gitlab_list_issues"));
    assert!(names.contains(&"gitlab_retry_pipeline"));
}

#[test]
fn test_tool_family_names_codeindex() {
    let names = tool_family_names("codeindex").unwrap();
    assert_eq!(names.len(), 6);
    assert!(names.contains(&"codeindex_search"));
    assert!(names.contains(&"codeindex_reindex"));
}

#[test]
fn test_tool_family_names_teams() {
    let names = tool_family_names("teams").unwrap();
    assert_eq!(names.len(), 20);
    assert!(names.contains(&"team_create"));
    assert!(names.contains(&"team_wait"));
}

#[test]
fn test_tool_family_names_agents() {
    let names = tool_family_names("agents").unwrap();
    assert_eq!(names.len(), 5);
    assert!(names.contains(&"new_task"));
    assert!(names.contains(&"wait_tasks"));
}

#[test]
fn test_tool_family_names_plan() {
    let names = tool_family_names("plan").unwrap();
    assert_eq!(names.len(), 2);
    assert!(names.contains(&"plan_enter"));
    assert!(names.contains(&"plan_exit"));
}

#[test]
fn test_tool_family_names_invalid() {
    assert!(tool_family_names("invalid").is_none());
}

#[test]
fn test_serde_defaults() {
    let json = "{}";
    let config: ToolVisibilityConfig = serde_json::from_str(json).unwrap();
    assert!(!config.office);
    assert!(!config.github);
    assert!(!config.gitlab);
    assert!(!config.teams);
    assert!(!config.agents);
    assert!(!config.plan);
    assert!(config.codeindex);
}

#[test]
fn test_serde_custom_values() {
    let json = r#"{
        "office": true,
        "github": true,
        "gitlab": true,
        "teams": true,
        "agents": true,
        "plan": true,
        "codeindex": false
    }"#;
    let config: ToolVisibilityConfig = serde_json::from_str(json).unwrap();
    assert!(config.office);
    assert!(config.github);
    assert!(config.gitlab);
    assert!(config.teams);
    assert!(config.agents);
    assert!(config.plan);
    assert!(!config.codeindex);
}

#[test]
fn test_effective_hidden_tools_uses_default_github_gitlab_switches() {
    let config = Config::default();
    let hidden = config.effective_hidden_tools();

    assert!(hidden.contains(&"github_list_issues".to_string()));
    assert!(hidden.contains(&"gitlab_list_issues".to_string()));
    assert!(hidden.contains(&"team_create".to_string()));
    assert!(hidden.contains(&"new_task".to_string()));
    assert!(hidden.contains(&"plan_enter".to_string()));
}

#[test]
fn test_runtime_merge_preserves_unspecified_tool_visibility_switches() {
    let mut base = Config::default();
    base.tool_visibility.github = true;
    base.tool_visibility.gitlab = true;
    base.tool_visibility.teams = true;
    base.tool_visibility.agents = true;
    base.tool_visibility.plan = true;

    let overlay: Config = serde_json::from_str(
        r#"{
            "tool_visibility": {
                "office": true
            }
        }"#,
    )
    .expect("overlay should parse");

    let merged = Config::merge(base, overlay);

    assert!(merged.tool_visibility.office);
    assert!(merged.tool_visibility.github);
    assert!(merged.tool_visibility.gitlab);
    assert!(merged.tool_visibility.teams);
    assert!(merged.tool_visibility.agents);
    assert!(merged.tool_visibility.plan);
}
