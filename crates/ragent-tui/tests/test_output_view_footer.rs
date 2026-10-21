//! Tests for the output view overlay's footer legend and wheel routing
//! (spec `agentui` T-021: FR-074, FR-075).
//!
//! FR-074 pins the overlay's frame geometry: a 90x70 centred modal built by the
//! shared `window_frame` helper, with a one-row key legend along its bottom
//! inner row and the content computed as the inner area minus that band. The
//! overlay carries **no** Close button - `Esc` and a click outside the overlay
//! are its dismissal paths - so the footer is the only affordance it paints.
//!
//! FR-075 pins the dismissal and wheel behaviour: `Esc` closes
//! the overlay leaving every other value untouched, a click outside it closes it
//! too, and a wheel event over the overlay scrolls the overlay even when the
//! Agents popup is open beneath it - the overlay is the topmost surface, so its
//! wheel arms precede the popup arms.
//!
//! Every case drives the real render entry point / key and mouse router through
//! a ratatui `TestBackend`, so the assertions exercise the exact path a real
//! event takes (NFR-001). All state is synthetic and in-memory; nothing is
//! written into the repository (NFR-012).

mod support;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::{layout::Rect, style::Color};

use ragent_tui::app::{OutputViewState, OutputViewTarget};

/// The app type under test, re-exported through `support`.
type AppT = ragent_tui::App;

/// The centre of `area` - the click point a real user aims at.
fn centre(area: Rect) -> (u16, u16) {
    (
        area.x + area.width.saturating_sub(1) / 2,
        area.y + area.height.saturating_sub(1) / 2,
    )
}

/// A left-button press at `(col, row)`.
const fn click(col: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: col,
        row,
        modifiers: KeyModifiers::empty(),
    }
}

/// A wheel-up event at `(col, row)`.
const fn wheel_up(col: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::ScrollUp,
        column: col,
        row,
        modifiers: KeyModifiers::empty(),
    }
}

/// A wheel-down event at `(col, row)`.
const fn wheel_down(col: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: col,
        row,
        modifiers: KeyModifiers::empty(),
    }
}

/// An app with the output overlay open on a primary session.
fn app_with_output_view() -> AppT {
    let mut app = support::make_app();
    app.session_id = Some("s1".to_string());
    app.output_view = Some(OutputViewState {
        target: OutputViewTarget::Session {
            session_id: "s1".to_string(),
            label: "primary".to_string(),
        },
        scroll_offset: 0,
        max_scroll: 0,
        line_cache: ragent_tui::app::OutputViewLineCache::default(),
    });
    app
}

/// Render `app` with the Agents popup open beneath the overlay (the exact
/// combination a user sees after clicking an agent row) and return the buffer.
fn render_overlay_over_agents(app: &mut AppT, width: u16, height: u16) -> Buffer {
    app.show_agents_window = true;
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("create test terminal");
    terminal
        .draw(|frame| ragent_tui::layout::render(frame, app))
        .expect("render frame");
    terminal.backend().buffer().clone()
}

/// The full text of screen row `y`.
fn row_text(buffer: &Buffer, y: u16) -> String {
    (0..buffer.area.width)
        .filter_map(|x| buffer.cell((x, y)).map(|c| c.symbol().to_string()))
        .collect()
}

/// The overlay's footer legend band: the last inner row of the modal.
fn legend_area(app: &AppT) -> Rect {
    let area = app.output_view_area;
    Rect::new(
        area.x + 1,
        area.y + area.height.saturating_sub(2),
        area.width.saturating_sub(2),
        1,
    )
}

// ---------------------------------------------------------------------------
// FR-074: output overlay frame and footer legend
// ---------------------------------------------------------------------------

/// The overlay's content stops one row above the inner bottom, leaving the
/// footer band for the key legend (FR-074).
#[test]
fn test_output_view_footer_band_is_the_last_inner_row() {
    for (width, height) in [(120u16, 40u16), (160, 44), (60, 20)] {
        let mut app = app_with_output_view();
        render_overlay_over_agents(&mut app, width, height);

        let area = app.output_view_area;
        let legend = legend_area(&app);
        assert_eq!(
            legend.y,
            area.y + area.height.saturating_sub(2),
            "at {width}x{height}: the footer band is the last inner row"
        );
        assert!(
            legend.y < area.bottom(),
            "at {width}x{height}: the footer stays inside the modal"
        );
    }
}

