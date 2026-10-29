//! Task list types and the file-locked `TaskStore`.
//!
//! `tasks.json` is shared among all teammates and the lead.  Concurrent writes
//! are serialised using an exclusive `flock` on the file via the `fs2` crate.

use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, Utc};
use fs2::FileExt as _;
use serde::{Deserialize, Serialize};

use crate::team::config::HookEvent;
use crate::team::manager::HookOutcome;
use crate::team::store::TeamStore;

// -- Task status -------------------------------------------------------------

/// Lifecycle state of a single task.
///
/// Serialises to snake_case (`"pending"`, `"in_progress"`, `"completed"`,
/// `"cancelled"`) via `#[serde(rename_all = "lowercase")]` and via
/// [`TaskStatus::as_str`] (M5-T1). All output paths (serde, Debug-derived
/// tool output, SSE) produce the same snake_case string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    /// Waiting to be claimed.
    #[default]
    Pending,
    /// Claimed by a teammate and actively being worked on.
    InProgress,
    /// Successfully completed.
    Completed,
    /// Cancelled by the lead.
    Cancelled,
}

impl TaskStatus {
    /// Return the canonical snake_case string used for serialization, tool
    /// output, and SSE (M5-T1).
    ///
    /// This matches the `#[serde(rename_all = "lowercase")]` on-disk format
    /// (`"pending"`, `"in_progress"`, `"completed"`, `"cancelled"`).
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::InProgress => "in_progress",
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
        }
    }

    /// Whether the task is in a terminal status (`Completed` or `Cancelled`).
    ///
    /// A terminal task can never change status again, so every write path must
    /// refuse to touch it rather than reasoning about the individual pair.
    #[must_use]
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Cancelled)
    }

    /// Whether a task may move from this status to `to` (spec `teamssetup`
    /// FR-003, FR-004; design decision D3).
    ///
    /// This is the single legal-transition table for [`TaskStatus`]. Every write
    /// path validates against it before mutating a task's `status`, so a
    /// reachable-but-unleaveable state cannot be introduced by accident and the
    /// four claim paths cannot disagree about which moves are possible.
    ///
    /// | From | May become |
    /// | --- | --- |
    /// | `Pending` | `InProgress`, `Cancelled` |
    /// | `InProgress` | `Completed`, `Pending` (release), `Cancelled` |
    /// | `Completed` | nothing (terminal) |
    /// | `Cancelled` | nothing (terminal) |
    ///
    /// The *ownership* restrictions that narrow these pairs - completion
    /// requires the owner or the lead (FR-001), releasing an `InProgress` task
    /// requires the owner or the lead, and cancelling an `InProgress` task is the
    /// lead's alone (FR-005) - are actor checks that the caller applies on top of
    /// this table. This function answers only "is this state pair legal", so it
    /// takes no actor.
    ///
    /// A transition to the same status is **not** legal: re-asserting a status is
    /// not a state change, and allowing it would let a writer claim it advanced a
    /// task when nothing moved. A caller that wants "already there" to succeed
    /// (idempotent claim or completion) must test for it before validating.
    ///
    /// # Arguments
    ///
    /// * `to` - the status the caller intends to write.
    ///
    /// # Returns
    ///
    /// `true` when the transition is allowed, `false` otherwise.
    #[must_use]
    pub fn can_transition_to(&self, to: Self) -> bool {
        match self {
            Self::Pending => matches!(to, Self::InProgress | Self::Cancelled),
            Self::InProgress => {
                matches!(to, Self::Completed | Self::Pending | Self::Cancelled)
            }
            // Both terminal statuses have no outgoing edge; listing them
            // explicitly keeps the table exhaustive as the enum grows.
            Self::Completed | Self::Cancelled => false,
        }
    }
}

// -- Task ---------------------------------------------------------------------

/// A single unit of work in the shared task list.
///
/// `#[serde(deny_unknown_fields)]` rejects unknown fields on manual edits so
/// typos are surfaced instead of silently ignored (M5-T3).
///
/// M6-T3: `completed_by` makes task completion idempotent - a second
/// completion by a *different* agent is rejected, while the same agent
/// repeating a completion it already owns returns success without mutation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Task {
    /// Unique task identifier (e.g. `"task-001"`).
    pub id: String,
    /// Short human-readable title.
    pub title: String,
    /// Full description of the work to be done.
    #[serde(default)]
    pub description: String,
    /// Current state.
    pub status: TaskStatus,
    /// Agent ID of the teammate this task is assigned to, if any.
    pub assigned_to: Option<String>,
    /// Task IDs that must be `Completed` before this task can be claimed.
    #[serde(default)]
    pub depends_on: Vec<String>,
    /// When the task was added to the list.
    pub created_at: DateTime<Utc>,
    /// When a teammate first claimed the task.
    #[serde(default)]
    pub claimed_at: Option<DateTime<Utc>>,
    /// When the task was marked `Completed`.
    #[serde(default)]
    pub completed_at: Option<DateTime<Utc>>,
    /// M6-T3: Agent ID that completed this task, if any. Used to make
    /// completion idempotent.
    #[serde(default)]
    pub completed_by: Option<String>,
}

impl Task {
    /// Validate the task's invariants (M5-T3).
    ///
    /// Returns `Ok(())` if the task is well-formed, or an error describing the
    /// first violation. Checks:
    /// - `id` is non-empty.
    /// - `assigned_to`, when set, is a plausible agent ID (`"lead"` or `tm-...`).
    /// - `depends_on` does not contain the task's own id.
    pub fn validate(&self) -> Result<()> {
        if self.id.is_empty() {
            return Err(anyhow!("task id is empty"));
        }
        if let Some(owner) = &self.assigned_to
            && !owner.is_empty()
            && owner != "lead"
            && !owner.starts_with("tm-")
        {
            return Err(anyhow!(
                "task {} assigned_to '{owner}' is not a valid agent id (expected 'lead' or 'tm-...')",
                self.id
            ));
        }
        if self.depends_on.iter().any(|d| d == &self.id) {
            return Err(anyhow!("task {} depends on itself", self.id));
        }
        Ok(())
    }
    /// Create a new task in `Pending` state.
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            description: String::new(),
            status: TaskStatus::Pending,
            assigned_to: None,
            depends_on: Vec::new(),
            created_at: Utc::now(),
            claimed_at: None,
            completed_at: None,
            completed_by: None,
        }
    }

    /// Return `true` if the task is pending and all dependencies are satisfied.
    ///
    /// This is the **single** "task is claimable" predicate (spec `teamssetup`
    /// FR-007). Every claim path - [`claim_task_in_list`], and therefore
    /// [`TaskStore::claim_next`], [`TaskStore::claim_specific`],
    /// [`TaskStore::pre_assign_task`] and
    /// [`TeamManager::claim_next_task`] - asks this function rather than
    /// spelling the claim rule again, so a change to what "claimable" means
    /// takes effect on all of them at once.
    ///
    /// A task is claimable only when its status is [`TaskStatus::Pending`]:
    /// `InProgress` is already owned, and both terminal statuses
    /// (`Completed`, `Cancelled`) are refused by the same test, which is what
    /// makes FR-006 hold for cancellation without a separate branch.
    ///
    /// PERF-026: `completed_ids` is a `HashSet<String>` so each
    /// `depends_on` lookup is O(1), giving O(D) per claim check (D =
    /// dependency count) instead of the previous O(T * D) when
    /// `completed_ids` was a `Vec<String>`.
    #[must_use]
    pub fn is_claimable(&self, completed_ids: &std::collections::HashSet<String>) -> bool {
        self.status == TaskStatus::Pending
            && self
                .depends_on
                .iter()
                .all(|dep| completed_ids.contains(dep))
    }
}

// -- Task list -----------------------------------------------------------------

/// Root of `tasks.json`.
///
/// Carries a `schema_version` (M5-T2) and `updated_at` (M5-T5) so future
/// schema changes can be migrated and concurrent races can be debugged.
/// `#[serde(deny_unknown_fields)]` rejects unknown fields on manual edits
/// (M5-T3).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct TaskList {
    /// Schema version of the on-disk format (M5-T2).
    #[serde(default)]
    pub schema_version: u32,
    /// Name of the owning team.
    #[serde(default)]
    pub team_name: String,
    /// All tasks in insertion order.
    pub tasks: Vec<Task>,
    /// When this list was last written (M5-T5).
    #[serde(default)]
    pub updated_at: Option<DateTime<Utc>>,
}

/// Current on-disk schema version for [`TaskList`] (M5-T2).
pub const TASK_LIST_SCHEMA_VERSION: u32 = 1;

impl TaskList {
    /// Create an empty task list for `team_name`.
    pub fn new(team_name: impl Into<String>) -> Self {
        Self {
            schema_version: TASK_LIST_SCHEMA_VERSION,
            team_name: team_name.into(),
            tasks: Vec::new(),
            updated_at: Some(Utc::now()),
        }
    }

    /// Validate the whole list, not just each task (M5-T3; spec `teamssetup`
    /// FR-038, FR-039).
    ///
    /// This is the invariant a write must satisfy, and [`TaskStore::write_locked`]
    /// enforces it after every write, so a list that fails here is never
    /// persisted. Three properties are checked, each against the list as a
    /// whole rather than against one task in isolation:
    ///
    /// 1. Every task passes [`Task::validate`] (non-empty id, plausible
    ///    `assigned_to`, no self-dependency).
    /// 2. No two tasks share an id. Duplicate ids make `get` / `get_mut`
    ///    ambiguous, so a claim could move one row while a completion moved the
    ///    other - the pair would then disagree about the same task forever.
    /// 3. Every `depends_on` entry names a task in this list, and the
    ///    dependency graph is acyclic (FR-039).
    ///
    /// A dependency on a missing task makes the dependent unclaimable *forever*:
    /// [`Task::is_claimable`] asks whether every dependency is `completed`, and
    /// an id that is not in the list can never complete. A cycle is worse - no
    /// member of it can ever be claimed either, and the deadlock is invisible
    /// from the task rows alone. Both are reported at write time so the author
    /// of the bad edge fixes it immediately, instead of the team discovering
    /// hours later that a task never ran.
    ///
    /// # Returns
    ///
    /// `Ok(())` when the list is internally consistent, otherwise an error
    /// naming the offending task, the offending dependency, and the chain or
    /// cycle that makes it invalid.
    pub fn validate(&self) -> Result<()> {
        let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for t in &self.tasks {
            t.validate()?;
            if !seen.insert(t.id.as_str()) {
                return Err(anyhow!(
                    "task '{}' appears more than once in the list; task ids must be unique",
                    t.id
                ));
            }
        }
        self.validate_dependencies()
    }

