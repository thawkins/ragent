//! `/spawn` — detached fire-and-forget background sub-agent launcher.
//!
//! Parses `/spawn <agent> <prompt...>`, validates `<agent>` against the
//! built-in roster plus loaded custom agents (`app::prompt::resolve_agent`),
//! and launches a DETACHED background sub-agent through
//! `AgentManager::spawn_detached`. A detached task runs in the background
//! and publishes the usual `SubagentStart`/`SubagentComplete` events (so the
//! Agents panel and log panel show it), but it is excluded from
//! `list_agents`, cannot be awaited via `wait_agents` (explicit or
//! omit-`task_ids`), is never injected into the parent session's message
//! stream, and is out of scope of `team_wait` (teams live on the separate
//! team manager).
//!
//! The launch itself is async (`spawn_detached` creates a child session), so
//! the command handler deposits the future's outcome in `App::spawn_result`
//! and the next event-loop pass (`poll_spawn_result`) surfaces it.

use crate::app::state::{App, LogLevel};

// Reuse the `/prompt` roster resolver so built-ins (hidden ones included via
// exact-case) and custom agents resolve identically to `/prompt <agent>`.
use crate::app::prompt::{AgentResolution, resolve_agent};

impl App {
    /// Handle `/spawn [help]|<agent> <prompt...>`.
    pub(crate) fn handle_spawn_command(&mut self, args: &str) {
        let args = args.trim();

        if args.is_empty() || matches!(args, "help" | "--help" | "-h") {
            self.append_assistant_text(
                "From: /spawn help\n\n## /spawn — Detached background sub-agent\n\n\
                 | Form | Description |\n\
                 |------|-------------|\n\
                 | `/spawn <agent> <prompt...>` | Spawn `<agent>` (built-in or custom) as a detached background sub-agent |\n\
                 | `/spawn help` | Show this help |\n\n\
                 The spawned task is **fire-and-forget**: it never appears in \
                 `list_agents`, cannot be awaited via `wait_agents` or any team \
                 tool, and its result is not injected back into this session. \
                 Use `/cancel <id-prefix>` or the Agents panel to stop it. \
                 Example: `/spawn general Summarise the src/ directory layout`.",
            );
            self.status = "spawn: help".to_string();
            return;
        }

        let Some((agent_name, prompt)) = args.split_once(char::is_whitespace) else {
            self.append_assistant_text(
                "From: /spawn\nUsage: `/spawn <agent> <prompt...>` — see `/spawn help`.",
            );
            self.status = "spawn: usage".to_string();
            return;
        };
        let agent_name = agent_name.trim();
        let prompt = prompt.trim();

        if prompt.is_empty() {
            self.append_assistant_text(
                "From: /spawn\nUsage: `/spawn <agent> <prompt...>` — the prompt must not be empty.",
            );
            self.status = "spawn: usage".to_string();
            return;
        }

        // Validate the agent name against built-ins + loaded custom agents.
        // `builtin_agents()` returns a static slice and `custom_agent_defs`
        // is only borrowed — validation needs no clones at all.
        let builtins = ragent_agent::agent::builtin_agents();
        let customs = self.custom_agent_defs.as_slice();
        match resolve_agent(agent_name, builtins, customs) {
            AgentResolution::Found(_) => {}
            AgentResolution::Miss { available, .. } => {
                let list = if available.is_empty() {
                    "(none resolvable)".to_string()
                } else {
                    available.join(", ")
                };
                self.status = format!("spawn: unknown agent '{agent_name}'");
                self.push_log_no_agent(
                    LogLevel::Warn,
                    format!("spawn: unknown agent '{agent_name}' — available: {list}"),
                );
                self.append_assistant_text(&format!(
                    "From: /spawn\n## [warn] Unknown agent `{agent_name}`\n\n\
                     Available agents: {list}\n"
                ));
                return;
            }
        }

        // Only one pending launch at a time — the slot doubles as the
        // "already waiting" guard (it is cleared by poll_spawn_result).
        let slot = {
            let mut guard =
                crate::app::session_ops::recover_poisoned(self.spawn_result.lock(), "spawn_result");
            if guard.is_some() {
                self.status =
                    "[warn] spawn: another /spawn is still launching — wait for it".to_string();
                return;
            }
            // Hold a marker so a second Enter in the same tick does not race
            // two launches (the real outcome replaces it before the next
            // paint).
            *guard = Some(Err(String::new()));
            std::sync::Arc::clone(&self.spawn_result)
        };

        let Some(parent_sid) = self.session_id.clone() else {
            // The gate in `execute_slash_command_inner` already created a
            // session; reaching here with none is a bug — fail visibly.
            self.status = "[warn] spawn: no active session".to_string();
            return;
        };
        let working_dir = self.cwd_path.clone();
        let agent_label = agent_name.to_string();

        let agent_manager = self.session_processor.agent_manager.get().cloned();
        let Some(agent_manager) = agent_manager else {
            self.status = "[warn] spawn: sub-agent manager unavailable in this session".to_string();
            let mut guard =
                crate::app::session_ops::recover_poisoned(self.spawn_result.lock(), "spawn_result");
            *guard = None;
            return;
        };

        let prompt_owned = prompt.to_string();
        let model_override = self.selected_model.clone();
        let agent_for_spawn = agent_label.clone();

        // The launch work is async (create child session + register the
        // task); dispatch on the ambient tokio runtime when one is available,
        // falling back to a tiny dedicated runtime on a helper thread so the
        // command also works from sync callers that lack a running reactor.
        let launch = async move {
            let outcome = agent_manager
                .spawn_detached(
                    &parent_sid,
                    &agent_for_spawn,
                    &prompt_owned,
                    model_override.as_deref(),
                    &working_dir,
                )
                .await
                .map(|entry| entry.id)
                .map_err(|e| e.to_string());
            if let Ok(mut guard) = slot.lock() {
                *guard = Some(outcome);
            } else {
                tracing::error!("spawn_result mutex poisoned; spawn outcome dropped");
            }
        };
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => {
                handle.spawn(launch);
            }
            Err(_) => {
                std::thread::spawn(move || {
                    match tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                    {
                        Ok(rt) => rt.block_on(launch),
                        Err(e) => {
                            tracing::error!(error = %e, "spawn: helper runtime build failed")
                        }
                    }
                });
            }
        }

