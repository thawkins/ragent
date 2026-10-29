//! Spec `teamssetup` T-019 (FR-015, F-11): `TeamSettings.auto_claim_tasks`
//! must *do* something.
//!
//! Before this task the setting was read by nothing: `config.rs` declared it,
//! defaulted it `true`, and no consumer existed, so its doc comment ("teammates
//! automatically claim the next available task when idle") described a
//! behaviour that did not exist (F-11).
//!
//! FR-015 gives it exactly two meanings, and these tests pin both:
//!
//! - **`true`**: a teammate that goes `Idle` with no current task is handed the
//!   next claimable task automatically - the task reads `in_progress` with
//!   `assigned_to` set, the roster records it on the member, and the member is
//!   `Working`;
//! - **`false`**: the member stays `Idle` and every task stays `pending`, until
//!   the lead assigns or the teammate claims explicitly.
//!
//! The interception lives in the single status writer (FR-008), so every route
//! into `Idle` answers the same question. The tests drive the writer directly
//! through both of its entry points - the agent-loop/resume path
//! (`set_member_status_auto_claiming`) and the team-tool path
//! (`set_member_status_in_auto_claiming`) - and assert they agree.
//!
//! Every test uses its own `tempfile::tempdir()`, so nothing is written into the
//! repository and no two files share a scratch directory.

use ragent_agent::team::{
    MemberStatus, Task, TaskStatus, TaskStore, TeamManager, TeamStore, TransitionCause,
};

#[path = "manager_support/mod.rs"]
mod manager_support;
use manager_support::team_manager;

/// Rewrite the team's `auto_claim_tasks` setting on disk.
fn set_auto_claim(manager: &TeamManager, enabled: bool) {
    let mut store = TeamStore::load(&manager.team_dir).expect("load team");
    store.config.settings.auto_claim_tasks = enabled;
    store.save().expect("save team");
}

/// Add one pending, dependency-free task.
async fn add_task(manager: &TeamManager, id: &str, title: &str) {
    let store = TaskStore::open(&manager.team_dir).expect("open task store");
    store
        .add_task(Task::new(id, title))
        .await
        .expect("add task");
}

/// Add one pending task that depends on `depends_on`.
async fn add_task_depending_on(
    manager: &TeamManager,
    id: &str,
    title: &str,
    depends_on: Vec<String>,
) {
    let store = TaskStore::open(&manager.team_dir).expect("open task store");
    let mut task = Task::new(id, title);
    task.depends_on = depends_on;
    store.add_task(task).await.expect("add task");
}

/// Every task's `(id, status, assigned_to)` as `tasks.json` records it.
fn tasks(manager: &TeamManager) -> Vec<(String, TaskStatus, Option<String>)> {
    TaskStore::open(&manager.team_dir)
        .expect("open task store")
        .read()
        .expect("read tasks")
        .tasks
        .iter()
        .map(|t| (t.id.clone(), t.status, t.assigned_to.clone()))
        .collect()
}

/// The fixture member's `(status, current_task_id)` as `config.json` records it.
fn member(manager: &TeamManager) -> (MemberStatus, Option<String>) {
    let store = TeamStore::load(&manager.team_dir).expect("load team");
    let m = store.config.member_by_id("tm-001").expect("fixture member");
    (m.status, m.current_task_id.clone())
}

/// Move the fixture member to `Working` so the parked move under test is legal
/// from where the member actually is.
fn start_working(manager: &TeamManager) {
    manager
        .set_member_status(
            "tm-001",
            MemberStatus::Working,
            TransitionCause::TaskClaimed,
        )
        .expect("Working is reachable from the fixture's Working status");
}

/// Give `tm-001` a live agent loop, because the auto-claim only hands work to a
/// member that has one to work it.
///
/// The claim is made for a member whose loop is still running - a task recorded
/// on a roster row whose loop has returned is work nobody will ever do (see
/// `TeamManager::auto_claim_on_idle`). The shared fixture's member has no loop
/// at all, so a test of the claim has to put one there: a task that parks until
/// the test drops the returned sender, which is the shape of a live turn. The
/// sender must be kept for the length of the test; when it drops, the loop
/// finishes, exactly as a teammate's loop does when its turn returns.
fn live_loop(manager: &TeamManager) -> tokio::sync::oneshot::Sender<()> {
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    let handle = tokio::spawn(async move {
        let _ = rx.await;
    });
    let agent_id = member_agent_id(manager);
    manager.set_agent_loop_handle(&agent_id, Some(handle));
    tx
}

