//! Compaction-summary labelling in the message window / output overlay.
//!
//! A sub-agent's mid-run auto-compaction summary ("## Objective / ## Work
//! State / ## Next Move") used to render with the same magenta bullet as
//! assistant text, so users read it as the sub-agent's completion report and
//! concluded the agent was failing to terminate. These tests pin the
//! distinct `[compaction]` labelling.

#[path = "support/mod.rs"]
mod support;

use ratatui::{Terminal, backend::TestBackend};

use ragent_agent::message::{Message, Role};
use ragent_tui::App;
use ragent_tui::app::{OutputViewState, OutputViewTarget, ScreenMode};

/// A test [`App`] built on the shared fixture, with the log panel open.
fn make_app() -> App {
    let mut app = support::make_app();
    app.show_log = true;
    app
}

/// Render the output overlay for a child session containing one compaction
/// summary and one assistant message, and return the overlay text.
fn render_overlay(child_session: &str, app: &mut App) -> String {
    app.current_screen = ScreenMode::Chat;
    app.session_id = Some("lead-s1".to_string());
    app.output_view = Some(OutputViewState {
        target: OutputViewTarget::Session {
            session_id: child_session.to_string(),
            label: "explore [a1b2c3d4]".to_string(),
        },
        scroll_offset: 0,
        max_scroll: 0,
        line_cache: ragent_tui::app::OutputViewLineCache {
            wrapped_lines: Vec::new(),
            content_lines: Vec::new(),
            wrapped_count: 0,
            cache_width: 0,
            source_generation: 0,
        },
    });

    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).expect("create test terminal");
    terminal
        .draw(|frame| ragent_tui::layout::render(frame, app))
        .expect("render frame");

    let area = app.output_view_area;
    let buffer = terminal.backend().buffer().clone();
    let mut overlay_text = String::new();
    for y in area.y..area.y.saturating_add(area.height) {
        for x in area.x..area.x.saturating_add(area.width) {
            if let Some(cell) = buffer.cell((x, y)) {
                overlay_text.push_str(cell.symbol());
            }
        }
        overlay_text.push('\n');
    }
    overlay_text
}

#[test]
fn test_compaction_summary_renders_with_distinct_label() {
    let mut app = make_app();
    app.storage
        .create_session("child-s1", "/tmp")
        .expect("create child session");
    let mut compaction = Message::new("child-s1", Role::Compaction, Vec::new());
    compaction
        .parts
        .push(ragent_agent::message::MessagePart::Text {
            text: "## Objective\n- review the files\n\n## Next Move\n- (none)".to_string(),
        });
    app.storage.create_message(&compaction).expect("seed");
    let text = render_overlay("child-s1", &mut app);

    assert!(
        text.contains("[compaction]"),
        "compaction summary must carry the [compaction] label, got:\n{text}"
    );
    // The compaction summary content is still visible.
    assert!(
        text.contains("review the files"),
        "compaction summary content must render, got:\n{text}"
    );
}

#[test]
fn test_assistant_text_keeps_bullet_marker_without_compaction_label() {
    let mut app = make_app();
    app.storage
        .create_session("child-s2", "/tmp")
        .expect("create child session");
    app.storage
        .create_message(&Message::assistant_text(
            "child-s2",
            "## Objective\n- the real completion report",
        ))
        .expect("seed");
    let text = render_overlay("child-s2", &mut app);

    assert!(
        text.contains("the real completion report"),
        "assistant text must render, got:\n{text}"
    );
    assert!(
        !text.contains("[compaction]"),
        "assistant text must not be labelled as compaction, got:\n{text}"
    );
}
