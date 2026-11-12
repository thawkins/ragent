//! Tests for the `/research` run lifecycle in the TUI: a new run under a name
//! already tracked gets its own tracker and message, and a failed run ends its
//! tracker and leaves the `[wait]` status behind.

use ragent_agent::event::Event;
use ragent_research::deck_progress::{DeckEvent, DeckStage};
use ragent_tui::app::App;
use ragent_tui::deck_progress::DeckTracker;
use ragent_tui::research_progress::{
    ResearchProgress, SessionPhase, encode_cluster_progress_event,
};

#[path = "support/mod.rs"]
mod support;

/// The first text part of every assistant message in the window.
fn assistant_texts(app: &App) -> Vec<String> {
    app.session
        .messages
        .iter()
        .filter(|message| message.role == ragent_types::message::Role::Assistant)
        .filter_map(|message| message.parts.first())
        .filter_map(|part| match part {
            ragent_types::message::MessagePart::Text { text } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

fn app_with_session() -> App {
    let mut app = support::make_app();
    app.session.session_id = Some("s1".to_string());
    app
}

#[test]
fn test_second_run_with_same_name_gets_its_own_tracker_and_message() {
    let mut app = app_with_session();
    app.begin_research_progress(ResearchProgress::new("item", "first topic"));
    app.begin_research_progress(ResearchProgress::new("item", "second topic"));

    let trackers: Vec<&ResearchProgress> = app
        .research_progress
        .iter()
        .filter(|p| p.name == "item")
        .collect();
    assert_eq!(trackers.len(), 1, "the superseded tracker is dropped");
    assert_eq!(trackers[0].topic, "second topic");

    let texts = assistant_texts(&app);
    let live = texts
        .iter()
        .filter(|t| t.lines().next() == Some("[research] Research Progress - `item`"))
        .count();
    let retired = texts
        .iter()
        .filter(|t| t.lines().next() == Some("[research] Research Progress - `item` (earlier run)"))
        .count();
    assert_eq!(live, 1, "one live message; window: {texts:?}");
    assert_eq!(
        retired, 1,
        "the earlier run stays visible; window: {texts:?}"
    );
    assert!(
        texts.iter().any(|t| t.contains("second topic")),
        "the live message tracks the new run: {texts:?}"
    );
}

#[tokio::test]
async fn test_failed_run_clears_wait_status_and_ends_tracker() {
    let mut app = app_with_session();
    app.begin_research_progress(ResearchProgress::new("item", "topic"));
    app.status = "[wait] research: item - web (>) - running".to_string();

    app.handle_event(Event::AgentNotice {
        session_id: "s1".to_string(),
        message: encode_cluster_progress_event(
            "item",
            "item",
            SessionPhase::Finalize,
            "error",
            "item failed: clarification cancelled",
        ),
    })
    .await;

    assert!(
        !app.status.starts_with("[wait]"),
        "status must leave [wait]; got {}",
        app.status
    );
    assert!(
        app.status.contains("clarification cancelled"),
        "{}",
        app.status
    );
    let tracker = app
        .research_progress
        .iter()
        .find(|p| p.name == "item")
        .expect("tracker");
    assert!(tracker.done, "a failed run is no longer running");
    assert!(tracker.web_phase_deadline.is_none());
}

#[test]
fn test_failed_deck_renders_failure_not_complete() {
    let mut tracker = DeckTracker::new();
    tracker.apply(&DeckEvent::started(DeckStage::Select, "looking up item"));
    tracker.fail("research item `x` not found");
    let rendered = tracker.render("x");
    assert!(rendered.contains("[err] Failed: research item `x` not found"));
    assert!(!rendered.contains("[ok] Complete"), "{rendered}");
}

#[tokio::test]
async fn test_continue_runs_and_reports_a_missing_item() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut app = app_with_session();
    app.cwd_path = dir.path().to_path_buf();
    let mut rx = app.event_bus.subscribe();

    app.execute_slash_command("/research continue ghost dig deeper")
        .await;
    assert!(app.status.starts_with("[wait]"), "{}", app.status);

    let mut error_text = None;
    let mut failure_report = false;
    for _ in 0..100 {
        match tokio::time::timeout(std::time::Duration::from_millis(100), rx.recv()).await {
            Ok(Ok(Event::TextDelta { text, .. }))
                if text.starts_with("From: /research continue") =>
            {
                error_text = Some(text);
            }
            Ok(Ok(Event::AgentNotice { message, .. })) if message.contains("ghost failed:") => {
                failure_report = true;
            }
            Ok(Ok(_)) | Ok(Err(_)) | Err(_) => {}
        }
        if error_text.is_some() && failure_report {
            break;
        }
    }
    let text = error_text.expect("continue publishes its outcome");
    assert!(text.contains("**Error:**"), "{text}");
    assert!(
        failure_report,
        "the run's tracker is ended by a failure report"
    );
}

#[test]
fn test_text_after_a_progress_message_starts_its_own_message() {
    let mut app = app_with_session();
    app.begin_research_progress(ResearchProgress::new("alpha", "topic"));
    app.append_assistant_text("From: /research create\n\n[ok] result for beta");
    // A refresh of the alpha run must not erase the text that followed it.
    app.refresh_research_progress_message("alpha");
    let texts = assistant_texts(&app);
    assert!(
        texts.iter().any(|t| t.contains("result for beta")),
        "the result survives the progress refresh: {texts:?}"
    );
    assert!(
        !texts
            .iter()
            .any(|t| t.starts_with("[research] Research Progress") && t.contains("result for beta")),
        "the result is not inside the progress message: {texts:?}"
    );
}

#[tokio::test]
async fn test_second_run_for_a_live_name_is_refused() {
    let mut app = app_with_session();
    app.research_live_runs
        .lock()
        .expect("lock")
        .insert("busy".to_string());

    app.execute_slash_command("/research update busy").await;

    let texts = assistant_texts(&app);
    assert!(
        texts
            .iter()
            .any(|t| t.contains("a research run for `busy` is already in progress")),
        "{texts:?}"
    );
    assert!(
        !app.research_progress.iter().any(|p| p.name == "busy"),
        "the refused command must not seed or replace a tracker"
    );
    assert!(!app.status.starts_with("[wait]"), "{}", app.status);
}

#[tokio::test]
async fn test_finished_run_frees_its_name() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut app = app_with_session();
    app.cwd_path = dir.path().to_path_buf();

    app.execute_slash_command("/research continue ghost").await;
    assert!(
        app.research_live_runs
            .lock()
            .expect("lock")
            .contains("ghost"),
        "the run holds its name while it runs"
    );
    // The item does not exist, so the run fails quickly and its task ends.
    let mut freed = false;
    for _ in 0..100 {
        if !app
            .research_live_runs
            .lock()
            .expect("lock")
            .contains("ghost")
        {
            freed = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert!(freed, "the name is released when the run's task ends");
}