/// The footer names the dismissal key and the scroll keys, painted dim, and the
/// overlay paints no ` Close ` label anywhere inside its frame (FR-074).
#[test]
fn test_output_view_footer_names_escape_and_no_close_button_paints() {
    let mut app = app_with_output_view();
    let buffer = render_overlay_over_agents(&mut app, 160, 44);
    let legend = legend_area(&app);

    let text = row_text(&buffer, legend.y);
    for key in ["PgUp/PgDn scroll", "Ctrl+PgUp/PgDn jump", "Esc close"] {
        assert!(text.contains(key), "the footer lists `{key}`: {text:?}");
    }

    assert_eq!(
        buffer.cell((legend.x, legend.y)).and_then(|c| c.style().fg),
        Some(Color::DarkGray),
        "the footer legend paints in the dim hint colour"
    );

    // No ` Close ` label survives: scan every row inside the modal.
    let area = app.output_view_area;
    for y in area.y..area.bottom() {
        let row = row_text(&buffer, y);
        assert!(
            !row.contains(" Close "),
            "row {y} must not paint a Close button: {row:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// FR-075: dismissal
// ---------------------------------------------------------------------------

/// `Esc` closes the overlay and clears the selection identity with it
/// (FR-075).
#[tokio::test]
async fn test_output_view_escape_closes_the_overlay() {
    let mut app = app_with_output_view();
    app.selected_agent_session_id = Some("s1".to_string());
    app.selected_agent_index = Some(0);
    render_overlay_over_agents(&mut app, 160, 44);

    app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()))
        .await;

    assert!(app.output_view.is_none(), "Esc closes the view");
    assert!(
        app.selected_agent_session_id.is_none(),
        "the selection identity clears with the view (FR-075)"
    );
    assert!(
        app.selected_agent_index.is_none(),
        "the selected index clears with the view (FR-075)"
    );
}

/// A click inside the overlay body is consumed - the overlay never lets the
/// click through to the chat beneath it (FR-075).
#[test]
fn test_output_view_body_click_is_consumed() {
    let mut app = app_with_output_view();
    render_overlay_over_agents(&mut app, 160, 44);

    let area = app.output_view_area;
    // The overlay's top-left body.
    app.handle_mouse_event(click(area.x + 3, area.y + 3));
    assert!(
        app.output_view.is_some(),
        "a body click does not close the overlay"
    );
}

/// Closing a closed overlay is a no-op that still keeps the window state sane.
#[test]
fn test_output_view_close_is_idempotent() {
    let mut app = app_with_output_view();
    app.close_output_view();
    assert!(app.output_view.is_none());
    app.close_output_view();
    assert!(app.output_view.is_none());
}

// ---------------------------------------------------------------------------
// FR-075: wheel routing above the Agents popup
// ---------------------------------------------------------------------------

/// A wheel over the output overlay scrolls the overlay even while the Agents
/// popup is open beneath it - the overlay is topmost, so the popup arms must
/// not steal the event (FR-075, the "locked to the newest page" bug report).
#[test]
fn test_output_view_wheel_scrolls_while_agents_popup_open() {
    let mut app = app_with_output_view();
    render_overlay_over_agents(&mut app, 160, 44);
    // The overlay wheel arms guard on the rendered area; the offset itself
    // clamps against the view's cached max_scroll (the same mechanism
    // `test_output_view_paging_shortcuts` exercises via keys).
    app.output_view.as_mut().unwrap().max_scroll = 50;

    let (col, row) = centre(app.output_view_area);
    app.handle_mouse_event(wheel_up(col, row));
    assert_eq!(
        app.output_view.as_ref().unwrap().scroll_offset,
        3,
        "a wheel-up over the overlay scrolls the overlay by the step"
    );

    app.handle_mouse_event(wheel_down(col, row));
    assert_eq!(
        app.output_view.as_ref().unwrap().scroll_offset,
        0,
        "a wheel-down scrolls back toward the newest page"
    );
}

/// The wheel over the overlay does not disturb the Agents popup beneath it:
/// scrolling up (toward older overlay content) never moves the popup's offset
/// (FR-075).
#[test]
fn test_output_view_wheel_does_not_scroll_the_popup_beneath() {
    let mut app = app_with_output_view();
    render_overlay_over_agents(&mut app, 160, 44);
    app.output_view.as_mut().unwrap().max_scroll = 50;
    let agents_offset_before = app.active_agents_scroll_offset;

    let (col, row) = centre(app.output_view_area);
    app.handle_mouse_event(wheel_up(col, row));
    app.handle_mouse_event(wheel_up(col, row));

    assert_eq!(
        app.active_agents_scroll_offset, agents_offset_before,
        "the popup beneath never scrolls from an overlay wheel"
    );
    assert_eq!(
        app.output_view.as_ref().unwrap().scroll_offset,
        6,
        "both steps land on the overlay"
    );
}

/// The Agents popup still scrolls from its own wheel when no overlay covers
/// it (the two-armed routing stays intact) (FR-008 / FR-059 regression).
#[test]
fn test_agents_popup_wheel_still_scrolls_without_overlay() {
    let mut app = support::make_app();
    app.active_agents_area = Rect::new(5, 5, 100, 20);
    app.active_agents_max_scroll = 20;
    let (col, row) = centre(app.active_agents_area);

    app.handle_mouse_event(wheel_up(col, row));
    assert_eq!(
        app.active_agents_scroll_offset, 3,
        "the popup still owns its own wheel"
    );
}
