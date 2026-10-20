//! Open/close routing for the Agents and Teams modal windows.
//!
//! The two windows are mutually exclusive popups over the chat surface: at most
//! one is visible at a time, and each is reachable only while its subject
//! exists (sub-agent or background shell tasks for Agents, an active team for
//! Teams). This module owns that decision so the input handler and the button
//! renderer cannot drift apart:
//!
//! - the renderer calls [`App::agents_window_available`] /
//!   [`App::teams_window_available`] to pick the enabled versus dimmed button
//!   style (spec `agentui` FR-002..FR-004);
//! - the mouse router calls [`App::toggle_agents_window`] /
//!   [`App::toggle_teams_window`] / [`App::close_agents_window`] /
//!   [`App::close_teams_window`] for the button and Close-button clicks
//!   (FR-028..FR-032);
//! - the destructive row-action arm calls [`App::open_row_action`] to arm the
//!   yes/no confirmation dialog and [`App::row_action_confirm_accept`] /
//!   [`App::row_action_confirm_cancel`] to answer it (spec `agentui` T-009,
//!   FR-036, FR-037, FR-040, FR-069).
//!
//! Every path that actually repaints a window sets [`App::needs_redraw`] so the
//! next frame reflects the change without waiting for an unrelated event
//! (NFR-005). A refused click (no subject) changes nothing at all, including
//! the redraw flag (FR-029, FR-031).

use super::state::{
    App, PendingRowAction, ROW_ACTION_CONFIRM_NO, ROW_ACTION_CONFIRM_YES, RowAction,
};
use ragent_agent::team::MemberStatus;

impl App {
    /// Whether the Agents window has anything to show.
    ///
    /// True while at least one sub-agent task or one background shell task is
    /// tracked, which is exactly the condition the input-side button uses to
    /// decide between its enabled and dimmed styles (FR-004).
    #[must_use]
    pub fn agents_window_available(&self) -> bool {
        !self.active_tasks.is_empty() || !self.bg_tasks.is_empty()
    }

    /// Whether the Teams window has anything to show.
    ///
    /// True while a team is active (FR-004).
    #[must_use]
    pub fn teams_window_available(&self) -> bool {
        self.active_team.is_some()
    }

    /// Toggle the Agents window (spec `agentui` FR-028, FR-029).
    ///
    /// With no sub-agent and no background shell task the click is a no-op that
    /// leaves both the window state and the redraw flag untouched (FR-029).
    /// Otherwise the window toggles and, when it is being opened, the Teams
    /// window is closed so the two can never be painted together.
    pub fn toggle_agents_window(&mut self) {
        if !self.agents_window_available() {
            return;
        }
        self.show_agents_window = !self.show_agents_window;
        if self.show_agents_window {
            self.show_teams_window = false;
        }
        self.needs_redraw = true;
    }

    /// Toggle the Teams window (spec `agentui` FR-030, FR-031).
    ///
    /// With no active team the click is a no-op (FR-031). Otherwise the window
    /// toggles and, when it is being opened, the Agents window is closed.
    pub fn toggle_teams_window(&mut self) {
        if !self.teams_window_available() {
            return;
        }
        self.show_teams_window = !self.show_teams_window;
        if self.show_teams_window {
            self.show_agents_window = false;
        }
        self.needs_redraw = true;
    }

    /// Dismiss the Agents window via its Close button (spec `agentui` FR-032).
    ///
    /// Only the visibility flag changes: every task, every team member and the
    /// input buffer are left exactly as they were. Closing an already-closed
    /// window is a no-op so nothing is repainted for no reason.
    pub fn close_agents_window(&mut self) {
        if !self.show_agents_window {
            return;
        }
        self.show_agents_window = false;
        self.needs_redraw = true;
    }

    /// Close the Agents window when its subject has emptied
    /// (spec `agentui` FR-063, TC-031).
    ///
    /// Called from every path that removes a sub-agent or background-shell row
    /// (the `SubagentComplete`/`SubagentCancelled`/`SubagentKilled` event arms,
    /// the `BackgroundTaskCompleted` arm and the registry reconciliation
    /// merge). Window open/close and row-button actions keep being serviced
    /// while a turn runs, so the removal that empties the list can happen at
    /// any time; a no-op when the window is already closed or still has a
    /// subject. The close itself is silent - FR-064's reportable-outcome list
    /// does not include it.
    #[doc(hidden)]
    pub fn close_agents_window_when_empty(&mut self) {
        if self.show_agents_window && self.agents_window_available() {
            return;
        }
        self.close_agents_window();
    }

    /// Dismiss the Teams window via its Close button (spec `agentui` FR-032).
    ///
    /// Only the visibility flag changes; see [`App::close_agents_window`].
    pub fn close_teams_window(&mut self) {
        if !self.show_teams_window {
            return;
        }
        self.show_teams_window = false;
        self.needs_redraw = true;
    }

