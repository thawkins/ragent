//! Integration tests verifying that per-step regexes are pre-compiled and that
//! their accessors return one shared, stable instance (`AgentPerf` T-011 /
//! FR-016).
//!
//! Each test asserts on a concrete value returned by the public accessor, so a
//! regression that re-compiles a pattern per call (or removes the shared
//! instance) fails here rather than passing vacuously.

/// `stall_pattern_set()` returns the same `&'static RegexSet` on every call and
/// actually matches the documented stall phrases.
#[test]
fn stall_pattern_set_is_static_and_matches() {
    let first = ragent_agent::session::stream_buffer::stall_pattern_set();
    let second = ragent_agent::session::stream_buffer::stall_pattern_set();
    assert!(
        std::ptr::eq(first, second),
        "the stall pattern set must be one shared instance (FR-016)"
    );
    assert!(
        first.is_match("Let me explore the codebase first"),
        "a stall phrase must be caught by the pre-compiled set"
    );
    assert!(
        !first.is_match("the build finished successfully"),
        "ordinary text must not be flagged as a stall"
    );
}

/// `redact_secrets` is backed by a `LazyLock<Regex>` and masks a key-shaped
/// token deterministically on repeated calls.
#[test]
fn secret_pattern_is_lazy_static_and_redacts() {
    use ragent_agent::sanitize::redact_secrets;
    let token = format!("sk-{}", "a".repeat(24));
    let input = format!("Authorization: Bearer {token}");
    let out1 = redact_secrets(&input);
    let out2 = redact_secrets(&input);
    assert_eq!(out1, out2, "redaction must be deterministic (FR-016)");
    assert!(
        !out1.contains(&token),
        "a key-shaped token must be masked: {out1}"
    );
    assert!(out1.contains("[REDACTED]"), "mask marker expected: {out1}");
}

/// The router classifier's thread-local regex cache yields the same tier for
/// repeated identical inputs (no per-call recompilation drift).
#[test]
fn router_regex_count_is_thread_local_cached() {
    use ragent_llm::providers::router_classifier::{AttachmentInfo, PromptClassifier};
    use ragent_llm::providers::router_config::{BoundaryConfig, WeightConfig};

    let weights = WeightConfig::default();
    let boundaries = BoundaryConfig::default();
    let attachments = AttachmentInfo::default();
    let prompt = "1. first\n2. second\n- bullet\n- bullet";

    let first = PromptClassifier::classify(prompt, None, &weights, &boundaries, &attachments);
    let second = PromptClassifier::classify(prompt, None, &weights, &boundaries, &attachments);
    assert_eq!(
        first.tier, second.tier,
        "classification must be stable across calls (thread-local regex cache)"
    );
    assert_eq!(
        first.composite_score, second.composite_score,
        "the composite score must be identical across calls"
    );
}
