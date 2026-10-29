//! Sub-agent event plumbing for the Agents window (spec `agentui` T-013).
//!
//! The Agents table is a projection of [`App::active_tasks`] (A2), so the
//! event handler must keep that list exactly in step with the runtime task
//! lifecycle. This module owns those state transitions so the event handlers
//! stay one-line delegations and the semantics are testable in one place:
//!
//! - [`App::note_subagent_started`] appends a running row (FR-047) or updates
//!   the existing row in place when the task identifier or the per-instance
//!   agent identifier (the child session) is already painted (FR-072);
//! - [`App::note_subagent_completed`] records the report status derived from
//!   the finish reason and removes the row (FR-048);
//! - [`App::note_subagent_cancelled`] removes the row (FR-049);
//!
//! Both removals snapshot the finished task into [`App::agents_history`] first
//! (spec `agentssetup` FR-021, FR-028), so a terminal row survives in the
//! panel's bounded history section - carrying the report status and the
//! `output_file` the row had at the moment it left - instead of vanishing from
//! both sections. The history row is where FR-032's "no durable copy" note is
//! read from, so the snapshot has to happen while the entry is still in hand.
//!
//! - [`App::note_subagent_suspended`] / [`App::note_subagent_resumed`]
//!   re-status the row so the painted status matches the runtime status
//!   (FR-049).
//!
//! None of these touch the event bus, the task registry or the log: the
//! caller keeps the log lines and the telemetry counters, which only fire when
//! a row was genuinely appended (a replayed spawn must not double-count).

use ragent_agent::task::{ReportStatus, TaskEntry, TaskStatus};

use super::state::{App, CompletedTaskRow};

impl App {
    /// Index of the painted row for `task_id`, matching either the task
    /// identifier or the per-instance agent identifier (the child session).
    ///
    /// FR-072 requires a replayed spawn to update the existing row instead of
    /// appending a second one. A task id identifies the run; the child session
    /// identifies the agent instance that run lives in, so either being already
    /// painted means this is the same row.
    fn painted_task_index(&self, task_id: &str, child_session_id: &str) -> Option<usize> {
        self.active_tasks
            .iter()
            .position(|t| t.id == task_id || t.child_session_id == child_session_id)
    }

    /// Append a running row for a sub-agent spawn, or update the row already
    /// painted for it (FR-047, FR-072).
    ///
    /// `background` and `detached` are the row's execution contract as the task
    /// registry recorded it: the event carries both, so the painted row reports
    /// what was spawned rather than an assumed default (FR-020). A detached run
    /// is also a background run, so the two flags are assigned independently
    /// and neither is derived from the other here.
    ///
    /// Returns `true` when a new row was appended and `false` when an existing
    /// row was refreshed, so the caller can keep the telemetry counters honest
    /// across a duplicated event.
    pub(crate) fn note_subagent_started(
        &mut self,
        session_id: &str,
        task_id: &str,
        child_session_id: &str,
        agent: &str,
        task: &str,
        background: bool,
        detached: bool,
    ) -> bool {
        if let Some(idx) = self.painted_task_index(task_id, child_session_id) {
            if let Some(entry) = self.active_tasks.get_mut(idx) {
                entry.id = task_id.to_string();
                entry.parent_session_id = session_id.to_string();
                entry.child_session_id = child_session_id.to_string();
                entry.agent_name = agent.to_string();
                entry.task_prompt = task.to_string();
                entry.background = background;
                entry.detached = detached;
                // A replayed spawn restarts the run: the row is live again and
                // a stale suspension or finished marker must not survive it.
                entry.status = TaskStatus::Running;
                entry.completed_at = None;
                entry.report_status = ReportStatus::default();
            }
            return false;
        }

        self.active_tasks.push(TaskEntry {
            id: task_id.to_string(),
            parent_session_id: session_id.to_string(),
            child_session_id: child_session_id.to_string(),
            agent_name: agent.to_string(),
            task_prompt: task.to_string(),
            background,
            detached,
            status: TaskStatus::Running,
            result: None,
            error: None,
            created_at: chrono::Utc::now(),
            completed_at: None,
            reported: false,
            waiter_count: 0,
            output_file: None,
            report_status: ReportStatus::default(),
            time_bound_secs: None,
        });
        true
    }

