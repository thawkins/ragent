//! Tests for test_orchestrator_policy.rs

//! Tests for policy-based conflict resolution and human-in-the-loop (Task 5.3).

use futures::future::FutureExt;
use std::sync::Arc;

use ragent_core::orchestrator::policy::{
    ConflictPolicy, ConflictResolver, HumanFallback, LoggingFallback,
};
use ragent_core::orchestrator::{AgentRegistry, Coordinator, JobDescriptor, Responder};

// ── ConflictResolver unit tests (supplement those in policy.rs) ──────────────

fn resolve(policy: ConflictPolicy, pairs: &[(&str, &str)]) -> anyhow::Result<String> {
    let r = ConflictResolver::new(policy);
    let responses: Vec<(String, String)> = pairs
        .iter()
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .collect();
    r.resolve("test-job", &responses)
}

#[test]
fn test_policy_concat_all_agents() {
    let res = resolve(ConflictPolicy::Concat, &[("a", "foo"), ("b", "bar")]).unwrap();
    assert!(res.contains("foo"));
    assert!(res.contains("bar"));
    assert!(res.contains("agent: a"));
    assert!(res.contains("agent: b"));
}

#[test]
fn test_policy_first_success_returns_first_non_error() {
    let res = resolve(
        ConflictPolicy::FirstSuccess,
        &[
            ("a", "error: nope"),
            ("b", "great answer"),
            ("c", "also fine"),
        ],
    )
    .unwrap();
    assert!(res.contains("great answer"));
    assert!(!res.contains("also fine"), "should stop at first success");
}

#[test]
fn test_policy_first_success_empty_result_on_all_errors() {
    let res = resolve(
        ConflictPolicy::FirstSuccess,
        &[("a", "error: one"), ("b", "error: two")],
    );
    assert!(res.is_err());
}

#[test]
fn test_policy_last_response_only_last() {
    let res = resolve(
        ConflictPolicy::LastResponse,
        &[("a", "ignored"), ("b", "winner")],
    )
    .unwrap();
    assert!(res.contains("winner"));
    assert!(!res.contains("ignored"));
}

#[test]
fn test_policy_consensus_met() {
    let res = resolve(
        ConflictPolicy::Consensus { threshold: 2 },
        &[("a", "Paris"), ("b", "Paris"), ("c", "London")],
    )
    .unwrap();
    assert!(res.contains("consensus"));
    assert!(res.contains("Paris"));
}

#[test]
fn test_policy_consensus_prefix_limited_to_64_chars() {
    let prefix = "x".repeat(64);
    let a = format!("{}A", prefix);
    let b = format!("{}B", prefix);
    let res = resolve(
        ConflictPolicy::Consensus { threshold: 2 },
        &[("a", &a), ("b", &b), ("c", "other")],
    )
    .unwrap();
    assert!(res.contains("consensus"));
    // Should choose the first agent that matches the consensus prefix.
    assert!(res.contains("A"));
}

#[test]
fn test_policy_consensus_not_met() {
    let res = resolve(
        ConflictPolicy::Consensus { threshold: 3 },
        &[("a", "X"), ("b", "Y"), ("c", "Z")],
    )
    .unwrap();
    assert!(res.contains("[no consensus]"));
    assert!(res.contains("X"));
    assert!(res.contains("Y"));
    assert!(res.contains("Z"));
}

#[test]
fn test_policy_human_review_uses_logging_fallback() {
    let r = ConflictResolver::new(ConflictPolicy::HumanReview);
    let responses = vec![
        ("a".to_string(), "one".to_string()),
        ("b".to_string(), "two".to_string()),
    ];
    let res = r.resolve("j1", &responses).unwrap();
    assert!(res.starts_with("[human-review]"));
    assert!(res.contains("agent: a"));
    assert!(res.contains("agent: b"));
}

#[test]
fn test_policy_human_review_custom_fallback() {
    struct StubFallback;
    impl HumanFallback for StubFallback {
        fn on_conflict(&self, job_id: &str, _responses: &[(String, String)]) -> String {
            format!("stub-handled:{}", job_id)
        }
    }

    let r = ConflictResolver::with_fallback(ConflictPolicy::HumanReview, Arc::new(StubFallback));
    let res = r
        .resolve("my-job", &[("a".to_string(), "resp".to_string())])
        .unwrap();
    assert_eq!(res, "stub-handled:my-job");
}

#[test]
fn test_logging_fallback_format() {
    let fb = LoggingFallback;
    let responses = vec![("ag1".to_string(), "result1".to_string())];
    let out = fb.on_conflict("jid", &responses);
    assert!(out.contains("[human-review]"));
    assert!(out.contains("ag1"));
    assert!(out.contains("result1"));
}

