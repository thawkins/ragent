//! Scratch probe for T-022 - delete before finishing.

mod support;

use ragent_agent::task::{TaskEntry, TaskStatus};
use ragent_types::event::Event;
use ratatui::{Terminal, backend::TestBackend};

fn entry(id: &str, status: TaskStatus, background: bool) -> TaskEntry {
    TaskEntry {
        id: id.to_string(),
        parent_session_id: "s1".to_string(),
        child_session_id: format!("child-{id}"),
        agent_name: "explore".to_string(),
        task_prompt: "x".to_string(),
        background,
        detached: false,
        status,
        result: None,
        error: None,
        created_at: chrono::Utc::now(),
        completed_at: None,
        reported: false,
        waiter_count: 0,
        output_file: None,
        report_status: ragent_agent::task::ReportStatus::default(),
        time_bound_secs: None,
    }
}

fn button_line(app: &mut ragent_tui::App) -> String {
    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal
        .draw(|f| ragent_tui::layout::render(f, app))
        .expect("render");
    let buf = terminal.backend().buffer().clone();
    let area = app.agents_button_area;
    let mut out = String::new();
    for x in area.x..(area.x + area.width + 12) {
        if let Some(c) = buf.cell((x, area.y)) {
            out.push_str(c.symbol());
        }
    }
    out
}

#[tokio::test]
async fn probe_gaps() {
    let mut app = support::make_app();
    app.session_id = Some("s1".to_string());

    // FR-031: terminating row must be excluded from the live count.
    app.active_tasks
        .push(entry("t-run", TaskStatus::Running, true));
    app.active_tasks
        .push(entry("t-term", TaskStatus::Terminating, true));
    println!(
        "PROBE A terminating+1 running button = {:?}",
        button_line(&mut app)
    );

    // FR-026: kill event must remove the row.
    app.handle_event(Event::SubagentKilled {
        session_id: "s1".to_string(),
        task_id: "t-term".to_string(),
        child_session_id: "child-t-term".to_string(),
        force: false,
    })
    .await;
    println!(
        "PROBE B after kill rows = {:?}",
        app.active_tasks
            .iter()
            .map(|t| t.id.clone())
            .collect::<Vec<_>>()
    );

    // FR-033: a foreground task must be marked as blocking the caller.
    app.active_tasks.clear();
    app.active_tasks
        .push(entry("t-fg", TaskStatus::Running, false));
    app.show_agents_window = true;
    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal
        .draw(|f| ragent_tui::layout::render(f, &mut app))
        .expect("render");
    let buf = terminal.backend().buffer().clone();
    let mut screen = String::new();
    for y in 0..30 {
        for x in 0..120 {
            if let Some(c) = buf.cell((x, y)) {
                screen.push_str(c.symbol());
            }
        }
        screen.push('\n');
    }
    println!("PROBE C fg screen:\n{screen}");
}