/// Take the member's live loop away once it has finished.
///
/// Replaces the production "the loop returned" instant: no handle is left for
/// [`TeamManager::has_live_agent_loop`] to find a running task in.
async fn finish_loop(manager: &TeamManager, done: tokio::sync::oneshot::Sender<()>) {
    drop(done);
    let agent_id = member_agent_id(manager);
    manager.set_agent_loop_handle(&agent_id, None);
}

/// The agent ID the shared fixture assigned its member (`alice`); `tm-001` in
/// every current test, but read from the roster rather than assumed.
fn member_agent_id(manager: &TeamManager) -> String {
    TeamStore::load(&manager.team_dir)
        .expect("load team")
        .config
        .members
        .first()
        .expect("the fixture has one member")
        .agent_id
        .clone()
}

// -- the setting on: the idle transition claims the next task -----------------

#[tokio::test]
async fn idle_with_auto_claim_on_takes_the_next_task_and_returns_to_working() {
    let (manager, _tmp, _bus) = team_manager("auto-on", "lead-1", true);
    let _live = live_loop(&manager);
    set_auto_claim(&manager, true);
    add_task(&manager, "task-001", "the only task").await;

    let (transition, interrupted) = manager
        .set_member_status_auto_claiming(
            "tm-001",
            MemberStatus::Idle,
            TransitionCause::ReportedIdle,
        )
        .await
        .expect("the idle move is legal");

    // The idle transition really happened: it is returned, persisted, and
    // published like any other. `interrupted` is what reports the follow-on.
    assert_eq!(
        transition.expect("Working -> Idle was a transition").to,
        MemberStatus::Idle
    );
    assert!(
        interrupted,
        "auto_claim_tasks claimed a task, so the member is no longer idle"
    );

    let (status, held) = member(&manager);
    assert_eq!(
        status,
        MemberStatus::Working,
        "the claim moved it to Working"
    );
    assert_eq!(held.as_deref(), Some("task-001"), "the task is held");

    assert_eq!(
        tasks(&manager),
        vec![(
            "task-001".to_string(),
            TaskStatus::InProgress,
            Some("tm-001".to_string())
        )],
        "the task must be claimed for the idle member"
    );
}

#[tokio::test]
async fn idle_with_auto_claim_on_but_nothing_claimable_stays_idle() {
    let (manager, _tmp, _bus) = team_manager("auto-on-empty", "lead-1", true);
    let _live = live_loop(&manager);
    set_auto_claim(&manager, true);

    let (transition, interrupted) = manager
        .set_member_status_auto_claiming(
            "tm-001",
            MemberStatus::Idle,
            TransitionCause::ReportedIdle,
        )
        .await
        .expect("the idle move is legal");

    assert_eq!(
        transition.expect("Working -> Idle was a transition").to,
        MemberStatus::Idle
    );
    assert!(
        !interrupted,
        "there was nothing to claim, so the member stays idle"
    );
    assert_eq!(member(&manager), (MemberStatus::Idle, None));
}

