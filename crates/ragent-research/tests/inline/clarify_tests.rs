//! Inline tests for `clarify.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;

#[test]
fn empty_topic_needs_clarification() {
    assert!(needs_clarification("").is_some());
}

#[test]
fn short_topic_needs_clarification() {
    let q = needs_clarification("rust").unwrap();
    assert!(q.contains("narrow this down"));
}

#[test]
fn broad_phrase_needs_clarification() {
    assert!(needs_clarification("research the inference market").is_some());
    assert!(needs_clarification("tell me about Rust").is_some());
}

#[test]
fn specific_comparison_is_clear() {
    assert!(
        needs_clarification("Compare Fireworks AI and Together.ai for LLM inference").is_none()
    );
}

#[test]
fn year_makes_topic_concrete() {
    assert!(needs_clarification("Rust async runtimes in 2024").is_none());
}
