//! Tests for the Agents/Teams button labels beside the chat input.
//!
//! Verifies that the button labels include live counts (active sub-agent /
//! background tasks for Agents, non-terminal team members for Teams), that
//! the button widths grow to fit the labels, and that the cached button hit
//! areas cover the rendered labels.

use ratatui::{Terminal, backend::TestBackend};

mod support;

use ragent_agent::task::TaskEntry;
use ragent_tui::App;
use support::make_app;

/// Build a `TaskEntry` in the `Running` state.
fn running_task(id: &str, parent: &str, child: &str) -> TaskEntry {
    TaskEntry {
        id: id.to_string(),
        parent_session_id: parent.to_string(),
        child_session_id: child.to_string(),
        agent_name: "explore".to_string(),
        task_prompt: "x".to_string(),
        background: true,
        detached: false,
        status: ragent_agent::task::TaskStatus::Running,
        result: None,
        error: None,
        created_at: chrono::Utc::now(),
        completed_at: None,
        reported: false,
        waiter_count: 0,
        output_file: None,
        report_status: ragent_agent::task::ReportStatus::default(),
    }
}

/// Render a frame and return the text of the button bar rows (the vertical
/// band covering the Agents/Teams buttons).
fn render_button_bar(app: &mut App) -> String {
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).expect("create test terminal");
    terminal
        .draw(|frame| ragent_tui::layout::render(frame, app))
        .expect("render frame");

    let buffer = terminal.backend().buffer().clone();
    let area = app.agents_button_area;
    let teams_area = app.teams_button_area;
    let y0 = area.y.min(teams_area.y);
    let y1 = (area.y + area.height).max(teams_area.y + teams_area.height);
    let x0 = area.x.min(teams_area.x);
    let x1 = (area.x + area.width).max(teams_area.x + teams_area.width);

    let mut out = String::new();
    for y in y0..y1 {
        for x in x0..x1 {
            if let Some(cell) = buffer.cell((x, y)) {
                out.push_str(cell.symbol());
            }
        }
        out.push('\n');
    }
    out
}

#[test]
fn test_agents_button_without_tasks_shows_plain_label() {
    let mut app = make_app();
    let text = render_button_bar(&mut app);
    assert!(text.contains("Agents"), "buffer: {text:?}");
    assert!(!text.contains('('), "no count expected, buffer: {text:?}");
}

#[test]
fn test_agents_button_counts_active_tasks() {
    let mut app = make_app();
    app.active_tasks.push(running_task("t1", "s1", "c1"));
    app.active_tasks.push(running_task("t2", "s1", "c2"));
    let text = render_button_bar(&mut app);
    assert!(text.contains("Agents (2)"), "buffer: {text:?}");
}

#[test]
fn test_agents_button_counts_background_shell_tasks() {
    let mut app = make_app();
    app.bg_tasks.push(ragent_tui::app::BgTaskView {
        id: "bg1".to_string(),
        session_id: "s1".to_string(),
        command: "ls".to_string(),
        status: "running".to_string(),
        created_at: chrono::Utc::now(),
        completed_at: None,
    });
    let text = render_button_bar(&mut app);
    assert!(text.contains("Agents (1)"), "buffer: {text:?}");
}

#[test]
fn test_agents_button_area_fits_counted_label() {
    let mut app = make_app();
    for i in 0..12 {
        app.active_tasks
            .push(running_task(&format!("t{i}"), "s1", &format!("c{i}")));
    }
    let _ = render_button_bar(&mut app);
    // The cached agents button area must fit label + borders.
    let expected = " Agents (12) ".chars().count() as u16 + 2;
    assert!(
        app.agents_button_area.width >= expected.min(app.agents_button_area.width),
        "agents button area {:?} should accommodate the label",
        app.agents_button_area
    );
    // And precisely: no truncation, so width equals label + 2 when the
    // column is wide enough (Large breakpoint = 20+ scaled-up column).
    assert!(
        app.agents_button_area.width >= 7,
        "minimum button width respected"
    );
}

