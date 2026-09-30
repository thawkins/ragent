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

// -- ImplOptions --

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
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the `--task` target.
    #[must_use]
    pub fn with_task(mut self, task_id: impl Into<String>) -> Self {
        self.task_id = Some(task_id.into());
        self
    }

    /// Enable `--dry-run` mode.
    #[must_use]
    pub const fn with_dry_run(mut self) -> Self {
        self.dry_run = true;
        self
    }
}

// -- ImplResult --

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
    /// Milestone groupings for the tasks that will be executed, used by
    /// the TUI to create parent/subtask session tasks.
    pub milestone_groups: Vec<MilestoneGroup>,
}

/// A group of spec tasks that belong to a single milestone.
///
/// Used by the TUI to create a parent session task for the milestone and
/// subtasks for each spec task inside it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MilestoneGroup {
    /// Milestone display name (heading text from `### Milestone N: Name`).
    pub name: String,
    /// Deliverable text from the milestone section, if present.
    pub deliverable: String,
    /// Spec task IDs that belong to this milestone, in execution order.
    pub task_ids: Vec<String>,
}

// -- SpecImplRunner --

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
    /// Parsed milestones from PLAN.md, used to look up deliverables.
    milestones: Vec<crate::plan_parser::Milestone>,
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
        let milestones = PlanParser::parse_milestones(&spec.plan_md);

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
            milestones,
            options,
        })
    }

    /// Get the spec name.
    #[must_use]
    pub fn spec_name(&self) -> &str {
        &self.spec_name
    }

    /// Get the parsed tasks.
    #[must_use]
    pub fn tasks(&self) -> &[PlanTask] {
        &self.tasks
    }

    /// Get the execution order.
    #[must_use]
    pub fn execution_order(&self) -> &[usize] {
        &self.execution_order
    }

    /// Number of tasks that will be executed in this run
    /// (i.e. the length of [`execution_order`](Self::execution_order),
    /// after resume filtering).
    #[must_use]
    pub const fn total_to_execute(&self) -> usize {
        self.execution_order.len()
    }

    /// Group the tasks that will be executed by milestone.
    ///
    /// Tasks without an assigned milestone are placed under a synthetic
    /// "Unmapped Tasks" group so every spec task has a parent session task.
    /// The groups and task IDs are returned in execution order.
    #[must_use]
    pub fn milestone_groups(&self) -> Vec<MilestoneGroup> {
        let deliverables: HashMap<&str, &str> = self
            .milestones
            .iter()
            .map(|m| (m.name.as_str(), m.deliverable.as_str()))
            .collect();
        let mut groups: Vec<MilestoneGroup> = Vec::new();
        let mut index: HashMap<String, usize> = HashMap::new();
        for &idx in &self.execution_order {
            let task = &self.tasks[idx];
            // Borrow the milestone name when present; allocate only for the
            // synthetic unmapped group label.
            let owned_name;
            let name: &str = match task.milestone.as_deref() {
                Some(m) => m,
                None => {
                    owned_name = "Unmapped Tasks".to_string();
                    &owned_name
                }
            };
            match index.get(name) {
                Some(&i) => {
                    groups[i].task_ids.push(task.id.clone());
                }
                None => {
                    index.insert(name.to_string(), groups.len());
                    groups.push(MilestoneGroup {
                        name: name.to_string(),
                        deliverable: deliverables
                            .get(name)
                            .map(|s| s.to_string())
                            .unwrap_or_default(),
                        task_ids: vec![task.id.clone()],
                    });
                }
            }
        }
        groups
    }

    /// Get the task ID at the given 1-based rank in the execution order.
    ///
    /// Returns `None` if `rank` is out of range (rank < 1 or rank > total).
    #[must_use]
    pub fn task_id_at(&self, rank: usize) -> Option<&str> {
        self.execution_order
            .get(rank.checked_sub(1)?)
            .map(|&idx| self.tasks[idx].id.as_str())
    }

    /// Build a focused single-task prompt for the task at the given 1-based
    /// rank in the execution order.
    ///
    /// The prompt tells the agent to implement exactly one task and then use
    /// `spec_task_update` to mark it `completed` (or `blocked` on failure).
    /// Returns `None` if `rank` is out of range.
    ///
    /// This is the per-task prompt used by the TUI's sequential task-driving
    /// loop: after each agent turn ends, the TUI checks the task status and,
    /// if completed, dispatches the next task's prompt.
    #[must_use]
    pub fn task_prompt(&self, rank: usize) -> Option<String> {
        let total = self.execution_order.len();
        let idx = *self.execution_order.get(rank.checked_sub(1)?)?;
        let task = &self.tasks[idx];
        Some(Self::build_single_task_prompt(
            task,
            &self.spec_name,
            rank,
            total,
        ))
    }

    /// Build the prompt for a single task (FR-021), with a header noting the
    /// task's position in the overall run.
    ///
    /// Public so tests can exercise it without constructing a full
    /// `SpecImplRunner` (which requires on-disk spec files).
    #[must_use]
    pub fn build_single_task_prompt(
        task: &PlanTask,
        spec_name: &str,
        rank: usize,
        total: usize,
    ) -> String {
        let mut prompt = format!(
            "Implement task **{}** ({}) for spec **{}**.\n\n\
             This is task {rank} of {total} in the implementation plan.\n\n",
            task.id, task.title, spec_name,
        );
        prompt.push_str(&format!(
            "#### Task {}: {}\n\n**Requirement:** {}\n\n",
            task.id, task.title, task.requirement,
        ));
        prompt.push_str(&format!(
            "Implement this task now. BEFORE starting work, you MUST use the \
             `spec_task_update` tool with spec_id=\"{spec}\", task_id=\"{id}\", \
             status=\"in_progress\". After completing it, you MUST use the \
             same tool with status=\"completed\". \
             If the task cannot be completed, mark it as `blocked` with the \
             same tool.\n",
            spec = spec_name,
            id = task.id,
        ));
        prompt
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
                milestone_groups: self.milestone_groups(),
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
            milestone_groups: self.milestone_groups(),
        })
    }

    // -- Prompt Construction --

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
                "{}. **{}** - {} (Effort: {}, Priority: {})\n",
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
            "At the start of this task, use `spec_task_update` \
             with spec_id=\"{}\", task_id=\"{}\", status=\"in_progress\". \
             After completing this task, use `spec_task_update` \
             with spec_id=\"{}\", task_id=\"{}\", status=\"completed\".\n",
            spec_name, task.id, spec_name, task.id
        ));

        prompt
    }

    // -- Display Helpers --

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
        lines.push(format!("**Effort estimate:** {effort_summary}"));
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
                "{}. `{}` - {} [{}] (deps: {})",
                rank + 1,
                task.id,
                task.title,
                task.effort,
                deps
            ));
        }

        // Append advisory file-order warning if present (FR-014, T-025)
        if let Some(warning) = self.build_file_order_warning() {
            lines.push(String::new());
            lines.push(warning);
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
                "-".to_string()
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

        // Append advisory file-order warning if present (FR-014, T-025)
        if let Some(warning) = self.build_file_order_warning() {
            lines.push(String::new());
            lines.push(warning);
        }

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
            parts.push(format!("{s}xS"));
        }
        if m > 0 {
            parts.push(format!("{m}xM"));
        }
        if l > 0 {
            parts.push(format!("{l}xL"));
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

    // -- File Creation Order (FR-014, T-025) --

    /// Build an advisory warning when tasks in the execution order violate the
    /// test-first file creation order (FR-014).
    ///
    /// The expected order is: contracts -> contract tests -> integration tests
    /// -> e2e tests -> unit tests -> source files. Each task is categorised into
    /// a tier by analysing its title and requirement text for keywords. If a
    /// lower-numbered tier task appears after a higher-numbered tier task, a
    /// violation is recorded.
    ///
    /// Returns `None` when no violations are detected (or when fewer than two
    /// tasks are to execute). The warning is advisory only - it does not block
    /// execution.
    #[must_use]
    pub fn build_file_order_warning(&self) -> Option<String> {
        if self.execution_order.len() < 2 {
            return None;
        }

        let tiered: Vec<(usize, &PlanTask, usize)> = self
            .execution_order
            .iter()
            .enumerate()
            .map(|(rank, &idx)| {
                let task = &self.tasks[idx];
                let tier = file_creation_tier(task);
                (rank, task, tier)
            })
            .collect();

        let mut violations: Vec<String> = Vec::new();
        let mut max_tier_seen = tiered[0].2;

        for &(rank, task, tier) in tiered.iter().skip(1) {
            if tier < max_tier_seen {
                // Find the task that established the higher tier
                // Find the task that established the higher tier. The element
                // that set `max_tier_seen` always precedes the current rank, so
                // the rfind cannot fail; a miss would only skip the advisory
                // line, never panic.
                let Some(&(offender_rank, offender_task, _)) = tiered
                    .iter()
                    .take(rank)
                    .rfind(|&&(_, _, t)| t == max_tier_seen)
                else {
                    continue;
                };
                violations.push(format!(
                    "  - `{}` (step {}) appears after `{}` (step {}): \
                     {} should come before {} per the test-first ordering",
                    task.id,
                    rank + 1,
                    offender_task.id,
                    offender_rank + 1,
                    tier_label(tier),
                    tier_label(max_tier_seen)
                ));
            } else {
                max_tier_seen = tier;
            }
        }

        if violations.is_empty() {
            return None;
        }

        let mut warning = String::from(
            "### [!] File Creation Order Advisory\n\n\
             The following tasks may violate the test-first file creation order \
             (contracts -> contract tests -> integration tests -> e2e tests -> unit \
             tests -> source files). Consider reordering tasks to maintain \
             test-first discipline:\n",
        );
        for v in &violations {
            warning.push_str(v);
            warning.push('\n');
        }
        warning.push_str(
            "\nThis is advisory only - implementation will proceed in the listed \
             order. To suppress this warning, reorder tasks in PLAN.md or adjust \
             task titles to clearly indicate file type.",
        );
        Some(warning)
    }
}

