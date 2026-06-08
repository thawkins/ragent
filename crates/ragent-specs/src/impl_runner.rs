//! Spec implementation runner: orchestrates task execution from a PLAN.md.
//!
//! `SpecImplRunner` reads a spec's PLAN.md, parses task tables, resolves
//! dependency order, and constructs agent prompts that drive the agent through
//! implementing each task sequentially. The runner produces:
//!
//! - A summary of the execution plan (before execution)
//! - Per-task prompts (injected one at a time into the agent session)
//! - Progress updates as tasks complete
//! - A completion summary when all tasks finish

use crate::error::SpecError;
use crate::manager::SpecManager;
use crate::plan_parser::{
    Effort, PlanParser, PlanTask, filter_for_resume, filter_for_task, resolve_execution_order,
};
use crate::spec::{SpecId, SpecStatus};
use std::collections::HashMap;
use std::path::PathBuf;

// ── ImplOptions ───────────────────────────────────────────────────────────

/// Options for `/spec impl` invocation.
#[derive(Debug, Clone, Default)]
pub struct ImplOptions {
    /// Execute only the specified task and its transitive dependencies.
    pub task_id: Option<String>,
    /// Display execution order without actually running tasks.
    pub dry_run: bool,
}

impl ImplOptions {
    /// Create default options.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the `--task` target.
    pub fn with_task(mut self, task_id: impl Into<String>) -> Self {
        self.task_id = Some(task_id.into());
        self
    }

    /// Enable `--dry-run` mode.
    pub fn with_dry_run(mut self) -> Self {
        self.dry_run = true;
        self
    }
}

// ── ImplResult ─────────────────────────────────────────────────────────────

/// Result of a `/spec impl` run.
#[derive(Debug, Clone)]
pub struct ImplResult {
    /// The spec name that was implemented.
    pub spec_name: String,
    /// Total number of tasks in the plan.
    pub total_tasks: usize,
    /// Number of tasks completed in this run.
    pub completed_count: usize,
    /// Number of tasks blocked due to failures.
    pub blocked_count: usize,
    /// Number of tasks skipped (already completed from previous run).
    pub skipped_count: usize,
    /// Execution order (indices into the task list).
    pub execution_order: Vec<usize>,
    /// The agent prompt to send for execution (empty for dry-run).
    pub prompt: String,
    /// Summary text for display.
    pub summary: String,
}

// ── SpecImplRunner ────────────────────────────────────────────────────────

/// Orchestrates the execution of a spec's implementation plan.
///
/// The runner parses the PLAN.md, resolves dependencies, and constructs
/// a compound prompt that instructs the agent to implement tasks in order.
/// Task status updates are tracked via the spec management system.
#[derive(Debug, Clone)]
pub struct SpecImplRunner {
    /// The spec name.
    spec_name: String,
    /// Root directory of the specs folder.
    specs_root: PathBuf,
    /// Parsed tasks from PLAN.md.
    tasks: Vec<PlanTask>,
    /// Execution order (indices into `tasks`).
    execution_order: Vec<usize>,
    /// Options for this run.
    options: ImplOptions,
}

impl SpecImplRunner {
    /// Create a new runner for the given spec.
    ///
    /// Reads and parses the PLAN.md from the spec directory, resolves
    /// dependencies, and prepares for execution.
    pub async fn new(
        spec_name: &str,
        specs_root: PathBuf,
        options: ImplOptions,
    ) -> Result<Self, SpecError> {
        let spec_id = SpecId::new(spec_name)
            .ok_or_else(|| SpecError::InvalidSpecId(spec_name.to_string()))?;

        let mgr = SpecManager::new(&specs_root);
        let spec = mgr.read_spec(&spec_id).await?;

        // Check if already implemented (FR-026)
        if matches!(spec.status, SpecStatus::Implemented | SpecStatus::Verified) {
            return Err(SpecError::AlreadyImplemented {
                spec_id: spec_name.to_string(),
                status: spec.status.as_str().to_string(),
            });
        }

        // Parse PLAN.md tasks
        let tasks = PlanParser::parse(&spec.plan_md)?;

        // Resolve execution order
        let execution_order = if let Some(ref task_id) = options.task_id {
            filter_for_task(&tasks, task_id)?
        } else {
            resolve_execution_order(&tasks)?
        };

        // Apply resume filter: skip already-completed tasks
        let execution_order = filter_for_resume(&tasks, &execution_order);

        Ok(Self {
            spec_name: spec_name.to_string(),
            specs_root,
            tasks,
            execution_order,
            options,
        })
    }

