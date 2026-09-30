//! Inline tests for `plan_parser.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;

#[test]
fn test_effort_parse() {
    assert_eq!(Effort::parse("S"), Some(Effort::S));
    assert_eq!(Effort::parse("M"), Some(Effort::M));
    assert_eq!(Effort::parse("L"), Some(Effort::L));
    assert_eq!(Effort::parse("s"), Some(Effort::S));
    assert_eq!(Effort::parse("X"), None);
    assert_eq!(Effort::parse(""), None);
}

#[test]
fn test_priority_parse() {
    assert_eq!(Priority::parse("Critical"), Some(Priority::Critical));
    assert_eq!(Priority::parse("high"), Some(Priority::High));
    assert_eq!(Priority::parse("MEDIUM"), Some(Priority::Medium));
    assert_eq!(Priority::parse("Low"), Some(Priority::Low));
    assert_eq!(Priority::parse("Urgent"), None);
}

#[test]
fn test_priority_ordering() {
    assert!(Priority::Critical > Priority::High);
    assert!(Priority::High > Priority::Medium);
    assert!(Priority::Medium > Priority::Low);
}

#[test]
fn test_parse_dependencies() {
    assert_eq!(
        PlanParser::parse_dependencies("T-001, T-002"),
        vec!["T-001", "T-002"]
    );
    assert_eq!(PlanParser::parse_dependencies("-"), Vec::<String>::new());
    assert_eq!(PlanParser::parse_dependencies("-"), Vec::<String>::new());
    assert_eq!(PlanParser::parse_dependencies(""), Vec::<String>::new());
    assert_eq!(PlanParser::parse_dependencies("T-003"), vec!["T-003"]);
}

#[test]
fn test_parse_valid_table() {
    let md = r"
# Plan

## Tasks

| ID | Title | Requirement | Effort | Priority | Dependencies |
|---|---|---|---|---|---|
| T-001 | Define types | FR-003 | S | Critical | - |
| T-002 | Build parser | FR-004 | M | High | T-001 |
| T-003 | Add tests | FR-005 | M | High | T-002 |

## Details
";
    let tasks = PlanParser::parse(md).unwrap();
    assert_eq!(tasks.len(), 3);
    assert_eq!(tasks[0].id, "T-001");
    assert_eq!(tasks[0].effort, Effort::S);
    assert_eq!(tasks[0].priority, Priority::Critical);
    assert!(
        tasks[0].dependencies.is_empty(),
        "first task should have no dependencies"
    );
    assert_eq!(tasks[1].dependencies, vec!["T-001"]);
    assert_eq!(tasks[2].dependencies, vec!["T-002"]);
}

#[test]
fn test_parse_with_status_column() {
    let md = r"
## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|---|---|---|---|---|---|---|
| T-001 | Define types | FR-003 | S | Critical | completed | - |
| T-002 | Build parser | FR-004 | M | High | in_progress | T-001 |
| T-003 | Add tests | FR-005 | M | High | pending | T-002 |
";
    let tasks = PlanParser::parse(md).unwrap();
    assert_eq!(tasks.len(), 3);
    assert_eq!(tasks[0].status, TaskStatus::Completed);
    assert_eq!(tasks[1].status, TaskStatus::InProgress);
    assert_eq!(tasks[2].status, TaskStatus::Pending);
}

#[test]
fn test_parse_empty_returns_error() {
    let md = "## Tasks\n\nNo table here.\n";
    let result = PlanParser::parse(md);
    assert!(result.is_err());
}

#[test]
fn test_parse_skips_malformed_rows() {
    let md = r"
## Tasks

| ID | Title | Requirement | Effort | Priority | Dependencies |
|---|---|---|---|---|---|
| T-001 | Valid task | FR-003 | S | Critical | - |
| bad-id | Invalid | FR-004 | M | High | - |
| T-002 | Also valid | FR-005 | M | High | T-001 |
";
    let tasks = PlanParser::parse(md).unwrap();
    assert_eq!(tasks.len(), 2);
    assert_eq!(tasks[0].id, "T-001");
    assert_eq!(tasks[1].id, "T-002");
}

