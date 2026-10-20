//! Single-row teammate strip rendered directly below the status bar.
//!
//! The strip carries one pill per member of the active team: a status glyph, the
//! member name clipped to [`NAME_COLUMNS`] display columns, and a distinct
//! background for the focused teammate. The ` Alt+^v:cycle ` hint is pinned to
//! the right edge so it stays visible when the pills would otherwise overflow.
//!
//! The row is omitted entirely when the active team has no members, so no blank
//! line is reserved (spec `agentui` FR-027, FR-066).
//!
//! Depends on [`crate::app::App`] for the team/focus state and on
//! `ragent_agent::team` for the member status enum.

use ragent_agent::team::{MemberStatus, TeamMember};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::app::App;

/// Background of the strip and of an unfocused pill.
const STRIP_BG: Color = Color::Rgb(30, 30, 40);

/// Background of the focused teammate's pill (FR-027).
const FOCUS_BG: Color = Color::Rgb(50, 50, 80);

/// Display columns a teammate name is clipped to (FR-027).
pub const NAME_COLUMNS: usize = 16;

/// Right-edge focus-cycling hint (FR-027).
pub const HINT: &str = " Alt+^v:cycle ";

/// Whether the strip occupies a screen line.
///
/// True only while a team is active and has at least one member (FR-027); a team
/// with no members reserves no line at all (FR-066).
pub fn is_visible(app: &App) -> bool {
    app.active_team.is_some() && !app.team_members.is_empty()
}

/// The glyph and colour used for one member status.
pub fn status_glyph(status: MemberStatus) -> (&'static str, Color) {
    match status {
        MemberStatus::Working => ("\u{25b6}", Color::Cyan),
        MemberStatus::Idle => ("\u{25cf}", Color::Green),
        MemberStatus::Spawning => ("\u{25cc}", Color::Yellow),
        MemberStatus::Blocked => ("\u{25c8}", Color::DarkGray),
        MemberStatus::PlanPending => ("\u{25ce}", Color::Magenta),
        MemberStatus::Suspended => ("[pause]", Color::DarkGray),
        MemberStatus::ShuttingDown => ("\u{25cc}", Color::Yellow),
        MemberStatus::Stopped => ("\u{25cb}", Color::DarkGray),
        MemberStatus::Failed => ("\u{2717}", Color::Red),
    }
}

/// Clip `name` to at most `budget` display columns.
///
/// The cut is taken between characters, so a multi-byte sequence can never be
/// split, and a wide character that would straddle the boundary is dropped
/// rather than allowed to overflow the budget (NFR-004).
pub fn clip_name(name: &str, budget: usize) -> String {
    let mut out = String::with_capacity(name.len().min(budget));
    let mut width = 0usize;
    for ch in name.chars() {
        let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if width + ch_width > budget {
            break;
        }
        out.push(ch);
        width += ch_width;
    }
    out
}

/// The spans of one teammate pill, plus their total display width.
///
/// The focused teammate gets a distinct background, brighter text and a leading
/// marker so the highlight survives a monochrome terminal (FR-027).
fn pill(member: &TeamMember, focused: bool) -> (Vec<Span<'static>>, usize) {
    let (glyph, glyph_color) = status_glyph(member.status);
    let bg = if focused { FOCUS_BG } else { STRIP_BG };
    let base = Style::default().bg(bg);
    let marker = if focused { "\u{25b8}" } else { " " };

    let name_style = if focused {
        base.fg(Color::White).add_modifier(Modifier::BOLD)
    } else {
        // Bright grey rather than terminal Gray: an unfocused pill stays
        // comfortably readable on the strip background while the focused
        // pill keeps both the bolder background and the bold weight.
        base.fg(Color::Rgb(208, 208, 208))
    };

    let head = format!("{marker}{glyph} ");
    let label = clip_name(&member.name, NAME_COLUMNS);
    let width = head.width() + label.width() + 1;

    (
        vec![
            Span::styled(head, base.fg(glyph_color)),
            Span::styled(label, name_style),
            Span::styled(" ", base),
        ],
        width,
    )
}

/// Build the strip's single line for a given row width.
///
/// Pills are painted left to right until the next one would not fit, then the
/// remainder is padded so the cycle hint lands on the right edge (FR-027). The
/// hint is never dropped, even when the row is narrower than the pills.
pub fn strip_line(app: &App, width: u16) -> Line<'static> {
    let width = usize::from(width);
    let budget = width.saturating_sub(HINT.width());

    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut used = 0usize;

    let header = " [team] ";
    if header.width() <= budget {
        used = header.width();
        spans.push(Span::styled(
            header,
            Style::default()
                .fg(Color::Blue)
                .bg(STRIP_BG)
                .add_modifier(Modifier::BOLD),
        ));
    }

    for member in &app.team_members {
        let focused = app.focused_teammate.as_deref() == Some(member.agent_id.as_str());
        let (pill_spans, pill_width) = pill(member, focused);
        if used + pill_width > budget {
            break;
        }
        used += pill_width;
        spans.extend(pill_spans);
    }

    let pad = budget.saturating_sub(used);
    if pad > 0 {
        spans.push(Span::styled(" ".repeat(pad), Style::default().bg(STRIP_BG)));
    }
    spans.push(Span::styled(
        HINT,
        Style::default().fg(Color::DarkGray).bg(STRIP_BG),
    ));

    Line::from(spans)
}

/// Paint the teammate strip into `area` (one row directly below the status bar).
pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    frame.render_widget(
        Paragraph::new(strip_line(app, area.width)).style(Style::default().bg(STRIP_BG)),
        area,
    );
}
