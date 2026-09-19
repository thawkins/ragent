//! PERF-044 / PERF-045 / PERF-046: TUI render-loop idle guards.
//!
//! * **PERF-044** — the periodic safety wake only paints when a
//!   wall-clock-driven display (countdown/spinner) is live.
//! * **PERF-045** — the cheap periodic polls/refreshes run at most once per
//!   `HOUSEKEEPING_INTERVAL`, and the loop keeps a wake scheduled for the end
//!   of that window.
//! * **PERF-046** — the chat input is re-wrapped and the cursor re-measured
//!   only when one of `(input, cursor, selection, width)` changes.

mod support;

use std::time::{Duration, Instant};

use ragent_agent::permission::PermissionRequest;
use ragent_tui::app::{HOUSEKEEPING_INTERVAL, InputRenderCache};
use ratatui::Terminal;
use ratatui::backend::TestBackend;

/// At least as long as the crate's private `IDLE_REDRAW_INTERVAL_MS` (2000 ms),
/// so `should_render` treats the frame as "the safety interval has elapsed".
fn past_idle_interval() -> Instant {
    Instant::now()
        .checked_sub(Duration::from_secs(3))
        .expect("monotonic clock newer than 3s")
}

fn render(app: &mut ragent_tui::App) {
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| ragent_tui::layout::render(frame, app))
        .expect("draw frame");
}

fn permission_request(id: &str) -> PermissionRequest {
    PermissionRequest {
        id: id.to_string(),
        session_id: "s1".to_string(),
        permission: "bash".to_string(),
        patterns: vec!["ls".to_string()],
        metadata: serde_json::json!({ "timeout_secs": 120u64 }),
        tool_call_id: None,
    }
}

// ── PERF-044: idle safety redraw ────────────────────────────────────────

#[test]
fn test_idle_wake_does_not_render() {
    let mut app = support::make_app();
    app.needs_redraw = false;

    assert!(
        !app.needs_periodic_redraw(),
        "a fresh app has no countdown or spinner"
    );
    assert!(
        !ragent_tui::should_render(&app, past_idle_interval()),
        "the periodic safety wake must not repaint an unchanged frame"
    );
}

#[test]
fn test_dirty_wake_always_renders() {
    let mut app = support::make_app();
    app.needs_redraw = true;

    assert!(
        ragent_tui::should_render(&app, Instant::now()),
        "a dirty UI must render immediately, regardless of the interval"
    );
}

