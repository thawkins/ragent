//! Coverage for the virtual `RouterProvider` (id/name/config plumbing and
//! provider-registry wiring).

use std::collections::HashMap;

use ragent_llm::Provider;
use ragent_llm::provider::ProviderRegistry;
use ragent_llm::providers::router::RouterProvider;
use ragent_llm::providers::router_config::RouterConfig;

#[test]
fn test_router_provider_identity_and_default_models() {
    let provider = RouterProvider::with_defaults();
    assert_eq!(provider.id(), "router");
    assert_eq!(provider.name(), "Model Router");
    let models = provider.default_models();
    assert_eq!(models.len(), 1);
    assert_eq!(models[0].id, "router");
    assert_eq!(models[0].provider_id, "router");
    assert!(models[0].capabilities.streaming);
    assert!(models[0].capabilities.tool_use);
    assert!(!models[0].capabilities.vision);
    assert!(models[0].max_output.is_none());
}

#[test]
fn test_router_provider_config_mutation() {
    let provider = RouterProvider::with_defaults();
    let initial_enabled = provider.is_enabled();
    provider.set_enabled(!initial_enabled);
    assert_eq!(provider.is_enabled(), !initial_enabled);

    let new_config = RouterConfig {
        enabled: true,
        ..RouterConfig::default()
    };
    provider.reload_config(new_config);
    assert!(provider.is_enabled());
    assert!(provider.config().enabled);
}

#[test]
fn test_router_provider_registry_storage_event_bus_wiring() {
    let provider = RouterProvider::with_defaults();
    assert!(provider.registry().is_none());
    assert!(provider.storage().is_none());
    assert!(provider.event_bus().is_none());

    let registry = std::sync::Arc::new(ProviderRegistry::new());
    provider.set_registry(registry);
    assert!(provider.registry().is_some());

    let bus = std::sync::Arc::new(ragent_types::event::EventBus::new(8));
    provider.set_event_bus(Some(bus));
    assert!(provider.event_bus().is_some());
    provider.set_event_bus(None);
    assert!(provider.event_bus().is_none());

    // The trait method must reach the inherent one (no-op override path).
    let bus2 = std::sync::Arc::new(ragent_types::event::EventBus::new(8));
    Provider::set_event_bus(&provider, Some(bus2));
    assert!(provider.event_bus().is_some());

    // as_any_static returns the concrete instance so downcasting succeeds.
    assert!(
        provider
            .as_any_static()
            .downcast_ref::<RouterProvider>()
            .is_some()
    );
}

#[tokio::test]
async fn test_router_provider_create_client_builds_router_client() {
    let provider = RouterProvider::with_defaults();
    let client = provider
        .create_client("", None, &HashMap::new())
        .await
        .expect("router client");
    // A RouterClient is produced; a chat with no registry must surface an error
    // rather than panic.
    let request = ragent_llm::llm::ChatRequest {
        model: "router".to_string(),
        messages: std::sync::Arc::new(vec![ragent_llm::llm::ChatMessage {
            role: "user".to_string(),
            content: ragent_llm::llm::ChatContent::Text("hello".to_string()),
        }]),
        tools: std::sync::Arc::new(vec![]),
        temperature: None,
        top_p: None,
        max_tokens: Some(16),
        system: None,
        options: HashMap::new(),
        session_id: None,
        request_id: None,
        stream_timeout_secs: Some(5),
        thinking: None,
    };
    assert!(
        client.chat(request).await.is_err(),
        "chat without a registry should error"
    );
}
