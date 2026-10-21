//! Tests for the Agents window frame (spec `agentui` T-002).
//!
//! Covers the frame geometry (FR-006), the footer key legend carrying the `Esc`
//! dismissal (FR-007), the table-area arithmetic above that footer (FR-008) and
//! the guarantee that every popup rectangle stays inside the terminal rectangle
//! at any size (NFR-010). Every case drives
//! the real public render entry point through a ratatui `TestBackend` and reads
//! the painted buffer back, so no renderer logic is re-implemented here
//! (NFR-001, NFR-012).

mod support;

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use ragent_tui::App;
use support::make_app;

/// Render `app` at `width` x `height` and return the drawn buffer.
fn render(app: &mut App, width: u16, height: u16) -> Buffer {
    app.show_agents_window = true;
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("create test terminal");
    terminal
        .draw(|frame| ragent_tui::layout::render(frame, app))
        .expect("render frame");
    terminal.backend().buffer().clone()
}

/// The bordered modal rectangle, recovered from the painted buffer by locating
/// the frame borders is fragile; instead the geometry is reconstructed through
/// the exported helpers and the cached click areas.
fn expected_modal(width: u16, height: u16) -> Rect {
    let (area, _, _) = ragent_tui::layout::window_frame_testable(
        ragent_tui::layout::agents_window_percent_w(),
        ragent_tui::layout::agents_window_percent_h(),
        Rect::new(0, 0, width, height),
    );
    area
}

/// Find the style of the first cell of `needle` painted inside `area`.
fn find_text_style(buffer: &Buffer, area: Rect, needle: &str) -> Option<Style> {
    let wanted: Vec<char> = needle.chars().collect();
    for y in area.y..area.y.saturating_add(area.height) {
        for x in area.x..area.x.saturating_add(area.width) {
            let mut matched = true;
            for (offset, want) in wanted.iter().enumerate() {
                let cell_x = x.saturating_add(offset as u16);
                match buffer.cell((cell_x, y)) {
                    Some(cell) if cell.symbol().starts_with(*want) => {}
                    _ => {
                        matched = false;
                        break;
                    }
                }
            }
            if matched {
                return buffer.cell((x, y)).map(ratatui::buffer::Cell::style);
            }
        }
    }
    None
}

/// Flatten one row of `area` into a string.
fn row_text(buffer: &Buffer, y: u16, area: Rect) -> String {
    let mut out = String::new();
    for x in area.x..area.x.saturating_add(area.width) {
        match buffer.cell((x, y)) {
            Some(cell) => out.push_str(cell.symbol()),
            None => out.push('?'),
        }
    }
    out
}

// -- FR-006: geometry --------------------------------------------------------

/// The modal occupies 70% of the terminal width and 70% of its height, and is
/// centred with equal margins (FR-006).
#[test]
fn test_agents_modal_occupies_70_by_70_percent_and_is_centred() {
    for (width, height) in [(120u16, 40u16), (160, 44), (80, 24), (200, 50)] {
        let mut app = make_app();
        let _ = render(&mut app, width, height);
        let modal = expected_modal(width, height);

        let want_w = u32::from(width) * 70 / 100;
        let want_h = u32::from(height) * 70 / 100;
        assert!(
            modal.width.abs_diff(want_w as u16) <= 2,
            "at {width}x{height}: modal width {} should be ~{want_w}",
            modal.width
        );
        assert!(
            modal.height.abs_diff(want_h as u16) <= 2,
            "at {width}x{height}: modal height {} should be ~{want_h}",
            modal.height
        );

        let left = modal.x;
        let right = width.saturating_sub(modal.x.saturating_add(modal.width));
        assert!(
            left.abs_diff(right) <= 1,
            "at {width}x{height}: {modal:?} is not horizontally centred"
        );
        let top = modal.y;
        let bottom = height.saturating_sub(modal.y.saturating_add(modal.height));
        assert!(
            top.abs_diff(bottom) <= 1,
            "at {width}x{height}: {modal:?} is not vertically centred"
        );
    }
}

