//! Unit tests for the `/loop` setup dialog state machine (spec `agentloop`,
//! task T-014): field navigation, comma-list parsing, and spec composition
//! (FR-002, FR-005). Dialog integration tests live in
//! `crates/ragent-tui/tests/test_loop_dialog.rs`.

use crate::app::loop_dialog::{
    LoopSetupField, LoopSetupState, build_spec_from_state, parse_comma_list, parse_optional_u64,
};
use ragent_config::LoopConfig;

fn dialog_state() -> LoopSetupState {
    LoopSetupState::new(
        vec![
            ("general".to_string(), "General assistant".to_string()),
            ("coder".to_string(), "Coding agent".to_string()),
        ],
        0,
    )
}

#[test]
fn test_field_cycle_covers_all_nine_fields() {
    let mut field = LoopSetupField::Agent;
    let mut seen = vec![field];
    for _ in 0..8 {
        field = field.next();
        seen.push(field);
    }
    assert_eq!(seen.len(), 9, "nine navigable fields");
    assert_eq!(field.next(), LoopSetupField::Agent, "wraps to first");
    assert_eq!(
        field.previous(),
        LoopSetupField::CostLimit,
        "previous of first is last"
    );
    assert!(!LoopSetupField::Agent.is_text());
    assert!(!LoopSetupField::Checkpoints.is_text());
    assert!(LoopSetupField::Goal.is_text());
    assert!(LoopSetupField::CostLimit.is_text());
}

#[test]
fn test_field_previous_walks_backwards() {
    let field = LoopSetupField::Goal;
    assert_eq!(field.previous(), LoopSetupField::Agent);
    assert_eq!(LoopSetupField::MaxSteps.previous(), LoopSetupField::ToolSet);
    assert_eq!(
        LoopSetupField::Checkpoints.previous(),
        LoopSetupField::CostLimit
    );
}

#[test]
fn test_agent_picker_wraps_and_defaults() {
    let mut state = dialog_state();
    assert_eq!(state.selected_agent_name(), "general");
    state.select_next_agent();
    assert_eq!(state.selected_agent_name(), "coder");
    state.select_next_agent();
    assert_eq!(state.selected_agent_name(), "general", "wraps forward");
    state.select_previous_agent();
    assert_eq!(state.selected_agent_name(), "coder", "wraps backward");
    // Out-of-range default index is clamped.
    let clamped = LoopSetupState::new(vec![("general".to_string(), "g".to_string())], 5);
    assert_eq!(clamped.selected_agent_name(), "general");
    let empty = LoopSetupState::new(Vec::new(), 0);
    assert_eq!(empty.selected_agent_name(), "");
    // Navigation on an empty list is a no-op.
    let mut empty = empty;
    empty.select_next_agent();
    empty.select_previous_agent();
    assert_eq!(empty.selected_agent_name(), "");
}

#[test]
fn test_parse_comma_list_trims_and_deduplicates() {
    assert_eq!(parse_comma_list("a, b ,c"), vec!["a", "b", "c"]);
    assert_eq!(parse_comma_list(" a ,, b,a "), vec!["a", "b"]);
    assert_eq!(parse_comma_list("  ,  "), Vec::<String>::new());
    assert_eq!(parse_comma_list(""), Vec::<String>::new());
}

#[test]
fn test_parse_optional_u64() {
    assert_eq!(parse_optional_u64("25"), Some(25));
    assert_eq!(parse_optional_u64(" 100 "), Some(100));
    assert_eq!(parse_optional_u64(""), None);
    assert_eq!(parse_optional_u64("abc"), None);
    assert_eq!(parse_optional_u64("-5"), None);
}

#[test]
fn test_build_spec_applies_defaults_and_parsed_fields() {
    let mut state = dialog_state();
    state.active_field = LoopSetupField::Goal;
    state.goal_field.insert_str("make the failing test pass");
    state.verify_cmd_field.insert_str("cargo test");
    state.scope_field.insert_str("src/**, tests/**");
    state.read_only_field.insert_str("tests/**");
    state.tool_set_field.insert_str("read, edit, bash");
    state.max_steps_field.insert_str("40");
    state.cost_limit_field.insert_str("5000");
    state.checkpoints = false;

    let defaults = LoopConfig {
        cost_limit: Some(123_456),
        ..LoopConfig::default()
    };
    let spec = build_spec_from_state(&state, &defaults).expect("goal present so spec builds");
    assert_eq!(spec.agent, "general");
    assert_eq!(spec.goal, "make the failing test pass");
    assert_eq!(spec.verify_cmd.as_deref(), Some("cargo test"));
    assert_eq!(spec.scope, vec!["src/**", "tests/**"]);
    assert_eq!(spec.read_only, vec!["tests/**"]);
    assert_eq!(spec.tool_set, vec!["read", "edit", "bash"]);
    assert_eq!(spec.max_steps, Some(40));
    assert_eq!(spec.cost_limit, Some(5000), "entered value wins");
    assert!(!spec.checkpoints);
}

#[test]
fn test_build_spec_blank_limits_use_config_defaults() {
    let mut state = dialog_state();
    state.goal_field.insert_str("ship the feature");
    state.select_next_agent();
    let defaults = LoopConfig {
        cost_limit: Some(42_000),
        ..LoopConfig::default()
    };
    let spec = build_spec_from_state(&state, &defaults).expect("spec builds");
    assert_eq!(spec.agent, "coder");
    assert_eq!(spec.max_steps, Some(defaults.max_steps));
    assert_eq!(spec.cost_limit, Some(42_000));
    assert!(spec.verify_cmd.is_none());
    assert_eq!(spec.scope, Vec::<String>::new());
    assert_eq!(spec.read_only, Vec::<String>::new());
    assert_eq!(spec.tool_set, Vec::<String>::new());
    assert!(spec.checkpoints, "checkpoints default on");
}

#[test]
fn test_build_spec_rejects_empty_goal_naming_the_field() {
    let state = dialog_state();
    let err = build_spec_from_state(&state, &LoopConfig::default())
        .expect_err("empty goal must be rejected");
    assert!(
        err.contains("goal"),
        "error must name the missing field: {err}"
    );
    let mut whitespace = dialog_state();
    whitespace.goal_field.insert_str("   ");
    let err = build_spec_from_state(&whitespace, &LoopConfig::default())
        .expect_err("whitespace-only goal rejected");
    assert!(err.contains("goal"), "error must name the field: {err}");
}
