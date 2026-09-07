//! External tests for `session/loop_state.rs` (spec `agentloop`, task
//! T-001): `LoopSpec` restriction predicates, `StopCondition` mapping, and
//! `LoopTracker` budget-gate behaviour (FR-006, FR-013, FR-014).
//!
//! The module is also compiled inline (via `#[path]` include in the source)
//! for private-field access; the tests here run through the public API.

// Pedantic-level warnings on collection-emptiness asserts are accepted in
// this suite (same treatment as the pre-agentloop test files).
#![allow(clippy::assert_is_empty)]

use ragent_agent::session::loop_state::{LoopSpec, LoopTracker, StopCondition};

// --- LoopSpec construction -------------------------------------------------

#[test]
fn test_loop_spec_new_defaults_checkpoints_on_and_no_limits() {
    let spec = LoopSpec::new("coder", "all existing tests should pass");
    assert_eq!(spec.agent, "coder");
    assert_eq!(spec.goal, "all existing tests should pass");
    assert!(spec.verify_cmd.is_none());
    assert!(spec.scope.is_empty());
    assert!(spec.read_only.is_empty());
    assert!(spec.tool_set.is_empty());
    assert!(spec.max_steps.is_none());
    assert!(spec.cost_limit.is_none());
    // Checkpoints default on (destructive-action safety, FR-015).
    assert!(spec.checkpoints);
}

#[test]
fn test_loop_spec_has_scope_and_has_tool_set() {
    let mut spec = LoopSpec::new("coder", "goal");
    assert!(!spec.has_scope());
    assert!(!spec.has_tool_set());

    spec.scope = vec!["src/**".to_string()];
    spec.tool_set = vec!["read".to_string()];
    assert!(spec.has_scope());
    assert!(spec.has_tool_set());
}

// --- StopCondition mapping -------------------------------------------------

#[test]
fn test_stop_condition_labels_round_trip() {
    for (condition, label) in [
        (StopCondition::GoalAchieved, "completed"),
        (StopCondition::UnrecoverableError, "error"),
        (StopCondition::BudgetExhausted, "budget_exhausted"),
        (StopCondition::HumanIntervention, "interrupted"),
    ] {
        assert_eq!(condition.as_str(), label);
        assert_eq!(StopCondition::from_str_label(label), Some(condition));
    }
    assert_eq!(StopCondition::from_str_label("nonsense"), None);
}

#[test]
fn test_stop_condition_success_only_goal_achieved() {
    assert!(StopCondition::GoalAchieved.is_success());
    assert!(!StopCondition::UnrecoverableError.is_success());
    assert!(!StopCondition::BudgetExhausted.is_success());
    assert!(!StopCondition::HumanIntervention.is_success());
}

// --- LoopTracker: budget gates (FR-013 / FR-014) ---------------------------

#[test]
fn test_tracker_step_budget_breach_before_request() {
    let mut tracker = LoopTracker::with_budgets(Some(2), None);
    // Two iterations fit within the budget.
    assert!(tracker.begin_step());
    assert!(tracker.begin_step());
    assert_eq!(tracker.steps(), 2);
    // FR-013: the gate fires BEFORE a third request is sent.
    assert_eq!(
        tracker.budget_breach(),
        Some(StopCondition::BudgetExhausted)
    );
    assert!(!tracker.begin_step());
    assert_eq!(tracker.steps(), 2);
}

#[test]
fn test_tracker_token_budget_breach_before_request() {
    let mut tracker = LoopTracker::with_budgets(None, Some(1500));
    assert!(tracker.budget_breach().is_none());
    tracker.record_tokens(700, 300); // 1000 total, under the limit
    assert!(tracker.budget_breach().is_none());
    tracker.record_tokens(600, 0); // 1600 total, breaches 1500
    // FR-014: gate fires before the next request.
    assert_eq!(
        tracker.budget_breach(),
        Some(StopCondition::BudgetExhausted)
    );
    assert!(!tracker.begin_step());
}

#[test]
fn test_tracker_unbounded_budgets_never_breach() {
    let mut tracker = LoopTracker::with_budgets(None, None);
    for _ in 0..100 {
        assert!(tracker.begin_step());
    }
    assert!(tracker.budget_breach().is_none());
    tracker.record_tokens(u64::MAX, 10); // saturating, must not panic
    assert!(tracker.budget_breach().is_none());
}

#[test]
fn test_tracker_stop_is_idempotent_first_condition_wins() {
    let mut tracker = LoopTracker::with_budgets(Some(5), None);
    assert!(tracker.stop_condition().is_none());
    assert!(!tracker.is_stopped());
    let first = tracker.stop(StopCondition::BudgetExhausted);
    assert_eq!(first, StopCondition::BudgetExhausted);
    assert!(tracker.is_stopped());
    // FR-017: once stopped, a later condition cannot override.
    let second = tracker.stop(StopCondition::HumanIntervention);
    assert_eq!(second, StopCondition::BudgetExhausted);
    assert_eq!(
        tracker.stop_condition(),
        Some(StopCondition::BudgetExhausted)
    );
    // No steps may begin after the stop flag is set.
    assert!(!tracker.begin_step());
}

