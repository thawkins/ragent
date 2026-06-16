//! Integration test for the in-process Foundry Local backend.
//!
//! This test requires Microsoft Foundry Local to be installed and its native
//! core library to be loadable.  It is skipped otherwise so it does not fail in
//! CI or on machines without the runtime.

use std::collections::HashMap;

use ragent_llm::Provider;

#[tokio::test]
async fn test_inproc_client_can_be_constructed() {
    if !ragent_llm::is_foundry_local_available() {
        println!("Foundry Local not available; skipping in-process integration test");
        return;
    }

    let provider = ragent_llm::FoundryLocalProvider::with_full_config(true, None, None, Some(true));
    let mut options = HashMap::new();
    options.insert("in_process".to_string(), serde_json::json!(true));

    let client = provider
        .create_client("", None, &options)
        .await
        .expect("create in-process Foundry Local client");

    // We cannot downcast the boxed LlmClient, but reaching this point means the
    // provider resolved the in_process flag and initialised the SDK manager.
    assert!(!provider.default_models().is_empty());
    let _ = client;
}
