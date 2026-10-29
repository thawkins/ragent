//! The Teams panel's message counters are recounted from the team's mailbox
//! files, not only from live delivery events.
//!
//! A mailbox is drained by whichever reader happens to see it first (the
//! teammate's own `team_read_messages`, the `TeamManager` poll loop, a
//! server-side reader), so an event-only counter left every row at zero in a
//! running team even though the messages had plainly been sent. Each test
//! below writes real messages through `Mailbox::push` - the same call
//! `team_message` makes - and asserts the panel's counters see them.

use ragent_agent::team::{Mailbox, MailboxMessage, MessageType, TeamStore};
use ragent_tui::app::App;

#[path = "support/mod.rs"]
mod support;

/// Write one message into `recipient`'s mailbox inside `team_dir`.
fn send(team_dir: &std::path::Path, from: &str, to: &str, body: &str) {
    let mailbox = Mailbox::open(team_dir, to).expect("open recipient mailbox");
    mailbox
        .push(MailboxMessage::new(
            from,
            to,
            MessageType::Message,
            body.to_string(),
        ))
        .expect("push message");
}

/// Build an app whose active team is the store at `team_dir`, with the real
/// process cwd unchanged (`TeamStore::load_by_name` resolves the team by
/// name, so the store has to live where the refresh looks for it).
fn app_for_team(store: &TeamStore, lead_agent_name: &str) -> App {
    let mut app = support::make_app();
    app.session_id = Some("sess-1".to_string());
    app.active_team = Some(store.config.clone());
    app.agent_name = lead_agent_name.to_string();
    app
}

/// A globally scoped team whose name is unique to this run.
///
/// `TeamStore::create` with `project_local = false` writes under
/// `~/.config/ragent/teams/`, so a fixed name would collide with the previous
/// run of the same test. The suffix makes a re-run clean, and the directory is
/// removed when the guard drops so the user's global team list is left as it
/// was found.
struct TempTeam {
    store: TeamStore,
    dir: std::path::PathBuf,
}

impl Drop for TempTeam {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn temp_team(prefix: &str) -> TempTeam {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("unix time")
        .as_nanos();
    let tmp = tempfile::tempdir().expect("tempdir");
    let store = TeamStore::create(&format!("{prefix}-{nanos}"), "sess-1", tmp.path(), false)
        .expect("create team");
    TempTeam {
        dir: store.dir.clone(),
        store,
    }
}

#[test]
fn mailbox_traffic_is_counted_for_sender_and_recipient() {
    let _guard = crate::global_env_lock();
    let team = temp_team("count-team");
    send(&team.store.dir, "tm-001", "lead", "found it");
    send(&team.store.dir, "tm-001", "lead", "and again");
    send(&team.store.dir, "tm-002", "tm-001", "p2p note");

    let mut app = app_for_team(&team.store, "lead");
    app.refresh_team_member_session_ids();

    // tm-001 sent two messages to the lead and received one from tm-002.
    assert_eq!(
        app.team_message_counts.get("tm-001").copied(),
        Some((2, 1)),
        "tm-001 must show its two sends and one receive"
    );
    // tm-002 sent one message.
    assert_eq!(app.team_message_counts.get("tm-002").copied(), Some((1, 0)));
    // The lead received both of tm-001's messages and sent none.
    assert_eq!(app.team_message_counts.get("lead").copied(), Some((0, 2)));
}

#[test]
fn control_messages_do_not_move_the_counters() {
    let _guard = crate::global_env_lock();
    let team = temp_team("control-team");
    let mailbox = Mailbox::open(&team.store.dir, "lead").expect("open lead mailbox");
    mailbox
        .push(MailboxMessage::new(
            "tm-001",
            "lead",
            MessageType::IdleNotify,
            "teammate idle",
        ))
        .expect("push idle notify");
    mailbox
        .push(MailboxMessage::new(
            "tm-001",
            "lead",
            MessageType::ShutdownAck,
            "ack",
        ))
        .expect("push shutdown ack");

    let mut app = app_for_team(&team.store, "lead");
    app.refresh_team_member_session_ids();

    assert!(
        app.team_message_counts.is_empty(),
        "control traffic is not a message the panel counts: {:?}",
        app.team_message_counts
    );
}

#[test]
fn empty_mailboxes_leave_the_counters_at_zero() {
    let _guard = crate::global_env_lock();
    let team = temp_team("quiet-team");

    let mut app = app_for_team(&team.store, "lead");
    app.refresh_team_member_session_ids();

    assert!(
        app.team_message_counts.is_empty(),
        "no traffic means no counter rows: {:?}",
        app.team_message_counts
    );
}

#[test]
fn recount_never_lowers_a_counter_an_event_already_raised() {
    // The event handlers accumulate as deliveries are observed; the recount
    // must be a floor, so a live event that has already landed is not wiped
    // back to the file's total on the next frame.
    let _guard = crate::global_env_lock();
    let team = temp_team("floor-team");
    send(&team.store.dir, "tm-001", "lead", "one");

    let mut app = app_for_team(&team.store, "lead");
    app.team_message_counts.insert("tm-001".to_string(), (5, 0));
    app.refresh_team_member_session_ids();

    assert_eq!(
        app.team_message_counts.get("tm-001").copied(),
        Some((5, 0)),
        "an already-raised counter must not be lowered by the recount"
    );
}
