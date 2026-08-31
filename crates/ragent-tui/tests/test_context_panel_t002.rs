//! Tests for the Context side panel (spec `contextpanel`).
//!
//! - **T-002**: Reserve right-hand layout column when panel is open.
//! - **T-011**: Render panel with title, border and percentage bars.

mod support;

use ragent_tui::app::SelectionPane;

/// Render the app at a fixed size with the Context panel visible.
fn render_with_context_panel(cols: u16, rows: u16) -> ratatui::backend::TestBackend {
    let mut app = support::make_app();
    app.show_context_panel = true;
    let backend = ratatui::backend::TestBackend::new(cols, rows);
    let mut terminal = ratatui::Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame| {
            ragent_tui::layout::render(frame, &mut app);
        })
        .expect("draw");
    terminal.backend().clone()
}

#[test]
fn test_layout_reserves_side_column_when_panel_open() {
    // FR-003: while the panel is visible, a right-hand column is reserved;
    // the messages area must shrink relative to the panel-hidden layout.
    let with_panel_app = support::make_app();
    let mut hidden_app = support::make_app();

    let backend_with = ratatui::backend::TestBackend::new(100, 30);
    let mut terminal_with = ratatui::Terminal::new(backend_with).expect("terminal");
    let mut app_with = with_panel_app;
    app_with.show_context_panel = true;
    terminal_with
        .draw(|frame| ragent_tui::layout::render(frame, &mut app_with))
        .expect("draw with panel");
    let msg_with = app_with.message_area.width;

    terminal_with
        .draw(|frame| ragent_tui::layout::render(frame, &mut hidden_app))
        .expect("draw without panel");
    let msg_without = hidden_app.message_area.width;
    let _ = msg_without;

    assert!(
        msg_with < hidden_app.message_area.width,
        "messages pane must shrink when the context panel opens: with={msg_with}, without={}",
        hidden_app.message_area.width
    );
    // FR-003: the panel's cached area occupies the reserved column.
    assert!(app_with.context_panel_area.width > 0);
    assert_eq!(app_with.context_panel_area.right(), 100);
}

#[test]
fn test_layout_full_width_when_panel_hidden() {
    // FR-004: with the panel hidden the messages area spans the full width
    // and no context panel area is reserved.
    let mut app = support::make_app();
    let backend = ratatui::backend::TestBackend::new(100, 30);
    let mut terminal = ratatui::Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame| ragent_tui::layout::render(frame, &mut app))
        .expect("draw");
    assert_eq!(app.message_area.width, 100);
    assert_eq!(app.context_panel_area.width, 0);
}

#[test]
fn test_panel_renders_title_and_border() {
    // FR-018: the panel shows a "Context" title bar and a border.
    let backend = render_with_context_panel(100, 30);
    let buffer = &backend.buffer();
    let mut has_title = false;
    for y in 0..buffer.area.height {
        for x in 0..buffer.area.width.saturating_sub(8) {
            let text: String = (x..x + 9)
                .filter_map(|cx| buffer.cell((cx, y)))
                .map(|c| c.symbol().to_string())
                .collect();
            if text == " Context " {
                has_title = true;
            }
        }
    }
    assert!(has_title, "panel title 'Context' must be rendered");
}

#[test]
fn test_panel_lists_partitions_and_totals() {
    // FR-005..FR-012: the required partition labels and total row render.
    // Height is kept at 40 so the newly-added "Context window" capacity row
    // still leaves room for the "Total" row on modest terminals.
    let backend = render_with_context_panel(100, 40);
    let buffer = &backend.buffer();
    let rendered: String = (0..buffer.area.height)
        .map(|y| {
            (0..buffer.area.width)
                .filter_map(|x| buffer.cell((x, y)))
                .map(|c| c.symbol().to_string())
                .collect::<String>()
        })
        .collect();
    for expected in [
        "Model context",
        "Context window",
        "System prompt",
        "Tool catalog",
        "Tool metadata",
        "History",
        "Total",
    ] {
        assert!(
            rendered.contains(expected),
            "panel must list '{expected}'; rendered:\n{rendered}"
        );
    }
    // FR-008: the message count line renders (0 messages in a fresh session).
    assert!(
        rendered.contains("0 messages"),
        "panel must show the message count"
    );
}

#[test]
fn test_pane_at_maps_context_panel_hits() {
    // FR-003: clicks inside the reserved column map to the context pane.
    let mut app = support::make_app();
    let backend = ratatui::backend::TestBackend::new(100, 30);
    let mut terminal = ratatui::Terminal::new(backend).expect("terminal");
    app.show_context_panel = true;
    terminal
        .draw(|frame| ragent_tui::layout::render(frame, &mut app))
        .expect("draw");
    let area = app.context_panel_area;
    let hit = app.pane_at(area.x + area.width / 2, area.y + area.height / 2);
    assert_eq!(hit, Some(SelectionPane::ContextPanel));
}

#[test]
fn test_panel_percent_or_unknown_labels() {
    // FR-010/FR-011: rows show either a percentage (when the window is
    // advertised via selected_model_ctx_window) or an "unknown" label.
    let mut app = support::make_app();
    app.selected_model_ctx_window = Some(200_000);
    let backend = ratatui::backend::TestBackend::new(100, 30);
    let mut terminal = ratatui::Terminal::new(backend).expect("terminal");
    app.show_context_panel = true;
    terminal
        .draw(|frame| ragent_tui::layout::render(frame, &mut app))
        .expect("draw");
    let buffer = &terminal.backend().buffer();
    let rendered: String = (0..buffer.area.height)
        .map(|y| {
            (0..buffer.area.width)
                .filter_map(|x| buffer.cell((x, y)))
                .map(|c| c.symbol().to_string())
                .collect::<String>()
        })
        .collect();
    assert!(
        rendered.contains('%') || rendered.contains("unknown"),
        "percentage values must render when the context window is known"
    );
}
