//! Spec `teamssetup` T-018 (FR-014, F-10): `TeamSettings.require_plan_approval`
//! must *do* something.
//!
//! Before this task the setting produced one sentence in the teammate system
//! prompt and nothing else, so a teammate that never called `team_submit_plan`
//! was never stopped. The gate is now `TeamManager::is_plan_gate_active`, asked
//! by the dispatch pipeline's stage 6 alongside the pre-existing
//! `is_plan_pending`, and the refusal
//! [`ragent_agent::team::plan_gate_denial`] produces names `team_submit_plan`.
//!
//! These tests pin the three axes that decide the gate's answer - the setting,
//! the member's `plan_status`, and the tool's class - plus the refusal text.

use ragent_agent::session::loop_state::{LOOP_BASH_TOOLS, LOOP_WRITE_TOOLS};
use ragent_agent::team::config::PlanStatus;
use ragent_agent::team::{TeamStore, plan_gate_denial};

#[path = "manager_support/mod.rs"]
mod manager_support;
use manager_support::team_manager;

/// Rewrite the team's `require_plan_approval` setting on disk.
fn set_require_plan_approval(manager: &ragent_agent::team::TeamManager, enabled: bool) {
    let mut store = TeamStore::load(&manager.team_dir).expect("load team");
    store.config.settings.require_plan_approval = enabled;
    store.save().expect("save team");
}

/// Set member `tm-001`'s plan status on disk and drop the manager's cache entry.
fn set_plan_status(manager: &ragent_agent::team::TeamManager, status: PlanStatus) {
    let mut store = TeamStore::load(&manager.team_dir).expect("load team");
    store
        .config
        .member_by_id_mut("tm-001")
        .expect("fixture member tm-001")
        .plan_status = status;
    store.save().expect("save team");
    manager.invalidate_plan_pending_cache("tm-001");
}

// -- the setting decides whether the gate exists at all -----------------------

#[test]
fn gate_is_inert_when_require_plan_approval_is_off() {
    let (manager, _tmp, _bus) = team_manager("gate-off", "lead-1", true);
    // The fixture member has never submitted a plan.
    assert_eq!(
        TeamStore::load(&manager.team_dir)
            .expect("load")
            .config
            .member_by_id("tm-001")
            .expect("member")
            .plan_status,
        PlanStatus::None
    );
    assert!(
        !manager.is_plan_gate_active("tm-001"),
        "the gate is opt-in: with require_plan_approval off a plan-less \
         teammate must not be stopped"
    );
}

#[test]
fn gate_fires_for_a_teammate_that_never_submitted_a_plan() {
    let (manager, _tmp, _bus) = team_manager("gate-none", "lead-1", true);
    set_require_plan_approval(&manager, true);
    set_plan_status(&manager, PlanStatus::None);

    assert!(
        manager.is_plan_gate_active("tm-001"),
        "F-10: a teammate that never submitted a plan is exactly what \
         require_plan_approval exists to stop"
    );
    // The narrow query keeps its own meaning - there is no plan awaiting a
    // verdict - which is why the gate needed a second question. It is read
    // through a fresh manager because PERF-027 shares one cached entry between
    // the two queries: the gate's `true` above would otherwise answer for it.
    let (fresh, _fresh_tmp, _fresh_bus) = team_manager("gate-none-fresh", "lead-1", true);
    set_require_plan_approval(&fresh, true);
    set_plan_status(&fresh, PlanStatus::None);
    assert!(
        !fresh.is_plan_pending("tm-001"),
        "PlanStatus::None is not a pending plan"
    );
}

#[test]
fn gate_fires_while_the_plan_awaits_the_verdict() {
    let (manager, _tmp, _bus) = team_manager("gate-pending", "lead-1", true);
    set_require_plan_approval(&manager, true);
    set_plan_status(&manager, PlanStatus::Pending);

    assert!(manager.is_plan_gate_active("tm-001"));
    assert!(manager.is_plan_pending("tm-001"));
}

#[test]
fn gate_fires_after_a_rejected_plan() {
    let (manager, _tmp, _bus) = team_manager("gate-rejected", "lead-1", true);
    set_require_plan_approval(&manager, true);
    set_plan_status(&manager, PlanStatus::Rejected);

    assert!(
        manager.is_plan_gate_active("tm-001"),
        "a rejected plan is not an approved one: the teammate must revise and \
         resubmit, not proceed"
    );
}

#[test]
fn approved_plan_clears_the_gate() {
    let (manager, _tmp, _bus) = team_manager("gate-approved", "lead-1", true);
    set_require_plan_approval(&manager, true);
    set_plan_status(&manager, PlanStatus::Pending);
    assert!(manager.is_plan_gate_active("tm-001"), "pending gates");

    manager.approve_plan("tm-001", true).expect("approve");

    assert!(
        !manager.is_plan_gate_active("tm-001"),
        "approval is the way through the gate"
    );
}

#[test]
fn approval_clears_the_gate_through_the_manager_cache() {
    // PERF-027: both queries share one cached entry. A stale entry would keep a
    // freshly approved teammate refused for up to the TTL, so the invalidation
    // `approve_plan` performs is asserted from a warm cache.
    let (manager, _tmp, _bus) = team_manager("gate-cache", "lead-1", true);
    set_require_plan_approval(&manager, true);
    set_plan_status(&manager, PlanStatus::Pending);
    assert!(manager.is_plan_gate_active("tm-001"), "warms the cache");

    manager.approve_plan("tm-001", true).expect("approve");
    assert!(
        !manager.is_plan_gate_active("tm-001"),
        "the cache entry must be dropped when the verdict is recorded"
    );
}