    /// Get the spec name.
    pub fn spec_name(&self) -> &str {
        &self.spec_name
    }

    /// Get the parsed tasks.
    pub fn tasks(&self) -> &[PlanTask] {
        &self.tasks
    }

    /// Get the execution order.
    pub fn execution_order(&self) -> &[usize] {
        &self.execution_order
    }

    /// Run the implementation plan.
    ///
    /// Returns an `ImplResult` with the prompt to inject into the agent
    /// session and a summary for display.
    pub async fn run(&self) -> Result<ImplResult, SpecError> {
        let total_tasks = self.execution_order.len();
        let skipped_count = self.tasks.len() - total_tasks;

        // Build summary
        let summary = self.build_summary();

        // Dry-run: just return the plan display
        if self.options.dry_run {
            return Ok(ImplResult {
                spec_name: self.spec_name.clone(),
                total_tasks,
                completed_count: 0,
                blocked_count: 0,
                skipped_count,
                execution_order: self.execution_order.clone(),
                prompt: String::new(),
                summary: self.build_dry_run_display(),
            });
        }

        // Build the compound prompt
        let prompt = self.build_execution_prompt();

        // Transition spec to in_progress (FR-007)
        let spec_id = SpecId::new(&self.spec_name)
            .ok_or_else(|| SpecError::InvalidSpecId(self.spec_name.clone()))?;
        let mgr = SpecManager::new(&self.specs_root);
        let mut spec = mgr.read_spec(&spec_id).await?;

        if spec.status == SpecStatus::Approved || spec.status == SpecStatus::Draft {
            // Only transition if not already in_progress
            if is_valid_transition(spec.status, SpecStatus::InProgress) {
                mgr.transition(&mut spec, SpecStatus::InProgress, "spec-impl")
                    .await?;
            }
        }

        Ok(ImplResult {
            spec_name: self.spec_name.clone(),
            total_tasks,
            completed_count: 0,
            blocked_count: 0,
            skipped_count,
            execution_order: self.execution_order.clone(),
            prompt,
            summary,
        })
    }

    // ── Prompt Construction ──────────────────────────────────────────────

    /// Build the compound agent prompt for executing all tasks.
    ///
    /// The prompt instructs the agent to implement each task in dependency
    /// order, using `spec_task_update` to mark progress after each task.
    fn build_execution_prompt(&self) -> String {
        let mut prompt = format!(
            "Implement the following tasks in the exact order listed for spec **{}**.\n\n\
             After completing EACH task, you MUST use the `spec_task_update` tool \
             with spec_id=\"{}\" to mark it as completed. \
             If a task fails, mark it as `blocked` using the same tool.\n\n",
            self.spec_name, self.spec_name
        );

        prompt.push_str("### Execution Order\n\n");
        for (rank, &idx) in self.execution_order.iter().enumerate() {
            let task = &self.tasks[idx];
            prompt.push_str(&format!(
                "{}. **{}** — {} (Effort: {}, Priority: {})\n",
                rank + 1,
                task.id,
                task.title,
                task.effort,
                task.priority
            ));
            if !task.dependencies.is_empty() {
                prompt.push_str(&format!(
                    "   - Depends on: {}\n",
                    task.dependencies.join(", ")
                ));
            }
        }

        prompt.push_str("\n### Task Details\n\n");
        for (rank, &idx) in self.execution_order.iter().enumerate() {
            let task = &self.tasks[idx];
            prompt.push_str(&Self::build_task_prompt(task, &self.spec_name, rank + 1));
            prompt.push('\n');
        }

        prompt.push_str(&format!(
            "\n### Completion\n\n\
             When ALL tasks are complete, use `spec_task_update` to mark the spec \
             status as `implemented` by transitioning spec **{}** from `in_progress` \
             to `implemented`.\n",
            self.spec_name
        ));

        prompt
    }

    /// Build the prompt for a single task (FR-021).
    fn build_task_prompt(task: &PlanTask, spec_name: &str, rank: usize) -> String {
        let mut prompt = format!(
            "#### {}. Task {}: {}\n\n**Requirement:** {}\n\n",
            rank, task.id, task.title, task.requirement
        );

        prompt.push_str(&format!(
            "After completing this task, use `spec_task_update` \
             with spec_id=\"{}\", task_id=\"{}\", status=\"completed\".\n",
            spec_name, task.id
        ));

        prompt
    }

