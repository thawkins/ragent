//! Inline tests for `anthropic.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;

#[test]
fn test_anthropic_model_to_info_uses_live_thinking_capability() {
    let defaults = AnthropicProvider
        .default_models()
        .into_iter()
        .map(|model| (model.id.clone(), model))
        .collect::<HashMap<_, _>>();
    let entry = json!({
        "id": "claude-sonnet-4-20250514",
        "display_name": "Claude Sonnet 4",
        "context_window": 250_000,
        "max_output_tokens": 32000,
        "capabilities": {
            "thinking": {
                "supported": true,
                "types": ["adaptive", "enabled"]
            }
        }
    });

    let model = anthropic_model_to_info(&entry, &defaults).expect("model info");
    assert!(model.capabilities.reasoning);
    assert_eq!(model.capabilities.thinking_levels, full_reasoning_levels());
    assert_eq!(model.context_window, 250_000);
    assert_eq!(model.max_output, Some(32_000));
}
