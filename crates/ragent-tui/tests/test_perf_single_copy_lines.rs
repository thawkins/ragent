//! PERF-048: single-copy rendered lines.
//!
//! The memory / research / output viewers must hold exactly one copy of their
//! rendered rows.  These tests drive the real render path and assert that the
//! caches are populated consistently after a frame:
//!
//! * `wrapped_lines` is the one retained copy of the rendered rows;
//! * `wrapped_count` and `content_lines` stay in lockstep with it;
//! * a second frame at the same width does not change the cached rows.

mod support;

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;

use ragent_tui::app::{MemoryViewState, OutputViewState, OutputViewTarget, ResearchViewState};

fn render_once(app: &mut ragent_tui::App, width: u16, height: u16) {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("create test terminal");
    terminal
        .draw(|frame| ragent_tui::layout::render(frame, app))
        .expect("draw frame");
}

fn memory_row() -> ragent_storage::storage::MemoryRow {
    ragent_storage::storage::MemoryRow {
        id: 7,
        content: "line one\nline two\nline three\n".repeat(30),
        category: "fact".to_string(),
        source: "test".to_string(),
        confidence: 0.9,
        project: "p".to_string(),
        session_id: "s".to_string(),
        created_at: "2026-01-01T00:00:00Z".to_string(),
        updated_at: "2026-01-01T00:00:00Z".to_string(),
        access_count: 0,
        last_accessed: None,
    }
}

#[test]
fn test_memory_view_holds_one_copy_of_rendered_lines() {
    let mut app = support::make_app();
    app.memory_view = Some(MemoryViewState {
        row: memory_row(),
        scroll_offset: 0,
        max_scroll: 0,
        line_cache: ragent_tui::app::OutputViewLineCache::default(),
    });

    render_once(&mut app, 120, 40);

    let cache = &app.memory_view.as_ref().expect("view open").line_cache;
    assert!(!cache.wrapped_lines.is_empty(), "rows must be cached");
    assert_eq!(
        cache.wrapped_count as usize,
        cache.wrapped_lines.len(),
        "wrapped_count must match the retained rows"
    );
    assert_eq!(
        cache.content_lines.len(),
        cache.wrapped_lines.len(),
        "content_lines must project the retained rows"
    );

    // Second frame at the same width: the cached rows are reused verbatim.
    let before: Vec<String> = cache
        .wrapped_lines
        .iter()
        .map(ToString::to_string)
        .collect();
    render_once(&mut app, 120, 40);
    let after: Vec<String> = app
        .memory_view
        .as_ref()
        .expect("view open")
        .line_cache
        .wrapped_lines
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(before, after, "a same-width frame must not re-render rows");
}

#[test]
fn test_research_view_holds_one_copy_of_rendered_lines() {
    let mut app = support::make_app();
    app.research_view = Some(ResearchViewState {
        name: "test".to_string(),
        path: std::path::PathBuf::from("/research/test/RESEARCH.md"),
        base_dir: std::path::PathBuf::from("/research/test"),
        markdown: "# Title\n\nbody line\n".repeat(40),
        scroll_offset: 0,
        max_scroll: 0,
        line_cache: ragent_tui::app::OutputViewLineCache::default(),
    });

    render_once(&mut app, 120, 40);

    let cache = &app.research_view.as_ref().expect("view open").line_cache;
    assert!(!cache.wrapped_lines.is_empty(), "rows must be cached");
    assert_eq!(cache.wrapped_count as usize, cache.wrapped_lines.len());
    assert_eq!(cache.content_lines.len(), cache.wrapped_lines.len());
}

#[test]
fn test_output_view_holds_one_copy_of_rendered_lines() {
    let mut app = support::make_app();
    app.session_id = Some("sess-1234".to_string());
    app.messages.push(ragent_agent::message::Message::user_text(
        "sess-1234".to_string(),
        "hello there, this is a prompt",
    ));
    app.output_view = Some(OutputViewState {
        target: OutputViewTarget::Session {
            session_id: "sess-1234".to_string(),
            label: "primary".to_string(),
        },
        scroll_offset: 0,
        max_scroll: 0,
        line_cache: ragent_tui::app::OutputViewLineCache::default(),
    });

    render_once(&mut app, 120, 40);

    let cache = &app.output_view.as_ref().expect("view open").line_cache;
    assert!(!cache.wrapped_lines.is_empty(), "rows must be cached");
    assert_eq!(
        cache.wrapped_count as usize,
        cache.wrapped_lines.len(),
        "wrapped_count must match the retained rows"
    );
    assert_eq!(cache.content_lines.len(), cache.wrapped_lines.len());

    // Re-render at the same width reuses the cached rows.
    let before = cache.content_lines.clone();
    render_once(&mut app, 120, 40);
    let after = app
        .output_view
        .as_ref()
        .expect("view open")
        .line_cache
        .content_lines
        .clone();
    assert_eq!(before, after, "a same-width frame must not re-render rows");
}

#[test]
fn test_output_view_area_tracks_rendered_overlay() {
    let mut app = support::make_app();
    app.session_id = Some("sess-1234".to_string());
    app.output_view = Some(OutputViewState {
        target: OutputViewTarget::Session {
            session_id: "sess-1234".to_string(),
            label: "primary".to_string(),
        },
        scroll_offset: 0,
        max_scroll: 0,
        line_cache: ragent_tui::app::OutputViewLineCache::default(),
    });

    render_once(&mut app, 120, 40);

    assert_ne!(
        app.output_view_area,
        Rect::default(),
        "the overlay render must record its area"
    );
}
