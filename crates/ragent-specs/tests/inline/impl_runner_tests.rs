//! Inline tests for `impl_runner.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;
use crate::plan_parser::{Effort, Priority};
use crate::spec::TaskStatus;

#[test]
fn test_impl_options_default() {
    let opts = ImplOptions::default();
    assert!(opts.task_id.is_none());
    assert!(!opts.dry_run);
}

#[test]
fn test_impl_options_builder() {
    let opts = ImplOptions::new().with_task("T-003").with_dry_run();
    assert_eq!(opts.task_id.as_deref(), Some("T-003"));
    assert!(opts.dry_run);
}

#[test]
fn test_parse_impl_args_basic() {
    let (name, opts) = parse_impl_args("myspec").unwrap();
    assert_eq!(name, "myspec");
    assert!(opts.task_id.is_none());
    assert!(!opts.dry_run);
}

#[test]
fn test_parse_impl_args_with_task() {
    let (name, opts) = parse_impl_args("myspec --task T-003").unwrap();
    assert_eq!(name, "myspec");
    assert_eq!(opts.task_id.as_deref(), Some("T-003"));
}

#[test]
fn test_parse_impl_args_dry_run() {
    let (name, opts) = parse_impl_args("myspec --dry-run").unwrap();
    assert_eq!(name, "myspec");
    assert!(opts.dry_run);
}

#[test]
fn test_parse_impl_args_all_options() {
    let (name, opts) = parse_impl_args("myspec --task T-005 --dry-run").unwrap();
    assert_eq!(name, "myspec");
    assert_eq!(opts.task_id.as_deref(), Some("T-005"));
    assert!(opts.dry_run);
}

#[test]
fn test_parse_impl_args_empty() {
    let result = parse_impl_args("");
    assert!(result.is_err());
}

#[test]
fn test_parse_impl_args_unknown_option() {
    let result = parse_impl_args("myspec --verbose");
    assert!(result.is_err());
}

#[test]
fn test_parse_impl_args_task_without_id() {
    let result = parse_impl_args("myspec --task");
    assert!(result.is_err());
}

#[test]
fn test_build_progress_update() {
    let msg = build_progress_update("MySpec", "T-001", 3, 12, Some("T-002"));
    assert!(msg.contains("T-001"));
    assert!(msg.contains("3/12"));
    assert!(msg.contains("T-002"));
}

#[test]
fn test_build_progress_update_last_task() {
    let msg = build_progress_update("MySpec", "T-012", 12, 12, None);
    assert!(msg.contains("12/12"));
    assert!(!msg.contains("Next"));
}

#[test]
fn test_build_completion_summary() {
    let msg = build_completion_summary("MySpec", 12);
    assert!(msg.contains("12"));
    assert!(msg.contains("MySpec"));
    assert!(msg.contains("implemented"));
}

#[test]
fn test_build_cancellation_summary() {
    let msg = build_cancellation_summary("MySpec", 5, 12);
    assert!(msg.contains("5/12"));
    assert!(msg.contains("in_progress"));
}

#[test]
fn test_build_blocked_summary() {
    let msg = build_blocked_summary("T-003", &["T-005".into(), "T-007".into()]);
    assert!(msg.contains("T-003"));
    assert!(msg.contains("T-005"));
    assert!(msg.contains("T-007"));
}

#[test]
fn test_find_dependents() {
    let tasks = vec![
        PlanTask {
            id: "T-001".into(),
            title: "A".into(),
            requirement: "FR-001".into(),
            effort: Effort::S,
            priority: Priority::Critical,
            dependencies: vec![],
            status: TaskStatus::Pending,
            milestone: None,
        },
        PlanTask {
            id: "T-002".into(),
            title: "B".into(),
            requirement: "FR-002".into(),
            effort: Effort::S,
            priority: Priority::High,
            dependencies: vec!["T-001".into()],
            status: TaskStatus::Pending,
            milestone: None,
        },
        PlanTask {
            id: "T-003".into(),
            title: "C".into(),
            requirement: "FR-003".into(),
            effort: Effort::M,
            priority: Priority::High,
            dependencies: vec!["T-001".into()],
            status: TaskStatus::Pending,
            milestone: None,
        },
        PlanTask {
            id: "T-004".into(),
            title: "D".into(),
            requirement: "FR-004".into(),
            effort: Effort::L,
            priority: Priority::Medium,
            dependencies: vec!["T-002".into(), "T-003".into()],
            status: TaskStatus::Pending,
            milestone: None,
        },
    ];
    let deps = find_dependents(&tasks, "T-001");
    assert!(deps.contains(&"T-002".to_string()));
    assert!(deps.contains(&"T-003".to_string()));
    assert!(deps.contains(&"T-004".to_string()));
}

