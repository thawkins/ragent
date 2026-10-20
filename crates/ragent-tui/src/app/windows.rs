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

    /// Close the output view overlay (spec `agentui` FR-075).
    ///
    /// The overlay and the selected agent identity it was opened from clear
    /// together - the same state the `Esc` and outside-click dismissal paths
    /// establish, so reopening the Agents/Teams window is left with a clean
    /// selection. Closing an already-closed overlay is a no-op.
    pub fn close_output_view(&mut self) {
        if self.output_view.is_none() {
            return;
        }
        self.output_view = None;
        self.selected_agent_session_id = None;
        self.selected_agent_index = None;
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
// -- Keyboard block-cursor navigation (Agents / Teams windows) ---------------

/// Key-handling contract for the two modal windows, keyboard-first (upheld by
/// `handle_key_event` in `input_handler.rs`):
///
/// - the window keys arm before the main input editor so no key leaks into the
///   message buffer (FR-055) while a popup owns the keyboard;
/// - Up/Down move the block cursor along the window's selectable rows; PgUp/
///   PgDn page one viewport at a time; both keep the viewport clamped to the
///   renderer-published maximum (FR-059 invariant shared with the wheel arms);
/// - Space toggles stop/resume on the selected row; `Del` (and the legacy
///   `Shift+K`) arms the destructive confirm dialog; `Enter` opens the row's
///   output overlay - the same destination chains the old mouse row click
///   drove (FR-033/FR-034, FR-038).
///
/// The lead row is excluded by construction: the Agents cursor range covers
/// sub-agent ordinals only and the Teams cursor range covers teammate
/// ordinals only, so no key path can select or terminate the primary agent or
/// the lead (FR-067).
impl App {
    /// Move the Agents-window cursor by `direction` rows (-1 = up, 1 = down).
    ///
    /// A first press with no cursor starts from the top of the list; moving
    /// past either end clamps at the end without reporting (the cursor simply
    /// stays) - the same quiet-clamp contract the wheel scroll uses.
    pub fn move_agents_cursor(&mut self, direction: i16) {
        let len = self.agents_subagent_task_ids.len();
        if len == 0 {
            return;
        }
        let next = match self.agents_cursor_subagent {
            Some(current) => {
                let target = current as isize + direction as isize;
                target.clamp(0, len as isize - 1) as usize
            }
            None => {
                // First press: the cursor appears on the first row when
                // moving down, on the last row when moving up.
                if direction < 0 { len - 1 } else { 0 }
            }
        };
        if self.agents_cursor_subagent != Some(next) {
            self.agents_cursor_subagent = Some(next);
            self.scroll_agents_cursor_into_view();
            self.status = format!(
                "agents: {}",
                self.agents_subagent_task_ids
                    .get(next)
                    .map(|id| crate::app::helpers::short_id(id))
                    .unwrap_or_default()
            );
            self.needs_redraw = true;
        }
    }

    /// Page the Agents-window cursor by one viewport worth of rows
    /// (`direction` -1 = up, 1 = down), clamped to the row range.
    ///
    /// The page height is the renderer's last viewport height minus a small
    /// overlap row so the landed-on row stays visible at the page edge; a
    /// renderer that has not painted yet falls back to a full page of
    /// [`SCROLL_STEP_LINES`] rows x 3 - enough for a sensible first page move
    /// before the first frame.
    pub fn page_agents_cursor(&mut self, direction: i16) {
        let len = self.agents_subagent_task_ids.len() as isize;
        if len == 0 {
            return;
        }
        let visible = self.active_agents_area.height.max(3).saturating_sub(1) as isize;
        let current = self
            .agents_cursor_subagent
            .map_or(if direction < 0 { len - 1 } else { 0 }, |c| c as isize);
        let next = (current + direction as isize * visible).clamp(0, len - 1) as usize;
        if self.agents_cursor_subagent != Some(next) {
            self.agents_cursor_subagent = Some(next);
            self.scroll_agents_cursor_into_view();
            self.status = format!(
                "agents: {}",
                self.agents_subagent_task_ids
                    .get(next)
                    .map(|id| crate::app::helpers::short_id(id))
                    .unwrap_or_default()
            );
            self.needs_redraw = true;
        }
    }

    /// Keep the Agents viewport positioned so the cursor's painted row is
    /// visible. Runs every cursor move; the renderer publishes the painted
    /// row positions (`agents_row_task_ids`) and the maximum scroll, both of
    /// which the viewport follows here.
    fn scroll_agents_cursor_into_view(&mut self) {
        let Some(cursor_id) = self
            .agents_cursor_subagent
            .and_then(|c| self.agents_subagent_task_ids.get(c).cloned())
        else {
            return;
        };
        // Resolve the cursor's painted row out of the last render's row map.
        // Before the first frame the map is empty and no adjust happens: the
        // offset is 0 and the renderer paints from the top.
        let Some(painted_row) = self
            .agents_row_task_ids
            .iter()
            .position(|key| *key == cursor_id)
        else {
            return;
        };
        let painted_row = painted_row as u16;
        let visible = self.active_agents_area.height.max(1);
        let offset = self
            .active_agents_scroll_offset
            .min(self.active_agents_max_scroll);
        if painted_row < offset {
            // Cursor scrolled above the viewport: pin it to the top row.
            self.active_agents_scroll_offset = painted_row;
        } else if painted_row >= offset.saturating_add(visible) {
            // Cursor scrolled below the viewport: pin it to the bottom row.
            self.active_agents_scroll_offset = painted_row + 1 - visible;
        }
    }

    /// Space on the Agents window: toggle stop/resume of the cursor row's
    /// sub-agent (the keyboard counterpart of the retired `[stop]` control,
    /// FR-035's no-dialog toggle direction preserved).
    pub fn agents_cursor_toggle_stop_resume(&mut self) {
        let task_id = self
            .agents_cursor_subagent
            .and_then(|c| self.agents_subagent_task_ids.get(c).cloned());
        let Some(task) = task_id
            .as_deref()
            .and_then(|id| self.active_tasks.iter().find(|t| t.id == id).cloned())
        else {
            // Stale cursor (row vanished between frames): a consumed no-op.
            self.status = "agents: selected row is gone".to_string();
            self.needs_redraw = true;
            return;
        };
        if task.status == ragent_agent::task::TaskStatus::Suspended {
            self.resume_agent_task(&task.id);
        } else {
            self.suspend_agent_task(&task.id);
        }
    }

    /// `Del` (or legacy `Shift+K`) on the Agents window: arm the destructive
    /// yes/no dialog for the cursor row's sub-agent (FR-036's confirm flow).
    pub fn agents_cursor_kill(&mut self) {
        let task_id = self
            .agents_cursor_subagent
            .and_then(|c| self.agents_subagent_task_ids.get(c).cloned());
        let Some(task) = task_id
            .as_deref()
            .and_then(|id| self.active_tasks.iter().find(|t| t.id == id).cloned())
        else {
            self.status = "agents: selected row is gone".to_string();
            self.needs_redraw = true;
            return;
        };
        self.open_row_action(crate::app::RowAction::KillSubagent {
            task_id: task.id,
            agent_name: task.agent_name,
        });
    }

    /// Mouse click-to-select for the Agents window: move the block cursor
    /// onto the painted row `abs_row` (0 = header, 1 = primary, then the
    /// sub-agent tree, then the background section - the renderer's
    /// `agents_row_task_ids` indexing).
    ///
    /// Selecting a sub-agent row restores the click-to-select half of the
    /// keyboard-first contract: the click only MOVES the cursor, so Space/
    /// `Del`/`Enter` stay the single action surface. The primary row, header
    /// and background rows are outside the cursor range (FR-067) and consume
    /// the click without mutating state.
    pub fn agents_cursor_from_click(&mut self, abs_row: usize) -> bool {
        let Some(key) = self.agents_row_task_ids.get(abs_row).cloned() else {
            return false; // Below the painted rows: bare consume.
        };
        let Some(ordinal) = self
            .agents_subagent_task_ids
            .iter()
            .position(|id| *id == key)
        else {
            return true; // Header, primary, bg heading or bg row: consumed, cursor untouched.
        };
        self.agents_cursor_subagent = Some(ordinal);
        self.scroll_agents_cursor_into_view();
        self.status = format!(
            "agents: {}",
            self.agents_subagent_task_ids
                .get(ordinal)
                .map(|id| crate::app::helpers::short_id(id))
                .unwrap_or_default()
        );
        self.needs_redraw = true;
        true
    }

    /// `Enter` on the Agents window: open the cursor row sub-agent's output
    /// overlay - the same destination chain the old row click drove (FR-034).
    pub fn agents_cursor_open_output(&mut self) {
        let task_id = self
            .agents_cursor_subagent
            .and_then(|c| self.agents_subagent_task_ids.get(c).cloned());
        let Some(task) = task_id
            .as_deref()
            .and_then(|id| self.active_tasks.iter().find(|t| t.id == id).cloned())
        else {
            self.status = "agents: selected row is gone".to_string();
            self.needs_redraw = true;
            return;
        };
        self.open_output_view_session(
            task.child_session_id.clone(),
            format!(
                "{} [{}]",
                task.agent_name,
                crate::app::helpers::short_id(&task.id)
            ),
        );
        self.needs_redraw = true;
    }

    /// Move the Teams-window cursor by `direction` rows (-1 = up, 1 = down)
    /// along the teammate rows. The same start/clamp contract as the Agents
    /// cursor.
    pub fn move_teams_cursor(&mut self, direction: i16) {
        let len = self.teams_teammate_agent_ids.len();
        if len == 0 {
            return;
        }
        let next = match self.teams_cursor_teammate {
            Some(current) => {
                let target = current as isize + direction as isize;
                target.clamp(0, len as isize - 1) as usize
            }
            None => {
                if direction < 0 {
                    len - 1
                } else {
                    0
                }
            }
        };
        if self.teams_cursor_teammate != Some(next) {
            self.teams_cursor_teammate = Some(next);
            self.scroll_teams_cursor_into_view();
            self.status = format!(
                "teams: cursor -> {}",
                self.teams_teammate_agent_ids
                    .get(next)
                    .cloned()
                    .unwrap_or_default()
            );
            self.needs_redraw = true;
        }
    }

    /// Page the Teams-window cursor by one viewport worth of rows, clamped to
    /// the teammate range (mirror of [`App::page_agents_cursor`]).
    pub fn page_teams_cursor(&mut self, direction: i16) {
        let len = self.teams_teammate_agent_ids.len() as isize;
        if len == 0 {
            return;
        }
        let visible = self.teams_rows_area.height.max(3).saturating_sub(1) as isize;
        let current = self
            .teams_cursor_teammate
            .map_or(if direction < 0 { len - 1 } else { 0 }, |c| c as isize);
        let next = (current + direction as isize * visible).clamp(0, len - 1) as usize;
        if self.teams_cursor_teammate != Some(next) {
            self.teams_cursor_teammate = Some(next);
            self.scroll_teams_cursor_into_view();
            self.status = format!(
                "teams: cursor -> {}",
                self.teams_teammate_agent_ids
                    .get(next)
                    .cloned()
                    .unwrap_or_default()
            );
            self.needs_redraw = true;
        }
    }

    /// Keep the Teams viewport positioned so the cursor's painted row is
    /// visible (mirror of [`App::scroll_agents_cursor_into_view`]; the Teams
    /// row map indexes the inner painted-rows rectangle).
    fn scroll_teams_cursor_into_view(&mut self) {
        let Some(cursor_id) = self
            .teams_cursor_teammate
            .and_then(|c| self.teams_teammate_agent_ids.get(c).cloned())
        else {
            return;
        };
        let Some(painted_row) = self
            .teams_row_agent_ids
            .iter()
            .position(|key| *key == cursor_id)
        else {
            return;
        };
        let painted_row = painted_row as u16;
        let visible = self.teams_rows_area.height.max(1);
        let offset = self.teams_scroll_offset.min(self.teams_max_scroll);
        if painted_row < offset {
            self.teams_scroll_offset = painted_row;
        } else if painted_row >= offset.saturating_add(visible) {
            self.teams_scroll_offset = painted_row + 1 - visible;
        }
    }

    /// Mouse click-to-select for the Teams window: move the cursor onto the
    /// painted row `abs_row` (0 = header, 1 = lead, then the member rows - the
    /// renderer's `teams_row_agent_ids` indexing).
    ///
    /// Same contract as [`App::agents_cursor_from_click`]: the click only
    /// MOVES the cursor; Space/`Del`/`Enter` stay the single action surface.
    /// The lead row and header/background rows are outside the cursor range
    /// (FR-067) and consume the click without mutating state.
    pub fn teams_cursor_from_click(&mut self, abs_row: usize) -> bool {
        let Some(key) = self.teams_row_agent_ids.get(abs_row).cloned() else {
            return false; // Below the painted rows: bare consume.
        };
        let Some(ordinal) = self
            .teams_teammate_agent_ids
            .iter()
            .position(|id| *id == key)
        else {
            return true; // Header, lead or bg row: consumed, cursor untouched.
        };
        self.teams_cursor_teammate = Some(ordinal);
        self.scroll_teams_cursor_into_view();
        self.status = format!(
            "teams: cursor -> {}",
            self.teams_teammate_agent_ids
                .get(ordinal)
                .cloned()
                .unwrap_or_default()
        );
        self.needs_redraw = true;
        true
    }

    /// Space on the Teams window: toggle stop/resume of the cursor row's
    /// teammate. Resuming is not destructive, so no dialog; suspending a live
    /// teammate destroys its in-flight turn, so the confirm dialog arms
    /// (FR-040's direction split preserved on the keyboard).
    pub fn teams_cursor_toggle_stop_resume(&mut self) {
        let agent_id = self
            .teams_cursor_teammate
            .and_then(|c| self.teams_teammate_agent_ids.get(c).cloned());
        let Some(member) = agent_id
            .as_deref()
            .and_then(|id| self.team_members.iter().find(|m| m.agent_id == id).cloned())
        else {
            self.status = "teams: selected row is gone".to_string();
            self.needs_redraw = true;
            return;
        };
        if member.status == ragent_agent::team::MemberStatus::Suspended {
            self.dispatch_resume_teammate(&member.agent_id);
        } else {
            self.open_row_action(crate::app::RowAction::SuspendTeammate {
                agent_id: member.agent_id,
                name: member.name,
            });
        }
    }

    /// `Del` (or legacy `K`) on the Teams window: arm the destructive yes/no
    /// dialog for the cursor row's teammate shutdown (FR-040's confirm flow).
    pub fn teams_cursor_shutdown(&mut self) {
        let agent_id = self
            .teams_cursor_teammate
            .and_then(|c| self.teams_teammate_agent_ids.get(c).cloned());
        let Some(member) = agent_id
            .as_deref()
            .and_then(|id| self.team_members.iter().find(|m| m.agent_id == id).cloned())
        else {
            self.status = "teams: selected row is gone".to_string();
            self.needs_redraw = true;
            return;
        };
        self.open_row_action(crate::app::RowAction::ShutdownTeammate {
            agent_id: member.agent_id,
            name: member.name,
        });
    }

    /// `Enter` on the Teams window: focus the cursor row's teammate and open
    /// its output overlay - the same destination chain the old row click
    /// drove (FR-038). The lead is not a cursor row, so no key path can
    /// target it (FR-067).
    pub fn teams_cursor_focus(&mut self) {
        let agent_id = self
            .teams_cursor_teammate
            .and_then(|c| self.teams_teammate_agent_ids.get(c).cloned());
        let Some(agent_id) = agent_id else {
            self.status = "teams: selected row is gone".to_string();
            self.needs_redraw = true;
            return;
        };
        self.focus_teammate_by_id(&agent_id);
    }
}
