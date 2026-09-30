//! Inline tests for `loop_state.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;

#[test]
fn test_parse_tags_empty_output() {
    let parsed = parse_tags("");
    assert!(parsed.loop_state.is_empty(), "loop_state should be empty");
    assert!(
        parsed.inbox_entries.is_empty(),
        "inbox_entries should be empty"
    );
}

#[test]
fn test_parse_tags_no_tags() {
    let parsed = parse_tags("Just some text without any tags.");
    assert!(parsed.loop_state.is_empty(), "loop_state should be empty");
    assert!(
        parsed.inbox_entries.is_empty(),
        "inbox_entries should be empty"
    );
}

#[test]
fn test_parse_tags_loop_state() {
    let output = "Some work done.\n<loop-state>\nRemember to check X.\n</loop-state>\nDone.";
    let parsed = parse_tags(output);
    assert_eq!(parsed.loop_state, "Remember to check X.");
    assert!(
        parsed.inbox_entries.is_empty(),
        "inbox_entries should be empty"
    );
}

#[test]
fn test_parse_tags_multiple_loop_state_last_wins() {
    let output = "<loop-state>first</loop-state> middle <loop-state>second</loop-state>";
    let parsed = parse_tags(output);
    assert_eq!(parsed.loop_state, "second");
}

#[test]
fn test_parse_tags_inbox_entries() {
    let output = "<inbox>finding 1</inbox>\n<inbox>finding 2</inbox>";
    let parsed = parse_tags(output);
    assert_eq!(parsed.inbox_entries.len(), 2);
    assert_eq!(parsed.inbox_entries[0], "finding 1");
    assert_eq!(parsed.inbox_entries[1], "finding 2");
}

#[test]
fn test_parse_tags_inbox_max_per_run() {
    let mut output = String::new();
    for i in 0..(INBOX_MAX_PER_RUN + 5) {
        output.push_str(&format!("<inbox>finding {i}</inbox>\n"));
    }
    let parsed = parse_tags(&output);
    assert_eq!(parsed.inbox_entries.len(), INBOX_MAX_PER_RUN);
    assert_eq!(parsed.inbox_entries[0], "finding 0");
    assert_eq!(
        parsed.inbox_entries[INBOX_MAX_PER_RUN - 1],
        format!("finding {}", INBOX_MAX_PER_RUN - 1)
    );
}

#[test]
fn test_parse_tags_loop_state_truncated() {
    let long_content = "x".repeat(LOOP_STATE_MAX_CHARS + 500);
    let output = format!("<loop-state>{long_content}</loop-state>");
    let parsed = parse_tags(&output);
    assert!(parsed.loop_state.chars().count() <= LOOP_STATE_MAX_CHARS);
    assert!(parsed.loop_state.ends_with("..."));
}

#[test]
fn test_parse_tags_inbox_entry_truncated() {
    let long_content = "y".repeat(INBOX_ENTRY_MAX_CHARS + 200);
    let output = format!("<inbox>{long_content}</inbox>");
    let parsed = parse_tags(&output);
    assert_eq!(parsed.inbox_entries.len(), 1);
    assert!(parsed.inbox_entries[0].chars().count() <= INBOX_ENTRY_MAX_CHARS);
    assert!(parsed.inbox_entries[0].ends_with("..."));
}

#[test]
fn test_parse_tags_malformed_no_close() {
    let parsed = parse_tags("<loop-state>no close tag");
    assert!(parsed.loop_state.is_empty(), "loop_state should be empty");
}

#[test]
fn test_parse_tags_mixed() {
    let output = "Work done.\n<loop-state>notes for next run</loop-state>\nMore text.\n<inbox>issue found</inbox>\n<inbox>another issue</inbox>";
    let parsed = parse_tags(output);
    assert_eq!(parsed.loop_state, "notes for next run");
    assert_eq!(parsed.inbox_entries.len(), 2);
    assert_eq!(parsed.inbox_entries[0], "issue found");
    assert_eq!(parsed.inbox_entries[1], "another issue");
}

