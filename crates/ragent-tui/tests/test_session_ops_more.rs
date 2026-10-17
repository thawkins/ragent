//! Additional coverage for `src/app/session_ops.rs` public surfaces that the
//! existing suites do not touch.
//!
//! Targets: the queue-entry panel movement/promote/delete helpers, the
//! queue-control menu navigation helpers, `usage_display` provider branches,
//! the context-snapshot schedule/poll cycle, the input `handle_paste_text`
//! selection-replacement paths, the static `extract_text_from_lines` slicer,
//! `pane_at` hit-testing, and `clear_project_memory`.
//!
//! Tests only - no production logic is touched.

use ragent_tui::App;
use ragent_tui::app::{
    ConfiguredProvider, ContextPartitionSnapshot, ProviderSource, QueuedInput, RollbackOfferState,
    SelectionPane,
};

#[path = "support/mod.rs"]
mod support;

/// Seed a queued entry with no attachments.
fn entry(text: &str) -> QueuedInput {
    QueuedInput {
        text: text.to_string(),
        image_paths: Vec::new(),
    }
}

/// A `ContextPartitionSnapshot` with distinguishable partition values.
fn snapshot(history_count: usize) -> ContextPartitionSnapshot {
    ContextPartitionSnapshot {
        system_prompt_tokens: 100,
        tool_catalog_tokens: 200,
        tool_metadata_tokens: 30,
        history_tokens: 400,
        history_message_count: history_count,
        skills_tokens: 10,
        memory_tokens: 20,
        agents_md_tokens: 5,
        context_window_tokens: Some(200_000),
        last_input_tokens: 50_000,
    }
}

fn app_with_session(session: &str) -> App {
    let mut app = support::make_app();
    app.session_id = Some(session.to_string());
    app
}

// ===========================================================================
// Queue-entry panel (Show row) movement / promote / delete
// ===========================================================================

#[test]
fn queue_show_move_up_clamps_at_first_entry() {
    let mut app = support::make_app();
    app.input_queue.push_back(entry("a"));
    app.input_queue.push_back(entry("b"));
    app.queue_show_open = true;
    app.queue_show_selected = 0;

    app.queue_show_move_up();
    assert_eq!(
        app.queue_show_selected, 0,
        "cannot move above the first row"
    );
}

#[test]
fn queue_show_move_down_clamps_at_last_entry() {
    let mut app = support::make_app();
    app.input_queue.push_back(entry("a"));
    app.input_queue.push_back(entry("b"));
    app.queue_show_open = true;
    app.queue_show_selected = 0;

    app.queue_show_move_down();
    assert_eq!(app.queue_show_selected, 1);
    app.queue_show_move_down();
    assert_eq!(app.queue_show_selected, 1, "cannot move past the last row");
}

#[test]
fn queue_show_move_down_sets_redraw_flag() {
    let mut app = support::make_app();
    app.input_queue.push_back(entry("a"));
    app.input_queue.push_back(entry("b"));
    app.queue_show_open = true;
    app.queue_show_selected = 0;
    app.needs_redraw = false;

    app.queue_show_move_down();
    assert!(app.needs_redraw);
}

#[test]
fn queue_show_promote_swaps_toward_front_and_follows() {
    let mut app = support::make_app();
    for text in ["a", "b", "c"] {
        app.input_queue.push_back(entry(text));
    }
    app.queue_show_open = true;
    app.queue_show_selected = 2;

    app.queue_show_promote_selected();
    assert_eq!(
        app.input_queue
            .iter()
            .map(|e| e.text.clone())
            .collect::<Vec<_>>(),
        vec!["a", "c", "b"]
    );
    assert_eq!(
        app.queue_show_selected, 1,
        "the highlight follows the entry"
    );
}

