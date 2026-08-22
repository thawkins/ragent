//! Tests for `extract_download_url`, the pure parser behind
//! `GitHubClient::fetch_readme` (FR-007).

use ragent_tools_vcs::github::extract_download_url;
use serde_json::json;

#[test]
fn test_extract_download_url_present() {
    let value = json!({
        "name": "README.md",
        "path": "README.md",
        "sha": "abc123",
        "size": 1024,
        "url": "https://api.github.com/repos/o/r/contents/README.md?ref=main",
        "html_url": "https://github.com/o/r/blob/main/README.md",
        "git_url": "https://api.github.com/repos/o/r/git/blobs/abc123",
        "download_url": "https://raw.githubusercontent.com/o/r/main/README.md",
        "type": "file",
        "encoding": "base64",
        "content": "IyBIZWxsbyBXb3JsZA==\n",
        "_links": {
            "self": "https://api.github.com/repos/o/r/contents/README.md?ref=main",
            "git": "https://api.github.com/repos/o/r/git/blobs/abc123",
            "html": "https://github.com/o/r/blob/main/README.md"
        }
    });
    let url = extract_download_url(&value).expect("download_url should be present");
    assert_eq!(url, "https://raw.githubusercontent.com/o/r/main/README.md");
}

#[test]
fn test_extract_download_url_missing() {
    let value = json!({
        "name": "README.md",
        "path": "README.md"
    });
    assert!(extract_download_url(&value).is_none());
}

#[test]
fn test_extract_download_url_null() {
    let value = json!({
        "name": "README.md",
        "download_url": null
    });
    assert!(extract_download_url(&value).is_none());
}

#[test]
fn test_extract_download_url_non_string() {
    let value = json!({
        "download_url": 42
    });
    assert!(extract_download_url(&value).is_none());
}

#[test]
fn test_extract_download_url_empty_string() {
    // GitHub can return an empty download_url for certain file types; the
    // parser returns Some("") so the caller can decide how to handle it.
    let value = json!({"download_url": ""});
    let url = extract_download_url(&value).expect("empty string is still a string");
    assert_eq!(url, "");
}

#[test]
fn test_extract_download_url_empty_object() {
    let value = json!({});
    assert!(extract_download_url(&value).is_none());
}

#[test]
fn test_extract_download_url_null_value() {
    let value = serde_json::Value::Null;
    assert!(extract_download_url(&value).is_none());
}