    /// Check every `depends_on` edge against the list (FR-039).
    ///
    /// Dangling edge first, then cycles: a missing id is the likelier mistake
    /// and the clearer message, and a cycle through a missing id is not a cycle
    /// at all.
    ///
    /// Cycles are found by a depth-first walk with a three-colour marking
    /// (`Unvisited` / `InProgress` / `Done`) over edges pointing from a task to
    /// the tasks it depends on. Re-entering a node still on the DFS stack closes
    /// a cycle, and the stack from that node onward is exactly the cycle, which
    /// is what the error prints. The walk is O(V + E) and visits each edge once,
    /// so a large list costs one pass per write rather than a matrix.
    fn validate_dependencies(&self) -> Result<()> {
        let by_id: std::collections::HashMap<&str, &Task> =
            self.tasks.iter().map(|t| (t.id.as_str(), t)).collect();

        // Pass 1: every edge must name a task in this list (FR-039, dangling).
        for task in &self.tasks {
            for dep in &task.depends_on {
                if !by_id.contains_key(dep.as_str()) {
                    return Err(anyhow!(
                        "task '{}' depends_on '{dep}', which is not a task in this list. \
                         A dependency on a task that does not exist can never be satisfied, \
                         so the task would stay unclaimable forever. Create '{dep}' first or \
                         drop it from depends_on. Known tasks: [{}]",
                        task.id,
                        self.task_ids().join(", ")
                    ));
                }
            }
        }

        // Pass 2: the dependency graph must be acyclic (FR-039).
        #[derive(Clone, Copy, PartialEq, Eq)]
        enum Mark {
            Unvisited,
            InProgress,
            Done,
        }
        let mut marks: std::collections::HashMap<&str, Mark> = self
            .tasks
            .iter()
            .map(|t| (t.id.as_str(), Mark::Unvisited))
            .collect();
        let mut stack: Vec<&str> = Vec::new();

        // Iterative DFS so a long dependency chain cannot exhaust the stack.
        for root in &self.tasks {
            if marks.get(root.id.as_str()).copied() == Some(Mark::Done) {
                continue;
            }
            let mut work: Vec<(&str, usize)> = vec![(root.id.as_str(), 0)];
            while let Some((node, next)) = work.pop() {
                if next == 0 {
                    marks.insert(node, Mark::InProgress);
                    stack.push(node);
                }
                let deps: &[String] = by_id
                    .get(node)
                    .map(|t| t.depends_on.as_slice())
                    .unwrap_or(&[]);
                if let Some(dep) = deps.get(next) {
                    work.push((node, next + 1));
                    match marks.get(dep.as_str()).copied() {
                        Some(Mark::InProgress) => {
                            let start = stack.iter().position(|n| *n == dep.as_str()).unwrap_or(0);
                            let mut cycle: Vec<&str> = stack[start..].to_vec();
                            cycle.push(dep.as_str());
                            return Err(anyhow!(
                                "task '{}' depends_on '{}' closes a dependency cycle: {}. \
                                 No task in the cycle can ever be claimed, because each waits \
                                 on another that also waits. Remove one edge in the chain.",
                                root.id,
                                dep,
                                cycle.join(" -> ")
                            ));
                        }
                        Some(Mark::Done) | None => {}
                        Some(Mark::Unvisited) => work.push((dep.as_str(), 0)),
                    }
                } else {
                    marks.insert(node, Mark::Done);
                    stack.pop();
                }
            }
        }
        Ok(())
    }

    /// Every task id in the list, in insertion order.
    ///
    /// Used to make a dangling-dependency refusal actionable: the error names
    /// the ids the author could have meant.
    #[must_use]
    pub fn task_ids(&self) -> Vec<String> {
        self.tasks.iter().map(|t| t.id.clone()).collect()
    }

    /// Migrate this list to the current schema version (M5-T2).
    ///
    /// Currently a no-op: the only schema change so far is the addition of
    /// `schema_version` and `updated_at`, both `#[serde(default)]`. Future
    /// breaking changes should bump [`TASK_LIST_SCHEMA_VERSION`] and perform
    /// the field transforms here before the list is used.
    pub fn migrate(&mut self) {
        if self.schema_version == 0 {
            self.schema_version = TASK_LIST_SCHEMA_VERSION;
        }
        if self.updated_at.is_none() {
            self.updated_at = Some(Utc::now());
        }
    }

    /// IDs of all tasks that are `Completed`.
    ///
    /// PERF-026: returns a `HashSet<String>` (not a `Vec`) so the caller's
    /// `is_claimable` dependency check is O(1) per dependency instead of a
    /// linear scan. Previously this rebuilt a `Vec<String>` on every
    /// `is_claimable` / `next_claimable` call, yielding O(T^2) complexity for a
    /// full `next_claimable` scan over T tasks.
    pub fn completed_ids(&self) -> std::collections::HashSet<String> {
        self.tasks
            .iter()
            .filter(|t| t.status == TaskStatus::Completed)
            .map(|t| t.id.clone())
            .collect()
    }

    /// Find the first pending task whose dependencies are all satisfied.
    #[must_use]
    pub fn next_claimable(&self) -> Option<&Task> {
        let done = self.completed_ids();
        self.tasks.iter().find(|t| t.is_claimable(&done))
    }

    /// Find the in-progress task currently owned by `agent_id`, if any.
    #[must_use]
    pub fn in_progress_for<'a>(&'a self, agent_id: &str) -> Option<&'a Task> {
        self.tasks.iter().find(|t| {
            t.status == TaskStatus::InProgress && t.assigned_to.as_deref() == Some(agent_id)
        })
    }

    /// Find a task by ID, returning a mutable reference.
    pub fn get_mut(&mut self, task_id: &str) -> Option<&mut Task> {
        self.tasks.iter_mut().find(|t| t.id == task_id)
    }

    /// Find a task by ID.
    #[must_use]
    pub fn get(&self, task_id: &str) -> Option<&Task> {
        self.tasks.iter().find(|t| t.id == task_id)
    }
}

// -- Task transition -----------------------------------------------------------

/// A record of one applied [`TaskStatus`] change (spec `teamssetup` FR-003).
///
/// Returned by [`transition_task`] for every transition it actually applies, so
/// a caller that needs to react to the move (notify an owner, fire a hook,
/// publish an event) reads it from the result instead of re-deriving "what
/// changed" by diffing before and after. A transition refused by the legal
/// table produces no `TaskTransition`: it produces an error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskTransition {
    /// Task ID that changed status.
    pub task_id: String,
    /// Status the task held before the transition.
    pub from: TaskStatus,
    /// Status the task holds after the transition.
    pub to: TaskStatus,
    /// Agent that requested the transition (an agent ID; `lead` for the lead).
    pub actor: String,
}

/// The outcome of [`claim_task_in_list`] (spec `teamssetup` FR-025).
///
/// Each variant maps onto one shape the four claim entry points already
/// returned, so the single implementation serves a "claim the next task"
/// caller and a "claim this task" caller without either needing a special case:
///
/// | Variant | `claim_next` | `claim_specific` |
/// | --- | --- | --- |
/// | `AlreadyOwned(task)` | `(Some(task), true)` | `Ok(task)` |
/// | `Claimed(task)` | `(Some(task), false)` | `Ok(task)` |
/// | `NoneAvailable` | `(None, false)` | never (a named task is resolved or refused) |
#[derive(Debug, Clone)]
pub enum TaskClaim {
    /// The agent already owned this task in `InProgress`; nothing was mutated.
    AlreadyOwned(Task),
    /// This call moved the task to `InProgress` and assigned it to the agent.
    Claimed(Task),
    /// No task in the list is claimable; nothing was mutated.
    NoneAvailable,
}

impl TaskClaim {
    /// The task this claim refers to, when it refers to one.
    #[must_use]
    pub fn task(&self) -> Option<&Task> {
        match self {
            Self::AlreadyOwned(task) | Self::Claimed(task) => Some(task),
            Self::NoneAvailable => None,
        }
    }

    /// Whether this claim found the agent already holding the task.
    #[must_use]
    pub fn is_already_owned(&self) -> bool {
        matches!(self, Self::AlreadyOwned(_))
    }
}

/// The canonical refusal text for an illegal task transition (FR-004, FR-032).
///
/// Names the task, the status it currently holds, the status the caller asked
/// for, and the transition table row that forbids it, so an operator can tell a
/// stale read (`Completed -> Completed` after someone else finished the same
/// task) from a genuine programming error (`Cancelled -> Completed`). The
/// closing sentence names the one action that works, so a refusal is never a
/// dead end: a finished task cannot move at all, so the next action is to read
/// the current list and pick live work.
fn transition_refusal(task_id: &str, from: TaskStatus, to: TaskStatus) -> String {
    let (row, next) = match from {
        TaskStatus::Pending => (
            "pending -> in_progress, pending -> cancelled (spec teamssetup D3 table)",
            "claim it first with `team_task_claim`, which moves it to in_progress",
        ),
        TaskStatus::InProgress => (
            "in_progress -> completed, in_progress -> pending, in_progress -> cancelled \
             (spec teamssetup D3 table)",
            "complete it with `team_task_complete`, or ask the lead to cancel it with \
             `team_task_cancel`",
        ),
        TaskStatus::Completed => (
            "completed -> nothing: completed is terminal (spec teamssetup D3 table)",
            "a finished task is terminal - create a follow-up task with `team_task_create` \
             instead of moving this one",
        ),
        TaskStatus::Cancelled => (
            "cancelled -> nothing: cancelled is terminal (spec teamssetup D3 table)",
            "a cancelled task is terminal - create a replacement task with \
             `team_task_create` instead of moving this one",
        ),
    };
    format!(
        "task '{task_id}' cannot move from {} to {}; allowed from {}: {row}. Next: {next}",
        from.as_str(),
        to.as_str(),
        from.as_str()
    )
}

