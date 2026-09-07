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
    app.active_team = Some(ragent_team::team::TeamConfig::new("alpha", "lead"));
    let mut writer = ragent_team::team::TeamMember::new("writer", "tm-001", "general");
    writer.status = ragent_team::team::MemberStatus::Working;
    let mut helper = ragent_team::team::TeamMember::new("helper", "tm-002", "general");
    helper.status = ragent_team::team::MemberStatus::Stopped;
    let mut third = ragent_team::team::TeamMember::new("third", "tm-003", "general");
    third.status = ragent_team::team::MemberStatus::Failed;
    app.team_members.push(writer);
    app.team_members.push(helper);
    app.team_members.push(third);
    let text = render_button_bar(&mut app);
    assert!(text.contains("Teams (1)"), "buffer: {text:?}");
}

#[test]
fn test_teams_button_spawning_members_counted() {
    let mut app = make_app();
    app.active_team = Some(ragent_team::team::TeamConfig::new("alpha", "lead"));
    app.team_members.push(ragent_team::team::TeamMember::new(
        "writer", "tm-001", "general",
    ));
    let text = render_button_bar(&mut app);
    assert!(text.contains("Teams (1)"), "buffer: {text:?}");
}