#[test]
fn test_pending_message_cache_group_forces_safety_paint() {
    // Regression: a ToolCallStart that lands inside the PERF-042 throttle
    // window leaves the message group pending (`message_cache_dirty_from <
    // messages.len()`) and the pending-preserving frame both clears
    // `needs_redraw` and re-arms the throttle.  If a burst of events re-sets
    // `needs_redraw` afterwards, the last wake of the burst repaints only the
    // status bar and no further repaint is scheduled: the tool-call row (and
    // any streamed text following it) would stay invisible until unrelated
    // mouse/keyboard input forced a frame.  A pending group is visible
    // content, so the safety wake must paint it even with no spinner or
    // countdown live.
    let mut app = support::make_app();
    app.session_id = Some("s1".to_string());
    let sid = "s1".to_string();

    // Prime: an assistant message rendered into the cache, all state clean.
    app.handle_event(ragent_agent::event::Event::TextDelta {
        session_id: sid.clone(),
        text: "working on it".to_string(),
    });
    render(&mut app);
    assert_eq!(
        app.message_cache_dirty_from,
        app.messages.len(),
        "primed frame leaves nothing pending"
    );

    // Stream one more token so the group has already been populated (the
    // tool-call part then lands in a *throttled* group, matching production).
    app.handle_event(ragent_agent::event::Event::TextDelta {
        session_id: sid.clone(),
        text: " more".to_string(),
    });
    render(&mut app); // deferred by the throttle; the group stays pending

    // The ToolCallStart arrives while the throttle window is still open.  Its
    // wake paints the status bar (needs_redraw) but the group stays pending.
    app.handle_event(ragent_agent::event::Event::ToolCallStart {
        session_id: sid.clone(),
        call_id: "call-1".to_string(),
        tool: "bash".to_string(),
    });

    // A burst of unrelated events (e.g. ToolResult/ToolCallArgs from earlier
    // calls) re-marks the UI dirty before the throttle tail wake fires; each
    // painted frame re-arms the throttle window and consumes the dirty flag.
    render(&mut app);
    std::thread::sleep(ragent_tui::layout::MESSAGE_STREAM_MIN_INTERVAL);
    render(&mut app);

    let tool_msg_idx = app.messages.len() - 1;
    if app.message_cache_dirty_from > tool_msg_idx {
        render(&mut app);
    }

    // Simulate the end of the burst: every event was consumed, so nothing is
    // flagged dirty, yet the tool-call group may still be pending while the
    // status bar already reads "running: bash".
    app.needs_redraw = false;
    assert!(
        !app.needs_periodic_redraw(),
        "no countdown or spinner is live in this scenario"
    );

    let pending_before = ragent_tui::should_render(&app, past_idle_interval());
    if pending_before {
        render(&mut app);
    }

    // The pending message must become visible at the safety interval without
    // any user input (previously it required mouse/keyboard to appear).
    assert!(
        app.message_cache_dirty_from > tool_msg_idx,
        "the pending tool-call group must be painted by the safety wake"
    );
}

#[test]
fn test_periodic_redraw_only_while_countdown_active() {
    let mut app = support::make_app();
    app.needs_redraw = false;
    assert!(!ragent_tui::should_render(&app, past_idle_interval()));

    // A queued permission request runs a wall-clock countdown in the dialog.
    app.permission_queue.push_back(permission_request("perm-1"));
    assert!(
        app.needs_periodic_redraw(),
        "a live permission countdown needs periodic repaints"
    );
    assert!(
        ragent_tui::should_render(&app, past_idle_interval()),
        "the countdown must repaint once the safety interval elapses"
    );
    assert!(
        !ragent_tui::should_render(&app, Instant::now()),
        "the countdown must not force repaints before the interval elapses"
    );
}

#[test]
fn test_spinner_latch_requests_periodic_redraw() {
    let mut app = support::make_app();
    app.needs_redraw = false;
    app.code_index_busy = true;
    assert!(app.periodic_animation_active());
    assert!(ragent_tui::should_render(&app, past_idle_interval()));
}

/// A visible Agents panel with a running task renders a per-second elapsed
/// clock, so the safety redraw must keep painting.
#[test]
fn test_live_agents_panel_requests_periodic_redraw() {
    use ragent_agent::task::{TaskEntry, TaskStatus};

    let mut app = support::make_app();
    app.needs_redraw = false;
    app.show_agents_window = true;
    assert!(app.active_tasks.is_empty());
    assert!(
        !ragent_tui::should_render(&app, past_idle_interval()),
        "an empty panel has no ticking clock"
    );

    app.active_tasks.push(TaskEntry {
        id: "explore-0001".to_string(),
        parent_session_id: "s1".to_string(),
        child_session_id: "c1".to_string(),
        agent_name: "explore".to_string(),
        task_prompt: "explore".to_string(),
        background: true,
        status: TaskStatus::Running,
        result: None,
        error: None,
        created_at: chrono::Utc::now(),
        completed_at: None,
        reported: false,
        waiter_count: 0,
        output_file: None,
        report_status: ragent_agent::task::ReportStatus::default(),
    });
    assert!(app.needs_periodic_redraw());
    assert!(ragent_tui::should_render(&app, past_idle_interval()));

    // A finished task no longer ticks.
    app.active_tasks[0].status = TaskStatus::Completed;
    assert!(
        !ragent_tui::should_render(&app, past_idle_interval()),
        "a completed task has no ticking clock"
    );
    // A closed panel must not request periodic repaints either.
    app.active_tasks[0].status = TaskStatus::Running;
    app.show_agents_window = false;
    assert!(!ragent_tui::should_render(&app, past_idle_interval()));
}

