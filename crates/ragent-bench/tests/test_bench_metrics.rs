//! Integration tests for `ragent-bench` metrics functions.
//!
//! Relocated from the inline `#[cfg(test)]` module in `src/suites/metrics.rs`
//! (T-007 of the testconsolidate spec). The metric functions are public in
//! `metrics.rs`; the re-export in `suites/mod.rs` was widened from `pub(crate)`
//! to `pub` so external tests can reach them via `ragent_bench::suites::`.

use ragent_bench::suites::{codebleu_score, edit_similarity, pass_at_k, resolution_rate};

#[test]
fn test_codebleu_score_prefers_similar_code() {
    let exact = codebleu_score("def add(a, b): return a + b", "def add(a, b): return a + b");
    let near = codebleu_score("def add(a, b): return a - b", "def add(a, b): return a + b");
    let far = codebleu_score("print('hello')", "def add(a, b): return a + b");
    assert!(exact > near);
    assert!(near > far);
}

#[allow(clippy::float_cmp)]
#[test]
fn test_pass_at_k_clamps_k_to_sample_count() {
    assert_eq!(pass_at_k(1, 0, 5), 0.0);
    assert_eq!(pass_at_k(1, 1, 5), 1.0);
}

#[allow(clippy::float_cmp)]
#[test]
fn test_resolution_rate_handles_empty_attempts() {
    assert_eq!(resolution_rate(0, 0), 0.0);
    assert_eq!(resolution_rate(3, 4), 0.75);
}

#[allow(clippy::float_cmp)]
#[test]
fn test_edit_similarity_returns_ratio() {
    let exact = edit_similarity("return helper(value)", "return helper(value)");
    let partial = edit_similarity("return helper()", "return helper(value)");
    assert_eq!(exact, 1.0);
    assert!(partial < 1.0);
    assert!(partial > 0.0);
}
