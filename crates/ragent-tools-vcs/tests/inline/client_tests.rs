//! Inline tests for `client.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::{extract_readme_url, gitlab_project_to_metadata, parse_gitlab_tree, top_language};
use serde_json::json;

#[test]
fn test_gitlab_project_all_fields_present() {
    let value = json!({
        "description": "A GitLab project",
        "topics": ["rust", "cli"],
        "star_count": 17,
        "default_branch": "main",
        "name": "my-project",
        "path_with_namespace": "group/my-project",
    });
    let md = gitlab_project_to_metadata(&value, "Rust");
    assert_eq!(md.description, "A GitLab project");
    assert_eq!(md.language, "Rust");
    assert_eq!(md.topics, vec!["rust".to_string(), "cli".to_string()]);
    assert_eq!(md.stargazers_count, 17);
    assert_eq!(md.default_branch, "main");
}

#[test]
fn test_gitlab_project_null_description() {
    let value = json!({
        "description": null,
        "star_count": 0,
        "default_branch": "master",
    });
    let md = gitlab_project_to_metadata(&value, "");
    assert_eq!(md.description, "");
    assert_eq!(md.language, "");
    assert_eq!(md.topics, Vec::<String>::new());
    assert_eq!(md.stargazers_count, 0);
}

#[test]
fn test_gitlab_project_missing_fields_default() {
    let value = json!({});
    let md = gitlab_project_to_metadata(&value, "");
    assert_eq!(md.description, "");
    assert_eq!(md.language, "");
    assert_eq!(md.topics, Vec::<String>::new());
    assert_eq!(md.stargazers_count, 0);
    assert_eq!(md.default_branch, "");
}

#[test]
fn test_gitlab_project_empty_topics_array() {
    let value = json!({
        "description": "desc",
        "topics": [],
        "star_count": 5,
        "default_branch": "develop",
    });
    let md = gitlab_project_to_metadata(&value, "Go");
    assert_eq!(md.topics, Vec::<String>::new());
    assert_eq!(md.language, "Go");
    assert_eq!(md.default_branch, "develop");
}

#[test]
fn test_gitlab_project_topics_with_non_string_entries_filtered() {
    let value = json!({
        "topics": ["valid", 123, true, "also-valid"],
        "star_count": 3,
    });
    let md = gitlab_project_to_metadata(&value, "");
    assert_eq!(
        md.topics,
        vec!["valid".to_string(), "also-valid".to_string()]
    );
}

#[test]
fn test_gitlab_project_star_count_maps_to_stargazers_count() {
    let value = json!({
        "star_count": 999,
    });
    let md = gitlab_project_to_metadata(&value, "");
    assert_eq!(md.stargazers_count, 999);
}

#[test]
fn test_gitlab_project_star_count_as_float_ignored() {
    let value = json!({
        "star_count": 42.5,
    });
    let md = gitlab_project_to_metadata(&value, "");
    assert_eq!(md.stargazers_count, 0);
}

#[test]
fn test_gitlab_project_nested_namespace_metadata() {
    // FR-022: nested namespaces like group/subgroup/project are URL-encoded
    // as a single :id. The metadata mapping does not depend on the namespace
    // shape; this verifies the mapping works for nested projects.
    let value = json!({
        "description": "nested",
        "path_with_namespace": "group/subgroup/project",
        "star_count": 1,
        "default_branch": "main",
        "topics": ["nested"],
    });
    let md = gitlab_project_to_metadata(&value, "Python");
    assert_eq!(md.default_branch, "main");
    assert_eq!(md.topics, vec!["nested".to_string()]);
    assert_eq!(md.language, "Python");
}

#[test]
fn test_top_language_picks_highest_share() {
    let value = json!({
        "Rust": 80.5,
        "Shell": 19.5,
    });
    assert_eq!(top_language(&value), "Rust");
}

#[test]
fn test_top_language_single_language() {
    let value = json!({
        "Go": 100.0,
    });
    assert_eq!(top_language(&value), "Go");
}

#[test]
fn test_top_language_empty_object() {
    let value = json!({});
    assert_eq!(top_language(&value), "");
}

#[test]
fn test_top_language_non_object_response() {
    let value = json!(["Rust", "Go"]);
    assert_eq!(top_language(&value), "");
}

#[test]
fn test_top_language_shares_not_numbers() {
    let value = json!({
        "Rust": "high",
        "Go": "low",
    });
    assert_eq!(top_language(&value), "");
}

