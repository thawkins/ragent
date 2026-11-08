//! The teammate wake path (review P0-2): after a turn, a teammate's agent loop
//! decides whether to run another one. New mail for an idle teammate must
//! start a turn that reads it; a parked teammate must not be woken; a task
//! auto-claimed for the member must be run; a terminal member ends the loop.

use std::collections::HashSet;

use ragent_agent::team::config::MemberStatus;
use ragent_agent::team::{Mailbox, MailboxMessage, MessageType, TeamStore, TeammateNextTurn};

#[path = "manager_support/mod.rs"]
mod manager_support;
use manager_support::team_manager;

/// Set `tm-001`'s status (and optionally its task) directly on disk.
fn set_member(team_dir: &std::path::Path, status: MemberStatus, task: Option<&str>) {
    let mut store = TeamStore::load(team_dir).expect("load");
    let member = store
        .config
        .members
        .iter_mut()
        .find(|m| m.agent_id == "tm-001")
        .expect("member");
    member.status = status;
    member.current_task_id = task.map(str::to_string);
    store.save().expect("save");
}

/// Push a lead -> `tm-001` message.
fn send(team_dir: &std::path::Path, text: &str) {
    Mailbox::open(team_dir, "tm-001")
        .expect("open")
        .push(
            MailboxMessage::new("lead", "tm-001", MessageType::Message, text)
                .with_sender_session("lead-sess"),
        )
        .expect("push");
}

#[tokio::test]
async fn test_team_wake_idle_member_with_mail_runs_once() {
    let (manager, _tmp, _bus) = team_manager("wake-a", "lead-sess", true);
    set_member(&manager.team_dir, MemberStatus::Idle, None);
    let mut prompted = HashSet::new();

    assert_eq!(
        manager.next_teammate_turn("tm-001", &mut prompted),
        TeammateNextTurn::Wait,
        "no mail, nothing to do"
    );

    send(&manager.team_dir, "please review src/lib.rs");
    match manager.next_teammate_turn("tm-001", &mut prompted) {
        TeammateNextTurn::Run(p) => assert!(p.contains("team_read_messages"), "prompt: {p}"),
        other => panic!("expected a turn, got {other:?}"),
    }
    assert_eq!(
        manager.next_teammate_turn("tm-001", &mut prompted),
        TeammateNextTurn::Wait,
        "a message left unread is not re-prompted"
    );

    send(&manager.team_dir, "and src/main.rs");
    assert!(matches!(
        manager.next_teammate_turn("tm-001", &mut prompted),
        TeammateNextTurn::Run(_)
    ));
}

#[tokio::test]
async fn test_team_wake_suspended_member_is_not_woken() {
    let (manager, _tmp, _bus) = team_manager("wake-b", "lead-sess", true);
    set_member(&manager.team_dir, MemberStatus::Suspended, None);
    send(&manager.team_dir, "hello");
    let mut prompted = HashSet::new();
    assert_eq!(
        manager.next_teammate_turn("tm-001", &mut prompted),
        TeammateNextTurn::Wait
    );
    assert!(prompted.is_empty(), "parked mail is prompted after resume");
}

#[tokio::test]
async fn test_team_wake_runs_auto_claimed_task() {
    let (manager, _tmp, _bus) = team_manager("wake-c", "lead-sess", true);
    set_member(&manager.team_dir, MemberStatus::Working, Some("task-007"));
    match manager.next_teammate_turn("tm-001", &mut HashSet::new()) {
        TeammateNextTurn::Run(p) => assert!(p.contains("task-007"), "prompt: {p}"),
        other => panic!("expected a task turn, got {other:?}"),
    }
}

#[tokio::test]
async fn test_team_wake_terminal_member_exits() {
    let (manager, _tmp, _bus) = team_manager("wake-d", "lead-sess", true);
    set_member(&manager.team_dir, MemberStatus::Stopped, None);
    send(&manager.team_dir, "too late");
    assert_eq!(
        manager.next_teammate_turn("tm-001", &mut HashSet::new()),
        TeammateNextTurn::Exit
    );
}

#[tokio::test]
async fn test_team_wake_prompted_ids_are_pruned_after_ack() {
    let (manager, _tmp, _bus) = team_manager("wake-e", "lead-sess", true);
    set_member(&manager.team_dir, MemberStatus::Idle, None);
    send(&manager.team_dir, "one");
    let mut prompted = HashSet::new();
    manager.next_teammate_turn("tm-001", &mut prompted);

    let mailbox = Mailbox::open(&manager.team_dir, "tm-001").expect("open");
    let id = mailbox.peek_unread().expect("peek")[0].message_id.clone();
    mailbox.acknowledge(&id).expect("ack");
    manager.next_teammate_turn("tm-001", &mut prompted);
    assert!(prompted.is_empty(), "acknowledged ids are dropped");
}
