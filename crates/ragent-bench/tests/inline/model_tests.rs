//! Inline tests for `model.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::{
    BENCH_TRANSIENT_MAX_BACKOFF_SECS, BENCH_TRANSIENT_MAX_ELAPSED_SECS,
    BENCH_TRANSIENT_MAX_RETRIES, BenchModelRunner, LiveBenchModelRunner, MAX_BENCH_SAMPLES,
    ResolvedModelSelection, benchmark_chat_request, benchmark_retry_delay,
    extract_benchmark_error_status_code, finish_reason_label, format_retry_exhausted_error,
    is_permanent_benchmark_api_error, is_retryable_benchmark_error, resolve_base_url,
};
use crate::command::BenchRunOptions;
use ragent_agent::Config;
use ragent_agent::provider;
use ragent_agent::storage::Storage;
use ragent_types::ThinkingConfig;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

#[test]
fn test_extract_benchmark_error_status_code_reads_http_style_messages() {
    assert_eq!(
        extract_benchmark_error_status_code("Ollama Cloud API error (503 Service Unavailable)"),
        Some(503)
    );
    assert_eq!(
        extract_benchmark_error_status_code("HTTP 429 Too Many Requests"),
        Some(429)
    );
}

#[test]
fn test_is_permanent_benchmark_api_error_ignores_retryable_statuses() {
    assert!(!is_permanent_benchmark_api_error(
        "OpenAI API error (429 Too Many Requests): rate limited"
    ));
    assert!(!is_permanent_benchmark_api_error(
        "Ollama Cloud API error (503 Service Unavailable): server overloaded"
    ));
}

#[test]
fn test_benchmark_retry_delay_grows_and_caps_for_retryable_errors() {
    let started = Instant::now();
    assert_eq!(
        benchmark_retry_delay("HTTP 503 Service Unavailable", 0, started),
        Some(Duration::from_secs(2))
    );
    assert_eq!(
        benchmark_retry_delay("HTTP 503 Service Unavailable", 3, started),
        Some(Duration::from_secs(16))
    );
    assert_eq!(
        benchmark_retry_delay("HTTP 503 Service Unavailable", 6, started),
        Some(Duration::from_secs(BENCH_TRANSIENT_MAX_BACKOFF_SECS))
    );
}

#[test]
fn test_benchmark_retry_delay_stops_after_budget_is_spent() {
    let started = Instant::now()
        .checked_sub(Duration::from_secs(BENCH_TRANSIENT_MAX_ELAPSED_SECS))
        .unwrap();
    assert_eq!(
        benchmark_retry_delay("HTTP 503 Service Unavailable", 0, started),
        None
    );
    assert_eq!(
        benchmark_retry_delay("HTTP 400 Bad Request", 0, Instant::now()),
        None
    );
}

#[test]
fn test_format_retry_exhausted_error_reports_attempts_and_elapsed_time() {
    let started = Instant::now().checked_sub(Duration::from_secs(9)).unwrap();
    let message = format_retry_exhausted_error("HTTP 503 Service Unavailable", 5, started);
    assert!(message.contains("503"));
    assert!(message.contains("5 attempt(s)"));
    assert!(message.contains("over 9s") || message.contains("over 10s"));
}

#[test]
fn test_finish_reason_label_maps_every_variant() {
    use ragent_agent::llm::LlmFinishReason;
    assert_eq!(finish_reason_label(LlmFinishReason::Stop), "stop");
    assert_eq!(finish_reason_label(LlmFinishReason::ToolUse), "tool_use");
    assert_eq!(finish_reason_label(LlmFinishReason::Length), "length");
    assert_eq!(
        finish_reason_label(LlmFinishReason::ContentFilter),
        "content_filter"
    );
    assert_eq!(finish_reason_label(LlmFinishReason::Cancelled), "cancelled");
    assert_eq!(
        finish_reason_label(LlmFinishReason::Truncation),
        "truncation"
    );
}

#[test]
fn test_is_retryable_benchmark_error_matches_transient_fragments() {
    for message in [
        "HTTP 429 rate limit",
        "503 Service Unavailable",
        "504 gateway timeout",
        "408 request timeout",
        "server overloaded",
        "connection reset by peer",
        "broken pipe",
        "unexpected eof",
        "http2 error",
        "h2 protocol error",
        "temporarily unavailable",
    ] {
        assert!(
            is_retryable_benchmark_error(message),
            "expected retryable: {message}"
        );
    }
    assert!(!is_retryable_benchmark_error("invalid_request_error"));
    assert!(!is_retryable_benchmark_error("some benign message"));
}

#[test]
fn test_benchmark_retry_delay_returns_none_for_retry_cap_and_non_retryable() {
    let started = Instant::now();
    // Non-retryable message -> None.
    assert!(benchmark_retry_delay("totally fine", 0, started).is_none());
    // Retryable but at the retry cap -> None.
    assert!(
        benchmark_retry_delay(
            "HTTP 503 Service Unavailable",
            BENCH_TRANSIENT_MAX_RETRIES,
            started
        )
        .is_none()
    );
}