// ── PERF-045: housekeeping gate ─────────────────────────────────────────

#[test]
fn test_housekeeping_runs_only_when_due() {
    let mut app = support::make_app();
    app.jobs_last_poll = Instant::now();
    assert!(
        !app.run_housekeeping_if_due(),
        "a wake inside the interval must skip the polls"
    );
    assert_eq!(app.housekeeping_runs, 0);

    std::thread::sleep(HOUSEKEEPING_INTERVAL + Duration::from_millis(20));

    assert!(
        app.run_housekeeping_if_due(),
        "a wake past the interval must run the polls"
    );
    assert_eq!(app.housekeeping_runs, 1);
    assert!(
        !app.run_housekeeping_if_due(),
        "the pass resets the interval window"
    );
}

#[test]
fn test_deadline_schedules_housekeeping_pass() {
    let mut app = support::make_app();
    app.needs_redraw = false;
    app.jobs_last_poll = Instant::now();
    let due_at = app.housekeeping_due_at();
    let deadline = ragent_tui::compute_next_deadline_test(&app, Instant::now());
    assert!(
        deadline <= due_at,
        "the loop must wake by the time the next housekeeping pass is due \
         (deadline {deadline:?} > due {due_at:?})"
    );
}

// ── PERF-046: input render cache ────────────────────────────────────────

#[test]
fn test_input_render_cache_reused_when_unchanged() {
    let mut app = support::make_app();
    app.input = "hello world".to_string();
    app.input_cursor = 5;

    render(&mut app);
    let cache = app.input_render_cache.clone();
    assert!(!cache.lines.is_empty(), "rows must be cached");
    assert_eq!(cache.key, "hello world");
    assert!(cache.width > 0, "the cache records the wrapped width");

    // A second frame with identical inputs must not rebuild the rows.
    render(&mut app);
    assert_eq!(
        app.input_render_cache.lines, cache.lines,
        "an unchanged frame must reuse the cached rows"
    );
    assert_eq!(app.input_render_cache.cursor_pos, cache.cursor_pos);
    assert_eq!(app.input_render_cache.height, cache.height);
}

#[test]
fn test_input_render_cache_updates_on_edit() {
    let mut app = support::make_app();
    app.input = "hello".to_string();
    app.input_cursor = 5;
    render(&mut app);
    let before = app.input_render_cache.lines.clone();

    app.insert_char_at_cursor('!');
    render(&mut app);

    assert_eq!(app.input_render_cache.key, "hello!");
    assert_ne!(
        app.input_render_cache.lines, before,
        "an edit must invalidate the cached rows"
    );
    let text: String = app
        .input_render_cache
        .lines
        .iter()
        .flat_map(|l| l.spans.iter().map(|s| s.content.to_string()))
        .collect();
    assert!(text.contains("hello!"), "row text must reflect the edit");
}

#[test]
fn test_input_render_cache_default_is_empty() {
    let cache = InputRenderCache::default();
    assert!(cache.lines.is_empty());
    assert_eq!(cache.height, 0);
}

/// Input height must equal the number of cached wrapped rows plus borders,
/// for a long single line and for a multi-line buffer.
#[test]
fn test_input_height_matches_wrapped_rows() {
    for input in ["a".repeat(300), "line one\nline two".to_string()] {
        let mut app = support::make_app();
        app.input = input.clone();
        app.input_cursor = input.chars().count();
        render(&mut app);
        let cache = &app.input_render_cache;
        assert_eq!(
            cache.height as usize,
            cache.lines.len() + 2,
            "height must be rows + 2 border rows for {input:?}"
        );
        assert!(!cache.lines.is_empty());
    }
}
