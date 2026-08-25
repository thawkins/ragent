//! Keyboard and mouse event handling for the TUI.
use std::sync::atomic::Ordering;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

use ragent_agent::event::Event;

use crate::input::{self, InputAction};

// Prompt optimization templates

// State types from app/state.rs
use crate::app::state::{
    App, ContextAction, ContextMenuState, LogLevel, ProviderSetupStep, ScrollbarDragPane,
    SelectionPane, TextSelection,
};

// Helpers
use crate::app::helpers::short_session_id;

// Re-export status types from theme

impl App {
    pub(crate) fn handle_history_picker_key(&mut self, key: KeyEvent) {
        use crossterm::event::KeyCode;
        let picker = match self.history_picker.as_mut() {
            Some(p) => p,
            None => return,
        };
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.history_picker = None;
            }
            KeyCode::Up | KeyCode::Char('k') if picker.selected > 0 => {
                picker.selected -= 1;
                if picker.selected < picker.scroll_offset {
                    picker.scroll_offset = picker.selected;
                }
            }
            KeyCode::Down | KeyCode::Char('j') if picker.selected + 1 < picker.entries.len() => {
                picker.selected += 1;
            }
            KeyCode::Enter => {
                let chosen = picker.entries[picker.selected].clone();
                self.history_picker = None;
                self.input = chosen;
                self.set_cursor_char_index_clamped(self.input_len_chars());
            }
            _ => {}
        }
    }

    /// Dispatch keyboard navigation for the `/config list` save-picker overlay.
    ///
    /// Mirrors the history picker: Up/Down (or k/j) move the selection,
    /// Enter restores the highlighted backup over the global `ragent.json`,
    /// and Esc/q close the picker without taking any action.
    pub fn handle_config_save_picker_key(&mut self, key: KeyEvent) {
        use crossterm::event::KeyCode;

        let picker = match self.config_save_picker.as_mut() {
            Some(p) => p,
            None => return,
        };

        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.config_save_picker = None;
                self.status = "config list: cancelled".to_string();
            }
            KeyCode::Up | KeyCode::Char('k') if picker.selected > 0 => {
                picker.selected -= 1;
                if picker.selected < picker.scroll_offset {
                    picker.scroll_offset = picker.selected;
                }
            }
            KeyCode::Down | KeyCode::Char('j') if picker.selected + 1 < picker.entries.len() => {
                picker.selected += 1;
            }
            KeyCode::Enter => {
                let backup = picker.entries[picker.selected].clone();
                let config_dir = picker.config_dir.clone();
                self.config_save_picker = None;

                match ragent_config::Config::restore_global_config(Some(&config_dir), &backup) {
                    Ok(target) => {
                        self.session_processor.invalidate_config_cache();
                        let name = backup
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("<unknown>");
                        self.append_assistant_text(&format!(
                            "From: /config list\n✅ **Restored configuration from `{name}`**\n\n\
                             Active config file updated:\n  `{}`",
                            target.display()
                        ));
                        self.push_log_no_agent(
                            LogLevel::Info,
                            format!(
                                "config list: restored {} -> {}",
                                backup.display(),
                                target.display()
                            ),
                        );
                        self.status = "config: restored".to_string();
                    }
                    Err(e) => {
                        let name = backup
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("<unknown>");
                        self.append_assistant_text(&format!(
                            "From: /config list\n❌ **Failed to restore `{name}`:**\n  {e}"
                        ));
                        self.push_log_no_agent(
                            LogLevel::Error,
                            format!("config list: restore failed: {e}"),
                        );
                        self.status = "config: restore error".to_string();
                    }
                }
            }
            _ => {}
        }
    }

    /// Dispatch a mouse event to the appropriate UI region, updating the input
    /// buffer / cursor / scroll / context menu as needed. Asserts UI
    /// invariants and logs the transition for diagnostics.
    pub fn handle_mouse_event(&mut self, event: MouseEvent) {
        let before_input = self.input.clone();
        let before_cursor = self.input_cursor;
        // Self-heal any selection/menu anchored on a panel that a recent
        // toggle (e.g. Alt+T) dismissed — the render pass zeroes those
        // areas, and a stale reference would trip assert_ui_invariants.
        self.prune_stale_selection();
        // If context menu is open, intercept clicks.
        if self.context_menu.is_some() {
            if let MouseEventKind::Down(MouseButton::Left) = event.kind {
                self.handle_context_menu_click(event.column, event.row);
            } else if let MouseEventKind::Down(MouseButton::Right) = event.kind {
                // Second right-click dismisses the menu.
                self.context_menu = None;
            }
            self.assert_ui_invariants();
            self.debug_log_input_transition("mouse-context", &before_input, before_cursor);
            return;
        }

        match event.kind {
            MouseEventKind::ScrollUp => {
                if self.research_view.is_some()
                    && self
                        .research_view_area
                        .contains((event.column, event.row).into())
                {
                    self.scroll_research_view_by(-3);
                } else if self.output_view.is_some()
                    && self
                        .output_view_area
                        .contains((event.column, event.row).into())
                {
                    self.scroll_output_view_by(-3);
                } else if self.show_profile
                    && self.profile_area.contains((event.column, event.row).into())
                {
                    self.profile_scroll_offset = self.profile_scroll_offset.saturating_add(3);                  } else if self.show_log && self.log_area.contains((event.column, event.row).into())
                  {
                      self.log_scroll_offset = self.log_scroll_offset.saturating_add(3);
                  } else if self.show_tasks_panel
                      && self.tasks_area.contains((event.column, event.row).into())
                  {
                      self.tasks_scroll_offset = self.tasks_scroll_offset.saturating_add(3);
                  } else if self.show_memory
                      && self.memory_area.contains((event.column, event.row).into())
                  {
                      self.memory_scroll_offset = self.memory_scroll_offset.saturating_add(3);
                  } else if self.show_telemetry
                      && self.telemetry_area.contains((event.column, event.row).into())
                  {
                      self.telemetry_scroll_offset = self.telemetry_scroll_offset.saturating_add(3);
                  } else if self.message_area.contains((event.column, event.row).into()) {
                      self.scroll_offset = self.scroll_offset.saturating_add(3);
                  }
              }
              MouseEventKind::ScrollDown => {                if self.research_view.is_some()
                    && self
                        .research_view_area
                        .contains((event.column, event.row).into())
                {
                    self.scroll_research_view_by(3);
                } else if self.output_view.is_some()
                    && self
                        .output_view_area
                        .contains((event.column, event.row).into())
                {
                    self.scroll_output_view_by(3);
                } else if self.show_profile
                    && self.profile_area.contains((event.column, event.row).into())
                {
                    self.profile_scroll_offset = self.profile_scroll_offset.saturating_sub(3);                  } else if self.show_log && self.log_area.contains((event.column, event.row).into())
                  {
                      self.log_scroll_offset = self.log_scroll_offset.saturating_sub(3);
                  } else if self.show_tasks_panel
                      && self.tasks_area.contains((event.column, event.row).into())
                  {
                      self.tasks_scroll_offset = self.tasks_scroll_offset.saturating_sub(3);
                  } else if self.show_memory
                      && self.memory_area.contains((event.column, event.row).into())
                  {
                      self.memory_scroll_offset = self.memory_scroll_offset.saturating_sub(3);
                  } else if self.show_telemetry
                      && self.telemetry_area.contains((event.column, event.row).into())
                  {
                      self.telemetry_scroll_offset = self.telemetry_scroll_offset.saturating_sub(3);
                  } else if self.message_area.contains((event.column, event.row).into()) {
                      self.scroll_offset = self.scroll_offset.saturating_sub(3);
                  }
              }
            MouseEventKind::Down(MouseButton::Left) => {
                let pos = (event.column, event.row);
                if self.agents_button_area.contains(pos.into()) {
                    if self.active_tasks.is_empty() && self.bg_tasks.is_empty() {
                        return;
                    }
                    self.show_agents_window = !self.show_agents_window;
                    if self.show_agents_window {
                        self.show_teams_window = false;
                    }
                    return;
                }
                if self.teams_button_area.contains(pos.into()) {
                    if self.active_team.is_none() {
                        return;
                    }
                    self.show_teams_window = !self.show_teams_window;
                    if self.show_teams_window {
                        self.show_agents_window = false;
                    }
                    return;
                }
                if self.agents_close_button_area.contains(pos.into()) {
                    self.show_agents_window = false;
                    return;
                }
                                  if self.teams_close_button_area.contains(pos.into()) {
                                      self.show_teams_window = false;
                                      return;
                                  }
                                  if self.research_view.is_some() {
                    if self.research_view_area.contains(pos.into()) {
                        return;
                    }
                    self.research_view = None;
                    return;
                }
                if self.output_view.is_some()
                    && self
                        .output_view_area
                        .contains((event.column, event.row).into())
                {
                    return;
                }
                if self.output_view.is_some() {
                    self.output_view = None;
                    self.selected_agent_session_id = None;
                    self.selected_agent_index = None;
                }
                                  if self
                                      .active_agents_area
                                      .contains((event.column, event.row).into())
                                  {
                                      let row = event.row.saturating_sub(self.active_agents_area.y);
                                      let absolute_row =
                                          row.saturating_add(self.active_agents_scroll_offset) as usize;

                                                                              // Check button clicks first (Play/Stop and Kill)
                                                                              for (i, area) in self.agent_row_button_areas.iter().enumerate() {
                                                                                  if area.contains((event.column, event.row).into()) {
                                                                                      let task_id = &self.agent_row_button_task_ids[i];
                                                                                      if let Some(task) = self.active_tasks.iter().find(|t| t.id == *task_id).cloned() {
                                                                                          if task.status == ragent_agent::task::TaskStatus::Suspended {
                                                                                              self.resume_agent_task(&task.id);
                                                                                          } else {
                                                                                              self.suspend_agent_task(&task.id);
                                                                                          }
                                                                                      } else if self.bg_tasks.iter().any(|t| t.id == *task_id) {
                                                                                          let id = task_id.clone();
                                                                                          self.cancel_bg_task(&id);
                                                                                      }
                                                                                      return;
                                                                                  }
                                                                              }
                                                                              for (i, area) in self.agent_row_kill_areas.iter().enumerate() {
                                                                                  if area.contains((event.column, event.row).into()) {
                                                                                      let task_id = &self.agent_row_kill_task_ids[i];
                                                                                      if let Some(task) = self.active_tasks.iter().find(|t| t.id == *task_id).cloned() {
                                                                                          self.kill_agent_task(&task.id);
                                                                                      } else if self.bg_tasks.iter().any(|t| t.id == *task_id) {
                                                                                          let id = task_id.clone();
                                                                                          self.cancel_bg_task(&id);
                                                                                      }
                                                                                      return;
                                                                                  }
                                                                                                                                                              }
                                                                                                                      if absolute_row == 1 {                                          if let Some(ref sid) = self.session_id {
                                              self.selected_agent_index = Some(0);
                                              self.open_output_view_session(sid.clone(), "primary".to_string());
                                          }
                                          return;
                                      }
                                      if absolute_row >= 2 {
                                          let idx = absolute_row - 2;
                                          if let Some(task) = self.active_tasks.get(idx).cloned() {
                                              self.selected_agent_index = Some(idx + 1);
                                              self.open_output_view_session(
                                                  task.child_session_id.clone(),
                                                  format!("{} [{}]", task.agent_name, short_session_id(&task.id)),                            );
                        }
                        return;
                    }
                }
                                  if self.teams_area.contains((event.column, event.row).into()) {
                                      let row = event.row.saturating_sub(self.teams_area.y);
                                      let absolute_row = row.saturating_add(self.teams_scroll_offset) as usize;

                                                                                                                                                              // Check button clicks first (Play/Stop and Kill)
                                                                                                                                                              for (i, area) in self.team_row_button_areas.iter().enumerate() {
                                                                                                                                                                  if area.contains((event.column, event.row).into()) {
                                                                                                                                                                      let agent_id = &self.team_row_button_agent_ids[i];
                                                                                                                                                                      if let Some(member) = self.team_members.iter().find(|m| m.agent_id == *agent_id).cloned() {
                                                                                                                                                                          if let Ok(handle) = tokio::runtime::Handle::try_current() {
                                                                                                                                                                              let tm = self.session_processor.team_manager.get().cloned();
                                                                                                                                                                              let id = member.agent_id.clone();
                                                                                                                                                                              let is_suspended = member.status == ragent_team::team::MemberStatus::Suspended;
                                                                                                                                                                              handle.spawn(async move {
                                                                                                                                                                                  if let Some(tm) = tm {
                                                                                                                                                                                      if is_suspended {
                                                                                                                                                                                          let _ = tm.resume_teammate(&id).await;
                                                                                                                                                                                      } else {
                                                                                                                                                                                          let _ = tm.suspend_teammate(&id).await;
                                                                                                                                                                                      }
                                                                                                                                                                                  }
                                                                                                                                                                              });
                                                                                                                                                                          }
                                                                                                                                                                      }
                                                                                                                                                                      return;
                                                                                                                                                                  }
                                                                                                                                                              }
                                                                                                                                                              for (i, area) in self.team_row_kill_areas.iter().enumerate() {
                                                                                                                                                                  if area.contains((event.column, event.row).into()) {
                                                                                                                                                                      let agent_id = &self.team_row_kill_agent_ids[i];
                                                                                                                                                                      if let Some(member) = self.team_members.iter().find(|m| m.agent_id == *agent_id).cloned() {
                                                                                                                                                                          if let Ok(handle) = tokio::runtime::Handle::try_current() {
                                                                                                                                                                              let tm = self.session_processor.team_manager.get().cloned();
                                                                                                                                                                              let id = member.agent_id.clone();
                                                                                                                                                                              handle.spawn(async move {
                                                                                                                                                                                  if let Some(tm) = tm {
                                                                                                                                                                                      let _ = tm.shutdown_teammate(&id, false).await;
                                                                                                                                                                                  }
                                                                                                                                                                              });
                                                                                                                                                                          }
                                                                                                                                                                      }
                                                                                                                                                                      return;
                                                                                                                                                                  }
                                                                                                                                                              }                                      // Account for border line at row 0
                                      if absolute_row == 2 {
                                          // Lead row clicked — unfocus any teammate
                                          self.focused_teammate = None;
                                          self.status = "focus: lead (you)".to_string();
                                          return;
                                      }
                                      if absolute_row >= 3 {
                                          // Teammate rows start at absolute_row 3 (after border, header, lead)
                                          let idx = absolute_row - 3;
                                          if let Some(member) = self.team_members.get(idx).cloned() {
                                              // Focus this teammate (same as /team focus <name>)
                                              self.focus_teammate_by_id(&member.agent_id);
                                          }
                                          return;
                                      }
                                                                      } // Scrollbar drag takes priority (rightmost column of pane)
                                                    if self.message_area.height > 0
                                                        && event.column == self.message_area.right().saturating_sub(1)
                                                        && self.message_area.contains(pos.into())
                                                        && self.message_max_scroll > 0
                                                    {                    self.scrollbar_drag = Some(ScrollbarDragPane::Messages);
                    self.text_selection = None;
                    self.apply_scrollbar_drag(event.row, ScrollbarDragPane::Messages);
                } else if self.show_log
                    && self.log_area.height > 0
                    && event.column == self.log_area.right().saturating_sub(1)
                    && self.log_area.contains(pos.into())
                    && self.log_max_scroll > 0
                {
                    self.scrollbar_drag = Some(ScrollbarDragPane::Log);
                    self.text_selection = None;
                    self.apply_scrollbar_drag(event.row, ScrollbarDragPane::Log);
                } else if self.show_profile
                    && self.profile_area.height > 0
                    && event.column == self.profile_area.right().saturating_sub(1)
                    && self.profile_area.contains(pos.into())
                    && self.profile_max_scroll > 0
                {
                    self.scrollbar_drag = Some(ScrollbarDragPane::Profile);
                    self.text_selection = None;
                    self.apply_scrollbar_drag(event.row, ScrollbarDragPane::Profile);
                } else if self.show_tasks_panel
                    && self.tasks_area.height > 0
                    && event.column == self.tasks_area.right().saturating_sub(1)
                    && self.tasks_area.contains(pos.into())
                    && self.tasks_max_scroll > 0
                {
                    self.scrollbar_drag = Some(ScrollbarDragPane::Tasks);
                    self.text_selection = None;
                    self.apply_scrollbar_drag(event.row, ScrollbarDragPane::Tasks);
                                  } else if self.show_memory
                                      && self.memory_area.height > 0
                                      && event.column == self.memory_area.right().saturating_sub(1)
                                      && self.memory_area.contains(pos.into())
                                      && self.memory_max_scroll > 0
                                  {
                                      // Click on the Memory panel scrollbar gutter (rightmost
                                      // column of `memory_area`) initiates a scrollbar drag so
                                      // the user can jump-scroll the Memory pane the same way as
                                      // the Log / Profile / TODO panels (FR-013 hit-testing).
                                      self.scrollbar_drag = Some(ScrollbarDragPane::Memory);
                                      self.text_selection = None;
                                      self.apply_scrollbar_drag(event.row, ScrollbarDragPane::Memory);
                                  } else if self.show_telemetry
                                      && self.telemetry_area.height > 0
                                      && event.column == self.telemetry_area.right().saturating_sub(1)
                                      && self.telemetry_area.contains(pos.into())
                                      && self.telemetry_max_scroll > 0
                                  {
                                      // Click on the Telemetry panel scrollbar gutter initiates
                                      // a scrollbar drag, mirroring the other side panels.
                                      self.scrollbar_drag = Some(ScrollbarDragPane::Telemetry);
                                      self.text_selection = None;
                                      self.apply_scrollbar_drag(event.row, ScrollbarDragPane::Telemetry);
                                  } else {                    // If the file menu is open and the click falls within its popup,
                    // handle file/directory selection via mouse.
                    if let Some(_menu_state) = self.file_menu.as_ref() {
                        // Compute popup rect used by the renderer so clicks map to rows.
                    }

                    if self.file_menu.is_some() {
                        // Recompute popup geometry identical to layout::render_file_menu
                        let Some(menu) = self.file_menu.as_ref() else {
                            return;
                        };
                        let input_area = self.active_input_widget_area();
                        let item_count = menu.matches.len() as u16;
                        let visible_items = item_count.max(1).min(8);
                        let height = (visible_items + 1 + 2).min(input_area.y);
                        let width = input_area.width.min(60);
                        let popup_x = input_area.x;
                        let popup_y = input_area.y.saturating_sub(height);

                        // If click is inside the popup, determine which row was clicked.
                        if event.column >= popup_x
                            && event.column < popup_x.saturating_add(width)
                            && event.row >= popup_y
                            && event.row < popup_y.saturating_add(height)
                        {
                            // Content lines start one row below the popup top (inside the border)
                            let clicked_row = event.row.saturating_sub(popup_y + 1) as usize;
                            let absolute_row = menu.scroll_offset + clicked_row;
                            if absolute_row < menu.matches.len() {
                                // Set the selected index (drop borrow immediately)
                                {
                                    if let Some(ref mut m) = self.file_menu.as_mut() {
                                        m.selected = absolute_row;
                                    }
                                }

                                // Accept the selection: this will navigate into directories
                                // or insert a file path into the input. We do not auto-send
                                // the message on mouse click; pressing Enter still sends.
                                let _ = self.accept_file_menu_selection();
                                return;
                            }
                        } else {
                            // Click outside popup dismisses the file menu.
                            self.file_menu = None;
                            return;
                        }
                    }

                    // Start text selection in whichever pane the click is in
                    let pane = self.pane_at(event.column, event.row);
                    if let Some(pane) = pane {
                        self.text_selection = Some(TextSelection {
                            pane,
                            anchor: pos,
                            endpoint: pos,
                        });
                    } else {
                        self.text_selection = None;
                    }
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                if let Some(pane) = self.scrollbar_drag {
                    self.apply_scrollbar_drag(event.row, pane);
                } else if let Some(ref mut sel) = self.text_selection {
                    sel.endpoint = (event.column, event.row);
                }
            }

            // Mouse move -> used for hover highlighting of file menu entries
            MouseEventKind::Moved
                // If file menu is open, update the highlighted row under the cursor.
                if self.file_menu.is_some() => {
                    // Snapshot needed values without holding immutable borrows while mutating.
                    let input_area = self.active_input_widget_area();
                    let item_count = self
                        .file_menu
                        .as_ref()
                        .map(|m| m.matches.len())
                        .unwrap_or(0) as u16;
                    let visible_items = item_count.max(1).min(8);
                    let height = (visible_items + 1 + 2).min(input_area.y);
                    let width = input_area.width.min(60);
                    let popup_x = input_area.x;
                    let popup_y = input_area.y.saturating_sub(height);

                    if event.column >= popup_x
                        && event.column < popup_x.saturating_add(width)
                        && event.row >= popup_y
                        && event.row < popup_y.saturating_add(height)
                    {
                        let hovered_row = event.row.saturating_sub(popup_y + 1) as usize;
                        let absolute_row = self
                            .file_menu
                            .as_ref()
                            .map(|m| m.scroll_offset)
                            .unwrap_or(0)
                            + hovered_row;
                        if absolute_row < (item_count as usize) {
                            // Update selection if changed.
                            if let Some(ref mut m) = self.file_menu.as_mut() {
                                if m.selected != absolute_row {
                                    m.selected = absolute_row;
                                }
                            }
                        }
                    }
                }

            MouseEventKind::Up(MouseButton::Left) => {
                self.scrollbar_drag = None;
                // Keep text_selection alive so it stays highlighted until right-click or next click
            }
            MouseEventKind::Down(MouseButton::Right) => {
                // Right-click contract: always open context menu when inside a pane.
                // Actions are enabled only when valid for that pane + selection context.
                let col = event.column;
                let row = event.row;
                let Some(pane) = self.pane_at(col, row) else {
                    self.context_menu = None;
                    return;
                };

                let selection_for_pane =
                    self.text_selection.as_ref().is_some_and(|s| s.pane == pane);
                let in_input = matches!(pane, SelectionPane::Input);
                let has_clipboard = Self::get_clipboard().is_some_and(|s| !s.is_empty());
                                  let provider_setup_input = matches!(
                                      self.provider_setup,
                                      Some(ProviderSetupStep::EnterKey { .. })
                                          | Some(ProviderSetupStep::GitLabSetup { .. })
                                          | Some(ProviderSetupStep::TelemetrySetup { .. })
                                  );
                let items = vec![
                    (ContextAction::Cut, selection_for_pane && in_input),
                    (ContextAction::Copy, selection_for_pane),
                    (
                        ContextAction::Paste,
                        if provider_setup_input {
                            true
                        } else {
                            in_input && has_clipboard
                        },
                    ),
                ];
                let selected = items.iter().position(|(_, en)| *en).unwrap_or(0);

                self.context_menu = Some(ContextMenuState {
                    x: col,
                    y: row,
                    pane,
                    selected,
                    items,
                });
            }
            _ => {}
        }
        self.assert_ui_invariants();
        self.debug_log_input_transition("mouse", &before_input, before_cursor);
    }

    pub(crate) fn copy_selection(&mut self, consume_selection: bool) {
        let sel = match self.text_selection.clone() {
            Some(s) => s,
            None => return,
        };
        if consume_selection {
            self.text_selection = None;
        }
        let ((start_col, mut start_row), (end_col, mut end_row)) = sel.normalized();

        // Account for pane scroll: the visible viewport starts at a scroll
        // offset, not at content line 0. Without this adjustment, selecting
        // text after scrolling copies from the wrong part of the buffer.
        let scroll_top = match sel.pane {
            SelectionPane::Messages => self.message_max_scroll.saturating_sub(self.scroll_offset),
            SelectionPane::Log => self.log_max_scroll.saturating_sub(self.log_scroll_offset),
            SelectionPane::Profile => self
                .profile_max_scroll
                .saturating_sub(self.profile_scroll_offset),
            SelectionPane::Tasks => self
                .tasks_max_scroll
                .saturating_sub(self.tasks_scroll_offset),
            SelectionPane::Memory => self
                .memory_max_scroll
                .saturating_sub(self.memory_scroll_offset),
            SelectionPane::Telemetry => self
                .telemetry_max_scroll
                .saturating_sub(self.telemetry_scroll_offset),
            _ => 0,
        };
        start_row = start_row.saturating_add(scroll_top);
        end_row = end_row.saturating_add(scroll_top);

        let lines: &[String] = match sel.pane {
            SelectionPane::Messages => &self.message_content_lines,
            SelectionPane::Log => &self.log_content_lines,
            SelectionPane::Profile => &self.profile_content_lines,
            SelectionPane::Tasks => &self.tasks_content_lines,
            SelectionPane::Memory => &self.memory_content_lines,
            SelectionPane::Telemetry => &self.telemetry_content_lines,
            SelectionPane::Input => {
                // For input widgets, build a single-line content from app.input
                let input_text = format!("> {}", self.input);
                let area = self.input_area;
                let inner_x = area.x + 1; // inside border
                let inner_y = area.y + 1;
                let inner_w = area.width.saturating_sub(2).max(1) as usize;
                // Wrap the input text into display lines (character-width based).
                let chars: Vec<char> = input_text.chars().collect();
                let mut wrapped: Vec<String> = Vec::new();
                let mut start = 0usize;
                while start < chars.len() {
                    let end = (start + inner_w).min(chars.len());
                    wrapped.push(chars[start..end].iter().collect::<String>());
                    start = end;
                }
                if wrapped.is_empty() {
                    wrapped.push(String::new());
                }
                let text = Self::extract_text_from_lines(
                    &wrapped, inner_x, inner_y, start_col, start_row, end_col, end_row,
                );
                if !text.is_empty() {
                    Self::set_clipboard(&text);
                    self.push_log_no_agent(LogLevel::Info, format!("Copied {} chars", text.len()));
                }
                return;
            }
        };

        let area = match sel.pane {
            SelectionPane::Messages => self.message_area,
            SelectionPane::Log => self.log_area,
            SelectionPane::Profile => self.profile_area,
            SelectionPane::Tasks => self.tasks_area,
            SelectionPane::Memory => self.memory_area,
            SelectionPane::Telemetry => self.telemetry_area,
            _ => unreachable!(),
        };
        // Inner area (accounting for borders)
        let inner_x = if sel.pane == SelectionPane::Messages {
            area.x + 1 // LEFT border only
        } else {
            area.x + 1 // ALL borders
        };
        let inner_y = if sel.pane == SelectionPane::Messages {
            area.y + 1 // Messages pane has a top border
        } else {
            area.y + 1 // ALL borders on side panels
        };

        let text = Self::extract_text_from_lines(
            lines, inner_x, inner_y, start_col, start_row, end_col, end_row,
        );
        if !text.is_empty() {
            Self::set_clipboard(&text);
            self.push_log_no_agent(LogLevel::Info, format!("Copied {} chars", text.len()));
        }
    }

    pub(crate) fn handle_context_menu_click(&mut self, col: u16, row: u16) {
        if let Some(menu) = self.context_menu.clone() {
            // Menu geometry: x, y is top-left; rows are y+1..y+1+items.len()
            let menu_x = menu.x;
            let menu_y = menu.y;
            let menu_w = 12u16; // matches render_context_menu width
            let item_count = menu.items.len() as u16;
            let menu_h = item_count + 2; // border top + items + border bottom

            if col >= menu_x && col < menu_x + menu_w && row >= menu_y && row < menu_y + menu_h {
                // Row inside border
                if row > menu_y && row < menu_y + menu_h - 1 {
                    let item_idx = (row - menu_y - 1) as usize;
                    if item_idx < menu.items.len() {
                        let (action, enabled) = menu.items[item_idx];
                        if enabled {
                            self.execute_context_action(action);
                        } else {
                            self.context_menu = None;
                        }
                    }
                }
            } else {
                // Click outside menu dismisses it.
                self.context_menu = None;
            }
        }
    }

    /// Dispatch a key event to the active UI region (history picker, agent /
    /// teams dialog, slash menu, context menu, or the main input editor).
    /// Asserts UI invariants and logs the transition for diagnostics.
    pub fn handle_key_event(&mut self, key: KeyEvent) {
        let before_input = self.input.clone();
        let before_cursor = self.input_cursor;
        // Self-heal any selection/menu anchored on a panel that a recent
        // toggle dismissed (the render pass may have zeroed its area since
        // the last input cycle).
        self.prune_stale_selection();
        // Dismiss the transient run-cost banner on any keypress (FR-012).
        // Non-character keys (Esc, arrows, Enter, modifier-only, etc.) are
        // consumed solely to clear the banner.  A plain printable character,
        // however, is the user starting to type their next message — we clear
        // the banner but let the character fall through to normal input
        // processing so the first keystroke is not lost.
        if self.run_cost_banner.take().is_some() {
            self.run_cost_banner_at = None;
            self.needs_redraw = true;
            let is_plain_char = matches!(key.code, KeyCode::Char(_))
                && !key.modifiers.contains(KeyModifiers::CONTROL)
                && !key.modifiers.contains(KeyModifiers::ALT);
            if !is_plain_char {
                return;
            }
        }
        // Config-save picker intercepts all keys while it is open.
        if self.config_save_picker.is_some() {
            self.handle_config_save_picker_key(key);
            self.assert_ui_invariants();
            self.debug_log_input_transition("key-config-save-picker", &before_input, before_cursor);
            return;
        }
        // History picker intercepts all keys while it is open
        if self.history_picker.is_some() {
            self.handle_history_picker_key(key);
            self.assert_ui_invariants();
            self.debug_log_input_transition("key-history-picker", &before_input, before_cursor);
            return;
        }
        // Agent / Teams dialog keyboard shortcuts
        if self.show_agents_window {
            if key.code == KeyCode::Char(' ') {
                // Space = Play/Stop toggle on selected agent row
                if let Some(idx) = self.selected_agent_index {
                    if idx == 0 {
                        self.status = "Cannot suspend primary agent".to_string();
                    } else if let Some(task) = self.active_tasks.get(idx - 1).cloned() {
                        if task.status == ragent_agent::task::TaskStatus::Suspended {
                            self.resume_agent_task(&task.id);
                        } else {
                            self.suspend_agent_task(&task.id);
                        }
                    }
                }
                return;
            }
            if key.code == KeyCode::Char('K') && key.modifiers.contains(KeyModifiers::SHIFT) {
                if let Some(idx) = self.selected_agent_index {
                    if idx == 0 {
                        self.status = "Cannot kill primary agent".to_string();
                    } else if let Some(task) = self.active_tasks.get(idx - 1).cloned() {
                        self.kill_agent_task(&task.id);
                    }
                }
                return;
            }
        }
        if self.show_teams_window {
            if key.code == KeyCode::Char(' ') {
                if let Some(ref focused) = self.focused_teammate {
                    // For teams, space on a focused teammate triggers shutdown
                    if let Ok(handle) = tokio::runtime::Handle::try_current() {
                        let tm = self.session_processor.team_manager.get().cloned();
                        let id = focused.clone();
                        handle.spawn(async move {
                            if let Some(tm) = tm {
                                let _ = tm.shutdown_teammate(&id, false).await;
                            }
                        });
                    }
                }
                return;
            }
            if key.code == KeyCode::Char('K') && key.modifiers.contains(KeyModifiers::SHIFT) {
                if let Some(ref focused) = self.focused_teammate {
                    if let Ok(handle) = tokio::runtime::Handle::try_current() {
                        let tm = self.session_processor.team_manager.get().cloned();
                        let id = focused.clone();
                        handle.spawn(async move {
                            if let Some(tm) = tm {
                                let _ = tm.shutdown_teammate(&id, false).await;
                            }
                        });
                    }
                    return;
                }
            }
        }

        if let Some(action) = input::handle_key(self, key) {
            match action {
                InputAction::BangCommand(text) => {
                    // Create session if needed, then run the command.
                    if self.session_id.is_none() {
                        let dir = std::env::current_dir().unwrap_or_default();
                        match self.session_processor.session_manager.create_session(dir) {
                            Ok(session) => {
                                self.session_id = Some(session.id.clone());
                                let short_sid = short_session_id(&session.id);
                                self.sid_to_display_name
                                    .insert(short_sid, self.agent_name.clone());
                            }
                            Err(e) => {
                                self.status = format!("error: {e}");
                                return;
                            }
                        }
                    }
                    self.dispatch_bang_command(text);
                }
                InputAction::SendMessage(text) => {
                    // When a teammate is focused, route the message to their
                    // mailbox instead of the lead session.
                    if let Some(ref focused_id) = self.focused_teammate.clone() {
                        if let Some(member) = self
                            .team_members
                            .iter()
                            .find(|m| m.agent_id == *focused_id)
                            .cloned()
                        {
                            let team_name = self
                                .active_team
                                .as_ref()
                                .map(|t| t.name.clone())
                                .unwrap_or_default();
                            self.send_teammate_message(&team_name, &member.name, &text);
                            self.input.clear();
                            self.input_cursor = 0;
                            self.history_index = None;
                            self.push_log_no_agent(
                                LogLevel::Info,
                                format!(
                                    "→ {} (focused): {}",
                                    member.name,
                                    &text[..text.len().min(60)]
                                ),
                            );
                            return;
                        }
                    }
                    // Block sending if no provider/model is configured
                    if self.configured_provider.is_none() {
                        self.status =
                            "⚠ No provider configured — use /provider to set up".to_string();
                        return;
                    }
                    if self.selected_model.is_none() {
                        self.status = "⚠ No model selected — use /model to choose".to_string();
                        return;
                    }
                    // Create session if needed
                    if self.session_id.is_none() {
                        let dir = std::env::current_dir().unwrap_or_default();
                        match self.session_processor.session_manager.create_session(dir) {
                            Ok(session) => {
                                self.session_id = Some(session.id.clone());
                                // Map the primary session's short_sid to the current agent name
                                let short_sid = short_session_id(&session.id);
                                self.sid_to_display_name
                                    .insert(short_sid, self.agent_name.clone());
                            }
                            Err(e) => {
                                self.status = format!("error: {}", e);
                                return;
                            }
                        }
                    }

                    // Take image attachments once; either queue for auto-compaction
                    // or send immediately.
                    let image_paths: Vec<std::path::PathBuf> =
                        std::mem::take(&mut self.pending_attachments);
                    if self.should_auto_compact_before_send() {
                        self.pending_send_after_compact = Some((text, image_paths));
                        if !self.start_compaction(true) {
                            // If compaction could not start, fall back to direct send.
                            if let Some((queued_text, queued_images)) =
                                self.pending_send_after_compact.take()
                            {
                                self.dispatch_user_message(queued_text, queued_images);
                            }
                        }
                        return;
                    }
                    self.dispatch_user_message(text, image_paths);
                }
                InputAction::Quit => {
                    self.quit_armed = true;
                    self.status = "Press Ctrl+D to quit (or use /quit or /exit)".to_string();
                }
                InputAction::ConfirmQuit => {
                    if self.quit_armed {
                        self.is_running = false;
                    } else {
                        self.status = "Press Ctrl+C first, then Ctrl+D to quit".to_string();
                    }
                }
                InputAction::ScrollUp => {
                    self.scroll_offset = self.scroll_offset.saturating_add(3);
                }
                InputAction::ScrollDown => {
                    self.scroll_offset = self.scroll_offset.saturating_sub(3);
                }
                InputAction::LogScrollUp => {
                    if self.show_log {
                        self.log_scroll_offset = self.log_scroll_offset.saturating_add(3);
                    } else if self.show_profile {
                        self.profile_scroll_offset = self.profile_scroll_offset.saturating_add(3);
                    } else if self.show_tasks_panel {
                        self.tasks_scroll_offset = self.tasks_scroll_offset.saturating_add(3);
                    } else if self.show_memory {
                        // Memory panel shares the LogScrollUp / LogScrollDown
                        // key bindings with the other side panels (FR-009).
                        self.memory_scroll_offset = self.memory_scroll_offset.saturating_add(3);
                    } else if self.show_telemetry {
                        self.telemetry_scroll_offset =
                            self.telemetry_scroll_offset.saturating_add(3);
                    }
                }
                InputAction::LogScrollDown => {
                    if self.show_log {
                        self.log_scroll_offset = self.log_scroll_offset.saturating_sub(3);
                    } else if self.show_profile {
                        self.profile_scroll_offset = self.profile_scroll_offset.saturating_sub(3);
                    } else if self.show_tasks_panel {
                        self.tasks_scroll_offset = self.tasks_scroll_offset.saturating_sub(3);
                    } else if self.show_memory {
                        self.memory_scroll_offset = self.memory_scroll_offset.saturating_sub(3);
                    } else if self.show_telemetry {
                        self.telemetry_scroll_offset =
                            self.telemetry_scroll_offset.saturating_sub(3);
                    }
                }
                InputAction::ToggleLog => {
                    self.show_log = !self.show_log;
                    if self.show_log {
                        // Entering log mode: dismiss the other side panels so
                        // only one occupies the side column (FR-012).
                        self.show_profile = false;
                        self.show_tasks_panel = false;
                        self.show_memory = false;
                        self.show_telemetry = false;
                        self.spool_log_window_history();
                    } else {
                        if self
                            .text_selection
                            .as_ref()
                            .is_some_and(|s| s.pane == SelectionPane::Log)
                        {
                            self.text_selection = None;
                        }
                        if self
                            .context_menu
                            .as_ref()
                            .is_some_and(|m| m.pane == SelectionPane::Log)
                        {
                            self.context_menu = None;
                        }
                    }
                    self.status = if self.show_log {
                        "log panel visible".to_string()
                    } else {
                        "log panel hidden".to_string()
                    };
                }
                InputAction::ToggleProfile => {
                    let enabled = !self.show_profile;
                    self.set_profile_panel_enabled(enabled);
                    if !enabled {
                        if self
                            .text_selection
                            .as_ref()
                            .is_some_and(|s| s.pane == SelectionPane::Profile)
                        {
                            self.text_selection = None;
                        }
                        if self
                            .context_menu
                            .as_ref()
                            .is_some_and(|m| m.pane == SelectionPane::Profile)
                        {
                            self.context_menu = None;
                        }
                    }
                }
                InputAction::ToggleYolo => {
                    match ragent_config::yolo::toggle_persist() {
                        Ok(enabled) => {
                            self.status = if enabled {
                                "YOLO mode enabled".to_string()
                            } else {
                                "YOLO mode disabled".to_string()
                            };
                            self.push_log_no_agent(
                                if enabled {
                                    LogLevel::Warn
                                } else {
                                    LogLevel::Info
                                },
                                format!(
                                    "YOLO mode {}",
                                    if enabled { "enabled" } else { "disabled" }
                                ),
                            );
                        }
                        Err(e) => {
                            self.status = format!("⚠ failed to persist YOLO mode: {e}");
                            self.push_log_no_agent(
                                LogLevel::Error,
                                format!("YOLO persist failed: {e}"),
                            );
                        }
                    }
                    self.needs_redraw = true;
                }
                InputAction::ToggleTasksPanel => {
                    // Toggle the TASKS side panel visibility (Alt+T). Implements
                    // FR-002 (toggle) and FR-003 (mutual exclusion of side
                    // panels — only one of log/profile/tasks is visible at a
                    // time, matching the `/log` and `/profile` slash commands
                    // in app/slash.rs). On hide, any active Tasks-pane text
                    // selection or context menu is cleared.
                    self.show_tasks_panel = !self.show_tasks_panel;
                    if self.show_tasks_panel {
                        // Entering TASKS mode: dismiss the other side panels so
                        // only one occupies the side column (FR-012/SPEC
                        // mutual-exclusion policy). Force a cache refresh on the
                        // next render so the panel is always up-to-date when
                        // first shown.
                        self.tasks_cache_dirty = true;
                        self.show_log = false;
                        self.show_profile = false;
                        self.show_memory = false;
                        self.show_telemetry = false;
                    } else {
                        // Leaving TODO mode: clear Todo-pane selection state.
                        if self
                            .text_selection
                            .as_ref()
                            .is_some_and(|s| s.pane == SelectionPane::Tasks)
                        {
                            self.text_selection = None;
                        }
                        if self
                            .context_menu
                            .as_ref()
                            .is_some_and(|m| m.pane == SelectionPane::Tasks)
                        {
                            self.context_menu = None;
                        }
                    }
                    self.status = if self.show_tasks_panel {
                        "tasks panel visible".to_string()
                    } else {
                        "tasks panel hidden".to_string()
                    };
                    self.needs_redraw = true;
                }
                // Alt+M toggles the Memory side panel (FR-003). Implements
                // mutual exclusion with the log / profile / TODO panels
                // (FR-004): when the Memory panel becomes visible the other
                // side panels are dismissed so only one occupies the side
                // column. On hide, any active Memory-pane text selection or
                // context menu is cleared (FR-005). A status-bar message is
                // set describing the new state (FR-014).
                InputAction::ToggleMemory => {
                    self.show_memory = !self.show_memory;
                    if self.show_memory {
                        // Entering Memory mode: dismiss the other side panels
                        // so only one occupies the side column (FR-004).
                        self.show_log = false;
                        self.show_profile = false;
                        self.show_tasks_panel = false;
                        self.show_telemetry = false;
                    } else {
                        if self
                            .text_selection
                            .as_ref()
                            .is_some_and(|s| s.pane == SelectionPane::Memory)
                        {
                            self.text_selection = None;
                        }
                        if self
                            .context_menu
                            .as_ref()
                            .is_some_and(|m| m.pane == SelectionPane::Memory)
                        {
                            self.context_menu = None;
                        }
                    }
                    self.status = if self.show_memory {
                        "memory panel visible".to_string()
                    } else {
                        "memory panel hidden".to_string()
                    };
                    self.needs_redraw = true;
                }
                InputAction::ToggleTelemetry => {
                    // Toggle the Telemetry side panel visibility (Alt+O). Mirrors
                    // the TODO panel toggle: mutually exclusive with the other side
                    // panels so only one occupies the side column. On hide, clear any
                    // active telemetry-pane text selection or context menu.
                    self.show_telemetry = !self.show_telemetry;
                    if self.show_telemetry {
                        self.show_log = false;
                        self.show_profile = false;
                        self.show_tasks_panel = false;
                        self.show_memory = false;
                    } else {
                        if self
                            .text_selection
                            .as_ref()
                            .is_some_and(|s| s.pane == SelectionPane::Telemetry)
                        {
                            self.text_selection = None;
                        }
                        if self
                            .context_menu
                            .as_ref()
                            .is_some_and(|m| m.pane == SelectionPane::Telemetry)
                        {
                            self.context_menu = None;
                        }
                    }
                    self.status = if self.show_telemetry {
                        "telemetry panel visible".to_string()
                    } else {
                        "telemetry panel hidden".to_string()
                    };
                    self.needs_redraw = true;
                }
                InputAction::ToggleEditLog => {
                    match ragent_config::edit_log::toggle_persist() {
                        Ok(enabled) => {
                            self.status = if enabled {
                                "Edit log enabled".to_string()
                            } else {
                                "Edit log disabled".to_string()
                            };
                            self.push_log_no_agent(
                                LogLevel::Info,
                                format!(
                                    "Edit log {}",
                                    if enabled { "enabled" } else { "disabled" }
                                ),
                            );
                        }
                        Err(e) => {
                            self.status = format!("⚠ failed to persist edit log state: {e}");
                            self.push_log_no_agent(
                                LogLevel::Error,
                                format!("Edit log persist failed: {e}"),
                            );
                        }
                    }
                    self.needs_redraw = true;
                }
                InputAction::OutputViewPageUp => {
                    self.scroll_output_view_by(-5);
                }
                InputAction::OutputViewPageDown => {
                    self.scroll_output_view_by(5);
                }
                InputAction::OutputViewToStart => {
                    self.jump_output_view_start();
                }
                InputAction::OutputViewToEnd => {
                    self.jump_output_view_end();
                }
                InputAction::ResearchViewPageUp => {
                    self.scroll_research_view_by(-5);
                }
                InputAction::ResearchViewPageDown => {
                    self.scroll_research_view_by(5);
                }
                InputAction::ResearchViewToStart => {
                    self.jump_research_view_start();
                }
                InputAction::ResearchViewToEnd => {
                    self.jump_research_view_end();
                }
                InputAction::ResearchViewLineUp => {
                    self.scroll_research_view_by(-1);
                }
                InputAction::ResearchViewLineDown => {
                    self.scroll_research_view_by(1);
                }
                InputAction::HistoryUp => {
                    // Within a multiline input, Up moves to the previous logical line.
                    // Only navigate history when already on the first logical line.
                    if self.history_index.is_none() && !self.cursor_on_first_logical_line() {
                        self.cursor_move_up_logical_line();
                        return;
                    }
                    if self.input_history.is_empty() {
                        return;
                    }
                    match self.history_index {
                        None => {
                            self.history_draft = self.input.clone();
                            let idx = self.input_history.len() - 1;
                            self.history_index = Some(idx);
                            self.input = self.input_history[idx].clone();
                        }
                        Some(idx) if idx > 0 => {
                            let idx = idx - 1;
                            self.history_index = Some(idx);
                            self.input = self.input_history[idx].clone();
                        }
                        _ => {}
                    }
                    self.input_cursor = self.input_len_chars();
                }
                InputAction::HistoryDown => {
                    // Within a multiline input (while not browsing history), Down moves
                    // to the next logical line before navigating history.
                    if self.history_index.is_none() && !self.cursor_on_last_logical_line() {
                        self.cursor_move_down_logical_line();
                        return;
                    }
                    match self.history_index {
                        Some(idx) if idx + 1 < self.input_history.len() => {
                            let idx = idx + 1;
                            self.history_index = Some(idx);
                            self.input = self.input_history[idx].clone();
                            self.input_cursor = self.input_len_chars();
                        }
                        Some(_) => {
                            self.history_index = None;
                            self.input = self.history_draft.clone();
                            self.history_draft.clear();
                            self.input_cursor = self.input_len_chars();
                        }
                        None => {}
                    }
                }
                InputAction::MoveCursorLeft => {
                    // Standard: if selection active, jump to left boundary and clear.
                    if let Some((start, _)) = self.kb_selection_char_range() {
                        self.kb_select_anchor = None;
                        self.set_cursor_char_index_clamped(start);
                    } else {
                        self.cursor_move_left();
                    }
                }
                InputAction::MoveCursorRight => {
                    // Standard: if selection active, jump to right boundary and clear.
                    if let Some((_, end)) = self.kb_selection_char_range() {
                        self.kb_select_anchor = None;
                        self.set_cursor_char_index_clamped(end);
                    } else {
                        self.cursor_move_right();
                    }
                }
                InputAction::MoveCursorWordLeft => {
                    self.clear_kb_selection();
                    self.cursor_move_word_left();
                }
                InputAction::MoveCursorWordRight => {
                    self.clear_kb_selection();
                    self.cursor_move_word_right();
                }
                InputAction::MoveCursorHome => {
                    self.clear_kb_selection();
                    self.cursor_move_home();
                }
                InputAction::MoveCursorEnd => {
                    self.clear_kb_selection();
                    self.cursor_move_end();
                }
                InputAction::Delete => {
                    if let Some((start, end)) = self.kb_selection_char_range() {
                        self.remove_input_char_range(start, end);
                        self.kb_select_anchor = None;
                    } else {
                        self.delete_next_char();
                    }
                }
                InputAction::DeletePrevWord => {
                    self.clear_kb_selection();
                    self.delete_prev_word();
                }
                InputAction::DeleteToLineEnd => {
                    self.clear_kb_selection();
                    self.delete_to_end_of_line();
                }
                InputAction::SelectAll => {
                    self.kb_select_anchor = Some(0);
                    self.cursor_move_end();
                }
                InputAction::SelectCharLeft => {
                    if self.kb_select_anchor.is_none() {
                        self.kb_select_anchor = Some(self.input_cursor);
                    }
                    self.cursor_move_left();
                }
                InputAction::SelectCharRight => {
                    if self.kb_select_anchor.is_none() {
                        self.kb_select_anchor = Some(self.input_cursor);
                    }
                    self.cursor_move_right();
                }
                InputAction::SelectWordLeft => {
                    if self.kb_select_anchor.is_none() {
                        self.kb_select_anchor = Some(self.input_cursor);
                    }
                    self.cursor_move_word_left();
                }
                InputAction::SelectWordRight => {
                    if self.kb_select_anchor.is_none() {
                        self.kb_select_anchor = Some(self.input_cursor);
                    }
                    self.cursor_move_word_right();
                }
                InputAction::CopyToClipboard => {
                    self.copy_kb_selection();
                }
                InputAction::CutToClipboard => {
                    self.cut_kb_selection();
                }
                InputAction::PasteFromClipboard => {
                    self.paste_text_from_clipboard();
                }
                InputAction::SwitchAgent => {
                    if self.cycleable_agents.len() > 1 {
                        let prev = self.agent_name.clone();
                        self.current_agent_index =
                            (self.current_agent_index + 1) % self.cycleable_agents.len();
                        self.agent_info = self.cycleable_agents[self.current_agent_index].clone();
                        self.agent_name = self.agent_info.name.clone();
                        self.status = format!("agent: {}", self.agent_name);
                        self.push_log_no_agent(
                            LogLevel::Info,
                            format!(
                                "Switched to: {} ({})",
                                self.agent_name, self.agent_info.description
                            ),
                        );

                        if let Some(ref sid) = self.session_id {
                            self.event_bus.publish(Event::AgentSwitched {
                                session_id: sid.clone(),
                                from: prev,
                                to: self.agent_name.clone(),
                            });
                        }
                    }
                }
                InputAction::SlashCommand(cmd) => {
                    self.execute_slash_command(&cmd);
                }
                InputAction::CancelAgent => {
                    if let Some(ref flag) = self.cancel_flag {
                        flag.store(true, Ordering::Relaxed);
                        self.status = "halting agent…".to_string();
                        self.push_log_no_agent(
                            LogLevel::Warn,
                            "User pressed Esc or Ctrl+X — halting agent".to_string(),
                        );
                    }
                }
                InputAction::ConfirmForceCleanup => {
                    if self.pending_forcecleanup.is_some() {
                        // Clear pending modal state and invoke forcecleanup with confirm arg.
                        self.pending_forcecleanup = None;
                        self.execute_slash_command("/team forcecleanup confirm");
                    }
                }
                InputAction::CancelForceCleanup => {
                    if self.pending_forcecleanup.is_some() {
                        self.pending_forcecleanup = None;
                        self.append_assistant_text(
                            "From: /team forcecleanup\nForce-cleanup cancelled.",
                        );
                        self.push_log_no_agent(
                            LogLevel::Info,
                            "forcecleanup cancelled".to_string(),
                        );
                        self.status = "forcecleanup cancelled".to_string();
                    }
                }
                InputAction::ConfirmRouterSave => {
                    if let Some(draft) = self.pending_router_save.take() {
                        match self.save_router_config(&draft) {
                            Ok(()) => {
                                self.select_router_as_active();
                                self.router_enabled = true;
                                self.status =
                                    "✓ Router cluster saved — Model Router active".to_string();
                                self.provider_setup = None;
                            }
                            Err(e) => {
                                self.status = format!("error: {e}");
                                self.push_log_no_agent(LogLevel::Error, e.clone());
                                if let Some(ProviderSetupStep::SetupRouter { error, .. }) =
                                    self.provider_setup.as_mut()
                                {
                                    *error = Some(e);
                                }
                            }
                        }
                    }
                }
                InputAction::CancelRouterSave => {
                    if self.pending_router_save.take().is_some() {
                        self.status = "Router save cancelled".to_string();
                    }
                }
                InputAction::ApprovePlan => {
                    if let Some(state) = self.plan_approval_pending.take() {
                        if let Some(ref session_id) = self.session_id.clone() {
                            self.push_log_no_agent(LogLevel::Info, "plan approved".to_string());
                            self.execute_plan_restore(session_id, &state.plan_text);
                        }
                    }
                }
                InputAction::RejectPlan => {
                    if let Some(state) = self.plan_approval_pending.take() {
                        if let Some(ref session_id) = self.session_id.clone() {
                            self.push_log_no_agent(
                                LogLevel::Info,
                                "plan rejected — re-delegating".to_string(),
                            );
                            self.append_assistant_text(
                                "From: /plan\n🔄 **Plan rejected** — re-delegating to plan agent for revision.\n",
                            );
                            self.execute_plan_delegation(
                                session_id,
                                "Revise the plan based on this feedback: please provide an improved plan".to_string(),
                                state.plan_text,
                            );
                        }
                    }
                }
                InputAction::TogglePlanCursor => {
                    if let Some(ref mut state) = self.plan_approval_pending {
                        state.cursor_approve = !state.cursor_approve;
                    }
                }
                InputAction::FocusNextTeammate => {
                    self.cycle_focused_teammate(true);
                }
                InputAction::FocusPrevTeammate => {
                    self.cycle_focused_teammate(false);
                }
                InputAction::InsertNewline => {
                    self.insert_char_at_cursor('\n');
                }
                InputAction::ClearLine => {
                    self.input.clear();
                    self.input_cursor = 0;
                    self.kb_select_anchor = None;
                }
            }
        }
        self.assert_ui_invariants();
        self.debug_log_input_transition("key", &before_input, before_cursor);
    }
}
