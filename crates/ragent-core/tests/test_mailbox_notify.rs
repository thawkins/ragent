//! Tests for push-based mailbox delivery (Milestone T6).
//!
//! Validates that the `MailboxNotifierRegistry` wakes poll loops
//! instantly on push, and that the 5s fallback catches writes from
//! unregistered sources.

use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::Notify;

use ragent_core::team::mailbox::{
    Mailbox, MailboxMessage, MessageType, deregister_notifier, register_notifier,
};

/// Helper: create a temp team dir and return its path.
fn tmp_team_dir(name: &str) -> tempfile::TempDir {
    let dir = tempfile::Builder::new()
        .prefix(&format!("ragent-t6-{name}-"))
        .tempdir()
        .expect("create temp dir");
    dir
}

// ── T6 basic: register / signal / deregister ────────────────────────────────

#[tokio::test]
async fn test_notify_wakes_on_push() {
    let dir = tmp_team_dir("wake");
    let team_dir = dir.path();
    let agent_id = "tm-001";

    let notify = Arc::new(Notify::new());
    register_notifier(team_dir, agent_id, Arc::clone(&notify));

    // Spawn a waiter that blocks on notify.
    let n2 = Arc::clone(&notify);
    let start = Instant::now();
    let waiter = tokio::spawn(async move {
        n2.notified().await;
        start.elapsed()
    });

    // Small delay to ensure waiter is parked.
    tokio::time::sleep(Duration::from_millis(10)).await;

    // Push a message — should signal the notify.
    let mailbox = Mailbox::open(team_dir, agent_id).unwrap();
    mailbox
        .push(MailboxMessage::new(
            "lead",
            agent_id,
            MessageType::Message,
            "hello",
        ))
        .unwrap();

    let elapsed = waiter.await.unwrap();
    // Should have woken nearly instantly (well under 1s).
    assert!(
        elapsed < Duration::from_millis(500),
        "notify wakeup took {elapsed:?}, expected < 500ms"
    );

    deregister_notifier(team_dir, agent_id);
}

#[tokio::test]
async fn test_deregister_prevents_signal() {
    let dir = tmp_team_dir("dereg");
    let team_dir = dir.path();
    let agent_id = "tm-002";

    let notify = Arc::new(Notify::new());
    register_notifier(team_dir, agent_id, Arc::clone(&notify));
    deregister_notifier(team_dir, agent_id);

    // Push after deregister — notify should NOT fire.
    let mailbox = Mailbox::open(team_dir, agent_id).unwrap();
    mailbox
        .push(MailboxMessage::new(
            "lead",
            agent_id,
            MessageType::Message,
            "hello",
        ))
        .unwrap();

    // If we wait briefly the notify should NOT have been signalled.
    let result = tokio::time::timeout(Duration::from_millis(100), notify.notified()).await;
    assert!(
        result.is_err(),
        "notify should have timed out after deregister"
    );
}

// ── T6 latency benchmark ────────────────────────────────────────────────────

