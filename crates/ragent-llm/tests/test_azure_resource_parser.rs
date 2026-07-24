//! Unit tests for the Azure Resource Provider JSON parser.

use std::io::Write;
use std::path::PathBuf;

use ragent_llm::Provider;
use ragent_llm::provider::azure_resource::{AzureResourceProvider, parse_azure_resources};
use tempfile::NamedTempFile;

/// Helper: write `content` to a temporary file and return the path.
fn temp_file(content: &str) -> NamedTempFile {
    let mut file = NamedTempFile::new().expect("tempfile creation");
    file.write_all(content.as_bytes()).expect("write");
    file.flush().expect("flush");
    file
}

#[test]
fn test_parse_valid_file() {
    let json = r#"{
        "version": "1",
        "resources": [
            {
                "id": "my-gpt-4o",
                "name": "My Azure GPT-4o",
                "endpoint": "https://my-resource.openai.azure.com",
                "api_key_env": "MY_AOAI_KEY",
                "context_window": 128000,
                "capabilities": ["streaming", "vision", "tool_use"]
            }
        ]
    }"#;
    let file = temp_file(json);
    let entries = parse_azure_resources(file.path()).unwrap();
    assert_eq!(entries.len(), 1);
    let entry = &entries[0];
    assert_eq!(entry.id, "my-gpt-4o");
    assert_eq!(entry.name, "My Azure GPT-4o");
    assert_eq!(entry.endpoint, "https://my-resource.openai.azure.com");
    assert_eq!(entry.api_key_env.as_deref(), Some("MY_AOAI_KEY"));
    assert_eq!(entry.api_key, None);
    assert_eq!(entry.context_window, Some(128_000));
    assert_eq!(
        entry.capabilities.as_ref().unwrap(),
        &vec![
            "streaming".to_string(),
            "vision".to_string(),
            "tool_use".to_string()
        ]
    );
    assert!(entry.thinking.is_none());
}

#[test]
fn test_parse_missing_mandatory_field() {
    let json = r#"{
        "version": "1",
        "resources": [
            {
                "id": "valid-id",
                "name": "Valid Name",
                "endpoint": "https://valid.example.com",
                "api_key_env": "VALID_KEY"
            },
            {
                "id": "",
                "name": "Missing ID",
                "endpoint": "https://missing-id.example.com",
                "api_key_env": "KEY"
            }
        ]
    }"#;
    let file = temp_file(json);
    let entries = parse_azure_resources(file.path()).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].id, "valid-id");
}

#[test]
fn test_parse_wrong_version() {
    let json = r#"{
        "version": "2",
        "resources": [
            {
                "id": "x",
                "name": "X",
                "endpoint": "https://x.example.com",
                "api_key_env": "KEY"
            }
        ]
    }"#;
    let file = temp_file(json);
    let result = parse_azure_resources(file.path());
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("version"),
        "Error should mention version: {err}"
    );
}

#[test]
fn test_parse_duplicate_ids() {
    let json = r#"{
        "version": "1",
        "resources": [
            {
                "id": "dup-id",
                "name": "First",
                "endpoint": "https://first.example.com",
                "api_key_env": "KEY1"
            },
            {
                "id": "dup-id",
                "name": "Second",
                "endpoint": "https://second.example.com",
                "api_key_env": "KEY2"
            }
        ]
    }"#;
    let file = temp_file(json);
    let entries = parse_azure_resources(file.path()).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, "First");
}

#[test]
fn test_parse_missing_file() {
    let path = std::path::PathBuf::from("/nonexistent/path/azureresources.json");
    let result = parse_azure_resources(&path);
    assert!(result.is_err());
}

#[test]
fn test_parse_malformed_json() {
    let json = r"{ not valid json }";
    let file = temp_file(json);
    let result = parse_azure_resources(file.path());
    assert!(result.is_err());
}

#[test]
fn test_parse_entry_without_api_key_or_env() {
    let json = r#"{
        "version": "1",
        "resources": [
            {
                "id": "no-key",
                "name": "No Key",
                "endpoint": "https://no-key.example.com"
            }
        ]
    }"#;
    let file = temp_file(json);
    let entries = parse_azure_resources(file.path()).unwrap();
    assert!(entries.is_empty());
}

#[test]
fn test_optional_fields_defaults() {
    let json = r#"{
        "version": "1",
        "resources": [
            {
                "id": "minimal",
                "name": "Minimal",
                "endpoint": "https://minimal.example.com",
                "api_key": "sk-123"
            }
        ]
    }"#;
    let file = temp_file(json);
    let entries = parse_azure_resources(file.path()).unwrap();
    assert_eq!(entries.len(), 1);
    let entry = &entries[0];
    assert_eq!(entry.id, "minimal");
    assert_eq!(entry.api_key, Some("sk-123".to_string()));
    assert_eq!(entry.api_key_env, None);
    assert_eq!(entry.context_window, None);
    assert_eq!(entry.capabilities, None);
    assert!(entry.thinking.is_none());
}

