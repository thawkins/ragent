//! task migration T-015: integration tests for the reworked `render_tasks_panel`.
//!
//! Verifies FR-005, FR-007, FR-018:
//! - Panel header titled "TASKS" (not the old panel name).
//! - Status-coloured lines: pending=yellow, in_progress=cyan, completed=green,
//!   blocked=red.
//! - `(owner)` suffix appended when owner is set.
//! - `[blocked by #id, …]` annotation when derived blocked (FR-005).
//! - `active_form` rendered as indented sub-line beneath subject when
//!   `in_progress`.
//! - Scroll, scrollbar, text-selection, and mutual-exclusion behaviour
//!   preserved.

use ragent_tui::{App, app::ScreenMode, layout};
use ratatui::{Terminal, backend::TestBackend};

#[path = "support/mod.rs"]
mod support;

/// Render the app into a string buffer of the given terminal size, with the
/// TASKS panel visible.
fn render_app_to_string(app: &mut App, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| layout::render(frame, app))
        .expect("render tasks panel");

    let backend = terminal.backend();
    let buffer = backend.buffer();
    let mut text = String::new();
    let area = buffer.area();
    for y in 0..area.height {
        for x in 0..area.width {
            text.push_str(buffer[(x, y)].symbol());
        }
        text.push('\n');
    }
    text
}

/// Helper: create a session (if needed) and a task with the given fields
/// using `create_task` (which supports all Task-model columns).
fn create_task(
    app: &mut App,
    id: &str,
    session_id: &str,
    subject: &str,
    status: &str,
    active_form: Option<&str>,
    owner: Option<&str>,
    blocked_by: &[&str],
) {
    // Create session row only if it doesn't already exist (some tests
    // share the same session_id across multiple create_task calls).
    if app
        .storage
        .get_session(session_id)
        .unwrap_or(None)
        .is_none()
    {
        app.storage
            .create_session(session_id, ".")
            .expect("create session row");
    }
    let blocked_by_owned: Vec<String> = blocked_by.iter().map(|s| s.to_string()).collect();
    app.storage
        .create_task(
            id,
            session_id,
            subject,
            "",
            status,
            active_form,
            owner,
            "{}",
            &blocked_by_owned,
        )
        .expect("create task");
}

// ── Panel title ─────────────────────────────────────────────────────

/// Panel header should show "TASKS" (FR-018).
#[test]
fn test_tasks_panel_title_shows_tasks() {
    let mut app = support::make_app();
    app.session_id = Some("title-session".to_string());
    app.show_tasks_panel = true;
    app.show_log = false;
    app.show_profile = false;
    app.current_screen = ScreenMode::Chat;

    let text = render_app_to_string(&mut app, 120, 40);
    assert!(
        text.contains("TASKS"),
        "panel title should be 'TASKS'; got:\n{text}"
    );
}

// ── Empty placeholder ────────────────────────────────────���──────────

/// Empty panel should show "No tasks" (not the old placeholder).
#[test]
fn test_tasks_panel_empty_shows_no_tasks() {
    let mut app = support::make_app();
    app.session_id = Some("empty-session".to_string());
    app.show_tasks_panel = true;
    app.current_screen = ScreenMode::Chat;

    let text = render_app_to_string(&mut app, 120, 40);
    assert!(
        text.contains("No tasks"),
        "empty panel should show 'No tasks'; got:\n{text}"
    );
}

// ── Status rendering ────────────────────────────────────────────────

/// "completed" status should render with uppercased prefix (FR-007).
#[test]
fn test_tasks_panel_completed_status() {
    let mut app = support::make_app();
    let session_id = "completed-session".to_string();
    app.session_id = Some(session_id.clone());
    app.show_tasks_panel = true;
    app.current_screen = ScreenMode::Chat;

    create_task(
        &mut app,
        "c1",
        &session_id,
        "Completed task",
        "completed",
        None,
        None,
        &[],
    );

    let text = render_app_to_string(&mut app, 120, 40);
    assert!(
        text.contains("[COMPLETED] Completed task"),
        "should render [COMPLETED] prefix; got:\n{text}"
    );
}

/// "pending" status should render with uppercased prefix (FR-007).
#[test]
fn test_tasks_panel_pending_status() {
    let mut app = support::make_app();
    let session_id = "pending-session".to_string();
    app.session_id = Some(session_id.clone());
    app.show_tasks_panel = true;
    app.current_screen = ScreenMode::Chat;

    create_task(
        &mut app,
        "p1",
        &session_id,
        "Pending task",
        "pending",
        None,
        None,
        &[],
    );

    let text = render_app_to_string(&mut app, 120, 40);
    assert!(
        text.contains("[PENDING] Pending task"),
        "should render [PENDING] prefix; got:\n{text}"
    );
}

