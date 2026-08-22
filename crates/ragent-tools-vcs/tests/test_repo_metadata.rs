//! Tests for `RepoMetadata::from_response`, the pure parser behind
//! `GitHubClient::fetch_repo_metadata` (FR-005).

use ragent_tools_vcs::github::RepoMetadata;
use serde_json::json;

#[test]
fn test_from_response_all_fields_present() {
    let value = json!({
        "description": "A hello-world app",
        "language": "Rust",
        "topics": ["cli", "ai", "rust"],
        "stargazers_count": 42,
        "default_branch": "main",
        "name": "Hello-World",
        "full_name": "octocat/Hello-World",
    });
    let md = RepoMetadata::from_response(&value);
    assert_eq!(md.description, "A hello-world app");
    assert_eq!(md.language, "Rust");
    assert_eq!(
        md.topics,
        vec!["cli".to_string(), "ai".to_string(), "rust".to_string()]
    );
    assert_eq!(md.stargazers_count, 42);
    assert_eq!(md.default_branch, "main");
}

#[test]
fn test_from_response_null_description_and_language() {
    let value = json!({
        "description": null,
        "language": null,
        "stargazers_count": 0,
        "default_branch": "main",
    });
    let md = RepoMetadata::from_response(&value);
    assert_eq!(md.description, "");
    assert_eq!(md.language, "");
    assert!(md.topics.is_empty());
    assert_eq!(md.stargazers_count, 0);
    assert_eq!(md.default_branch, "main");
}

#[test]
fn test_from_response_missing_fields_default() {
    let value = json!({});
    let md = RepoMetadata::from_response(&value);
    assert_eq!(md.description, "");
    assert_eq!(md.language, "");
    assert!(md.topics.is_empty());
    assert_eq!(md.stargazers_count, 0);
    assert_eq!(md.default_branch, "");
}

#[test]
fn test_from_response_empty_topics_array() {
    let value = json!({
        "description": "desc",
        "language": "Python",
        "topics": [],
        "stargazers_count": 10,
        "default_branch": "master",
    });
    let md = RepoMetadata::from_response(&value);
    assert!(md.topics.is_empty());
    assert_eq!(md.language, "Python");
    assert_eq!(md.default_branch, "master");
}

#[test]
fn test_from_response_topics_with_non_string_entries_filtered() {
    let value = json!({
        "description": "desc",
        "language": "Go",
        "topics": ["valid", 123, true, "also-valid"],
        "stargazers_count": 100,
        "default_branch": "develop",
    });
    let md = RepoMetadata::from_response(&value);
    assert_eq!(
        md.topics,
        vec!["valid".to_string(), "also-valid".to_string()]
    );
    assert_eq!(md.stargazers_count, 100);
}

#[test]
fn test_from_response_star_count_as_float_ignored() {
    let value = json!({
        "stargazers_count": 42.5,
    });
    let md = RepoMetadata::from_response(&value);
    assert_eq!(md.stargazers_count, 0);
}
