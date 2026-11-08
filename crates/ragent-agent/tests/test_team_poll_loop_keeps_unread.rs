//! Regression: the teammate mailbox poll loop must not consume messages.
//!
//! The poll loop only announces messages on the event bus; it does not deliver
//! them to the teammate. It used to call `drain_unread()`, which marked every
//! message read, so the teammate's own `team_read_messages` (which uses
//! `peek_unread()`) saw "No new messages" and lead directives, assignments,
//! plan verdicts and shutdown requests were lost. These tests pin that one
//! poll tick leaves messages unread and announces each message exactly once.

use std::collections::HashSet;

use ragent_agent::event::{Event, EventBus};
use ragent_agent::team::{Mailbox, MailboxMessage, MessageType, poll_mailbox_once};

/// Push a lead -> `tm-001` message into a fresh team dir and return the dir.
fn seed_mailbox() -> (tempfile::TempDir, std::path::PathBuf) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let team_dir = tmp.path().join("team-a");
    std::fs::create_dir_all(&team_dir).expect("team dir");
    let msg = MailboxMessage::new("lead", "tm-001", MessageType::Message, "start task 3")
        .with_sender_session("lead-sess");
    Mailbox::open(&team_dir, "tm-001")
        .expect("open")
        .push(msg)
        .expect("push");
    (tmp, team_dir)
}

#[test]
fn test_team_poll_loop_tick_leaves_message_unread() {
    let (_tmp, team_dir) = seed_mailbox();
    let bus = EventBus::new(16);
    let mut announced = HashSet::new();

    let n = poll_mailbox_once(
        &team_dir,
        "tm-001",
        &mut announced,
        &bus,
        "lead-sess",
        "team-a",
    )
    .expect("poll");
    assert_eq!(n, 1);

    let unread = Mailbox::open(&team_dir, "tm-001")
        .expect("open")
        .peek_unread()
        .expect("peek");
    assert_eq!(unread.len(), 1, "teammate must still see the message");
    assert_eq!(unread[0].content, "start task 3");
}

#[test]
fn test_team_poll_loop_announces_each_message_once() {
    let (_tmp, team_dir) = seed_mailbox();
    let bus = EventBus::new(16);
    let mut rx = bus.subscribe();
    let mut announced = HashSet::new();

    for _ in 0..3 {
        poll_mailbox_once(
            &team_dir,
            "tm-001",
            &mut announced,
            &bus,
            "lead-sess",
            "team-a",
        )
        .expect("poll");
    }

    let mut events = 0;
    while let Ok(ev) = rx.try_recv() {
        if matches!(ev, Event::TeammateMessage { .. }) {
            events += 1;
        }
    }
    assert_eq!(
        events, 1,
        "a still-unread message is announced once, not per tick"
    );
}

#[test]
fn test_team_poll_loop_forgets_acknowledged_ids() {
    let (_tmp, team_dir) = seed_mailbox();
    let bus = EventBus::new(16);
    let mut announced = HashSet::new();
    poll_mailbox_once(
        &team_dir,
        "tm-001",
        &mut announced,
        &bus,
        "lead-sess",
        "team-a",
    )
    .expect("poll");

    let mailbox = Mailbox::open(&team_dir, "tm-001").expect("open");
    let id = mailbox.peek_unread().expect("peek")[0].message_id.clone();
    assert!(mailbox.acknowledge(&id).expect("ack"));

    let n = poll_mailbox_once(
        &team_dir,
        "tm-001",
        &mut announced,
        &bus,
        "lead-sess",
        "team-a",
    )
    .expect("poll");
    assert_eq!(n, 0);
    assert!(announced.is_empty(), "acknowledged ids are pruned");
}