#[test]
fn test_tracker_consecutive_failures_stop_at_allowance() {
    let mut tracker = LoopTracker::with_budgets(None, None);
    // Allowance is 3: three failures in a row are still tolerated.
    assert!(!tracker.record_failure());
    assert!(!tracker.record_failure());
    assert!(!tracker.record_failure());
    assert_eq!(tracker.consecutive_failures(), 3);
    // The fourth consecutive failure exceeds the allowance.
    assert!(tracker.record_failure());
    // A success resets the counter.
    tracker.record_success();
    assert_eq!(tracker.consecutive_failures(), 0);
    assert!(!tracker.record_failure());
}

// --- LoopTracker from LoopSpec ---------------------------------------------

#[test]
fn test_tracker_new_applies_spec_budgets() {
    let spec = LoopSpec {
        max_steps: Some(7),
        cost_limit: Some(20000),
        ..LoopSpec::new("coder", "goal")
    };
    let mut tracker = LoopTracker::new(&spec);
    assert_eq!(tracker.steps(), 0);
    assert!(tracker.budget_breach().is_none());
    for _ in 0..7 {
        assert!(tracker.begin_step());
    }
    assert_eq!(
        tracker.budget_breach(),
        Some(StopCondition::BudgetExhausted)
    );
    tracker.record_tokens(20_000, 0);
    // Step budget fires first but both gates point at the same condition.
    assert_eq!(tracker.stop_condition(), None);
    let breach = tracker.budget_breach().unwrap();
    assert_eq!(breach, StopCondition::BudgetExhausted);
    tracker.stop(breach);
    assert!(tracker.is_stopped());
}

// --- Tool-call tally (FR-025, T-017) ----------------------------------------

#[test]
fn test_tracker_tool_call_tally_accumulates_per_iteration() {
    let mut tracker = LoopTracker::with_budgets(Some(5), None);
    assert_eq!(tracker.tool_calls(), 0);
    // Two iterations of tool work: two calls in the first, one in the second.
    assert!(tracker.begin_step());
    tracker.record_tool_calls(2);
    assert!(tracker.begin_step());
    tracker.record_tool_calls(1);
    assert_eq!(tracker.tool_calls(), 3, "per-iteration counts accumulate");
    // The tally does not consume the step budget.
    assert!(tracker.budget_breach().is_none());
    assert_eq!(tracker.steps(), 2);
    // The tally is unaffected by stopping the loop.
    tracker.stop(StopCondition::GoalAchieved);
    assert_eq!(tracker.tool_calls(), 3);
}

// --- LoopSpec restriction predicates (used by T-009) ------------------------

#[test]
fn test_spec_allows_tool_with_and_without_tool_set() {
    let mut spec = LoopSpec::new("coder", "goal");
    // No tool set: every tool allowed (FR-008 "empty list means all tools").
    assert!(spec.allows_tool("read"));
    assert!(spec.allows_tool("bash"));

    spec.tool_set = vec!["read".to_string(), "grep".to_string()];
    assert!(spec.allows_tool("read"));
    assert!(spec.allows_tool("grep"));
    assert!(!spec.allows_tool("bash"));
    assert!(!spec.allows_tool("write"));
}

#[test]
fn test_spec_path_in_scope_globs() {
    let spec = LoopSpec {
        scope: vec!["src/**".to_string(), "*.md".to_string()],
        ..LoopSpec::new("coder", "goal")
    };
    assert!(spec.path_in_scope("src/main.rs"));
    assert!(spec.path_in_scope("src/agent/loop.rs"));
    assert!(spec.path_in_scope("README.md"));
    assert!(!spec.path_in_scope("target/debug/foo"));
    assert!(!spec.path_in_scope("docs/deep/nested/file.txt"));
}

#[test]
fn test_spec_path_in_scope_empty_means_everything() {
    let spec = LoopSpec::new("coder", "goal");
    assert!(spec.path_in_scope("anything/at/all.rs"));
    assert!(spec.path_in_scope("/etc/passwd"));
}

#[test]
fn test_spec_path_is_read_only_globs() {
    let spec = LoopSpec {
        read_only: vec!["tests/**".to_string()],
        ..LoopSpec::new("coder", "goal")
    };
    // FR-021: protected files such as tests cannot be written.
    assert!(spec.path_is_read_only("tests/test_foo.rs"));
    assert!(spec.path_is_read_only("tests/unit/bar.rs"));
    assert!(!spec.path_is_read_only("src/main.rs"));
    // No constraints configured: nothing is read-only.
    let unrestricted = LoopSpec::new("coder", "goal");
    assert!(!unrestricted.path_is_read_only("tests/test_foo.rs"));
}

#[test]
fn test_spec_invalid_glob_patterns_are_skipped_safely() {
    let spec = LoopSpec {
        scope: vec!["[unclosed".to_string(), "src/**".to_string()],
        ..LoopSpec::new("coder", "goal")
    };
    // The invalid pattern must not panic or match everything; the valid one
    // still applies.
    assert!(spec.path_in_scope("src/lib.rs"));
    assert!(!spec.path_in_scope("other/file.rs"));
}

// --- Serde round trip -------------------------------------------------------

#[test]
fn test_loop_spec_serde_round_trip() {
    let spec = LoopSpec {
        verify_cmd: Some("cargo test".to_string()),
        scope: vec!["src/**".to_string()],
        read_only: vec!["tests/**".to_string()],
        tool_set: vec!["read".to_string(), "edit".to_string()],
        max_steps: Some(25),
        cost_limit: Some(100_000),
        checkpoints: false,
        checkpoint_timeout_secs: Some(45),
        ..LoopSpec::new("coder", "make the tests pass")
    };
    let json = serde_json::to_string(&spec).expect("serialize");
    let back: LoopSpec = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, spec);
}
