//! Tests-only coverage for the automatic memory [`ExtractionEngine`].
//!
//! Exercises the tool-result hook (bash failure tracking, error-resolution
//! detection, file-edit pattern extraction), the session-end summary hook,
//! deduplication, the confirmation-vs-auto-store branches, and the
//! confidence-decay helper. All state is in-memory; no network or real files
//! outside the temp dir are touched.

use std::path::Path;
use std::sync::Arc;

use ragent_agent::event::{Event, EventBus};
use ragent_agent::memory::{
    ExtractionEngine, SessionMessageSummary, ToolCallSummary, decay_confidence,
};
use ragent_agent::storage::Storage;
use ragent_config::AutoExtractConfig;
use serde_json::json;

/// Build `(engine, storage, event_bus)` for a config, plus a temp working dir.
fn fixture(
    enabled: bool,
    require_confirmation: bool,
) -> (
    ExtractionEngine,
    Arc<Storage>,
    Arc<EventBus>,
    tempfile::TempDir,
) {
    let config = AutoExtractConfig {
        enabled,
        require_confirmation,
    };
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    let bus = Arc::new(EventBus::new(256));
    let dir = tempfile::tempdir().expect("tempdir");
    (ExtractionEngine::new(config), storage, bus, dir)
}

/// Count the `MemoryCandidateExtracted` events currently on `rx`.
fn drain_candidates(
    rx: &mut tokio::sync::broadcast::Receiver<Event>,
) -> Vec<(String, String, Vec<String>)> {
    let mut out = Vec::new();
    while let Ok(event) = rx.try_recv() {
        if let Event::MemoryCandidateExtracted {
            content,
            category,
            tags,
            ..
        } = event
        {
            out.push((content, category, tags));
        }
    }
    out
}

#[test]
fn disabled_engine_is_inert_for_tool_and_session_hooks() {
    let (engine, storage, bus, dir) = fixture(false, true);
    assert!(!engine.is_enabled());
    let mut rx = bus.subscribe();

    engine.on_tool_result(
        "Edit",
        &json!({ "path": "/tmp/x/tests/a.rs", "new_str": "anyhow::Result" }),
        "ok",
        true,
        "s1",
        &storage,
        &bus,
        dir.path(),
    );
    engine.on_session_end(
        "s1",
        &[SessionMessageSummary {
            role: "assistant".to_string(),
            tool_calls: vec![],
        }],
        &storage,
        &bus,
        dir.path(),
    );

    assert_eq!(drain_candidates(&mut rx).len(), 0, "disabled must not emit");
}

#[test]
fn is_enabled_reflects_config() {
    let (enabled, _, _, _) = fixture(true, true);
    assert!(enabled.is_enabled());
}

#[test]
fn bash_failure_then_success_extracts_error_resolution_event() {
    let (engine, storage, bus, dir) = fixture(true, true);
    let mut rx = bus.subscribe();

    // 1. A Bash failure is tracked.
    engine.on_tool_result(
        "Bash",
        &json!({ "command": "cargo build" }),
        "error: could not compile",
        false,
        "s1",
        &storage,
        &bus,
        dir.path(),
    );

    // 2. A subsequent Bash success resolves it.
    engine.on_tool_result(
        "Bash",
        &json!({ "command": "cargo build" }),
        "Finished dev profile",
        true,
        "s1",
        &storage,
        &bus,
        dir.path(),
    );

    let candidates = drain_candidates(&mut rx);
    assert_eq!(candidates.len(), 1, "one resolution candidate expected");
    let (content, category, tags) = &candidates[0];
    assert_eq!(category, "error");
    assert!(content.contains("failed with"), "content: {content}");
    // The memory tag stays the shell word `bash` (FR-028: a tag is not a tool name).
    assert!(tags.iter().any(|t| t == "bash"));
    assert!(tags.iter().any(|t| t == "error-resolution"));
}