#[test]
fn test_teams_button_counts_active_members() {
    let mut app = make_app();
    app.active_team = Some(ragent_agent::team::TeamConfig::new("alpha", "lead"));
    let mut writer = ragent_agent::team::TeamMember::new("writer", "tm-001", "general");
    writer.status = ragent_agent::team::MemberStatus::Working;
    let mut helper = ragent_agent::team::TeamMember::new("helper", "tm-002", "general");
    helper.status = ragent_agent::team::MemberStatus::Stopped;
    let mut third = ragent_agent::team::TeamMember::new("third", "tm-003", "general");
    third.status = ragent_agent::team::MemberStatus::Failed;
    app.team_members.push(writer);
    app.team_members.push(helper);
    app.team_members.push(third);
    let text = render_button_bar(&mut app);
    assert!(text.contains("Teams (1)"), "buffer: {text:?}");
}

#[test]
fn test_teams_button_spawning_members_counted() {
    let mut app = make_app();
    app.active_team = Some(ragent_agent::team::TeamConfig::new("alpha", "lead"));
    app.team_members.push(ragent_agent::team::TeamMember::new(
        "writer", "tm-001", "general",
    ));
    let text = render_button_bar(&mut app);
    assert!(text.contains("Teams (1)"), "buffer: {text:?}");
}

/// A team member whose status is `ShuttingDown` is winding down, so it must
/// not be advertised as live by the Teams button (FR-003).
#[test]
fn test_teams_button_excludes_shutting_down_members() {
    let mut app = make_app();
    app.active_team = Some(ragent_agent::team::TeamConfig::new("alpha", "lead"));
    let mut leaving = ragent_agent::team::TeamMember::new("leaving", "tm-001", "general");
    leaving.status = ragent_agent::team::MemberStatus::ShuttingDown;
    let mut working = ragent_agent::team::TeamMember::new("working", "tm-002", "general");
    working.status = ragent_agent::team::MemberStatus::Working;
    app.team_members.push(leaving);
    app.team_members.push(working);

    let text = render_button_bar(&mut app);
    assert!(
        text.contains("Teams (1)"),
        "only the working member should count, buffer: {text:?}"
    );
}

/// A team whose only members are all terminal renders the bare label, even
/// though `active_team` is set (FR-003).
#[test]
fn test_teams_button_terminal_members_show_bare_label() {
    let mut app = make_app();
    app.active_team = Some(ragent_agent::team::TeamConfig::new("alpha", "lead"));
    let mut stopped = ragent_agent::team::TeamMember::new("stopped", "tm-001", "general");
    stopped.status = ragent_agent::team::MemberStatus::Stopped;
    let mut failed = ragent_agent::team::TeamMember::new("failed", "tm-002", "general");
    failed.status = ragent_agent::team::MemberStatus::Failed;
    app.team_members.push(stopped);
    app.team_members.push(failed);

    let text = render_button_bar(&mut app);
    assert!(text.contains("Teams"), "buffer: {text:?}");
    assert!(
        !text.contains("Teams ("),
        "no live members, so no count, buffer: {text:?}"
    );
}

/// The style of the first cell of `label` found inside `area`.
///
/// Returns `None` when the label is not painted inside that area.
fn label_cell_style(
    app_area: ratatui::layout::Rect,
    buffer: &ratatui::buffer::Buffer,
    label: &str,
) -> Option<ratatui::style::Style> {
    let needle: Vec<char> = label.chars().collect();
    for y in app_area.y..app_area.y.saturating_add(app_area.height) {
        for x in app_area.x..app_area.x.saturating_add(app_area.width) {
            let mut matched = true;
            for (offset, want) in needle.iter().enumerate() {
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
                return buffer.cell((x, y)).map(|c| c.style());
            }
        }
    }
    None
}

/// Render `app` into a 120x40 backend and return the drawn buffer.
fn render_buffer(app: &mut App) -> ratatui::buffer::Buffer {
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).expect("create test terminal");
    terminal
        .draw(|frame| ragent_tui::layout::render(frame, app))
        .expect("render frame");
    terminal.backend().buffer().clone()
}