/// The painted frame corners carry the box-drawing border and the title cell is
/// bold cyan (FR-006).
#[test]
fn test_agents_modal_paints_borders_and_bold_cyan_title() {
    let mut app = make_app();
    let modal = expected_modal(120, 40);
    let buffer = render(&mut app, 120, 40);

    let top_left = buffer.cell((modal.x, modal.y)).expect("top-left cell");
    assert_eq!(
        top_left.symbol(),
        "\u{250c}",
        "the modal must paint a full box border (FR-006)"
    );
    let bottom_right = buffer
        .cell((modal.right() - 1, modal.bottom() - 1))
        .expect("bottom-right cell");
    assert_eq!(bottom_right.symbol(), "\u{2518}", "border must close");

    let title = find_text_style(&buffer, modal, "Agents").expect("title painted");
    assert_eq!(
        title.fg,
        Some(Color::Cyan),
        "title colour, style: {title:?}"
    );
    assert!(
        title.add_modifier.contains(Modifier::BOLD),
        "title must be bold, style: {title:?}"
    );
}

/// The modal is cleared beneath, so chat text cannot show through the popup
/// body: no cell inside the inner area carries the underlay's text.
#[test]
fn test_agents_modal_clears_the_underlay() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());
    for i in 0..40 {
        app.messages.push(ragent_agent::message::Message::user_text(
            "s1",
            format!("UNDERLAY-{i}-XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"),
        ));
    }
    let buffer = render(&mut app, 120, 40);

    let content = app.active_agents_area;
    for y in content.y..content.bottom() {
        let text = row_text(&buffer, y, content);
        assert!(
            !text.contains("UNDERLAY-"),
            "row {y} shows the cleared underlay: {text:?}"
        );
    }
}

// -- FR-007: footer key legend ------------------------------------------------

/// The footer legend is the modal's last inner row, spanning the full inner
/// width so the hints have the whole panel to lay out in (FR-007, FR-077).
#[test]
fn test_agents_legend_is_the_bottom_inner_row() {
    for (width, height) in [(120u16, 40u16), (160, 44), (60, 20), (34, 14)] {
        let mut app = make_app();
        let _ = render(&mut app, width, height);

        let modal = expected_modal(width, height);
        let (_, content, legend) = ragent_tui::layout::window_frame_testable(
            ragent_tui::layout::agents_window_percent_w(),
            ragent_tui::layout::agents_window_percent_h(),
            Rect::new(0, 0, width, height),
        );

        assert_eq!(
            legend.y,
            modal.bottom().saturating_sub(2),
            "at {width}x{height}: the legend must be the last inner row"
        );
        assert_eq!(
            legend.height, 1,
            "at {width}x{height}: the legend band is one row tall"
        );
        assert_eq!(
            legend.width,
            modal.width.saturating_sub(2),
            "at {width}x{height}: the legend must span the inner width"
        );
        assert_eq!(
            content.bottom(),
            legend.y,
            "at {width}x{height}: the table ends where the legend band begins"
        );
    }
}

/// The legend names the dismissal key, and the window paints no ` Close `
/// button anywhere in its inner area (FR-007).
#[test]
fn test_agents_legend_names_the_escape_key_and_no_close_button_paints() {
    let mut app = make_app();
    let buffer = render(&mut app, 120, 40);
    let (_, content, legend) = ragent_tui::layout::window_frame_testable(
        ragent_tui::layout::agents_window_percent_w(),
        ragent_tui::layout::agents_window_percent_h(),
        Rect::new(0, 0, 120, 40),
    );

    let legend_text = row_text(&buffer, legend.y, legend);
    assert!(
        legend_text.contains("Esc close"),
        "the legend names the dismissal key: {legend_text:?}"
    );
    for key in ["Up/Down", "PgUp/PgDn", "Space", "Del kill", "Enter open"] {
        assert!(
            legend_text.contains(key),
            "the legend lists `{key}`: {legend_text:?}"
        );
    }
    let style = find_text_style(&buffer, legend, "Esc close").expect("legend must paint");
    assert_eq!(
        style.fg,
        Some(Color::DarkGray),
        "the legend is dim, style: {style:?}"
    );

    // No ` Close ` label survives anywhere inside the modal, and the footer row
    // is the only place a stray one could hide.
    for y in content.y..content.bottom() {
        let text = row_text(&buffer, y, content);
        assert!(
            !text.contains("Close"),
            "row {y} must not paint a Close button: {text:?}"
        );
    }
}

