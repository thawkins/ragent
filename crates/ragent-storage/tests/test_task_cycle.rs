#![allow(clippy::assert_is_empty)]
//! todo2tasks T-005: tests for cycle detection in the task dependency
//! DAG (`detect_cycle`).
//!
//! These tests verify FR-004: when a `task_update` call attempts to add
//! a `blocked_by` or `add_blocks` edge that would create a dependency
//! cycle, the system rejects the update with an error describing the
//! cycle and does not persist the edge.
//!
//! `detect_cycle` is a pure function — no database required.  Tests
//! construct `TaskRow` snapshots directly and check that cycles are
//! detected (or not) as expected.

use ragent_storage::storage::{CycleError, TaskRow, detect_cycle};

// ── Helper ─────────────────────────────────────────────────────────

fn make_task(id: &str, blocked_by: &[&str]) -> TaskRow {
    TaskRow {
        id: id.to_string(),
        session_id: "sess".to_string(),
        title: format!("Task {id}"),
        status: "pending".to_string(),
        description: String::new(),
        created_at: "2025-01-01T00:00:00Z".to_string(),
        updated_at: "2025-01-01T00:00:00Z".to_string(),
        active_form: None,
        owner: None,
        metadata: "{}".to_string(),
        blocked_by: blocked_by.iter().map(|s| s.to_string()).collect(),
    }
}

// ── No-cycle cases ─────────────────────────────────────────────────

/// Adding a blocked_by edge when there are no existing deps is safe.
#[test]
fn test_no_cycle_empty_graph() {
    let tasks = vec![make_task("a", &[]), make_task("b", &[])];
    assert!(detect_cycle(&tasks, "a", "b").is_ok());
}

/// Adding a forward edge A→B when B has no deps is safe.
#[test]
fn test_no_cycle_forward_edge() {
    let tasks = vec![make_task("a", &[]), make_task("b", &[])];
    assert!(detect_cycle(&tasks, "a", "b").is_ok());
}

/// Adding A→C when B→C already exists is safe (no path from C to A).
#[test]
fn test_no_cycle_independent_deps() {
    let tasks = vec![
        make_task("a", &[]),
        make_task("b", &[]),
        make_task("c", &["b"]),
    ];
    // Adding c to a's blocked_by: a depends on c.  Does c depend on a? No.
    assert!(detect_cycle(&tasks, "a", "c").is_ok());
}

/// Chain A→B→C: adding D→A is safe (no path from A to D).
#[test]
fn test_no_cycle_chain_extension() {
    let tasks = vec![
        make_task("a", &["b"]),
        make_task("b", &["c"]),
        make_task("c", &[]),
        make_task("d", &[]),
    ];
    // Adding a to d's blocked_by: d depends on a.  Does a depend on d? No.
    assert!(detect_cycle(&tasks, "d", "a").is_ok());
}

/// Adding an edge that already exists (duplicate) is safe — no cycle.
#[test]
fn test_no_cycle_duplicate_edge() {
    let tasks = vec![make_task("a", &["b"]), make_task("b", &[])];
    // a already depends on b.  Re-adding should be fine (no cycle).
    assert!(detect_cycle(&tasks, "a", "b").is_ok());
}

/// Empty task list ��� edge to non-existent target is safe (no cycle).
#[test]
fn test_no_cycle_empty_tasks() {
    let tasks: Vec<TaskRow> = vec![];
    // target "b" doesn't exist; DFS goes nowhere.  No cycle.
    assert!(detect_cycle(&tasks, "a", "b").is_ok());
}

// ── Self-loop ──────────────────────────────────────────────────────

/// A task cannot depend on itself.
#[test]
fn test_cycle_self_loop() {
    let tasks = vec![make_task("a", &[])];
    let err = detect_cycle(&tasks, "a", "a").unwrap_err();
    assert_eq!(err.cycle_path, vec!["a", "a"]);
}

/// Self-loop even when other deps exist.
#[test]
fn test_cycle_self_loop_with_deps() {
    let tasks = vec![make_task("a", &["b"]), make_task("b", &[])];
    let err = detect_cycle(&tasks, "a", "a").unwrap_err();
    assert_eq!(err.cycle_path, vec!["a", "a"]);
}

// ── Direct cycle (A→B, adding B→A) ─────────────────────────────────

/// Adding B→A when A→B exists creates a 2-node cycle.
#[test]
fn test_cycle_direct_two_nodes() {
    let tasks = vec![make_task("a", &["b"]), make_task("b", &[])];
    // a depends on b.  Adding b to depend on a: b→a, but a→b already.
    // DFS from a (target): a depends on b (source).  Found!
    // Cycle: b → a → b
    let err = detect_cycle(&tasks, "b", "a").unwrap_err();
    assert_eq!(err.cycle_path, vec!["b", "a", "b"]);
}

