//! Integration tests for the stateful loop cron mode (spec `piegap` T-005).
//!
//! Tests the `<loop-state>` / `<inbox>` tag protocol, state file persistence,
//! inbox JSONL writing, prompt injection, and all character/count caps.

use std::fs;

use ragent_agent::loop_state::{
    INBOX_ENTRY_MAX_CHARS, INBOX_MAX_PER_RUN, InboxEntry, LOOP_STATE_MAX_CHARS, LoopState,
    inject_state_into_prompt, parse_tags, read_inbox, strip_tags, write_inbox_entries,
};

/// Helper: create a unique temp directory for test isolation.
fn temp_dir() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "ragent-loop-state-int-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

// ── Tag Parsing ───────────────────────────────────────────────────────

#[test]
fn test_parse_tags_extracts_loop_state() {
    let output = "Work done.\n<loop-state>\nCheck build status next time.\n</loop-state>\nDone.";
    let parsed = parse_tags(output);
    assert_eq!(parsed.loop_state, "Check build status next time.");
    assert!(parsed.inbox_entries.is_empty());
}

#[test]
fn test_parse_tags_extracts_inbox_entries() {
    let output = "<inbox>Found a bug in module X</inbox>\n<inbox>Need to update docs</inbox>";
    let parsed = parse_tags(output);
    assert_eq!(parsed.inbox_entries.len(), 2);
    assert_eq!(parsed.inbox_entries[0], "Found a bug in module X");
    assert_eq!(parsed.inbox_entries[1], "Need to update docs");
}

#[test]
fn test_parse_tags_extracts_both() {
    let output = "Running checks...\n<loop-state>Iteration 3 complete</loop-state>\n<inbox>Flaky test found</inbox>";
    let parsed = parse_tags(output);
    assert_eq!(parsed.loop_state, "Iteration 3 complete");
    assert_eq!(parsed.inbox_entries.len(), 1);
    assert_eq!(parsed.inbox_entries[0], "Flaky test found");
}

#[test]
fn test_parse_tags_no_tags_returns_empty() {
    let parsed = parse_tags("Just some output text.");
    assert!(parsed.loop_state.is_empty());
    assert!(parsed.inbox_entries.is_empty());
}

#[test]
fn test_parse_tags_empty_input() {
    let parsed = parse_tags("");
    assert!(parsed.loop_state.is_empty());
    assert!(parsed.inbox_entries.is_empty());
}

#[test]
fn test_parse_tags_last_loop_state_wins() {
    let output =
        "<loop-state>first attempt</loop-state> middle <loop-state>corrected notes</loop-state>";
    let parsed = parse_tags(output);
    assert_eq!(parsed.loop_state, "corrected notes");
}

#[test]
fn test_parse_tags_malformed_missing_close_ignored() {
    let parsed = parse_tags("<loop-state>no closing tag here");
    assert!(parsed.loop_state.is_empty());
}

#[test]
fn test_parse_tags_malformed_inbox_missing_close_ignored() {
    let parsed = parse_tags("<inbox>incomplete finding");
    assert!(parsed.inbox_entries.is_empty());
}

#[test]
fn test_parse_tags_nested_tags_not_supported() {
    // Tags are not nested — inner tags are treated as text content.
    let output = "<loop-state>notes <inbox>not a real finding</inbox> more notes</loop-state>";
    let parsed = parse_tags(output);
    assert!(parsed.loop_state.contains("notes"));
    assert!(parsed.loop_state.contains("not a real finding"));
}

// ── Caps ──────────────────────────────────────────────────────────────

#[test]
fn test_loop_state_capped_at_max_chars() {
    let long = "x".repeat(LOOP_STATE_MAX_CHARS + 500);
    let output = format!("<loop-state>{long}</loop-state>");
    let parsed = parse_tags(&output);
    assert!(
        parsed.loop_state.chars().count() <= LOOP_STATE_MAX_CHARS,
        "loop state must be capped at {} chars, got {}",
        LOOP_STATE_MAX_CHARS,
        parsed.loop_state.chars().count()
    );
    assert!(parsed.loop_state.ends_with('…'));
}

#[test]
fn test_inbox_entry_capped_at_max_chars() {
    let long = "y".repeat(INBOX_ENTRY_MAX_CHARS + 200);
    let output = format!("<inbox>{long}</inbox>");
    let parsed = parse_tags(&output);
    assert_eq!(parsed.inbox_entries.len(), 1);
    assert!(
        parsed.inbox_entries[0].chars().count() <= INBOX_ENTRY_MAX_CHARS,
        "inbox entry must be capped at {} chars, got {}",
        INBOX_ENTRY_MAX_CHARS,
        parsed.inbox_entries[0].chars().count()
    );
    assert!(parsed.inbox_entries[0].ends_with('…'));
}

