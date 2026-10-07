//! Helpers shared by the `ollama` and `ollama_cloud` providers (T-307).
//!
//! Both providers derive a model's context window from its parameter size when
//! the server does not return explicit metadata. That heuristic previously
//! lived as a byte-identical copy in each provider; it now has a single
//! definition here.

/// Estimate a model's context window from its parameter-size string.
///
/// Ollama reports sizes like `"7B"` or `"0.5B"`. Early builds assumed a small
/// window for anything under 70B, which starved capable mid-size models and
/// caused the context panel to display nonsensical ">100% full" percentages.
/// This defaults to 128k for any model with at least 1B parameters and falls
/// back to 32k only for sub-1B models. An explicit `context_length`/`num_ctx`
/// returned by the server takes precedence over this heuristic at the call
/// site.
#[must_use]
pub fn estimate_context_window(parameter_size: &str) -> usize {
    let size = parameter_size
        .trim_end_matches('B')
        .trim_end_matches('b')
        .parse::<f64>()
        .unwrap_or_else(|_| {
            tracing::warn!(
                parameter_size,
                "failed to parse Ollama parameter size; defaulting context window to 128k"
            );
            7.0
        });

    if size >= 1.0 { 131_072 } else { 32_768 }
}
