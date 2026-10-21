//! Tests for the Teams window frame (spec `agentui` T-004).
//!
//! Covers the modal geometry (FR-017), the footer key legend carrying the `Esc`
//! dismissal (FR-018), the ` Team: <name>  (lead + <n> teammate) ` table title
//! (FR-019), the twelve-column dimmed header (FR-020) and the guarantee that
//! every popup rectangle stays inside the terminal rectangle (NFR-010).
//!
//! Every case drives the real public render entry point through a ratatui
//! `TestBackend` and reads the painted buffer back, so no renderer logic is
//! re-implemented here (NFR-001, NFR-012).

mod support;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use ragent_agent::team::{TeamConfig, TeamMember};
use ragent_tui::App;
use ragent_tui::theme::colors;
use support::make_app;

/// An app with the Teams window open on team `alpha`.
fn open_teams(members: usize) -> App {
    let mut app = make_app();
    app.session_id = Some("lead-session-0001".to_string());
    app.agent_name = "general".to_string();
    app.active_team = Some(TeamConfig::new("alpha", "lead-session-0001"));
    for i in 0..members {
        let mut member = TeamMember::new(format!("reviewer-{i}"), format!("tm-{i:03}"), "general");
        member.session_id = Some(format!("child-session-{i}"));
        app.team_members.push(member);
    }
    app.show_teams_window = true;
    app
}

/// Render `app` at `width` x `height` and return the drawn buffer.
fn render(app: &mut App, width: u16, height: u16) -> Buffer {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("create test terminal");
    terminal
        .draw(|frame| ragent_tui::layout::render(frame, app))
        .expect("render frame");
    terminal.backend().buffer().clone()
}

/// The bordered modal rectangle, reconstructed from the exported geometry
/// helpers so the assertions do not depend on pixel sniffing.
fn expected_modal(width: u16, height: u16) -> Rect {
    let (area, _, _) = ragent_tui::layout::window_frame_testable(
        ragent_tui::layout::teams_window_percent_w(),
        ragent_tui::layout::teams_window_percent_h(),
        Rect::new(0, 0, width, height),
    );
    area
}