#[test]
fn test_inbox_max_per_run_findings() {
    let mut output = String::new();
    for i in 0..(INBOX_MAX_PER_RUN + 10) {
        output.push_str(&format!("<inbox>finding {i}</inbox>\n"));
    }
    let parsed = parse_tags(&output);
    assert_eq!(
        parsed.inbox_entries.len(),
        INBOX_MAX_PER_RUN,
        "at most {} findings per run, got {}",
        INBOX_MAX_PER_RUN,
        parsed.inbox_entries.len()
    );
    assert_eq!(parsed.inbox_entries[0], "finding 0");
    assert_eq!(
        parsed.inbox_entries[INBOX_MAX_PER_RUN - 1],
        format!("finding {}", INBOX_MAX_PER_RUN - 1)
    );
}

#[test]
fn test_loop_state_exact_max_not_truncated() {
    let exact = "x".repeat(LOOP_STATE_MAX_CHARS);
    let output = format!("<loop-state>{exact}</loop-state>");
    let parsed = parse_tags(&output);
    assert_eq!(parsed.loop_state.chars().count(), LOOP_STATE_MAX_CHARS);
    assert!(!parsed.loop_state.ends_with('…'));
}

#[test]
fn test_inbox_entry_exact_max_not_truncated() {
    let exact = "y".repeat(INBOX_ENTRY_MAX_CHARS);
    let output = format!("<inbox>{exact}</inbox>");
    let parsed = parse_tags(&output);
    assert_eq!(
        parsed.inbox_entries[0].chars().count(),
        INBOX_ENTRY_MAX_CHARS
    );
    assert!(!parsed.inbox_entries[0].ends_with('…'));
}

// ── State File I/O ────────────────────────────────────────────────────