    /// Record the report status for a finished row and remove it (FR-048).
    ///
    /// The status is derived from the finish reason through
    /// [`ReportStatus::from_finish_reason_label`] - the same classifier the
    /// task registry uses for `list_agents` / `wait_agents` - so the panel and
    /// the tools can never disagree about whether a report was cut short. It is
    /// written onto the entry before the row is dropped and returned so the
    /// caller can log the completion (with its truncation marker) from the same
    /// derived value instead of re-matching the raw label.
    ///
    /// Returns `None` when no row was painted for the task (the completion is
    /// then log-only and must not mutate anything else).
    pub(crate) fn note_subagent_completed(
        &mut self,
        task_id: &str,
        finish_reason: &str,
    ) -> Option<ReportStatus> {
        let idx = self.active_tasks.iter().position(|t| t.id == task_id)?;
        let report_status = ReportStatus::from_finish_reason_label(finish_reason);
        if let Some(entry) = self.active_tasks.get_mut(idx) {
            entry.report_status = report_status;
            entry.status = TaskStatus::Completed;
            entry.completed_at = Some(chrono::Utc::now());
        }
        let entry = self.active_tasks.remove(idx);
        // Record into history *before* the row stops being reachable
        // (FR-028): the live row is gone from here on, so a completed run
        // that is not snapshotted now is lost from both sections.
        self.record_completed_task(&entry);
        Some(report_status)
    }

    /// Remove the row for a cancelled task (FR-049).
    ///
    /// Returns the row's child session id when one was tracked, so the caller
    /// can attribute the cancel log line to that agent's output view.
    pub(crate) fn note_subagent_cancelled(&mut self, task_id: &str) -> Option<String> {
        let idx = self.active_tasks.iter().position(|t| t.id == task_id)?;
        let child_session_id = self
            .active_tasks
            .get(idx)
            .map(|t| t.child_session_id.clone());
        let entry = self.active_tasks.remove(idx);
        // A cancelled run is finished too, and it is exactly the status most
        // likely to have lost its durable copy (FR-032), so it is snapshotted
        // on the same record-before-removal rule as a completion (FR-028).
        self.record_completed_task(&entry);
        child_session_id
    }

    /// Snapshot a finished task into the completed history (FR-021, FR-028).
    ///
    /// History is newest-first and capped at [`App::agents_history_cap`], so
    /// the oldest row is evicted once the cap is exceeded and the panel's
    /// memory cost stays constant in the number of tasks ever spawned
    /// (FR-036, NFR-004). A cap of `0` cannot happen through configuration
    /// (`agents.completed_history` is clamped to `1..=200`) but is treated as
    /// "keep nothing" rather than indexing an empty deque.
    fn record_completed_task(&mut self, entry: &TaskEntry) {
        if self.agents_history_cap == 0 {
            return;
        }
        self.agents_history.push_front(CompletedTaskRow {
            id: entry.id.clone(),
            child_session_id: entry.child_session_id.clone(),
            agent_name: entry.agent_name.clone(),
            status: entry.status.clone(),
            background: entry.background,
            detached: entry.detached,
            created_at: entry.created_at,
            completed_at: entry.completed_at.unwrap_or_else(chrono::Utc::now),
            report_status: entry.report_status,
            output_file: entry.output_file.clone(),
        });
        while self.agents_history.len() > self.agents_history_cap {
            self.agents_history.pop_back();
        }
    }

    /// Mark the tracked row suspended (FR-049).
    ///
    /// Returns `true` when a row was re-statused; an untracked task is left
    /// alone so a stale or replayed event cannot resurrect a removed row.
    pub(crate) fn note_subagent_suspended(&mut self, task_id: &str) -> bool {
        self.restatus_subagent(task_id, TaskStatus::Suspended)
    }

    /// Mark the tracked row running again (FR-049).
    ///
    /// Returns `true` when a row was re-statused; see
    /// [`App::note_subagent_suspended`].
    pub(crate) fn note_subagent_resumed(&mut self, task_id: &str) -> bool {
        self.restatus_subagent(task_id, TaskStatus::Running)
    }

    /// Shared body of the suspend / resume transitions.
    fn restatus_subagent(&mut self, task_id: &str, status: TaskStatus) -> bool {
        let Some(entry) = self.active_tasks.iter_mut().find(|t| t.id == task_id) else {
            return false;
        };
        entry.status = status;
        true
    }
}