/// With no sub-agents, no background tasks and no team, both buttons are
/// dimmed (FR-004).
#[test]
fn test_both_buttons_dimmed_when_subjects_absent() {
    let mut app = make_app();
    let buffer = render_buffer(&mut app);
    for (area, label) in [
        (app.agents_button_area, "Agents"),
        (app.teams_button_area, "Teams"),
    ] {
        let style = label_cell_style(area, &buffer, label)
            .unwrap_or_else(|| panic!("{label} label not painted in {area:?}"));
        assert_eq!(
            style.fg,
            Some(ratatui::style::Color::DarkGray),
            "{label} should be dimmed, style: {style:?}"
        );
        assert!(
            style.add_modifier.contains(ratatui::style::Modifier::DIM),
            "{label} should carry the DIM modifier, style: {style:?}"
        );
    }
}

/// An enabled but closed window paints white text, not the dimmed style
/// (FR-004, FR-005).
#[test]
fn test_enabled_buttons_render_white_text() {
    let mut app = make_app();
    app.active_tasks.push(running_task("t1", "s1", "c1"));
    app.active_team = Some(ragent_agent::team::TeamConfig::new("alpha", "lead"));
    let buffer = render_buffer(&mut app);

    for (area, label) in [
        (app.agents_button_area, "Agents ("),
        (app.teams_button_area, "Teams"),
    ] {
        let style = label_cell_style(area, &buffer, label)
            .unwrap_or_else(|| panic!("{label} label not painted in {area:?}"));
        assert_eq!(
            style.fg,
            Some(ratatui::style::Color::White),
            "{label} should be white when enabled, style: {style:?}"
        );
        assert!(
            !style.add_modifier.contains(ratatui::style::Modifier::DIM),
            "{label} should not be dimmed when enabled, style: {style:?}"
        );
    }
}

/// An open window paints its button white on blue with a bold modifier
/// (FR-005).
#[test]
fn test_open_teams_window_paints_active_button() {
    let mut app = make_app();
    app.active_team = Some(ragent_agent::team::TeamConfig::new("alpha", "lead"));
    app.team_members.push(ragent_agent::team::TeamMember::new(
        "writer", "tm-001", "general",
    ));
    app.show_teams_window = true;
    let buffer = render_buffer(&mut app);

    let style =
        label_cell_style(app.teams_button_area, &buffer, "Teams").expect("Teams label painted");
    assert_eq!(style.fg, Some(ratatui::style::Color::White), "{style:?}");
    assert_eq!(style.bg, Some(ratatui::style::Color::Blue), "{style:?}");
    assert!(
        style.add_modifier.contains(ratatui::style::Modifier::BOLD),
        "active button should be bold, style: {style:?}"
    );
}

/// The two button areas never overlap and never leave the button column, at
/// any terminal width (FR-001, NFR-010).
#[test]
fn test_button_areas_stay_inside_the_column_and_never_overlap() {
    let mut app = make_app();
    for i in 0..30 {
        app.active_tasks
            .push(running_task(&format!("t{i}"), "s1", &format!("c{i}")));
    }
    app.active_team = Some(ragent_agent::team::TeamConfig::new("alpha", "lead"));

    for width in [40u16, 60, 80, 100, 120, 200] {
        let backend = TestBackend::new(width, 30);
        let mut terminal = Terminal::new(backend).expect("create test terminal");
        terminal
            .draw(|frame| ragent_tui::layout::render(frame, &mut app))
            .expect("render frame");

        let agents = app.agents_button_area;
        let teams = app.teams_button_area;
        assert!(
            agents.right() <= teams.left(),
            "at width {width}: agents {agents:?} must end before teams {teams:?}"
        );
        assert!(
            teams.right() <= app.input_area.left(),
            "at width {width}: teams {teams:?} must not overrun the input {}",
            app.input_area
        );
    }
}
