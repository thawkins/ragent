//! Swarm decomposition handling for the TUI.



use ragent_team::team::{
    self, MemberStatus,
};


// State types from app/state.rs
use crate::app::state::{
    LogLevel, App
};

// Helpers

// Re-export status types from theme

impl App {
    pub(crate) fn execute_swarm_decomposition(&mut self, decomposition: team::SwarmDecomposition) {
        use ragent_team::team::{SwarmState, TaskStore, TeamStore, task::Task};

        let task_count = decomposition.tasks.len();
        if task_count == 0 {
            self.status = "⚠ swarm: LLM returned 0 subtasks".to_string();
            self.append_assistant_text(
                "From: /swarm\n## ⚠ No subtasks\n\nThe LLM returned an empty task list.\n",
            );
            return;
        }

        // Create ephemeral team name
        let ts = chrono::Utc::now().format("%Y%m%d-%H%M%S");
        let team_name = format!("swarm-{ts}");
        let working_dir = std::env::current_dir().unwrap_or_default();
        let lead_sid = self
            .session_id
            .clone()
            .unwrap_or_else(|| "unknown".to_string());

        // Create the team
        match TeamStore::create(&team_name, &lead_sid, &working_dir, true) {
            Ok(store) => {
                // Seed tasks into tasks.json
                if let Ok(task_store) = TaskStore::open(&store.dir) {
                    for st in &decomposition.tasks {
                        let mut task = Task::new(&st.id, &st.title);
                        task.description = st.description.clone();
                        task.depends_on = st.depends_on.clone();
                        if let Err(e) = task_store.add_task(task) {
                            self.push_log_no_agent(
                                LogLevel::Warn,
                                format!("Swarm: failed to add task {}: {e}", st.id),
                            );
                        }
                    }
                }

                // Set up active team state
                self.active_team = Some(store.config.clone());
                self.team_members.clear();
                self.show_teams = true;
                self.ensure_team_manager_for_team(&team_name, Some(store.dir.clone()));

                // Build display table
                let mut output = format!(
                    "From: /swarm\n## 🐝 Swarm Created: {team_name}\n\n\
                                      **{task_count} subtasks** decomposed and seeded.\n\n\
                                      | ID | Title | Agent Type | Dependencies |\n\
                                      |----|-------|------------|--------------|\n"
                );
                for st in &decomposition.tasks {
                    let deps = if st.depends_on.is_empty() {
                        "—".to_string()
                    } else {
                        st.depends_on.join(", ")
                    };
                    let agent = st
                        .agent_type
                        .as_deref()
                        .unwrap_or(ragent_team::team::DEFAULT_AGENT_TYPE);
                    output.push_str(&format!(
                        "| {} | {} | {} | {} |\n",
                        st.id, st.title, agent, deps
                    ));
                }
                output.push_str("\nSpawning teammates…\n");
                self.append_assistant_text(&output);

                // Record swarm state (prompt is blank for now — it was consumed in the slash command)
                let swarm_prompt = String::new();
                let default_agent_type = self
                    .swarm_state
                    .as_ref()
                    .and_then(|s| s.default_agent_type.clone());
                self.swarm_state = Some(SwarmState {
                    team_name: team_name.clone(),
                    prompt: swarm_prompt,
                    decomposition: decomposition.clone(),
                    spawned: false,
                    completed: false,
                    default_agent_type,
                });
                // Spawn one teammate per subtask
                self.spawn_swarm_teammates(&team_name, &decomposition, &store.dir);
            }
            Err(e) => {
                self.status = format!("⚠ swarm: team creation failed: {e}");
                self.push_log_no_agent(LogLevel::Warn, format!("Swarm team creation failed: {e}"));
            }
        }
    }