    // -- Destructive row-action confirmation dialog (T-009) -------------------

    /// Open the yes/no confirmation dialog for a destructive row action
    /// (spec `agentui` T-009: FR-036, FR-037, FR-040).
    ///
    /// Only arms the dialog - no suspension, termination or shutdown runs until
    /// the user answers `Yes`. The selection resets to `No` every time it opens
    /// so a stray `Enter` cannot confirm the action by accident (FR-069).
    pub fn open_row_action(&mut self, action: RowAction) {
        self.row_action_confirm = Some(PendingRowAction {
            action,
            selected: ROW_ACTION_CONFIRM_NO,
        });
        self.needs_redraw = true;
    }

    /// Whether the destructive row-action confirmation dialog is open.
    #[must_use]
    pub fn row_action_confirm_open(&self) -> bool {
        self.row_action_confirm.is_some()
    }

    /// Flip the destructive row-action dialog selection between `Yes` and `No`
    /// (spec `agentui` T-009). The dialog has exactly two options, so any
    /// horizontal move (`Left` / `Right` / `Tab` / `BackTab`) simply toggles
    /// the current selection; the redraw flag is set so the change paints on
    /// the next frame (NFR-005).
    pub fn row_action_confirm_toggle(&mut self) {
        let Some(pending) = self.row_action_confirm.as_mut() else {
            return;
        };
        pending.selected = if pending.selected == ROW_ACTION_CONFIRM_YES {
            ROW_ACTION_CONFIRM_NO
        } else {
            ROW_ACTION_CONFIRM_YES
        };
        self.needs_redraw = true;
    }

    /// Whether the destructive row-action dialog currently has `Yes` selected.
    #[must_use]
    pub fn row_action_confirm_is_yes(&self) -> bool {
        self.row_action_confirm
            .as_ref()
            .is_some_and(|p| p.selected == ROW_ACTION_CONFIRM_YES)
    }

    /// Run the pending row action and close the dialog.
    ///
    /// Called by the `Yes` arm of the input dispatch. The subject is re-checked
    /// against the live lists first, so answering a dialog whose row vanished
    /// in the meantime mutates nothing (FR-069) - the same stale-row rule the
    /// mouse router applies to the row controls themselves. The dispatch is
    /// spawned on the async runtime and never blocks the caller (NFR-008 path
    /// shared with T-010).
    pub fn row_action_confirm_accept(&mut self) {
        let Some(pending) = self.row_action_confirm.take() else {
            return;
        };
        self.needs_redraw = true;
        match pending.action {
            RowAction::KillSubagent { task_id, .. } => {
                if self.active_tasks.iter().any(|t| t.id == task_id) {
                    self.kill_agent_task(&task_id);
                } else if self.bg_tasks.iter().any(|t| t.id == task_id) {
                    // The row could have transitioned to a background shell
                    // task between open and answer; cancel via the same
                    // service either way (FR-037).
                    self.cancel_bg_task(&task_id);
                }
            }
            RowAction::CancelBackground { task_id, .. } => {
                if self.bg_tasks.iter().any(|t| t.id == task_id) {
                    self.cancel_bg_task(&task_id);
                }
            }
            RowAction::SuspendTeammate { agent_id, .. } => {
                // Suspend only when the member is actually live and not
                // already suspended - the re-check mirrors the stale-row
                // rule (FR-069).
                let status = member_status_of(self, &agent_id);
                if let Some(status) = status
                    && status != MemberStatus::Suspended
                    && !is_terminal_member_status(status)
                {
                    self.dispatch_suspend_teammate(&agent_id);
                }
            }
            RowAction::ShutdownTeammate { agent_id, .. } => {
                let status = member_status_of(self, &agent_id);
                if let Some(status) = status
                    && !is_terminal_member_status(status)
                {
                    self.dispatch_shutdown_teammate(&agent_id);
                }
            }
        }
    }

    /// Dismiss the destructive row-action dialog: the `No` option or `Esc`.
    /// Leaves the row untouched (FR-069) and only resets the dialog state.
    pub fn row_action_confirm_cancel(&mut self) {
        if self.row_action_confirm.take().is_some() {
            self.needs_redraw = true;
        }
    }
}

/// The status of the team member `agent_id`, `None` when the list no longer
/// carries it (FR-069 stale-row rule).
fn member_status_of(app: &App, agent_id: &str) -> Option<MemberStatus> {
    app.team_members
        .iter()
        .find(|m| m.agent_id == agent_id)
        .map(|m| m.status)
}

/// Whether `status` is terminal, in which case no suspend or shutdown can be
/// requested (mirrors the row-control terminal-row rule, FR-071).
fn is_terminal_member_status(status: MemberStatus) -> bool {
    matches!(
        status,
        MemberStatus::ShuttingDown | MemberStatus::Stopped | MemberStatus::Failed
    )
}