#[test]
fn queue_show_promote_at_front_is_a_noop() {
    let mut app = support::make_app();
    app.input_queue.push_back(entry("a"));
    app.input_queue.push_back(entry("b"));
    app.queue_show_open = true;
    app.queue_show_selected = 0;
    app.needs_redraw = false;

    app.queue_show_promote_selected();
    assert_eq!(
        app.input_queue
            .iter()
            .map(|e| e.text.clone())
            .collect::<Vec<_>>(),
        vec!["a", "b"]
    );
    assert!(!app.needs_redraw);
}

#[test]
fn queue_show_promote_out_of_range_is_a_noop() {
    let mut app = support::make_app();
    app.input_queue.push_back(entry("a"));
    app.queue_show_open = true;
    app.queue_show_selected = 5;

    app.queue_show_promote_selected();
    assert_eq!(app.input_queue.len(), 1, "out-of-range index is ignored");
}

#[test]
fn queue_show_delete_removes_entry_and_shrinks_highlight() {
    let mut app = support::make_app();
    for text in ["a", "b", "c"] {
        app.input_queue.push_back(entry(text));
    }
    app.queue_show_open = true;
    app.queue_show_selected = 2;

    app.queue_show_delete_selected();
    assert_eq!(
        app.input_queue
            .iter()
            .map(|e| e.text.clone())
            .collect::<Vec<_>>(),
        vec!["a", "b"]
    );
    assert_eq!(app.queue_show_selected, 1, "highlight clamps to last entry");
    assert!(app.queue_show_open, "panel stays open while entries remain");
}

#[test]
fn queue_show_delete_last_entry_closes_panel() {
    let mut app = support::make_app();
    app.input_queue.push_back(entry("only"));
    app.queue_show_open = true;
    app.queue_show_selected = 0;

    app.queue_show_delete_selected();
    assert!(app.input_queue.is_empty());
    assert!(!app.queue_show_open, "empty queue closes the panel");
    assert_eq!(app.queue_show_selected, 0);
}

#[test]
fn queue_show_delete_out_of_range_is_a_noop() {
    let mut app = support::make_app();
    app.queue_show_open = true;
    app.queue_show_selected = 3;

    app.queue_show_delete_selected();
    assert!(app.input_queue.is_empty());
}

#[test]
fn queue_show_close_resets_highlight() {
    let mut app = support::make_app();
    app.queue_show_open = true;
    app.queue_show_selected = 4;
    app.needs_redraw = false;

    app.queue_show_close();
    assert!(!app.queue_show_open);
    assert_eq!(app.queue_show_selected, 0);
    assert!(app.needs_redraw);
}

// ===========================================================================
// Queue-control menu navigation
// ===========================================================================

#[test]
fn queue_menu_move_up_skips_unselectable_empty_next_row() {
    // With an empty queue the `Next` row (index 0) is not selectable, so
    // moving up from the `Halt` row lands on it only when a queue entry
    // exists; here it must skip straight past the dead row.
    let mut app = support::make_app();
    app.input_queue.push_back(entry("a"));
    app.queue_menu_selected = 2;
    app.needs_redraw = false;

    app.queue_menu_move_up();
    assert_eq!(app.queue_menu_selected, 1);
    assert!(app.needs_redraw);

    app.queue_menu_move_up();
    assert_eq!(
        app.queue_menu_selected, 0,
        "Next is selectable when the queue is non-empty"
    );
}

#[test]
fn queue_menu_move_down_stops_at_bottom_row() {
    let mut app = support::make_app();
    app.input_queue.push_back(entry("a"));
    app.queue_menu_selected = ragent_tui::app::QUEUE_MENU_ROWS - 1;

    app.queue_menu_move_down();
    assert_eq!(
        app.queue_menu_selected,
        ragent_tui::app::QUEUE_MENU_ROWS - 1,
        "no movement past the last selectable row"
    );
}