    pub(crate) fn spawn_swarm_teammates(
        &mut self,
        team_name: &str,
        decomposition: &team::SwarmDecomposition,
        team_dir: &std::path::Path,
    ) {
        let working_dir = std::env::current_dir().unwrap_or_default();

        for subtask in &decomposition.tasks {
            let teammate_name = format!("swarm-{}", subtask.id);

            // Resolve per-subtask agent type (classification fallback already applied).
            let agent_type = subtask
                .agent_type
                .as_deref()
                .unwrap_or(ragent_team::team::DEFAULT_AGENT_TYPE)
                .to_string();

            // Parse per-subtask model override
            let teammate_model: Option<ragent_agent::agent::ModelRef> =
                subtask.model.as_deref().and_then(|s| {
                    s.split_once('/')
                        .or_else(|| s.split_once(':'))
                        .map(|(p, m)| ragent_agent::agent::ModelRef {
                            provider_id: p.to_string(),
                            model_id: m.to_string(),
                        })
                });

            // Tasks with unresolved dependencies start as Blocked; others as Spawning
            let has_deps = !subtask.depends_on.is_empty();
            let initial_status = if has_deps {
                MemberStatus::Blocked
            } else {
                MemberStatus::Spawning
            };

            // Build a rich prompt with task context
            let prompt = format!(
                "## Swarm Task: {}\n\n\
                **Task ID:** {}\n\
                **Title:** {}\n\n\
                {}\n\n\
                You are part of a swarm team. Complete this specific task.\n\n\
                IMPORTANT: Your VERY FIRST action must be a tool call. \
                Call `team_read_messages` with team_name set to the team name from your context. \
                Do NOT respond with text first — call the tool immediately.\n\n\
                After reading messages, do the work described above using tool calls \
                (glob, read, bash, etc.). \
                When done, call `team_task_complete` to mark task \"{}\" as completed.\
                Focus only on your assigned task — other teammates are handling other parts.",
                subtask.title, subtask.id, subtask.title, subtask.description, subtask.id
            );

            // Record member in config
            {
                if let Ok(mut store) = team::TeamStore::load_by_name(team_name, &working_dir) {
                    if store.config.member_by_name(&teammate_name).is_none() {
                        let agent_id = store.next_agent_id();
                        let mut member =
                            team::TeamMember::new(&teammate_name, &agent_id, &agent_type);
                        member.spawn_prompt = Some(prompt.clone());
                        member.model_override = teammate_model.clone();
                        member.status = initial_status;
                        store.config.members.push(member.clone());
                        let _ = store.save();

                        // Add to local state
                        self.team_members.push(member);
                    }
                }
            }

            let status_label = if has_deps {
                "blocked (deps)"
            } else {
                "spawning"
            };
            self.push_log_no_agent(
                LogLevel::Info,
                format!(
                    "🐝 Swarm teammate: {} ({}) — {} ({} agent)",
                    teammate_name, subtask.id, status_label, agent_type
                ),
            );
        }

        // Trigger reconcile — the manager picks up Spawning members and spawns them.
        // Blocked members are skipped by reconcile (they aren't MemberStatus::Spawning).
        if let Some(manager) = self.session_processor.team_manager.get() {
            manager.clone().reconcile_spawning_members();
        } else {
            self.ensure_team_manager_for_team_inner(team_name, Some(team_dir.to_path_buf()), true);
        }

        if let Some(ref mut swarm) = self.swarm_state {
            swarm.spawned = true;
        }

        let ready = decomposition
            .tasks
            .iter()
            .filter(|t| t.depends_on.is_empty())
            .count();
        let blocked = decomposition.tasks.len() - ready;
        self.status = format!("🐝 swarm: {ready} spawning, {blocked} blocked");
    }

