//! PERF-022: mailbox JSONL format regression tests.
//!
//! Verifies that:
//! - The new newline-delimited JSON (JSONL) format is written on push.
//! - Legacy single-JSON-array mailbox files are still read correctly.
//! - A legacy file is transparently migrated to JSONL on the first `push`.
//! - Round-trip through read/write preserves message semantics.
//! - `push` is an O(1) append (does not rewrite unrelated lines).
//! - Blank lines in a JSONL file are skipped by the reader.

use ragent_team::team::{Mailbox, MailboxMessage, MessageType, TeamStore};

#[path = "support/mod.rs"]
mod support;
use support::setup_workspace;

fn team_dir_for(dir: &std::path::Path, name: &str) -> std::path::PathBuf {
    dir.join(name)
}

fn mailbox_path(team_dir: &std::path::Path, agent_id: &str) -> std::path::PathBuf {
    team_dir.join("mailbox").join(format!("{agent_id}.json"))
}

fn write_raw_mailbox(team_dir: &std::path::Path, agent_id: &str, body: &str) -> std::path::PathBuf {
    let path = mailbox_path(team_dir, agent_id);
    std::fs::create_dir_all(path.parent().unwrap()).expect("create mailbox dir");
    std::fs::write(&path, body).expect("write raw mailbox");
    path
}

/// Push produces a JSONL file (one message per line, each line a complete
/// JSON object). Each line must start with `{`, not with `[`.
#[test]
fn test_push_writes_jsonl_format() {
    let (_tmp, dir) = setup_workspace();
    TeamStore::create("jsonl-team", "lead-sess", &dir, true).expect("create team");
    let team_dir = team_dir_for(&dir, "jsonl-team");
    let mailbox = Mailbox::open(&team_dir, "tm-001").expect("open mailbox");
    mailbox
        .push(MailboxMessage::new(
            "lead",
            "tm-001",
            MessageType::Message,
            "hello",
        ))
        .expect("push");

    let path = mailbox_path(&team_dir, "tm-001");
    let raw = std::fs::read_to_string(&path).expect("read");
    let first = raw.trim_start().chars().next();
    assert_eq!(
        first,
        Some('{'),
        "first char should be {{ for JSONL, got {first:?}"
    );
    // Each non-empty line must be a complete JSON object.
    let lines: Vec<&str> = raw.lines().filter(|l| !l.trim().is_empty()).collect();
    assert_eq!(lines.len(), 1, "one line expected after first push");
    assert!(
        lines[0].contains("\"content\":\"hello\""),
        "line should contain the message content"
    );
}

/// A legacy single-JSON-array mailbox file is still readable via `read_all`
/// and `peek_unread`.
#[test]
fn test_legacy_array_format_is_readable() {
    let (_tmp, dir) = setup_workspace();
    TeamStore::create("legacy-team", "lead-sess", &dir, true).expect("create team");
    let team_dir = team_dir_for(&dir, "legacy-team");

    // Write a legacy-format file (single JSON array).
    let legacy = r#"[
        {
            "message_id": "legacy-001",
            "from": "lead",
            "to": "tm-001",
            "type": "message",
            "content": "legacy body",
            "sent_at": "2026-01-01T00:00:00Z",
            "read": false
        }
    ]"#;
    let _path = write_raw_mailbox(&team_dir, "tm-001", legacy);

    let mailbox = Mailbox::open(&team_dir, "tm-001").expect("open mailbox");
    let all = mailbox.read_all().expect("read_all");
    assert_eq!(all.len(), 1, "legacy file should yield one message");
    assert_eq!(all[0].message_id, "legacy-001");
    assert_eq!(all[0].content, "legacy body");

    let unread = mailbox.peek_unread().expect("peek_unread");
    assert_eq!(unread.len(), 1, "legacy message should be unread");
}