/// Categorise a task into a file-creation tier (1-6) based on its title and
/// requirement text (FR-014, T-025).
///
/// | Tier | Category | Keywords |
/// |------|----------|----------|
/// | 1 | Contracts | `contract`, `api spec`, `schema`, `interface` |
/// | 2 | Contract tests | `contract test` |
/// | 3 | Integration tests | `integration test` |
/// | 4 | E2E tests | `e2e`, `end-to-end`, `end to end` |
/// | 5 | Unit tests | `unit test`, `test` |
/// | 6 | Source files | (default - no test/contract keywords) |
///
/// More specific patterns are checked first so that "contract test" is not
/// mis-categorised as "contract".
#[must_use]
fn file_creation_tier(task: &PlanTask) -> usize {
    let text = format!("{} {}", task.title, task.requirement).to_lowercase();

    // Check most specific patterns first
    if text.contains("contract test") || text.contains("contract-test") {
        return 2;
    }
    if text.contains("integration test") {
        return 3;
    }
    if text.contains("e2e") || text.contains("end-to-end test") || text.contains("end to end test")
    {
        return 4;
    }
    if text.contains("unit test") {
        return 5;
    }
    if text.contains("contract")
        || text.contains("api spec")
        || text.contains("schema definition")
        || text.contains("interface definition")
    {
        return 1;
    }
    if text.contains("test") {
        return 5;
    }
    // Default: source files
    6
}