// ── Three-node cycle ───────────────────────────────────────────────

/// A→B→C: adding C→A creates a 3-node cycle.
#[test]
fn test_cycle_three_nodes() {
    let tasks = vec![
        make_task("a", &["b"]),
        make_task("b", &["c"]),
        make_task("c", &[]),
    ];
    // Adding c to depend on a: c→a.  But a→b→c already.
    // DFS from a (target): a→b→c.  c is not source (c is source).
    // Wait, source is "c", target is "a".
    // DFS from "a": a depends on b, b depends on c.  c == source.  Found!
    // Cycle: c → a → b → c
    let err = detect_cycle(&tasks, "c", "a").unwrap_err();
    assert_eq!(err.cycle_path[0], "c");
    assert_eq!(err.cycle_path.last().unwrap(), "c");
    assert_eq!(err.cycle_path.len(), 4); // c → a → b → c
}

// ── Longer chain cycle ───────────────────────────────────────��─────

/// A→B→C→D: adding D→A creates a 4-node cycle.
#[test]
fn test_cycle_four_nodes() {
    let tasks = vec![
        make_task("a", &["b"]),
        make_task("b", &["c"]),
        make_task("c", &["d"]),
        make_task("d", &[]),
    ];
    let err = detect_cycle(&tasks, "d", "a").unwrap_err();
    assert_eq!(err.cycle_path[0], "d");
    assert_eq!(err.cycle_path.last().unwrap(), "d");
    assert_eq!(err.cycle_path.len(), 5); // d → a → b → c → d
}

// ── Cycle through diamond ──────────────────────────────────────────

/// Diamond: A→B, A→C, B→D, C→D.
/// Adding D→A creates a cycle through either path.
#[test]
fn test_cycle_diamond() {
    let tasks = vec![
        make_task("a", &["b", "c"]),
        make_task("b", &["d"]),
        make_task("c", &["d"]),
        make_task("d", &[]),
    ];
    // Adding d to depend on a: d→a.  a→b→d or a→c→d.  Found!
    let err = detect_cycle(&tasks, "d", "a").unwrap_err();
    assert_eq!(err.cycle_path[0], "d");
    assert_eq!(err.cycle_path.last().unwrap(), "d");
    // Path should be d → a → (b or c) → d
    assert_eq!(err.cycle_path.len(), 4);
    assert_eq!(err.cycle_path[1], "a");
    assert!(err.cycle_path[2] == "b" || err.cycle_path[2] == "c");
    assert_eq!(err.cycle_path[3], "d");
}

// ── No false positive: parallel paths ──────────────────────────────

/// A→B, A→C, B→D, C→D.  Adding D→E is safe (E is not an ancestor of D).
#[test]
fn test_no_cycle_diamond_parallel() {
    let tasks = vec![
        make_task("a", &["b", "c"]),
        make_task("b", &["d"]),
        make_task("c", &["d"]),
        make_task("d", &[]),
        make_task("e", &[]),
    ];
    assert!(detect_cycle(&tasks, "d", "e").is_ok());
}

// ── Error message is descriptive ───────────────────────────────────

/// The error message contains the cycle path with arrows.
#[test]
fn test_cycle_error_message_format() {
    let tasks = vec![make_task("a", &["b"]), make_task("b", &[])];
    let err = detect_cycle(&tasks, "b", "a").unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("cycle"), "msg: {msg}");
    assert!(msg.contains('b'), "msg: {msg}");
    assert!(msg.contains('a'), "msg: {msg}");
    assert!(msg.contains("→"), "msg: {msg}");
}

/// The error message for a self-loop mentions the task ID.
#[test]
fn test_cycle_error_self_loop_message() {
    let tasks = vec![make_task("x", &[])];
    let err = detect_cycle(&tasks, "x", "x").unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains('x'), "msg: {msg}");
    assert!(msg.contains("cycle"), "msg: {msg}");
}

// ── CycleError is Clone and Debug ──────────────────────────────────

#[test]
fn test_cycle_error_clone_debug() {
    let err = CycleError {
        cycle_path: vec!["a".to_string(), "b".to_string(), "a".to_string()],
    };
    let cloned = err.clone();
    assert_eq!(err.cycle_path, cloned.cycle_path);
    let debug_str = format!("{err:?}");
    assert!(debug_str.contains("CycleError"));
    assert!(debug_str.contains('a'));
}

// ── Dangling reference (non-existent target) ───────────────────────

/// Target ID doesn't exist in tasks — DFS goes nowhere, no cycle.
#[test]
fn test_no_cycle_nonexistent_target() {
    let tasks = vec![make_task("a", &[])];
    // target "nonexistent" not in graph; DFS from it finds nothing.
    assert!(detect_cycle(&tasks, "a", "nonexistent").is_ok());
}