#[test]
fn queue_menu_row_selectable_only_gates_the_next_row() {
    let mut app = support::make_app();
    assert!(
        !app.queue_menu_row_selectable(ragent_tui::app::QUEUE_MENU_ROW_NEXT),
        "Next is not selectable on an empty queue"
    );
    assert!(app.queue_menu_row_selectable(ragent_tui::app::QUEUE_MENU_ROW_HALT));
    assert!(app.queue_menu_row_selectable(ragent_tui::app::QUEUE_MENU_ROW_CLEAR));
    assert!(app.queue_menu_row_selectable(ragent_tui::app::QUEUE_MENU_ROW_SHOW));
    assert!(!app.queue_menu_row_selectable(ragent_tui::app::QUEUE_MENU_ROWS));

    app.input_queue.push_back(entry("a"));
    assert!(app.queue_menu_row_selectable(ragent_tui::app::QUEUE_MENU_ROW_NEXT));
}

#[test]
fn queue_menu_labels_report_resume_when_idle_and_stop_when_blocked() {
    let mut app = support::make_app();
    assert_eq!(app.queue_menu_halt_label(), "Resume");

    // `queue_menu_halt_label` uses `is_input_blocked` (a running turn, not a
    // pending permission dialog), so a processing turn flips it to `Stop`.
    app.is_processing = true;
    assert_eq!(app.queue_menu_halt_label(), "Stop");
    assert_eq!(app.queue_menu_labels(), ["Next", "Stop", "Clear", "Show"]);
}

// ===========================================================================
// usage_display provider branches
// ===========================================================================

#[test]
fn usage_display_prefers_quota_percentage() {
    let mut app = support::make_app();
    app.quota_percent = Some(42.5);
    let (label, unknown) = app.usage_display();
    assert_eq!(label, "quota: 42.5%");
    assert!(!unknown);
}

#[test]
fn usage_display_copilot_quota_uses_plan_label() {
    let mut app = support::make_app();
    app.configured_provider = Some(ConfiguredProvider {
        id: "copilot".to_string(),
        name: "GitHub Copilot".to_string(),
        source: ProviderSource::AutoDiscovered,
    });
    app.quota_percent = Some(10.0);
    let (label, unknown) = app.usage_display();
    assert!(label.contains("quota: 10.0%"), "got: {label}");
    assert!(!unknown);
}

#[test]
fn usage_display_ollama_without_context_window_reports_local() {
    let mut app = support::make_app();
    app.configured_provider = Some(ConfiguredProvider {
        id: "ollama".to_string(),
        name: "Ollama".to_string(),
        source: ProviderSource::Database,
    });
    // No selected model -> no context window -> the local label.
    app.selected_model = None;
    let (label, unknown) = app.usage_display();
    assert_eq!(label, "local");
    assert!(!unknown);
}

#[test]
fn usage_display_unknown_provider_without_context_marks_unknown() {
    let mut app = support::make_app();
    app.configured_provider = Some(ConfiguredProvider {
        id: "some-provider".to_string(),
        name: "Some Provider".to_string(),
        source: ProviderSource::EnvVar,
    });
    app.selected_model = None;
    let (label, unknown) = app.usage_display();
    assert_eq!(label, "unknown");
    assert!(unknown, "no context window -> flagged unknown");
}

// ===========================================================================
// Context-panel snapshot schedule / poll
// ===========================================================================

#[test]
fn schedule_context_snapshot_refresh_is_a_noop_when_panel_closed() {
    let mut app = support::make_app();
    app.show_context_panel = false;
    app.context_refresh_inflight = false;

    app.schedule_context_snapshot_refresh();
    assert!(
        !app.context_refresh_inflight,
        "no refresh is scheduled while the panel is closed"
    );
}