/// Human-readable label for a file-creation tier.
#[must_use]
const fn tier_label(tier: usize) -> &'static str {
    match tier {
        1 => "contracts",
        2 => "contract tests",
        3 => "integration tests",
        4 => "e2e tests",
        5 => "unit tests",
        _ => "source files",
    }
}

// -- Helper Functions --

/// Check if a transition is valid (re-exported from manager for convenience).
fn is_valid_transition(from: SpecStatus, to: SpecStatus) -> bool {
    crate::manager::is_valid_transition(from, to)
}

/// Build a progress update message for a completed task (FR-015).
#[must_use]
pub fn build_progress_update(
    spec_name: &str,
    task_id: &str,
    completed: usize,
    total: usize,
    next_task_id: Option<&str>,
) -> String {
    let next = match next_task_id {
        Some(id) => format!(" - Next: {id}"),
        None => String::new(),
    };
    format!("[ok] {task_id} ({completed}/{total}){next} - spec {spec_name}")
}

/// Build a completion summary (FR-016).
#[must_use]
pub fn build_completion_summary(spec_name: &str, total: usize) -> String {
    format!(
        "[done] All {total} tasks completed for spec **{spec_name}**. \
         Spec status has been set to `implemented`."
    )
}

/// Build a cancellation summary (FR-017).
#[must_use]
pub fn build_cancellation_summary(spec_name: &str, completed: usize, total: usize) -> String {
    format!(
        "[!] Implementation cancelled for spec **{spec_name}**. \
         Completed {completed}/{total} tasks. Spec status remains `in_progress`. \
         Run `/spec impl {spec_name}` again to resume."
    )
}

/// Build a blocked task summary (FR-011).
#[must_use]
pub fn build_blocked_summary(task_id: &str, dependent_ids: &[String]) -> String {
    let deps = if dependent_ids.is_empty() {
        String::new()
    } else {
        format!(" (also blocked: {})", dependent_ids.join(", "))
    };
    format!("[blocked] Task {task_id} blocked{deps}")
}

/// Find all tasks that transitively depend on a blocked task.
#[must_use]
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
                    "Unknown option: {other}. Valid options: --task <ID>, --dry-run"
                )));
            }
        }
        i += 1;
    }

    Ok((spec_name, options))
}

#[cfg(test)]
#[path = "../tests/inline/impl_runner_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../tests/inline/impl_runner_milestone_tests.rs"]
mod milestone_tests;
