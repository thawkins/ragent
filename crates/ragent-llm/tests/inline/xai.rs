//! Tests for xai.rs (M8/T8.4).
//! Compiled as a submodule via #[path], super::* resolves to the source module.

use super::*;

#[test]
fn test_provider_id() {
    let provider = XaiProvider;
    assert_eq!(provider.id(), "xai");
}

#[test]
fn test_provider_name() {
    let provider = XaiProvider;
    assert_eq!(provider.name(), "xAI");
}

#[test]
fn test_default_models_count() {
    let models = xai_default_models("xai");
    assert_eq!(models.len(), 6);
}

#[test]
fn test_default_models_ids() {
    let models = xai_default_models("xai");
    let ids: Vec<&str> = models.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(
        ids,
        &[
            "grok-3",
            "grok-3-mini",
            "grok-3-mini-fast",
            "grok-2",
            "grok-2-mini",
            "grok-2-vision-1212",
        ]
    );
}

#[test]
fn test_default_models_provider_id() {
    let models = xai_default_models("xai");
    for model in &models {
        assert_eq!(model.provider_id, "xai");
    }
}

#[test]
fn test_vision_only_for_vision_models() {
    let models = xai_default_models("xai");
    for model in &models {
        let expected_vision = model.id.contains("vision");
        assert_eq!(
            model.capabilities.vision, expected_vision,
            "model {} vision={}; expected={}",
            model.id, model.capabilities.vision, expected_vision
        );
    }
}

#[test]
fn test_all_models_support_tool_use() {
    let models = xai_default_models("xai");
    for model in &models {
        assert!(
            model.capabilities.tool_use,
            "model {} should support tool_use",
            model.id
        );
    }
}

#[test]
fn test_all_models_support_streaming() {
    let models = xai_default_models("xai");
    for model in &models {
        assert!(
            model.capabilities.streaming,
            "model {} should support streaming",
            model.id
        );
    }
}

#[test]
fn test_context_window() {
    let models = xai_default_models("xai");
    for model in &models {
        assert_eq!(
            model.context_window, 131_072,
            "model {} context_window should be 131072",
            model.id
        );
    }
}

#[test]
fn test_max_output() {
    let models = xai_default_models("xai");
    for model in &models {
        assert_eq!(
            model.max_output,
            Some(16_384),
            "model {} max_output should be 16384",
            model.id
        );
    }
}

#[test]
fn test_vendor_suffix_stripping() {
    assert_eq!(resolve_xai_model_id("grok-3@xai"), "grok-3");
    assert_eq!(
        resolve_xai_model_id("grok-2-vision-1212@xai"),
        "grok-2-vision-1212"
    );
    assert_eq!(resolve_xai_model_id("grok-3@XAI"), "grok-3");
}

#[test]
fn test_vendor_suffix_non_xai_unchanged() {
    // Non-xAI suffixes should be left unchanged
    assert_eq!(resolve_xai_model_id("grok-3@other"), "grok-3@other");
}

#[test]
fn test_no_suffix_unchanged() {
    assert_eq!(resolve_xai_model_id("grok-3"), "grok-3");
    assert_eq!(
        resolve_xai_model_id("grok-2-vision-1212"),
        "grok-2-vision-1212"
    );
}

#[test]
fn test_alias_resolution() {
    assert_eq!(resolve_xai_model_id("grok3"), "grok-3");
    assert_eq!(resolve_xai_model_id("grok2"), "grok-2");
    assert_eq!(resolve_xai_model_id("grok2mini"), "grok-2-mini");
    assert_eq!(resolve_xai_model_id("grok2vision"), "grok-2-vision-1212");
    assert_eq!(resolve_xai_model_id("grok3mini"), "grok-3-mini");
    assert_eq!(resolve_xai_model_id("grok3minifast"), "grok-3-mini-fast");
}

#[test]
fn test_alias_case_insensitive() {
    assert_eq!(resolve_xai_model_id("Grok3"), "grok-3");
    assert_eq!(resolve_xai_model_id("GROK2"), "grok-2");
}

#[test]
fn test_base_url_default() {
    // When no env var and no base_url, should default to XAI_API_BASE
    // We can't easily test env var behavior in unit tests, but we verify
    // the constant is correct.
    assert_eq!(XAI_API_BASE, "https://api.x.ai");
}

#[test]
fn test_base_url_env_key() {
    assert_eq!(XAI_API_BASE_ENV, "XAI_API_BASE");
}

#[test]
fn test_model_costs() {
    let models = xai_default_models("xai");
    let grok3 = models.iter().find(|m| m.id == "grok-3").unwrap();
    assert_eq!(grok3.cost.input, 3.00);
    assert_eq!(grok3.cost.output, 15.00);

    let grok3mini = models.iter().find(|m| m.id == "grok-3-mini").unwrap();
    assert_eq!(grok3mini.cost.input, 0.35);
    assert_eq!(grok3mini.cost.output, 0.50);

    let grok2 = models.iter().find(|m| m.id == "grok-2").unwrap();
    assert_eq!(grok2.cost.input, 2.00);
    assert_eq!(grok2.cost.output, 10.00);

    let grok2vision = models
        .iter()
        .find(|m| m.id == "grok-2-vision-1212")
        .unwrap();
    assert_eq!(grok2vision.cost.input, 2.00);
    assert_eq!(grok2vision.cost.output, 10.00);
}
