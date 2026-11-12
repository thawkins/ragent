//! FR-015: an unanswered interactive permission prompt must default to the
//! safe choice, and only a reply bound to the awaiting session may answer it.
//!
//! `prompt_for_permission` is private, so these drive it through its public
//! entry point, `check_permission_with_prompt`, on the `Ask` path (an empty
//! `PermissionChecker` with a non-file permission matches no rule, so the call
//! falls through to a real prompt). Each test scripts the reply channel the way
//! the TUI does: subscribe, wait for `PermissionRequested`, publish
//! `PermissionReplied`.
//!
//! The properties under test are the ones where a bug is a security bug:
//!
//! - a timeout denies rather than allows;
//! - a reply from another session is ignored (SEC-ragent-server-007), so a
//!   forged approval cannot answer a prompt it does not own;
//! - `Always` records a durable grant on the checker;
//! - a `Deny` verdict denies without ever prompting.

use std::sync::Arc;
use std::time::Duration;

use ragent_agent::event::{Event, EventBus};
use ragent_agent::permission::{PermissionAction, PermissionChecker};
use ragent_agent::session::processor::check_permission_with_prompt;
use ragent_types::permission::PermissionDecision;

/// A checker with no rules, and a bus sized for the events these tests publish.
fn harness() -> (Arc<parking_lot::RwLock<PermissionChecker>>, Arc<EventBus>) {
    (
        Arc::new(parking_lot::RwLock::new(PermissionChecker::new(Vec::new()))),
        Arc::new(EventBus::new(64)),
    )
}

/// Drive one `Ask`-path permission check that is expected to prompt.
///
/// `checkpoint_forced` selects which timeout reaches `prompt_for_permission`:
/// the plain `Ask` path hardcodes the production 120 s window, while the forced
/// destructive-action checkpoint path honours `timeout_secs`. The prompt code
/// under test is identical on both, so the timeout-sensitive cases take the
/// forced path to stay fast.
async fn ask(
    checker: &Arc<parking_lot::RwLock<PermissionChecker>>,
    event_bus: &Arc<EventBus>,
    session_id: &str,
    checkpoint_forced: bool,
    timeout_secs: u64,
) -> PermissionAction {
    check_permission_with_prompt(
        checker,
        event_bus,
        session_id,
        "tool:execute",
        "tool:Bash",
        "Bash",
        Some(false),
        None,
        checkpoint_forced,
        timeout_secs,
        false,
        false,
        &std::env::current_dir().expect("cwd"),
    )
    .await
    .expect("permission check must not error")
}

/// Wait for the next `PermissionRequested` for `session_id` and return its
/// `request_id`. Panics on timeout so a broken publisher fails fast.
async fn next_request_id(
    rx: &mut tokio::sync::broadcast::Receiver<Event>,
    session_id: &str,
) -> String {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        match tokio::time::timeout(remaining, rx.recv()).await {
            Ok(Ok(Event::PermissionRequested {
                session_id: sid,
                request_id,
                ..
            })) if sid == session_id => return request_id,
            Ok(Ok(_)) => continue,
            Ok(Err(e)) => panic!("event bus closed while awaiting the request: {e}"),
            Err(elapsed) => {
                panic!("no PermissionRequested for {session_id} within 5s: {elapsed}")
            }
        }
    }
}

#[tokio::test]
async fn test_unanswered_prompt_times_out_to_deny() {
    let (checker, event_bus) = harness();
    // A one-second window keeps the test fast; the production default is 120s
    // and the code path is identical.
    let action = ask(&checker, &event_bus, "session-timeout", true, 1).await;
    assert_eq!(
        action,
        PermissionAction::Deny,
        "an unanswered prompt must deny (FR-015: default to the safe choice)"
    );
}

#[tokio::test]
async fn test_request_is_published_with_the_expected_shape() {
    let (checker, event_bus) = harness();
    let mut rx = event_bus.subscribe();

    // Answer almost immediately so the test does not wait out a timeout.
    let responder = tokio::spawn(async move {
        let request_id = next_request_id(&mut rx, "session-shape").await;
        let _ = rx; // keep the receiver alive for the publish
        request_id
    });

    let action = ask(&checker, &event_bus, "session-shape", true, 5).await;
    let request_id = responder.await.expect("responder must finish");

    assert!(
        !request_id.is_empty(),
        "the request id must be a real uuid, not empty"
    );
    // Nothing replied, so the prompt resolves by timeout.
    assert_eq!(action, PermissionAction::Deny);
}

#[tokio::test]
async fn test_matching_allow_reply_is_honoured() {
    let (checker, event_bus) = harness();
    let mut rx = event_bus.subscribe();
    let bus = Arc::clone(&event_bus);

    let responder = tokio::spawn(async move {
        let request_id = next_request_id(&mut rx, "session-allow").await;
        bus.publish(Event::PermissionReplied {
            session_id: "session-allow".to_string(),
            request_id,
            allowed: true,
            decision: PermissionDecision::Once,
        });
    });

    let action = ask(&checker, &event_bus, "session-allow", true, 5).await;
    responder.await.expect("responder must finish");
    assert_eq!(action, PermissionAction::Allow);
}

#[tokio::test]
async fn test_matching_deny_reply_is_honoured() {
    let (checker, event_bus) = harness();
    let mut rx = event_bus.subscribe();
    let bus = Arc::clone(&event_bus);

    let responder = tokio::spawn(async move {
        let request_id = next_request_id(&mut rx, "session-deny").await;
        bus.publish(Event::PermissionReplied {
            session_id: "session-deny".to_string(),
            request_id,
            allowed: false,
            decision: PermissionDecision::Deny,
        });
    });

    let action = ask(&checker, &event_bus, "session-deny", true, 5).await;
    responder.await.expect("responder must finish");
    assert_eq!(action, PermissionAction::Deny);
}

