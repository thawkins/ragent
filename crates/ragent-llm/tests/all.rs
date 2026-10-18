//! Consolidated integration-test target: one test binary per crate.
//!
//! Each former top-level `tests/test_*.rs` file is included as a child
//! module via `#[path]`, so it keeps its own file-relative `mod` /
//! `#[path]` resolution and its `test_<file>::<fn>` test name. See
//! `AGENTS.md` ("Test Layout") and
//! `docs/reports/testconsolidation-review.md`.

/// The one process-wide lock that serialises tests which mutate or observe
/// process-global state (`set_current_dir`, `XDG_CONFIG_HOME`, `HOME`,
/// `PATH`).
///
/// Before consolidation each test file was its own binary, so a file-local
/// mutex was enough. One binary per crate merges them, so every mutating test
/// in every child file must contend on the SAME mutex; `crate::` resolves at
/// this binary root, which is what makes that possible. See AGENTS.md
/// ("Test Layout").
pub fn global_env_mutex() -> &'static std::sync::Mutex<()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
}

/// Acquire [`global_env_mutex`], recovering from a poisoned lock (a panicking
/// test must not wedge every later test).
pub fn global_env_lock() -> std::sync::MutexGuard<'static, ()> {
    global_env_mutex()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[path = "test_azure_foundry_provider.rs"]
mod test_azure_foundry_provider;

#[path = "test_azure_resource_anthropic.rs"]
mod test_azure_resource_anthropic;

#[path = "test_azure_resource_parser.rs"]
mod test_azure_resource_parser;

#[path = "test_bedrock_credentials_env.rs"]
mod test_bedrock_credentials_env;

#[path = "test_bedrock_provider.rs"]
mod test_bedrock_provider;

#[path = "test_env_key.rs"]
mod test_env_key;

#[path = "test_gemini_stream_spin.rs"]
mod test_gemini_stream_spin;

#[path = "test_gemini_tool_calls.rs"]
mod test_gemini_tool_calls;

#[path = "test_http_client_helpers.rs"]
mod test_http_client_helpers;

#[path = "test_http_client_stream_chunk.rs"]
mod test_http_client_stream_chunk;

#[path = "test_m5_6_provider_consistency.rs"]
mod test_m5_6_provider_consistency;

#[path = "test_mock_llm_client.rs"]
mod test_mock_llm_client;

#[path = "test_model_discovery_extra.rs"]
mod test_model_discovery_extra;

#[path = "test_ollama_cloud_image_handling.rs"]
mod test_ollama_cloud_image_handling;

#[path = "test_ollama_cloud_real.rs"]
mod test_ollama_cloud_real;

#[path = "test_ollama_shared.rs"]
mod test_ollama_shared;

#[path = "test_ollama_tool_handling.rs"]
mod test_ollama_tool_handling;

#[path = "test_openai_compat_builder.rs"]
mod test_openai_compat_builder;

#[path = "test_openai_provider_extra.rs"]
mod test_openai_provider_extra;

#[path = "test_openrouter_provider.rs"]
mod test_openrouter_provider;

#[path = "test_provider_offline_coverage.rs"]
mod test_provider_offline_coverage;

#[path = "test_rate_limit_retry.rs"]
mod test_rate_limit_retry;

#[path = "test_router_client.rs"]
mod test_router_client;

#[path = "test_router_config.rs"]
mod test_router_config;

#[path = "test_router_provider.rs"]
mod test_router_provider;

#[path = "test_shared_chat_request.rs"]
mod test_shared_chat_request;

#[path = "test_shared_request.rs"]
mod test_shared_request;

#[path = "test_thinking_adapters.rs"]
mod test_thinking_adapters;
