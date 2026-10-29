//! FR-031 coverage for `team_wait`: every member the tool was waiting on is
//! reported at the end of the call as `Idle`, as parked, as failed, or as still
//! running when the timeout fired.
//!
//! Before T-034 the tool could only say "all idle" or "timed out, N still
//! working", and it counted a deliberately suspended or plan-pending teammate as
//! "still working" - so an operator could not tell a pause from a wedge (F-17).
//! These tests pin the four outcomes, and pin the F-17 regression directly: a
//! parked teammate must not consume the wait budget.

use std::sync::Arc;

use ragent_agent::event::EventBus;
use ragent_agent::team::{MemberStatus, TeamMember, TeamStore};
use ragent_agent::tool::{Tool, ToolContext};
use serde_json::json;

#[path = "team_support/mod.rs"]
mod team_support;
use team_support::setup_workspace;

/// Build a `ToolContext` rooted at `working_dir`, publishing on `event_bus`.
fn make_ctx(
    working_dir: &std::path::Path,
    session_id: &str,
    event_bus: Arc<EventBus>,
) -> ToolContext {
    ToolContext {
        session_id: session_id.to_string(),
        working_dir: working_dir.to_path_buf(),
        event_bus,
        storage: None,
        agent_manager: None,
        active_model: None,
        provider_registry: None,
        team_context: None,
        team_manager: None,
        code_index: None,
        spec_manager: None,
        active_spec_id: None,
        config: None,
        bg_service: None,
        cached_team_dir: std::sync::Arc::new(std::sync::Mutex::new(None)),
        permission_checker: None,
        tool_registry: ToolContext::default_tool_registry(),
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        canonical_cache: Arc::new(ragent_tools_core::CanonicalPathCache::new()),
        allowed_roots: Vec::new(),
    }
}

/// Seed a team whose members carry the given `(name, agent_id, status)` rows.
fn seed_team(dir: &std::path::Path, team: &str, members: &[(&str, &str, MemberStatus)]) {
    let mut store = TeamStore::create(team, "lead-sess", dir, true).expect("create team");
    for (name, agent_id, status) in members {
        let mut member = TeamMember::new(*name, *agent_id, "general");
        member.status = *status;
        store.add_member(member).expect("add_member");
    }
}

/// The `outcome` token recorded for `agent_id` in the tool metadata.
fn outcome_of(out: &ragent_agent::tool::ToolOutput, agent_id: &str) -> String {
    out.metadata
        .as_ref()
        .and_then(|m| m.get("outcomes"))
        .and_then(|o| o.get(agent_id))
        .and_then(|entry| entry.get("outcome"))
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| panic!("no outcome recorded for {agent_id}: {:?}", out.metadata))
        .to_string()
}

// -- one member per outcome ------------------------------------------------

/// A team where each awaited member ends in a different FR-031 category must
/// report all four distinctly in one call: `idle`, `parked`, `failed`, and
/// `still_running`.
#[tokio::test]
async fn team_wait_reports_each_outcome_category() {
    let (_tmp, dir) = setup_workspace();
    seed_team(
        &dir,
        "mixed-team",
        &[
            ("finisher", "tm-001", MemberStatus::Idle),
            ("pauser", "tm-002", MemberStatus::Suspended),
            ("crasher", "tm-003", MemberStatus::Failed),
            ("runner", "tm-004", MemberStatus::Working),
        ],
    );

    let bus = Arc::new(EventBus::new(64));
    let ctx = make_ctx(&dir, "lead-sess", bus);
    let tool = ragent_agent::tool::team_wait::TeamWaitTool;

    // Only the Working member keeps the wait open, so the timeout applies to it
    // and nothing else.
    let out = tool
        .execute(
            json!({
                "team_name": "mixed-team",
                "agent_ids": ["tm-001", "tm-002", "tm-003", "tm-004"],
                "timeout_secs": 1
            }),
            &ctx,
        )
        .await
        .expect("team_wait execute");

    assert_eq!(outcome_of(&out, "tm-001"), "idle");
    assert_eq!(outcome_of(&out, "tm-002"), "parked");
    assert_eq!(outcome_of(&out, "tm-003"), "failed");
    assert_eq!(outcome_of(&out, "tm-004"), "still_running");
}

