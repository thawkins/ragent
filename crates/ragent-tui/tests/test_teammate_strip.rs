//! Render tests for the single-row teammate strip (spec `agentui` T-006).
//!
//! The strip is driven straight from synthetic in-memory state through
//! [`ragent_tui::teammate_strip`], so nothing here touches the repository
//! working tree (NFR-012) and no database or network call is made.
//!
//! Covers FR-027 (one pill per teammate with a status glyph, a 16-column name,
//! a distinct background for the focused teammate and the ` Alt+^v:cycle ` hint
//! at the right edge) and FR-066 (the row is omitted when the team has no
//! members).
//!
//! Tests only - no production logic is touched.

mod support;

use ragent_agent::team::{MemberStatus, TeamConfig, TeamMember};
use ragent_tui::App;
use ragent_tui::teammate_strip::{self, HINT, NAME_COLUMNS};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::style::Color;

use support::make_app;

/// A team with the given members attached and nothing else populated.
fn team_app(members: Vec<TeamMember>) -> App {
    let mut app = make_app();
    app.session_id = Some("sess-lead".to_string());
    app.active_team = Some(TeamConfig::new("alpha", "sess-lead"));
    app.team_members = members;
    app
}

/// A member with the given name, agent id and status.
fn member(name: &str, agent_id: &str, status: MemberStatus) -> TeamMember {
    let mut m = TeamMember::new(name, agent_id, "general");
    m.status = status;
    m
}

/// Render one frame of the whole TUI and return the terminal.
fn render(app: &mut App, width: u16, height: u16) -> Terminal<TestBackend> {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| ragent_tui::layout::render(frame, app))
        .expect("draw");
    terminal
}

/// Flatten the painted buffer into one string.
fn painted(terminal: &Terminal<TestBackend>) -> String {
    terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect()
}

/// The buffer row the strip paints into: directly below the 2-row status bar.
fn strip_row() -> u16 {
    2
}

/// The painted text of one buffer row.
fn row_text(buffer: &Buffer, y: u16) -> String {
    (0..buffer.area.width)
        .map(|x| buffer[(x, y)].symbol())
        .collect()
}

/// Paint the strip alone into a fresh one-row buffer of the given width.
fn render_strip(app: &App, width: u16) -> Buffer {
    let backend = TestBackend::new(width, 1);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| teammate_strip::render(frame, app, frame.area()))
        .expect("draw");
    terminal.backend().buffer().clone()
}

// ---------------------------------------------------------------------------
// Visibility (FR-027, FR-066)
// ---------------------------------------------------------------------------

#[test]
fn strip_is_visible_only_with_a_team_that_has_members() {
    let mut app = make_app();
    assert!(
        !teammate_strip::is_visible(&app),
        "no team at all must not show the strip"
    );

    app.active_team = Some(TeamConfig::new("alpha", "sess-lead"));
    assert!(
        !teammate_strip::is_visible(&app),
        "a team with no members must not show the strip (FR-066)"
    );

    app.team_members
        .push(member("reviewer", "tm-001", MemberStatus::Idle));
    assert!(
        teammate_strip::is_visible(&app),
        "a member must reveal the strip (FR-027)"
    );
}

#[test]
fn empty_team_reserves_no_screen_line() {
    let mut without_members = team_app(Vec::new());
    render(&mut without_members, 100, 30);
    let empty_chat = without_members.message_area;

    let mut with_member = team_app(vec![member("reviewer", "tm-001", MemberStatus::Idle)]);
    render(&mut with_member, 100, 30);
    let populated_chat = with_member.message_area;

    assert_eq!(
        populated_chat.y,
        empty_chat.y + 1,
        "a populated strip takes exactly one line above the chat"
    );
    assert!(
        !painted(&render(&mut team_app(Vec::new()), 100, 30)).contains(HINT.trim()),
        "no hint may be painted when the team has no members"
    );
    assert!(
        painted(&render(&mut with_member, 100, 30)).contains(HINT.trim()),
        "the hint must be painted once a member exists"
    );
}

// ---------------------------------------------------------------------------
// Pills (FR-027)
// ---------------------------------------------------------------------------