#[tokio::test]
async fn auto_claim_skips_tasks_that_are_not_claimable() {
    let (manager, _tmp, _bus) = team_manager("auto-on-blocked", "lead-1", true);
    let _live = live_loop(&manager);
    set_auto_claim(&manager, true);
    // A completed task and one with an unmet dependency are both unclaimable,
    // so the claim has to look past them for the one that can start. FR-039
    // validates `depends_on` at write time, so the dependency names a real task
    // that simply has not finished: a dangling id would now be refused.
    {
        let store = TaskStore::open(&manager.team_dir).expect("open task store");
        let mut finished = Task::new("task-done", "already finished");
        finished.status = TaskStatus::Completed;
        store.add_task(finished).await.expect("add completed task");
        let mut open = Task::new("task-open", "still in progress elsewhere");
        open.status = TaskStatus::InProgress;
        open.assigned_to = Some("tm-009".to_string());
        store.add_task(open).await.expect("add open task");
    }
    add_task_depending_on(
        &manager,
        "task-blocked",
        "waiting on a task that has not completed",
        vec!["task-open".to_string()],
    )
    .await;
    add_task(&manager, "task-pending", "the claimable one").await;

    let (_transition, interrupted) = manager
        .set_member_status_auto_claiming(
            "tm-001",
            MemberStatus::Idle,
            TransitionCause::ReportedIdle,
        )
        .await
        .expect("the idle move is legal");

    assert!(interrupted, "one task was claimable");
    assert_eq!(
        member(&manager).1.as_deref(),
        Some("task-pending"),
        "the claim must skip the completed and the dependency-blocked task"
    );
    let by_id = tasks(&manager);
    let status_of = |id: &str| {
        by_id
            .iter()
            .find(|(t, _, _)| t == id)
            .map(|(_, s, _)| *s)
            .expect("task listed")
    };
    assert_eq!(status_of("task-pending"), TaskStatus::InProgress);
    assert_eq!(
        status_of("task-blocked"),
        TaskStatus::Pending,
        "an unclaimable task must be left alone"
    );
    assert_eq!(status_of("task-done"), TaskStatus::Completed);
}

// -- the setting off: the member is held idle ---------------------------------

#[tokio::test]
async fn idle_with_auto_claim_off_holds_the_member_idle() {
    let (manager, _tmp, _bus) = team_manager("auto-off", "lead-1", true);
    set_auto_claim(&manager, false);
    add_task(&manager, "task-001", "one").await;
    add_task(&manager, "task-002", "two").await;

    let (transition, interrupted) = manager
        .set_member_status_auto_claiming(
            "tm-001",
            MemberStatus::Idle,
            TransitionCause::ReportedIdle,
        )
        .await
        .expect("the idle move is legal");

    assert_eq!(
        transition.expect("Working -> Idle was a transition").to,
        MemberStatus::Idle
    );
    assert!(!interrupted, "the setting is off, so nothing was claimed");
    assert_eq!(
        member(&manager),
        (MemberStatus::Idle, None),
        "the member must stay idle until the lead assigns or it claims"
    );
    assert!(
        tasks(&manager)
            .iter()
            .all(|(_, status, assigned)| *status == TaskStatus::Pending && assigned.is_none()),
        "with auto_claim_tasks off every task must still be pending"
    );
}

#[tokio::test]
async fn auto_claim_off_leaves_the_task_for_an_explicit_claim() {
    // The point of the `false` reading: the lead (or the teammate) decides, and
    // the ordinary claim path still works on the very task the runtime declined
    // to take.
    let (manager, _tmp, _bus) = team_manager("auto-off-manual", "lead-1", true);
    let _live = live_loop(&manager);
    set_auto_claim(&manager, false);
    add_task(&manager, "task-001", "one").await;

    manager
        .set_member_status_auto_claiming(
            "tm-001",
            MemberStatus::Idle,
            TransitionCause::ReportedIdle,
        )
        .await
        .expect("the idle move is legal");
    assert_eq!(member(&manager), (MemberStatus::Idle, None));

    let (claimed, already_had) = manager
        .claim_next_task("tm-001")
        .await
        .expect("an explicit claim still works");
    assert!(!already_had);
    assert_eq!(
        claimed.map(|t| t.id),
        Some("task-001".to_string()),
        "the task the runtime declined to auto-claim must still be claimable"
    );
}

// -- both entry points agree --------------------------------------------------

#[tokio::test]
async fn the_tool_path_claims_on_idle_exactly_like_the_agent_loop_path() {
    let (manager, _tmp, _bus) = team_manager("auto-tool-path", "lead-1", true);
    let _live = live_loop(&manager);
    set_auto_claim(&manager, true);
    add_task(&manager, "task-001", "one").await;
    start_working(&manager);

    let transition = manager
        .set_member_status_in_auto_claiming(
            &manager.team_dir.clone(),
            "tm-001",
            MemberStatus::Idle,
            TransitionCause::ReportedIdle,
        )
        .await
        .expect("the tool path's idle move is legal");

    assert_eq!(
        transition.expect("Working -> Idle was a transition").to,
        MemberStatus::Idle,
        "the tool observes the idle transition it asked for"
    );
    assert_eq!(
        member(&manager),
        (MemberStatus::Working, Some("task-001".to_string())),
        "and the roster shows the member is working the auto-claimed task"
    );
}

