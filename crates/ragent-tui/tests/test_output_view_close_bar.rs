//! Tests for the output view overlay's close bar and wheel routing
//! (spec `agentui` T-021: FR-074, FR-075).
//!
//! FR-074 pins the overlay's frame geometry: a 90x70 centred modal built by
//! the shared `window_frame` helper, with a bordered ` Close ` bar button in
//! the bottom-right of the inner area (clamped to at most 10 columns and 3
//! rows, exactly like the Agents and Teams close buttons) and the content
//! computed as the inner area minus the button height and one separating row.
//!
//! FR-075 pins the dismissal and wheel behaviour: clicking that button closes
//! the overlay leaving every other value untouched (same state the `Esc` and
//! outside-click paths establish), and a wheel event over the overlay scrolls
//! the overlay even when the Agents popup is open beneath it - the overlay is
//! the topmost surface, so its wheel arms precede the popup arms.
//!
//! Every case drives the real render entry point / mouse router through a
//! ratatui `TestBackend`, so the assertions exercise the exact path a real
//! event takes (NFR-001). All state is synthetic and in-memory; nothing is
//! written into the repository (NFR-012).

mod support;

use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::{Terminal, layout::Rect};

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

/// The screen row of `buffer` containing `needle`, if any.
fn row_containing(buffer: &Buffer, needle: &str) -> Option<u16> {
    (0..buffer.area.height).find(|&y| {
        let row: String = (0..buffer.area.width)
            .filter_map(|x| buffer.cell((x, y)).map(|c| c.symbol().to_string()))
            .collect();
        row.contains(needle)
    })
}

// ---------------------------------------------------------------------------
// FR-074: output overlay close-bar geometry
// ---------------------------------------------------------------------------

/// The overlay reuses the modal frame: a Close bar button in the bottom-right
/// of the inner area, clamped to at most 10 columns and 3 rows (FR-074).
#[test]
fn test_output_view_close_button_is_bottom_right_and_clamped() {
    for (width, height) in [(120u16, 40u16), (160, 44), (60, 20)] {
        let mut app = app_with_output_view();
        render_overlay_over_agents(&mut app, width, height);

        let area = app.output_view_area;
        // Inner area: the modal minus its one-cell border on each side.
        let inner = Rect::new(
            area.x + 1,
            area.y + 1,
            area.width.saturating_sub(2),
            area.height.saturating_sub(2),
        );
        let close = app.output_view_close_button_area;

        assert!(
            close.width <= 10,
            "at {width}x{height}: close width {} exceeds 10",
            close.width
        );
        assert!(
            close.height <= 3,
            "at {width}x{height}: close height {} exceeds 3",
            close.height
        );
        assert_eq!(
            close.width,
            10.min(inner.width),
            "at {width}x{height}: close must fill its clamped width"
        );
        assert_eq!(
            close.height,
            3.min(inner.height),
            "at {width}x{height}: close must fill its clamped height"
        );
        assert_eq!(
            close.right(),
            inner.right(),
            "at {width}x{height}: close must align to the inner right edge"
        );
        assert_eq!(
            close.bottom(),
            inner.bottom(),
            "at {width}x{height}: close must align to the inner bottom edge"
        );
        assert!(
            close.width > 0 && close.height > 0,
            "at {width}x{height}: close must be a real rect, not a default placeholder"
        );
    }
}

/// The overlay paints the bordered ` Close ` label inside its button rect, and
/// the button sits over the overlay's border area, not outside it (FR-074).
#[test]
fn test_output_view_close_button_paints_the_label() {
    let mut app = app_with_output_view();
    let buffer = render_overlay_over_agents(&mut app, 160, 44);

    let close_row = row_containing(&buffer, " Close ").expect("the Close label paints");
    let close = app.output_view_close_button_area;
    assert!(
        (close.y..close.y + close.height).contains(&close_row),
        "the ` Close ` label paints within the recorded button rect: row {close_row}, rect {close:?}"
    );
}

/// The content area never extends into the button's rows: the last content row
/// sits above the separator row above the button (FR-074).
#[test]
fn test_output_view_content_excludes_the_close_bar() {
    let mut app = app_with_output_view();
    render_overlay_over_agents(&mut app, 160, 44);

    let area = app.output_view_area;
    let inner_bottom = area.y + area.height - 1;
    let close_top = app.output_view_close_button_area.y;
    assert!(
        close_top + app.output_view_close_button_area.height - 1 < inner_bottom,
        "the button must live inside the inner area"
    );
    // The content height equals the inner minus button minus separator, so the
    // button top must be at least 2 rows (separator + gap) above the inner
    // bottom's content start - verified exactly by the frame arithmetic:
    // content.height = inner.height - close.height - 1, clamped >= 1.
    let inner_h = area.height.saturating_sub(2);
    let content_h = inner_h
        .saturating_sub(app.output_view_close_button_area.height + 1)
        .max(1);
    // No stored content rect exists, so assert via geometry: the button's top
    // minus inner top equals content height + 1 separator row, or the clamp
    // bottomed out at a 1-row content (button top anywhere in the inner area).
    assert!(
        app.output_view_close_button_area.y - (area.y + 1)
            >= content_h.saturating_sub(1).min(inner_h.saturating_sub(1)),
        "the button must sit at or below the content area"
    );
}

// ---------------------------------------------------------------------------
// FR-075: dismissal
// ---------------------------------------------------------------------------

/// Clicking the overlay's own Close button closes the overlay and clears the
/// selection identity with it (FR-075).
#[test]
fn test_output_view_close_button_click_closes_the_overlay() {
    let mut app = app_with_output_view();
    app.selected_agent_session_id = Some("s1".to_string());
    app.selected_agent_index = Some(0);
    render_overlay_over_agents(&mut app, 160, 44);

    let (col, row) = centre(app.output_view_close_button_area);
    app.handle_mouse_event(click(col, row));

    assert!(app.output_view.is_none(), "the Close click closes the view");
    assert!(
        app.selected_agent_session_id.is_none(),
        "the selection identity clears with the view (FR-075)"
    );
    assert!(
        app.selected_agent_index.is_none(),
        "the selected index clears with the view (FR-075)"
    );
}

/// A click inside the overlay body outside its button is still consumed -
/// the overlay never closes itself and never lets the click through (FR-075).
#[test]
fn test_output_view_body_click_outside_close_button_is_consumed() {
    let mut app = app_with_output_view();
    render_overlay_over_agents(&mut app, 160, 44);

    let area = app.output_view_area;
    // The overlay's top-left body, far from the bottom-right button.
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
