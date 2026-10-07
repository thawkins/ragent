//! Inline tests for `impl_runner.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;
use crate::plan_parser::{Effort, Priority};
use crate::spec::TaskStatus;

#[test]
fn test_milestone_groups_group_by_milestone() {
    let tasks = vec![
        PlanTask {
            id: "T-001".into(),
            title: "Define types".into(),
            requirement: "FR-001".into(),
            effort: Effort::S,
            priority: Priority::Critical,
            dependencies: vec![],
            status: TaskStatus::Pending,
            milestone: Some("Milestone 1: Core".into()),
        },
        PlanTask {
            id: "T-002".into(),
            title: "Build parser".into(),
            requirement: "FR-002".into(),
            effort: Effort::M,
            priority: Priority::High,
            dependencies: vec!["T-001".into()],
            status: TaskStatus::Pending,
            milestone: Some("Milestone 1: Core".into()),
        },
        PlanTask {
            id: "T-003".into(),
            title: "Add tests".into(),
            requirement: "FR-003".into(),
            effort: Effort::M,
            priority: Priority::High,
            dependencies: vec!["T-002".into()],
            status: TaskStatus::Pending,
            milestone: Some("Milestone 2: Tests".into()),
        },
    ];
    let execution_order = vec![0, 1, 2];
    let runner = SpecImplRunner {
        spec_name: "test".into(),
        specs_root: PathBuf::from("target/temp"),
        tasks,
        execution_order,
        options: ImplOptions::default(),
        milestones: vec![],
    };
    let groups = runner.milestone_groups();
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].name, "Milestone 1: Core");
    assert_eq!(groups[0].task_ids, vec!["T-001", "T-002"]);
    assert_eq!(groups[1].name, "Milestone 2: Tests");
    assert_eq!(groups[1].task_ids, vec!["T-003"]);
}

#[test]
fn test_milestone_groups_unmapped_tasks_are_grouped() {
    let tasks = vec![
        PlanTask {
            id: "T-001".into(),
            title: "A".into(),
            requirement: "FR-001".into(),
            effort: Effort::S,
            priority: Priority::Medium,
            dependencies: vec![],
            status: TaskStatus::Pending,
            milestone: Some("Known".into()),
        },
        PlanTask {
            id: "T-002".into(),
            title: "B".into(),
            requirement: "FR-002".into(),
            effort: Effort::S,
            priority: Priority::Medium,
            dependencies: vec![],
            status: TaskStatus::Pending,
            milestone: None,
        },
    ];
    let execution_order = vec![0, 1];
    let runner = SpecImplRunner {
        spec_name: "test".into(),
        specs_root: PathBuf::from("target/temp"),
        tasks,
        execution_order,
        options: ImplOptions::default(),
        milestones: vec![],
    };
    let groups = runner.milestone_groups();
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].name, "Known");
    assert_eq!(groups[1].name, "Unmapped Tasks");
    assert_eq!(groups[1].task_ids, vec!["T-002"]);
}