#[tokio::test]
async fn the_tool_path_holds_the_member_idle_when_the_setting_is_off() {
    let (manager, _tmp, _bus) = team_manager("auto-tool-off", "lead-1", true);
    set_auto_claim(&manager, false);
    add_task(&manager, "task-001", "one").await;
    start_working(&manager);

    let transition = manager
        .set_member_status_in_auto_claiming(
            &manager.team_dir.clone(),
            "tm-001",
            MemberStatus::Idle,
            TransitionCause::ReportedIdle,
        )
        .await
        .expect("the tool path's idle move is legal");

    assert_eq!(
        transition.expect("Working -> Idle was a transition").to,
        MemberStatus::Idle
    );
    assert_eq!(member(&manager), (MemberStatus::Idle, None));
    assert_eq!(
        tasks(&manager),
        vec![("task-001".to_string(), TaskStatus::Pending, None)],
        "the setting off must leave the task untouched"
    );
}

// -- the interception is scoped to the idle edge ------------------------------

#[tokio::test]
async fn a_move_to_another_status_never_claims() {
    let (manager, _tmp, _bus) = team_manager("auto-other-edge", "lead-1", true);
    set_auto_claim(&manager, true);
    add_task(&manager, "task-001", "one").await;

    let (_transition, interrupted) = manager
        .set_member_status_auto_claiming(
            "tm-001",
            MemberStatus::Suspended,
            TransitionCause::Suspended,
        )
        .await
        .expect("Working -> Suspended is legal");

    assert!(!interrupted, "only the idle edge claims");
    assert_eq!(member(&manager).0, MemberStatus::Suspended);
    assert_eq!(
        tasks(&manager),
        vec![("task-001".to_string(), TaskStatus::Pending, None)],
        "a suspended member must not be given work"
    );
}

#[tokio::test]
async fn re_asserting_idle_does_not_claim() {
    // A member that is already idle and reports idle again is not a transition,
    // so the setting is not even consulted: `team_idle` called twice must not
    // hand out a second task.
    let (manager, _tmp, _bus) = team_manager("auto-reassert", "lead-1", true);
    let _live = live_loop(&manager);
    set_auto_claim(&manager, true);
    manager
        .set_member_status("tm-001", MemberStatus::Idle, TransitionCause::ReportedIdle)
        .expect("Working -> Idle");
    add_task(&manager, "task-001", "one").await;

    let (transition, interrupted) = manager
        .set_member_status_auto_claiming(
            "tm-001",
            MemberStatus::Idle,
            TransitionCause::ReportedIdle,
        )
        .await
        .expect("a re-assertion is not an error");

    assert!(
        transition.is_none(),
        "re-asserting the held status is not a transition"
    );
    assert!(!interrupted);
    assert_eq!(
        tasks(&manager),
        vec![("task-001".to_string(), TaskStatus::Pending, None)],
        "a re-asserted idle must not claim"
    );
}

#[tokio::test]
async fn auto_claim_never_takes_a_second_task_from_a_member_that_holds_one() {
    // `team_idle` refuses to park a member that still holds a task, but the
    // writer must not depend on that guard: a member with `current_task_id` set
    // is not looking for work even if some other route moves it to idle.
    let (manager, _tmp, _bus) = team_manager("auto-holds-task", "lead-1", true);
    set_auto_claim(&manager, true);
    add_task(&manager, "task-001", "already mine").await;
    add_task(&manager, "task-002", "someone else's").await;
    manager
        .update_member("tm-001", |m| {
            m.current_task_id = Some("task-001".to_string());
        })
        .expect("record a held task");

    let (transition, interrupted) = manager
        .set_member_status_auto_claiming(
            "tm-001",
            MemberStatus::Idle,
            TransitionCause::ReportedIdle,
        )
        .await
        .expect("Working -> Idle is legal");

    assert!(
        transition.is_some(),
        "the idle move itself is not suppressed"
    );
    assert!(
        !interrupted,
        "a member that already holds a task is not given another"
    );
    let by_id = tasks(&manager);
    assert!(
        by_id
            .iter()
            .all(|(_, status, _)| *status == TaskStatus::Pending),
        "neither task may be claimed: {by_id:?}"
    );
}