    // ── Display Helpers ──────────────────────────────────────────────────

    /// Build the initial progress summary (FR-014).
    fn build_summary(&self) -> String {
        let total = self.execution_order.len();
        let skipped = self.tasks.len() - total;
        let effort_summary = self.effort_summary();

        let mut lines = vec![format!(
            "From: /spec impl\n\n## Implementation Plan: {}\n",
            self.spec_name
        )];

        lines.push(format!(
            "**Total tasks:** {} ({} to execute, {} already completed)",
            self.tasks.len(),
            total,
            skipped
        ));
        lines.push(format!("**Effort estimate:** {}", effort_summary));
        lines.push(String::new());

        lines.push("### Execution Order\n".to_string());
        for (rank, &idx) in self.execution_order.iter().enumerate() {
            let task = &self.tasks[idx];
            let deps = if task.dependencies.is_empty() {
                "none".to_string()
            } else {
                task.dependencies.join(", ")
            };
            lines.push(format!(
                "{}. `{}` — {} [{}] (deps: {})",
                rank + 1,
                task.id,
                task.title,
                task.effort,
                deps
            ));
        }

        lines.join("\n")
    }

    /// Build the dry-run display (FR-013).
    fn build_dry_run_display(&self) -> String {
        let mut lines = vec![
            format!(
                "From: /spec impl --dry-run\n\n## Dry Run: {}\n",
                self.spec_name
            ),
            format!("**Tasks to execute:** {}", self.execution_order.len()),
            String::new(),
            "| # | ID | Title | Effort | Priority | Dependencies | Status |".to_string(),
            "|---|----|-------|--------|----------|--------------|--------|".to_string(),
        ];

        for (rank, &idx) in self.execution_order.iter().enumerate() {
            let task = &self.tasks[idx];
            let deps = if task.dependencies.is_empty() {
                "—".to_string()
            } else {
                task.dependencies.join(", ")
            };
            lines.push(format!(
                "| {} | {} | {} | {} | {} | {} | {} |",
                rank + 1,
                task.id,
                task.title,
                task.effort,
                task.priority,
                deps,
                task.status.as_str()
            ));
        }

        lines.push(String::new());
        lines.push(
            "No tasks were executed. Remove `--dry-run` to begin implementation.".to_string(),
        );

        lines.join("\n")
    }

    /// Summarize total effort across all tasks to execute.
    fn effort_summary(&self) -> String {
        let mut s = 0usize;
        let mut m = 0usize;
        let mut l = 0usize;
        for &idx in &self.execution_order {
            match self.tasks[idx].effort {
                Effort::S => s += 1,
                Effort::M => m += 1,
                Effort::L => l += 1,
            }
        }
        let mut parts = Vec::new();
        if s > 0 {
            parts.push(format!("{}×S", s));
        }
        if m > 0 {
            parts.push(format!("{}×M", m));
        }
        if l > 0 {
            parts.push(format!("{}×L", l));
        }
        if parts.is_empty() {
            "none".to_string()
        } else {
            parts.join(", ")
        }
    }

    /// Extract requirement text from SPEC.md for a given requirement reference
    /// (FR-022).
    ///
    /// Looks up requirement IDs (e.g. "FR-014") in the spec and returns
    /// the full text for each.
    pub async fn resolve_requirements(
        specs_root: &PathBuf,
        spec_name: &str,
        requirement_refs: &[String],
    ) -> HashMap<String, String> {
        let spec_id = match SpecId::new(spec_name) {
            Some(id) => id,
            None => return HashMap::new(),
        };
        let mgr = SpecManager::new(specs_root);
        let spec = match mgr.read_spec(&spec_id).await {
            Ok(s) => s,
            Err(_) => return HashMap::new(),
        };

        let mut resolved = HashMap::new();
        for ref_id in requirement_refs {
            for req in &spec.requirements {
                if req.id == *ref_id {
                    resolved.insert(ref_id.clone(), req.text.clone());
                }
            }
        }
        resolved
    }
}

// ── Helper Functions ───────────────────────────────────────────────────────

/// Check if a transition is valid (re-exported from manager for convenience).
fn is_valid_transition(from: SpecStatus, to: SpecStatus) -> bool {
    crate::manager::is_valid_transition(from, to)
}

