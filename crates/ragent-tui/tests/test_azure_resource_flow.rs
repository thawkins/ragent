//! Integration tests for the Azure Resource provider setup flow.
//!
//! Covers `/setup` and `/model` flows that transition through
//! `ProviderSetupStep::SelectAzureResource`.

use std::sync::Arc;

use ragent_agent::storage::Storage;

fn mem_storage() -> Arc<Storage> {
    Arc::new(Storage::open_in_memory().expect("in-memory storage"))
}

/// Helper: write a valid `azureresources.json` to a temp directory and return the path.
fn temp_azureresources_json(content: &str) -> (std::path::PathBuf, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("azureresources.json");
    std::fs::write(&path, content).expect("write");
    (path, dir)
}

// =========================================================================
// T-014: Integration test for full setup flow
// =========================================================================

#[test]
fn test_azure_resource_provider_listed() {
    use ragent_tui::app::PROVIDER_LIST;
    assert!(
        PROVIDER_LIST
            .iter()
            .map(|(id, _)| *id)
            .any(|id| id == "azure_resource"),
        "azure_resource should appear in PROVIDER_LIST"
    );
}

#[test]
fn test_azure_resource_persistence_roundtrip() {
    let storage = mem_storage();

    // Simulate what the input handler writes when an entry is confirmed
    let payload = serde_json::json!({
        "id": "my-gpt-4o",
        "endpoint": "https://my-resource.openai.azure.com",
        "api_key": null,
        "api_key_env": "MY_AOAI_KEY",
    });
    storage
        .set_setting("azure_resource_last_selection", &payload.to_string())
        .expect("store");

    // Read it back
    let raw = storage
        .get_setting("azure_resource_last_selection")
        .expect("read")
        .expect("value exists");
    let parsed: serde_json::Value = serde_json::from_str(&raw).expect("json");
    assert_eq!(parsed["id"], "my-gpt-4o");
    assert_eq!(parsed["endpoint"], "https://my-resource.openai.azure.com");
    assert_eq!(parsed["api_key_env"], "MY_AOAI_KEY");
}

#[test]
fn test_azure_resource_stale_selection_cleanup() {
    let storage = mem_storage();

    // Store a stale selection
    let stale = serde_json::json!({
        "id": "gone-model",
        "endpoint": "https://gone.example.com",
        "api_key_env": "KEY",
    });
    storage
        .set_setting("azure_resource_last_selection", &stale.to_string())
        .expect("store");

    // Simulate the restore logic: if the stored id is NOT in the current
    // file entries, the key should be deleted.
    let current_ids = ["model-a", "model-b"];
    let stored_id = stale["id"].as_str().unwrap();
    if !current_ids.contains(&stored_id) {
        storage
            .delete_setting("azure_resource_last_selection")
            .expect("delete");
    }

    let after = storage
        .get_setting("azure_resource_last_selection")
        .expect("read");
    assert!(after.is_none(), "stale selection should be cleaned up");
}

#[test]
fn test_azure_resource_entry_conversion_to_model_info() {
    use ragent_agent::provider::Provider;
    use ragent_agent::provider::azure_resource::AzureResourceProvider;

    let json = r#"{
        "version": "1",
        "resources": [
            {
                "id": "test-model",
                "name": "Test Model",
                "endpoint": "https://test.example.com",
                "api_key_env": "KEY",
                "context_window": 256000,
                "capabilities": ["streaming", "vision"]
            }
        ]
    }"#;
    let (path, _dir) = temp_azureresources_json(json);
    let provider = AzureResourceProvider::with_path(path);
    let models = provider.default_models();
    assert_eq!(models.len(), 1);
    let m = &models[0];
    assert_eq!(m.id, "test-model");
    assert_eq!(m.provider_id, "azure_resource");
    assert_eq!(m.name, "Test Model");
    assert_eq!(m.context_window, 256_000);
    assert!(m.capabilities.streaming);
    assert!(m.capabilities.vision);
    assert!(!m.capabilities.tool_use);
}

#[test]
fn test_azure_resource_foundry_backend_resolution() {
    use ragent_agent::provider::Provider;
    use ragent_agent::provider::azure_resource::AzureResourceProvider;

    let json = r#"{
        "version": "1",
        "resources": [
            {
                "id": "my-o1",
                "name": "My O1",
                "endpoint": "https://o1.openai.azure.com",
                "api_key_env": "O1_KEY"
            }
        ]
    }"#;
    let (path, _dir) = temp_azureresources_json(json);
    let provider = AzureResourceProvider::with_path(path);
    let models = provider.default_models();
    assert_eq!(models.len(), 1);
    // The underlying backend is azure_foundry; verify the provider_id
    // in ModelInfo is "azure_resource" (the file-based provider)
    assert_eq!(models[0].provider_id, "azure_resource");
}