// -- the default -------------------------------------------------------------

#[tokio::test]
async fn a_team_created_without_the_field_auto_claims() {
    // The setting defaults `true`, and its documented meaning is now the
    // enforced one, so a team whose `config.json` never mentioned it behaves
    // like an explicit `true`.
    let (manager, _tmp, _bus) = team_manager("auto-default", "lead-1", true);
    let _live = live_loop(&manager);
    assert!(
        TeamStore::load(&manager.team_dir)
            .expect("load team")
            .config
            .settings
            .auto_claim_tasks,
        "the fixture writes the documented default"
    );
    add_task(&manager, "task-001", "one").await;

    let (_transition, interrupted) = manager
        .set_member_status_auto_claiming(
            "tm-001",
            MemberStatus::Idle,
            TransitionCause::ReportedIdle,
        )
        .await
        .expect("Working -> Idle is legal");

    assert!(interrupted, "the default is on, so the idle claims");
    assert_eq!(member(&manager).1.as_deref(), Some("task-001"));
}

// -- the loop is over: the member stays idle ---------------------------------

#[tokio::test]
async fn a_finished_loop_leaves_the_member_idle_and_the_task_claimable() {
    // The failure this pins: the spawn loop parks a teammate *after* awaiting
    // its turn, so at the instant `set_member_status_auto_claiming(.., Idle, ..)`
    // runs its loop is over. The auto-claim still handed it the next task and
    // moved the roster to `working`, so the roster claimed a member was working
    // a task that no run existed to work: `team_idle` then refused to park it
    // ("you still hold a task"), nothing recorded progress, and the watchdog
    // eventually failed a teammate which had in fact completed.
    let (manager, _tmp, _bus) = team_manager("auto-loop-over", "lead-1", true);
    set_auto_claim(&manager, true);
    let live = live_loop(&manager);
    add_task(&manager, "task-001", "the next task").await;
    finish_loop(&manager, live).await;
    assert!(
        !manager.has_live_agent_loop("tm-001"),
        "fixture: the member's loop has returned"
    );

    let (transition, interrupted) = manager
        .set_member_status_auto_claiming(
            "tm-001",
            MemberStatus::Idle,
            TransitionCause::ReportedIdle,
        )
        .await
        .expect("Working -> Idle is legal");

    assert_eq!(
        transition.expect("the idle move happened").to,
        MemberStatus::Idle
    );
    assert!(
        !interrupted,
        "a member with no loop left must not be moved back to working"
    );
    assert_eq!(
        member(&manager),
        (MemberStatus::Idle, None),
        "the roster must not hand a task to a member that cannot run it"
    );
    assert_eq!(
        tasks(&manager),
        vec![("task-001".to_string(), TaskStatus::Pending, None)],
        "the task must stay claimable by a live member or by the lead"
    );
}

#[tokio::test]
async fn a_running_loop_still_takes_the_next_task() {
    // The other half of the guard: the spawn loop is not the only route into
    // `Idle` (a turn can also park itself from inside its own loop, and
    // `resume_teammate` runs while the loop is alive). Those must keep claiming,
    // so the gate is on the loop, not on the caller.
    let (manager, _tmp, _bus) = team_manager("auto-loop-live", "lead-1", true);
    set_auto_claim(&manager, true);
    let _live = live_loop(&manager);
    add_task(&manager, "task-001", "the next task").await;

    let (_transition, interrupted) = manager
        .set_member_status_auto_claiming(
            "tm-001",
            MemberStatus::Idle,
            TransitionCause::ReportedIdle,
        )
        .await
        .expect("Working -> Idle is legal");

    assert!(interrupted, "a member with a live loop still claims");
    assert_eq!(
        member(&manager),
        (MemberStatus::Working, Some("task-001".to_string())),
        "the roster records the claim on the member that will work it"
    );
}