#[test]
fn test_provider_id_and_name() {
    let provider = AzureResourceProvider::new();
    assert_eq!(provider.id(), "azure_resource");
    assert_eq!(provider.name(), "Azure Resource (File)");
}

#[test]
fn test_provider_with_explicit_path() {
    let json = r#"{
        "version": "1",
        "resources": [
            {
                "id": "custom",
                "name": "Custom Model",
                "endpoint": "https://custom.example.com",
                "api_key_env": "KEY"
            }
        ]
    }"#;
    let file = temp_file(json);
    let provider = AzureResourceProvider::with_path(file.path().to_path_buf());
    let models = provider.default_models();
    assert_eq!(models.len(), 1);
    assert_eq!(models[0].id, "custom");
    assert_eq!(models[0].name, "Custom Model");
    assert_eq!(models[0].provider_id, "azure_resource");
    assert_eq!(models[0].context_window, 128_000);
}

#[test]
fn test_provider_model_capabilities_parsing() {
    let json = r#"{
        "version": "1",
        "resources": [
            {
                "id": "cap-model",
                "name": "Cap Model",
                "endpoint": "https://cap.example.com",
                "api_key_env": "KEY",
                "capabilities": ["reasoning", "vision"],
                "context_window": 256000
            }
        ]
    }"#;
    let file = temp_file(json);
    let provider = AzureResourceProvider::with_path(file.path().to_path_buf());
    let models = provider.default_models();
    assert_eq!(models.len(), 1);
    let model = &models[0];
    assert!(model.capabilities.reasoning);
    assert!(model.capabilities.vision);
    // When capabilities are explicitly listed, only listed ones are enabled
    assert!(!model.capabilities.streaming);
    assert!(!model.capabilities.tool_use);
    assert_eq!(model.context_window, 256_000);
}

#[test]
fn test_provider_empty_when_file_missing() {
    let provider = AzureResourceProvider::with_path(PathBuf::from("/does/not/exist.json"));
    let models = provider.default_models();
    assert!(models.is_empty());
}

#[test]
fn test_parse_with_thinking_config() {
    let json = r#"{
        "version": "1",
        "resources": [
            {
                "id": "thinky",
                "name": "Thinky Model",
                "endpoint": "https://thinky.example.com",
                "api_key_env": "KEY",
                "thinking": { "enabled": true, "level": "medium", "budget_tokens": 8192 }
            }
        ]
    }"#;
    let file = temp_file(json);
    let entries = parse_azure_resources(file.path()).unwrap();
    assert_eq!(entries.len(), 1);
    let thinking = entries[0].thinking.as_ref().unwrap();
    assert!(thinking.is_effective_enabled());
}

#[test]
fn test_api_type_openai_accepted() {
    let json = r#"{
        "version": "1",
        "resources": [
            {
                "id": "openai-model",
                "name": "OpenAI Model",
                "endpoint": "https://openai.example.com",
                "api_key_env": "KEY",
                "api_type": "openai"
            }
        ]
    }"#;
    let file = temp_file(json);
    let entries = parse_azure_resources(file.path()).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].api_type.as_deref(), Some("openai"));
}

#[test]
fn test_api_type_anthropic_accepted() {
    let json = r#"{
        "version": "1",
        "resources": [
            {
                "id": "anthropic-model",
                "name": "Anthropic Model",
                "endpoint": "https://anthropic.example.com",
                "api_key_env": "KEY",
                "api_type": "anthropic"
            }
        ]
    }"#;
    let file = temp_file(json);
    let entries = parse_azure_resources(file.path()).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].api_type.as_deref(), Some("anthropic"));
}

#[test]
fn test_api_type_missing_defaults_to_openai() {
    let json = r#"{
        "version": "1",
        "resources": [
            {
                "id": "default-model",
                "name": "Default Model",
                "endpoint": "https://default.example.com",
                "api_key_env": "KEY"
            }
        ]
    }"#;
    let file = temp_file(json);
    let entries = parse_azure_resources(file.path()).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].api_type.as_deref(), None);
}

#[test]
fn test_api_type_invalid_skipped_with_warning() {
    let json = r#"{
        "version": "1",
        "resources": [
            {
                "id": "gemini-model",
                "name": "Gemini Model",
                "endpoint": "https://gemini.example.com",
                "api_key_env": "KEY",
                "api_type": "gemini"
            },
            {
                "id": "valid-model",
                "name": "Valid Model",
                "endpoint": "https://valid.example.com",
                "api_key_env": "KEY",
                "api_type": "openai"
            }
        ]
    }"#;
    let file = temp_file(json);
    let entries = parse_azure_resources(file.path()).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].id, "valid-model");
}