#[test]
fn test_resolver_empty_responses_returns_err() {
    let r = ConflictResolver::new(ConflictPolicy::Concat);
    assert!(r.resolve("j", &[]).is_err());
}

// ── Coordinator integration with ConflictPolicy ──────────────────────────────

#[tokio::test]
async fn test_coordinator_with_policy_concat() {
    let registry = AgentRegistry::new();
    let r_a: Responder = Arc::new(|p: String| async move { format!("A:{}", p) }.boxed());
    let r_b: Responder = Arc::new(|p: String| async move { format!("B:{}", p) }.boxed());
    registry
        .register("a", vec!["cap".to_string()], Some(r_a))
        .await;
    registry
        .register("b", vec!["cap".to_string()], Some(r_b))
        .await;

    let coord =
        Coordinator::new(registry).with_policy(ConflictResolver::new(ConflictPolicy::Concat));

    let desc = JobDescriptor {
        id: "concat-job".to_string(),
        required_capabilities: vec!["cap".to_string()],
        payload: "work".to_string(),
    };
    let result = coord.start_job_sync(desc).await.unwrap();
    assert!(result.contains("A:work"));
    assert!(result.contains("B:work"));
}

#[tokio::test]
async fn test_coordinator_with_policy_first_success() {
    let registry = AgentRegistry::new();
    let r_err: Responder =
        Arc::new(|_: String| async move { "error: unavailable".to_string() }.boxed());
    let r_ok: Responder = Arc::new(|p: String| async move { format!("ok:{}", p) }.boxed());
    registry
        .register("err-agent", vec!["cap".to_string()], Some(r_err))
        .await;
    registry
        .register("ok-agent", vec!["cap".to_string()], Some(r_ok))
        .await;

    let coord =
        Coordinator::new(registry).with_policy(ConflictResolver::new(ConflictPolicy::FirstSuccess));

    let desc = JobDescriptor {
        id: "fs-policy-job".to_string(),
        required_capabilities: vec!["cap".to_string()],
        payload: "query".to_string(),
    };
    let result = coord.start_job_sync(desc).await.unwrap();
    assert!(
        result.contains("ok:query"),
        "expected first-success to skip error agent"
    );
}

#[tokio::test]
async fn test_coordinator_with_policy_consensus_met() {
    let registry = AgentRegistry::new();
    // Two agents that agree; one that disagrees.
    let agree =
        Arc::new(|_: String| async move { "the answer is 42".to_string() }.boxed()) as Responder;
    let agree2 =
        Arc::new(|_: String| async move { "the answer is 42".to_string() }.boxed()) as Responder;
    let disagree =
        Arc::new(|_: String| async move { "something completely different".to_string() }.boxed())
            as Responder;
    registry
        .register("ag1", vec!["cap".to_string()], Some(agree))
        .await;
    registry
        .register("ag2", vec!["cap".to_string()], Some(agree2))
        .await;
    registry
        .register("ag3", vec!["cap".to_string()], Some(disagree))
        .await;

    let coord =
        Coordinator::new(registry).with_policy(ConflictResolver::new(ConflictPolicy::Consensus {
            threshold: 2,
        }));

    let desc = JobDescriptor {
        id: "consensus-job".to_string(),
        required_capabilities: vec!["cap".to_string()],
        payload: "question".to_string(),
    };
    let result = coord.start_job_sync(desc).await.unwrap();
    assert!(result.contains("consensus"));
    assert!(result.contains("the answer is 42"));
}

#[tokio::test]
async fn test_coordinator_with_human_review_policy() {
    let registry = AgentRegistry::new();
    let r: Responder = Arc::new(|_: String| async move { "agent opinion".to_string() }.boxed());
    registry
        .register("review-agent", vec!["cap".to_string()], Some(r))
        .await;

    let coord =
        Coordinator::new(registry).with_policy(ConflictResolver::new(ConflictPolicy::HumanReview));

    let desc = JobDescriptor {
        id: "review-job".to_string(),
        required_capabilities: vec!["cap".to_string()],
        payload: "controversial task".to_string(),
    };
    let result = coord.start_job_sync(desc).await.unwrap();
    assert!(result.contains("[human-review]"));
}

#[tokio::test]
async fn test_coordinator_without_policy_uses_concat_default() {
    let registry = AgentRegistry::new();
    let r: Responder = Arc::new(|p: String| async move { format!("default:{}", p) }.boxed());
    registry
        .register("def-agent", vec!["cap".to_string()], Some(r))
        .await;

    // No .with_policy() call.
    let coord = Coordinator::new(registry);
    let desc = JobDescriptor {
        id: "default-job".to_string(),
        required_capabilities: vec!["cap".to_string()],
        payload: "data".to_string(),
    };
    let result = coord.start_job_sync(desc).await.unwrap();
    assert!(result.contains("default:data"));
}