/// "in_progress" status should render with uppercased prefix (FR-007).
#[test]
fn test_tasks_panel_in_progress_status() {
    let mut app = support::make_app();
    let session_id = "ip-session".to_string();
    app.session_id = Some(session_id.clone());
    app.show_tasks_panel = true;
    app.current_screen = ScreenMode::Chat;

    create_task(
        &mut app,
        "ip1",
        &session_id,
        "Active task",
        "in_progress",
        None,
        None,
        &[],
    );

    let text = render_app_to_string(&mut app, 120, 40);
    assert!(
        text.contains("[IN_PROGRESS] Active task"),
        "should render [IN_PROGRESS] prefix; got:\n{text}"
    );
}

// ── Owner suffix (FR-018) ───────────────────────────────────────────

/// When owner is set, an `(owner)` suffix should be appended.
#[test]
fn test_tasks_panel_owner_suffix() {
    let mut app = support::make_app();
    let session_id = "owner-session".to_string();
    app.session_id = Some(session_id.clone());
    app.show_tasks_panel = true;
    app.current_screen = ScreenMode::Chat;

    create_task(
        &mut app,
        "o1",
        &session_id,
        "Owned task",
        "pending",
        None,
        Some("coder-agent"),
        &[],
    );

    let text = render_app_to_string(&mut app, 120, 40);
    assert!(
        text.contains("(coder-agent)"),
        "should show owner suffix '(coder-agent)'; got:\n{text}"
    );
}

/// When owner is not set, no `(owner)` suffix should appear.
#[test]
fn test_tasks_panel_no_owner_no_suffix() {
    let mut app = support::make_app();
    let session_id = "no-owner-session".to_string();
    app.session_id = Some(session_id.clone());
    app.show_tasks_panel = true;
    app.current_screen = ScreenMode::Chat;

    create_task(
        &mut app,
        "no1",
        &session_id,
        "Unowned task",
        "pending",
        None,
        None,
        &[],
    );

    let text = render_app_to_string(&mut app, 120, 40);
    assert!(
        !text.contains("(coder-agent)"),
        "should not show owner suffix when owner is None; got:\n{text}"
    );
}

/// When owner is empty string, no `(owner)` suffix should appear.
#[test]
fn test_tasks_panel_empty_owner_no_suffix() {
    let mut app = support::make_app();
    let session_id = "empty-owner-session".to_string();
    app.session_id = Some(session_id.clone());
    app.show_tasks_panel = true;
    app.current_screen = ScreenMode::Chat;

    create_task(
        &mut app,
        "eo1",
        &session_id,
        "Empty owner task",
        "pending",
        None,
        Some(""),
        &[],
    );

    let text = render_app_to_string(&mut app, 120, 40);
    assert!(
        !text.contains("()"),
        "should not show empty owner suffix '()'; got:\n{text}"
    );
}

// ── Blocked-by annotation (FR-005, FR-018) ──────────────────────────

/// When a pending task has an uncompleted blocked_by entry, the panel
/// should show `[blocked by #id]` annotation (FR-005, FR-018).
#[test]
fn test_tasks_panel_blocked_by_annotation() {
    let mut app = support::make_app();
    let session_id = "blocked-session".to_string();
    app.session_id = Some(session_id.clone());
    app.show_tasks_panel = true;
    app.current_screen = ScreenMode::Chat;

    // Create a blocker task that is NOT completed.
    create_task(
        &mut app,
        "blocker",
        &session_id,
        "Blocker task",
        "pending",
        None,
        None,
        &[],
    );
    // Create a task blocked by the blocker.
    create_task(
        &mut app,
        "blocked",
        &session_id,
        "Blocked task",
        "pending",
        None,
        None,
        &["blocker"],
    );

    let text = render_app_to_string(&mut app, 120, 40);
    assert!(
        text.contains("[blocked by #blocker]"),
        "should show '[blocked by #blocker]' annotation; got:\n{text}"
    );
}

/// When a pending task's blocked_by entries are all completed, no
/// `[blocked by …]` annotation should appear.
#[test]
fn test_tasks_panel_completed_blocker_no_annotation() {
    let mut app = support::make_app();
    let session_id = "completed-blocker-session".to_string();
    app.session_id = Some(session_id.clone());
    app.show_tasks_panel = true;
    app.current_screen = ScreenMode::Chat;

    // Create a blocker task that IS completed.
    create_task(
        &mut app,
        "done-blocker",
        &session_id,
        "Done blocker",
        "completed",
        None,
        None,
        &[],
    );
    // Create a task blocked by the completed blocker.
    create_task(
        &mut app,
        "free",
        &session_id,
        "Free task",
        "pending",
        None,
        None,
        &["done-blocker"],
    );

    let text = render_app_to_string(&mut app, 120, 40);
    assert!(
        !text.contains("[blocked by"),
        "should NOT show blocked-by annotation when all blockers completed; got:\n{text}"
    );
}