    pub(crate) fn handle_swarm_status(&mut self) {
        let Some(ref swarm) = self.swarm_state else {
            self.append_assistant_text(
                "From: /swarm status\n\nNo active swarm. Use `/swarm <prompt>` to start one.\n",
            );
            return;
        };

        let mut output = format!("From: /swarm status\n## 🐝 Swarm: {}\n\n", swarm.team_name);

        // Load tasks from disk for current status
        let working_dir = std::env::current_dir().unwrap_or_default();
        let tasks = if let Ok(store) = team::TeamStore::load_by_name(&swarm.team_name, &working_dir)
        {
            if let Ok(ts) = team::TaskStore::open(&store.dir) {
                ts.read().ok()
            } else {
                None
            }
        } else {
            None
        };

        let total = swarm.decomposition.tasks.len();
        let (completed, in_progress, pending) = if let Some(ref tl) = tasks {
            let c = tl
                .tasks
                .iter()
                .filter(|t| t.status == team::TaskStatus::Completed)
                .count();
            let ip = tl
                .tasks
                .iter()
                .filter(|t| t.status == team::TaskStatus::InProgress)
                .count();
            let p = tl
                .tasks
                .iter()
                .filter(|t| t.status == team::TaskStatus::Pending)
                .count();
            (c, ip, p)
        } else {
            (0, 0, total)
        };

        // Progress bar
        let bar_width = 30;
        let filled = total
            .saturating_mul(bar_width)
            .checked_div(total)
            .unwrap_or(0);
        let bar: String = "█".repeat(filled) + &"░".repeat(bar_width - filled);
        output.push_str(&format!(
            "**Progress:** [{bar}] {completed}/{total} ({} in progress, {} pending)\n\n",
            in_progress, pending
        ));

        // Task table
        output.push_str("| ID | Title | Status | Assigned | Dependencies |\n");
        output.push_str("|----|-------|--------|----------|-------------|\n");

        if let Some(ref tl) = tasks {
            for task in &tl.tasks {
                let status_icon = match task.status {
                    team::TaskStatus::Completed => "✅",
                    team::TaskStatus::InProgress => "🔄",
                    team::TaskStatus::Pending => "⏳",
                    team::TaskStatus::Cancelled => "❌",
                };
                let assigned = task.assigned_to.as_deref().unwrap_or("—");
                let deps = if task.depends_on.is_empty() {
                    "—".to_string()
                } else {
                    task.depends_on.join(", ")
                };
                output.push_str(&format!(
                    "| {} | {} | {} | {} | {} |\n",
                    task.id, task.title, status_icon, assigned, deps
                ));
            }
        } else {
            for st in &swarm.decomposition.tasks {
                let deps = if st.depends_on.is_empty() {
                    "—".to_string()
                } else {
                    st.depends_on.join(", ")
                };
                output.push_str(&format!(
                    "| {} | {} | ⏳ | — | {} |\n",
                    st.id, st.title, deps
                ));
            }
        }

        // Teammate status
        output.push_str("\n**Teammates:**\n");
        if self.team_members.is_empty() {
            output.push_str("  (spawning…)\n");
        } else {
            for m in &self.team_members {
                let status = format!("{:?}", m.status).to_lowercase();
                output.push_str(&format!(
                    "  • {} — {} ({} agent)\n",
                    m.name, status, m.agent_type
                ));
            }
        }

        if completed == total && total > 0 {
            output.push_str("\n🎉 **All tasks complete!** Use `/swarm cancel` to clean up.\n");
        }
        self.append_assistant_text(&output);
    }

    pub(crate) fn handle_swarm_cancel(&mut self) {
        let Some(swarm) = self.swarm_state.take() else {
            self.append_assistant_text("From: /swarm cancel\n\nNo active swarm to cancel.\n");
            return;
        };

        // Reuse the existing team cleanup path
        let team_name = swarm.team_name.clone();

        // Trigger team cleanup
        self.execute_slash_command(&format!("/team close {}", team_name));

        self.append_assistant_text(&format!(
            "From: /swarm cancel\n## 🐝 Swarm Cancelled\n\n\
            Swarm **{team_name}** has been shut down.\n"
        ));
        self.status = "swarm: cancelled".to_string();
        self.push_log_no_agent(LogLevel::Info, format!("Swarm cancelled: {team_name}"));
    }

