//! Plan parser: parses PLAN.md task tables into structured types and resolves
//! dependency order via topological sort.
//!
//! Supports both 6-column tables (without Status) and 7-column tables (with
//! Status), matching the format used in spec PLAN.md files.

use crate::error::SpecError;
use crate::spec::TaskStatus;
use std::collections::{HashMap, HashSet, VecDeque};

// ── Effort ────────────────────────────────────────────────────────────────

/// Estimated effort for a task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Effort {
    /// Small (hours).
    S,
    /// Medium (half-day).
    M,
    /// Large (day+).
    L,
}

impl Effort {
    /// Parse from a single character: "S", "M", "L" (case-insensitive).
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_uppercase().as_str() {
            "S" => Some(Self::S),
            "M" => Some(Self::M),
            "L" => Some(Self::L),
            _ => None,
        }
    }

    /// Human-readable label.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::S => "S",
            Self::M => "M",
            Self::L => "L",
        }
    }
}

impl std::fmt::Display for Effort {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// ── Priority ──────────────────────────────────────────────────────────────

/// Task priority level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Priority {
    /// Low priority.
    Low,
    /// Medium priority.
    Medium,
    /// High priority.
    High,
    /// Critical priority.
    Critical,
}

impl Priority {
    /// Parse from a human-readable string (case-insensitive).
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "low" => Some(Self::Low),
            "medium" => Some(Self::Medium),
            "high" => Some(Self::High),
            "critical" => Some(Self::Critical),
            _ => None,
        }
    }

    /// Human-readable label.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Low => "Low",
            Self::Medium => "Medium",
            Self::High => "High",
            Self::Critical => "Critical",
        }
    }
}

impl std::fmt::Display for Priority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// ── PlanTask ──────────────────────────────────────────────────────────────

/// A single task parsed from a PLAN.md task table.
///
/// Unlike `spec::Task`, this uses typed `Effort` and `Priority` enums and
/// preserves the original `requirement` string for prompt construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanTask {
    /// Unique identifier, e.g. "T-001".
    pub id: String,
    /// Human-readable title.
    pub title: String,
    /// Requirement references from the task table (e.g. "FR-003, FR-004").
    pub requirement: String,
    /// Estimated effort.
    pub effort: Effort,
    /// Priority level.
    pub priority: Priority,
    /// IDs of prerequisite tasks.
    pub dependencies: Vec<String>,
    /// Current status.
    pub status: TaskStatus,
}

// ── PlanParser ────────────────────────────────────────────────────────────

/// Parser that extracts `PlanTask` structs from a PLAN.md markdown string.
pub struct PlanParser;

impl PlanParser {
    /// Parse all tasks from a PLAN.md string.
    ///
    /// Locates the `## Tasks` section, finds the markdown table, and parses
    /// each data row into a `PlanTask`. Malformed rows are skipped with a
    /// warning. Returns an error if zero valid rows are found (FR-025).
    pub fn parse(plan_md: &str) -> Result<Vec<PlanTask>, SpecError> {
        let tasks = Self::parse_raw(plan_md);
        if tasks.is_empty() {
            return Err(SpecError::PlanParse(
                "PLAN.md contains zero valid task rows in the ## Tasks table".to_string(),
            ));
        }
        Ok(tasks)
    }