/// Apply a validated [`TaskStatus`] transition to `list` (spec `teamssetup`
/// FR-003, FR-004; design decision D3).
///
/// This is the **single** writer of [`Task::status`]. Every write path - the
/// four claim paths, both completion paths, cancellation, release, pre-assign
/// and orphan adoption - calls it, and it is the only caller of
/// [`TaskStatus::can_transition_to`]. Validating here rather than at each call
/// site is what makes FR-003 hold: a path cannot forget the check because the
/// only way to write a status is through the check.
///
/// The helper mutates `list` in memory only. It never touches the disk, so a
/// refused transition leaves `tasks.json` byte-identical (FR-004) by
/// construction: the caller has not reached [`TaskStore::write_locked`] yet.
///
/// Order of checks:
///
/// 1. `task_id` must exist in `list` - an unknown id is an error naming the
///    available task IDs, not a silent no-op.
/// 2. `to` must equal the current status, or the pair must be in the D3 table.
///    A self-transition is legal here (unlike [`TaskStatus::can_transition_to`],
///    which refuses it) and applies no change, returning `Ok(None)`: re-asserting
///    a status is not a state change, so the idempotent paths need no special
///    case at the call site.
///
/// # Arguments
///
/// * `list` - the task list to mutate (already loaded under the store lock).
/// * `task_id` - ID of the task to transition.
/// * `to` - the status to write.
/// * `actor` - the agent requesting the transition. Recorded on the returned
///   [`TaskTransition`] for callers that fire events or hooks; the legal table
///   itself is actor-independent, and the ownership restrictions D3 attaches to
///   individual pairs (completion is owner-or-lead, release is owner-or-lead,
///   cancelling an `InProgress` task is lead-only) are checked by the caller
///   before calling this helper.
///
/// # Returns
///
/// `Ok(Some(transition))` when the status changed, `Ok(None)` when `to` already
/// equals the current status (no mutation, nothing to persist), or an error when
/// the task does not exist or the transition is forbidden by the table.
pub fn transition_task(
    list: &mut TaskList,
    task_id: &str,
    to: TaskStatus,
    actor: &str,
) -> Result<Option<TaskTransition>> {
    let available_ids: Vec<String> = list
        .tasks
        .iter()
        .map(|t| format!("{} ({})", t.id, t.title))
        .collect();
    let Some(task) = list.get_mut(task_id) else {
        return Err(anyhow!(
            "task '{task_id}' not found. Available tasks: [{}]",
            available_ids.join(", ")
        ));
    };

    let from = task.status;
    if from == to {
        // Idempotent re-assert: not a state change, so not a transition.
        return Ok(None);
    }
    if !from.can_transition_to(to) {
        return Err(anyhow!(transition_refusal(task_id, from, to)));
    }
    task.status = to;
    Ok(Some(TaskTransition {
        task_id: task_id.to_owned(),
        from,
        to,
        actor: actor.to_owned(),
    }))
}

// -- Task hooks ----------------------------------------------------------------

/// Fire the lifecycle hook a task mutation implies (spec `teamssetup` FR-017).
///
/// FR-017 makes the *runtime* the trigger point: a task created, claimed or
/// completed fires the same hook whether the caller was the `team_task_*` tool,
/// a lead-side claim, an auto-claim or an adoption. So the hook is fired from
/// here - reached by every mutation path - and the tools no longer call
/// [`run_team_hook`](crate::team::run_team_hook) themselves.
///
/// The event is derived from the mutation rather than passed in by every
/// caller, so a caller cannot forget or mis-name it:
///
/// | Hook event | Fired when | stdin payload |
/// | --- | --- | --- |
/// | [`HookEvent::TaskCreated`] | a task is added to the list | `team_name`, `task_id`, `title`, `description`, `depends_on` |
/// | [`HookEvent::TaskClaimed`] | [`TaskTransition::to`] is `InProgress` | `team_name`, `task_id`, `title`, `description`, `depends_on`, `agent_id` |
/// | [`HookEvent::TaskCompleted`] | [`TaskTransition::to`] is `Completed` | `team_name`, `task_id`, `title`, `description`, `completed_by`, `completed_at` |
///
/// `Cancelled` and `Pending` fire nothing: a cancellation has no hook event in
/// this schema (`MessageType::TaskCancelled` is the lead's notice to the owner)
/// and a release back to `Pending` is not a lifecycle milestone.
///
/// `actor` is the agent that made the move, which is the claiming agent for a
/// claim and the completing agent for a completion.
///
/// # Returns
///
/// The hook's [`HookOutcome`]. `HookOutcome::Allow` covers both "no hook is
/// configured for this event" and "the hook allowed it", which is what every
/// caller but the completion path wants. The completion path returns
/// `HookOutcome::Feedback` to its caller instead of acting on it, and
/// `team_task_complete` delivers that verdict to the completing agent in its
/// tool result (FR-018).
///
/// # Errors
///
/// Only when the team directory cannot be read to find the team's configured
/// hooks - the same lookup failure [`run_team_hook`] treats as "no hook",
/// surfaced here so a caller that *acts* on the outcome knows the difference
/// between "allowed" and "could not ask". Note that this is a hook-lookup
/// failure, not a task-mutation failure: the mutation has already been
/// persisted by the time this runs.
///
/// [`run_team_hook`]: crate::team::run_team_hook
pub async fn fire_task_hook(
    team_dir: &Path,
    transition: Option<&TaskTransition>,
    task: &Task,
    actor: &str,
) -> Result<HookOutcome> {
    let store = TeamStore::load(team_dir)
        .with_context(|| format!("load team store in {}", team_dir.display()))?;
    let team_name = store.config.name.clone();

    let (event, stdin_json) = match transition {
        Some(TaskTransition {
            to: TaskStatus::InProgress,
            ..
        }) => (
            HookEvent::TaskClaimed,
            serde_json::json!({
                "team_name": team_name,
                "task_id": task.id,
                "title": task.title,
                "description": task.description,
                "depends_on": task.depends_on,
                "agent_id": actor,
            })
            .to_string(),
        ),
        Some(TaskTransition {
            to: TaskStatus::Completed,
            ..
        }) => (
            HookEvent::TaskCompleted,
            serde_json::json!({
                "team_name": team_name,
                "task_id": task.id,
                "title": task.title,
                "description": task.description,
                "completed_by": actor,
                "completed_at": task.completed_at.map(|t| t.to_rfc3339()),
            })
            .to_string(),
        ),
        // A task that has just been added to the list: no transition, because
        // the creation is a push, not a status move.
        None => (
            HookEvent::TaskCreated,
            serde_json::json!({
                "team_name": team_name,
                "task_id": task.id,
                "title": task.title,
                "description": task.description,
                "depends_on": task.depends_on,
            })
            .to_string(),
        ),
        // `Cancelled` and a release to `Pending` have no hook event.
        Some(_) => return Ok(HookOutcome::Allow),
    };

    Ok(crate::team::run_team_hook(team_dir, event, Some(&stdin_json)).await)
}

/// Fire the [`HookEvent::TaskCreated`] hook for a freshly added task.
///
/// The creation counterpart of [`fire_task_hook`], which is keyed off a
/// [`TaskTransition`] that a creation does not have. Both entry points that add
/// a task ([`TaskStore::add_task`] and the seed path in `team_create`) call
/// this, so a seeded task fires the same hook a `team_task_create` call does
/// (FR-017).
///
/// # Errors
///
/// A hook-lookup failure - see [`fire_task_hook`]. The task has already been
/// persisted when this runs.
pub async fn fire_task_created_hook(team_dir: &Path, task: &Task) -> Result<HookOutcome> {
    fire_task_hook(team_dir, None, task, "lead").await
}

// -- Task completion -----------------------------------------------------------

/// Why a completion was refused (spec `teamssetup` FR-001, FR-044).
///
/// The reason travels with the refusal so the `TeamTaskCompletionRejected`
/// event can report *which* rule fired. It is a canonical snake_case token
/// because the event carries strings, not this enum (the event type lives in
/// `ragent-types`, which `ragent-agent` depends on).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionRefusalReason {
    /// The caller is not the task's `assigned_to` and is not the lead. This is
    /// the FR-001 rule, and the case the previous release silently accepted by
    /// re-assigning the task to whoever completed it.
    NotOwner,
    /// The task is `Pending`: completing is not claiming, so the lead and the
    /// owner alike must claim or assign it first.
    NotClaimed,
    /// The task is already `Completed` and the caller is not the agent that
    /// completed it.
    AlreadyCompleted,
}

impl CompletionRefusalReason {
    /// Return the canonical snake_case token carried on the
    /// `TeamTaskCompletionRejected` event and in the tool's JSON metadata.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NotOwner => "not_owner",
            Self::NotClaimed => "not_claimed",
            Self::AlreadyCompleted => "already_completed",
        }
    }
}

/// A refused completion, carrying everything the FR-044 event reports.
///
/// [`complete_task_in_list`] returns it as an `anyhow::Error`, so both
/// completion entry points keep their existing error shape and message, and the
/// entry points that publish the FR-044 event recover it with
/// [`downcast_ref`](anyhow::Error::downcast_ref).
#[derive(Debug)]
pub struct CompletionRefusal {
    /// ID of the task the caller asked to complete.
    pub task_id: String,
    /// Agent ID that asked to complete the task.
    pub agent_id: String,
    /// The task's `assigned_to`, or `"unassigned"` when it has no owner.
    pub owner: String,
    /// Which rule refused the completion.
    pub reason: CompletionRefusalReason,
    /// The refusal text the caller receives, verbatim.
    pub message: String,
}

impl std::fmt::Display for CompletionRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for CompletionRefusal {}

/// Record an FR-044 refusal in the log even when no event consumer is attached.
///
/// The `TeamTaskCompletionRejected` event is the primary signal, but it is
/// published on the in-process [`EventBus`], which only exists while a session
/// is running. The TUI's diagnostic file (`log/…`) is fed by the tracing layer,
/// so a refusal is *also* written at `warn` under the `ragent_agent::task`
/// target: "`rg 'TaskCompletionRejected' log/`" then finds the refusal whether
/// or not a subscriber was listening (the check `TESTPLAN.md` TC-001 names).
///
/// # Arguments
///
/// * `refusal` - the refusal to record. Its `message` is already the text the
///   calling agent received, so the log line and the tool result agree.
/// * `team_name` - name of the team, needed by the event variant.
/// * `session_id` - the lead session id, needed by the event variant.
/// * `event_bus` - the bus to publish on. `None` from a [`TaskStore`], which
///   holds no bus; the tracing line is written either way.
///
/// [`EventBus`]: crate::event::EventBus
pub(crate) fn publish_completion_rejected(
    refusal: &CompletionRefusal,
    team_name: &str,
    session_id: Option<&str>,
    event_bus: Option<&crate::event::EventBus>,
) {
    tracing::warn!(
        target: "ragent_agent::task",
        event = "TaskCompletionRejected",
        team_name = %team_name,
        agent_id = %refusal.agent_id,
        task_id = %refusal.task_id,
        owner = %refusal.owner,
        reason = refusal.reason.as_str(),
        message = %refusal.message,
        "team task completion refused (FR-001 ownership rule)"
    );

    if let Some(bus) = event_bus {
        bus.publish(crate::event::Event::TeamTaskCompletionRejected {
            session_id: session_id.unwrap_or_default().to_owned(),
            team_name: team_name.to_owned(),
            agent_id: refusal.agent_id.clone(),
            task_id: refusal.task_id.clone(),
            owner: refusal.owner.clone(),
            reason: refusal.reason.as_str().to_owned(),
            message: refusal.message.clone(),
        });
    }
}