#[test]
fn test_topological_sort_simple_chain() {
    let tasks = vec![
        PlanTask {
            id: "T-001".into(),
            title: "First".into(),
            requirement: "FR-001".into(),
            effort: Effort::S,
            priority: Priority::Critical,
            dependencies: vec![],
            status: TaskStatus::Pending,
            milestone: None,
        },
        PlanTask {
            id: "T-002".into(),
            title: "Second".into(),
            requirement: "FR-002".into(),
            effort: Effort::M,
            priority: Priority::High,
            dependencies: vec!["T-001".into()],
            status: TaskStatus::Pending,
            milestone: None,
        },
        PlanTask {
            id: "T-003".into(),
            title: "Third".into(),
            requirement: "FR-003".into(),
            effort: Effort::L,
            priority: Priority::Medium,
            dependencies: vec!["T-002".into()],
            status: TaskStatus::Pending,
            milestone: None,
        },
    ];
    let order = resolve_execution_order(&tasks).unwrap();
    assert_eq!(order.len(), 3);
    // T-001 must come before T-002, T-002 before T-003
    let _pos: HashMap<&str, usize> = order.iter().map(|&i| (tasks[i].id.as_str(), i)).collect();
    // Not checking exact positions, just relative ordering
    let t1 = order.iter().position(|&i| tasks[i].id == "T-001").unwrap();
    let t2 = order.iter().position(|&i| tasks[i].id == "T-002").unwrap();
    let t3 = order.iter().position(|&i| tasks[i].id == "T-003").unwrap();
    assert!(t1 < t2);
    assert!(t2 < t3);
}

#[test]
fn test_topological_sort_diamond() {
    // T-001 -> T-002, T-001 -> T-003, T-002 -> T-004, T-003 -> T-004
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
            effort: Effort::S,
            priority: Priority::High,
            dependencies: vec!["T-001".into()],
            status: TaskStatus::Pending,
            milestone: None,
        },
        PlanTask {
            id: "T-004".into(),
            title: "D".into(),
            requirement: "FR-004".into(),
            effort: Effort::M,
            priority: Priority::Medium,
            dependencies: vec!["T-002".into(), "T-003".into()],
            status: TaskStatus::Pending,
            milestone: None,
        },
    ];
    let order = resolve_execution_order(&tasks).unwrap();
    assert_eq!(order.len(), 4);
    let t1 = order.iter().position(|&i| tasks[i].id == "T-001").unwrap();
    let t2 = order.iter().position(|&i| tasks[i].id == "T-002").unwrap();
    let t3 = order.iter().position(|&i| tasks[i].id == "T-003").unwrap();
    let t4 = order.iter().position(|&i| tasks[i].id == "T-004").unwrap();
    assert!(t1 < t2);
    assert!(t1 < t3);
    assert!(t2 < t4);
    assert!(t3 < t4);
}

#[test]
fn test_topological_sort_cycle_detection() {
    let tasks = vec![
        PlanTask {
            id: "T-001".into(),
            title: "A".into(),
            requirement: "FR-001".into(),
            effort: Effort::S,
            priority: Priority::Critical,
            dependencies: vec!["T-002".into()],
            status: TaskStatus::Pending,
            milestone: None,
        },
        PlanTask {
            id: "T-002".into(),
            title: "B".into(),
            requirement: "FR-002".into(),
            effort: Effort::S,
            priority: Priority::Critical,
            dependencies: vec!["T-001".into()],
            status: TaskStatus::Pending,
            milestone: None,
        },
    ];
    let result = resolve_execution_order(&tasks);
    assert!(result.is_err());
    if let Err(SpecError::DependencyCycle { task_ids }) = result {
        assert!(task_ids.contains(&"T-001".to_string()));
        assert!(task_ids.contains(&"T-002".to_string()));
    } else {
        panic!("Expected DependencyCycle error");
    }
}

#[test]
fn test_topological_sort_empty() {
    let order = resolve_execution_order(&[]).unwrap();
    assert!(order.is_empty(), "order should be empty");
}

#[test]
fn test_filter_for_task() {
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
    // --task T-003 should include T-001 and T-003
    let filtered = filter_for_task(&tasks, "T-003").unwrap();
    assert_eq!(filtered.len(), 2);
    let ids: Vec<&str> = filtered.iter().map(|&i| tasks[i].id.as_str()).collect();
    assert!(ids.contains(&"T-001"));
    assert!(ids.contains(&"T-003"));
}

