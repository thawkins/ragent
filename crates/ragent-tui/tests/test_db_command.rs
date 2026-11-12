//! `/db` slash-command tests: registration, autocomplete, help, pruning.
//!
//! Covers the operator-facing surface of `/db prune [--days N] [--dry-run]`
//! and `/db help`: the `SLASH_COMMANDS` entry, the autocomplete suggestions and
//! parameter hint, `/help` indexing, the help page, argument parsing (both
//! `--days N` and `--days=N`), the `--dry-run` no-delete guarantee, the
//! confirmation gate on a real prune, and the configured default window.

use std::sync::Arc;

use support::make_app;

mod support;

use ragent_types::message::Message;

/// Insert `count` user messages into `session_id`, each older than `age_days`.
///
/// `create_message` honours a caller-supplied `created_at`, so the seeding is a
/// plain insert with the timestamp rewound - no direct SQL and no test-only
/// storage helper needed.
fn seed_old_messages(
    storage: &ragent_storage::Storage,
    session_id: &str,
    count: usize,
    age_days: i64,
) {
    // The messages table has a foreign key onto sessions, so the session row
    // must exist before any message is written.
    storage
        .create_session(session_id, "/tmp/db-prune-test")
        .expect("create_session must succeed");
    let created = chrono::Utc::now() - chrono::Duration::days(age_days);
    for i in 0..count {
        let mut msg = Message::user_text(session_id, format!("old message {i}"));
        msg.id = format!("old-{i}");
        msg.created_at = created;
        msg.updated_at = created;
        storage
            .create_message(&msg)
            .expect("create_message must succeed");
    }
}

#[test]
fn test_db_registered_in_slash_commands() {
    let found = ragent_tui::app::SLASH_COMMANDS
        .iter()
        .find(|cmd| cmd.trigger == "db");
    let def = found.expect("db must be registered in SLASH_COMMANDS");
    assert!(
        def.description.contains("prune") && def.description.contains("help"),
        "description must advertise the prune and help subcommands: {}",
        def.description
    );
    assert!(
        def.description.contains("--dry-run"),
        "description must advertise the --dry-run flag: {}",
        def.description
    );
}

#[test]
fn test_db_suggestions_offer_prune_and_help() {
    let mut app = make_app();
    app.session.input = "/db".to_string();
    app.update_slash_menu();
    let menu = app
        .slash_menu
        .as_ref()
        .expect("/db prefix must open the slash menu");
    let entry = menu
        .matches
        .iter()
        .find(|m| m.trigger == "db")
        .expect("menu must contain the db entry");
    assert_eq!(
        entry.suggestions,
        vec!["prune".to_string(), "help".to_string()],
        "db autocomplete suggestions must be [prune, help]"
    );
    assert_eq!(
        entry.parameter_hint.as_deref(),
        Some("[prune [--days N] [--dry-run]|help]"),
        "db parameter hint must list the prune/flags/help forms"
    );
}

#[tokio::test]
async fn test_help_index_lists_db() {
    let mut app = make_app();
    app.session.session_id = Some("test-session".to_string());

    app.execute_slash_command("/help").await;

    let text = app
        .session
        .messages
        .last()
        .expect("help should create a message")
        .text_content();
    assert!(
        text.contains("/db"),
        "/help should list the /db command, got: {text}"
    );
}

#[tokio::test]
async fn test_db_help_lists_subcommands() {
    let mut app = make_app();
    app.session.session_id = Some("test-session".to_string());

    app.execute_slash_command("/db help").await;

    let text = app
        .session
        .messages
        .last()
        .expect("db help should create a message")
        .text_content();
    assert!(
        text.starts_with("From: /db help"),
        "help must carry the From: prefix, got: {text}"
    );
    assert!(
        text.contains("/db prune"),
        "help must document the prune subcommand, got: {text}"
    );
    assert!(
        text.contains("--dry-run"),
        "help must document --dry-run, got: {text}"
    );
    assert!(
        text.contains("VACUUM"),
        "help must warn about VACUUM, got: {text}"
    );
    assert_eq!(app.status, "db: help");
}

#[tokio::test]
async fn test_bare_db_shows_help() {
    // A bare `/db` must not prune anything - it shows help instead.
    let mut app = make_app();
    app.session.session_id = Some("test-session".to_string());

    app.execute_slash_command("/db").await;

    assert!(
        !app.db_prune_confirm_open,
        "bare /db must not arm the prune dialog"
    );
    let text = app
        .session
        .messages
        .last()
        .expect("bare /db should create a message")
        .text_content();
    assert!(
        text.starts_with("From: /db help"),
        "bare /db must show help, got: {text}"
    );
}