/// Build a progress update message for a completed task (FR-015).
pub fn build_progress_update(
    spec_name: &str,
    task_id: &str,
    completed: usize,
    total: usize,
    next_task_id: Option<&str>,
) -> String {
    let next = match next_task_id {
        Some(id) => format!(" — Next: {}", id),
        None => String::new(),
    };
    format!(
        "✅ {} ({}/{}){} — spec {}",
        task_id, completed, total, next, spec_name
    )
}

/// Build a completion summary (FR-016).
pub fn build_completion_summary(spec_name: &str, total: usize) -> String {
    format!(
        "🎉 All {} tasks completed for spec **{}**. \
         Spec status has been set to `implemented`.",
        total, spec_name
    )
}

/// Build a cancellation summary (FR-017).
pub fn build_cancellation_summary(spec_name: &str, completed: usize, total: usize) -> String {
    format!(
        "⚠️ Implementation cancelled for spec **{}**. \
         Completed {}/{} tasks. Spec status remains `in_progress`. \
         Run `/spec impl {}` again to resume.",
        spec_name, completed, total, spec_name
    )
}

/// Build a blocked task summary (FR-011).
pub fn build_blocked_summary(task_id: &str, dependent_ids: &[String]) -> String {
    let deps = if dependent_ids.is_empty() {
        String::new()
    } else {
        format!(" (also blocked: {})", dependent_ids.join(", "))
    };
    format!("🚫 Task {} blocked{}", task_id, deps)
}

/// Find all tasks that transitively depend on a blocked task.
pub fn find_dependents(tasks: &[PlanTask], blocked_id: &str) -> Vec<String> {
    let _id_to_idx: HashMap<&str, usize> = tasks
        .iter()
        .enumerate()
        .map(|(i, t)| (t.id.as_str(), i))
        .collect();

    let mut dependents = Vec::new();
    let mut visited = std::collections::HashSet::new();
    let mut stack = vec![blocked_id];

    while let Some(id) = stack.pop() {
        if visited.contains(id) {
            continue;
        }
        visited.insert(id);

        for task in tasks {
            if task.dependencies.iter().any(|d| d == id) && !visited.contains(task.id.as_str()) {
                dependents.push(task.id.clone());
                stack.push(&task.id);
            }
        }
    }

    dependents
}

/// Parse the `/spec impl` argument string to extract spec name and options.
pub fn parse_impl_args(args: &str) -> Result<(String, ImplOptions), SpecError> {
    let parts: Vec<&str> = args.split_whitespace().collect();
    if parts.is_empty() {
        return Err(SpecError::PlanParse(
            "Usage: /spec impl <specname> [--task <ID>] [--dry-run]".to_string(),
        ));
    }

    let spec_name = parts[0].to_string();
    let mut options = ImplOptions::new();

    let mut i = 1;
    while i < parts.len() {
        match parts[i] {
            "--task" => {
                i += 1;
                if let Some(task_id) = parts.get(i) {
                    options.task_id = Some(task_id.to_string());
                } else {
                    return Err(SpecError::PlanParse(
                        "--task requires a task ID argument".to_string(),
                    ));
                }
            }
            "--dry-run" => {
                options.dry_run = true;
            }
            other => {
                return Err(SpecError::PlanParse(format!(
                    "Unknown option: {}. Valid options: --task <ID>, --dry-run",
                    other
                )));
            }
        }
        i += 1;
    }

    Ok((spec_name, options))
}

#[cfg(test)]
mod tests {
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
    }

    #[test]
    fn test_effort_summary_calculation() {
        // Test via the runner's internal method indirectly
        let runner = SpecImplRunner {
            spec_name: "test".into(),
            specs_root: PathBuf::from("/tmp"),
            tasks: vec![
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
                    effort: Effort::M,
                    priority: Priority::High,
                    dependencies: vec!["T-001".into()],
                    status: TaskStatus::Pending,
                },
                PlanTask {
                    id: "T-003".into(),
                    title: "C".into(),
                    requirement: "FR-003".into(),
                    effort: Effort::L,
                    priority: Priority::Medium,
                    dependencies: vec!["T-002".into()],
                    status: TaskStatus::Pending,
                },
            ],
            execution_order: vec![0, 1, 2],
            options: ImplOptions::default(),
        };
        let summary = runner.effort_summary();
        assert_eq!(summary, "1×S, 1×M, 1×L");
    }
}