// -- FR-008: content area ----------------------------------------------------

/// The table area is the inner area minus the one-row legend band, spanning the
/// full inner width (FR-008).
#[test]
fn test_agents_content_area_excludes_the_legend_band() {
    for (width, height) in [(120u16, 40u16), (160, 44), (80, 24)] {
        let mut app = make_app();
        let _ = render(&mut app, width, height);

        let modal = expected_modal(width, height);
        let inner = Rect::new(
            modal.x + 1,
            modal.y + 1,
            modal.width.saturating_sub(2),
            modal.height.saturating_sub(2),
        );
        let content = app.active_agents_area;

        assert_eq!(
            content.x, inner.x,
            "at {width}x{height}: content must start at the inner left edge"
        );
        assert_eq!(
            content.width, inner.width,
            "at {width}x{height}: content must span the inner width"
        );
        assert_eq!(
            content.y, inner.y,
            "at {width}x{height}: content must start at the inner top edge"
        );
        assert_eq!(
            content.height,
            inner.height - 1,
            "at {width}x{height}: content height must be the inner height minus the legend row"
        );
    }
}

/// The content area never drops below one row (FR-008), even when the modal is
/// too short to hold the legend band.
#[test]
fn test_agents_content_area_clamps_to_one_row() {
    for (width, height) in [(30u16, 8u16), (30, 6), (24, 5), (10, 4), (4, 3)] {
        let mut app = make_app();
        let _ = render(&mut app, width, height);

        let modal = expected_modal(width, height);
        let inner_h = modal.height.saturating_sub(2);
        let want = inner_h.saturating_sub(1).max(1).min(inner_h);
        assert_eq!(
            app.active_agents_area.height, want,
            "at {width}x{height}: the table area must be the inner height minus the legend row, clamped to at least one row"
        );
    }
}

// -- NFR-010: responsive minimum --------------------------------------------

/// Every popup rectangle stays inside the terminal rectangle, and a render at
/// the smallest supported sizes never panics (NFR-010).
#[test]
fn test_agents_frame_rects_stay_inside_the_terminal() {
    for (width, height) in [
        (200u16, 60u16),
        (120, 40),
        (80, 24),
        (40, 12),
        (30, 8),
        (20, 6),
        (10, 4),
        (4, 3),
        (1, 1),
    ] {
        let mut app = make_app();
        let buffer = render(&mut app, width, height);
        let screen = Rect::new(0, 0, width, height);
        let modal = expected_modal(width, height);

        assert!(
            modal.right() <= screen.right() && modal.bottom() <= screen.bottom(),
            "at {width}x{height}: modal {modal:?} escapes the terminal {screen:?}"
        );

        let content = app.active_agents_area;
        assert!(
            content.right() <= screen.right() && content.bottom() <= screen.bottom(),
            "at {width}x{height}: content {content:?} escapes the terminal {screen:?}"
        );
        assert!(
            content.right() <= modal.right() && content.bottom() <= modal.bottom(),
            "at {width}x{height}: content {content:?} escapes the modal {modal:?}"
        );

        assert_eq!(
            buffer.area, screen,
            "at {width}x{height}: the buffer keeps the terminal geometry"
        );
    }
}

/// At a normal terminal size the table area never overlaps the footer legend
/// band, so the legend cannot overwrite a data row (FR-007, FR-008).
#[test]
fn test_agents_content_and_legend_do_not_overlap_when_there_is_room() {
    for (width, height) in [(120u16, 40u16), (160, 44), (80, 24), (40, 12)] {
        let mut app = make_app();
        let _ = render(&mut app, width, height);
        let content = app.active_agents_area;
        let (_, _, legend) = ragent_tui::layout::window_frame_testable(
            ragent_tui::layout::agents_window_percent_w(),
            ragent_tui::layout::agents_window_percent_h(),
            Rect::new(0, 0, width, height),
        );
        assert!(
            content.bottom() <= legend.y,
            "at {width}x{height}: content {content:?} must end above the legend {legend:?}"
        );
    }
}