/// The rendered report names every awaited teammate with its outcome word, so
/// an operator reading the text (not the metadata) can still tell a parked
/// teammate from a wedged one.
#[tokio::test]
async fn team_wait_text_report_names_each_outcome() {
    let (_tmp, dir) = setup_workspace();
    seed_team(
        &dir,
        "report-team",
        &[
            ("pauser", "tm-001", MemberStatus::Suspended),
            ("runner", "tm-002", MemberStatus::Working),
        ],
    );

    let bus = Arc::new(EventBus::new(64));
    let ctx = make_ctx(&dir, "lead-sess", bus);
    let tool = ragent_agent::tool::team_wait::TeamWaitTool;

    let out = tool
        .execute(
            json!({ "team_name": "report-team", "timeout_secs": 1 }),
            &ctx,
        )
        .await
        .expect("team_wait execute");

    assert!(
        out.content.contains("## Wait outcome"),
        "report needs an outcome block: {}",
        out.content
    );
    assert!(
        out.content.contains("parked"),
        "a suspended teammate must be reported as parked: {}",
        out.content
    );
    assert!(
        out.content.contains("still running"),
        "the Working teammate must be reported as still running: {}",
        out.content
    );
    // The parked teammate is named with the status word the rest of the UI uses.
    assert!(
        out.content.contains("pauser") && out.content.contains("suspended"),
        "the parked row must name the member and its status: {}",
        out.content
    );
}

// -- F-17: a parked member must not consume the wait budget ----------------

/// A team whose only member is parked must return immediately, well inside the
/// timeout. Before T-034 `Suspended` and `PlanPending` were in the waitable set,
/// so this call burned the whole budget and then mislabelled the pause as work
/// still in progress (F-17).
#[tokio::test]
async fn team_wait_does_not_block_on_a_parked_member() {
    let (_tmp, dir) = setup_workspace();
    seed_team(
        &dir,
        "parked-team",
        &[("pauser", "tm-001", MemberStatus::Suspended)],
    );

    let bus = Arc::new(EventBus::new(64));
    let ctx = make_ctx(&dir, "lead-sess", bus);
    let tool = ragent_agent::tool::team_wait::TeamWaitTool;

    let started = std::time::Instant::now();
    let out = tool
        .execute(
            json!({ "team_name": "parked-team", "timeout_secs": 30 }),
            &ctx,
        )
        .await
        .expect("team_wait execute");
    let elapsed = started.elapsed();

    assert_eq!(outcome_of(&out, "tm-001"), "parked");
    assert_eq!(
        out.metadata.as_ref().and_then(|m| m.get("timed_out")),
        Some(&json!(false)),
        "a parked teammate is settled, not a timeout"
    );
    assert!(
        elapsed < std::time::Duration::from_secs(5),
        "waiting on a parked teammate must not consume the budget; took {elapsed:?}"
    );
}

/// The same rule holds mid-wait: a teammate that is suspended while the lead is
/// blocked in `team_wait` settles the wait instead of holding it to the deadline.
#[tokio::test]
async fn team_wait_settles_when_a_member_is_parked_mid_wait() {
    let (_tmp, dir) = setup_workspace();
    seed_team(
        &dir,
        "midpark-team",
        &[("runner", "tm-001", MemberStatus::Working)],
    );

    let bus = Arc::new(EventBus::new(64));
    let bus2 = Arc::clone(&bus);
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        bus2.publish(ragent_agent::event::Event::TeammateSuspended {
            session_id: "lead-sess".to_string(),
            team_name: "midpark-team".to_string(),
            agent_id: "tm-001".to_string(),
        });
    });

    let ctx = make_ctx(&dir, "lead-sess", bus);
    let tool = ragent_agent::tool::team_wait::TeamWaitTool;

    let started = std::time::Instant::now();
    let out = tool
        .execute(
            json!({ "team_name": "midpark-team", "timeout_secs": 30 }),
            &ctx,
        )
        .await
        .expect("team_wait execute");
    let elapsed = started.elapsed();

    assert_eq!(
        out.metadata.as_ref().and_then(|m| m.get("timed_out")),
        Some(&json!(false)),
        "a mid-wait suspension settles the wait"
    );
    assert!(
        elapsed < std::time::Duration::from_secs(5),
        "the suspension should end the wait promptly; took {elapsed:?}"
    );
}

// -- parked variants are all reported as parked ----------------------------

