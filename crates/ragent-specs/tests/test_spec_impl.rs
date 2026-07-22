//! Integration tests for the SpecImplRunner and PlanParser.
//!
//! Tests the full flow of parsing a PLAN.md, resolving execution order,
//! and constructing implementation prompts.

use ragent_specs::impl_runner::{
    build_blocked_summary, build_cancellation_summary, build_completion_summary,
    build_progress_update, find_dependents, parse_impl_args,
};
use ragent_specs::plan_parser::{Effort, PlanParser, PlanTask, Priority, resolve_execution_order};
use ragent_specs::spec::TaskStatus;

#[test]
fn test_full_parse_and_order() {
    let md = r#"
# SpecImpl — Implementation Plan

## Architecture

Some architecture text.

## Tasks

| ID | Title | Requirement | Effort | Priority | Dependencies |
|---|---|---|---|---|---|
| T-001 | Define types | FR-003, FR-004 | S | Critical | — |
| T-002 | Build parser | FR-003, FR-004 | M | Critical | T-001 |
| T-003 | DAG sort | FR-005, FR-006 | M | Critical | T-001 |
| T-004 | Status column | FR-019 | M | High | T-002 |
| T-005 | Runner | FR-007, FR-008 | L | Critical | T-002, T-003 |
| T-006 | Prompt construction | FR-021, FR-022 | S | Critical | T-001 |

## Task Details

### T-001 — Define PlanTask Struct

Details here.
"#;

    let tasks = PlanParser::parse(md).unwrap();
    assert_eq!(tasks.len(), 6);

    // Verify parsing
    assert_eq!(tasks[0].id, "T-001");
    assert_eq!(tasks[0].effort, Effort::S);
    assert_eq!(tasks[0].priority, Priority::Critical);
    assert!(tasks[0].dependencies.is_empty());

    assert_eq!(tasks[1].id, "T-002");
    assert_eq!(tasks[1].dependencies, vec!["T-001"]);

    assert_eq!(tasks[4].id, "T-005");
    assert_eq!(tasks[4].effort, Effort::L);
    assert_eq!(tasks[4].dependencies, vec!["T-002", "T-003"]);

    // Verify topological order
    let order = resolve_execution_order(&tasks).unwrap();
    assert_eq!(order.len(), 6);

    // T-001 must come before T-002, T-003, T-006
    let t001_pos = order.iter().position(|&i| tasks[i].id == "T-001").unwrap();
    let t002_pos = order.iter().position(|&i| tasks[i].id == "T-002").unwrap();
    let t003_pos = order.iter().position(|&i| tasks[i].id == "T-003").unwrap();
    let t004_pos = order.iter().position(|&i| tasks[i].id == "T-004").unwrap();
    let t005_pos = order.iter().position(|&i| tasks[i].id == "T-005").unwrap();
    let t006_pos = order.iter().position(|&i| tasks[i].id == "T-006").unwrap();

    assert!(t001_pos < t002_pos);
    assert!(t001_pos < t003_pos);
    assert!(t001_pos < t006_pos);
    assert!(t002_pos < t004_pos);
    assert!(t002_pos < t005_pos);
    assert!(t003_pos < t005_pos);
}

#[test]
fn test_resume_from_partial_completion() {
    let md = r#"
## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|---|---|---|---|---|---|---|
| T-001 | Define types | FR-003 | S | Critical | completed | — |
| T-002 | Build parser | FR-004 | M | Critical | completed | T-001 |
| T-003 | DAG sort | FR-005 | M | Critical | pending | T-001 |
| T-004 | Status column | FR-019 | M | High | pending | T-002 |
| T-005 | Runner | FR-007 | L | Critical | pending | T-002, T-003 |
"#;

    let tasks = PlanParser::parse(md).unwrap();
    assert_eq!(tasks.len(), 5);
    assert_eq!(tasks[0].status, TaskStatus::Completed);
    assert_eq!(tasks[1].status, TaskStatus::Completed);

    let order = resolve_execution_order(&tasks).unwrap();
    let resumed = ragent_specs::plan_parser::filter_for_resume(&tasks, &order);

    // T-001 and T-002 are completed and skipped.
    // T-003, T-004 and T-005 remain in topological order even if some
    // dependencies are still pending — the sequential driver will stop on any
    // blocked task before its dependents run.
    assert_eq!(resumed.len(), 3);
    let ids: Vec<&str> = resumed.iter().map(|&i| tasks[i].id.as_str()).collect();
    assert!(ids.contains(&"T-003"));
    assert!(ids.contains(&"T-004"));
    assert!(ids.contains(&"T-005"));
}

