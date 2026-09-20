//! Tests for the message input queue foundation and counter render
//! (spec `inputqueue`).
//!
//! T-001 covers FR-001 (in-memory FIFO queue preserving submission order),
//! FR-004 (bounded to 32 entries), NFR-001 (O(1) enqueue/dequeue backed by a
//! deque) and NFR-005 (cleared on session reset, never persisted).
//!
//! T-004 covers FR-008/FR-009/FR-010 (two-digit zero-padded counter before `>`
//! while entries are pending; bare prompt when empty) and NFR-002/NFR-003 (a
//! fixed-width prefix that repaints on a queue change).

#[path = "support/mod.rs"]
mod support;

use ratatui::{Terminal, backend::TestBackend};

use ragent_tui::app::{MAX_INPUT_QUEUE, QueuedInput};
use ragent_tui::layout;

use ragent_tui::App;

/// Render one frame and return the text painted on the input area's first row.
fn input_row_text(app: &mut App) -> String {
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| layout::render(frame, app))
        .expect("draw");

    let area = app.input_area;
    let buffer = terminal.backend().buffer();
    // Row of text starts one column inside the block border.
    (area.x + 1..area.x + area.width.saturating_sub(1))
        .map(|x| buffer[(x, area.y + 1)].symbol().to_string())
        .collect()
}

/// Build a queued entry with no attachments.
fn entry(text: &str) -> QueuedInput {
    QueuedInput {
        text: text.to_string(),
        image_paths: Vec::new(),
    }
}

#[test]
fn test_queued_input_default_is_empty() {
    let queued = QueuedInput::default();
    assert!(queued.text.is_empty());
    assert!(queued.image_paths.is_empty());
}

#[test]
fn test_queued_input_is_clonable_and_comparable() {
    let queued = QueuedInput {
        text: "hello".to_string(),
        image_paths: vec![std::path::PathBuf::from("/tmp/a.png")],
    };
    assert_eq!(queued.clone(), queued);
    assert_ne!(queued, entry("hello"));
}

#[test]
fn test_max_input_queue_is_32() {
    assert_eq!(MAX_INPUT_QUEUE, 32);
}

#[test]
fn test_new_app_starts_with_empty_queue() {
    let app = support::make_app();
    assert_eq!(app.input_queue_len(), 0);
    assert!(app.input_queue.is_empty());
}

#[test]
fn test_input_queue_preserves_fifo_order() {
    let mut app = support::make_app();
    app.input_queue.push_back(entry("first"));
    app.input_queue.push_back(entry("second"));
    app.input_queue.push_back(entry("third"));
    assert_eq!(app.input_queue_len(), 3);

    // Oldest first: popping the front yields the entries in submission order.
    let popped: Vec<String> = (0..3)
        .map(|_| {
            app.input_queue
                .pop_front()
                .expect("queue should have entries")
                .text
        })
        .collect();
    assert_eq!(popped, vec!["first", "second", "third"]);
    assert_eq!(app.input_queue_len(), 0);
}

#[test]
fn test_input_queue_can_hold_up_to_capacity() {
    let mut app = support::make_app();
    for i in 0..MAX_INPUT_QUEUE {
        app.input_queue.push_back(entry(&format!("message-{i}")));
    }
    assert_eq!(app.input_queue_len(), MAX_INPUT_QUEUE);
    // The head is still the oldest entry; eviction from the front is O(1).
    assert_eq!(
        app.input_queue.front().map(|q| q.text.as_str()),
        Some("message-0")
    );
    app.input_queue.pop_front();
    assert_eq!(app.input_queue_len(), MAX_INPUT_QUEUE - 1);
}

#[test]
fn test_clear_input_queue_empties_the_queue() {
    let mut app = support::make_app();
    app.input_queue.push_back(entry("a"));
    app.input_queue.push_back(entry("b"));
    app.clear_input_queue();
    assert_eq!(app.input_queue_len(), 0);
    assert!(app.input_queue.is_empty());
}

#[test]
fn test_queued_input_preserves_image_attachments() {
    let mut app = support::make_app();
    app.input_queue.push_back(QueuedInput {
        text: "describe this image".to_string(),
        image_paths: vec![
            std::path::PathBuf::from("/tmp/one.png"),
            std::path::PathBuf::from("/tmp/two.jpg"),
        ],
    });
    let head = app.input_queue.front().expect("entry queued");
    assert_eq!(head.text, "describe this image");
    assert_eq!(head.image_paths.len(), 2);
}

#[test]
fn test_load_session_clears_the_input_queue() {
    let mut app = support::make_app();
    let dir = std::env::current_dir().expect("cwd");
    let session = app
        .session_processor
        .session_manager
        .create_session(dir)
        .expect("create session");

    app.input_queue
        .push_back(entry("must not survive a session switch"));
    assert_eq!(app.input_queue_len(), 1);

    app.load_session(&session.id).expect("load session");
    assert_eq!(
        app.input_queue_len(),
        0,
        "switching sessions must drop the previous session's queued messages"
    );
}