#[test]
fn one_pill_per_teammate_with_name_and_glyph() {
    let app = team_app(vec![
        member("alpha", "tm-001", MemberStatus::Working),
        member("bravo", "tm-002", MemberStatus::Idle),
        member("charlie", "tm-003", MemberStatus::Failed),
    ]);
    let buffer = render_strip(&app, 120);
    let text = row_text(&buffer, 0);

    assert!(text.contains("[team]"), "the strip is labelled: {text}");
    for name in ["alpha", "bravo", "charlie"] {
        assert!(text.contains(name), "missing pill for {name}: {text}");
    }
    assert_eq!(
        text.matches('\u{25b6}').count(),
        1,
        "the working glyph appears once: {text}"
    );
    assert_eq!(
        text.matches('\u{25cf}').count(),
        1,
        "the idle glyph appears once: {text}"
    );
    assert_eq!(
        text.matches('\u{2717}').count(),
        1,
        "the failed glyph appears once: {text}"
    );
}

#[test]
fn every_member_status_has_a_status_glyph() {
    let statuses = [
        MemberStatus::Spawning,
        MemberStatus::Working,
        MemberStatus::Idle,
        MemberStatus::PlanPending,
        MemberStatus::Blocked,
        MemberStatus::Suspended,
        MemberStatus::ShuttingDown,
        MemberStatus::Stopped,
        MemberStatus::Failed,
    ];
    let members = statuses
        .iter()
        .enumerate()
        .map(|(i, status)| member(&format!("tm-{i}"), &format!("tm-{i:03}"), *status))
        .collect();
    let app = team_app(members);
    let text = row_text(&render_strip(&app, 200), 0);

    for status in statuses {
        let (glyph, _) = teammate_strip::status_glyph(status);
        assert!(
            text.contains(glyph),
            "missing glyph {glyph:?} for {status:?}: {text}"
        );
    }
}

#[test]
fn long_name_is_clipped_to_sixteen_columns() {
    let long = "teammate-with-a-very-long-name";
    let app = team_app(vec![member(long, "tm-001", MemberStatus::Idle)]);
    let text = row_text(&render_strip(&app, 120), 0);

    let clipped: String = long.chars().take(NAME_COLUMNS).collect();
    assert_eq!(clipped, "teammate-with-a-");
    assert!(text.contains(&clipped), "expected clip in {text}");
    assert!(
        !text.contains("teammate-with-a-v"),
        "the name must not exceed {NAME_COLUMNS} columns: {text}"
    );
}

#[test]
fn clip_name_is_character_safe_and_width_aware() {
    assert_eq!(teammate_strip::clip_name("reviewer", 16), "reviewer");
    assert_eq!(
        teammate_strip::clip_name("abcdefghijklmnopq", 16),
        "abcdefghijklmnop"
    );
    // Multi-byte characters are never split (NFR-004).
    assert_eq!(
        teammate_strip::clip_name("naïve-café-teammate", 16),
        "naïve-café-teamm"
    );
    // A wide character that would straddle the budget is dropped.
    assert_eq!(
        teammate_strip::clip_name("\u{4e2d}\u{6587}\u{540d}\u{79f0}", 3),
        "\u{4e2d}"
    );
    assert_eq!(teammate_strip::clip_name("anything", 0), "");
}

#[test]
fn hint_is_pinned_to_the_right_edge() {
    let app = team_app(vec![member("reviewer", "tm-001", MemberStatus::Idle)]);
    let width = 100u16;
    let text = row_text(&render_strip(&app, width), 0);

    assert!(text.ends_with(HINT), "the hint must end the row: {text:?}");
    assert_eq!(
        text.chars().count(),
        usize::from(width),
        "the row is padded to the full width"
    );
}

#[test]
fn hint_survives_a_narrow_row() {
    let app = team_app(vec![
        member("alphabetic-teammate", "tm-001", MemberStatus::Working),
        member("bravissimo-teammate", "tm-002", MemberStatus::Idle),
    ]);
    let text = row_text(&render_strip(&app, HINT.len() as u16), 0);
    assert_eq!(text, HINT, "the hint is never dropped for width");
}

