//! Tests for config file parsing error messages.

use ragent_config::Config;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_invalid_json_shows_line_and_column() {
    let temp_dir = TempDir::new().unwrap();

    // Set up a fake .ragent directory
    let ragent_dir = temp_dir.path().join(".ragent");
    fs::create_dir(&ragent_dir).unwrap();
    let config_path = ragent_dir.join("ragent.json");

    // Create a config file with invalid JSON (unclosed array)
    let invalid_json = r#"{
    "username": "testuser",
    "permission": [
        {"permission": "file:write"
    ]
}"#;

    fs::write(&config_path, invalid_json).unwrap();

    // Change to the temp directory so Config::load() finds it
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    // Try to load it
    let result = Config::load();

    // Restore original directory
    std::env::set_current_dir(original_dir).unwrap();

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();

    // Verify error message contains:
    // 1. File path reference
    assert!(
        err_msg.contains("ragent.json"),
        "Error should contain file reference. Error was:\n{}",
        err_msg
    );

    // 2. Line number
    assert!(
        err_msg.contains("line"),
        "Error should contain line number. Error was:\n{}",
        err_msg
    );

    // 3. Visual indicator (^) showing where error is
    assert!(
        err_msg.contains("^"),
        "Error should have a caret indicator. Error was:\n{}",
        err_msg
    );

    // 4. Separator line
    assert!(
        err_msg.contains("─"),
        "Error should have a separator. Error was:\n{}",
        err_msg
    );
}

#[test]
fn test_unknown_field_shows_clear_error() {
    let temp_dir = TempDir::new().unwrap();
    let ragent_dir = temp_dir.path().join(".ragent");
    fs::create_dir(&ragent_dir).unwrap();
    let config_path = ragent_dir.join("ragent.json");

    // Unknown field (typo in username)
    let invalid_json = r#"{
    "usernam": "alice",
    "defaultAgent": "build"
}"#;

    fs::write(&config_path, invalid_json).unwrap();

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    let result = Config::load();

    std::env::set_current_dir(original_dir).unwrap();

    // This should succeed (unknown fields are ignored in serde)
    // but if it fails, we should see clear error
    if result.is_err() {
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("line"),
            "Error should identify line. Error was:\n{}",
            err_msg
        );
    }
}

#[test]
fn test_type_mismatch_shows_clear_error() {
    let temp_dir = TempDir::new().unwrap();
    let ragent_dir = temp_dir.path().join(".ragent");
    fs::create_dir(&ragent_dir).unwrap();
    let config_path = ragent_dir.join("ragent.json");

    // Wrong type: permission should be array but is string
    let invalid_json = r#"{
    "username": "bob",
    "permission": "not an array"
}"#;

    fs::write(&config_path, invalid_json).unwrap();

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    let result = Config::load();

    std::env::set_current_dir(original_dir).unwrap();

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();

    // Should identify the problem with clear line reference
    assert!(
        err_msg.contains("line"),
        "Error should contain line number. Error was:\n{}",
        err_msg
    );
    assert!(
        err_msg.contains("^"),
        "Error should contain caret indicator. Error was:\n{}",
        err_msg
    );
}

#[test]
fn test_valid_json_loads_successfully() {
    let temp_dir = TempDir::new().unwrap();
    let ragent_dir = temp_dir.path().join(".ragent");
    fs::create_dir(&ragent_dir).unwrap();
    let config_path = ragent_dir.join("ragent.json");

    let valid_json = r#"{
    "username": "alice",
    "defaultAgent": "general"
}"#;

    fs::write(&config_path, valid_json).unwrap();

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    let result = Config::load();

    std::env::set_current_dir(original_dir).unwrap();

    assert!(result.is_ok(), "Valid JSON should load successfully");
    let config = result.unwrap();
    assert_eq!(config.username.as_deref(), Some("alice"));
    assert_eq!(config.default_agent, "general");
}
