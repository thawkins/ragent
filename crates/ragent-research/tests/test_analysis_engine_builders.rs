//! Pure-logic tests for the `LlmAnalysisEngine` builder/accessor surface and
//! the `NoopAnalysisEngine` trait implementation (no network access).
//!
//! Constructing the engine only needs an (empty) provider registry; the
//! builder methods merely set fields, and `Debug`/accessors/`with_brief` never
//! touch the network.

use ragent_llm::provider::ProviderRegistry;
use ragent_research::analysis::{
    AnalysisEngine, AnalysisResult, LlmAnalysisEngine, NoopAnalysisEngine,
};
use ragent_research::run_config::OutputFormat;
use std::sync::Arc;

fn engine() -> LlmAnalysisEngine {
    LlmAnalysisEngine::new(Arc::new(ProviderRegistry::new()), "p", "m")
}

#[test]
fn builder_methods_set_and_expose_fields() {
    let engine = engine()
        .with_api_key(Some("secret".to_string()))
        .with_base_url(Some("https://api.example.com".to_string()))
        .with_persona(Some("You are terse.".to_string()))
        .with_output_format(Some(OutputFormat::Imrad))
        .with_source_summary_budget(Some(1234))
        .with_synthesis_chunk_threshold(Some(9999))
        .with_brief(Some("mission".to_string()));
    assert_eq!(engine.api_key(), Some("secret"));
    assert_eq!(engine.base_url(), Some("https://api.example.com"));

    let plain = engine_no_extras();
    assert_eq!(plain.api_key(), None);
    assert_eq!(plain.base_url(), None);
}

fn engine_no_extras() -> LlmAnalysisEngine {
    engine()
}

#[test]
fn debug_impl_omits_secrets_but_reports_presence() {
    let engine = engine()
        .with_api_key(Some("super-secret".to_string()))
        .with_persona(Some("persona".to_string()));
    let rendered = format!("{engine:?}");
    assert!(rendered.contains("has_api_key: true"));
    assert!(rendered.contains("has_persona: true"));
    // The Debug impl must never leak the key value.
    assert!(!rendered.contains("super-secret"));
}

#[test]
fn validate_provider_rejects_unknown_and_accepts_known() {
    let unknown = engine();
    assert!(unknown.validate_provider().is_err());

    let known = LlmAnalysisEngine::new(
        Arc::new(ragent_llm::provider::create_default_registry()),
        "anthropic",
        "claude-sonnet-4",
    );
    assert!(known.validate_provider().is_ok());
}

#[test]
fn with_brief_clones_engine_and_keeps_config() {
    let engine = engine().with_api_key(Some("k".to_string()));
    let rebound = engine.with_brief(Some("new brief".to_string()));
    // The trait method returns a usable engine; provider validation still
    // behaves consistently.
    assert!(rebound.validate_provider().is_err());
}

#[test]
fn noop_engine_reports_marker_and_default_result() {
    let noop = NoopAnalysisEngine;
    assert!(noop.is_noop_marker());
    assert!(noop.validate_provider().is_ok());
    // The default `analyze_with_outcome` delegates to `analyze`, which for the
    // no-op engine yields an empty result tagged `Llm`.
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let (result, _outcome) = rt
        .block_on(noop.analyze_with_outcome("topic", &[]))
        .expect("noop analyze succeeds");
    assert_eq!(result, AnalysisResult::default());
    let rebound = noop.with_brief(Some("ignored".to_string()));
    assert!(rebound.is_noop_marker());
}