#[test]
fn test_top_language_tie_returns_one_of_the_tied() {
    // When shares are equal, the max_by result is implementation-defined
    // but must return one of the tied languages (not empty).
    let value = json!({
        "Rust": 50.0,
        "Go": 50.0,
    });
    let lang = top_language(&value);
    assert!(lang == "Rust" || lang == "Go");
}

// --- parse_gitlab_tree -------------------------------------------------

#[test]
fn test_parse_gitlab_tree_mixed_blobs_and_trees() {
    let value = json!([
        {"id": "abc", "name": "README.md", "type": "blob"},
        {"id": "def", "name": "src", "type": "tree"},
        {"id": "ghi", "name": "Cargo.toml", "type": "blob"}
    ]);
    let tree = parse_gitlab_tree(&value);
    assert_eq!(
        tree,
        vec![
            "README.md".to_string(),
            "src".to_string(),
            "Cargo.toml".to_string()
        ]
    );
}

#[test]
fn test_parse_gitlab_tree_preserves_api_ordering() {
    let value = json!([
        {"name": "zlib", "type": "tree"},
        {"name": "Apple", "type": "blob"},
        {"name": "apple", "type": "blob"}
    ]);
    let tree = parse_gitlab_tree(&value);
    assert_eq!(
        tree,
        vec!["zlib".to_string(), "Apple".to_string(), "apple".to_string()]
    );
}

#[test]
fn test_parse_gitlab_tree_empty_array() {
    let value = json!([]);
    assert_eq!(parse_gitlab_tree(&value), Vec::<String>::new());
}

#[test]
fn test_parse_gitlab_tree_non_array_yields_empty() {
    let value = json!({"message": "404 Not Found"});
    assert_eq!(parse_gitlab_tree(&value), Vec::<String>::new());
}

#[test]
fn test_parse_gitlab_tree_null_value() {
    let value = serde_json::Value::Null;
    assert_eq!(parse_gitlab_tree(&value), Vec::<String>::new());
}

#[test]
fn test_parse_gitlab_tree_entries_without_name_skipped() {
    let value = json!([
        {"name": "keep.rs", "type": "blob"},
        {"id": "abc", "type": "tree"},
        {"name": "also-keep.txt", "type": "blob"}
    ]);
    let tree = parse_gitlab_tree(&value);
    assert_eq!(
        tree,
        vec!["keep.rs".to_string(), "also-keep.txt".to_string()]
    );
}

#[test]
fn test_parse_gitlab_tree_non_string_name_skipped() {
    let value = json!([
        {"name": 123, "type": "blob"},
        {"name": "valid.rs", "type": "blob"}
    ]);
    let tree = parse_gitlab_tree(&value);
    assert_eq!(tree, vec!["valid.rs".to_string()]);
}

#[test]
fn test_parse_gitlab_tree_extra_fields_ignored() {
    let value = json!([
        {"id": "a1", "name": "src", "type": "tree", "path": "src", "mode": "040000"},
        {"id": "b2", "name": "main.rs", "type": "blob", "path": "main.rs", "mode": "100644"}
    ]);
    let tree = parse_gitlab_tree(&value);
    assert_eq!(tree, vec!["src".to_string(), "main.rs".to_string()]);
}

// --- extract_readme_url ------------------------------------------------

#[test]
fn test_extract_readme_url_present() {
    let value = json!({
        "readme_url": "https://gitlab.com/group/project/-/raw/main/README.md"
    });
    assert_eq!(
        extract_readme_url(&value),
        Some("https://gitlab.com/group/project/-/raw/main/README.md".to_string())
    );
}

#[test]
fn test_extract_readme_url_missing() {
    let value = json!({"file_path": "README.md"});
    assert_eq!(extract_readme_url(&value), None);
}

#[test]
fn test_extract_readme_url_null() {
    let value = json!({"readme_url": null});
    assert_eq!(extract_readme_url(&value), None);
}

#[test]
fn test_extract_readme_url_non_string() {
    let value = json!({"readme_url": 123});
    assert_eq!(extract_readme_url(&value), None);
}

#[test]
fn test_extract_readme_url_empty_object() {
    let value = json!({});
    assert_eq!(extract_readme_url(&value), None);
}

#[test]
fn test_extract_readme_url_empty_string() {
    let value = json!({"readme_url": ""});
    assert_eq!(extract_readme_url(&value), Some(String::new()));
}
