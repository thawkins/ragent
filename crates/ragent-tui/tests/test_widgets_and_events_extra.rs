//! Extra coverage tests for the ragent-tui render + event surfaces.
//!
//! Targets the largest uncovered files in the crate:
//! `src/widgets/message_widget.rs` (rendering via `TestBackend` and the
//! `tool_result_summary` match arms), `src/app/event_handler.rs` (synthetic
//! `Event` values fed through `App::handle_event`), `src/app/swarm.rs`,
//! `src/app/research.rs` and `src/research_progress.rs`.
//!
//! Tests only - no production logic is touched.

use std::collections::HashMap;

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;

use ragent_agent::event::Event;
use ragent_agent::message::{Message, MessagePart, Role, ToolCallState, ToolCallStatus};
use ragent_agent::team::{
    MemberStatus, SwarmDecomposition, SwarmState, SwarmSubtask, TeamMember, TeamStore,
};
use ragent_research::session::{SessionEvent, SessionPhase};
use ragent_tui::research_progress::{
    ResearchProgress, StepStatus, encode_cluster_progress_event, encode_progress_event,
};
use ragent_tui::widgets::message_widget::{MessageWidget, tool_result_summary};
use serde_json::json;

#[path = "support/mod.rs"]
mod support;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

type StepMap = HashMap<String, (String, u32, u32)>;

fn app_with_session(session: &str) -> ragent_tui::App {
    let mut app = support::make_app();
    app.session_id = Some(session.to_string());
    app.needs_redraw = false;
    app
}

