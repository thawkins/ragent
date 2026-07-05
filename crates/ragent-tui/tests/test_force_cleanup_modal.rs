//! Tests for test_force_cleanup_modal.rs

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ragent_team::team::{MemberStatus, TeamConfig, TeamMember, TeamStore};

use std::sync::Mutex;

#[path = "support/mod.rs"]
mod support;
static CWD_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn test_forcecleanup_modal_confirm_flow() {
    let _cwd_guard = CWD_LOCK.lock().expect("cwd lock");
    let tmp = tempfile::tempdir().expect("tempdir");
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(tmp.path()).unwrap();
    std::fs::create_dir_all(tmp.path().join(".ragent/teams")).unwrap();

    let _store =
        TeamStore::create("force-team", "lead-session", tmp.path(), true).expect("create team");
    // Add a member to the store so the forcecleanup has something to deactivate
    let mut store = TeamStore::load_by_name("force-team", tmp.path()).expect("load store");
    store
        .add_member(TeamMember::new("alice", "tm-001", "general"))
        .expect("add member");

    let mut app = support::make_app();
    app.session_id = Some("s1".to_string());
    app.active_team = Some(TeamConfig::new("force-team", "s1"));
    let mut m = TeamMember::new("alice", "tm-001", "general");
    m.status = MemberStatus::Working;
    app.team_members.push(m);

    app.execute_slash_command("/team forcecleanup");

    assert!(
        app.pending_forcecleanup.is_some(),
        "pending modal should be set"
    );
    let last_msg = app.messages.last().unwrap().text_content();
    assert!(last_msg.contains("Active teammates") || last_msg.contains("Press Enter"));
    assert!(
        app.log_entries
            .iter()
            .any(|e| e.message.contains("forcecleanup confirmation required"))
    );

    // Press Enter to confirm
    app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert!(
        app.pending_forcecleanup.is_none(),
        "pending modal should be cleared after confirm"
    );
    // After confirming, team should be cleaned up (status updated)
    assert!(
        app.status.contains("team force"),
        "expected status to reflect force cleanup: {}",
        app.status
    );

    let _ = std::env::set_current_dir(original_dir);
}

#[test]
fn test_forcecleanup_modal_cancel_flow() {
    let mut app = support::make_app();
    app.session_id = Some("s1".to_string());
    app.active_team = Some(TeamConfig::new("force-team", "s1"));
    let mut m = TeamMember::new("bob", "tm-002", "general");
    m.status = MemberStatus::Working;
    app.team_members.push(m);

    app.execute_slash_command("/team forcecleanup");
    assert!(
        app.pending_forcecleanup.is_some(),
        "pending modal should be set"
    );

    // Press Esc to cancel
    app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

    assert!(
        app.pending_forcecleanup.is_none(),
        "pending modal should be cleared after cancel"
    );
    let last_msg = app.messages.last().unwrap().text_content();
    assert!(last_msg.contains("Force-cleanup cancelled"));
    assert!(
        app.log_entries
            .iter()
            .any(|e| e.message.contains("forcecleanup cancelled"))
    );
}