/// The canonical "you do not own this task" refusal (FR-001, FR-002, FR-032).
///
/// Names the entity (the task and its owner), the state that makes the call
/// illegal (the task is owned by another agent, or by nobody), and the two ways
/// out (`team_assign_task` for the lead and `team_task_claim` for the
/// teammate), so a caller that guessed at a task ID is told what to do instead
/// of only what went wrong. Both completion entry points return this exact
/// text: a divergence between them is a defect (FR-026).
fn completion_ownership_refusal(task_id: &str, owner: &str, agent_id: &str) -> String {
    if owner == "unassigned" {
        return format!(
            "task '{task_id}' cannot be completed: it is unassigned, and completing is \
             not claiming. Claim it with `team_task_claim` (task_id: '{task_id}'), or \
             ask the lead to assign it with `team_assign_task`"
        );
    }
    format!(
        "task '{task_id}' cannot be completed by {agent_id}: it is assigned to {owner}. \
         Ask the lead to reassign it with `team_assign_task`, or claim it yourself once \
         it is released with `team_task_claim`"
    )
}

/// Apply a completion to `list` under the FR-001 ownership rule.
///
/// This is the **single** completion implementation (spec `teamssetup` FR-025).
/// [`TaskStore::complete`] and [`TeamManager::complete_task`] both delegate
/// here, so the cached and uncached paths cannot diverge in behaviour or in
/// error text (FR-026).
///
/// # Ownership (FR-001, FR-002)
///
/// A completion is accepted only from the task's `assigned_to` agent or from
/// `lead`. A refusal names the current owner and mutates nothing: `assigned_to`,
/// `claimed_at`, `status`, `completed_at` and `completed_by` are all left as
/// they were, because every check runs before the first write.
///
/// The previous release silently re-assigned an unowned or `Pending` task to
/// whoever completed it; that branch is gone rather than guarded, so "complete
/// it and it becomes mine" is no longer a way to claim work.
///
/// # Idempotency
///
/// Completing a task that is already `Completed` is a no-op success for an
/// actor the ownership rule would have accepted (its owner or `lead`) and the
/// `already completed by` refusal otherwise.
///
/// # Refusals (FR-044)
///
/// Every refusal caused by an ownership or status rule is a
/// [`CompletionRefusal`], not a bare string, so the entry points can publish
/// `TeamTaskCompletionRejected` with the same facts and the same words the
/// caller saw. A task that does not exist is still a plain error: there is no
/// owner to name and the previous release refused it too.
pub fn complete_task_in_list(
    list: &mut TaskList,
    task_id: &str,
    agent_id: &str,
    is_lead: bool,
) -> Result<Task> {
    let available_ids: Vec<String> = list
        .tasks
        .iter()
        .map(|t| format!("{} ({})", t.id, t.title))
        .collect();
    let Some(task) = list.get_mut(task_id) else {
        return Err(anyhow!(
            "task '{task_id}' not found. Available tasks: [{}]",
            available_ids.join(", ")
        ));
    };

    if !is_lead && task.assigned_to.as_deref() != Some(agent_id) {
        let owner = task
            .assigned_to
            .clone()
            .unwrap_or_else(|| "unassigned".to_owned());
        return Err(CompletionRefusal {
            task_id: task_id.to_owned(),
            agent_id: agent_id.to_owned(),
            owner: owner.clone(),
            reason: CompletionRefusalReason::NotOwner,
            message: completion_ownership_refusal(task_id, &owner, agent_id),
        }
        .into());
    }

    // Idempotent completion: an accepted actor re-completing a finished task is
    // a no-op success, anyone else is refused by name.
    if task.status == TaskStatus::Completed {
        let completer = task
            .completed_by
            .clone()
            .unwrap_or_else(|| "unknown".to_owned());
        if completer != agent_id && !is_lead {
            return Err(CompletionRefusal {
                task_id: task_id.to_owned(),
                agent_id: agent_id.to_owned(),
                owner: task
                    .assigned_to
                    .clone()
                    .unwrap_or_else(|| "unassigned".to_owned()),
                reason: CompletionRefusalReason::AlreadyCompleted,
                message: format!(
                    "task '{task_id}' cannot be completed by '{agent_id}': it is in status \
                     completed (completed by '{completer}'). A finished task is terminal - \
                     ask the lead to create a follow-up task with `team_task_create`"
                ),
            }
            .into());
        }
        return Ok(task.clone());
    }

    // An unowned task cannot be completed: completing is not claiming. The
    // lead may close it, but the move is still Pending -> Completed, which the
    // D3 table forbids, so the caller must claim or assign it first.
    if task.status == TaskStatus::Pending {
        return Err(CompletionRefusal {
            task_id: task_id.to_owned(),
            agent_id: agent_id.to_owned(),
            owner: task
                .assigned_to
                .clone()
                .unwrap_or_else(|| "unassigned".to_owned()),
            reason: CompletionRefusalReason::NotClaimed,
            message: format!(
                "task '{task_id}' cannot be completed: it is in status pending and has no \
                 owner. Claim it with `team_task_claim` (task_id: '{task_id}'), or assign it \
                 with `team_assign_task`, then complete it"
            ),
        }
        .into());
    }

    let transition = transition_task(list, task_id, TaskStatus::Completed, agent_id)?;
    if transition.is_none() {
        // Only reachable via the idempotent re-assert above, which already
        // returned; kept explicit so a future table change cannot silently make
        // this function a no-op.
        return Err(anyhow!(
            "task '{task_id}' is already {}; nothing to complete",
            TaskStatus::Completed.as_str()
        ));
    }

    let Some(task) = list.get_mut(task_id) else {
        return Err(anyhow!("task '{task_id}' not found"));
    };
    task.completed_at = Some(Utc::now());
    task.completed_by = Some(agent_id.to_owned());
    Ok(task.clone())
}

// -- Task claim ----------------------------------------------------------------

/// The canonical "another task is already in progress" refusal (FR-025,
/// FR-032).
///
/// One agent works one task at a time, so a second claim is refused until the
/// first is completed. The text names the entity (the agent), its state (the
/// task it holds), and the next action (complete that task first), so a refused
/// claim is actionable without reading the task list. Both claim entry points
/// return this exact string: a divergence between them is a defect (FR-026).
fn claim_in_flight_refusal(agent_id: &str, in_flight: &str, task_id: &str) -> String {
    format!(
        "agent {agent_id} cannot claim '{task_id}': it is working on {} (one task at a \
         time). Complete it with `team_task_complete` (task_id: '{in_flight}'), then \
         claim '{task_id}'",
        crate::team::current_task_clause(Some(in_flight))
    )
}

/// The canonical "status does not permit a claim" refusal (FR-006, FR-032,
/// FR-026).
///
/// Names the entity (the task), the status it actually holds, and the one next
/// action that resolves it, so a refused claim reports `cancelled` for a
/// cancelled task rather than calling it pending (FR-006). A cancelled task says
/// outright that it is cancelled and what to do instead, because `is_claimable`
/// is false for it and no `(from, to)` row in the D3 transition table leaves
/// `Cancelled` - retrying is never the answer, so the second sentence names the
/// reason and a real alternative rather than only the accepting status.
fn claim_status_refusal(task_id: &str, status: TaskStatus) -> String {
    let what_next = match status {
        TaskStatus::Cancelled => {
            "a cancelled task is terminal and cannot be claimed again - ask the lead to \
             create a replacement task with `team_task_create`"
        }
        TaskStatus::Completed => {
            "a finished task is terminal and cannot be claimed again - claim another task \
             with `team_task_claim` (no task_id) to take the next available one"
        }
        TaskStatus::Pending | TaskStatus::InProgress => {
            "only pending tasks can be claimed - another agent already holds it, so claim \
             a different task with `team_task_claim` (no task_id)"
        }
    };
    format!(
        "{}; {what_next}",
        crate::team::task_status_clause(task_id, status)
    )
}