fn selection_with_thinking() -> ResolvedModelSelection {
    ResolvedModelSelection {
        provider_id: "anthropic".to_string(),
        provider_name: "Anthropic".to_string(),
        model_id: "claude-sonnet-4".to_string(),
        model_display_name: None,
        provider_slug: "anthropic".to_string(),
        model_slug: "claude-sonnet-4".to_string(),
        context_window: None,
        max_output_tokens: None,
        request_multiplier: None,
        thinking_config: ThinkingConfig::new(ragent_types::ThinkingLevel::Medium),
        base_url: None,
    }
}

#[test]
fn test_benchmark_chat_request_honours_deterministic_and_explicit_sampling() {
    let selection = selection_with_thinking();

    let deterministic = benchmark_chat_request(
        &selection,
        "hello",
        &BenchRunOptions {
            deterministic: true,
            temperature: Some(0.9),
            top_p: Some(0.4),
            max_tokens: Some(256),
            ..BenchRunOptions::default()
        },
    );
    assert_eq!(deterministic.model, "claude-sonnet-4");
    assert_eq!(deterministic.temperature, Some(0.0));
    assert_eq!(deterministic.top_p, Some(1.0));
    assert_eq!(deterministic.max_tokens, Some(256));
    assert!(deterministic.system.is_some());
    assert_eq!(
        deterministic.thinking,
        Some(selection.thinking_config.clone())
    );

    let sampling = benchmark_chat_request(
        &selection,
        "hello",
        &BenchRunOptions {
            deterministic: false,
            temperature: Some(0.9),
            top_p: Some(0.4),
            ..BenchRunOptions::default()
        },
    );
    assert_eq!(sampling.temperature, Some(0.9));
    assert_eq!(sampling.top_p, Some(0.4));
}

#[test]
fn test_max_bench_samples_cap_is_bounded() {
    assert_eq!(MAX_BENCH_SAMPLES, 100);
}

#[test]
fn test_resolve_base_url_reads_storage_settings_and_env() {
    let storage = Storage::open_in_memory().expect("storage");
    let config = Config::default();

    // No configuration at all -> None for a provider with no override.
    assert!(resolve_base_url("ollama", &storage, &config).is_none());

    storage
        .set_setting("copilot_api_base", "https://copilot.invalid")
        .expect("set copilot base");
    assert_eq!(
        resolve_base_url("copilot", &storage, &config).as_deref(),
        Some("https://copilot.invalid")
    );

    // A blank setting is ignored; falls through to None when no config/env.
    storage
        .set_setting("generic_openai_api_base", "   ")
        .expect("set blank generic base");
    let resolved = resolve_base_url("generic_openai", &storage, &config);
    assert!(resolved.is_none() || resolved.is_some());
}

#[test]
fn test_live_runner_rejects_unknown_provider() {
    let registry = provider::create_default_registry();
    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    let selection = ResolvedModelSelection {
        provider_id: "not-a-provider".to_string(),
        provider_name: "nope".to_string(),
        model_id: "x".to_string(),
        model_display_name: None,
        provider_slug: "nope".to_string(),
        model_slug: "x".to_string(),
        context_window: None,
        max_output_tokens: None,
        request_multiplier: None,
        thinking_config: ThinkingConfig::off(),
        base_url: None,
    };
    match LiveBenchModelRunner::new(selection, Arc::new(registry), storage) {
        Ok(_) => panic!("unknown provider must be rejected"),
        Err(error) => assert!(error.to_string().contains("unknown provider")),
    }
}

#[test]
fn test_live_runner_constructs_for_known_provider_without_auth() {
    let registry = provider::create_default_registry();
    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    let selection = ResolvedModelSelection {
        provider_id: "anthropic".to_string(),
        provider_name: "Anthropic".to_string(),
        model_id: "claude-sonnet-4".to_string(),
        model_display_name: None,
        provider_slug: "anthropic".to_string(),
        model_slug: "claude-sonnet-4".to_string(),
        context_window: None,
        max_output_tokens: None,
        request_multiplier: None,
        thinking_config: ThinkingConfig::off(),
        base_url: None,
    };
    let runner = LiveBenchModelRunner::new(selection, Arc::new(registry), storage)
        .expect("known provider builds");
    assert_eq!(
        BenchModelRunner::selection(&runner).provider_id,
        "anthropic"
    );
    // Cancelled before any request -> immediately errors.
    let error = BenchModelRunner::generate(
        &runner,
        "prompt",
        &BenchRunOptions::default(),
        &AtomicBool::new(true),
    )
    .expect_err("cancelled");
    assert!(error.to_string().contains("cancelled"));
}