    /// Parse tasks, returning an empty vec instead of an error when no rows found.
    fn parse_raw(plan_md: &str) -> Vec<PlanTask> {
        let mut tasks = Vec::new();
        let mut in_task_section = false;
        let mut header_seen = false;
        let mut has_status_column = false;

        for line in plan_md.lines() {
            let trimmed = line.trim();

            // Detect ## Tasks section
            if trimmed.eq_ignore_ascii_case("## Tasks") || trimmed.eq_ignore_ascii_case("### Tasks")
            {
                in_task_section = true;
                continue;
            }

            // Exit task section at next ## heading (but not ###)
            if in_task_section && trimmed.starts_with("## ") && !trimmed.starts_with("### ") {
                break;
            }

            if !in_task_section {
                continue;
            }

            // Skip non-table lines
            if !trimmed.starts_with('|') {
                continue;
            }

            // Parse header row to detect column layout
            if !header_seen && trimmed.contains("ID") && trimmed.contains("Title") {
                header_seen = true;
                let cells: Vec<&str> = trimmed
                    .split('|')
                    .map(str::trim)
                    .filter(|c| !c.is_empty())
                    .collect();
                has_status_column = cells.iter().any(|c| c.eq_ignore_ascii_case("Status"));
                continue;
            }

            // Skip separator row (|---|---|)
            if !header_seen {
                continue;
            }
            if trimmed.split('|').all(|c| {
                let t = c.trim();
                t.is_empty() || t.chars().all(|ch| ch == '-' || ch == ':' || ch == ' ')
            }) {
                continue;
            }

            // Parse data row
            let cells: Vec<&str> = trimmed
                .split('|')
                .map(str::trim)
                .filter(|c| !c.is_empty())
                .collect();

            // Need at least 6 columns: ID, Title, Requirement, Effort, Priority, Dependencies
            // With Status column: 7 columns: ID, Title, Requirement, Effort, Priority, Status, Dependencies
            if cells.len() < 6 {
                tracing::warn!("Skipping malformed task row: not enough columns");
                continue;
            }

            let id = cells[0].to_string();
            if !id.starts_with("T-") {
                tracing::warn!("Skipping row with invalid task ID: {}", id);
                continue;
            }

            // Validate ID format T-NNN (just warn, don't skip)
            if !id.starts_with("T-") || !id[2..].chars().all(|c| c.is_ascii_digit()) {
                tracing::warn!("Non-standard task ID format: {}", id);
            }
            let title = cells.get(1).copied().unwrap_or("").to_string();
            let requirement = cells.get(2).copied().unwrap_or("").to_string();
            let effort_str = cells.get(3).copied().unwrap_or("");
            let priority_str = cells.get(4).copied().unwrap_or("");

            let effort = if let Some(e) = Effort::parse(effort_str) {
                e
            } else {
                tracing::warn!(
                    "Task {}: unrecognized effort '{}', defaulting to M",
                    id,
                    effort_str
                );
                Effort::M
            };

            let priority = if let Some(p) = Priority::parse(priority_str) {
                p
            } else {
                tracing::warn!(
                    "Task {}: unrecognized priority '{}', defaulting to Medium",
                    id,
                    priority_str
                );
                Priority::Medium
            };

            let (status, dependencies) = if has_status_column && cells.len() >= 7 {
                // 7-column: ID | Title | Req | Effort | Priority | Status | Dependencies
                let status_str = cells.get(5).copied().unwrap_or("");
                let status = TaskStatus::parse(status_str).unwrap_or(TaskStatus::Pending);
                let deps_str = cells.get(6).copied().unwrap_or("");
                let deps = Self::parse_dependencies(deps_str);
                (status, deps)
            } else {
                // 6-column: ID | Title | Req | Effort | Priority | Dependencies
                let deps_str = cells.get(5).copied().unwrap_or("");
                let deps = Self::parse_dependencies(deps_str);
                (TaskStatus::Pending, deps)
            };

            tasks.push(PlanTask {
                id,
                title,
                requirement,
                effort,
                priority,
                dependencies,
                status,
            });
        }

        tasks
    }

    /// Parse a comma-separated dependency string.
    fn parse_dependencies(deps_str: &str) -> Vec<String> {
        let trimmed = deps_str.trim();
        if trimmed.is_empty() || trimmed == "—" || trimmed == "-" {
            return Vec::new();
        }
        trimmed
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    }
}

// ── Topological Sort ──────────────────────────────────────────────────────

/// Resolve task execution order using Kahn's algorithm (BFS topological sort).
///
/// Returns ordered task indices. Detects cycles and returns an error listing
/// the task IDs involved in the cycle (FR-006).
pub fn resolve_execution_order(tasks: &[PlanTask]) -> Result<Vec<usize>, SpecError> {
    let n = tasks.len();
    if n == 0 {
        return Ok(Vec::new());
    }

    // Build index by task ID
    let id_to_idx: HashMap<&str, usize> = tasks
        .iter()
        .enumerate()
        .map(|(i, t)| (t.id.as_str(), i))
        .collect();

    // Build adjacency list and in-degree counts
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut in_degree: Vec<usize> = vec![0; n];

    for (i, task) in tasks.iter().enumerate() {
        for dep_id in &task.dependencies {
            if let Some(&dep_idx) = id_to_idx.get(dep_id.as_str()) {
                adj[dep_idx].push(i);
                in_degree[i] += 1;
            } else {
                // Unknown dependency — warn but don't fail
                tracing::warn!("Task {} references unknown dependency {}", task.id, dep_id);
            }
        }
    }

    // Kahn's algorithm
    let mut queue: VecDeque<usize> = VecDeque::new();
    for (i, &deg) in in_degree.iter().enumerate() {
        if deg == 0 {
            queue.push_back(i);
        }
    }

    let mut order = Vec::with_capacity(n);
    while let Some(idx) = queue.pop_front() {
        order.push(idx);
        for &neighbor in &adj[idx] {
            in_degree[neighbor] -= 1;
            if in_degree[neighbor] == 0 {
                queue.push_back(neighbor);
            }
        }
    }

    if order.len() != n {
        // Cycle detected — find tasks remaining in the cycle
        let visited: HashSet<usize> = order.iter().copied().collect();
        let cycle_ids: Vec<String> = tasks
            .iter()
            .enumerate()
            .filter(|(i, _)| !visited.contains(i))
            .map(|(_, t)| t.id.clone())
            .collect();
        return Err(SpecError::DependencyCycle {
            task_ids: cycle_ids,
        });
    }

    Ok(order)
}