#[test]
fn test_effort_summary_calculation() {
    // Test via the runner's internal method indirectly
    let runner = SpecImplRunner {
        spec_name: "test".into(),
        specs_root: PathBuf::from("target/temp"),
        tasks: vec![
            PlanTask {
                id: "T-001".into(),
                title: "A".into(),
                requirement: "FR-001".into(),
                effort: Effort::S,
                priority: Priority::Critical,
                dependencies: vec![],
                status: TaskStatus::Pending,
                milestone: None,
            },
            PlanTask {
                id: "T-002".into(),
                title: "B".into(),
                requirement: "FR-002".into(),
                effort: Effort::M,
                priority: Priority::High,
                dependencies: vec!["T-001".into()],
                status: TaskStatus::Pending,
                milestone: None,
            },
            PlanTask {
                id: "T-003".into(),
                title: "C".into(),
                requirement: "FR-003".into(),
                effort: Effort::L,
                priority: Priority::Medium,
                dependencies: vec!["T-002".into()],
                status: TaskStatus::Pending,
                milestone: None,
            },
        ],
        execution_order: vec![0, 1, 2],
        options: ImplOptions::default(),
        milestones: vec![],
    };
    let summary = runner.effort_summary();
    assert_eq!(summary, "1xS, 1xM, 1xL");
}

// -- File Creation Order tests (FR-014, T-025) ------------------------

/// Helper: build a runner with the given task titles in execution order.
fn runner_with_titles(titles: &[&str]) -> SpecImplRunner {
    let tasks: Vec<PlanTask> = titles
        .iter()
        .enumerate()
        .map(|(i, title)| PlanTask {
            id: format!("T-{:03}", i + 1),
            title: (*title).to_string(),
            requirement: format!("FR-{:03}", i + 1),
            effort: Effort::S,
            priority: Priority::Medium,
            dependencies: vec![],
            status: TaskStatus::Pending,
            milestone: None,
        })
        .collect();
    let execution_order: Vec<usize> = (0..tasks.len()).collect();
    SpecImplRunner {
        spec_name: "test".into(),
        specs_root: PathBuf::from("target/temp"),
        tasks,
        execution_order,
        options: ImplOptions::default(),
        milestones: vec![],
    }
}

#[test]
fn test_file_order_warning_no_violation() {
    let runner = runner_with_titles(&[
        "Define API contracts in contracts/",
        "Write contract tests",
        "Write integration tests",
        "Write unit tests",
        "Implement source files",
    ]);
    assert!(
        runner.build_file_order_warning().is_none(),
        "no warning expected for correct order"
    );
}

#[test]
fn test_file_order_warning_source_before_tests() {
    let runner = runner_with_titles(&["Implement source files", "Write unit tests"]);
    let warning = runner
        .build_file_order_warning()
        .expect("warning expected when source precedes tests");
    assert!(warning.contains("File Creation Order Advisory"));
    assert!(warning.contains("T-002"));
    assert!(warning.contains("T-001"));
}

#[test]
fn test_file_order_warning_tests_before_contracts() {
    let runner = runner_with_titles(&["Write unit tests", "Define API contracts in contracts/"]);
    let warning = runner
        .build_file_order_warning()
        .expect("warning expected when tests precede contracts");
    assert!(warning.contains("contracts"));
    assert!(warning.contains("unit tests"));
}

#[test]
fn test_file_order_warning_single_task_no_warning() {
    let runner = runner_with_titles(&["Implement source files"]);
    assert!(
        runner.build_file_order_warning().is_none(),
        "no warning for single task"
    );
}

#[test]
fn test_file_order_warning_empty_no_warning() {
    let runner = runner_with_titles(&[]);
    assert!(
        runner.build_file_order_warning().is_none(),
        "no warning for zero tasks"
    );
}

#[test]
fn test_file_order_warning_advisory_text_present() {
    let runner = runner_with_titles(&["Implement source files", "Write contract tests"]);
    let warning = runner.build_file_order_warning().expect("warning expected");
    assert!(
        warning.contains("advisory only"),
        "warning should state it is advisory: {warning}"
    );
}