#[test]
fn test_cycle_detection_in_plan() {
    let md = r#"
## Tasks

| ID | Title | Requirement | Effort | Priority | Dependencies |
|---|---|---|---|---|---|
| T-001 | First | FR-001 | S | Critical | T-003 |
| T-002 | Second | FR-002 | M | High | T-001 |
| T-003 | Third | FR-003 | M | High | T-002 |
"#;

    let tasks = PlanParser::parse(md).unwrap();
    let result = resolve_execution_order(&tasks);
    assert!(result.is_err());
}

#[test]
fn test_parse_impl_args_all_variants() {
    // Basic
    let (name, opts) = parse_impl_args("myspec").unwrap();
    assert_eq!(name, "myspec");
    assert!(opts.task_id.is_none());
    assert!(!opts.dry_run);

    // With task
    let (name, opts) = parse_impl_args("myspec --task T-003").unwrap();
    assert_eq!(name, "myspec");
    assert_eq!(opts.task_id.as_deref(), Some("T-003"));

    // Dry run
    let (_name, opts) = parse_impl_args("myspec --dry-run").unwrap();
    assert!(opts.dry_run);

    // All options
    let (_name, opts) = parse_impl_args("myspec --task T-005 --dry-run").unwrap();
    assert_eq!(opts.task_id.as_deref(), Some("T-005"));
    assert!(opts.dry_run);

    // Empty
    assert!(parse_impl_args("").is_err());

    // Unknown option
    assert!(parse_impl_args("myspec --verbose").is_err());

    // Missing task ID
    assert!(parse_impl_args("myspec --task").is_err());
}

#[test]
fn test_progress_and_completion_messages() {
    let progress = build_progress_update("MySpec", "T-001", 3, 12, Some("T-002"));
    assert!(progress.contains("✅ T-001"));
    assert!(progress.contains("3/12"));
    assert!(progress.contains("Next: T-002"));

    let progress_last = build_progress_update("MySpec", "T-012", 12, 12, None);
    assert!(progress_last.contains("12/12"));
    assert!(!progress_last.contains("Next"));

    let completion = build_completion_summary("MySpec", 12);
    assert!(completion.contains("🎉"));
    assert!(completion.contains("implemented"));

    let cancel = build_cancellation_summary("MySpec", 5, 12);
    assert!(cancel.contains("⚠️"));
    assert!(cancel.contains("5/12"));
    assert!(cancel.contains("in_progress"));

    let blocked = build_blocked_summary("T-003", &["T-005".into(), "T-007".into()]);
    assert!(blocked.contains("🚫"));
    assert!(blocked.contains("T-003"));
    assert!(blocked.contains("T-005"));
}