        self.status = format!("[wait] spawn: launching {agent_label} …");
        self.push_log_no_agent(
            LogLevel::Info,
            format!("spawn: launching detached {agent_label}"),
        );
    }

    /// Poll the `/spawn` launch slot and surface the outcome. Runs on every
    /// event-loop pass alongside the other `poll_*` fns.
    pub(crate) fn poll_spawn_result(&mut self) {
        let outcome = {
            let mut guard =
                crate::app::session_ops::recover_poisoned(self.spawn_result.lock(), "spawn_result");
            guard.take()
        };
        let Some(outcome) = outcome else { return };
        match outcome {
            Ok(task_id) => {
                self.status = format!(
                    "spawn: detached task {} running",
                    &task_id[..8.min(task_id.len())]
                );
                self.push_log_no_agent(
                    LogLevel::Info,
                    format!("spawn: detached background task launched ({task_id})"),
                );
                self.append_assistant_text(&format!(
                    "From: /spawn\n## Detached sub-agent launched\n\n\
                     Task **{task_id}** is running in the background.\n\n\
                     It is detached: nothing will wait for it, it will not appear \
                     in `list_agents`, and its output will not be injected into \
                     this chat. Track it in the Agents panel or stop it with \
                     `/cancel {}`.\n",
                    &task_id[..8.min(task_id.len())]
                ));
            }
            Err(msg) if msg.is_empty() => {
                // Marker entry — the real outcome has not landed yet; leave
                // the slot occupied so concurrent /spawn invocations stay
                // serialised. Restore the marker ONLY if the slot is still
                // empty: if the async launch task landed its outcome between
                // our `take()` above and this lock acquisition, the real
                // result must win or the user never sees the success path.
                let mut guard = crate::app::session_ops::recover_poisoned(
                    self.spawn_result.lock(),
                    "spawn_result",
                );
                if guard.is_none() {
                    *guard = Some(Err(String::new()));
                }
            }
            Err(msg) => {
                self.status = format!("[warn] spawn failed: {msg}");
                self.push_log_no_agent(LogLevel::Warn, format!("spawn failed: {msg}"));
                self.append_assistant_text(&format!(
                    "From: /spawn\n## [err] Could not launch detached sub-agent\n\n{msg}\n"
                ));
            }
        }
    }
}