#[test]
fn test_filter_for_resume() {
    let tasks = vec![
        PlanTask {
            id: "T-001".into(),
            title: "A".into(),
            requirement: "FR-001".into(),
            effort: Effort::S,
            priority: Priority::Critical,
            dependencies: vec![],
            status: TaskStatus::Completed,
            milestone: None,
        },
        PlanTask {
            id: "T-002".into(),
            title: "B".into(),
            requirement: "FR-002".into(),
            effort: Effort::S,
            priority: Priority::High,
            dependencies: vec!["T-001".into()],
            status: TaskStatus::InProgress,
            milestone: None,
        },
        PlanTask {
            id: "T-003".into(),
            title: "C".into(),
            requirement: "FR-003".into(),
            effort: Effort::M,
            priority: Priority::High,
            dependencies: vec!["T-002".into()],
            status: TaskStatus::Pending,
            milestone: None,
        },
    ];
    let order = resolve_execution_order(&tasks).unwrap();
    let resumed = filter_for_resume(&tasks, &order);
    // Completed T-001 is skipped; T-002 and T-003 remain in topological order.
    assert_eq!(resumed.len(), 2);
    assert_eq!(tasks[resumed[0]].id, "T-002");
    assert_eq!(tasks[resumed[1]].id, "T-003");
}
#[test]
fn test_filter_for_resume_blocked_unblocked() {
    // T-002 was blocked because T-001 wasn't done, but now T-001 is completed
    let tasks = vec![
        PlanTask {
            id: "T-001".into(),
            title: "A".into(),
            requirement: "FR-001".into(),
            effort: Effort::S,
            priority: Priority::Critical,
            dependencies: vec![],
            status: TaskStatus::Completed,
            milestone: None,
        },
        PlanTask {
            id: "T-002".into(),
            title: "B".into(),
            requirement: "FR-002".into(),
            effort: Effort::S,
            priority: Priority::High,
            dependencies: vec!["T-001".into()],
            status: TaskStatus::Blocked,
            milestone: None,
        },
    ];
    let order = resolve_execution_order(&tasks).unwrap();
    let resumed = filter_for_resume(&tasks, &order);
    // T-002 should be unblocked since T-001 is now completed
    assert_eq!(resumed.len(), 1);
    assert_eq!(tasks[resumed[0]].id, "T-002");
}

// ── Phase -1 Gates tests (T-015, FR-008) ──────────────────────────────

#[test]
fn test_parse_gates_all_checked() {
    let md = "\
## Phase -1 Gates

- [x] **Simplicity:** The plan does the simplest thing that works.
- [x] **Anti-Abstraction:** No premature abstraction.
- [x] **Integration-First:** Tests written before source files.

## Tasks

| ID | Title | Req | Effort | Priority | Dependencies |
";
    let gates = PlanParser::parse_phase_minus_one_gates(md);
    assert_eq!(gates.gates.len(), 3);
    assert!(gates.gates[0].checked);
    assert!(gates.gates[1].checked);
    assert!(gates.gates[2].checked);
    assert!(gates.is_all_checked());
    assert!(gates.has_all_required_gates());
    assert!(!gates.has_complexity_tracking);
}

#[test]
fn test_parse_gates_one_unchecked() {
    let md = "\
## Phase -1 Gates

- [x] **Simplicity:** Done.
- [x] **Anti-Abstraction:** Done.
- [ ] **Integration-First:** Not yet.

## Tasks
";
    let gates = PlanParser::parse_phase_minus_one_gates(md);
    assert_eq!(gates.gates.len(), 3);
    assert!(!gates.is_all_checked());
    let unchecked = gates.unchecked_required_gates();
    assert_eq!(unchecked, vec!["Integration-First"]);
}

#[test]
fn test_parse_gates_all_unchecked() {
    let md = "\
## Phase -1 Gates

- [ ] **Simplicity:** Not done.
- [ ] **Anti-Abstraction:** Not done.
- [ ] **Integration-First:** Not done.
";
    let gates = PlanParser::parse_phase_minus_one_gates(md);
    assert!(!gates.is_all_checked());
    assert_eq!(gates.unchecked_required_gates().len(), 3);
}

#[test]
fn test_parse_gates_section_missing() {
    let md = "## Tasks\n\n| ID | Title |\n|---|---|\n| T-001 | Foo |";
    let gates = PlanParser::parse_phase_minus_one_gates(md);
    assert!(gates.is_empty(), "gates should be empty");
    assert!(!gates.is_all_checked());
    assert!(!gates.has_all_required_gates());
    assert_eq!(gates.unchecked_required_gates().len(), 3);
}

#[test]
fn test_parse_gates_partial_gates_present() {
    let md = "\
## Phase -1 Gates

- [x] **Simplicity:** Done.
";
    let gates = PlanParser::parse_phase_minus_one_gates(md);
    assert_eq!(gates.gates.len(), 1);
    assert!(!gates.has_all_required_gates());
    assert!(!gates.is_all_checked());
    let unchecked = gates.unchecked_required_gates();
    assert_eq!(unchecked, vec!["Anti-Abstraction", "Integration-First"]);
}

#[test]
fn test_parse_gates_complexity_tracking_detected() {
    let md = "\
## Phase -1 Gates

- [x] **Simplicity:** Done.
- [x] **Anti-Abstraction:** Done.
- [x] **Integration-First:** Done.

## Complexity Tracking

| Gate | Exception | Rationale |
|------|-----------|-----------|
";
    let gates = PlanParser::parse_phase_minus_one_gates(md);
    assert!(gates.has_complexity_tracking);
    assert!(gates.is_all_checked());
}