/// Simulates push-based delivery for N agents, each receiving M messages.
/// Measures per-message delivery latency.
#[tokio::test]
async fn test_notify_latency_benchmark() {
    const NUM_AGENTS: usize = 8;
    const MSGS_PER_AGENT: usize = 50;

    let dir = tmp_team_dir("bench");
    let team_dir = dir.path();

    // Set up agents with notify handles.
    let mut notifies = Vec::new();
    for i in 0..NUM_AGENTS {
        let agent_id = format!("tm-{i:03}");
        let notify = Arc::new(Notify::new());
        register_notifier(team_dir, &agent_id, Arc::clone(&notify));
        notifies.push((agent_id, notify));
    }

    // Spawn receivers that wait for MSGS_PER_AGENT notifications each.
    let mut receivers = Vec::new();
    for (agent_id, notify) in &notifies {
        let n = Arc::clone(notify);
        let aid = agent_id.clone();
        let td = team_dir.to_path_buf();
        receivers.push(tokio::spawn(async move {
            let mut latencies = Vec::with_capacity(MSGS_PER_AGENT);
            for _ in 0..MSGS_PER_AGENT {
                let start = Instant::now();
                n.notified().await;
                latencies.push(start.elapsed());
                // Drain the mailbox like the real poll loop does.
                let mb = Mailbox::open(&td, &aid).unwrap();
                let _ = mb.drain_unread();
            }
            latencies
        }));
    }

    // Give receivers time to park.
    tokio::time::sleep(Duration::from_millis(20)).await;

    // Push messages from "lead" to each agent.
    for msg_idx in 0..MSGS_PER_AGENT {
        for (agent_id, _) in &notifies {
            let mb = Mailbox::open(team_dir, agent_id).unwrap();
            mb.push(MailboxMessage::new(
                "lead",
                agent_id.as_str(),
                MessageType::Message,
                &format!("msg-{msg_idx}"),
            ))
            .unwrap();
        }
        // Small yield to let receivers process.
        tokio::task::yield_now().await;
    }

    // Collect results.
    let mut all_latencies = Vec::new();
    for handle in receivers {
        let lats = handle.await.unwrap();
        all_latencies.extend(lats);
    }

    let total = all_latencies.len();
    let avg = all_latencies.iter().sum::<Duration>() / total as u32;
    let max = all_latencies.iter().max().unwrap();
    let p99_idx = (total as f64 * 0.99) as usize;
    let mut sorted = all_latencies.clone();
    sorted.sort();
    let p99 = sorted[p99_idx.min(total - 1)];

    eprintln!("T6 Benchmark: {NUM_AGENTS} agents × {MSGS_PER_AGENT} msgs = {total} deliveries");
    eprintln!("  avg latency: {avg:?}");
    eprintln!("  p99 latency: {p99:?}");
    eprintln!("  max latency: {max:?}");

    // Notify-based delivery should be sub-100ms average (relaxed for CI runners).
    // On local machines this is typically < 5ms, but CI environments are slower.
    assert!(
        avg < Duration::from_millis(100),
        "average latency {avg:?} exceeds 100ms target"
    );

    // Cleanup.
    for (agent_id, _) in &notifies {
        deregister_notifier(team_dir, agent_id);
    }
}

// ── T6 fallback: no notifier registered ─────────────────────────────────────

/// When no notifier is registered, push still succeeds (no panic/error).
/// The fallback 5s poll would eventually pick it up.
#[tokio::test]
async fn test_push_without_notifier_succeeds() {
    let dir = tmp_team_dir("noop");
    let team_dir = dir.path();
    let agent_id = "tm-099";

    // No register_notifier call — push should still work.
    let mailbox = Mailbox::open(team_dir, agent_id).unwrap();
    mailbox
        .push(MailboxMessage::new(
            "lead",
            agent_id,
            MessageType::Message,
            "hello",
        ))
        .unwrap();

    let msgs = mailbox.read_all().unwrap();
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0].content, "hello");
}

/// Simulates select! pattern used by real poll loop: notify wakes before
/// the 5s fallback timer.
#[tokio::test]
async fn test_select_notify_beats_fallback() {
    let dir = tmp_team_dir("select");
    let team_dir = dir.path();
    let agent_id = "tm-010";

    let notify = Arc::new(Notify::new());
    register_notifier(team_dir, agent_id, Arc::clone(&notify));

    let n2 = Arc::clone(&notify);
    let start = Instant::now();
    let waiter = tokio::spawn(async move {
        tokio::select! {
            _ = n2.notified() => {}
            _ = tokio::time::sleep(Duration::from_secs(5)) => {}
        }
        start.elapsed()
    });

    tokio::time::sleep(Duration::from_millis(10)).await;

    // Push triggers the notify.
    let mb = Mailbox::open(team_dir, agent_id).unwrap();
    mb.push(MailboxMessage::new(
        "lead",
        agent_id,
        MessageType::Message,
        "fast",
    ))
    .unwrap();

    let elapsed = waiter.await.unwrap();
    assert!(
        elapsed < Duration::from_millis(500),
        "select! wakeup took {elapsed:?}, should be < 500ms (not 5s fallback)"
    );

    deregister_notifier(team_dir, agent_id);
}