#[tokio::test]
async fn test_db_unknown_subcommand_is_rejected() {
    let mut app = make_app();
    app.session.session_id = Some("test-session".to_string());

    app.execute_slash_command("/db wat").await;

    let text = app
        .session
        .messages
        .last()
        .expect("unknown subcommand should create a message")
        .text_content();
    assert!(
        text.contains("Unknown subcommand"),
        "unknown subcommand must be rejected, got: {text}"
    );
    assert_eq!(app.status, "db: usage");
    assert!(!app.db_prune_confirm_open);
}

#[tokio::test]
async fn test_db_prune_dry_run_deletes_nothing() {
    let mut app = make_app();
    app.session.session_id = Some("test-session".to_string());
    let storage = Arc::clone(&app.storage);
    seed_old_messages(&storage, "test-session", 3, 30);

    let before = storage
        .count_messages_older_than(chrono::Duration::days(14))
        .expect("count must succeed");
    assert_eq!(
        before, 3,
        "three seeded messages must be older than 14 days"
    );

    app.execute_slash_command("/db prune --dry-run --days 14")
        .await;

    let after = storage
        .count_messages_older_than(chrono::Duration::days(14))
        .expect("count must succeed");
    assert_eq!(after, before, "--dry-run must delete nothing");
    let text = app
        .session
        .messages
        .last()
        .expect("dry run should create a message")
        .text_content();
    assert!(
        text.contains("Would delete **3**"),
        "dry run must report the count, got: {text}"
    );
    assert!(
        !app.db_prune_confirm_open,
        "--dry-run must not arm the confirmation dialog"
    );
}

#[tokio::test]
async fn test_db_prune_arms_confirmation_and_deletes_only_on_yes() {
    let mut app = make_app();
    app.session.session_id = Some("test-session".to_string());
    let storage = Arc::clone(&app.storage);
    seed_old_messages(&storage, "test-session", 2, 40);

    app.execute_slash_command("/db prune --days 14").await;

    assert!(
        app.db_prune_confirm_open,
        "a real prune must arm the confirmation dialog"
    );
    assert_eq!(
        app.db_prune_confirm_days,
        Some(14),
        "the dialog must remember the window it will apply"
    );
    assert!(
        !app.db_prune_confirm_is_yes(),
        "the dialog must open on `No` so Enter cannot delete"
    );
    let still_there = storage
        .count_messages_older_than(chrono::Duration::days(14))
        .expect("count must succeed");
    assert_eq!(
        still_there, 2,
        "nothing may be deleted before the user confirms"
    );
}

#[tokio::test]
async fn test_db_prune_accepts_joined_days_form() {
    // `--days=N` (joined) must arm the dialog with the same window as the
    // two-token `--days N` form.
    let mut app = make_app();
    app.session.session_id = Some("test-session".to_string());

    app.execute_slash_command("/db prune --days=21").await;

    assert!(app.db_prune_confirm_open);
    assert_eq!(app.db_prune_confirm_days, Some(21));
}

#[tokio::test]
async fn test_db_prune_defaults_to_configured_window() {
    let mut app = make_app();
    app.session.session_id = Some("test-session".to_string());
    app.db_retention_days = 7;

    app.execute_slash_command("/db prune").await;

    assert!(app.db_prune_confirm_open);
    assert_eq!(
        app.db_prune_confirm_days,
        Some(7),
        "an omitted --days must use the configured retention window"
    );
}

#[tokio::test]
async fn test_db_prune_rejects_zero_days() {
    // A zero-day window would empty the store; it must be refused, not silently
    // accepted or coerced.
    let mut app = make_app();
    app.session.session_id = Some("test-session".to_string());

    app.execute_slash_command("/db prune --days 0").await;

    assert!(
        !app.db_prune_confirm_open,
        "a zero-day window must not arm the dialog"
    );
    assert_eq!(app.status, "db: usage");
}

#[tokio::test]
async fn test_db_prune_rejects_unknown_flag() {
    let mut app = make_app();
    app.session.session_id = Some("test-session".to_string());

    app.execute_slash_command("/db prune --wat").await;

    assert!(!app.db_prune_confirm_open);
    let text = app
        .session
        .messages
        .last()
        .expect("bad flag should create a message")
        .text_content();
    assert!(
        text.contains("Unrecognised argument `--wat`"),
        "the bad flag must be named, got: {text}"
    );
    assert_eq!(app.status, "db: usage");
}