#[test]
fn test_file_order_warning_correct_full_order() {
    let runner = runner_with_titles(&[
        "Define API contracts and schema definitions",
        "Write contract tests for API endpoints",
        "Write integration tests for cross-component flows",
        "Write e2e tests for user-facing flows",
        "Write unit tests for individual functions",
        "Implement source code modules",
    ]);
    assert!(
        runner.build_file_order_warning().is_none(),
        "no warning for full correct test-first order"
    );
}

#[test]
fn test_file_order_warning_e2e_before_integration() {
    let runner = runner_with_titles(&["Write e2e tests for user flows", "Write integration tests"]);
    let warning = runner
        .build_file_order_warning()
        .expect("warning expected when e2e precedes integration tests");
    assert!(warning.contains("integration tests"));
    assert!(warning.contains("e2e tests"));
}

#[test]
fn test_file_creation_tier_contracts() {
    let task = PlanTask {
        id: "T-001".into(),
        title: "Define API contracts".into(),
        requirement: "FR-001".into(),
        effort: Effort::S,
        priority: Priority::Medium,
        dependencies: vec![],
        status: TaskStatus::Pending,
        milestone: None,
    };
    assert_eq!(file_creation_tier(&task), 1);
}

#[test]
fn test_file_creation_tier_contract_tests() {
    let task = PlanTask {
        id: "T-001".into(),
        title: "Write contract tests".into(),
        requirement: "FR-001".into(),
        effort: Effort::S,
        priority: Priority::Medium,
        dependencies: vec![],
        status: TaskStatus::Pending,
        milestone: None,
    };
    assert_eq!(file_creation_tier(&task), 2);
}

#[test]
fn test_file_creation_tier_integration_tests() {
    let task = PlanTask {
        id: "T-001".into(),
        title: "Write integration tests".into(),
        requirement: "FR-001".into(),
        effort: Effort::S,
        priority: Priority::Medium,
        dependencies: vec![],
        status: TaskStatus::Pending,
        milestone: None,
    };
    assert_eq!(file_creation_tier(&task), 3);
}

#[test]
fn test_file_creation_tier_e2e_tests() {
    let task = PlanTask {
        id: "T-001".into(),
        title: "Write end-to-end tests".into(),
        requirement: "FR-001".into(),
        effort: Effort::S,
        priority: Priority::Medium,
        dependencies: vec![],
        status: TaskStatus::Pending,
        milestone: None,
    };
    assert_eq!(file_creation_tier(&task), 4);
}

#[test]
fn test_file_creation_tier_unit_tests() {
    let task = PlanTask {
        id: "T-001".into(),
        title: "Write unit tests".into(),
        requirement: "FR-001".into(),
        effort: Effort::S,
        priority: Priority::Medium,
        dependencies: vec![],
        status: TaskStatus::Pending,
        milestone: None,
    };
    assert_eq!(file_creation_tier(&task), 5);
}

#[test]
fn test_file_creation_tier_source_files() {
    let task = PlanTask {
        id: "T-001".into(),
        title: "Implement the feature".into(),
        requirement: "FR-001".into(),
        effort: Effort::S,
        priority: Priority::Medium,
        dependencies: vec![],
        status: TaskStatus::Pending,
        milestone: None,
    };
    assert_eq!(file_creation_tier(&task), 6);
}

#[test]
fn test_file_creation_tier_contract_test_not_misclassified() {
    // "contract test" should be tier 2, not tier 1 (contract)
    let task = PlanTask {
        id: "T-001".into(),
        title: "Write contract tests for endpoints".into(),
        requirement: "FR-001".into(),
        effort: Effort::S,
        priority: Priority::Medium,
        dependencies: vec![],
        status: TaskStatus::Pending,
        milestone: None,
    };
    assert_eq!(file_creation_tier(&task), 2);
}

#[test]
fn test_file_order_warning_included_in_summary() {
    let runner = runner_with_titles(&["Implement source files", "Write unit tests"]);
    let summary = runner.build_summary();
    assert!(
        summary.contains("File Creation Order Advisory"),
        "summary should include advisory warning: {summary}"
    );
}

#[test]
fn test_file_order_warning_included_in_dry_run() {
    let runner = runner_with_titles(&["Implement source files", "Write unit tests"]);
    let display = runner.build_dry_run_display();
    assert!(
        display.contains("File Creation Order Advisory"),
        "dry-run display should include advisory warning: {display}"
    );
}

#[test]
fn test_file_order_warning_not_included_when_no_violation() {
    let runner = runner_with_titles(&["Write unit tests", "Implement source files"]);
    let summary = runner.build_summary();
    assert!(
        !summary.contains("File Creation Order Advisory"),
        "summary should not include warning when order is correct: {summary}"
    );
}