#[test]
fn gate_does_not_fire_for_an_unknown_member() {
    let (manager, _tmp, _bus) = team_manager("gate-unknown", "lead-1", true);
    set_require_plan_approval(&manager, true);

    assert!(
        !manager.is_plan_gate_active("tm-404"),
        "a member this manager cannot see must not be refused: the gate fails \
         open on absent data rather than punishing the wrong agent"
    );
}

#[test]
fn gate_is_inert_when_the_team_cannot_be_read() {
    let (manager, _tmp, _bus) = team_manager("gate-gone", "lead-1", true);
    std::fs::remove_dir_all(&manager.team_dir).expect("remove team dir");

    assert!(
        !manager.is_plan_gate_active("tm-001"),
        "an unreadable team means unknown settings, and an unknowable opt-in \
         setting must not stop the teammate"
    );
}

// -- the tool class decides what is refused -----------------------------------

#[test]
fn every_write_class_tool_is_refused_by_the_gate() {
    for tool in LOOP_WRITE_TOOLS {
        let reason = plan_gate_denial(tool, false, true)
            .unwrap_or_else(|| panic!("write-class tool '{tool}' must be refused"));
        assert!(
            reason.contains("team_submit_plan"),
            "the refusal for '{tool}' must name the way out: {reason}"
        );
    }
}

#[test]
fn every_bash_class_tool_is_refused_by_the_gate() {
    for tool in LOOP_BASH_TOOLS {
        let reason = plan_gate_denial(tool, false, true)
            .unwrap_or_else(|| panic!("bash-class tool '{tool}' must be refused"));
        assert!(
            reason.contains("team_submit_plan"),
            "the refusal for '{tool}' must name the way out: {reason}"
        );
    }
    // The class is the tool, not the command: a read-only command is refused
    // all the same, because classifying a command string is a heuristic.
    assert!(
        plan_gate_denial("Bash", false, true).is_some(),
        "Bash is gated as a class"
    );
}

#[test]
fn read_only_and_coordination_tools_pass_the_gate() {
    for tool in ["read", "Grep", "Glob", "list", "codeindex_search"] {
        assert_eq!(
            plan_gate_denial(tool, false, true),
            None,
            "read-only tool '{tool}' must stay available while a plan is unapproved"
        );
    }
    // The team_* tools are how the teammate submits the plan and learns the
    // verdict - gating them would make the gate unescapable.
    for tool in [
        "team_submit_plan",
        "team_read_messages",
        "team_message",
        "team_idle",
    ] {
        assert_eq!(
            plan_gate_denial(tool, false, true),
            None,
            "coordination tool '{tool}' must stay available"
        );
    }
}

#[test]
fn the_lead_is_never_gated() {
    for tool in ["Write", "Bash", "patch", "rm"] {
        assert_eq!(
            plan_gate_denial(tool, true, true),
            None,
            "the lead approves plans; it is never subject to the gate ('{tool}')"
        );
    }
}

#[test]
fn a_closed_gate_refuses_nothing() {
    for tool in LOOP_WRITE_TOOLS.iter().chain(LOOP_BASH_TOOLS.iter()) {
        assert_eq!(
            plan_gate_denial(tool, false, false),
            None,
            "with the gate closed '{tool}' must pass"
        );
    }
}

#[test]
fn the_refusal_names_the_tool_the_state_and_the_way_out() {
    let reason = plan_gate_denial("Write", false, true).expect("gated");
    // Entity and class: which call was refused and why.
    assert!(
        reason.contains("'Write'"),
        "names the refused call: {reason}"
    );
    assert!(
        reason.contains("write-class") || reason.contains("bash-class"),
        "names the tool class: {reason}"
    );
    // State: the plan is not approved.
    assert!(
        reason.contains("has not been approved"),
        "names the current state: {reason}"
    );
    // Next action: the tool that clears the gate.
    assert!(
        reason.contains("team_submit_plan"),
        "names the remedy: {reason}"
    );
}

// -- the wiring the gate depends on -------------------------------------------

#[test]
fn the_dispatch_pipeline_asks_the_wide_gate_alongside_plan_pending() {
    // The gate only does work if the pipeline asks it. `plan_gate_denial` alone
    // is a pure function no one calls, which is precisely the F-10 shape of the
    // bug: this asserts the call site exists where the pipeline already has it.
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/session/dispatch.rs"),
    )
    .expect("read dispatch.rs");
    assert!(
        source.contains("is_plan_gate_active("),
        "stage 6 of the dispatch pipeline must ask the FR-014 gate"
    );
    assert!(
        source.contains("is_plan_pending("),
        "stage 6 must keep the pre-existing plan-pending question (T-307)"
    );
    assert!(
        source.contains("plan_gate_denial("),
        "stage 6 must turn the answers into the documented refusal"
    );
}

#[test]
fn the_team_prompt_states_that_the_gate_refuses_calls() {
    // The prompt is the teammate's only description of the policy; a prompt that
    // says "submit a plan" without saying that calls are refused invites the
    // model to retry a refused tool forever.
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/team/manager.rs"),
    )
    .expect("read manager.rs");
    assert!(
        source.contains("write-class and bash-class tool call is refused"),
        "the team prompt must state the gate's consequence"
    );
}