#[test]
fn test_find_dependents_transitive() {
    let tasks = vec![
        PlanTask {
            id: "T-001".into(),
            title: "A".into(),
            requirement: "FR-001".into(),
            effort: Effort::S,
            priority: Priority::Critical,
            dependencies: vec![],
            status: TaskStatus::Pending,
        },
        PlanTask {
            id: "T-002".into(),
            title: "B".into(),
            requirement: "FR-002".into(),
            effort: Effort::S,
            priority: Priority::High,
            dependencies: vec!["T-001".into()],
            status: TaskStatus::Pending,
        },
        PlanTask {
            id: "T-003".into(),
            title: "C".into(),
            requirement: "FR-003".into(),
            effort: Effort::M,
            priority: Priority::High,
            dependencies: vec!["T-001".into()],
            status: TaskStatus::Pending,
        },
        PlanTask {
            id: "T-004".into(),
            title: "D".into(),
            requirement: "FR-004".into(),
            effort: Effort::L,
            priority: Priority::Medium,
            dependencies: vec!["T-002".into(), "T-003".into()],
            status: TaskStatus::Pending,
        },
    ];

    let deps = find_dependents(&tasks, "T-001");
    assert!(deps.contains(&"T-002".to_string()));
    assert!(deps.contains(&"T-003".to_string()));
    assert!(deps.contains(&"T-004".to_string()));

    // Only direct and transitive from T-002
    let deps = find_dependents(&tasks, "T-002");
    assert!(deps.contains(&"T-004".to_string()));
    assert!(!deps.contains(&"T-003".to_string()));
}

/// `SpecImplRunner::build_single_task_prompt` is the per-task prompt used by
/// the TUI's sequential `/spec impl` driver. It must name the task, its spec,
/// its position in the run, the requirement, and the `spec_task_update`
/// instruction with the correct spec_id/task_id.
#[test]
fn test_build_single_task_prompt_contains_required_fields() {
    use ragent_specs::impl_runner::SpecImplRunner;

    let task = PlanTask {
        id: "T-003".into(),
        title: "Build parser".into(),
        requirement: "FR-003, FR-004".into(),
        effort: Effort::M,
        priority: Priority::Critical,
        dependencies: vec!["T-001".into()],
        status: TaskStatus::Pending,
    };
    let prompt = SpecImplRunner::build_single_task_prompt(&task, "myspec", 2, 5);

    // Header identifies the task, title, and spec.
    assert!(prompt.contains("T-003"), "prompt must name the task id");
    assert!(
        prompt.contains("Build parser"),
        "prompt must name the title"
    );
    assert!(prompt.contains("myspec"), "prompt must name the spec");

    // Position in the run.
    assert!(
        prompt.contains("task 2 of 5"),
        "prompt must state rank/total"
    );

    // Requirement is included.
    assert!(
        prompt.contains("FR-003"),
        "prompt must include requirement refs"
    );
    assert!(prompt.contains("FR-004"));

    // spec_task_update instruction with correct spec_id and task_id.
    assert!(prompt.contains("spec_task_update"));
    assert!(prompt.contains("spec_id=\"myspec\""));
    assert!(prompt.contains("task_id=\"T-003\""));
    assert!(prompt.contains("status=\"completed\""));
    assert!(
        prompt.contains("blocked"),
        "prompt must mention blocked fallback"
    );
}

/// `build_single_task_prompt` should not depend on the task's dependencies
/// field (it is the prompt for implementing THIS task, not its dependents).
#[test]
fn test_build_single_task_prompt_independent_of_dependencies() {
    use ragent_specs::impl_runner::SpecImplRunner;

    let task_no_deps = PlanTask {
        id: "T-001".into(),
        title: "Define types".into(),
        requirement: "FR-001".into(),
        effort: Effort::S,
        priority: Priority::Critical,
        dependencies: vec![],
        status: TaskStatus::Pending,
    };
    let task_with_deps = PlanTask {
        id: "T-001".into(),
        title: "Define types".into(),
        requirement: "FR-001".into(),
        effort: Effort::S,
        priority: Priority::Critical,
        dependencies: vec!["T-000".into()],
        status: TaskStatus::Pending,
    };

    let p1 = SpecImplRunner::build_single_task_prompt(&task_no_deps, "s", 1, 1);
    let p2 = SpecImplRunner::build_single_task_prompt(&task_with_deps, "s", 1, 1);
    // The per-task prompt is identical regardless of dependencies — the
    // driver only guarantees the task's own deps are already completed by
    // the time it is dispatched.
    assert_eq!(p1, p2);
}