/// A legacy array-format file is transparently migrated to JSONL on the
/// first `push`, preserving existing messages and appending the new one.
#[test]
fn test_legacy_file_migrates_to_jsonl_on_push() {
    let (_tmp, dir) = setup_workspace();
    TeamStore::create("migrate-team", "lead-sess", &dir, true).expect("create team");
    let team_dir = team_dir_for(&dir, "migrate-team");

    let legacy = r#"[
        {
            "message_id": "old-001",
            "from": "lead",
            "to": "tm-001",
            "type": "message",
            "content": "old body",
            "sent_at": "2026-01-01T00:00:00Z",
            "read": true
        }
    ]"#;
    write_raw_mailbox(&team_dir, "tm-001", legacy);

    let mailbox = Mailbox::open(&team_dir, "tm-001").expect("open mailbox");
    mailbox
        .push(MailboxMessage::new(
            "lead",
            "tm-001",
            MessageType::Message,
            "new body",
        ))
        .expect("push after legacy");

    let path = mailbox_path(&team_dir, "tm-001");
    let raw = std::fs::read_to_string(&path).expect("read migrated file");
    let first = raw.trim_start().chars().next();
    assert_eq!(
        first,
        Some('{'),
        "file should be JSONL after migration push"
    );

    let all = mailbox.read_all().expect("read_all after migrate");
    assert_eq!(all.len(), 2, "both old and new messages should be present");
    assert_eq!(all[0].message_id, "old-001");
    assert_eq!(all[0].content, "old body");
    assert!(all[0].read, "old message read-state preserved");
    assert_eq!(all[1].content, "new body");
    assert!(!all[1].read, "new message should be unread");
}

/// `push` must not rewrite lines that are unrelated to it — append only.
/// We detect this by writing two messages and asserting the byte offset
/// of the first message's content is unchanged after the second push.
#[test]
fn test_push_is_append_only_after_jsonl() {
    let (_tmp, dir) = setup_workspace();
    TeamStore::create("append-team", "lead-sess", &dir, true).expect("create team");
    let team_dir = team_dir_for(&dir, "append-team");
    let mailbox = Mailbox::open(&team_dir, "tm-001").expect("open mailbox");
    mailbox
        .push(MailboxMessage::new(
            "lead",
            "tm-001",
            MessageType::Message,
            "first",
        ))
        .expect("first push");

    let path = mailbox_path(&team_dir, "tm-001");
    let raw_before = std::fs::read_to_string(&path).expect("read");
    mailbox
        .push(MailboxMessage::new(
            "lead",
            "tm-001",
            MessageType::Message,
            "second",
        ))
        .expect("second push");
    let raw_after = std::fs::read_to_string(&path).expect("read again");

    // The body written by the first push must be a prefix of the final file.
    assert!(
        raw_after.starts_with(&raw_before),
        "second push should append, not rewrite: before={raw_before:?} after={raw_after:?}"
    );
}

/// Blank lines in a JSONL file are tolerated by the reader (defensive against
/// a trailing newline or a partially-flushed write).
#[test]
fn test_jsonl_reader_skips_blank_lines() {
    let (_tmp, dir) = setup_workspace();
    TeamStore::create("blank-team", "lead-sess", &dir, true).expect("create team");
    let team_dir = team_dir_for(&dir, "blank-team");

    // Build a JSONL body with a blank line in the middle.
    let m1 = serde_json::to_string(&MailboxMessage::new(
        "lead",
        "tm-001",
        MessageType::Message,
        "one",
    ))
    .unwrap();
    let m2 = serde_json::to_string(&MailboxMessage::new(
        "lead",
        "tm-001",
        MessageType::Message,
        "two",
    ))
    .unwrap();
    let body = format!("{m1}\n\n{m2}\n");
    write_raw_mailbox(&team_dir, "tm-001", &body);

    let mailbox = Mailbox::open(&team_dir, "tm-001").expect("open mailbox");
    let all = mailbox.read_all().expect("read_all");
    assert_eq!(
        all.len(),
        2,
        "blank line should be skipped, yielding two messages"
    );
    assert_eq!(all[0].content, "one");
    assert_eq!(all[1].content, "two");
}

/// Round-trip through drain/mark_all_read works on a JSONL file.
#[test]
fn test_mark_all_read_works_on_jsonl() {
    let (_tmp, dir) = setup_workspace();
    TeamStore::create("markread-team", "lead-sess", &dir, true).expect("create team");
    let team_dir = team_dir_for(&dir, "markread-team");
    let mailbox = Mailbox::open(&team_dir, "tm-001").expect("open mailbox");
    for content in ["a", "b", "c"] {
        mailbox
            .push(MailboxMessage::new(
                "lead",
                "tm-001",
                MessageType::Message,
                content,
            ))
            .expect("push");
    }

    let unread = mailbox.peek_unread().expect("peek");
    assert_eq!(unread.len(), 3);
    let ids: Vec<String> = unread.iter().map(|m| m.message_id.clone()).collect();
    let n = mailbox.mark_all_read(&ids).expect("mark_all_read");
    assert_eq!(n, 3, "all three should transition unread → read");

    let still = mailbox.peek_unread().expect("peek after ack");
    assert!(still.is_empty(), "no unread after batch ack");
}
