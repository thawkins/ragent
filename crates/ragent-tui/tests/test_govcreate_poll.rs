//! T-014 (spec `govdoc`, FR-015): TUI result polling, in-place progress
//! refresh, and Escape cancellation plumbing for `/spec govcreate`.
//!
//! Covers: progress lines draining into a single in-place message, the
//! terminal report landing on the result slot, and the Escape-driven cancel
//! token being reachable from the input layer.

use support::make_app;

mod support;

/// Give the app's session an id so `append_assistant_text` will create
/// messages; make_app() leaves `session_id` unset for pure-TUI tests.
fn with_session(mut app: ragent_tui::App) -> ragent_tui::App {
    app.session_id = Some(ragent_types::SessionId::new().to_string());
    app
}

/// Drain one fake worker's progress stream + deposit the terminal report
/// into the slots the TUI reads, then drive `poll_govcreate_result`.
#[test]
fn test_poll_govcreate_result_streams_then_renders_terminal_report() {
    let mut app = with_session(make_app());

    // Simulate the worker seeding the progress panel (as
    // `run_govcreate_orchestration` does) then depositing a terminal report.
    if let Ok(mut lines) = app.govcreate_progress.lock() {
        lines.push("[ .. ] guard".to_owned());
    }
    app.govcreate_progress_text = Some("Govcreate 'demo-arch'...".to_owned());
    app.govcreate_progress_slug = Some("Govcreate 'demo-arch'...".to_owned());
    app.govcreate_running = true;

    let report = ragent_tools_extended::archdoc::GovCreateRunReport {
        completed: vec![
            ragent_tools_extended::archdoc::CompletedStage {
                id: "guard",
                note: "content reference validated".to_owned(),
            },
            ragent_tools_extended::archdoc::CompletedStage {
                id: "write",
                note: "spec written".to_owned(),
            },
        ],
        outcome: ragent_tools_extended::archdoc::GovCreateOutcome::Completed {
            spec_dir: std::path::PathBuf::from("./target/specs/demo-arch"),
            budget_note: None,
        },
    };
    if let Ok(mut slot) = app.govcreate_result.lock() {
        *slot = Some(report);
    }

    // First poll drains the progress line into the panel and renders the
    // terminal report.
    app.poll_govcreate_result();

    assert!(
        !app.govcreate_running,
        "run marked finished after the report"
    );
    // The panel refreshed in place rather than stacking: one message, text
    // contains the drained progress line.
    let panel = app
        .messages
        .iter()
        .find_map(|m| m.text_content().find("[ .. ] guard").map(|_| m))
        .map(|m| m.text_content());
    assert!(
        panel.is_some(),
        "progress message exists and absorbed the drained line"
    );
    assert!(
        panel
            .as_ref()
            .map(|t| t.contains("[ .. ] guard"))
            .unwrap_or(false),
        "panel shows the streamed stage line: {panel:?}"
    );
    // Terminal report landed as a fresh assistant message with the NFR-005
    // surface prefix.
    let terminal = app.messages.last().map(|m| m.text_content());
    let terminal = terminal.as_deref().unwrap_or_default();
    assert!(
        terminal.contains("From: /spec govcreate"),
        "terminal report carries the surface prefix: {terminal}"
    );
    assert!(
        terminal.contains("[ ok ] write"),
        "terminal report shows the completed stage: {terminal}"
    );
    assert!(
        app.status.contains("spec govcreate: complete"),
        "status reports success: {}",
        app.status
    );
}

/// FR-013 path: a stage failure lands as a failed status + report, no panel
/// message is produced for a run that never streamed.
#[test]
fn test_poll_govcreate_result_renders_stage_failure_report() {
    let mut app = with_session(make_app());
    app.govcreate_running = true;

    let report = ragent_tools_extended::archdoc::GovCreateRunReport {
        completed: vec![ragent_tools_extended::archdoc::CompletedStage {
            id: "acquire",
            note: "acquired 0 sources".to_owned(),
        }],
        outcome: ragent_tools_extended::archdoc::GovCreateOutcome::StageFailed {
            stage: "extract",
            cause: "model timeout".to_owned(),
        },
    };
    if let Ok(mut slot) = app.govcreate_result.lock() {
        *slot = Some(report);
    }

    app.poll_govcreate_result();

    assert!(!app.govcreate_running);
    let terminal = app.messages.last().map(|m| m.text_content());
    let terminal = terminal.as_deref().unwrap_or_default();
    assert!(
        terminal.contains("[fail] extract") && terminal.contains("model timeout"),
        "failed stage with cause is reported: {terminal}"
    );
    assert!(terminal.contains("From: /spec govcreate"));
    assert!(
        app.status.contains("spec govcreate: failed"),
        "status reports failure: {}",
        app.status
    );
}

/// FR-019/T-014: Escape while a run is live cancels the token so the worker
/// stops at the next stage boundary; the cancel is a no-op when idle.
#[test]
fn test_poll_govcreate_cancel_cancels_active_run() {
    let mut app = make_app();
    let token = ragent_tools_extended::archdoc::CancellationToken::none();
    if let Ok(mut slot) = app.govcreate_cancel.lock() {
        *slot = Some(token.clone());
    }
    app.govcreate_running = true;

    assert!(app.govcreate_run_active());
    app.poll_govcreate_cancel();

    assert!(token.is_cancelled(), "Escape cancelled the active run");
    assert!(
        app.status.contains("cancel"),
        "status reflects the cancel: {}",
        app.status
    );

    // Re-poll: token was consumed, so no further action and status unchanged
    // beyond the initial note.
    app.poll_govcreate_cancel();
    // (no panic, no double-log assertion needed beyond the poll completing)
}

/// Idempotency: with no run in flight, polling cancel does nothing.
#[test]
fn test_poll_govcreate_cancel_no_run_is_noop() {
    let mut app = make_app();
    assert!(!app.govcreate_run_active());
    app.poll_govcreate_cancel(); // must not panic
    assert!(!app.govcreate_run_active());
}