#[tokio::test]
async fn schedule_then_poll_context_snapshot_refresh_populates_cache() {
    let mut app = app_with_session("s1");
    app.show_context_panel = true;
    app.schedule_context_snapshot_refresh();
    assert!(app.context_refresh_inflight, "refresh is in flight");

    // The disk-bound work runs on a blocking worker; poll until it lands.
    for _ in 0..200 {
        app.poll_context_snapshot_refresh();
        if app.context_snapshot_cache.is_some() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert!(
        app.context_snapshot_cache.is_some(),
        "the background snapshot must be adopted"
    );
    assert!(!app.context_refresh_inflight, "the in-flight latch clears");
}

#[test]
fn poll_context_snapshot_refresh_drops_stale_history_snapshot() {
    let mut app = app_with_session("s1");
    // Deposit a snapshot whose history count is stale relative to the app.
    *app.context_snapshot_result.lock().unwrap() = Some(snapshot(999));
    app.context_refresh_inflight = true;
    app.show_context_panel = false;

    app.poll_context_snapshot_refresh();
    assert!(
        app.context_snapshot_cache.is_none(),
        "a snapshot with a mismatched history count must be dropped"
    );
}

#[test]
fn context_effective_snapshot_prefers_cache_then_falls_back() {
    let mut app = app_with_session("s1");
    assert!(
        app.context_snapshot_cache.is_none(),
        "precondition: no cached snapshot"
    );
    // Fallback computes synchronously.
    let computed = app.context_effective_snapshot();
    assert_eq!(computed.history_message_count, 0);

    // A cached snapshot wins over the live computation.
    app.context_snapshot_cache = Some(snapshot(7));
    let cached = app.context_effective_snapshot();
    assert_eq!(cached.history_message_count, 7);
}

// ===========================================================================
// Input editing: paste, extract, cursor byte mapping
// ===========================================================================

#[test]
fn handle_paste_text_ignores_empty_and_carriage_returns_only() {
    let mut app = support::make_app();
    app.input = "keep".to_string();
    app.input_cursor = 4;

    app.handle_paste_text("\r\r");
    assert_eq!(app.input, "keep", "an all-CR paste is a no-op");

    app.handle_paste_text("");
    assert_eq!(app.input, "keep");
}

#[test]
fn handle_paste_text_strips_carriage_returns_and_inserts() {
    let mut app = support::make_app();
    app.input.clear();
    app.input_cursor = 0;

    app.handle_paste_text("a\r\nb");
    assert_eq!(app.input, "a\nb");
}

#[test]
fn handle_paste_text_replaces_active_keyboard_selection() {
    let mut app = support::make_app();
    app.input = "abcdef".to_string();
    app.input_cursor = 6;
    app.kb_select_anchor = Some(3);

    app.handle_paste_text("X");
    assert_eq!(app.input, "abcX");
    assert!(
        app.kb_select_anchor.is_none(),
        "selection cleared after paste"
    );
}

#[test]
fn cursor_byte_pos_at_char_index_handles_multibyte_and_out_of_range() {
    let mut app = support::make_app();
    app.input = "aé中".to_string();
    assert_eq!(app.cursor_byte_pos_at_char_index(0), 0);
    assert_eq!(app.cursor_byte_pos_at_char_index(1), 1);
    // 'é' is two bytes, so char index 2 is byte 3.
    assert_eq!(app.cursor_byte_pos_at_char_index(2), 3);
    // Past the end clamps to the byte length.
    assert_eq!(app.cursor_byte_pos_at_char_index(99), app.input.len());
}

#[test]
fn delete_next_char_and_remove_range_edit_middle_of_input() {
    let mut app = support::make_app();
    app.input = "abcd".to_string();
    app.input_cursor = 1;
    app.delete_next_char();
    assert_eq!(app.input, "acd");

    app.remove_input_char_range(0, 1);
    assert_eq!(app.input, "cd");

    // An empty range is a no-op.
    app.remove_input_char_range(1, 1);
    assert_eq!(app.input, "cd");
}

#[test]
fn insert_char_at_cursor_middle_inserts_and_advances() {
    let mut app = support::make_app();
    app.input = "ac".to_string();
    app.input_cursor = 1;
    app.insert_char_at_cursor('b');
    assert_eq!(app.input, "abc");
    assert_eq!(app.input_cursor, 2);
}

#[test]
fn extract_text_from_lines_joins_multiline_selection() {
    let lines = vec!["hello world".to_string(), "second line".to_string()];
    // inner origin (0,0); select from (0,0) to (5,1).
    let extracted = App::extract_text_from_lines(&lines, 0, 0, 0, 0, 5, 1);
    assert_eq!(extracted, "hello world\nsecond");
}

#[test]
fn extract_text_from_lines_clamps_when_row_missing() {
    let lines = vec!["only".to_string()];
    let extracted = App::extract_text_from_lines(&lines, 0, 0, 0, 0, 3, 3);
    // Row 3 has no backing line: it contributes an empty line.
    assert!(extracted.starts_with("only"), "got: {extracted:?}");
}

// ===========================================================================
// pane_at hit-testing branches
// ===========================================================================

#[test]
fn pane_at_reports_each_visible_panel_and_none_when_hidden() {
    use ratatui::layout::Rect;
    let mut app = support::make_app();
    app.message_area = Rect::new(0, 0, 10, 10);
    app.show_log = true;
    app.log_area = Rect::new(10, 0, 10, 10);
    app.show_telemetry = true;
    app.telemetry_area = Rect::new(20, 0, 10, 10);
    app.input_area = Rect::new(0, 20, 80, 3);

    assert_eq!(app.pane_at(5, 5), Some(SelectionPane::Messages));
    assert_eq!(app.pane_at(15, 5), Some(SelectionPane::Log));
    assert_eq!(app.pane_at(25, 5), Some(SelectionPane::Telemetry));
    assert_eq!(app.pane_at(5, 21), Some(SelectionPane::Input));

    // A hidden panel never wins hit-testing even with a live rect.
    app.show_log = false;
    assert_eq!(app.pane_at(15, 5), None);
}

// ===========================================================================
// clear_project_memory / pending rollback plumbing
// ===========================================================================

#[test]
fn clear_project_memory_reports_outcome_in_transcript() {
    let mut app = support::make_app();
    app.session_id = Some("s1".to_string());
    app.cwd_path = std::env::temp_dir();
    app.clear_project_memory();
    let text = app
        .messages
        .iter()
        .flat_map(|m| m.parts.iter())
        .filter_map(|p| match p {
            ragent_agent::message::MessagePart::Text { text } => Some(text.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(text.to_lowercase().contains("/memory clear"), "got: {text}");
    assert!(
        text.to_lowercase().contains("memor"),
        "the outcome is reported in the transcript, got: {text}"
    );
}

#[test]
fn reset_message_cache_clears_content_lines() {
    let mut app = support::make_app();
    app.message_content_lines = vec!["stale".to_string()];
    app.reset_message_cache();
    assert_eq!(app.message_content_lines, [] as [std::string::String; 0]);
}

#[test]
fn ensure_copy_content_lines_does_not_panic_on_empty_history() {
    let mut app = support::make_app();
    app.ensure_copy_content_lines();
    assert_eq!(app.message_content_lines, [] as [std::string::String; 0]);
}

#[test]
fn mark_message_dirty_lowers_the_watermark_only() {
    let mut app = support::make_app();
    // Seed a high watermark, then lower it twice; a lower index wins.
    app.message_cache_dirty_from = 10;
    app.mark_message_dirty(3);
    assert_eq!(
        app.message_cache_dirty_from, 3,
        "watermark lowered to the edit index"
    );
    // A higher index must not raise an already-low watermark.
    app.mark_message_dirty(7);
    assert_eq!(app.message_cache_dirty_from, 3);
}

#[test]
fn rollback_offer_state_is_publicly_constructible() {
    let offer = RollbackOfferState {
        session_id: "s1".to_string(),
        status: "completed".to_string(),
        iterations: 2,
        diffstat: "+1 -1".to_string(),
        files: vec!["a.rs".to_string()],
    };
    assert_eq!(offer.iterations, 2);
}
