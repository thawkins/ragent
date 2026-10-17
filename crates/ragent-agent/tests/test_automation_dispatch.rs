//! Tests-only coverage for automation dispatch payloads and endpoint/token
//! resolution (`ragent_agent::automation::dispatch`).
//!
//! Covers the linear/notion provider payloads, the endpoint inert-detection,
//! and `dispatch_all`'s no-target and inert-target branches (no network is
//! reached for an inert target).

use ragent_agent::automation::dispatch::{
    DispatchOutcome, dispatch_all, endpoint_for, payload_for,
};
use ragent_config::DispatchTarget;
use ragent_types::{AutomationRun, AutomationTrigger, RunOutcome};

fn target(kind: &str) -> DispatchTarget {
    DispatchTarget {
        kind: kind.to_string(),
        url: None,
        token_env: None,
        target: None,
    }
}

fn finished_run() -> AutomationRun {
    let mut run = AutomationRun::start(
        "run-1".to_string(),
        "auto-1".to_string(),
        AutomationTrigger::Manual,
        "local".to_string(),
    );
    run.outcome = Some(RunOutcome::Success);
    run.ended_at = Some(run.started_at);
    run
}

#[test]
fn payload_for_linear_is_issue_shaped() {
    let mut t = target("linear");
    t.target = Some("TEAM-42".to_string());
    let payload = payload_for(&t, "auto-1", &finished_run());
    assert!(payload["title"].as_str().unwrap().contains("auto-1"));
    assert_eq!(payload["teamId"], "TEAM-42");
    assert!(payload["description"].as_str().unwrap().contains("run-1"));
}

#[test]
fn payload_for_notion_is_database_shaped() {
    let mut t = target("notion");
    t.target = Some("db-abc".to_string());
    let payload = payload_for(&t, "auto-1", &finished_run());
    assert_eq!(payload["parent"]["database_id"], "db-abc");
    assert_eq!(
        payload["properties"]["Name"]["title"][0]["text"]["content"],
        "Automation `auto-1`"
    );
    assert_eq!(
        payload["properties"]["Outcome"]["rich_text"][0]["text"]["content"],
        "success"
    );
}

#[test]
fn payload_for_running_run_uses_running_outcome() {
    // A run with no terminal outcome reports `running`.
    let run = AutomationRun::start(
        "run-2".to_string(),
        "auto-2".to_string(),
        AutomationTrigger::Webhook,
        "local".to_string(),
    );
    let payload = payload_for(&target("webhook"), "auto-2", &run);
    assert_eq!(payload["outcome"], "running");
    assert_eq!(payload["trigger"], "webhook");
    assert!(payload["ended_at"].is_null());
}

#[test]
fn endpoint_for_returns_trimmed_url_or_none() {
    let mut t = target("webhook");
    assert!(endpoint_for(&t).is_none());
    t.url = Some("   ".to_string());
    assert!(endpoint_for(&t).is_none(), "a blank url is inert");
    t.url = Some("  http://x/y  ".to_string());
    assert_eq!(endpoint_for(&t).as_deref(), Some("http://x/y"));
}

#[test]
fn dispatch_target_kind_lower_normalises() {
    let mut t = target("  SLACK ");
    assert_eq!(t.kind_lower(), "slack");
    t.kind = "GitHub".to_string();
    assert_eq!(t.kind_lower(), "github");
}

#[tokio::test]
async fn dispatch_all_with_no_targets_is_empty() {
    let outcomes = dispatch_all(&[], "auto-1", &finished_run()).await;
    assert_eq!(
        outcomes,
        [] as [ragent_agent::automation::DispatchOutcome; 0]
    );
}

#[tokio::test]
async fn dispatch_all_reports_a_target_with_no_url_as_inert() {
    // A target with no url is inert without any network call.
    let outcomes = dispatch_all(&[target("webhook")], "auto-1", &finished_run()).await;
    assert_eq!(outcomes.len(), 1);
    match &outcomes[0] {
        DispatchOutcome::Inert { kind, reason } => {
            assert_eq!(kind, "webhook");
            assert!(reason.contains("no `url`"), "reason: {reason}");
        }
        other => panic!("expected Inert, got {other:?}"),
    }
    assert!(!outcomes[0].is_delivered());
}