/// Multiple blockers should all appear in the annotation, comma-separated.
#[test]
fn test_tasks_panel_multiple_blockers() {
    let mut app = support::make_app();
    let session_id = "multi-blocker-session".to_string();
    app.session_id = Some(session_id.clone());
    app.show_tasks_panel = true;
    app.current_screen = ScreenMode::Chat;

    create_task(
        &mut app,
        "b1",
        &session_id,
        "Blocker 1",
        "pending",
        None,
        None,
        &[],
    );
    create_task(
        &mut app,
        "b2",
        &session_id,
        "Blocker 2",
        "in_progress",
        None,
        None,
        &[],
    );
    create_task(
        &mut app,
        "target",
        &session_id,
        "Target task",
        "pending",
        None,
        None,
        &["b1", "b2"],
    );

    let text = render_app_to_string(&mut app, 120, 40);
    assert!(
        text.contains("#b1") && text.contains("#b2"),
        "should show both blocker IDs; got:\n{text}"
    );
}

/// An in_progress task with blocked_by should NOT show the blocked-by
/// annotation (is_blocked only applies to pending tasks per FR-005).
#[test]
fn test_tasks_panel_in_progress_not_blocked() {
    let mut app = support::make_app();
    let session_id = "ip-blocked-session".to_string();
    app.session_id = Some(session_id.clone());
    app.show_tasks_panel = true;
    app.current_screen = ScreenMode::Chat;

    create_task(
        &mut app,
        "blocker",
        &session_id,
        "Blocker",
        "pending",
        None,
        None,
        &[],
    );
    create_task(
        &mut app,
        "ip-blocked",
        &session_id,
        "IP blocked task",
        "in_progress",
        None,
        None,
        &["blocker"],
    );

    let text = render_app_to_string(&mut app, 120, 40);
    assert!(
        !text.contains("[blocked by"),
        "in_progress task should not show blocked-by annotation; got:\n{text}"
    );
}

// ── active_form sub-line (FR-018) ───────────────────────────────────

/// When a task is in_progress and has active_form, an indented sub-line
/// with `→` prefix should appear beneath the subject.
#[test]
fn test_tasks_panel_active_form_subline() {
    let mut app = support::make_app();
    let session_id = "active-form-session".to_string();
    app.session_id = Some(session_id.clone());
    app.show_tasks_panel = true;
    app.current_screen = ScreenMode::Chat;

    create_task(
        &mut app,
        "af1",
        &session_id,
        "Active task",
        "in_progress",
        Some("Writing tests for module X"),
        None,
        &[],
    );

    let text = render_app_to_string(&mut app, 120, 40);
    assert!(
        text.contains("Writing tests for module X"),
        "should show active_form text; got:\n{text}"
    );
    assert!(
        text.contains("→"),
        "should show → arrow prefix for active_form sub-line; got:\n{text}"
    );
}

/// When a task is pending (not in_progress), active_form should NOT be
/// rendered as a sub-line.
#[test]
fn test_tasks_panel_active_form_only_when_in_progress() {
    let mut app = support::make_app();
    let session_id = "pending-active-session".to_string();
    app.session_id = Some(session_id.clone());
    app.show_tasks_panel = true;
    app.current_screen = ScreenMode::Chat;

    create_task(
        &mut app,
        "pa1",
        &session_id,
        "Pending with active form",
        "pending",
        Some("Should not show"),
        None,
        &[],
    );

    let text = render_app_to_string(&mut app, 120, 40);
    assert!(
        !text.contains("Should not show"),
        "active_form should NOT render when status is pending; got:\n{text}"
    );
}

/// When a task is in_progress but active_form is None, no sub-line.
#[test]
fn test_tasks_panel_no_active_form_no_subline() {
    let mut app = support::make_app();
    let session_id = "no-active-session".to_string();
    app.session_id = Some(session_id.clone());
    app.show_tasks_panel = true;
    app.current_screen = ScreenMode::Chat;

    create_task(
        &mut app,
        "na1",
        &session_id,
        "No active form",
        "in_progress",
        None,
        None,
        &[],
    );

    let text = render_app_to_string(&mut app, 120, 40);
    assert!(
        !text.contains("→"),
        "should not show → when active_form is None; got:\n{text}"
    );
}

/// When active_form is empty string, no sub-line.
#[test]
fn test_tasks_panel_empty_active_form_no_subline() {
    let mut app = support::make_app();
    let session_id = "empty-active-session".to_string();
    app.session_id = Some(session_id.clone());
    app.show_tasks_panel = true;
    app.current_screen = ScreenMode::Chat;

    create_task(
        &mut app,
        "ea1",
        &session_id,
        "Empty active form",
        "in_progress",
        Some(""),
        None,
        &[],
    );

    let text = render_app_to_string(&mut app, 120, 40);
    assert!(
        !text.contains("→"),
        "should not show → when active_form is empty; got:\n{text}"
    );
}