    /// Periodically (every 2s) check whether any swarm teammate is waiting on
    /// a permission prompt and, if so, surface the unblock UI so the lead can
    /// approve/deny without leaving the swarm view.
    pub fn poll_swarm_unblock(&mut self) {
        if self.swarm_unblock_last_poll.elapsed() < std::time::Duration::from_secs(2) {
            return;
        }
        self.swarm_unblock_last_poll = std::time::Instant::now();
        let Some(ref swarm) = self.swarm_state else {
            return;
        };
        if swarm.completed {
            return;
        }

        // Clone what we need from swarm_state to avoid borrow issues
        let team_name = swarm.team_name.clone();
        let decomp_tasks = swarm.decomposition.tasks.clone();

        // Find blocked members
        let blocked_members: Vec<(String, String)> = self
            .team_members
            .iter()
            .filter(|m| m.status == MemberStatus::Blocked)
            .map(|m| (m.name.clone(), m.agent_id.clone()))
            .collect();

        if blocked_members.is_empty() {
            return;
        }

        // Build set of completed task IDs from member status.
        // A task ID is the suffix after "swarm-" in the teammate name.
        let completed_task_ids: std::collections::HashSet<String> = self
            .team_members
            .iter()
            .filter(|m| matches!(m.status, MemberStatus::Idle | MemberStatus::Stopped))
            .filter_map(|m| m.name.strip_prefix("swarm-").map(|s| s.to_string()))
            .collect();

        // Also check TaskStore for explicitly completed tasks
        let working_dir = std::env::current_dir().unwrap_or_default();
        let task_completed_ids: std::collections::HashSet<String> =
            if let Ok(store) = team::TeamStore::load_by_name(&team_name, &working_dir) {
                if let Ok(ts) = team::TaskStore::open(&store.dir) {
                    if let Ok(tl) = ts.read() {
                        tl.tasks
                            .iter()
                            .filter(|t| t.status == team::TaskStatus::Completed)
                            .map(|t| t.id.clone())
                            .collect()
                    } else {
                        std::collections::HashSet::new()
                    }
                } else {
                    std::collections::HashSet::new()
                }
            } else {
                std::collections::HashSet::new()
            };

        let all_completed: std::collections::HashSet<String> = completed_task_ids
            .union(&task_completed_ids)
            .cloned()
            .collect();

        // Check each blocked member's dependencies
        let mut unblocked = Vec::new();
        for (member_name, agent_id) in &blocked_members {
            let task_id = member_name.strip_prefix("swarm-").unwrap_or(member_name);
            // Find the task's depends_on from decomposition
            let deps = decomp_tasks
                .iter()
                .find(|t| t.id == task_id)
                .map(|t| &t.depends_on);

            if let Some(deps) = deps {
                let missing: Vec<_> = deps
                    .iter()
                    .filter(|d| !all_completed.contains(*d))
                    .cloned()
                    .collect();
                tracing::debug!(
                    task = %task_id,
                    deps = ?deps,
                    missing = ?missing,
                    completed_ids = ?all_completed,
                    "Checking swarm dependency resolution"
                );
                if missing.is_empty() && !deps.is_empty() {
                    unblocked.push((member_name.clone(), agent_id.clone(), task_id.to_string()));
                } else if deps.is_empty() {
                    // No deps — should have been Spawning, but unblock anyway
                    unblocked.push((member_name.clone(), agent_id.clone(), task_id.to_string()));
                }
            }
        }

        if unblocked.is_empty() {
            return;
        }

        // Transition unblocked members from Blocked → Spawning
        for (member_name, agent_id, task_id) in &unblocked {
            // Update local state
            if let Some(m) = self
                .team_members
                .iter_mut()
                .find(|m| m.agent_id == *agent_id)
            {
                m.status = MemberStatus::Spawning;
            }
            // Update persisted config
            if let Ok(mut store) = team::TeamStore::load_by_name(&team_name, &working_dir) {
                if let Some(m) = store.config.member_by_id_mut(agent_id) {
                    m.status = MemberStatus::Spawning;
                }
                let _ = store.save();
            }
            // Log with actual deps for debugging
            let dep_info = decomp_tasks
                .iter()
                .find(|t| t.id == *task_id)
                .map(|t| t.depends_on.join(", "))
                .unwrap_or_default();
            self.push_log_no_agent(
                LogLevel::Info,
                format!(
                    "🔓 Unblocking {} ({}) — deps [{}] all in {:?}",
                    member_name, task_id, dep_info, all_completed
                ),
            );
        }

        // Trigger reconcile to spawn newly-unblocked members
        if let Some(manager) = self.session_processor.team_manager.get() {
            manager.clone().reconcile_spawning_members();
        }

        let remaining_blocked = blocked_members.len() - unblocked.len();
        if remaining_blocked > 0 {
            self.status = format!(
                "🐝 swarm: {} unblocked, {} still blocked",
                unblocked.len(),
                remaining_blocked
            );
        } else {
            self.status = format!("🐝 swarm: all teammates spawned");
        }
    }

