//! Regression tests for tool-family visibility config handling.

use ragent_config::{Config, tool_family_names};

#[test]
fn test_tool_visibility_defaults_match_phase_one_plan() {
    let config = Config::default();

    assert!(!config.tool_visibility.office);
    assert!(!config.tool_visibility.journal);
    assert!(!config.tool_visibility.github);
    assert!(!config.tool_visibility.gitlab);
    assert!(config.tool_visibility.codeindex);
}

#[test]
fn test_config_parses_tool_visibility_section() {
    let config: Config = serde_json::from_str(
        r#"{
            "tool_visibility": {
                "office": true,
                "journal": true,
                "codeindex": false
            }
        }"#,
    )
    .expect("config should parse");

    assert!(config.tool_visibility.office);
    assert!(config.tool_visibility.journal);
    assert!(!config.tool_visibility.github);
    assert!(!config.tool_visibility.gitlab);
    assert!(!config.tool_visibility.codeindex);
}

#[test]
fn test_merge_preserves_unspecified_tool_visibility_switches() {
    let mut base = Config::default();
    base.tool_visibility.office = true;
    base.tool_visibility.github = true;
    base.tool_visibility.codeindex = false;

    let overlay: Config = serde_json::from_str(
        r#"{
            "tool_visibility": {
                "journal": true,
                "codeindex": true
            }
        }"#,
    )
    .expect("overlay should parse");

    let merged = Config::merge(base, overlay);

    assert!(merged.tool_visibility.office);
    assert!(merged.tool_visibility.github);
    assert!(merged.tool_visibility.journal);
    assert!(merged.tool_visibility.codeindex);
    assert!(!merged.tool_visibility.gitlab);
}

#[test]
fn test_tool_family_names_returns_expected_family_members() {
    let github = tool_family_names("github").expect("github family should exist");
    assert!(github.contains(&"github_list_issues"));
    assert!(github.contains(&"github_review_pr"));
    assert!(tool_family_names("missing").is_none());
}

#[test]
fn test_effective_hidden_tools_combines_legacy_and_family_switches() {
    let mut config = Config::default();
    config.hidden_tools = vec!["custom_tool".to_string(), "github_list_issues".to_string()];
    config.tool_visibility.github = false;
    config.tool_visibility.codeindex = false;

    let hidden = config.effective_hidden_tools();

    assert!(hidden.contains(&"custom_tool".to_string()));
    assert!(hidden.contains(&"github_list_issues".to_string()));
    assert!(hidden.contains(&"github_review_pr".to_string()));
    assert!(hidden.contains(&"codeindex_search".to_string()));
}

#[test]
fn test_merge_preserves_unspecified_codeindex_fields() {
    let mut base = Config::default();
    base.code_index.enabled = false;
    base.code_index.max_file_size = 2048;
    base.code_index.extra_exclude_dirs = vec!["vendor".to_string()];

    let overlay: Config = serde_json::from_str(
        r#"{
            "code_index": {
                "extra_exclude_patterns": ["*.snap"]
            }
        }"#,
    )
    .expect("overlay should parse");

    let merged = Config::merge(base, overlay);

    assert!(!merged.code_index.enabled);
    assert_eq!(merged.code_index.max_file_size, 2048);
    assert_eq!(
        merged.code_index.extra_exclude_dirs,
        vec!["vendor".to_string()]
    );
    assert_eq!(
        merged.code_index.extra_exclude_patterns,
        vec!["*.snap".to_string()]
    );
}