// ── T-004: two-digit queue counter render ────────────────────────────────

#[test]
fn test_prompt_is_bare_when_queue_is_empty() {
    let mut app = support::make_app();

    let row = input_row_text(&mut app);

    assert!(
        row.starts_with("> "),
        "an empty queue renders the bare prompt with no counter (FR-010): {row:?}"
    );
}

#[test]
fn test_counter_is_shown_when_queue_is_non_empty() {
    let mut app = support::make_app();
    app.input_queue.push_back(entry("one"));

    let row = input_row_text(&mut app);

    assert!(
        row.starts_with("01> "),
        "one queued entry renders a zero-padded counter immediately before `>` (FR-009): {row:?}"
    );
}

#[test]
fn test_counter_is_two_digits_zero_padded() {
    let mut app = support::make_app();
    for i in 0..3 {
        app.input_queue.push_back(entry(&format!("m{i}")));
    }

    let row = input_row_text(&mut app);

    assert!(
        row.starts_with("03> "),
        "the counter is zero-padded to two digits (FR-009): {row:?}"
    );
}

#[test]
fn test_counter_is_rendered_before_typed_text() {
    let mut app = support::make_app();
    app.input_queue.push_back(entry("one"));
    app.input_queue.push_back(entry("two"));
    app.input = "hello".to_string();
    app.input_cursor = app.input.chars().count();

    let row = input_row_text(&mut app);

    assert!(
        row.starts_with("02> hello"),
        "the counter renders immediately before the typed text (FR-009): {row:?}"
    );
}

#[test]
fn test_counter_is_clamped_to_two_digits() {
    let mut app = support::make_app();
    // Push directly past the cap to exercise the clamp (the render never
    // depends on the enqueue path enforcing the bound).
    for i in 0..120 {
        app.input_queue.push_back(entry(&format!("m{i}")));
    }

    let row = input_row_text(&mut app);

    assert!(
        row.starts_with("99> "),
        "the counter stays a fixed two columns wide and clamps at 99 (NFR-002): {row:?}"
    );
}

#[test]
fn test_prompt_reverts_when_queue_empties() {
    let mut app = support::make_app();
    app.input_queue.push_back(entry("one"));
    assert!(input_row_text(&mut app).starts_with("01> "));

    app.input_queue.pop_front();

    let row = input_row_text(&mut app);
    assert!(
        row.starts_with("> "),
        "the counter disappears once the queue is empty (FR-008, FR-010): {row:?}"
    );
}

#[test]
fn test_render_reflects_enqueue_on_next_frame() {
    let mut app = support::make_app();
    assert!(input_row_text(&mut app).starts_with("> "));

    app.input_queue.push_back(entry("one"));

    assert!(
        input_row_text(&mut app).starts_with("01> "),
        "the counter updates on the next frame after an enqueue (FR-009, NFR-003)"
    );
}

#[test]
fn test_input_render_cache_tracks_queue_len() {
    let mut app = support::make_app();
    let _ = input_row_text(&mut app);
    assert_eq!(app.input_render_cache.queue_len, 0);

    app.input_queue.push_back(entry("one"));
    let _ = input_row_text(&mut app);

    assert_eq!(
        app.input_render_cache.queue_len, 1,
        "the render cache must rebuild when the counter width changes (NFR-002)"
    );
}

#[test]
fn test_clear_input_queue_sets_redraw_flag() {
    let mut app = support::make_app();
    app.input_queue.push_back(entry("one"));
    app.needs_redraw = false;

    app.clear_input_queue();

    assert!(
        app.needs_redraw,
        "clearing a non-empty queue must repaint to drop the counter (NFR-003)"
    );
}

#[test]
fn test_clear_empty_queue_does_not_force_redraw() {
    let mut app = support::make_app();
    app.needs_redraw = false;

    app.clear_input_queue();

    assert!(
        !app.needs_redraw,
        "clearing an already-empty queue must not schedule a spurious repaint"
    );
}

// ---------------------------------------------------------------------------
// T-011 - configurable capacity with a 32 default (FR-015)
// ---------------------------------------------------------------------------

#[test]
fn test_app_capacity_falls_back_to_default_when_unset() {
    let app = support::make_app();

    // No `input_queue_capacity` is configured in this workspace, so the App
    // resolves the compiled default of 32 (FR-015, MAX_INPUT_QUEUE).
    assert_eq!(app.input_queue_capacity, MAX_INPUT_QUEUE);
}

#[test]
fn test_app_capacity_matches_resolved_config() {
    let app = support::make_app();
    let expected = ragent_config::Config::load()
        .unwrap_or_default()
        .effective_input_queue_capacity();

    assert_eq!(
        app.input_queue_capacity, expected,
        "App must adopt the capacity resolved from config (FR-015)"
    );
}