/// The inner (borderless) rectangle of the modal.
fn expected_inner(width: u16, height: u16) -> Rect {
    let modal = expected_modal(width, height);
    Rect::new(
        modal.x + 1,
        modal.y + 1,
        modal.width.saturating_sub(2),
        modal.height.saturating_sub(2),
    )
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

/// The first row inside `area` that contains every needle.
fn row_containing(buffer: &Buffer, area: Rect, needles: &[&str]) -> Option<u16> {
    (area.y..area.y.saturating_add(area.height)).find(|&y| {
        let text = row_text(buffer, y, area);
        needles.iter().all(|needle| text.contains(needle))
    })
}

/// The style of the cell where `needle` starts on row `y`.
fn style_of(buffer: &Buffer, y: u16, area: Rect, needle: &str) -> Style {
    let text = row_text(buffer, y, area);
    let offset = text
        .find(needle)
        .unwrap_or_else(|| panic!("{needle:?} not found on row {y}: {text:?}"));
    buffer
        .cell((area.x + offset as u16, y))
        .map(ratatui::buffer::Cell::style)
        .expect("cell exists")
}

// -- FR-017: geometry -------------------------------------------------------

/// The modal occupies 90% of the terminal width and 70% of its height, and is
/// centred with equal margins (FR-017).
#[test]
fn test_teams_modal_occupies_90_by_70_percent_and_is_centred() {
    assert_eq!(ragent_tui::layout::teams_window_percent_w(), 90);
    assert_eq!(ragent_tui::layout::teams_window_percent_h(), 70);

    for (width, height) in [(120u16, 40u16), (160, 44), (80, 24), (200, 50)] {
        let mut app = open_teams(2);
        let _ = render(&mut app, width, height);
        let modal = expected_modal(width, height);

        let want_w = u32::from(width) * 90 / 100;
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

/// The painted frame carries full box-drawing borders and a bold blue
/// ` Teams ` title (FR-017).
#[test]
fn test_teams_modal_paints_borders_and_bold_blue_title() {
    let mut app = open_teams(2);
    let modal = expected_modal(120, 40);
    let buffer = render(&mut app, 120, 40);

    let top_left = buffer.cell((modal.x, modal.y)).expect("top-left cell");
    assert_eq!(
        top_left.symbol(),
        "\u{250c}",
        "the modal must paint a full box border (FR-017)"
    );
    let top_right = buffer
        .cell((modal.right() - 1, modal.y))
        .expect("top-right cell");
    assert_eq!(top_right.symbol(), "\u{2510}", "border must close");
    let bottom_left = buffer
        .cell((modal.x, modal.bottom() - 1))
        .expect("bottom-left cell");
    assert_eq!(bottom_left.symbol(), "\u{2514}", "border must close");
    let bottom_right = buffer
        .cell((modal.right() - 1, modal.bottom() - 1))
        .expect("bottom-right cell");
    assert_eq!(bottom_right.symbol(), "\u{2518}", "border must close");

    let title = find_text_style(&buffer, modal, "Teams").expect("title painted");
    assert_eq!(
        title.fg,
        Some(Color::Blue),
        "title colour, style: {title:?}"
    );
    assert!(
        title.add_modifier.contains(Modifier::BOLD),
        "title must be bold, style: {title:?}"
    );
}

/// The modal is cleared beneath, so chat text cannot show through the popup
/// body (FR-017).
#[test]
fn test_teams_modal_clears_the_underlay() {
    let mut app = open_teams(2);
    for i in 0..40 {
        app.messages.push(ragent_agent::message::Message::user_text(
            "lead-session-0001",
            format!("UNDERLAY-{i}-XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"),
        ));
    }
    let buffer = render(&mut app, 120, 40);

    let content = app.teams_area;
    for y in content.y..content.bottom() {
        let text = row_text(&buffer, y, content);
        assert!(
            !text.contains("UNDERLAY-"),
            "row {y} shows the cleared underlay: {text:?}"
        );
    }
}

// -- FR-018: footer key legend ----------------------------------------------

/// The footer legend is the modal's last inner row, spanning the full inner
/// width (FR-018, FR-077).
#[test]
fn test_teams_legend_is_the_bottom_inner_row() {
    for (width, height) in [(120u16, 40u16), (160, 44), (60, 20), (34, 14)] {
        let mut app = open_teams(1);
        let _ = render(&mut app, width, height);

        let modal = expected_modal(width, height);
        let (_, content, legend) = ragent_tui::layout::window_frame_testable(
            ragent_tui::layout::teams_window_percent_w(),
            ragent_tui::layout::teams_window_percent_h(),
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

/// The legend names the dismissal key and the Teams wording, and the window
/// paints no ` Close ` button anywhere in its inner area (FR-018).
#[test]
fn test_teams_legend_names_the_escape_key_and_no_close_button_paints() {
    let mut app = open_teams(1);
    let buffer = render(&mut app, 120, 40);
    let (_, content, legend) = ragent_tui::layout::window_frame_testable(
        ragent_tui::layout::teams_window_percent_w(),
        ragent_tui::layout::teams_window_percent_h(),
        Rect::new(0, 0, 120, 40),
    );

    let legend_text = row_text(&buffer, legend.y, legend);
    for key in [
        "Up/Down",
        "PgUp/PgDn",
        "Space",
        "Del shut down",
        "Enter open",
        "Esc close",
    ] {
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

    for y in content.y..content.bottom() {
        let text = row_text(&buffer, y, content);
        assert!(
            !text.contains("Close"),
            "row {y} must not paint a Close button: {text:?}"
        );
    }
}

/// `Esc` dismisses the Teams window and leaves the team and its members
/// untouched (FR-018, FR-032).
#[tokio::test]
async fn test_teams_escape_dismisses_the_window() {
    let mut app = open_teams(2);
    let _ = render(&mut app, 120, 40);
    assert!(app.show_teams_window, "the window starts open");

    app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()))
        .await;

    assert!(
        !app.show_teams_window,
        "Esc must dismiss the Teams window (FR-018)"
    );
    assert_eq!(
        app.team_members.len(),
        2,
        "dismissing the window must not touch the member list"
    );
    assert!(
        app.active_team.is_some(),
        "dismissing the window must not clear the active team"
    );
}

// -- FR-019: table title ----------------------------------------------------

/// The Teams table is titled ` Team: <name>  (lead + <n> teammates) ` and uses
/// the singular form for exactly one teammate (FR-019).
#[test]
fn test_teams_table_title_names_the_team_and_pluralises_the_count() {
    for (members, expected) in [
        (0usize, "Team: alpha  (lead + 0 teammates)"),
        (1, "Team: alpha  (lead + 1 teammate)"),
        (2, "Team: alpha  (lead + 2 teammates)"),
    ] {
        let mut app = open_teams(members);
        let buffer = render(&mut app, 160, 44);
        let content = app.teams_area;
        let text = row_text(&buffer, content.y, content);
        assert!(
            text.contains(expected),
            "with {members} members the table title must read {expected:?}, got {text:?}"
        );
    }
}

// -- FR-020: header ---------------------------------------------------------

/// The first row of the Teams table names all twelve columns in dimmed hint
/// colour (FR-020).
#[test]
fn test_teams_table_header_names_twelve_dimmed_columns() {
    let mut app = open_teams(2);
    let buffer = render(&mut app, 200, 50);
    let content = app.teams_area;

    let columns = [
        "id", "name", "status", "model", "elapsed", "steps", "claimed", "done", "sent", "recv",
        "[stop]", "[x]",
    ];
    let header_row = row_containing(&buffer, content, &columns)
        .unwrap_or_else(|| panic!("the header row must name all twelve columns: {columns:?}"));
    assert_eq!(
        header_row,
        content.y + 1,
        "the header must be the first row inside the table block (FR-020)"
    );

    // Columns appear left to right in the documented order.
    let text = row_text(&buffer, header_row, content);
    let mut last = 0usize;
    for column in columns {
        let at = text
            .find(column)
            .unwrap_or_else(|| panic!("{column:?} missing from the header: {text:?}"));
        assert!(
            at >= last,
            "{column:?} is out of order in the header: {text:?}"
        );
        last = at;

        let style = style_of(&buffer, header_row, content, column);
        assert_eq!(
            style.fg,
            Some(colors::HINT),
            "{column:?} must use the hint colour, style: {style:?}"
        );
        assert!(
            style.add_modifier.contains(Modifier::DIM),
            "{column:?} must be dimmed, style: {style:?}"
        );
    }
}

// -- NFR-010: responsive minimum --------------------------------------------

/// Every Teams popup rectangle stays inside the terminal rectangle, and a
/// render at the smallest supported sizes never panics (NFR-010).
#[test]
fn test_teams_frame_rects_stay_inside_the_terminal() {
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
        let mut app = open_teams(2);
        let buffer = render(&mut app, width, height);
        let screen = Rect::new(0, 0, width, height);
        let modal = expected_modal(width, height);

        assert!(
            modal.right() <= screen.right() && modal.bottom() <= screen.bottom(),
            "at {width}x{height}: modal {modal:?} escapes the terminal {screen:?}"
        );

        let content = app.teams_area;
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

/// The table area is the inner area minus the one-row legend band, and never
/// overlaps it when there is room (FR-018, NFR-010).
#[test]
fn test_teams_content_area_excludes_the_legend_band() {
    for (width, height) in [(120u16, 40u16), (160, 44), (80, 24), (40, 12)] {
        let mut app = open_teams(2);
        let _ = render(&mut app, width, height);

        let inner = expected_inner(width, height);
        let content = app.teams_area;
        let (_, _, legend) = ragent_tui::layout::window_frame_testable(
            ragent_tui::layout::teams_window_percent_w(),
            ragent_tui::layout::teams_window_percent_h(),
            Rect::new(0, 0, width, height),
        );

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
        assert!(
            content.bottom() <= legend.y,
            "at {width}x{height}: content {content:?} must end above the legend {legend:?}"
        );
    }
}