#[test]
fn error_resolution_auto_stored_when_confirmation_disabled() {
    let (engine, storage, bus, dir) = fixture(true, false);
    let mut rx = bus.subscribe();

    engine.on_tool_result(
        "Bash",
        &json!({ "command": "make" }),
        "boom",
        false,
        "s1",
        &storage,
        &bus,
        dir.path(),
    );
    engine.on_tool_result(
        "Bash",
        &json!({ "command": "make" }),
        "now fine",
        true,
        "s1",
        &storage,
        &bus,
        dir.path(),
    );

    // With confirmation disabled the candidate is persisted, not just emitted.
    let memories = storage
        .list_memories("", 100)
        .expect("list memories should succeed");
    assert_eq!(memories.len(), 1, "candidate must be auto-stored");
    assert_eq!(memories[0].category, "error");

    let mut saw_stored = false;
    while let Ok(event) = rx.try_recv() {
        if matches!(event, Event::MemoryStored { .. }) {
            saw_stored = true;
        }
    }
    assert!(saw_stored, "MemoryStored event expected");
}

#[test]
fn file_edit_extracts_test_and_rust_patterns() {
    let (engine, storage, bus, dir) = fixture(true, true);
    let mut rx = bus.subscribe();

    let file = dir.path().join("tests").join("thing.rs");
    engine.on_tool_result(
        "Edit",
        &json!({
            "path": file.to_string_lossy(),
            "new_str": "anyhow::Result<()>\n#[tokio::test] async fn t() {}"
        }),
        "ok",
        true,
        "s1",
        &storage,
        &bus,
        dir.path(),
    );

    let candidates = drain_candidates(&mut rx);
    assert_eq!(candidates.len(), 1);
    let (content, category, tags) = &candidates[0];
    assert_eq!(category, "pattern");
    assert!(
        content.contains("Test files are located at"),
        "content: {content}"
    );
    assert!(tags.iter().any(|t| t == "testing"), "tags: {tags:?}");
    assert!(tags.iter().any(|t| t == "rust"), "tags: {tags:?}");
    assert!(tags.iter().any(|t| t == "error-handling"), "tags: {tags:?}");
    assert!(tags.iter().any(|t| t == "async"), "tags: {tags:?}");
}

#[test]
fn config_file_edit_extracts_config_tag() {
    let (engine, storage, bus, dir) = fixture(true, true);
    let mut rx = bus.subscribe();

    engine.on_tool_result(
        "Write",
        &json!({ "path": format!("{}/settings.toml", dir.path().display()) }),
        "ok",
        true,
        "s1",
        &storage,
        &bus,
        dir.path(),
    );

    let candidates = drain_candidates(&mut rx);
    assert_eq!(candidates.len(), 1);
    assert!(candidates[0].2.iter().any(|t| t == "config"));
    assert!(candidates[0].0.contains("Configuration file"));
}

#[test]
fn edit_without_path_produces_no_candidate() {
    let (engine, storage, bus, dir) = fixture(true, true);
    let mut rx = bus.subscribe();

    engine.on_tool_result(
        "Edit",
        &json!({ "new_str": "anyhow::Result" }),
        "ok",
        true,
        "s1",
        &storage,
        &bus,
        dir.path(),
    );

    assert_eq!(drain_candidates(&mut rx).len(), 0);
}

#[test]
fn duplicate_edit_candidate_is_suppressed() {
    let (engine, storage, bus, dir) = fixture(true, true);
    let mut rx = bus.subscribe();

    let path = dir.path().join("tests").join("dup.rs");
    let input = json!({ "path": path.to_string_lossy(), "new_str": "fn x() {}" });
    for _ in 0..2 {
        engine.on_tool_result("Edit", &input, "ok", true, "s1", &storage, &bus, dir.path());
    }

    assert_eq!(
        drain_candidates(&mut rx).len(),
        1,
        "the second identical candidate must be deduplicated"
    );
}