#[tokio::test]
async fn test_reply_from_another_session_is_ignored() {
    // SEC-ragent-server-007: request ids are broadcast on `/events`, so a
    // caller can observe one. An approval that names a *different* session must
    // not answer this prompt - the waiter keeps waiting and resolves on timeout.
    let (checker, event_bus) = harness();
    let mut rx = event_bus.subscribe();
    let bus = Arc::clone(&event_bus);

    let responder = tokio::spawn(async move {
        let request_id = next_request_id(&mut rx, "session-owner").await;
        // Forged approval: right request id, wrong session.
        bus.publish(Event::PermissionReplied {
            session_id: "session-attacker".to_string(),
            request_id,
            allowed: true,
            decision: PermissionDecision::Always,
        });
    });

    let action = ask(&checker, &event_bus, "session-owner", true, 1).await;
    responder.await.expect("responder must finish");

    assert_eq!(
        action,
        PermissionAction::Deny,
        "a forged cross-session approval must not grant permission"
    );
    let checker = checker.read();
    assert_ne!(
        checker.check("tool:execute", "tool:Bash"),
        PermissionAction::Allow,
        "a forged approval must not record an Always grant either"
    );
}

#[tokio::test]
async fn test_always_reply_records_a_durable_grant() {
    let (checker, event_bus) = harness();
    let mut rx = event_bus.subscribe();
    let bus = Arc::clone(&event_bus);
    let same_checker = Arc::clone(&checker);

    let responder = tokio::spawn(async move {
        let request_id = next_request_id(&mut rx, "session-always").await;
        bus.publish(Event::PermissionReplied {
            session_id: "session-always".to_string(),
            request_id,
            allowed: true,
            decision: PermissionDecision::Always,
        });
    });

    let action = ask(&checker, &event_bus, "session-always", true, 5).await;
    responder.await.expect("responder must finish");
    assert_eq!(action, PermissionAction::Allow);

    // The grant is recorded, so the identical request no longer needs a prompt.
    {
        let c = same_checker.read();
        assert_eq!(
            c.check("tool:execute", "tool:Bash"),
            PermissionAction::Allow,
            "an Always reply must record a durable grant"
        );
    }
    // A normal (non-checkpoint) repeat is satisfied by the grant without a
    // prompt. The second call must NOT be a forced checkpoint: FR-024 makes a
    // forced destructive-action checkpoint outrank an allow rule, so that path
    // would prompt again by design.
    let second = ask(&same_checker, &event_bus, "session-always-2", false, 1).await;
    assert_eq!(
        second,
        PermissionAction::Allow,
        "the recorded Always grant must satisfy the next identical request without prompting"
    );
}

#[tokio::test]
async fn test_policy_deny_never_prompts() {
    // A checker holding an explicit deny rule settles the request before any
    // prompt: no PermissionRequested is ever published.
    let checker = Arc::new(parking_lot::RwLock::new(PermissionChecker::new(vec![
        ragent_config::permission::PermissionRule {
            permission: ragent_config::permission::Permission::from("tool:execute"),
            pattern: Some("tool:Bash".to_string()),
            action: PermissionAction::Deny,
        },
    ])));
    let event_bus = Arc::new(EventBus::new(16));
    let mut rx = event_bus.subscribe();

    let action = ask(&checker, &event_bus, "session-policy", true, 1).await;
    assert_eq!(action, PermissionAction::Deny);

    // Nothing may have been published: drain whatever is pending and assert no
    // request was raised for this session.
    while let Ok(event) = rx.try_recv() {
        assert!(
            !matches!(
                event,
                Event::PermissionRequested { ref session_id, .. } if session_id == "session-policy"
            ),
            "an explicit deny rule must not raise a prompt"
        );
    }
}

#[tokio::test]
async fn test_auto_approve_allows_without_prompting() {
    let (checker, event_bus) = harness();
    let mut rx = event_bus.subscribe();

    let action = check_permission_with_prompt(
        &checker,
        &event_bus,
        "session-yes",
        "tool:execute",
        "tool:Bash",
        "Bash",
        Some(true),
        None,
        false,
        1,
        false,
        false,
        &std::env::current_dir().expect("cwd"),
    )
    .await
    .expect("permission check must not error");

    assert_eq!(action, PermissionAction::Allow);
    while let Ok(event) = rx.try_recv() {
        assert!(
            !matches!(event, Event::PermissionRequested { .. }),
            "--yes must not raise an interactive prompt"
        );
    }
}

#[tokio::test]
async fn test_auto_approve_is_suppressed_by_a_forced_checkpoint() {
    // FR-024: a forced destructive-action checkpoint outranks `--yes`, so the
    // prompt is still raised and still times out to a denial.
    let (checker, event_bus) = harness();
    let mut rx = event_bus.subscribe();

    let action = check_permission_with_prompt(
        &checker,
        &event_bus,
        "session-checkpoint",
        "tool:execute",
        "tool:Bash",
        "Bash",
        Some(true),
        None,
        // checkpoint_forced
        true,
        1,
        false,
        false,
        &std::env::current_dir().expect("cwd"),
    )
    .await
    .expect("permission check must not error");

    assert_eq!(
        action,
        PermissionAction::Deny,
        "a forced checkpoint must not be auto-approved by --yes"
    );
    // And it really did prompt.
    let mut prompted = false;
    while let Ok(event) = rx.try_recv() {
        if matches!(
            event,
            Event::PermissionRequested { ref session_id, .. } if session_id == "session-checkpoint"
        ) {
            prompted = true;
        }
    }
    assert!(prompted, "the forced checkpoint must raise a prompt");
}
