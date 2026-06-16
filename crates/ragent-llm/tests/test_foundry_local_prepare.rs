//! Integration test for Foundry Local model preparation.
//!
//! This test requires Microsoft Foundry Local to be installed and running; it
//! is skipped otherwise so it does not fail in CI or on machines without the
//! runtime.

use std::sync::Arc;

#[tokio::test]
async fn test_foundry_local_prepares_qwen25_coder_with_version_suffix() {
    if !ragent_llm::is_foundry_local_available() {
        println!("Foundry Local not available; skipping integration test");
        return;
    }

    let registry = ragent_llm::provider::create_default_registry();
    let provider = registry
        .get("foundry_local")
        .expect("foundry_local provider registered");

    // Discover models so the test can also exercise discovery.
    let models = provider.discover_models().await.unwrap_or_default();
    println!("discovered {} models", models.len());

    let client = provider
        .create_client("", None, &std::collections::HashMap::new())
        .await
        .expect("create foundry local client");

    let req = ragent_llm::llm::ChatRequest {
        model: "qwen2.5-coder-7b-instruct-generic-cpu:4".to_string(),
        messages: Arc::new(vec![ragent_llm::llm::ChatMessage {
            role: "user".to_string(),
            content: ragent_llm::llm::ChatContent::Text("hello".to_string()),
        }]),
        tools: Arc::new(vec![]),
        temperature: None,
        top_p: None,
        max_tokens: Some(10),
        system: None,
        options: std::collections::HashMap::new(),
        thinking: None,
        session_id: Some("test".to_string()),
        request_id: None,
        stream_timeout_secs: None,
    };

    let mut stream = client.chat(req).await.expect("chat should start");
    let mut saw_text = false;
    use futures::StreamExt;
    while let Some(event) = stream.next().await {
        println!("{:?}", event);
        if matches!(event, ragent_llm::llm::StreamEvent::TextDelta { text: _ }) {
            saw_text = true;
        }
    }
    assert!(saw_text, "should have received text event");
}
