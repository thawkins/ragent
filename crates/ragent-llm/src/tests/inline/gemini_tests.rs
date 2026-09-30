//! Inline tests for `gemini.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;

#[test]
fn test_gemini_discovered_model_to_info_uses_live_thinking_flag() {
    let defaults = gemini_default_models("gemini")
        .into_iter()
        .map(|model| (model.id.clone(), model))
        .collect::<HashMap<_, _>>();
    let model = GeminiDiscoveredModel {
        name: "models/gemini-2.5-pro-preview-05-06".to_string(),
        base_model_id: Some("gemini-2.5-pro-preview-05-06".to_string()),
        display_name: Some("Gemini 2.5 Pro Preview".to_string()),
        input_token_limit: Some(2_000_000),
        output_token_limit: Some(65_536),
        supported_generation_methods: vec!["generateContent".to_string()],
        thinking: Some(true),
    };

    let model = gemini_discovered_model_to_info(model, &defaults).expect("model info");
    assert!(model.capabilities.reasoning);
    assert_eq!(
        model.capabilities.thinking_levels,
        gemini_thinking_levels_for_model("gemini-2.5-pro-preview-05-06")
    );
    assert_eq!(model.context_window, 2_000_000);
    assert_eq!(model.max_output, Some(65_536));
}