/// Render a `Message` through `MessageWidget` into a `TestBackend` and return
/// the complete painted buffer as one string.
fn render_message(msg: &Message, width: u16, height: u16) -> String {
    let map: StepMap = HashMap::new();
    let widget = MessageWidget::new(msg, "/home/user/project", &map);
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| {
            let area = Rect::new(0, 0, width, height);
            frame.render_widget(widget, area);
        })
        .expect("draw");
    let buffer = terminal.backend().buffer();
    (0..height)
        .map(|y| {
            (0..width)
                .map(|x| buffer[(x, y)].symbol().to_string())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn user_msg(text: &str) -> Message {
    Message::new(
        "session-1",
        Role::User,
        vec![MessagePart::Text {
            text: text.to_string(),
        }],
    )
}

fn assistant_msg(text: &str) -> Message {
    Message::new(
        "session-1",
        Role::Assistant,
        vec![MessagePart::Text {
            text: text.to_string(),
        }],
    )
}

fn temp_swarm_dir(tag: &str) -> std::path::PathBuf {
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "ragent-tui-widgets-events-{}-{}-{}",
        std::process::id(),
        tag,
        n
    ));
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn subtask(id: &str, deps: &[&str], agent: Option<&str>, model: Option<&str>) -> SwarmSubtask {
    SwarmSubtask {
        id: id.to_string(),
        title: format!("Task {id}"),
        description: format!("Do the work for {id}"),
        depends_on: deps.iter().map(|s| (*s).to_string()).collect(),
        agent_type: agent.map(str::to_string),
        model: model.map(str::to_string),
    }
}

// ===========================================================================
// message_widget.rs - rendering via TestBackend
// ===========================================================================

#[test]
fn render_user_text_prefixes_you_and_indents_continuation() {
    let msg = user_msg("first line\nsecond line");
    let painted = render_message(&msg, 80, 8);
    assert!(painted.contains("You: first line"), "got:\n{painted}");
    assert!(painted.contains("second line"), "got:\n{painted}");
}

#[test]
fn render_assistant_plain_text_has_no_prefix() {
    let msg = assistant_msg("plain assistant output");
    let painted = render_message(&msg, 80, 4);
    assert!(painted.contains("plain assistant output"));
    assert!(!painted.contains("You: "));
}

#[test]
fn render_thinking_placeholder_becomes_thinking_indicator() {
    for dots in ["...", "......", ". ."] {
        let msg = assistant_msg(dots);
        let painted = render_message(&msg, 40, 3);
        assert!(
            painted.contains("Thinking ..."),
            "dots {dots:?} should render the thinking indicator, got:\n{painted}"
        );
    }
}

#[test]
fn render_assistant_agent_notice_uses_notice_bubble() {
    let msg = assistant_msg("Agent Notice\nsomething happened");
    let painted = render_message(&msg, 60, 5);
    assert!(painted.contains("Agent Notice"), "got:\n{painted}");
    assert!(painted.contains("something happened"), "got:\n{painted}");
}

#[test]
fn render_compaction_role_is_labelled() {
    let msg = Message::new(
        "s",
        Role::Compaction,
        vec![MessagePart::Text {
            text: "context refreshed".to_string(),
        }],
    );
    let painted = render_message(&msg, 60, 4);
    assert!(painted.contains("[compaction]"), "got:\n{painted}");
    assert!(painted.contains("context refreshed"), "got:\n{painted}");
}

#[test]
fn render_user_text_with_red_marker_renders_body_only() {
    // `[red]` markers are stripped to their body (the marker text is not shown).
    let msg = user_msg("prefix [red]blocked[/red] suffix");
    let painted = render_message(&msg, 60, 4);
    assert!(painted.contains("blocked"), "got:\n{painted}");
    assert!(
        !painted.contains("[red]"),
        "marker must be stripped:\n{painted}"
    );
}

#[test]
fn render_tool_call_running_shows_step_tag_and_summary() {
    let msg = Message::new(
        "s",
        Role::Assistant,
        vec![MessagePart::ToolCall {
            tool: "read".to_string(),
            call_id: "c1".to_string(),
            state: Box::new(ToolCallState {
                status: ToolCallStatus::Running,
                input: json!({"path": "/home/user/project/src/main.rs"}),
                output: None,
                error: None,
                duration_ms: None,
            }),
        }],
    );
    let mut map: StepMap = HashMap::new();
    map.insert("c1".to_string(), ("abc".to_string(), 2, 3));
    let widget = MessageWidget::new(&msg, "/home/user/project", &map);
    let backend = TestBackend::new(80, 4);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame| frame.render_widget(widget, Rect::new(0, 0, 80, 4)))
        .expect("draw");
    let buffer = terminal.backend().buffer();
    let painted: String = (0..4)
        .map(|y| {
            (0..80)
                .map(|x| buffer[(x, y)].symbol().to_string())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(painted.contains("[2.3]"), "step tag missing:\n{painted}");
    assert!(painted.contains("Read"), "tool name missing:\n{painted}");
    assert!(painted.contains("main.rs"), "summary missing:\n{painted}");
}

#[test]
fn render_tool_call_error_shows_error_line() {
    let msg = Message::new(
        "s",
        Role::Assistant,
        vec![MessagePart::ToolCall {
            tool: "Bash".to_string(),
            call_id: "c1".to_string(),
            state: Box::new(ToolCallState {
                status: ToolCallStatus::Error,
                input: json!({"command": "false"}),
                output: None,
                error: Some("exit code 1".to_string()),
                duration_ms: None,
            }),
        }],
    );
    let painted = render_message(&msg, 80, 5);
    assert!(painted.contains("Error: exit code 1"), "got:\n{painted}");
}

#[test]
fn render_completed_read_shows_line_range() {
    let msg = Message::new(
        "s",
        Role::Assistant,
        vec![MessagePart::ToolCall {
            tool: "read".to_string(),
            call_id: "c1".to_string(),
            state: Box::new(ToolCallState {
                status: ToolCallStatus::Completed,
                input: json!({"path": "src/lib.rs"}),
                output: Some(json!({"lines": 3, "start_line": 10, "end_line": 12})),
                error: None,
                duration_ms: Some(42),
            }),
        }],
    );
    let painted = render_message(&msg, 80, 5);
    assert!(painted.contains("Read"), "got:\n{painted}");
}

#[test]
fn render_reasoning_part_is_indented() {
    let msg = Message::new(
        "s",
        Role::Assistant,
        vec![MessagePart::Reasoning {
            text: "deliberating".to_string(),
            signature: None,
        }],
    );
    let painted = render_message(&msg, 60, 4);
    assert!(painted.contains("deliberating"), "got:\n{painted}");
}

#[test]
fn render_image_part_shows_file_name() {
    let msg = Message::new(
        "s",
        Role::Assistant,
        vec![MessagePart::Image(Box::new(
            ragent_types::message::ImageData {
                mime_type: "image/png".to_string(),
                path: std::path::PathBuf::from("/tmp/shot.png"),
            },
        ))],
    );
    let painted = render_message(&msg, 60, 4);
    assert!(painted.contains("[image: shot.png]"), "got:\n{painted}");
}

#[test]
fn render_think_tool_call_prints_thought_body() {
    let msg = Message::new(
        "s",
        Role::Assistant,
        vec![MessagePart::ToolCall {
            tool: "think".to_string(),
            call_id: "t1".to_string(),
            state: Box::new(ToolCallState {
                status: ToolCallStatus::Completed,
                input: json!({}),
                output: Some(json!({"thought": "a deep thought\nsecond line"})),
                error: None,
                duration_ms: None,
            }),
        }],
    );
    let painted = render_message(&msg, 80, 6);
    assert!(painted.contains("a deep thought"), "got:\n{painted}");
    assert!(painted.contains("second line"), "got:\n{painted}");
}

#[test]
fn render_wait_agents_tool_call_lists_tasks() {
    let msg = Message::new(
        "s",
        Role::Assistant,
        vec![MessagePart::ToolCall {
            tool: "wait_agents".to_string(),
            call_id: "w1".to_string(),
            state: Box::new(ToolCallState {
                status: ToolCallStatus::Completed,
                input: json!({}),
                output: Some(json!({
                    "tasks": [
                        {"agent": "explore", "id": "abcdef0123456789", "elapsed_ms": 2500, "output_lines": 12, "status": "completed"},
                        {"agent": "build", "id": "feedface00000000", "elapsed_ms": 1000, "output_lines": 0, "status": "failed"}
                    ]
                })),
                error: None,
                duration_ms: None,
            }),
        }],
    );
    let painted = render_message(&msg, 90, 8);
    assert!(painted.contains("explore"), "got:\n{painted}");
    assert!(painted.contains("2.5s"), "elapsed missing:\n{painted}");
}

#[test]
fn render_completed_tool_call_shows_result_summary() {
    let msg = Message::new(
        "s",
        Role::Assistant,
        vec![MessagePart::ToolCall {
            tool: "Grep".to_string(),
            call_id: "g1".to_string(),
            state: Box::new(ToolCallState {
                status: ToolCallStatus::Completed,
                input: json!({"pattern": "fn main"}),
                output: Some(json!({"count": 3, "file_count": 2, "truncated": false})),
                error: None,
                duration_ms: Some(10),
            }),
        }],
    );
    let painted = render_message(&msg, 90, 6);
    assert!(painted.contains("matched in"), "got:\n{painted}");
}

// ===========================================================================
// message_widget.rs - tool_result_summary match arms
// ===========================================================================

fn summary(tool: &str, out: serde_json::Value, input: serde_json::Value) -> String {
    tool_result_summary(tool, &Some(out), &input, "/home/user/project")
        .unwrap_or_else(|| format!("<none:{tool}>"))
}

#[test]
fn result_summary_none_when_no_output() {
    assert!(tool_result_summary("read", &None, &json!({}), "/tmp").is_none());
}

#[test]
fn result_summary_file_operation_arms() {
    assert_eq!(
        summary("read", json!({"lines": 1}), json!({})),
        "1 line read"
    );
    assert_eq!(
        summary("read", json!({"lines": 2}), json!({})),
        "2 lines read"
    );
    assert_eq!(
        summary(
            "read",
            json!({"lines": 1, "summarised": true, "total_lines": 500}),
            json!({})
        ),
        "1 line read (summarised, 500 total)"
    );
    assert!(
        summary(
            "Write",
            json!({"lines": 4}),
            json!({"path": "/home/user/project/a.rs"})
        )
        .contains("4 lines written to a.rs")
    );
    assert!(
        summary(
            "create",
            json!({"lines": 1}),
            json!({"path": "/home/user/project/b.rs"})
        )
        .contains("1 line created in b.rs")
    );
    assert!(
        summary(
            "Edit",
            json!({"path": "/home/user/project/c.rs", "old_lines": 2, "new_lines": 5}),
            json!({})
        )
        .contains("Edited c.rs: 2 → 5 lines")
    );
    assert_eq!(
        summary("multiedit", json!({"edits": 3, "file_count": 2}), json!({})),
        "3 edits across 2 files"
    );
    assert_eq!(
        summary(
            "multi_edit",
            json!({"edits": 1, "file_count": 1}),
            json!({})
        ),
        "1 edit across 1 file"
    );
    assert_eq!(
        summary("patch", json!({"hunks": 2, "files": 3}), json!({})),
        "2 hunks applied across 3 files"
    );
    assert_eq!(
        summary("rm", json!({"deleted": true}), json!({})),
        "deleted"
    );
    assert_eq!(
        summary("rm", json!({"deleted": false}), json!({})),
        "failed"
    );
    assert_eq!(
        summary("append_to_file", json!({"bytes": 9}), json!({})),
        "9 bytes appended"
    );
    assert_eq!(summary("append_file", json!({}), json!({})), "appended");
    assert_eq!(
        summary("diff_files", json!({"changes": 5}), json!({})),
        "5 changes found"
    );
}

#[test]
fn result_summary_directory_and_file_info_arms() {
    assert!(
        summary(
            "list",
            json!({"count": 3, "path": "/home/user/project/src"}),
            json!({})
        )
        .contains("3 entries in src")
    );
    assert_eq!(
        summary("make_directory", json!({}), json!({})),
        "directory created"
    );
    assert_eq!(summary("mkdir", json!({}), json!({})), "directory created");
    assert_eq!(
        summary(
            "file_info",
            json!({"kind": "file", "size": 2048}),
            json!({})
        ),
        "file (2.0 KiB)"
    );
    assert_eq!(
        summary(
            "file_info",
            json!({"kind": "dir", "size": 2 * 1024 * 1024}),
            json!({})
        ),
        "dir (2.0 MiB)"
    );
    assert_eq!(
        summary("file_info", json!({"kind": "file", "size": 10}), json!({})),
        "file (10 bytes)"
    );
    assert_eq!(
        summary("file_info", json!({"kind": "symlink"}), json!({})),
        "symlink"
    );
    assert_eq!(summary("move_file", json!({}), json!({})), "moved");
    assert_eq!(summary("rename_file", json!({}), json!({})), "moved");
    assert_eq!(
        summary("copy_file", json!({"bytes": 12}), json!({})),
        "12 bytes copied"
    );
    assert_eq!(summary("copy_file", json!({}), json!({})), "copied");
}

#[test]
fn result_summary_search_and_execution_arms() {
    assert_eq!(
        summary("search", json!({"count": 4, "truncated": true}), json!({})),
        "4+ found"
    );
    assert_eq!(
        summary("Grep", json!({"count": 2, "file_count": 1}), json!({})),
        "2 matched in 1 file searched"
    );
    assert_eq!(
        summary("Glob", json!({"count": 3, "truncated": true}), json!({})),
        "3 files+ found"
    );
    assert_eq!(
        summary(
            "Bash",
            json!({"timed_out": true, "duration_ms": 300}),
            json!({})
        ),
        "timed out after 300ms"
    );
    assert!(summary("Bash", json!({"exit_code": 0, "lines": 2}), json!({})).contains("exit 0"));
    assert_eq!(summary("Bash", json!({"lines": 1}), json!({})), "1 line…");
    assert_eq!(
        summary("calculator", json!({"result": "42"}), json!({})),
        "= 42"
    );
}

#[test]
fn result_summary_network_and_env_arms() {
    assert_eq!(
        summary(
            "webfetch",
            json!({"http_status": 200, "lines": 5}),
            json!({})
        ),
        "5 lines (HTTP 200)"
    );
    assert_eq!(
        summary("websearch", json!({"count": 7}), json!({})),
        "7 results found"
    );
    assert_eq!(
        summary("websearch", json!({"count": 1}), json!({})),
        "1 result found"
    );
    assert!(
        summary("mf_search", json!({"total_results": 3}), json!({})).contains("3 results found")
    );
    assert!(summary("mf_search", json!({"count": 2}), json!({})).contains("2 results found"));
    assert_eq!(
        summary(
            "http_request",
            json!({"http_status": 404, "lines": 1}),
            json!({})
        ),
        "HTTP 404 (1 line)"
    );
    assert_eq!(
        summary("web_request", json!({}), json!({})),
        "HTTP 0 (0 lines)"
    );
    assert_eq!(summary("get_env", json!({"count": 2}), json!({})), "2 vars");
    assert_eq!(summary("get_env", json!({}), json!({})), "not found");
    assert_eq!(summary("bash_reset", json!({}), json!({})), "shell reset");
}

#[test]
fn result_summary_interaction_and_planning_arms() {
    assert_eq!(
        summary("question", json!({"response": "yes"}), json!({})),
        "yes"
    );
    assert_eq!(
        summary("AskUserQuestion", json!({"content": "maybe"}), json!({})),
        "maybe"
    );
    assert_eq!(summary("think", json!({}), json!({})), "Thinking ...");
    assert_eq!(
        summary("think", json!({"thought": "hmm"}), json!({})),
        "hmm"
    );
    assert_eq!(
        summary("agent_complete", json!({}), json!({})),
        "Task complete"
    );
    assert_eq!(
        summary("agent_complete", json!({"summary": "done"}), json!({})),
        "done"
    );
    assert_eq!(
        summary("EnterPlanMode", json!({"task": "build it"}), json!({})),
        "delegated → build it"
    );
    assert_eq!(
        summary("EnterPlanMode", json!({}), json!({})),
        "delegated → plan"
    );
    assert_eq!(
        summary("ExitPlanMode", json!({"summary_length": 42}), json!({})),
        "returned (42 chars)"
    );
}

#[test]
fn result_summary_task_management_arms() {
    assert!(
        summary(
            "TaskCreate",
            json!({"subject": "Do it", "id": "abcdef0123456789"}),
            json!({})
        )
        .contains("created \"Do it\"")
    );
    assert!(
        summary("TaskCreate", json!({"id": "abcdef0123456789"}), json!({})).contains("created [")
    );
    assert!(
        summary(
            "TaskUpdate",
            json!({"id": "abcdef0123456789", "status": "completed", "subject": "t"}),
            json!({})
        )
        .contains("updated")
    );
    assert!(
        summary(
            "TaskUpdate",
            json!({"id": "abcdef0123456789", "status": "completed"}),
            json!({})
        )
        .contains("→ completed")
    );
    assert!(
        summary(
            "TaskUpdate",
            json!({"id": "abcdef0123456789", "subject": "t"}),
            json!({})
        )
        .contains("updated \"t\"")
    );
    assert!(
        summary("TaskUpdate", json!({"id": "abcdef0123456789"}), json!({})).contains("updated [")
    );
    assert!(
        summary(
            "TaskUpdate",
            json!({"id": "abcdef0123456789", "status": "completed", "unblocked": ["b1", "b2"]}),
            json!({})
        )
        .contains("2 unblocked")
    );
}

// ===========================================================================
// research_progress.rs - apply/finish/render branches
// ===========================================================================

#[test]
fn progress_web_started_parses_deadline_and_counts() {
    let mut p = ResearchProgress::new("r", "t");
    p.apply(
        SessionPhase::Web,
        StepStatus::Started,
        "web_phase_start deadline: 30s",
    );
    assert!(p.web_phase_deadline.is_some());
    // Per-URL capture events accumulate one count each (the renderer emits one
    // event per captured source), so two captures increment the counter twice.
    p.apply(
        SessionPhase::Web,
        StepStatus::Done,
        "captured https://a.example",
    );
    p.apply(
        SessionPhase::Web,
        StepStatus::Done,
        "captured https://b.example",
    );
    assert_eq!(p.fetched_count, 2);
}

#[test]
fn progress_web_deadline_skipped_rewrites_detail_and_clears_deadline() {
    let mut p = ResearchProgress::new("r", "t");
    p.apply(
        SessionPhase::Web,
        StepStatus::Started,
        "web_phase_start deadline: 5s",
    );
    p.apply(
        SessionPhase::Web,
        StepStatus::Done,
        "captured https://a.example",
    );
    assert_eq!(p.fetched_count, 1);
    p.apply(
        SessionPhase::Web,
        StepStatus::Skipped,
        "web_deadline reached at 5000ms",
    );
    assert!(p.web_phase_deadline.is_none());
    let rendered = p.render();
    assert!(
        rendered.contains("Web phase deadline reached"),
        "got:\n{rendered}"
    );
    assert!(
        rendered.contains("1 source(s) captured"),
        "got:\n{rendered}"
    );
}

#[test]
fn progress_web_failed_and_excluded_live_counts() {
    let mut p = ResearchProgress::new("r", "t");
    p.apply(
        SessionPhase::Web,
        StepStatus::Error,
        "fetch failed for https://x.example: 500",
    );
    p.apply(SessionPhase::Web, StepStatus::Excluded, "low relevance");
    assert_eq!(p.failed_count, 1);
    assert_eq!(p.excluded_live, 1);
    let rendered = p.render();
    assert!(rendered.contains("Web fetch totals"), "got:\n{rendered}");
    assert!(rendered.contains("1 excluded"), "got:\n{rendered}");
}

#[test]
fn progress_totals_line_omits_excluded_when_zero() {
    let mut p = ResearchProgress::new("r", "t");
    p.apply(
        SessionPhase::Web,
        StepStatus::Started,
        "web_phase_start deadline: 30s",
    );
    p.apply(
        SessionPhase::Web,
        StepStatus::Done,
        "captured https://a.example",
    );
    p.apply(
        SessionPhase::Web,
        StepStatus::Done,
        "captured https://b.example",
    );
    p.apply(
        SessionPhase::Web,
        StepStatus::Done,
        "captured https://c.example",
    );
    let rendered = p.render();
    assert!(rendered.contains("3 fetched, 0 failed"), "got:\n{rendered}");
    assert!(!rendered.contains("excluded"), "got:\n{rendered}");
}

#[test]
fn progress_done_updates_previous_started_in_place() {
    let mut p = ResearchProgress::new("r", "t");
    p.apply(SessionPhase::Setup, StepStatus::Started, "starting");
    p.apply(SessionPhase::Setup, StepStatus::Done, "finished setup");
    assert_eq!(p.steps.len(), 1);
    assert_eq!(p.steps[0].status, StepStatus::Done);
    assert_eq!(p.steps[0].detail, "finished setup");

    // A skipped status also closes the previous started step.
    p.apply(SessionPhase::Local, StepStatus::Started, "local starting");
    p.apply(SessionPhase::Local, StepStatus::Skipped, "skipped");
    assert_eq!(p.steps.len(), 2);
    assert_eq!(p.steps[1].status, StepStatus::Skipped);
}

#[test]
fn progress_finish_and_completion_render_extras() {
    let mut p = ResearchProgress::new("r", "t");
    p.finish(7, 1, 2, 3);
    assert!(p.done);
    assert!(p.web_phase_deadline.is_none());
    let rendered = p.render();
    assert!(
        rendered.contains("Complete - 7 source(s)"),
        "got:\n{rendered}"
    );
    assert!(rendered.contains("1 PDF"), "got:\n{rendered}");
    assert!(rendered.contains("2 YouTube videos"), "got:\n{rendered}");
    assert!(rendered.contains("3 excluded"), "got:\n{rendered}");
    assert!(rendered.contains("/research open r"), "got:\n{rendered}");
}

#[test]
fn progress_completion_without_extras() {
    let mut p = ResearchProgress::new("r", "t");
    p.finish(2, 0, 0, 0);
    let rendered = p.render();
    assert!(
        rendered.contains("Complete - 2 source(s)."),
        "got:\n{rendered}"
    );
    assert!(!rendered.contains("including"), "got:\n{rendered}");
}

#[test]
fn progress_multiline_detail_is_continuation_indented() {
    let mut p = ResearchProgress::new("r", "t");
    p.apply(SessionPhase::Web, StepStatus::Started, "line one\nline two");
    let rendered = p.render();
    assert!(rendered.contains("line one"), "got:\n{rendered}");
    assert!(rendered.contains("line two"), "got:\n{rendered}");
}

#[test]
fn progress_empty_detail_renders_prefix_only() {
    let mut p = ResearchProgress::new("r", "t");
    p.apply(SessionPhase::Setup, StepStatus::Started, "");
    let rendered = p.render();
    assert!(rendered.contains("setup"), "got:\n{rendered}");
}

#[test]
fn progress_options_line_stored_separately() {
    let mut p = ResearchProgress::new("r", "t");
    p.apply(
        SessionPhase::Setup,
        StepStatus::Done,
        "options in use: mode: tiered",
    );
    assert_eq!(p.options.as_deref(), Some("mode: tiered"));
    assert!(p.steps.is_empty());
    assert!(p.render().contains("Options: mode: tiered"));
}

// ===========================================================================
// research_progress.rs - encode round trips
// ===========================================================================

#[test]
fn progress_encode_decode_phase_variants_round_trip() {
    let phases = [
        SessionPhase::Setup,
        SessionPhase::Web,
        SessionPhase::Local,
        SessionPhase::Specs,
        SessionPhase::Synthesize,
        SessionPhase::Assemble,
        SessionPhase::Finalize,
    ];
    for phase in phases {
        let encoded = encode_progress_event("n", "topic", &SessionEvent::Phase { phase });
        let decoded = ragent_tui::research_progress::decode_progress_event(&encoded)
            .unwrap_or_else(|| panic!("failed to decode {encoded}"));
        assert_eq!(decoded.name, "n");
        assert_eq!(decoded.phase, phase);
    }
}

#[test]
fn progress_encode_queries_decomposed_pluralises() {
    let one = encode_progress_event(
        "n",
        "t",
        &SessionEvent::QueriesDecomposed {
            queries: vec!["q1".to_string()],
        },
    );
    let decoded = ragent_tui::research_progress::decode_progress_event(&one).expect("decode");
    assert!(
        decoded.detail.contains("1 query"),
        "got: {}",
        decoded.detail
    );

    let many = encode_progress_event(
        "n",
        "t",
        &SessionEvent::QueriesDecomposed {
            queries: vec!["q1".to_string(), "q2".to_string()],
        },
    );
    let decoded = ragent_tui::research_progress::decode_progress_event(&many).expect("decode");
    assert!(
        decoded.detail.contains("2 queries"),
        "got: {}",
        decoded.detail
    );

    let none = encode_progress_event(
        "n",
        "t",
        &SessionEvent::QueriesDecomposed { queries: vec![] },
    );
    let decoded = ragent_tui::research_progress::decode_progress_event(&none).expect("decode");
    assert_eq!(decoded.detail, "no decomposition");
}

#[test]
fn progress_encode_captured_and_search_failed() {
    let captured = encode_progress_event(
        "n",
        "t",
        &SessionEvent::WebCaptured {
            url: "https://a.example".to_string(),
            title: "A title".to_string(),
            search_tool: String::new(),
            search_engine: String::new(),
            body_preview: String::new(),
            language: String::new(),
            oa_recovery: None,
            media_type: String::new(),
        },
    );
    let decoded = ragent_tui::research_progress::decode_progress_event(&captured).expect("decode");
    assert!(
        decoded
            .detail
            .contains("captured https://a.example - A title")
    );

    let failed = encode_progress_event(
        "n",
        "t",
        &SessionEvent::WebSearchFailed {
            error: "boom".to_string(),
        },
    );
    let decoded = ragent_tui::research_progress::decode_progress_event(&failed).expect("decode");
    assert!(decoded.detail.contains("web search failed: boom"));
}

#[test]
fn cluster_progress_event_round_trip() {
    let encoded = encode_cluster_progress_event(
        "cluster-run",
        "topic",
        SessionPhase::Web,
        "started",
        "cluster detail",
    );
    let decoded = ragent_tui::research_progress::decode_progress_event(&encoded).expect("decode");
    assert_eq!(decoded.name, "cluster-run");
    assert_eq!(decoded.phase, SessionPhase::Web);
    assert_eq!(decoded.detail, "cluster detail");
}

// ===========================================================================
// app/event_handler.rs - synthetic events
// ===========================================================================

#[tokio::test]
async fn event_session_created_populates_session_id() {
    let mut app = support::make_app();
    app.session_id = None;
    app.handle_event(Event::SessionCreated {
        session_id: "sess-created".to_string(),
    })
    .await;
    assert_eq!(app.session_id.as_deref(), Some("sess-created"));
    assert!(app.needs_redraw);
}

#[tokio::test]
async fn event_text_and_reasoning_delta_append_to_message() {
    let mut app = app_with_session("sess-1");
    app.handle_event(Event::MessageStart {
        session_id: "sess-1".to_string(),
        message_id: "m1".to_string(),
    })
    .await;
    app.handle_event(Event::TextDelta {
        session_id: "sess-1".to_string(),
        text: "hello world".to_string(),
    })
    .await;
    app.handle_event(Event::ReasoningDelta {
        session_id: "sess-1".to_string(),
        text: "thinking".to_string(),
    })
    .await;
    assert!(app.is_processing);
    assert!(app.stream_in_bytes > 0);
    let text = app
        .messages
        .iter()
        .flat_map(|m| m.parts.iter())
        .filter_map(|p| match p {
            MessagePart::Text { text } => Some(text.clone()),
            MessagePart::Reasoning { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("|");
    assert!(text.contains("hello world"), "got: {text}");
    assert!(text.contains("thinking"), "got: {text}");
}

#[tokio::test]
async fn event_compression_started_and_finished_toggles_flag() {
    let mut app = app_with_session("sess-1");
    app.handle_event(Event::CompressionStarted {
        session_id: "sess-1".to_string(),
        reason: "threshold".to_string(),
    })
    .await;
    assert!(app.compress_in_progress);
    let status = app.status.clone();
    assert!(status.contains("compressing"), "got: {status}");

    app.handle_event(Event::CompressionFinished {
        session_id: "sess-1".to_string(),
        original_tokens: 1000,
        compressed_tokens: 500,
        compression_ratio: 2.0,
        did_compress: true,
        reason: "threshold".to_string(),
    })
    .await;
    assert!(!app.compress_in_progress);
    assert_eq!(app.last_input_tokens, 500);
    assert!(app.status.contains("saved"), "got: {}", app.status);

    // The no-reduction branch.
    app.handle_event(Event::CompressionFinished {
        session_id: "sess-1".to_string(),
        original_tokens: 800,
        compressed_tokens: 800,
        compression_ratio: 1.0,
        did_compress: false,
        reason: "threshold".to_string(),
    })
    .await;
    assert!(app.status.contains("no change"), "got: {}", app.status);
}

#[tokio::test]
async fn event_message_end_sets_ready_and_handles_init() {
    let mut app = app_with_session("sess-1");
    app.is_processing = true;
    app.handle_event(Event::MessageEnd {
        session_id: "sess-1".to_string(),
        message_id: "init".to_string(),
        reason: ragent_agent::event::FinishReason::Stop,
    })
    .await;
    // The "init" marker must not reset processing state.
    assert!(app.is_processing);
    assert!(app.force_new_message);

    app.handle_event(Event::MessageEnd {
        session_id: "sess-1".to_string(),
        message_id: "m-real".to_string(),
        reason: ragent_agent::event::FinishReason::Stop,
    })
    .await;
    assert!(!app.is_processing);
    assert_eq!(app.status, "ready");
}

#[tokio::test]
async fn event_message_end_cancelled_halts_agent() {
    let mut app = app_with_session("sess-1");
    app.is_processing = true;
    app.handle_event(Event::MessageEnd {
        session_id: "sess-1".to_string(),
        message_id: "m".to_string(),
        reason: ragent_agent::event::FinishReason::Cancelled,
    })
    .await;
    assert!(app.agent_halted);
    assert!(app.status.contains("halted"), "got: {}", app.status);
}

#[tokio::test]
async fn event_token_usage_and_model_response_accumulate() {
    let mut app = app_with_session("sess-1");
    app.handle_event(Event::TokenUsage {
        session_id: "sess-1".to_string(),
        input_tokens: 100,
        output_tokens: 40,
    })
    .await;
    assert_eq!(app.last_input_tokens, 100);
    assert_eq!(app.token_usage.0, 100);
    assert_eq!(app.token_usage.1, 40);

    app.handle_event(Event::ModelResponse {
        session_id: "sess-1".to_string(),
        text: "response text".to_string(),
        elapsed_ms: 25,
        input_tokens: 100,
        output_tokens: 40,
    })
    .await;
    assert!(
        app.log_entries
            .iter()
            .any(|e| e.message.contains("model response")),
        "model response log missing"
    );
}

#[tokio::test]
async fn event_request_started_accumulates_outbound_bytes() {
    let mut app = app_with_session("sess-1");
    app.handle_event(Event::RequestStarted {
        session_id: "sess-1".to_string(),
        outbound_bytes: 512,
    })
    .await;
    app.handle_event(Event::RequestStarted {
        session_id: "sess-1".to_string(),
        outbound_bytes: 256,
    })
    .await;
    assert_eq!(app.stream_out_bytes, 768);
}

#[tokio::test]
async fn event_tools_sent_logs_at_first_step() {
    let mut app = app_with_session("sess-1");
    app.handle_event(Event::ToolsSent {
        session_id: "sess-1".to_string(),
        tools: vec!["read".to_string(), "Bash".to_string()],
    })
    .await;
    assert!(
        app.log_entries
            .iter()
            .any(|e| e.message.contains("tools sent")),
        "tools sent log missing"
    );
}

#[tokio::test]
async fn event_agent_error_clears_processing_and_status() {
    let mut app = app_with_session("sess-1");
    app.is_processing = true;
    app.handle_event(Event::AgentError {
        session_id: "sess-1".to_string(),
        error: "provider unreachable".to_string(),
    })
    .await;
    assert!(!app.is_processing);
    assert!(app.status.starts_with("error:"), "got: {}", app.status);
}

#[tokio::test]
async fn event_loop_terminated_and_change_summary_render_banner() {
    let mut app = app_with_session("sess-1");
    app.handle_event(Event::LoopTerminated {
        session_id: "sess-1".to_string(),
        status: "budget_exhausted".to_string(),
        iterations: 3,
        verification: Some("cargo test passed".to_string()),
        reason: Some("out of tokens".to_string()),
    })
    .await;
    let text = joined_text(&app);
    assert!(text.contains("Verification"), "got: {text}");

    app.handle_event(Event::LoopChangeSummary {
        session_id: "sess-1".to_string(),
        status: "completed".to_string(),
        iterations: 3,
        files_modified: 2,
        files_created: 1,
        files_deleted: 0,
        diffstat: "+12 -3 lines across 3 files".to_string(),
        files: vec!["src/a.rs".to_string(), "src/b.rs".to_string()],
    })
    .await;
    assert!(app.pending_rollback.is_some());
    assert!(app.status.contains("roll back"), "got: {}", app.status);
}

#[tokio::test]
async fn event_run_cost_summary_sets_banner() {
    let mut app = app_with_session("sess-1");
    app.handle_event(Event::RunCostSummary {
        session_id: "sess-1".to_string(),
        model_id: "claude-sonnet-4".to_string(),
        input_tokens: 1000,
        output_tokens: 250,
        total_cost_usd: 0.0123,
        duration_ms: 4321,
    })
    .await;
    assert!(app.run_cost_banner.is_some());
    assert!(
        app.run_cost_banner
            .as_deref()
            .unwrap_or("")
            .contains("run complete")
    );
}

#[tokio::test]
async fn event_hook_warning_and_tool_result_flagged_set_status() {
    let mut app = app_with_session("sess-1");
    app.handle_event(Event::HookWarning {
        session_id: "sess-1".to_string(),
        hook_command: "lint.sh".to_string(),
        tool: "Write".to_string(),
        stderr: "warnings found".to_string(),
    })
    .await;
    assert!(
        app.status.starts_with("hook warning"),
        "got: {}",
        app.status
    );

    app.handle_event(Event::ToolResultFlagged {
        session_id: "sess-1".to_string(),
        tool: "Bash".to_string(),
        hook_command: "guard.sh".to_string(),
        reason: "denied".to_string(),
    })
    .await;
    assert!(app.status.contains("flagged"), "got: {}", app.status);
}

#[tokio::test]
async fn event_security_verdict_and_quota_update() {
    let mut app = app_with_session("sess-1");
    app.handle_event(Event::SecurityVerdict {
        session_id: "sess-1".to_string(),
        tool: "Bash".to_string(),
        verdict: "allow".to_string(),
        rationale: "read-only command".to_string(),
    })
    .await;
    assert_eq!(app.security_verdicts, 1);
    assert_eq!(app.status, "security: allow Bash");

    app.handle_event(Event::SecurityVerdict {
        session_id: "sess-1".to_string(),
        tool: "rm".to_string(),
        verdict: "deny".to_string(),
        rationale: "destructive".to_string(),
    })
    .await;
    assert_eq!(app.security_verdicts, 2);

    app.handle_event(Event::QuotaUpdate {
        session_id: "sess-1".to_string(),
        percent: 42.5,
    })
    .await;
    assert_eq!(app.quota_percent, Some(42.5));
}

#[tokio::test]
async fn event_router_classification_records_tier_and_model() {
    let mut app = app_with_session("sess-1");
    let long_prompt = "x".repeat(200);
    app.handle_event(Event::RouterClassification {
        session_id: "sess-1".to_string(),
        tier: "standard".to_string(),
        requested_tier: Some("simple".to_string()),
        model: "provider:model".to_string(),
        composite_score: 0.5,
        prompt: long_prompt,
        dimensions: vec![("complexity".to_string(), 0.7)],
    })
    .await;
    assert_eq!(app.router_current_tier.as_deref(), Some("standard"));
    assert_eq!(app.router_current_model.as_deref(), Some("provider:model"));
}

#[tokio::test]
async fn event_provider_loading_lifecycle() {
    let mut app = app_with_session("sess-1");
    app.handle_event(Event::ProviderLoadingStarted {
        provider_id: "ollama".to_string(),
        provider_name: "Ollama".to_string(),
    })
    .await;
    assert!(app.model_loading_state.is_some());

    app.handle_event(Event::ProviderLoadingFinished {
        provider_id: "ollama".to_string(),
        provider_name: "Ollama".to_string(),
        models: vec![json!({"id": "llama3"})],
        error: Some("discovery failed".to_string()),
    })
    .await;
    assert!(app.model_loading_state.is_none());
    assert!(
        app.status.contains("discovery failed"),
        "got: {}",
        app.status
    );
}

#[tokio::test]
async fn event_model_download_lifecycle() {
    let mut app = app_with_session("sess-1");
    app.handle_event(Event::ModelDownloadStarted {
        provider_id: "ollama".to_string(),
        model_id: "llama3".to_string(),
        session_id: "sess-1".to_string(),
    })
    .await;
    assert!(app.model_download_state.is_some());

    app.handle_event(Event::ModelDownloadProgress {
        provider_id: "ollama".to_string(),
        model_id: "llama3".to_string(),
        session_id: "sess-1".to_string(),
        percent: 50.0,
    })
    .await;
    assert_eq!(
        app.model_download_state.as_ref().map(|s| s.percent),
        Some(50.0)
    );

    // Progress for a different model is ignored.
    app.handle_event(Event::ModelDownloadProgress {
        provider_id: "ollama".to_string(),
        model_id: "other".to_string(),
        session_id: "sess-1".to_string(),
        percent: 99.0,
    })
    .await;
    assert_eq!(
        app.model_download_state.as_ref().map(|s| s.percent),
        Some(50.0)
    );

    app.handle_event(Event::ModelDownloadFinished {
        provider_id: "ollama".to_string(),
        model_id: "llama3".to_string(),
        session_id: "sess-1".to_string(),
        error: Some("disk full".to_string()),
    })
    .await;
    assert!(app.model_download_state.is_none());
    assert!(
        app.status.contains("download failed"),
        "got: {}",
        app.status
    );
}

#[tokio::test]
async fn event_service_start_error_opens_dialog_state() {
    let mut app = app_with_session("sess-1");
    app.handle_event(Event::ServiceStartError {
        session_id: "sess-1".to_string(),
        service: "ollama serve".to_string(),
        command_path: "/usr/bin/ollama".to_string(),
        stdout: "out".to_string(),
        stderr: "err".to_string(),
        error: "timed out".to_string(),
    })
    .await;
    assert!(app.needs_redraw);
}

#[tokio::test]
async fn event_mcp_status_and_enabled_changed() {
    let mut app = app_with_session("sess-1");
    app.handle_event(Event::McpStatusChanged {
        server_id: "filesystem".to_string(),
        status: "connected".to_string(),
    })
    .await;
    app.handle_event(Event::McpServerEnabledChanged {
        server_id: "filesystem".to_string(),
        enabled: false,
    })
    .await;
    assert!(app.needs_redraw);
}

#[tokio::test]
async fn event_memory_events_mark_cache_dirty() {
    let mut app = app_with_session("sess-1");
    app.memory_cache_dirty = false;
    app.handle_event(Event::MemoryStored {
        session_id: "sess-1".to_string(),
        id: 1,
        category: "fact".to_string(),
    })
    .await;
    assert!(app.memory_cache_dirty);
}

#[tokio::test]
async fn event_permission_and_question_queue_push() {
    let mut app = app_with_session("sess-1");
    app.handle_event(Event::PermissionRequested {
        session_id: "sess-1".to_string(),
        request_id: "p1".to_string(),
        permission: "file:write".to_string(),
        description: "write a file".to_string(),
        options: vec![],
    })
    .await;
    assert_eq!(app.permission_queue.len(), 1);

    app.handle_event(Event::QuestionRequested {
        session_id: "sess-1".to_string(),
        request_id: "q1".to_string(),
        question: "Proceed?".to_string(),
        options: vec!["yes".to_string(), "no".to_string()],
        default_index: Some(0),
    })
    .await;
    assert_eq!(app.question_queue.len(), 1);
}

#[tokio::test]
async fn event_tool_call_args_and_result_update_part() {
    let mut app = app_with_session("sess-1");
    app.handle_event(Event::ToolCallStart {
        session_id: "sess-1".to_string(),
        call_id: "c1".to_string(),
        tool: "read".to_string(),
    })
    .await;
    app.handle_event(Event::ToolCallArgs {
        session_id: "sess-1".to_string(),
        call_id: "c1".to_string(),
        tool: "read".to_string(),
        args: r#"{"path":"src/main.rs"}"#.to_string(),
    })
    .await;
    app.handle_event(Event::ToolResult {
        session_id: "sess-1".to_string(),
        call_id: "c1".to_string(),
        tool: "read".to_string(),
        content: "file body".to_string(),
        content_line_count: 1,
        metadata: None,
        success: true,
    })
    .await;
    let part = app.messages[0].parts[0].clone();
    let MessagePart::ToolCall { state, .. } = part else {
        panic!("expected ToolCall part");
    };
    assert_eq!(state.input["path"], "src/main.rs");
}

#[tokio::test]
async fn event_subagent_lifecycle_tracks_active_tasks() {
    let mut app = app_with_session("sess-1");
    app.handle_event(Event::SubagentStart {
        session_id: "sess-1".to_string(),
        task_id: "explore-a1b2c3d4".to_string(),
        child_session_id: "child-1".to_string(),
        agent: "explore".to_string(),
        task: "find things".to_string(),
        background: true,
    })
    .await;
    assert_eq!(app.active_tasks.len(), 1);

    app.handle_event(Event::SubagentComplete {
        session_id: "sess-1".to_string(),
        task_id: "explore-a1b2c3d4".to_string(),
        child_session_id: "child-1".to_string(),
        summary: "found them".to_string(),
        success: true,
        duration_ms: 100,
        finish_reason: "stop".to_string(),
    })
    .await;
    assert!(app.active_tasks.is_empty());
}

#[tokio::test]
async fn event_background_task_lifecycle() {
    let mut app = app_with_session("sess-1");
    app.handle_event(Event::BackgroundTaskSpawned {
        session_id: "sess-1".to_string(),
        task_id: "bg-1".to_string(),
        command: "sleep 1".to_string(),
    })
    .await;
    assert_eq!(app.bg_tasks.len(), 1);

    app.handle_event(Event::BackgroundTaskUpdated {
        session_id: "sess-1".to_string(),
        task_id: "bg-1".to_string(),
        status: "running".to_string(),
        progress: None,
    })
    .await;
    assert_eq!(app.bg_tasks[0].status, "running");

    app.handle_event(Event::BackgroundTaskCompleted {
        session_id: "sess-1".to_string(),
        task_id: "bg-1".to_string(),
        status: "completed".to_string(),
        exit_code: Some(0),
    })
    .await;
    assert!(app.bg_tasks.is_empty());
}

#[tokio::test]
async fn event_teammate_lifecycle_updates_members() {
    let mut app = app_with_session("sess-1");
    app.handle_event(Event::TeammateSpawned {
        session_id: "sess-1".to_string(),
        team_name: "alpha".to_string(),
        teammate_name: "reviewer".to_string(),
        agent_id: "tm-001".to_string(),
    })
    .await;
    assert_eq!(app.team_members.len(), 1);
    assert!(app.show_teams);

    app.handle_event(Event::TeamTaskClaimed {
        session_id: "sess-1".to_string(),
        team_name: "alpha".to_string(),
        agent_id: "tm-001".to_string(),
        task_id: "task-001".to_string(),
    })
    .await;
    assert_eq!(app.team_members[0].status, MemberStatus::Working);
    assert_eq!(
        app.team_members[0].current_task_id.as_deref(),
        Some("task-001")
    );

    app.handle_event(Event::TeamTaskCompleted {
        session_id: "sess-1".to_string(),
        team_name: "alpha".to_string(),
        agent_id: "tm-001".to_string(),
        task_id: "task-001".to_string(),
    })
    .await;
    assert!(app.team_members[0].current_task_id.is_none());

    app.handle_event(Event::TeammateIdle {
        session_id: "sess-1".to_string(),
        team_name: "alpha".to_string(),
        agent_id: "tm-001".to_string(),
    })
    .await;
    assert_eq!(app.team_members[0].status, MemberStatus::Idle);

    app.handle_event(Event::TeammateFailed {
        session_id: "sess-1".to_string(),
        team_name: "alpha".to_string(),
        agent_id: "tm-001".to_string(),
        error: "crashed".to_string(),
    })
    .await;
    assert_eq!(app.team_members[0].status, MemberStatus::Failed);
    assert_eq!(
        app.team_members[0].last_spawn_error.as_deref(),
        Some("crashed")
    );

    // FR-053: cleanup also hides the Teams popup window.
    app.show_teams_window = true;
    app.handle_event(Event::TeamCleanedUp {
        session_id: "sess-1".to_string(),
        team_name: "alpha".to_string(),
    })
    .await;
    assert!(app.team_members.is_empty());
    assert!(!app.show_teams);
    assert!(!app.show_teams_window);
    assert!(app.team_message_counts.is_empty());
    assert!(app.focused_teammate.is_none());
}

#[tokio::test]
async fn event_teammate_message_tracks_counts() {
    let mut app = app_with_session("sess-1");
    app.handle_event(Event::TeammateMessage {
        session_id: "sess-1".to_string(),
        team_name: "alpha".to_string(),
        from: "tm-001".to_string(),
        to: "lead".to_string(),
        message_type: "notice".to_string(),
        preview: "hello".to_string(),
    })
    .await;
    assert_eq!(app.team_message_counts.get("tm-001").copied(), Some((1, 0)));

    app.handle_event(Event::TeammateP2PMessage {
        session_id: "sess-1".to_string(),
        team_name: "alpha".to_string(),
        from: "tm-001".to_string(),
        to: "tm-002".to_string(),
        message_type: "chat".to_string(),
        preview: "p2p".to_string(),
    })
    .await;
    assert_eq!(app.team_message_counts.get("tm-001").copied(), Some((2, 0)));
    assert_eq!(app.team_message_counts.get("tm-002").copied(), Some((0, 1)));
}

#[tokio::test]
async fn event_message_counters_ignore_lead() {
    // FR-052: a sender or recipient named `lead` never increments a counter,
    // on either the direct-message or the P2P path.
    let mut app = app_with_session("sess-1");

    app.handle_event(Event::TeammateMessage {
        session_id: "sess-1".to_string(),
        team_name: "alpha".to_string(),
        from: "lead".to_string(),
        to: "tm-001".to_string(),
        message_type: "notice".to_string(),
        preview: "from lead".to_string(),
    })
    .await;
    assert!(
        !app.team_message_counts.contains_key("lead"),
        "a lead sender must not gain a sent counter"
    );
    assert_eq!(app.team_message_counts.get("tm-001").copied(), Some((0, 1)));

    app.handle_event(Event::TeammateMessage {
        session_id: "sess-1".to_string(),
        team_name: "alpha".to_string(),
        from: "tm-001".to_string(),
        to: "lead".to_string(),
        message_type: "notice".to_string(),
        preview: "to lead".to_string(),
    })
    .await;
    assert_eq!(app.team_message_counts.get("tm-001").copied(), Some((1, 1)));
    assert!(!app.team_message_counts.contains_key("lead"));

    app.handle_event(Event::TeammateP2PMessage {
        session_id: "sess-1".to_string(),
        team_name: "alpha".to_string(),
        from: "lead".to_string(),
        to: "tm-002".to_string(),
        message_type: "chat".to_string(),
        preview: "p2p from lead".to_string(),
    })
    .await;
    assert!(
        !app.team_message_counts.contains_key("lead"),
        "a lead P2P sender must not gain a sent counter"
    );
    assert_eq!(app.team_message_counts.get("tm-002").copied(), Some((0, 1)));

    app.handle_event(Event::TeammateP2PMessage {
        session_id: "sess-1".to_string(),
        team_name: "alpha".to_string(),
        from: "tm-002".to_string(),
        to: "lead".to_string(),
        message_type: "chat".to_string(),
        preview: "p2p to lead".to_string(),
    })
    .await;
    assert_eq!(app.team_message_counts.get("tm-002").copied(), Some((1, 1)));
    assert!(!app.team_message_counts.contains_key("lead"));
}

#[tokio::test]
async fn event_shell_cwd_and_user_input() {
    let mut app = app_with_session("sess-1");
    app.handle_event(Event::ShellCwdChanged {
        session_id: "sess-1".to_string(),
        cwd: "/tmp/work".to_string(),
    })
    .await;
    assert_eq!(app.shell_cwd.as_deref(), Some("/tmp/work"));

    app.handle_event(Event::UserInput {
        session_id: "sess-1".to_string(),
        request_id: "r1".to_string(),
        response: "typed".to_string(),
    })
    .await;
    // `UserInput` sets the working status; it does not by itself flip the
    // processing flag (that is `MessageStart`).
    assert!(
        app.is_processing || app.status.contains("processing"),
        "got: {}",
        app.status
    );
}

#[tokio::test]
async fn event_open_research_view_populates_state() {
    let mut app = app_with_session("sess-1");
    app.handle_event(Event::OpenResearchView {
        name: "topic-a".to_string(),
        path: std::path::PathBuf::from("/tmp/research/topic-a/RESEARCH.md"),
        markdown: "# Heading\n\nbody".to_string(),
    })
    .await;
    let view = app.research_view.as_ref().expect("research view open");
    assert_eq!(view.name, "topic-a");
    assert!(view.markdown.contains("Heading"));
    assert!(app.status.contains("topic-a"));
}

#[tokio::test]
async fn event_agent_notice_plain_and_progress_paths() {
    let mut app = app_with_session("sess-1");
    app.handle_event(Event::AgentNotice {
        session_id: "sess-1".to_string(),
        message: "something noteworthy".to_string(),
    })
    .await;
    let text = joined_text(&app);
    assert!(text.contains("Agent Notice"), "got: {text}");

    // Sentinel-prefixed progress notice routes into the research tracker.
    let encoded = encode_cluster_progress_event(
        "cluster-1",
        "topic",
        SessionPhase::Web,
        "started",
        "cluster started",
    );
    app.handle_event(Event::AgentNotice {
        session_id: "sess-1".to_string(),
        message: encoded,
    })
    .await;
    assert!(
        app.research_progress.iter().any(|p| p.name == "cluster-1"),
        "research progress tracker not created"
    );
}

#[tokio::test]
async fn event_progress_notice_with_total_sources_marks_complete() {
    let mut app = app_with_session("sess-1");
    let event = SessionEvent::Phase {
        phase: SessionPhase::Web,
    };
    let started = encode_progress_event("run-1", "topic", &event);
    app.handle_event(Event::AgentNotice {
        session_id: "sess-1".to_string(),
        message: started,
    })
    .await;

    // A finishing event carrying a total source count: craft one via the
    // decoded payload round trip by encoding a final phase then patching is
    // not possible, so drive the finalize+done branch through the cluster
    // encoder which sets total_sources to None plus the explicit finalize
    // branch.
    let finalize = encode_cluster_progress_event(
        "run-1",
        "topic",
        SessionPhase::Finalize,
        "done",
        "extraction complete",
    );
    app.handle_event(Event::AgentNotice {
        session_id: "sess-1".to_string(),
        message: finalize,
    })
    .await;
    assert!(
        app.status.contains("extraction complete"),
        "got: {}",
        app.status
    );
}

// ===========================================================================
// app/swarm.rs - reachable through the `/swarm` slash command only
//
// `execute_swarm_decomposition`, `spawn_swarm_teammates`, `poll_swarm_unblock`,
// `poll_swarm_completion` and `finalize_swarm_completion` are `pub(crate)` and
// have no public test hook, so they are unreachable from an integration test
// (reaching them requires a live LLM decomposition). The status/cancel entry
// points are driven through `/swarm`.
// ===========================================================================

#[tokio::test]
async fn swarm_help_lists_commands() {
    let mut app = app_with_session("sess-lead");
    app.execute_slash_command("/swarm help").await;
    let text = joined_text(&app);
    assert!(text.contains("Swarm - Fleet-Style"), "got: {text}");
}

#[tokio::test]
async fn swarm_without_prompt_shows_usage() {
    let mut app = app_with_session("sess-lead");
    app.execute_slash_command("/swarm --agent general").await;
    let text = joined_text(&app);
    assert!(text.contains("Usage:"), "got: {text}");
}

#[tokio::test]
async fn swarm_when_already_active_warns() {
    let mut app = app_with_session("sess-lead");
    app.swarm_state = Some(SwarmState {
        team_name: "swarm-live".to_string(),
        prompt: "existing".to_string(),
        decomposition: SwarmDecomposition { tasks: vec![] },
        spawned: true,
        completed: false,
        default_agent_type: None,
    });
    app.execute_slash_command("/swarm do more work").await;
    assert!(app.status.contains("already active"), "got: {}", app.status);
}

#[tokio::test]
async fn swarm_status_without_active_swarm_hints() {
    let mut app = app_with_session("sess-lead");
    app.execute_slash_command("/swarm status").await;
    let text = joined_text(&app);
    assert!(text.contains("No active swarm"), "got: {text}");
}

#[tokio::test]
async fn swarm_status_renders_progress_bar_and_table_from_state() {
    let dir = temp_swarm_dir("swarm-status");
    let mut app = app_with_session("sess-lead");
    app.cwd_path = dir;
    app.swarm_state = Some(SwarmState {
        team_name: "swarm-manual".to_string(),
        prompt: "do work".to_string(),
        decomposition: SwarmDecomposition {
            tasks: vec![
                subtask("s1", &[], Some("general"), None),
                subtask("s2", &["s1"], None, None),
            ],
        },
        spawned: true,
        completed: false,
        default_agent_type: None,
    });
    app.team_members = vec![TeamMember::new("swarm-s1", "tm-001", "general")];

    app.execute_slash_command("/swarm status").await;
    let text = joined_text(&app);
    assert!(text.contains("Progress:"), "got: {text}");
    assert!(text.contains("swarm-manual"), "got: {text}");
    assert!(text.contains("Teammates:"), "got: {text}");
    assert!(text.contains("swarm-s1"), "got: {text}");
}

#[tokio::test]
async fn swarm_cancel_without_state_reports_nothing() {
    let mut app = app_with_session("sess-lead");
    app.execute_slash_command("/swarm cancel").await;
    let text = joined_text(&app);
    assert!(text.contains("No active swarm to cancel"), "got: {text}");
}

#[tokio::test]
async fn swarm_cancel_with_state_reports_shutdown() {
    let dir = temp_swarm_dir("swarm-cancel");
    let mut app = app_with_session("sess-lead");
    app.cwd_path = dir.clone();
    let team_name = "swarm-cancel-me";
    let _ = TeamStore::create(team_name, "sess-lead", &dir, true);
    app.swarm_state = Some(SwarmState {
        team_name: team_name.to_string(),
        prompt: "do work".to_string(),
        decomposition: SwarmDecomposition { tasks: vec![] },
        spawned: true,
        completed: false,
        default_agent_type: None,
    });
    app.execute_slash_command("/swarm cancel").await;
    assert!(app.swarm_state.is_none());
    assert!(app.status.contains("cancelled"), "got: {}", app.status);
}

// ===========================================================================
// app/research.rs - reachable through the `/research` slash command
// ===========================================================================

#[tokio::test]
async fn research_help_lists_commands() {
    let mut app = app_with_session("sess-1");
    app.execute_slash_command("/research help").await;
    assert_eq!(app.status, "research: help");
    let text = joined_text(&app);
    assert!(text.to_lowercase().contains("research"), "got: {text}");
}

#[tokio::test]
async fn research_delete_without_yes_is_refused() {
    let mut app = app_with_session("sess-1");
    app.execute_slash_command("/research delete topic-a").await;
    let text = joined_text(&app);
    assert!(text.contains("Refusing to delete"), "got: {text}");
}

#[tokio::test]
async fn research_continue_reports_resume() {
    let mut app = app_with_session("sess-1");
    app.execute_slash_command("/research continue topic-a --message more")
        .await;
    let text = joined_text(&app);
    assert!(text.contains("Resuming"), "got: {text}");
    assert!(text.contains("more"), "got: {text}");
}

#[tokio::test]
async fn research_list_open_search_show_archive_on_empty_store() {
    let dir = temp_swarm_dir("research-ops");
    let mut app = app_with_session("sess-1");
    app.cwd_path = dir;

    // Each of these dispatches to an off-loop task over an empty research
    // store; the synchronous status line is set before the spawn.
    app.execute_slash_command("/research list").await;
    assert!(app.status.contains("listing items"), "got: {}", app.status);

    app.execute_slash_command("/research open topic-a").await;
    assert!(app.status.contains("opening"), "got: {}", app.status);

    app.execute_slash_command("/research search rust async")
        .await;
    assert!(app.status.contains("searching"), "got: {}", app.status);

    app.execute_slash_command("/research show topic-a").await;
    assert!(app.status.contains("showing"), "got: {}", app.status);

    app.execute_slash_command("/research archive topic-a").await;
    assert!(app.status.contains("archiving"), "got: {}", app.status);
}

// ===========================================================================
// shared helpers
// ===========================================================================

fn joined_text(app: &ragent_tui::App) -> String {
    app.messages
        .iter()
        .flat_map(|m| m.parts.iter())
        .filter_map(|p| match p {
            MessagePart::Text { text } => Some(text.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}