#[test]
fn test_inject_state_empty() {
    let state = LoopState::default();
    let prompt = "do something";
    let result = inject_state_into_prompt(prompt, &state);
    assert_eq!(result, prompt);
}

#[test]
fn test_inject_state_with_content() {
    let state = LoopState {
        content: "previous notes".to_string(),
    };
    let prompt = "do something";
    let result = inject_state_into_prompt(prompt, &state);
    assert!(result.starts_with("<loop-state>"));
    assert!(result.contains("previous notes"));
    assert!(result.contains("do something"));
}

#[test]
fn test_strip_tags() {
    let output = "Work done.\n<loop-state>notes</loop-state>\nMore text.\n<inbox>finding</inbox>";
    let stripped = strip_tags(output);
    assert!(!stripped.contains("<loop-state>"));
    assert!(!stripped.contains("</loop-state>"));
    assert!(!stripped.contains("<inbox>"));
    assert!(!stripped.contains("</inbox>"));
    assert!(stripped.contains("Work done."));
    assert!(stripped.contains("More text."));
}

#[test]
fn test_strip_tags_no_tags() {
    let output = "Just plain text.";
    assert_eq!(strip_tags(output), "Just plain text.");
}

#[test]
fn test_loop_state_save_load() {
    let dir = std::env::temp_dir().join(format!(
        "ragent-loop-state-test-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&dir).unwrap();

    let state = LoopState {
        content: "test notes".to_string(),
    };
    state.save(&dir, "event-1").unwrap();

    let loaded = LoopState::load(&dir, "event-1").unwrap();
    assert_eq!(loaded.content, "test notes");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_loop_state_load_missing() {
    let dir = std::env::temp_dir().join(format!(
        "ragent-loop-state-test-missing-{}",
        uuid::Uuid::new_v4()
    ));
    let loaded = LoopState::load(&dir, "nonexistent").unwrap();
    assert!(loaded.content.is_empty(), "loaded content should be empty");
}

#[test]
fn test_loop_state_truncated_on_load() {
    let dir = std::env::temp_dir().join(format!(
        "ragent-loop-state-test-trunc-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&dir).unwrap();

    let long_content = "x".repeat(LOOP_STATE_MAX_CHARS + 500);
    let path = dir.join("loop-state").join("event-trunc.txt");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, &long_content).unwrap();

    let loaded = LoopState::load(&dir, "event-trunc").unwrap();
    assert!(loaded.content.chars().count() <= LOOP_STATE_MAX_CHARS);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_inbox_write_read() {
    let dir = std::env::temp_dir().join(format!(
        "ragent-inbox-test-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&dir).unwrap();

    let entries = vec![
        InboxEntry::new("event-1", "first finding"),
        InboxEntry::new("event-1", "second finding"),
    ];
    write_inbox_entries(&dir, &entries).unwrap();

    let read = read_inbox(&dir).unwrap();
    assert_eq!(read.len(), 2);
    assert_eq!(read[0].content, "first finding");
    assert_eq!(read[1].content, "second finding");
    assert_eq!(read[0].source_event_id, "event-1");
    assert_eq!(read[0].status, "open");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_inbox_read_missing() {
    let dir = std::env::temp_dir().join(format!(
        "ragent-inbox-test-missing-{}",
        uuid::Uuid::new_v4()
    ));
    let read = read_inbox(&dir).unwrap();
    assert!(read.is_empty(), "read should be empty");
}

#[test]
fn test_inbox_entry_truncation() {
    let long_content = "z".repeat(INBOX_ENTRY_MAX_CHARS + 100);
    let entry = InboxEntry::new("event-1", &long_content);
    assert!(entry.content.chars().count() <= INBOX_ENTRY_MAX_CHARS);
    assert!(entry.content.ends_with("..."));
}

#[test]
fn test_write_inbox_empty_no_file() {
    let dir =
        std::env::temp_dir().join(format!("ragent-inbox-test-empty-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    write_inbox_entries(&dir, &[]).unwrap();
    assert!(!dir.join("inbox.jsonl").exists());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_truncate_chars_no_truncation() {
    assert_eq!(truncate_chars("hello", 10), "hello");
}

#[test]
fn test_truncate_chars_exact() {
    assert_eq!(truncate_chars("hello", 5), "hello");
}

#[test]
fn test_truncate_chars_truncated() {
    let result = truncate_chars("hello world", 5);
    assert_eq!(result.chars().count(), 5);
    assert!(result.ends_with("..."));
}

#[test]
fn test_update_inbox_entry_status_found() {
    let dir = std::env::temp_dir().join(format!(
        "ragent-inbox-update-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&dir).unwrap();

    let entry = InboxEntry::new("event-1", "test finding");
    let entry_id = entry.id.clone();
    write_inbox_entries(&dir, &[entry]).unwrap();

    let found = update_inbox_entry_status(&dir, &entry_id, "claimed").unwrap();
    assert!(found);

    let read = read_inbox(&dir).unwrap();
    assert_eq!(read.len(), 1);
    assert_eq!(read[0].status, "claimed");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_update_inbox_entry_status_not_found() {
    let dir = std::env::temp_dir().join(format!(
        "ragent-inbox-update-nf-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&dir).unwrap();

    let entry = InboxEntry::new("event-1", "test finding");
    write_inbox_entries(&dir, &[entry]).unwrap();

    let found = update_inbox_entry_status(&dir, "nonexistent-id", "dismissed").unwrap();
    assert!(!found);

    // File should be unchanged
    let read = read_inbox(&dir).unwrap();
    assert_eq!(read.len(), 1);
    assert_eq!(read[0].status, "open");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_update_inbox_entry_status_no_file() {
    let dir = std::env::temp_dir().join(format!(
        "ragent-inbox-update-nofile-{}",
        uuid::Uuid::new_v4()
    ));
    let found = update_inbox_entry_status(&dir, "any-id", "claimed").unwrap();
    assert!(!found);
}

#[test]
fn test_clear_inbox_with_entries() {
    let dir = std::env::temp_dir().join(format!(
        "ragent-inbox-clear-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&dir).unwrap();

    let entries = vec![
        InboxEntry::new("event-1", "first"),
        InboxEntry::new("event-2", "second"),
        InboxEntry::new("event-1", "third"),
    ];
    write_inbox_entries(&dir, &entries).unwrap();

    let count = clear_inbox(&dir).unwrap();
    assert_eq!(count, 3);
    assert!(!dir.join("inbox.jsonl").exists());

    // Clearing again should return 0
    let count2 = clear_inbox(&dir).unwrap();
    assert_eq!(count2, 0);
}

#[test]
fn test_clear_inbox_empty() {
    let dir =
        std::env::temp_dir().join(format!("ragent-inbox-clear-empty-{}", uuid::Uuid::new_v4()));
    let count = clear_inbox(&dir).unwrap();
    assert_eq!(count, 0);
}

#[test]
fn test_update_inbox_preserves_other_entries() {
    let dir = std::env::temp_dir().join(format!(
        "ragent-inbox-preserve-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&dir).unwrap();

    let entry1 = InboxEntry::new("event-1", "first");
    let entry2 = InboxEntry::new("event-2", "second");
    let entry1_id = entry1.id.clone();
    write_inbox_entries(&dir, &[entry1, entry2]).unwrap();

    update_inbox_entry_status(&dir, &entry1_id, "dismissed").unwrap();

    let read = read_inbox(&dir).unwrap();
    assert_eq!(read.len(), 2);
    assert_eq!(read[0].status, "dismissed");
    assert_eq!(read[1].status, "open");

    std::fs::remove_dir_all(&dir).ok();
}