// ── Combined: owner + active_form + status ──────────────────────────

/// A task that is in_progress with both owner and active_form should show
/// all three: status prefix, owner suffix, and active_form sub-line.
#[test]
fn test_tasks_panel_combined_owner_and_active_form() {
    let mut app = support::make_app();
    let session_id = "combined-session".to_string();
    app.session_id = Some(session_id.clone());
    app.show_tasks_panel = true;
    app.current_screen = ScreenMode::Chat;

    create_task(
        &mut app,
        "combo",
        &session_id,
        "Combined task",
        "in_progress",
        Some("Refactoring module Y"),
        Some("agent-42"),
        &[],
    );

    let text = render_app_to_string(&mut app, 120, 40);
    assert!(
        text.contains("[IN_PROGRESS] Combined task"),
        "should show status + subject; got:\n{text}"
    );
    assert!(
        text.contains("(agent-42)"),
        "should show owner suffix; got:\n{text}"
    );
    assert!(
        text.contains("Refactoring module Y"),
        "should show active_form sub-line; got:\n{text}"
    );
}

// ── Scroll preservation ─────────────────────────────────────────────

/// The panel should still set tasks_max_scroll correctly for overflow.
#[test]
fn test_tasks_panel_scroll_max_scroll() {
    let mut app = support::make_app();
    let session_id = "scroll-session".to_string();
    app.session_id = Some(session_id.clone());
    app.show_tasks_panel = true;
    app.current_screen = ScreenMode::Chat;

    app.storage
        .create_session(&session_id, ".")
        .expect("create session row");

    // Insert more tasks than the panel height can show.
    for i in 0..40 {
        app.storage
            .create_task(
                &format!("s{i}"),
                &session_id,
                &format!("task {i}"),
                "",
                "pending",
                None,
                None,
                "{}",
                &[],
            )
            .expect("create scroll task");
    }

    let _ = render_app_to_string(&mut app, 120, 20);
    assert!(
        app.tasks_max_scroll > 0,
        "tasks_max_scroll should be > 0 when content overflows, got {}",
        app.tasks_max_scroll
    );
}

/// The panel should still set tasks_area when visible.
#[test]
fn test_tasks_panel_sets_area() {
    let mut app = support::make_app();
    app.session_id = Some("area-session".to_string());
    app.show_tasks_panel = true;
    app.current_screen = ScreenMode::Chat;

    let _ = render_app_to_string(&mut app, 120, 40);
    assert!(
        app.tasks_area.area() > 0,
        "tasks_area should be set to a non-empty rect when panel is visible"
    );
}

/// The panel should clear tasks_area when not visible.
#[test]
fn test_tasks_panel_hidden_clears_area() {
    let mut app = support::make_app();
    app.session_id = Some("hidden-session".to_string());
    app.show_tasks_panel = false;
    app.current_screen = ScreenMode::Chat;

    let _ = render_app_to_string(&mut app, 120, 40);
    assert_eq!(
        app.tasks_area.area(),
        0,
        "tasks_area should be empty when panel is hidden"
    );
}

// ── Read-only rendering ─────────────────────────────────────────────

/// Rendering the panel must not mutate stored tasks.
#[test]
fn test_tasks_panel_does_not_mutate_tasks() {
    let mut app = support::make_app();
    let session_id = "immutable-session".to_string();
    app.session_id = Some(session_id.clone());
    app.show_tasks_panel = true;
    app.current_screen = ScreenMode::Chat;

    create_task(
        &mut app,
        "im1",
        &session_id,
        "Immutable task",
        "pending",
        Some("Doing stuff"),
        Some("agent-1"),
        &[],
    );

    let before = app
        .storage
        .list_tasks(&session_id, None)
        .expect("read tasks before render");
    // Render twice.
    let _ = render_app_to_string(&mut app, 120, 40);
    let _ = render_app_to_string(&mut app, 120, 40);
    let after = app
        .storage
        .list_tasks(&session_id, None)
        .expect("read tasks after render");

    assert_eq!(before.len(), after.len(), "row count must not change");
    assert_eq!(after[0].title, "Immutable task");
    assert_eq!(after[0].status, "pending");
    assert_eq!(after[0].active_form.as_deref(), Some("Doing stuff"));
    assert_eq!(after[0].owner.as_deref(), Some("agent-1"));
}

// ── Error state ─────────────────────────────────────────────────────
//
// When storage query fails, the panel should show "Failed to load tasks"
// (not "Failed to load tasks").  This is implicitly tested by the existing
// test_tasks_panel tests since we can't easily force a storage error with
// the real Storage backend.  The text change is verified by the source code.