    /// Periodically (every 2s) check the active swarm's overall completion
    /// status and update the status line / log panel accordingly.
    pub fn poll_swarm_completion(&mut self) {
        if self.swarm_completion_last_poll.elapsed() < std::time::Duration::from_secs(2) {
            return;
        }
        self.swarm_completion_last_poll = std::time::Instant::now();
        let Some(ref swarm) = self.swarm_state else {
            return;
        };
        if swarm.completed || !swarm.spawned {
            return;
        }
        let team_name = swarm.team_name.clone();

        let working_dir = std::env::current_dir().unwrap_or_default();

        // Check member status — if all non-lead members are terminal (idle/failed/stopped),
        // the swarm is effectively done regardless of task store state.
        let members: Vec<_> = self
            .team_members
            .iter()
            .filter(|m| m.name != "lead" && !m.name.is_empty())
            .collect();
        let has_members = !members.is_empty();
        let all_members_terminal = has_members
            && members.iter().all(|m| {
                matches!(
                    m.status,
                    MemberStatus::Idle | MemberStatus::Failed | MemberStatus::Stopped
                )
            });

        // If all members are terminal, auto-complete any non-completed tasks in the task store
        if all_members_terminal {
            if let Ok(store) = team::TeamStore::load_by_name(&team_name, &working_dir) {
                if let Ok(ts) = team::TaskStore::open(&store.dir) {
                    if let Ok(tl) = ts.read() {
                        for task in &tl.tasks {
                            if task.status != team::TaskStatus::Completed
                                && task.status != team::TaskStatus::Cancelled
                            {
                                let agent_id = task.assigned_to.as_deref().unwrap_or("swarm");
                                if let Err(e) = ts.complete(&task.id, agent_id) {
                                    tracing::warn!(task = %task.id, error = %e, "failed to auto-complete swarm task");
                                }
                            }
                        }
                    }
                }
            }
        }

        // Now check task store for final tally
        let tasks = if let Ok(store) = team::TeamStore::load_by_name(&team_name, &working_dir) {
            if let Ok(ts) = team::TaskStore::open(&store.dir) {
                ts.read().ok()
            } else {
                None
            }
        } else {
            None
        };

        let Some(ref tl) = tasks else {
            // No task store — fall back to member-only check
            if all_members_terminal {
                self.finalize_swarm_completion(&team_name, 0, 0, 0);
            }
            return;
        };
        let total = tl.tasks.len();
        if total == 0 && all_members_terminal {
            self.finalize_swarm_completion(&team_name, 0, 0, 0);
            return;
        }
        if total == 0 {
            return;
        }

        let completed = tl
            .tasks
            .iter()
            .filter(|t| t.status == team::TaskStatus::Completed)
            .count();
        let cancelled = tl
            .tasks
            .iter()
            .filter(|t| t.status == team::TaskStatus::Cancelled)
            .count();
        let failed_members = members
            .iter()
            .filter(|m| m.status == MemberStatus::Failed)
            .count();

        if completed + cancelled >= total {
            self.finalize_swarm_completion(&team_name, total, completed, cancelled);
        } else if all_members_terminal {
            // Members done but tasks not all completed — report partial completion
            self.finalize_swarm_completion(
                &team_name,
                total,
                completed,
                cancelled + failed_members,
            );
        }
    }

    pub(crate) fn finalize_swarm_completion(
        &mut self,
        team_name: &str,
        total: usize,
        completed: usize,
        cancelled: usize,
    ) {
        let working_dir = std::env::current_dir().unwrap_or_default();

        let mut output = format!(
            "From: /swarm\n## 🎉 Swarm Complete: {team_name}\n\n\
            All **{total}** subtasks have finished ({completed} completed, {cancelled} failed/cancelled).\n\n"
        );

        // Include task table if we have tasks
        if total > 0 {
            if let Ok(store) = team::TeamStore::load_by_name(team_name, &working_dir) {
                if let Ok(ts) = team::TaskStore::open(&store.dir) {
                    if let Ok(tl) = ts.read() {
                        output.push_str("| ID | Title | Status |\n|----|-------|--------|\n");
                        for task in &tl.tasks {
                            let icon = match task.status {
                                team::TaskStatus::Completed => "✅",
                                team::TaskStatus::Cancelled => "❌",
                                _ => "⚠️",
                            };
                            output.push_str(&format!(
                                "| {} | {} | {} |\n",
                                task.id, task.title, icon
                            ));
                        }
                        output.push('\n');
                    }
                }
            }
        }

        output.push_str("Use `/swarm cancel` to clean up the ephemeral team.\n");

        self.append_assistant_text(&output);
        self.status = format!("🎉 swarm complete: {team_name}");
        self.push_log_no_agent(
            LogLevel::Info,
            format!("Swarm complete: {team_name} — {completed}/{total} tasks done"),
        );

        if let Some(ref mut s) = self.swarm_state {
            s.completed = true;
        }
    }

}