/// Claim a task for `agent_id` under the shared claim rule (spec `teamssetup`
/// FR-007, FR-025, FR-026).
///
/// This is the **single** claim implementation. [`TaskStore::claim_next`],
/// [`TaskStore::claim_specific`], [`TaskStore::pre_assign_task`] and
/// [`TeamManager::claim_next_task`] all delegate here, so the cached and
/// uncached paths cannot disagree about which task may be taken, what the move
/// does to the task, or what a refusal says.
///
/// The helper mutates `list` in memory only. It never touches the disk, so a
/// refused claim leaves `tasks.json` byte-identical by construction: the caller
/// has not reached [`TaskStore::write_locked`] yet.
///
/// # Claim rule
///
/// * **One task at a time.** An agent that already owns an `InProgress` task
///   cannot take another. Asking for that same task again is an idempotent
///   success; asking for a different one is refused.
/// * **Claimability** is [`Task::is_claimable`] and nothing else (FR-007), so a
///   `Completed` or `Cancelled` task is refused by the same test that refuses an
///   unsatisfied dependency.
/// * **Assignment.** A task pre-assigned to another agent is refused; a task
///   pre-assigned to this agent may be claimed by it.
/// * **Status.** The write goes through [`transition_task`], so the claim is
///   validated against the legal-transition table like every other status write.
///
/// # Arguments
///
/// * `list` - the task list to mutate (already loaded under the store lock).
/// * `target` - which task to claim: `Some(id)` for a specific task, `None` for
///   the first claimable task in insertion order.
/// * `agent_id` - the claiming agent.
///
/// # Returns
///
/// A [`TaskClaim`] describing the outcome: `AlreadyOwned` when the agent
/// already had a task in flight (nothing mutated), `Claimed` when this call took
/// one, or `NoneAvailable` when a `None` target found nothing claimable.
///
/// # Errors
///
/// The refusal for an unknown task, a task held by another agent, a task in a
/// non-pending status (`cancelled` and `completed` both name their own status),
/// an unsatisfied dependency, or a different task already in progress for this
/// agent.
pub fn claim_task_in_list(
    list: &mut TaskList,
    target: Option<&str>,
    agent_id: &str,
) -> Result<TaskClaim> {
    // One task at a time. An agent already holding a task is answered from that
    // task rather than being allowed to take a second one, and `AlreadyOwned`
    // (not an error) is the answer, so `claim_next` keeps its long-standing
    // "(Some(task), already_had=true)" contract and a caller can tell "you are
    // already working" from a genuine refusal. The *named* claim is the
    // exception: asking for a different task while another is in flight is a
    // mistake the caller can act on, so it is refused.
    let in_flight = list.in_progress_for(agent_id).map(|t| t.id.clone());
    if let Some(in_flight) = in_flight {
        if let Some(task_id) = target
            && task_id != in_flight
        {
            return Err(anyhow!(claim_in_flight_refusal(
                agent_id, &in_flight, task_id
            )));
        }
        let Some(task) = list.tasks.iter().find(|t| t.id == in_flight) else {
            return Err(anyhow!("task '{in_flight}' not found"));
        };
        return Ok(TaskClaim::AlreadyOwned(task.clone()));
    }

    // Specific claim: resolve the task and apply the assignment / status /
    // dependency guards. A "next" claim has no task to name yet, so it defers
    // those guards to the claimability scan below.
    let index = match target {
        Some(task_id) => {
            let available_ids: Vec<String> = list
                .tasks
                .iter()
                .map(|t| format!("{} ({})", t.id, t.title))
                .collect();
            let Some(idx) = list.tasks.iter().position(|t| t.id == task_id) else {
                return Err(anyhow!(
                    "task '{task_id}' not found. Available tasks: [{}]",
                    available_ids.join(", ")
                ));
            };
            // Assignment is only a barrier while the task is unclaimed: an
            // `InProgress` task cannot be taken by another agent because the
            // status guard refuses it, and reporting "already assigned" for a
            // task a previous run left in progress would hide the real reason.
            let task = &list.tasks[idx];
            if task.status == TaskStatus::Pending
                && let Some(owner) = task.assigned_to.as_deref()
                && owner != agent_id
            {
                return Err(anyhow!(
                    "task '{task_id}' cannot be claimed by {agent_id}: it is in status \
                     pending and pre-assigned to {owner}. Ask the lead to reassign it with \
                     `team_assign_task`, or claim a different task with `team_task_claim` \
                     (no task_id)"
                ));
            }
            Some(idx)
        }
        None => {
            let done = list.completed_ids();
            list.tasks.iter().position(|t| t.is_claimable(&done))
        }
    };

    let Some(index) = index else {
        return Ok(TaskClaim::NoneAvailable);
    };

    let done = list.completed_ids();
    let task = &list.tasks[index];
    if !task.is_claimable(&done) {
        // `is_claimable` alone decides (FR-007); this branch only explains the
        // refusal. A non-`Pending` status names itself, so a cancelled task
        // reports "cancelled" (FR-006) rather than being described as pending,
        // and a `Pending` task is unclaimable only because of its dependencies.
        if task.status != TaskStatus::Pending {
            return Err(anyhow!(claim_status_refusal(&task.id, task.status)));
        }
        return Err(anyhow!(
            "task '{}' cannot be claimed: it is in status pending but its dependencies are \
             unmet - depends on [{}]. Wait for those tasks to be completed, then claim it \
             again with `team_task_claim`",
            task.id,
            task.depends_on.join(", ")
        ));
    }

    let task_id = list.tasks[index].id.clone();
    // The only writer of `Task::status` validates the move (FR-003, FR-004).
    if transition_task(list, &task_id, TaskStatus::InProgress, agent_id)?.is_none() {
        return Err(anyhow!(
            "task '{task_id}' is already in status in_progress; nothing to claim"
        ));
    }
    let Some(task) = list.get_mut(&task_id) else {
        return Err(anyhow!("task '{task_id}' not found"));
    };
    task.assigned_to = Some(agent_id.to_owned());
    task.claimed_at = Some(Utc::now());
    Ok(TaskClaim::Claimed(task.clone()))
}

// -- Task cancellation ----------------------------------------------------------

/// The canonical "this task cannot be cancelled" refusal (FR-005, FR-006).
///
/// Names the task, the status it actually holds, and the two statuses a
/// cancellation accepts, so a lead that asked to cancel an already-finished task
/// is told what it is rather than being told the task is pending.
fn cancel_status_refusal(task_id: &str, status: TaskStatus) -> String {
    format!(
        "{}; only pending and in_progress tasks can be cancelled. A terminal task is \
         already finished - nothing was changed",
        crate::team::task_status_clause(task_id, status)
    )
}

/// Apply a cancellation to `list` under the FR-005 rule.
///
/// This is the **single** cancellation implementation. [`TaskStore::cancel`] and
/// [`TeamManager::cancel_task`] both delegate here, so the cached and uncached
/// paths cannot diverge in behaviour or in error text (FR-025, FR-026), exactly
/// as [`claim_task_in_list`] and [`complete_task_in_list`] are shared.
///
/// The helper mutates `list` in memory only. It never touches the disk, so a
/// refused cancellation leaves `tasks.json` byte-identical by construction
/// (FR-004): the caller has not reached [`TaskStore::write_locked`] yet.
///
/// # Cancellation rule
///
/// * **Which statuses may be cancelled.** Only `Pending` and `InProgress`
///   (FR-005). Both terminal statuses are refused by [`cancel_status_refusal`],
///   naming the status the task holds, so cancelling a completed task reports
///   `completed` and re-cancelling a cancelled one reports `cancelled` rather
///   than pretending it was pending (FR-006).
/// * **Status.** The write goes through [`transition_task`], so the move is
///   validated against the legal-transition table like every other status write
///   (FR-003). The table already carries `pending -> cancelled` and
///   `in_progress -> cancelled`; the status check above exists to produce the
///   cancellation-specific message, not to bypass the table.
/// * **Assignment.** Cancellation leaves `assigned_to` and `claimed_at` as they
///   are: the record of who was doing the abandoned work is worth keeping, and
///   clearing the owner would let the task be mistaken for unworked.
/// * **Actor.** The lead alone may cancel (FR-005). That is an actor check the
///   caller applies before calling this helper, which is actor-independent for
///   the same reason [`transition_task`] is.
///
/// # Arguments
///
/// * `list` - the task list to mutate (already loaded under the store lock).
/// * `task_id` - ID of the task to cancel.
/// * `actor` - the agent requesting the cancellation (`"lead"` in practice),
///   recorded on the returned [`TaskTransition`] for callers that notify the
///   owner or publish an event.
///
/// # Returns
///
/// A [`TaskCancellation`] naming the cancelled task, its previous status and the
/// transition that was applied. The caller uses `was_in_progress` to decide
/// whether an owning teammate needs a mailbox notification: a `Pending` task has
/// no one working on it, so nobody needs telling.
///
/// # Errors
///
/// The refusal for an unknown task ID or a task in a terminal status.
pub fn cancel_task_in_list(
    list: &mut TaskList,
    task_id: &str,
    actor: &str,
) -> Result<TaskCancellation> {
    let available_ids: Vec<String> = list
        .tasks
        .iter()
        .map(|t| format!("{} ({})", t.id, t.title))
        .collect();
    let Some(task) = list.tasks.iter().find(|t| t.id == task_id) else {
        return Err(anyhow!(
            "task '{task_id}' not found. Available tasks: [{}]",
            available_ids.join(", ")
        ));
    };

    let from = task.status;
    if from.is_terminal() {
        return Err(anyhow!(cancel_status_refusal(task_id, from)));
    }

    // The only writer of `Task::status` validates the move (FR-003, FR-004).
    let Some(transition) = transition_task(list, task_id, TaskStatus::Cancelled, actor)? else {
        // Unreachable: the terminal check above already refused the only status
        // that could make this a self-transition. Kept explicit so a future
        // table change cannot silently make this function a no-op.
        return Err(anyhow!(cancel_status_refusal(task_id, from)));
    };

    let Some(task) = list.get_mut(task_id) else {
        return Err(anyhow!("task '{task_id}' not found"));
    };
    Ok(TaskCancellation {
        task: task.clone(),
        // The owner stays on the record so the lead can see who was working on
        // the abandoned task; only the status moves.
        owner: task.assigned_to.clone(),
        from,
        transition,
        // The in-list helper never touches the mailbox; the entry point that sends
        // the FR-005 notification records who it told.
        notified: None,
    })
}

/// The outcome of [`cancel_task_in_list`] (spec `teamssetup` FR-005).
///
/// Carries everything the two cancellation entry points need *after* the write:
/// the lead's confirmation text, and the target of the FR-005 mailbox
/// notification. `owner` is `Some` only when the cancelled task was assigned, and
/// [`Self::was_in_progress`] says whether that owner was actually working on it.
#[derive(Debug, Clone)]
pub struct TaskCancellation {
    /// The task as it now stands: `status` is `Cancelled`.
    pub task: Task,
    /// The agent the task was assigned to, if any. Retained on the task after
    /// cancellation, so this is simply `task.assigned_to`.
    pub owner: Option<String>,
    /// The status the task held before it was cancelled.
    pub from: TaskStatus,
    /// The transition applied by [`transition_task`].
    pub transition: TaskTransition,
    /// The teammate told about the cancellation, when one was told.
    ///
    /// Filled in by the entry point that sends the FR-005 mailbox notification
    /// ([`TeamManager::cancel_task`]); the in-list helper leaves it `None` because
    /// it never touches the mailbox. `None` after a manager call therefore means
    /// "nobody needed telling" (the task was `Pending` or unassigned).
    ///
    /// [`TeamManager::cancel_task`]: crate::team::manager::TeamManager::cancel_task
    pub notified: Option<String>,
}

impl TaskCancellation {
    /// Whether the owning teammate was actually working on the task.
    ///
    /// Only an `InProgress` task had someone working on it, so only that case
    /// notifies the owner (FR-005). A `Pending` task is unowned work the lead
    /// withdrew, and a pre-assigned-but-unclaimed task was never started.
    #[must_use]
    pub fn was_in_progress(&self) -> bool {
        self.from == TaskStatus::InProgress
    }

    /// The agent to notify, when there is one and it was working.
    #[must_use]
    pub fn notify_target(&self) -> Option<&str> {
        if self.was_in_progress() {
            self.owner.as_deref()
        } else {
            None
        }
    }
}

// -- Task store ----------------------------------------------------------------