#[test]
fn test_loop_state_save_and_load() {
    let dir = temp_dir();
    let state = LoopState {
        content: "Remember to check CI status".to_string(),
    };
    state.save(&dir, "event-save-load").unwrap();

    let loaded = LoopState::load(&dir, "event-save-load").unwrap();
    assert_eq!(loaded.content, "Remember to check CI status");

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_loop_state_load_missing_returns_empty() {
    let dir = temp_dir();
    let loaded = LoopState::load(&dir, "nonexistent-event").unwrap();
    assert!(loaded.content.is_empty());
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_loop_state_overwrite_on_save() {
    let dir = temp_dir();
    let state1 = LoopState {
        content: "first run notes".to_string(),
    };
    state1.save(&dir, "event-overwrite").unwrap();

    let state2 = LoopState {
        content: "second run notes".to_string(),
    };
    state2.save(&dir, "event-overwrite").unwrap();

    let loaded = LoopState::load(&dir, "event-overwrite").unwrap();
    assert_eq!(loaded.content, "second run notes");

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_loop_state_save_truncates_content() {
    let dir = temp_dir();
    let long = "z".repeat(LOOP_STATE_MAX_CHARS + 1000);
    let state = LoopState { content: long };
    state.save(&dir, "event-trunc-save").unwrap();

    let loaded = LoopState::load(&dir, "event-trunc-save").unwrap();
    assert!(loaded.content.chars().count() <= LOOP_STATE_MAX_CHARS);
    assert!(loaded.content.ends_with('…'));

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_loop_state_creates_parent_directory() {
    let dir = temp_dir();
    // The state file goes to <dir>/loop-state/<event>.txt — the
    // loop-state subdirectory should be created automatically.
    let state = LoopState {
        content: "test".to_string(),
    };
    state.save(&dir, "event-mkdir").unwrap();
    assert!(dir.join("loop-state").join("event-mkdir.txt").exists());
    fs::remove_dir_all(&dir).ok();
}

// ── Inbox JSONL ───────────────────────────────────────────────────────

#[test]
fn test_inbox_write_and_read() {
    let dir = temp_dir();
    let entries = vec![
        InboxEntry::new("event-a", "first finding"),
        InboxEntry::new("event-a", "second finding"),
        InboxEntry::new("event-b", "cross-event finding"),
    ];
    write_inbox_entries(&dir, &entries).unwrap();

    let read = read_inbox(&dir).unwrap();
    assert_eq!(read.len(), 3);
    assert_eq!(read[0].content, "first finding");
    assert_eq!(read[0].source_event_id, "event-a");
    assert_eq!(read[1].content, "second finding");
    assert_eq!(read[2].content, "cross-event finding");
    assert_eq!(read[2].source_event_id, "event-b");
    assert_eq!(read[0].status, "open");

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_inbox_read_missing_returns_empty() {
    let dir = temp_dir();
    let read = read_inbox(&dir).unwrap();
    assert!(read.is_empty());
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_inbox_append_to_existing() {
    let dir = temp_dir();
    let first_batch = vec![InboxEntry::new("event-1", "first")];
    write_inbox_entries(&dir, &first_batch).unwrap();

    let second_batch = vec![InboxEntry::new("event-1", "second")];
    write_inbox_entries(&dir, &second_batch).unwrap();

    let read = read_inbox(&dir).unwrap();
    assert_eq!(read.len(), 2);
    assert_eq!(read[0].content, "first");
    assert_eq!(read[1].content, "second");

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_inbox_empty_write_creates_no_file() {
    let dir = temp_dir();
    write_inbox_entries(&dir, &[]).unwrap();
    assert!(!dir.join("log").join("inbox").join("inbox.jsonl").exists());
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_inbox_entry_truncation_in_constructor() {
    let long = "w".repeat(INBOX_ENTRY_MAX_CHARS + 300);
    let entry = InboxEntry::new("event-1", &long);
    assert!(entry.content.chars().count() <= INBOX_ENTRY_MAX_CHARS);
    assert!(entry.content.ends_with('…'));
}

#[test]
fn test_inbox_entry_has_unique_id() {
    let entry1 = InboxEntry::new("event-1", "finding a");
    let entry2 = InboxEntry::new("event-1", "finding b");
    assert_ne!(entry1.id, entry2.id);
}

#[test]
fn test_inbox_jsonl_format() {
    let dir = temp_dir();
    let entry = InboxEntry::new("event-fmt", "test finding");
    write_inbox_entries(&dir, &[entry]).unwrap();

    let content = fs::read_to_string(dir.join("log").join("inbox").join("inbox.jsonl")).unwrap();
    // Each line should be valid JSON.
    let parsed: serde_json::Value = serde_json::from_str(content.trim()).unwrap();
    assert_eq!(parsed["content"], "test finding");
    assert_eq!(parsed["source_event_id"], "event-fmt");
    assert_eq!(parsed["status"], "open");
    assert!(parsed["id"].as_str().unwrap().len() > 0);
    assert!(parsed["timestamp"].as_str().unwrap().len() > 0);

    fs::remove_dir_all(&dir).ok();
}

// ── Prompt Injection ──────────────────────────────────────────────────

#[test]
fn test_inject_state_empty_returns_original() {
    let state = LoopState::default();
    let prompt = "run cargo test";
    let result = inject_state_into_prompt(prompt, &state);
    assert_eq!(result, prompt);
}

#[test]
fn test_inject_state_with_content_prepends_block() {
    let state = LoopState {
        content: "previous iteration notes".to_string(),
    };
    let prompt = "run cargo test";
    let result = inject_state_into_prompt(prompt, &state);
    assert!(result.starts_with("<loop-state>"));
    assert!(result.contains("previous iteration notes"));
    assert!(result.contains("run cargo test"));
    // The original prompt should come after the state block.
    let prompt_pos = result.find("run cargo test").unwrap();
    let state_end = result.find("</loop-state>").unwrap();
    assert!(prompt_pos > state_end);
}

#[test]
fn test_inject_state_whitespace_only_returns_original() {
    let state = LoopState {
        content: "   \n  ".to_string(),
    };
    let prompt = "do work";
    let result = inject_state_into_prompt(prompt, &state);
    assert_eq!(result, prompt);
}

// ── Strip Tags ────────────────────────────────────────────────────────

#[test]
fn test_strip_tags_removes_all_protocol_tags() {
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
fn test_strip_tags_no_tags_unchanged() {
    assert_eq!(strip_tags("plain text"), "plain text");
}

#[test]
fn test_strip_tags_multiple_inbox() {
    let output = "<inbox>a</inbox><inbox>b</inbox>text";
    let stripped = strip_tags(output);
    assert_eq!(stripped, "text");
}

#[test]
fn test_strip_tags_malformed_preserves_text() {
    let output = "text <loop-state>no close";
    let stripped = strip_tags(output);
    // Malformed tag is left in place since there's no close tag to match.
    assert!(stripped.contains("text"));
}

// ── End-to-End Stateful Loop Simulation ──────────────────────────────

#[test]
fn test_e2e_stateful_loop_cycle() {
    let dir = temp_dir();
    let event_id = "e2e-loop-event";

    // ── Run 1: no previous state, agent outputs state + inbox ──
    let state_before = LoopState::load(&dir, event_id).unwrap();
    assert!(state_before.content.is_empty());

    let prompt = "check build status";
    let injected_prompt = inject_state_into_prompt(prompt, &state_before);
    assert_eq!(injected_prompt, prompt); // no state to inject

    // Simulate agent output after run 1.
    let agent_output_1 = "Build succeeded.\n<loop-state>\nLast check: green. Next: run tests.\n</loop-state>\n<inbox>\nAll tests passed.\n</inbox>";
    let parsed_1 = parse_tags(agent_output_1);

    // Save state from run 1.
    let state_after_1 = LoopState {
        content: parsed_1.loop_state.clone(),
    };
    state_after_1.save(&dir, event_id).unwrap();

    // Write inbox from run 1.
    let inbox_entries_1: Vec<_> = parsed_1
        .inbox_entries
        .iter()
        .map(|c| InboxEntry::new(event_id, c))
        .collect();
    write_inbox_entries(&dir, &inbox_entries_1).unwrap();

    // ── Run 2: previous state is injected, agent outputs new state ──
    let state_before_2 = LoopState::load(&dir, event_id).unwrap();
    assert_eq!(
        state_before_2.content,
        "Last check: green. Next: run tests."
    );

    let injected_prompt_2 = inject_state_into_prompt(prompt, &state_before_2);
    assert!(injected_prompt_2.starts_with("<loop-state>"));
    assert!(injected_prompt_2.contains("Last check: green"));
    assert!(injected_prompt_2.contains("check build status"));

    // Simulate agent output after run 2.
    let agent_output_2 = "Tests failed.\n<loop-state>\nLast check: tests failed. Next: fix module X.\n</loop-state>\n<inbox>\nFlaky test in module X.\n</inbox>\n<inbox>\nTimeout in integration test.\n</inbox>";
    let parsed_2 = parse_tags(agent_output_2);

    // Save state from run 2 (overwrites run 1 state).
    let state_after_2 = LoopState {
        content: parsed_2.loop_state.clone(),
    };
    state_after_2.save(&dir, event_id).unwrap();

    // Write inbox from run 2.
    let inbox_entries_2: Vec<_> = parsed_2
        .inbox_entries
        .iter()
        .map(|c| InboxEntry::new(event_id, c))
        .collect();
    write_inbox_entries(&dir, &inbox_entries_2).unwrap();

    // ── Verify: state file has run 2's notes ──
    let final_state = LoopState::load(&dir, event_id).unwrap();
    assert_eq!(
        final_state.content,
        "Last check: tests failed. Next: fix module X."
    );

    // ── Verify: inbox has 3 entries total (1 from run 1, 2 from run 2) ──
    let inbox = read_inbox(&dir).unwrap();
    assert_eq!(inbox.len(), 3);
    assert_eq!(inbox[0].content, "All tests passed.");
    assert_eq!(inbox[1].content, "Flaky test in module X.");
    assert_eq!(inbox[2].content, "Timeout in integration test.");
    assert!(inbox.iter().all(|e| e.source_event_id == event_id));

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_e2e_stateful_loop_with_caps() {
    let dir = temp_dir();
    let event_id = "e2e-caps-event";

    // Agent outputs more than the allowed caps.
    let long_state = "s".repeat(LOOP_STATE_MAX_CHARS + 500);
    let long_finding = "f".repeat(INBOX_ENTRY_MAX_CHARS + 200);
    let mut output = format!("<loop-state>{long_state}</loop-state>");
    for i in 0..(INBOX_MAX_PER_RUN + 5) {
        output.push_str(&format!("<inbox>finding {i}: {long_finding}</inbox>"));
    }

    let parsed = parse_tags(&output);

    // State is capped.
    assert!(parsed.loop_state.chars().count() <= LOOP_STATE_MAX_CHARS);

    // Only MAX_PER_RUN findings are kept, each capped.
    assert_eq!(parsed.inbox_entries.len(), INBOX_MAX_PER_RUN);
    for entry in &parsed.inbox_entries {
        assert!(entry.chars().count() <= INBOX_ENTRY_MAX_CHARS);
    }

    // Save and verify persistence respects caps.
    let state = LoopState {
        content: parsed.loop_state.clone(),
    };
    state.save(&dir, event_id).unwrap();
    let loaded = LoopState::load(&dir, event_id).unwrap();
    assert!(loaded.content.chars().count() <= LOOP_STATE_MAX_CHARS);

    // Write inbox and verify.
    let entries: Vec<_> = parsed
        .inbox_entries
        .iter()
        .map(|c| InboxEntry::new(event_id, c))
        .collect();
    write_inbox_entries(&dir, &entries).unwrap();
    let read = read_inbox(&dir).unwrap();
    assert_eq!(read.len(), INBOX_MAX_PER_RUN);
    for entry in &read {
        assert!(entry.content.chars().count() <= INBOX_ENTRY_MAX_CHARS);
    }

    fs::remove_dir_all(&dir).ok();
}
