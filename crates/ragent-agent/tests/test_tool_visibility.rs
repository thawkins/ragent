//! Tests for tool visibility configuration.

use ragent_agent::config::{Config, ToolVisibilityConfig, tool_family_names};

#[test]
fn test_default_tool_visibility_config() {
    let config = ToolVisibilityConfig::default();
    assert!(!config.office);
    assert!(!config.journal);
    assert!(!config.github);
    assert!(!config.gitlab);
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
fn test_tool_family_names_journal() {
    let names = tool_family_names("journal").unwrap();
    assert_eq!(names.len(), 3);
    assert!(names.contains(&"journal_write"));
    assert!(names.contains(&"journal_search"));
    assert!(names.contains(&"journal_read"));
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
fn test_tool_family_names_invalid() {
    assert!(tool_family_names("invalid").is_none());
}

#[test]
fn test_serde_defaults() {
    let json = "{}";
    let config: ToolVisibilityConfig = serde_json::from_str(json).unwrap();
    assert!(!config.office);
    assert!(!config.journal);
    assert!(!config.github);
    assert!(!config.gitlab);
    assert!(config.codeindex);
}

#[test]
fn test_serde_custom_values() {
    let json = r#"{
        "office": true,
        "journal": false,
        "github": true,
        "gitlab": true,
        "codeindex": false
    }"#;
    let config: ToolVisibilityConfig = serde_json::from_str(json).unwrap();
    assert!(config.office);
    assert!(!config.journal);
    assert!(config.github);
    assert!(config.gitlab);
    assert!(!config.codeindex);
}

#[test]
fn test_effective_hidden_tools_uses_default_github_gitlab_switches() {
    let config = Config::default();
    let hidden = config.effective_hidden_tools();

    assert!(hidden.contains(&"github_list_issues".to_string()));
    assert!(hidden.contains(&"gitlab_list_issues".to_string()));
}

#[test]
fn test_runtime_merge_preserves_unspecified_tool_visibility_switches() {
    let mut base = Config::default();
    base.tool_visibility.github = true;
    base.tool_visibility.gitlab = true;

    let overlay: Config = serde_json::from_str(
        r#"{
            "tool_visibility": {
                "journal": true
            }
        }"#,
    )
    .expect("overlay should parse");

    let merged = Config::merge(base, overlay);

    assert!(merged.tool_visibility.github);
    assert!(merged.tool_visibility.gitlab);
    assert!(merged.tool_visibility.journal);
}
