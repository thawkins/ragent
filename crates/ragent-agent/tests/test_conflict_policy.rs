#![allow(clippy::assert_is_empty)]
//! External tests for `tests` from `crates/ragent-agent/src/orchestrator/policy.rs`
//!
//! Relocated from the inline `#[cfg(test)]` module.

use ragent_agent::orchestrator::policy::*;

fn responses(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .collect()
}

#[test]
fn test_concat_joins_all() {
    let r = ConflictResolver::new(ConflictPolicy::Concat);
    let res = r
        .resolve("j", &responses(&[("a", "hello"), ("b", "world")]))
        .unwrap();
    assert!(res.contains("hello"));
    assert!(res.contains("world"));
}

#[test]
fn test_first_success_skips_errors() {
    let r = ConflictResolver::new(ConflictPolicy::FirstSuccess);
    let res = r
        .resolve("j", &responses(&[("a", "error: bad"), ("b", "ok result")]))
        .unwrap();
    assert!(res.contains("ok result"));
    assert!(!res.contains("error:"));
}

#[test]
fn test_first_success_all_errors_returns_err() {
    let r = ConflictResolver::new(ConflictPolicy::FirstSuccess);
    let res = r.resolve("j", &responses(&[("a", "error: one"), ("b", "error: two")]));
    assert!(res.is_err());
}

#[test]
fn test_last_response_returns_last() {
    let r = ConflictResolver::new(ConflictPolicy::LastResponse);
    let res = r
        .resolve(
            "j",
            &responses(&[("a", "first"), ("b", "second"), ("c", "third")]),
        )
        .unwrap();
    assert!(res.contains("third"));
    assert!(!res.contains("first"));
}

#[test]
fn test_consensus_met() {
    let r = ConflictResolver::new(ConflictPolicy::Consensus { threshold: 2 });
    // a and b agree; c disagrees.
    let res = r
        .resolve(
            "j",
            &responses(&[
                ("a", "the answer is 42"),
                ("b", "the answer is 42"),
                ("c", "different"),
            ]),
        )
        .unwrap();
    assert!(res.contains("consensus"));
    assert!(res.contains("the answer is 42"));
}

#[test]
fn test_consensus_not_met_returns_all_tagged() {
    let r = ConflictResolver::new(ConflictPolicy::Consensus { threshold: 3 });
    let res = r
        .resolve("j", &responses(&[("a", "aaa"), ("b", "bbb"), ("c", "ccc")]))
        .unwrap();
    assert!(res.contains("[no consensus]"));
}

#[test]
fn test_human_review_uses_fallback() {
    let r = ConflictResolver::new(ConflictPolicy::HumanReview);
    let res = r
        .resolve("j", &responses(&[("a", "one"), ("b", "two")]))
        .unwrap();
    assert!(res.contains("[human-review]"));
}

#[test]
fn test_empty_responses_returns_err() {
    let r = ConflictResolver::new(ConflictPolicy::Concat);
    assert!(r.resolve("j", &[]).is_err());
}