#[test]
fn test_parse_gates_complexity_tracking_without_gate_section() {
    let md = "\
## Complexity Tracking

| Gate | Exception | Rationale |
";
    let gates = PlanParser::parse_phase_minus_one_gates(md);
    assert!(gates.has_complexity_tracking);
    assert_eq!(gates.gates.len(), 0);
}

#[test]
fn test_parse_gates_case_insensitive_heading() {
    let md = "\
## phase -1 gates

- [x] Simplicity: Done.
- [x] Anti-Abstraction: Done.
- [x] Integration-First: Done.
";
    let gates = PlanParser::parse_phase_minus_one_gates(md);
    assert_eq!(gates.gates.len(), 3);
    assert!(gates.is_all_checked());
}

#[test]
fn test_parse_gates_uppercase_x_checkbox() {
    let md = "\
## Phase -1 Gates

- [X] **Simplicity:** Done.
- [X] **Anti-Abstraction:** Done.
- [X] **Integration-First:** Done.
";
    let gates = PlanParser::parse_phase_minus_one_gates(md);
    assert_eq!(gates.gates.len(), 3);
    assert!(gates.gates.iter().all(|g| g.checked));
    assert!(gates.is_all_checked());
}

#[test]
fn test_parse_gates_without_bold_markers() {
    let md = "\
## Phase -1 Gates

- [x] Simplicity: The plan does the simplest thing.
- [x] Anti-Abstraction: No premature abstraction.
- [x] Integration-First: Tests before source.
";
    let gates = PlanParser::parse_phase_minus_one_gates(md);
    assert_eq!(gates.gates.len(), 3);
    assert_eq!(gates.gates[0].name, "Simplicity");
    assert_eq!(gates.gates[1].name, "Anti-Abstraction");
    assert_eq!(gates.gates[2].name, "Integration-First");
    assert!(gates.is_all_checked());
}

#[test]
fn test_parse_gates_name_without_colon() {
    let md = "\
## Phase -1 Gates

- [x] Simplicity
- [x] Anti-Abstraction
- [x] Integration-First
";
    let gates = PlanParser::parse_phase_minus_one_gates(md);
    assert_eq!(gates.gates.len(), 3);
    assert_eq!(gates.gates[0].name, "Simplicity");
    assert!(gates.is_all_checked());
}

#[test]
fn test_parse_gates_extra_non_checkbox_lines_ignored() {
    let md = "\
## Phase -1 Gates

Some introductory text about gates.

- [x] **Simplicity:** Done.
- [x] **Anti-Abstraction:** Done.
- [x] **Integration-First:** Done.

A trailing comment.
";
    let gates = PlanParser::parse_phase_minus_one_gates(md);
    assert_eq!(gates.gates.len(), 3);
    assert!(gates.is_all_checked());
}

#[test]
fn test_parse_gates_case_insensitive_gate_names() {
    let md = "\
## Phase -1 Gates

- [x] simplicity: Done.
- [x] anti-abstraction: Done.
- [x] integration-first: Done.
";
    let gates = PlanParser::parse_phase_minus_one_gates(md);
    assert_eq!(gates.gates.len(), 3);
    assert!(gates.is_all_checked());
    assert!(gates.has_all_required_gates());
}

#[test]
fn test_parse_gates_stops_at_next_h2_heading() {
    let md = "\
## Phase -1 Gates

- [x] **Simplicity:** Done.
- [x] **Anti-Abstraction:** Done.
- [x] **Integration-First:** Done.

## Tasks

- [x] Simplicity: should not be parsed
";
    let gates = PlanParser::parse_phase_minus_one_gates(md);
    assert_eq!(gates.gates.len(), 3);
    assert!(gates.is_all_checked());
}

#[test]
fn test_parse_gates_empty_checkbox_name_skipped() {
    let md = "\
## Phase -1 Gates

- [x] **:** Empty name.
- [x] **Simplicity:** Done.
- [x] **Anti-Abstraction:** Done.
- [x] **Integration-First:** Done.
";
    let gates = PlanParser::parse_phase_minus_one_gates(md);
    assert_eq!(gates.gates.len(), 3);
    assert!(gates.is_all_checked());
}

#[test]
fn test_required_gate_names_constant() {
    assert_eq!(REQUIRED_GATE_NAMES.len(), 3);
    assert!(REQUIRED_GATE_NAMES.contains(&"Simplicity"));
    assert!(REQUIRED_GATE_NAMES.contains(&"Anti-Abstraction"));
    assert!(REQUIRED_GATE_NAMES.contains(&"Integration-First"));
}