/// File-backed store for a team's task list.
///
/// All mutating operations acquire an exclusive `flock` on `tasks.json` for
/// the duration of the read-modify-write cycle, making claim races safe even
/// across multiple ragent processes on the same machine.
pub struct TaskStore {
    path: PathBuf,
    /// PERF-016: team directory retained so the `*_blocking` async wrappers
    /// can reconstruct the store inside a `spawn_blocking` closure.
    pub team_dir: PathBuf,
}

// -- Parse diagnostics ---------------------------------------------------------

/// Parse `raw` as a [`TaskList`], diagnosing a malformed file (spec `teamssetup`
/// FR-037).
///
/// This is the one parse entry point for `tasks.json`, so every read path
/// reports the same facts in the same order. FR-037 forbids the old silent
/// `TaskList::default()` fallback, which discarded every task on a malformed
/// file and let the next write persist the empty list over it; this returns an
/// error instead and names what an operator needs to fix the file by hand:
///
/// - the **file** the bytes came from (`path`),
/// - the **byte offset** of the failure, with the 1-based line and column that
///   offset falls on, and the offending line quoted so a long JSON file can be
///   located without counting bytes,
/// - the **offending field** - the JSON key the parser rejected, taken from
///   `serde_json`'s own message (which names the field for a type mismatch or an
///   unknown key, both of which `#[serde(deny_unknown_fields)]` makes common on a
///   hand-edited file).
///
/// An empty file (or one holding only whitespace) is not malformed: it is a team
/// that has never had a task, so it parses as [`TaskList::default`] exactly as
/// before. Every other failure is an error.
///
/// `context` names the operation for a caller that wants its own frame
/// ("parse tasks.json"); the diagnostic itself is appended to it.
fn parse_task_list(path: &Path, raw: &str, context: &str) -> Result<TaskList> {
    if raw.trim().is_empty() {
        return Ok(TaskList::default());
    }
    let mut list: TaskList = serde_json::from_str(raw)
        .map_err(|e| anyhow!("{context}: {}", task_list_parse_diagnostic(path, raw, &e)))?;
    list.migrate();
    Ok(list)
}

/// Render the FR-037 diagnostic for a `serde_json` parse failure.
///
/// Names the file, the byte offset (plus the line and column it falls on), the
/// offending line when it can be extracted, and the offending field. Split out
/// from [`parse_task_list`] so the text is one string built in one place and a
/// test can assert on it without constructing a store.
fn task_list_parse_diagnostic(path: &Path, raw: &str, err: &serde_json::Error) -> String {
    let offset = err.column();
    // `serde_json::Error` carries a 1-based line and column but not a byte
    // offset; convert, because the offset is what a hex dump or `dd` needs.
    let byte_offset = byte_offset_of(raw, err.line(), err.column());
    let field = offending_field(err, raw);
    let mut message = format!(
        "note: tasks.json is malformed, so no task was read. Fix the file, then retry: \
         the runtime refuses to fall back to an empty list because the next write would \
         persist it over your tasks.\n  file: {}\n  byte offset: {byte_offset} \
         (line {}, column {offset})\n  offending field: {field}",
        path.display(),
        err.line()
    );
    if let Some(line) = offending_line(raw, err.line()) {
        message.push_str("\n  offending line: ");
        message.push_str(line.trim_end());
    }
    message
}

/// The byte offset of a 1-based `line`/`column` pair inside `raw`.
///
/// `serde_json` reports positions as line/column, not bytes; this walks the
/// string once to convert. An out-of-range position yields the end of input,
/// which is where a truncated file's failure actually is.
fn byte_offset_of(raw: &str, line: usize, column: usize) -> usize {
    if line == 0 {
        return 0;
    }
    let mut current_line: usize = 1;
    let mut offset: usize = 0;
    for (idx, ch) in raw.char_indices() {
        if current_line == line {
            offset = idx;
            break;
        }
        if ch == '\n' {
            current_line += 1;
        }
        offset = idx + ch.len_utf8();
    }
    // `column` is 1-based over characters; a byte offset only needs the line
    // base, so this stays an approximation on a multi-byte line - which is
    // enough to locate the failure, and the line/column pair is exact.
    offset + column.saturating_sub(1).min(raw.len())
}

/// The offending JSON field named by a `serde_json` parse error.
///
/// `serde_json` names the key in its own message for the two failures a
/// hand-edited `tasks.json` actually hits: an unknown field (rejected here by
/// `#[serde(deny_unknown_fields)]`) reads `unknown field \`foo\`, ...`, and a type
/// mismatch reads `invalid type: string "x", expected a boolean`. Anything else
/// (truncated input, a stray comma) has no field to name and reports that.
fn offending_field(err: &serde_json::Error, raw: &str) -> String {
    let message = err.to_string();
    if let Some(rest) = message.split("unknown field `").nth(1)
        && let Some(name) = rest.split('`').next()
        && !name.is_empty()
    {
        return format!("`{name}` (unknown field)");
    }
    if message.contains("invalid type:")
        && let Some(field) = field_nearest_offset(raw, err.line(), err.column())
    {
        return format!("`{field}` (wrong type)");
    }
    "none - the JSON itself is not well-formed (not a named field)".to_string()
}

/// The line the failure is on, when it can be extracted.
fn offending_line(raw: &str, line: usize) -> Option<&str> {
    if line == 0 {
        return None;
    }
    raw.lines().nth(line - 1)
}

/// The JSON key on (or immediately before) the failure line.
///
/// A type mismatch points at the *value*, so the key is found by scanning the
/// failing line for the last `"key":` before the reported column, falling back
/// to the previous line when the value starts a new one.
fn field_nearest_offset(raw: &str, line: usize, column: usize) -> Option<String> {
    let mut candidates: Vec<String> = Vec::new();
    if let Some(text) = offending_line(raw, line) {
        let up_to: String = text.chars().take(column.max(1)).collect();
        candidates.extend(json_keys_in(&up_to));
    }
    if candidates.is_empty()
        && line > 1
        && let Some(text) = offending_line(raw, line - 1)
    {
        candidates.extend(json_keys_in(text));
    }
    candidates.pop()
}

/// Every `"name":` key in `text`, in order.
fn json_keys_in(text: &str) -> Vec<String> {
    let mut keys = Vec::new();
    let mut rest = text;
    while let Some(quote) = rest.find('"') {
        let after_open = &rest[quote + 1..];
        let Some(close) = after_open.find('"') else {
            break;
        };
        let name = &after_open[..close];
        let after_close = &after_open[close + 1..];
        let trimmed = after_close.trim_start();
        if trimmed.starts_with(':') && !name.is_empty() {
            keys.push(name.to_string());
        }
        rest = after_close;
    }
    keys
}

impl TaskStore {
    /// Open (or create) a `TaskStore` at `team_dir/tasks.json`.
    pub fn open(team_dir: &Path) -> Result<Self> {
        let path = team_dir.join("tasks.json");
        Ok(Self {
            path,
            team_dir: team_dir.to_path_buf(),
        })
    }

    /// Return the companion lock-file path for `tasks.json`.
    fn lock_path(&self) -> PathBuf {
        let mut name = self.path.as_os_str().to_os_string();
        name.push(".lock");
        PathBuf::from(name)
    }

