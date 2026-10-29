//! Regression guard for the mailbox oversize refusal (spec `teamssetup`
//! FR-021 / F-16).
//!
//! F-16 claimed that `MAX_MESSAGE_BYTES` "truncates silently". That half of the
//! finding was wrong on inspection: `Mailbox::push` already refuses an
//! oversized body at the write boundary (`mailbox.rs:402`) and names both the
//! limit and the actual size in the error. FR-021 therefore records existing
//! behaviour, and this file is the regression guard that keeps it.
//!
//! What must not regress:
//!
//! 1. A body of exactly `MAX_MESSAGE_BYTES` is accepted (the bound is `>`, not
//!    `>=`).
//! 2. A body of `MAX_MESSAGE_BYTES + 1` is refused.
//! 3. The refusal text contains the *actual* size, the *limit*, and the
//!    offending `message_id`, so an operator can tell which message to fix and
//!    by how much.
//! 4. Nothing is written for a refused message - the failure is a refusal, not
//!    a silent truncation that leaves a short message behind.

use ragent_agent::team::{MAX_MESSAGE_BYTES, Mailbox, MailboxMessage, MessageType};

fn temp_team_dir(name: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let dir = std::env::temp_dir().join(format!(
        "ragent-msglimit-{name}-{}-{nanos}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("create temp team dir");
    dir
}

/// The exact boundary: `MAX_MESSAGE_BYTES` is the largest acceptable body.
#[test]
fn at_the_limit_is_accepted() {
    let dir = temp_team_dir("at-limit");
    let mailbox = Mailbox::open(&dir, "tm-001").expect("open mailbox");

    let at_limit = "x".repeat(MAX_MESSAGE_BYTES);
    let message = MailboxMessage::new("lead", "tm-001", MessageType::Message, at_limit)
        .with_sender_session("lead-session");
    mailbox
        .push(message)
        .expect("a body of exactly MAX_MESSAGE_BYTES must be accepted");

    let read_back = mailbox.read_all().expect("read mailbox");
    assert_eq!(read_back.len(), 1, "the accepted message must be stored");
    assert_eq!(
        read_back[0].content.len(),
        MAX_MESSAGE_BYTES,
        "the stored body must be complete, not truncated"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// The refusal names the limit, the actual size, and the message id.
#[test]
fn over_the_limit_error_names_limit_and_actual_size() {
    let dir = temp_team_dir("over-limit");
    let mailbox = Mailbox::open(&dir, "tm-001").expect("open mailbox");

    let actual_size = MAX_MESSAGE_BYTES + 1;
    let message = MailboxMessage::new(
        "lead",
        "tm-001",
        MessageType::Message,
        "x".repeat(actual_size),
    )
    .with_sender_session("lead-session");
    let message_id = message.message_id.clone();

    let err = mailbox
        .push(message)
        .expect_err("a body over MAX_MESSAGE_BYTES must be refused");
    let text = err.to_string();

    assert!(
        text.contains(&actual_size.to_string()),
        "FR-021: the error must name the actual size ({actual_size}); got: {text}"
    );
    assert!(
        text.contains(&MAX_MESSAGE_BYTES.to_string()),
        "FR-021: the error must name the limit ({MAX_MESSAGE_BYTES}); got: {text}"
    );
    assert!(
        text.contains(&message_id),
        "the error must name the offending message id ({message_id}); got: {text}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// A refused message is not written and does not disturb the mailbox.
#[test]
fn refused_message_is_not_written_and_does_not_truncate() {
    let dir = temp_team_dir("not-written");
    let mailbox = Mailbox::open(&dir, "tm-001").expect("open mailbox");

    let small = MailboxMessage::new("lead", "tm-001", MessageType::Message, "first")
        .with_sender_session("lead-session");
    mailbox.push(small).expect("small message accepted");

    let oversized = MailboxMessage::new(
        "lead",
        "tm-001",
        MessageType::Message,
        "y".repeat(MAX_MESSAGE_BYTES + 1),
    )
    .with_sender_session("lead-session");
    mailbox
        .push(oversized)
        .expect_err("oversized message refused");

    let read_back = mailbox.read_all().expect("read mailbox");
    assert_eq!(
        read_back.len(),
        1,
        "a refused message must leave the mailbox untouched"
    );
    assert_eq!(read_back[0].content, "first");
    assert!(
        !read_back
            .iter()
            .any(|m| m.content.len() > MAX_MESSAGE_BYTES),
        "no stored message may exceed the limit"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