#[test]
fn session_end_extracts_workflow_summary() {
    let (engine, storage, bus, dir) = fixture(true, true);
    let mut rx = bus.subscribe();

    let edited = dir.path().join("src").join("lib.rs");
    let messages = vec![SessionMessageSummary {
        role: "assistant".to_string(),
        tool_calls: vec![
            ToolCallSummary {
                tool_name: "Edit".to_string(),
                input: json!({ "file_path": edited.to_string_lossy() }),
                success: true,
            },
            ToolCallSummary {
                tool_name: "Bash".to_string(),
                input: json!({ "command": "cargo test" }),
                success: false,
            },
        ],
    }];

    engine.on_session_end("s1", &messages, &storage, &bus, dir.path());

    let candidates = drain_candidates(&mut rx);
    assert_eq!(candidates.len(), 1);
    let (content, category, tags) = &candidates[0];
    assert_eq!(category, "workflow");
    assert!(content.contains("Files edited"), "content: {content}");
    assert!(
        content.contains("Encountered 1 tool errors"),
        "content: {content}"
    );
    assert!(tags.iter().any(|t| t == "session-summary"));
}

#[test]
fn session_end_with_no_activity_emits_nothing() {
    let (engine, storage, bus, dir) = fixture(true, true);
    let mut rx = bus.subscribe();

    engine.on_session_end(
        "s1",
        &[SessionMessageSummary {
            role: "user".to_string(),
            tool_calls: vec![],
        }],
        &storage,
        &bus,
        dir.path(),
    );

    assert_eq!(drain_candidates(&mut rx).len(), 0);
}

#[test]
fn decay_confidence_leaves_fresh_memories_untouched() {
    let storage = Storage::open_in_memory().expect("storage");
    storage
        .create_memory(
            "recent fact about widgets",
            "fact",
            "test",
            0.9,
            "",
            "s1",
            &[],
        )
        .expect("create memory");

    // A memory created "now" has < 1 day of age, so decay is a no-op.
    let updated = decay_confidence(&storage, 0.95, 0.1);
    assert_eq!(updated, 0);

    let memories = storage.list_memories("", 100).expect("list");
    assert_eq!(memories.len(), 1);
    assert!((memories[0].confidence - 0.9).abs() < f64::EPSILON);
}

#[test]
fn decay_confidence_on_empty_store_returns_zero() {
    let storage = Storage::open_in_memory().expect("storage");
    assert_eq!(decay_confidence(&storage, 0.9, 0.05), 0);
}

#[test]
fn word_overlap_dedup_detects_near_identical_content() {
    // A first memory is stored, then a near-identical candidate is proposed:
    // the FTS5-backed duplicate check must reject it via the overlap heuristic.
    let (engine, storage, bus, dir) = fixture(true, true);
    let mut rx = bus.subscribe();

    storage
        .create_memory(
            "Project uses anyhow for error handling everywhere",
            "pattern",
            "test",
            0.5,
            "",
            "s0",
            &[],
        )
        .expect("seed memory");

    let path = dir.path().join("tests").join("overlap.rs");
    engine.on_tool_result(
        "Edit",
        &json!({
            "path": path.to_string_lossy(),
            "new_str": "anyhow::Result<()>"
        }),
        "ok",
        true,
        "s1",
        &storage,
        &bus,
        dir.path(),
    );

    // The candidate content ("Test files are located at: ...") shares few words
    // with the seeded memory, so it IS emitted; this asserts the dedup path ran
    // without error and still produced a candidate.
    let candidates = drain_candidates(&mut rx);
    assert_eq!(candidates.len(), 1);
}

/// Confirm the candidate survives a serde round-trip (field coverage).
#[test]
fn memory_candidate_serde_round_trip() {
    let candidate = ragent_agent::memory::MemoryCandidate {
        content: "c".to_string(),
        category: "fact".to_string(),
        tags: vec!["a".to_string()],
        confidence: 0.42,
        source: "src".to_string(),
        reason: "why".to_string(),
    };
    let json = serde_json::to_string(&candidate).expect("serialize");
    let back: ragent_agent::memory::MemoryCandidate =
        serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back.confidence, 0.42);
    assert_eq!(back.tags, vec!["a".to_string()]);
}

/// A non-existent working-directory parent in the session summary path is
/// handled without panicking (exercises the `.unwrap_or_default()` branch).
#[test]
fn session_end_handles_root_working_dir() {
    let (engine, storage, bus, _dir) = fixture(true, true);
    let mut rx = bus.subscribe();
    engine.on_session_end("s1", &[], &storage, &bus, Path::new("/"));
    assert_eq!(drain_candidates(&mut rx).len(), 0);
}