/// Filter execution order to include only a target task and its transitive
/// dependencies. Used by the `--task` flag (FR-012).
pub fn filter_for_task(tasks: &[PlanTask], target_id: &str) -> Result<Vec<usize>, SpecError> {
    let id_to_idx: HashMap<&str, usize> = tasks
        .iter()
        .enumerate()
        .map(|(i, t)| (t.id.as_str(), i))
        .collect();

    let target_idx = id_to_idx
        .get(target_id)
        .ok_or_else(|| SpecError::UnknownId(target_id.to_string()))?;

    // BFS backwards through dependencies to find all ancestors
    let mut needed: HashSet<usize> = HashSet::new();
    let mut stack: VecDeque<usize> = VecDeque::new();
    stack.push_back(*target_idx);

    while let Some(idx) = stack.pop_front() {
        if needed.contains(&idx) {
            continue;
        }
        needed.insert(idx);
        for dep_id in &tasks[idx].dependencies {
            if let Some(&dep_idx) = id_to_idx.get(dep_id.as_str()) {
                stack.push_back(dep_idx);
            }
        }
    }

    // Get full execution order, then filter to only needed tasks
    let full_order = resolve_execution_order(tasks)?;
    let filtered: Vec<usize> = full_order
        .into_iter()
        .filter(|i| needed.contains(i))
        .collect();

    Ok(filtered)
}

/// Skip already-completed tasks for resume support (FR-020).
///
/// Returns the execution order with completed tasks removed. Tasks that are
/// still pending or blocked are kept in their topological order; the
/// sequential driver processes them one at a time so dependencies are satisfied
/// naturally, and any blocked task stops the run before its dependents run.
#[must_use]
pub fn filter_for_resume(tasks: &[PlanTask], order: &[usize]) -> Vec<usize> {
    order
        .iter()
        .copied()
        .filter(|&i| tasks[i].status != TaskStatus::Completed)
        .collect()
}

#[cfg(test)]
mod tests {
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
        assert_eq!(PlanParser::parse_dependencies("—"), Vec::<String>::new());
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
| T-001 | Define types | FR-003 | S | Critical | — |
| T-002 | Build parser | FR-004 | M | High | T-001 |
| T-003 | Add tests | FR-005 | M | High | T-002 |

## Details
";
        let tasks = PlanParser::parse(md).unwrap();
        assert_eq!(tasks.len(), 3);
        assert_eq!(tasks[0].id, "T-001");
        assert_eq!(tasks[0].effort, Effort::S);
        assert_eq!(tasks[0].priority, Priority::Critical);
        assert!(tasks[0].dependencies.is_empty());
        assert_eq!(tasks[1].dependencies, vec!["T-001"]);
        assert_eq!(tasks[2].dependencies, vec!["T-002"]);
    }

    #[test]
    fn test_parse_with_status_column() {
        let md = r"
## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|---|---|---|---|---|---|---|
| T-001 | Define types | FR-003 | S | Critical | completed | — |
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
| T-001 | Valid task | FR-003 | S | Critical | — |
| bad-id | Invalid | FR-004 | M | High | — |
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
            },
            PlanTask {
                id: "T-002".into(),
                title: "Second".into(),
                requirement: "FR-002".into(),
                effort: Effort::M,
                priority: Priority::High,
                dependencies: vec!["T-001".into()],
                status: TaskStatus::Pending,
            },
            PlanTask {
                id: "T-003".into(),
                title: "Third".into(),
                requirement: "FR-003".into(),
                effort: Effort::L,
                priority: Priority::Medium,
                dependencies: vec!["T-002".into()],
                status: TaskStatus::Pending,
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
        // T-001 → T-002, T-001 → T-003, T-002 → T-004, T-003 → T-004
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
                effort: Effort::S,
                priority: Priority::High,
                dependencies: vec!["T-001".into()],
                status: TaskStatus::Pending,
            },
            PlanTask {
                id: "T-004".into(),
                title: "D".into(),
                requirement: "FR-004".into(),
                effort: Effort::M,
                priority: Priority::Medium,
                dependencies: vec!["T-002".into(), "T-003".into()],
                status: TaskStatus::Pending,
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
            },
            PlanTask {
                id: "T-002".into(),
                title: "B".into(),
                requirement: "FR-002".into(),
                effort: Effort::S,
                priority: Priority::Critical,
                dependencies: vec!["T-001".into()],
                status: TaskStatus::Pending,
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
        assert!(order.is_empty());
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
            },
            PlanTask {
                id: "T-002".into(),
                title: "B".into(),
                requirement: "FR-002".into(),
                effort: Effort::S,
                priority: Priority::High,
                dependencies: vec!["T-001".into()],
                status: TaskStatus::InProgress,
            },
            PlanTask {
                id: "T-003".into(),
                title: "C".into(),
                requirement: "FR-003".into(),
                effort: Effort::M,
                priority: Priority::High,
                dependencies: vec!["T-002".into()],
                status: TaskStatus::Pending,
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
            },
            PlanTask {
                id: "T-002".into(),
                title: "B".into(),
                requirement: "FR-002".into(),
                effort: Effort::S,
                priority: Priority::High,
                dependencies: vec!["T-001".into()],
                status: TaskStatus::Blocked,
            },
        ];
        let order = resolve_execution_order(&tasks).unwrap();
        let resumed = filter_for_resume(&tasks, &order);
        // T-002 should be unblocked since T-001 is now completed
        assert_eq!(resumed.len(), 1);
        assert_eq!(tasks[resumed[0]].id, "T-002");
    }
}