/// `PlanPending` and `Blocked` are parked exactly like `Suspended`: the report
/// groups them under `parked` while the status word names which kind of park it
/// was. This is what makes "planning" distinguishable from "stuck".
#[tokio::test]
async fn team_wait_reports_all_parked_variants_as_parked() {
    let (_tmp, dir) = setup_workspace();
    seed_team(
        &dir,
        "parked-kinds-team",
        &[
            ("planner", "tm-001", MemberStatus::PlanPending),
            ("blocked", "tm-002", MemberStatus::Blocked),
            ("pauser", "tm-003", MemberStatus::Suspended),
        ],
    );

    let bus = Arc::new(EventBus::new(64));
    let ctx = make_ctx(&dir, "lead-sess", bus);
    let tool = ragent_agent::tool::team_wait::TeamWaitTool;

    let out = tool
        .execute(
            json!({ "team_name": "parked-kinds-team", "timeout_secs": 30 }),
            &ctx,
        )
        .await
        .expect("team_wait execute");

    for id in ["tm-001", "tm-002", "tm-003"] {
        assert_eq!(
            outcome_of(&out, id),
            "parked",
            "member {id} should be parked"
        );
    }
    // Each keeps its own status word in `status`, so nothing is flattened to a
    // generic "parked" and lost.
    let statuses = ["plan_pending", "blocked", "suspended"];
    for (id, expected) in ["tm-001", "tm-002", "tm-003"].iter().zip(statuses) {
        let recorded = out
            .metadata
            .as_ref()
            .and_then(|m| m.get("outcomes"))
            .and_then(|o| o.get(id))
            .and_then(|e| e.get("status"))
            .and_then(|v| v.as_str());
        assert_eq!(recorded, Some(expected), "member {id} status word");
    }
}

// -- timeout still reports the runner as still running --------------------

/// A genuinely wedged teammate is still a timeout, and the call must say so
/// while naming the member that kept it waiting.
#[tokio::test]
async fn team_wait_timeout_names_the_still_running_member() {
    let (_tmp, dir) = setup_workspace();
    seed_team(
        &dir,
        "stuck-team",
        &[("stuck", "tm-001", MemberStatus::Working)],
    );

    let bus = Arc::new(EventBus::new(64));
    let ctx = make_ctx(&dir, "lead-sess", bus);
    let tool = ragent_agent::tool::team_wait::TeamWaitTool;

    let out = tool
        .execute(
            json!({ "team_name": "stuck-team", "timeout_secs": 1 }),
            &ctx,
        )
        .await
        .expect("team_wait execute");

    assert_eq!(
        out.metadata.as_ref().and_then(|m| m.get("timed_out")),
        Some(&json!(true)),
        "an unsettled live member is a timeout"
    );
    assert_eq!(outcome_of(&out, "tm-001"), "still_running");
    assert!(
        out.content.contains("tm-001"),
        "the timeout message must name the member: {}",
        out.content
    );
    assert_eq!(
        out.metadata
            .as_ref()
            .and_then(|m| m.get("still_working"))
            .and_then(|v| v.as_array())
            .map(|a| a.len()),
        Some(1)
    );
}

/// A teammate that fails mid-wait settles the call rather than running out the
/// clock, and is reported as `failed` rather than as still running.
#[tokio::test]
async fn team_wait_reports_a_mid_wait_failure_as_failed() {
    let (_tmp, dir) = setup_workspace();
    seed_team(
        &dir,
        "midfail-team",
        &[("crasher", "tm-001", MemberStatus::Working)],
    );

    let bus = Arc::new(EventBus::new(64));
    let bus2 = Arc::clone(&bus);
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        // The manager marks the member Failed on disk before publishing, so the
        // post-loop reconcile and the event agree on the outcome.
        bus2.publish(ragent_agent::event::Event::TeammateFailed {
            session_id: "lead-sess".to_string(),
            team_name: "midfail-team".to_string(),
            agent_id: "tm-001".to_string(),
            error: "simulated crash".to_string(),
        });
    });

    // Mark the member Failed on disk up-front so the reported status is the
    // failure the event announces.
    {
        let mut store = TeamStore::load_by_name("midfail-team", &dir).expect("load team");
        if let Some(m) = store.config.member_by_id_mut("tm-001") {
            m.status = MemberStatus::Failed;
        }
        store.save().expect("save failed status");
    }

    let ctx = make_ctx(&dir, "lead-sess", bus);
    let tool = ragent_agent::tool::team_wait::TeamWaitTool;

    let out = tool
        .execute(
            json!({ "team_name": "midfail-team", "timeout_secs": 30 }),
            &ctx,
        )
        .await
        .expect("team_wait execute");

    assert_eq!(outcome_of(&out, "tm-001"), "failed");
    assert_eq!(
        out.metadata.as_ref().and_then(|m| m.get("timed_out")),
        Some(&json!(false)),
        "a failed member settles the wait"
    );
}