#[test]
fn pills_that_do_not_fit_are_omitted_rather_than_overrunning() {
    let members = (0..12)
        .map(|i| {
            member(
                &format!("teammate-{i:02}"),
                &format!("tm-{i:03}"),
                MemberStatus::Idle,
            )
        })
        .collect();
    let app = team_app(members);
    let width = 60u16;
    let text = row_text(&render_strip(&app, width), 0);

    assert_eq!(text.chars().count(), usize::from(width));
    assert!(
        text.ends_with(HINT),
        "the hint stays right-aligned: {text:?}"
    );
    assert!(text.contains("teammate-00"), "the first pill fits: {text}");
    assert!(
        !text.contains("teammate-11"),
        "the last pill must be dropped when it does not fit: {text}"
    );
}

// ---------------------------------------------------------------------------
// Focus highlight (FR-027)
// ---------------------------------------------------------------------------

/// The buffer x of the first character of `needle` in one painted row.
fn column_of(buffer: &Buffer, y: u16, needle: &str) -> u16 {
    let text = row_text(buffer, y);
    let byte = text
        .find(needle)
        .unwrap_or_else(|| panic!("{needle:?} not in {text:?}"));
    let chars_before = text[..byte].chars().count();
    u16::try_from(chars_before).expect("col in range")
}

#[test]
fn focused_teammate_has_a_distinct_background() {
    let plain = team_app(vec![
        member("alpha", "tm-001", MemberStatus::Working),
        member("bravo", "tm-002", MemberStatus::Idle),
    ]);
    let mut focused = team_app(vec![
        member("alpha", "tm-001", MemberStatus::Working),
        member("bravo", "tm-002", MemberStatus::Idle),
    ]);
    focused.focused_teammate = Some("tm-002".to_string());

    let plain = render_strip(&plain, 100);
    let highlighted = render_strip(&focused, 100);

    let bravo = column_of(&plain, 0, "bravo");
    assert_eq!(
        plain[(bravo, 0)].bg,
        Color::Rgb(30, 30, 40),
        "an unfocused pill keeps the strip background"
    );
    assert_eq!(
        highlighted[(bravo, 0)].bg,
        Color::Rgb(50, 50, 80),
        "the focused pill carries the highlight background"
    );
}

#[test]
fn only_the_focused_pill_is_highlighted() {
    let mut app = team_app(vec![
        member("alpha", "tm-001", MemberStatus::Working),
        member("bravo", "tm-002", MemberStatus::Idle),
    ]);
    app.focused_teammate = Some("tm-001".to_string());
    let buffer = render_strip(&app, 100);

    let alpha = column_of(&buffer, 0, "alpha");
    let bravo = column_of(&buffer, 0, "bravo");

    assert_eq!(buffer[(alpha, 0)].bg, Color::Rgb(50, 50, 80));
    assert_eq!(buffer[(bravo, 0)].bg, Color::Rgb(30, 30, 40));
}

#[test]
fn stale_focus_id_highlights_nobody() {
    let mut app = team_app(vec![member("alpha", "tm-001", MemberStatus::Idle)]);
    app.focused_teammate = Some("tm-gone".to_string());
    let buffer = render_strip(&app, 100);

    let alpha = column_of(&buffer, 0, "alpha");
    assert_eq!(
        buffer[(alpha, 0)].bg,
        Color::Rgb(30, 30, 40),
        "an unknown focus id must not highlight any pill"
    );
}

// ---------------------------------------------------------------------------
// Full-frame placement (FR-027)
// ---------------------------------------------------------------------------

#[test]
fn strip_is_painted_directly_below_the_status_bar() {
    let mut app = team_app(vec![
        member("reviewer", "tm-001", MemberStatus::Working),
        member("tester", "tm-002", MemberStatus::Idle),
    ]);
    let terminal = render(&mut app, 120, 40);
    let buffer = terminal.backend().buffer();
    let text = row_text(buffer, strip_row());

    assert!(text.contains("[team]"), "strip row: {text:?}");
    assert!(text.contains("reviewer"), "strip row: {text:?}");
    assert!(text.ends_with(HINT), "strip row: {text:?}");
}

#[test]
fn strip_renders_from_the_full_frame_after_the_input_area() {
    let mut app = team_app(vec![member("auditor", "tm-001", MemberStatus::PlanPending)]);
    let terminal = render(&mut app, 120, 40);
    let painted = painted(&terminal);
    assert!(painted.contains("auditor"));
    assert!(painted.contains(HINT.trim()));
}