/// Source ID doesn't exist — still works, just checks if target
/// reaches source.
#[test]
fn test_no_cycle_nonexistent_source() {
    let tasks = vec![make_task("a", &["b"]), make_task("b", &[])];
    // source "nonexistent" not in graph.  DFS from b: b has no deps.
    // Never reaches "nonexistent".  No cycle.
    assert!(detect_cycle(&tasks, "nonexistent", "b").is_ok());
}

/// Existing blocked_by reference to non-existent ID is followed as a
/// dead-end (no adjacency), not treated as a cycle.
#[test]
fn test_no_cycle_dangling_blocked_by() {
    let tasks = vec![make_task("a", &["ghost"]), make_task("b", &[])];
    // a depends on "ghost" which doesn't exist.  Adding a→b is fine.
    assert!(detect_cycle(&tasks, "a", "b").is_ok());
}

// ── Larger graph: no false positive ────────────────────────────────

/// A complex DAG with 10 tasks and multiple paths — verify a safe edge
/// is not flagged as a cycle.
#[test]
fn test_no_cycle_complex_dag_safe_edge() {
    // 1→2→3→4→5, 1→6→4, 7→8→9, 3→9
    let tasks = vec![
        make_task("1", &["2", "6"]),
        make_task("2", &["3"]),
        make_task("3", &["4", "9"]),
        make_task("4", &["5"]),
        make_task("5", &[]),
        make_task("6", &["4"]),
        make_task("7", &["8"]),
        make_task("8", &["9"]),
        make_task("9", &[]),
        make_task("10", &[]),
    ];
    // Adding 10→7 is safe: 7 doesn't depend on 10.
    assert!(detect_cycle(&tasks, "10", "7").is_ok());
    // Adding 5→10 is safe: 10 doesn't depend on 5.
    assert!(detect_cycle(&tasks, "5", "10").is_ok());
}

/// Same complex DAG — adding 5→1 creates a cycle (1→2→3→4→5, 5→1).
#[test]
fn test_cycle_complex_dag() {
    let tasks = vec![
        make_task("1", &["2", "6"]),
        make_task("2", &["3"]),
        make_task("3", &["4", "9"]),
        make_task("4", &["5"]),
        make_task("5", &[]),
        make_task("6", &["4"]),
        make_task("7", &["8"]),
        make_task("8", &["9"]),
        make_task("9", &[]),
    ];
    // Adding 5→1: does 1 depend on 5?  1→2→3→4→5.  Yes!
    let err = detect_cycle(&tasks, "5", "1").unwrap_err();
    assert_eq!(err.cycle_path[0], "5");
    assert_eq!(err.cycle_path.last().unwrap(), "5");
}

// ── Cycle path starts and ends with source ────────────────���────────

/// Verify that cycle_path always starts and ends with the source node.
#[test]
fn test_cycle_path_starts_ends_with_source() {
    let tasks = vec![
        make_task("a", &["b"]),
        make_task("b", &["c"]),
        make_task("c", &["d"]),
        make_task("d", &[]),
    ];
    let err = detect_cycle(&tasks, "d", "a").unwrap_err();
    assert_eq!(err.cycle_path.first().unwrap(), "d");
    assert_eq!(err.cycle_path.last().unwrap(), "d");
    // The second element should be the target.
    assert_eq!(err.cycle_path[1], "a");
}

// ── Cycle path is a valid path in the graph ────────────────────────

/// Verify every consecutive pair in the cycle path corresponds to an
/// actual blocked_by edge (or the proposed edge for the first pair).
#[test]
fn test_cycle_path_edges_are_valid() {
    let tasks = vec![
        make_task("a", &["b"]),
        make_task("b", &["c"]),
        make_task("c", &[]),
    ];
    let err = detect_cycle(&tasks, "c", "a").unwrap_err();
    let path = &err.cycle_path;
    // path: c → a → b → c
    assert_eq!(path.len(), 4);

    // First edge c→a is the proposed edge (not yet in graph).
    // Remaining edges must exist in the graph:
    // a→b: a.blocked_by contains b
    // b→c: b.blocked_by contains c
    let adj: std::collections::HashMap<&str, &[String]> = tasks
        .iter()
        .map(|t| (t.id.as_str(), t.blocked_by.as_slice()))
        .collect();
    for w in path.windows(2).skip(1) {
        let from = &w[0];
        let to = &w[1];
        let deps = adj.get(from.as_str()).expect("task must exist");
        assert!(
            deps.iter().any(|d| d == to),
            "edge {from}→{to} not found in graph"
        );
    }
}