    /// Acquire an advisory `flock` on the companion lock file.
    fn acquire_lock(&self, exclusive: bool) -> Result<File> {
        let lock = self.lock_path();
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock)
            .with_context(|| format!("open lock file {}", lock.display()))?;
        if exclusive {
            file.lock_exclusive()
                .with_context(|| format!("acquire exclusive lock on {}", lock.display()))?;
        } else {
            file.lock_shared()
                .with_context(|| format!("acquire shared lock on {}", lock.display()))?;
        }
        Ok(file)
    }

    /// Read the current task list (acquires a shared lock on the lock file).
    ///
    /// M5-T2: migrates the loaded list to the current schema version before
    /// returning it.
    ///
    /// # Errors
    ///
    /// A malformed `tasks.json` is an error naming the file, the byte offset and
    /// the offending field (FR-037) - never a silent empty list.
    pub fn read(&self) -> Result<TaskList> {
        let lock = self.acquire_lock(false)?;
        let raw = if self.path.exists() {
            fs::read_to_string(&self.path)
                .with_context(|| format!("read {}", self.path.display()))?
        } else {
            String::new()
        };
        drop(lock);
        parse_task_list(&self.path, &raw, &format!("parse {}", self.path.display()))
    }

    /// Write `list` to `path` atomically while the caller holds the lock file.
    ///
    /// The temp file name includes a UUID so concurrent writers cannot collide
    /// on the same temp path (Milestone 1, M1-T4).
    ///
    /// FR-038: the list is validated here, after every mutation and before a
    /// single byte reaches the disk, so an invalid list is refused rather than
    /// persisted. Putting the check at the one place that writes means no caller
    /// can skip it: every `TaskStore` mutator and the `TeamManager` cache's
    /// write-through both end up in this function. A refusal happens before the
    /// temp file is created, so `tasks.json` is left byte-identical and a bad
    /// edge never reaches an operator's file.
    ///
    /// # Errors
    ///
    /// The [`TaskList::validate`] refusal (FR-039 names the dangling
    /// dependency or the cycle), or the I/O / serialisation error from the
    /// write itself.
    fn write_locked(path: &Path, list: &TaskList) -> Result<()> {
        // FR-038: refuse to persist a list that fails validation. The refusal is
        // folded into one message rather than wrapped as an `anyhow` context:
        // `Display` on a context chain prints only the outermost frame, and the
        // frame an operator needs is the one naming the task and the edge.
        list.validate()
            .map_err(|e| anyhow!("refusing to write invalid {}: {e}", path.display()))?;
        // M5-T5: stamp updated_at on every write.
        let mut to_write = list.clone();
        to_write.updated_at = Some(Utc::now());
        if to_write.schema_version == 0 {
            to_write.schema_version = TASK_LIST_SCHEMA_VERSION;
        }
        let json = serde_json::to_string_pretty(&to_write)?;
        let file_name = path
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "tasks.json".to_string());
        let temp_path = path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(format!(".{file_name}.{}", uuid::Uuid::new_v4()));
        fs::write(&temp_path, json)?;
        let result: Result<()> = (|| {
            let temp = OpenOptions::new().read(true).open(&temp_path)?;
            temp.sync_all()
                .with_context(|| format!("sync temp file {}", temp_path.display()))?;
            fs::rename(&temp_path, path)
                .with_context(|| format!("rename {} -> {}", temp_path.display(), path.display()))?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temp_path);
        }
        result
    }

    /// PERF-023: write-through persistence entry point used by
    /// [`TeamManager`]'s in-memory `TaskList` cache.
    ///
    /// Acquires an exclusive `flock`, re-reads the on-disk list, applies
    /// `f` to it, and writes the result back atomically. The re-read +
    /// merge guard makes writes safe against external processes that may
    /// have mutated the file between the cache's load and this write: if
    /// the on-disk list has diverged from the cached snapshot, the merge
    /// function reconciles the in-memory mutation with the latest disk
    /// state (rather than blindly overwriting it).
    ///
    /// Returns the freshly-written [`TaskList`] so the caller can refresh
    /// its in-memory cache without an extra round-trip.
    pub fn write_through<F>(&self, f: F) -> Result<TaskList>
    where
        F: FnOnce(&mut TaskList),
    {
        self.write_through_fallible(|list| {
            f(list);
            Ok(())
        })
    }

    /// Fallible [`TaskStore::write_through`]: `f` may refuse the mutation by
    /// returning an error, in which case nothing is written and the refusal is
    /// propagated with `tasks.json` byte-identical.
    ///
    /// This is what `TaskStore::complete` gets structurally from returning
    /// before `write_locked`; the cached `TeamManager` path needs it explicitly
    /// because it reaches the write through a closure. Refusing inside the
    /// closure rather than after it is what keeps a rejected operation from
    /// bumping `updated_at` and so keeps FR-002's "shall not mutate" true for
    /// the file, not just for the task (FR-004).
    pub fn write_through_fallible<F>(&self, f: F) -> Result<TaskList>
    where
        F: FnOnce(&mut TaskList) -> Result<()>,
    {
        let lock = self.acquire_lock(true)?;
        let raw = if self.path.exists() {
            fs::read_to_string(&self.path)?
        } else {
            String::new()
        };
        let mut list: TaskList = parse_task_list(&self.path, &raw, "parse tasks.json")?;
        if let Err(e) = f(&mut list) {
            drop(lock);
            return Err(e);
        }
        Self::write_locked(&self.path, &list)?;
        drop(lock);
        Ok(list)
    }

    /// Return the path to the `tasks.json` file (PERF-023: exposed so the
    /// in-memory cache on [`TeamManager`] can stat it for mtime-based
    /// invalidation without re-opening a `TaskStore`).
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Atomically claim the next available task for `agent_id`.
    ///
    /// This is the uncached entry point; the whole decision - "one task at a
    /// time", which tasks are claimable, what the move writes, and what a
    /// refusal says - belongs to [`claim_task_in_list`], which
    /// [`TeamManager::claim_next_task`] also calls, so the two paths cannot
    /// diverge (FR-025, FR-026). This method supplies only the caching shape
    /// the store adds: the exclusive lock and the atomic write, then the
    /// [`HookEvent::TaskClaimed`] hook (FR-017) once the claim is committed.
    ///
    /// # Returns
    ///
    /// `(Some(task), already_had)` where `already_had` is `true` if the agent
    /// already owned an in-progress task (no new claim was made) or `false` for
    /// a fresh claim. Returns `(None, false)` when no tasks are available.
    ///
    /// # Errors
    ///
    /// The claim refusal, or - only after a committed claim - a hook-lookup or
    /// hook-feedback failure. The claim itself stands in that case: the caller
    /// is told the hook did not pass rather than that the claim was refused.
    ///
    /// [`TeamManager::claim_next_task`]: crate::team::manager::TeamManager::claim_next_task
    pub async fn claim_next(&self, agent_id: &str) -> Result<(Option<Task>, bool)> {
        let lock = self.acquire_lock(true)?;

        let raw = if self.path.exists() {
            fs::read_to_string(&self.path)?
        } else {
            String::new()
        };
        let mut list: TaskList = parse_task_list(&self.path, &raw, "parse tasks.json")?;

        let claim = match claim_task_in_list(&mut list, None, agent_id) {
            Ok(claim) => claim,
            Err(e) => {
                drop(lock);
                return Err(e);
            }
        };
        let already_had = claim.is_already_owned();
        let claimed = claim.task().cloned();
        // An `AlreadyOwned`/`NoneAvailable` outcome mutated nothing, so writing
        // would only bump `updated_at` for a no-op.
        if claimed.is_none() || already_had {
            drop(lock);
            return Ok((claimed, already_had));
        }

        let result = Self::write_locked(&self.path, &list);
        drop(lock);
        result?;

        // FR-017: the claim hook fires from the mutation, after the write, so an
        // auto-claim or a lead-side claim fires it too.
        if let Some(task) = claimed.as_ref() {
            let transition = TaskTransition {
                task_id: task.id.clone(),
                from: TaskStatus::Pending,
                to: TaskStatus::InProgress,
                actor: agent_id.to_owned(),
            };
            fire_task_hook(&self.team_dir, Some(&transition), task, agent_id).await?;
        }
        Ok((claimed, already_had))
    }

    /// Claim a specific task by ID for the given agent.
    ///
    /// Delegates the decision to [`claim_task_in_list`], the same function
    /// [`TaskStore::claim_next`] and [`TeamManager::claim_next_task`] call, so a
    /// named claim and a "next" claim cannot disagree about the rule or the
    /// refusal text (FR-025, FR-026). Like them, it fires the
    /// [`HookEvent::TaskClaimed`] hook once the claim is committed (FR-017).
    ///
    /// # Returns
    ///
    /// The claimed task, or an error if the task doesn't exist, is already
    /// assigned to a different agent, is in a status a claim does not accept
    /// (naming that status, so a cancelled task reads `cancelled`), has an
    /// unsatisfied dependency, or the agent already has a different task in
    /// progress.
    ///
    /// [`TeamManager::claim_next_task`]: crate::team::manager::TeamManager::claim_next_task
    pub async fn claim_specific(&self, task_id: &str, agent_id: &str) -> Result<Task> {
        let lock = self.acquire_lock(true)?;

        let raw = if self.path.exists() {
            fs::read_to_string(&self.path)?
        } else {
            String::new()
        };
        let mut list: TaskList = parse_task_list(&self.path, &raw, "parse tasks.json")?;

        let claim = match claim_task_in_list(&mut list, Some(task_id), agent_id) {
            Ok(claim) => claim,
            Err(e) => {
                drop(lock);
                return Err(e);
            }
        };
        let Some(claimed) = claim.task().cloned() else {
            drop(lock);
            return Err(anyhow!("task '{task_id}' not found"));
        };
        if claim.is_already_owned() {
            drop(lock);
            return Ok(claimed);
        }

        let result = Self::write_locked(&self.path, &list);
        drop(lock);
        result?;

        // FR-017: fire the claim hook from the mutation, not from the tool.
        let transition = TaskTransition {
            task_id: claimed.id.clone(),
            from: TaskStatus::Pending,
            to: TaskStatus::InProgress,
            actor: agent_id.to_owned(),
        };
        fire_task_hook(&self.team_dir, Some(&transition), &claimed, agent_id).await?;
        Ok(claimed)
    }

    /// Mark a task as `Completed`.  Unblocks dependents automatically (they
    /// become claimable on the next `claim_next` call).
    ///
    /// This is the uncached entry point; it delegates the whole decision to
    /// [`complete_task_in_list`], which [`TeamManager::complete_task`] also
    /// calls, so the two paths cannot diverge (FR-025). Once the completion is
    /// committed it fires the [`HookEvent::TaskCompleted`] hook (FR-017) and
    /// returns [`HookOutcome::Feedback`] unchanged, so the caller can show the
    /// gate's verdict; `team_task_complete` is that caller on every path and
    /// delivers it to the completing agent (FR-018).
    ///
    /// # Arguments
    ///
    /// * `task_id` - ID of the task to complete.
    /// * `agent_id` - the caller's agent ID. `"lead"` completes on any owner's
    ///   behalf; any other agent must be the task's `assigned_to` owner.
    ///
    /// # Returns
    ///
    /// The completed [`Task`] plus the completion hook's [`HookOutcome`], or an
    /// error when the task does not exist, is not owned by `agent_id` (FR-001,
    /// FR-002), or is still `Pending` and therefore has no
    /// `InProgress -> Completed` move available (FR-003). A hook lookup failure
    /// is also an error, but it is reported *after* the completion is
    /// persisted: the task is not left half-done.
    ///
    /// [`TeamManager::complete_task`]: crate::team::manager::TeamManager::complete_task
    pub async fn complete(&self, task_id: &str, agent_id: &str) -> Result<(Task, HookOutcome)> {
        let lock = self.acquire_lock(true)?;

        let raw = if self.path.exists() {
            fs::read_to_string(&self.path)?
        } else {
            String::new()
        };
        let mut list: TaskList = parse_task_list(&self.path, &raw, "parse tasks.json")?;

        // FR-001: the lead is exempt from the ownership rule at both entry
        // points, so the uncached path recognises the same `"lead"` agent ID the
        // manager path does. `complete_task_in_list` still holds the lead to the
        // legal-transition model.
        let is_lead = agent_id == "lead";
        let completed = match complete_task_in_list(&mut list, task_id, agent_id, is_lead) {
            Ok(task) => task,
            Err(e) => {
                drop(lock);
                // FR-044: an ownership refusal is a behaviour change from the
                // previous release, so it is recorded instead of returning
                // silently. The store holds no bus, so this writes the tracing
                // line only; `TeamManager::complete_task` publishes the event.
                if let Some(refusal) = e.downcast_ref::<CompletionRefusal>()
                    && let Ok(store) = TeamStore::load(&self.team_dir)
                {
                    publish_completion_rejected(
                        refusal,
                        &store.config.name,
                        Some(&store.config.lead_session_id),
                        None,
                    );
                }
                return Err(e);
            }
        };

        let result = Self::write_locked(&self.path, &list);
        drop(lock);
        result?;

        // FR-017: fire the completion hook from the mutation, not from the tool.
        // Idempotent re-completions land here too; the hook is the caller's to
        // interpret (it sees `completed_by`/`completed_at` in the payload).
        let transition = TaskTransition {
            task_id: completed.id.clone(),
            from: TaskStatus::InProgress,
            to: TaskStatus::Completed,
            actor: agent_id.to_owned(),
        };
        let outcome =
            fire_task_hook(&self.team_dir, Some(&transition), &completed, agent_id).await?;
        Ok((completed, outcome))
    }

    /// Cancel a task (acquires an exclusive lock) - spec `teamssetup` FR-005.
    ///
    /// The decision (which statuses may be cancelled, the refusal text, the legal
    /// transition) is [`cancel_task_in_list`]'s, the same function
    /// [`TeamManager::cancel_task`] calls, so the cached and uncached paths cannot
    /// disagree (FR-025, FR-026). This method only supplies the locking shape: the
    /// exclusive `flock` is held across the whole read-modify-write cycle
    /// (NFR-003) and dropped before the result is returned.
    ///
    /// # Arguments
    ///
    /// * `task_id` - ID of the task to cancel.
    /// * `actor` - the agent requesting the cancellation; `"lead"` in practice
    ///   (FR-005 makes cancellation the lead's alone - the caller checks that).
    ///
    /// # Returns
    ///
    /// A [`TaskCancellation`] for the cancelled task, or the refusal
    /// [`cancel_task_in_list`] produced - returned before the write lock is
    /// reached, so `tasks.json` is left byte-identical.
    ///
    /// [`TeamManager::cancel_task`]: crate::team::manager::TeamManager::cancel_task
    pub fn cancel(&self, task_id: &str, actor: &str) -> Result<TaskCancellation> {
        let lock = self.acquire_lock(true)?;

        let raw = if self.path.exists() {
            fs::read_to_string(&self.path)?
        } else {
            String::new()
        };
        let mut list: TaskList = parse_task_list(&self.path, &raw, "parse tasks.json")?;

        let cancelled = match cancel_task_in_list(&mut list, task_id, actor) {
            Ok(cancelled) => cancelled,
            Err(e) => {
                drop(lock);
                return Err(e);
            }
        };

        let result = Self::write_locked(&self.path, &list);
        drop(lock);
        result?;
        Ok(cancelled)
    }

    /// Assign a `Pending` task straight to `agent_id` in `InProgress` (the
    /// lead's `team_assign_task` path).
    ///
    /// This is the lead-side counterpart of a claim: the task moves
    /// `Pending -> InProgress` and is assigned without the teammate asking, so
    /// the status write goes through [`transition_task`] like every other one
    /// (FR-003) and the [`HookEvent::TaskClaimed`] hook fires afterwards
    /// (FR-017) - FR-017 names a lead-side claim as one of the paths that must
    /// run the same hooks as the tool.
    ///
    /// A task that is not `Pending` is left exactly as it was (the previous
    /// release silently ignored it and returned the task unchanged), and no hook
    /// fires: nothing moved.
    ///
    /// # Errors
    ///
    /// An unknown task ID, or the transition refusal, both raised before the
    /// write lock is taken - so a refused assignment leaves `tasks.json`
    /// byte-identical. A hook-lookup failure is reported after a committed
    /// assignment.
    pub async fn assign(&self, task_id: &str, agent_id: &str) -> Result<Task> {
        let lock = self.acquire_lock(true)?;

        let raw = if self.path.exists() {
            fs::read_to_string(&self.path)?
        } else {
            String::new()
        };
        let mut list: TaskList = parse_task_list(&self.path, &raw, "parse tasks.json")?;

        let Some(existing) = list.get(task_id) else {
            drop(lock);
            return Err(anyhow!("task '{task_id}' not found"));
        };
        if existing.status != TaskStatus::Pending {
            let unchanged = existing.clone();
            drop(lock);
            return Ok(unchanged);
        }

        let transition = match transition_task(&mut list, task_id, TaskStatus::InProgress, agent_id)
        {
            Ok(transition) => transition,
            Err(e) => {
                drop(lock);
                return Err(e);
            }
        };
        let Some(task) = list.get_mut(task_id) else {
            drop(lock);
            return Err(anyhow!("task '{task_id}' not found"));
        };
        task.assigned_to = Some(agent_id.to_owned());
        task.claimed_at = Some(Utc::now());
        let assigned = task.clone();

        let result = Self::write_locked(&self.path, &list);
        drop(lock);
        result?;

        // FR-017: a lead-side assignment is a claim, so it runs the claim hook.
        if transition.is_some() {
            fire_task_hook(&self.team_dir, transition.as_ref(), &assigned, agent_id).await?;
        }
        Ok(assigned)
    }

    /// Add a new task to the list (acquires an exclusive lock).
    /// M8-T4: the previous doc comment incorrectly claimed this method does
    /// **not** acquire a lock. It does - `acquire_lock(true)` is called at
    /// the top, and the lock is held for the duration of the
    /// read-modify-write cycle via `write_locked`. This is the same
    /// race-free pattern used by all other mutating `TaskStore` methods.
    ///
    /// Once the task is persisted this fires the [`HookEvent::TaskCreated`]
    /// hook, so a task added by any route - `team_task_create`, the
    /// `team_create` seed path, a test fixture - fires it (FR-017). A rejection
    /// (`HookOutcome::Feedback`) is returned to the caller rather than undoing
    /// the add: removing a task is the creator's decision, and the tool is the
    /// only place that knows the task was a seed.
    pub async fn add_task(&self, task: Task) -> Result<HookOutcome> {
        let lock = self.acquire_lock(true)?;

        let raw = if self.path.exists() {
            fs::read_to_string(&self.path)?
        } else {
            String::new()
        };
        let mut list: TaskList = parse_task_list(&self.path, &raw, "parse tasks.json")?;

        if list.tasks.iter().any(|t| t.id == task.id) {
            drop(lock);
            return Err(anyhow!(
                "task '{}' was not created: a task with that id already exists in this team. \
                 Use a different id, or read the existing one with `team_task_list`",
                task.id
            ));
        }

        list.tasks.push(task.clone());
        let result = Self::write_locked(&self.path, &list);
        drop(lock);
        result?;
        fire_task_created_hook(&self.team_dir, &task).await
    }

    /// Atomically pre-assign a pending task to an agent in `InProgress` state.
    ///
    /// Delegates to [`claim_task_in_list`] with an explicit target, so a
    /// pre-assignment obeys the same claimability / status / one-task-at-a-time
    /// rule every other claim does (FR-007, FR-025). A task that is not
    /// claimable - including one whose dependencies are unsatisfied - is
    /// refused rather than pinned to an agent that cannot work it.
    pub fn pre_assign_task(&self, task_id: &str, agent_id: &str) -> Result<Task> {
        let lock = self.acquire_lock(true)?;

        let raw = if self.path.exists() {
            fs::read_to_string(&self.path)?
        } else {
            String::new()
        };
        let mut list: TaskList = parse_task_list(&self.path, &raw, "parse tasks.json")?;

        let claim = match claim_task_in_list(&mut list, Some(task_id), agent_id) {
            Ok(claim) => claim,
            Err(e) => {
                drop(lock);
                return Err(e);
            }
        };
        let Some(assigned) = claim.task().cloned() else {
            drop(lock);
            return Err(anyhow!("task '{task_id}' not found"));
        };
        if claim.is_already_owned() {
            drop(lock);
            return Ok(assigned);
        }

        let result = Self::write_locked(&self.path, &list);
        drop(lock);
        result?;
        Ok(assigned)
    }

    /// Update an existing task's status and/or assignment (used by `team_task_update`).
    pub fn update_task<F>(&self, task_id: &str, f: F) -> Result<Task>
    where
        F: FnOnce(&mut Task),
    {
        let lock = self.acquire_lock(true)?;

        let raw = if self.path.exists() {
            fs::read_to_string(&self.path)?
        } else {
            String::new()
        };
        let mut list: TaskList = parse_task_list(&self.path, &raw, "parse tasks.json")?;

        let available_ids: Vec<String> = list
            .tasks
            .iter()
            .map(|t| format!("{} ({})", t.id, t.title))
            .collect();
        let Some(task) = list.get_mut(task_id) else {
            drop(lock);
            return Err(anyhow!(
                "task '{task_id}' not found. Available tasks: [{}]",
                available_ids.join(", ")
            ));
        };
        f(task);
        let updated = task.clone();

        let result = Self::write_locked(&self.path, &list);
        drop(lock);
        result?;
        Ok(updated)
    }

    /// Remove a task from the store by ID.
    pub fn remove_task(&self, task_id: &str) -> Result<Task> {
        let lock = self.acquire_lock(true)?;

        let raw = if self.path.exists() {
            fs::read_to_string(&self.path)?
        } else {
            String::new()
        };
        let mut list: TaskList = parse_task_list(&self.path, &raw, "parse tasks.json")?;

        let pos = list
            .tasks
            .iter()
            .position(|t| t.id == task_id)
            .ok_or_else(|| anyhow!("task '{task_id}' not found"))?;
        let removed = list.tasks.remove(pos);

        let result = Self::write_locked(&self.path, &list);
        drop(lock);
        result?;
        Ok(removed)
    }

    // -- PERF-016: async `spawn_blocking` wrappers ------------------------------
    //
    // Each of these mirrors a synchronous `TaskStore` method but moves the
    // full read-modify-write cycle (file lock + `fs::read_to_string` +
    // `serde_json` deserialise + `fs::write`) onto a tokio blocking-pool
    // thread so the async executor is never stalled on synchronous I/O
    // during concurrent teammate activity.

    /// PERF-016: `spawn_blocking` wrapper around [`TaskStore::read`].
    pub async fn read_blocking(&self) -> Result<TaskList> {
        let team_dir = self.team_dir.clone();
        tokio::task::spawn_blocking(move || {
            let store = Self::open(&team_dir)?;
            store.read()
        })
        .await?
    }

    /// PERF-016: `spawn_blocking` wrapper around [`TaskStore::cancel`].
    pub async fn cancel_blocking(
        &self,
        task_id: String,
        actor: String,
    ) -> Result<TaskCancellation> {
        let team_dir = self.team_dir.clone();
        tokio::task::spawn_blocking(move || {
            let store = Self::open(&team_dir)?;
            store.cancel(&task_id, &actor)
        })
        .await?
    }

    /// `spawn_blocking` wrapper around the pre-assign half of the claim path.
    pub async fn pre_assign_task_blocking(
        &self,
        task_id: String,
        agent_id: String,
    ) -> Result<Task> {
        let team_dir = self.team_dir.clone();
        tokio::task::spawn_blocking(move || {
            let store = Self::open(&team_dir)?;
            store.pre_assign_task(&task_id, &agent_id)
        })
        .await?
    }

    /// PERF-016: `spawn_blocking` wrapper around [`TaskStore::update_task`].
    pub async fn update_task_blocking<F>(&self, task_id: String, f: F) -> Result<Task>
    where
        F: FnOnce(&mut Task) + Send + 'static,
    {
        let team_dir = self.team_dir.clone();
        tokio::task::spawn_blocking(move || {
            let store = Self::open(&team_dir)?;
            store.update_task(&task_id, f)
        })
        .await?
    }
}
