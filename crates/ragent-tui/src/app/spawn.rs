//! `/spawn` - detached fire-and-forget background sub-agent launcher.
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
//! the command handler registers the request on the shared bounded launch set
//! ([`App::begin_subagent_launch`]) and hands the work to
//! [`run_subagent_launch`]; the next event-loop pass
//! ([`App::poll_subagent_launch_results`]) releases the slot and surfaces the
//! outcome. That helper is the only launch plumbing in the TUI, so this command
//! carries no state machine of its own (spec `agentssetup` D3).

use crate::app::launch::{detached_launch_work, dispatch_launch, run_subagent_launch};
use crate::app::state::{App, LaunchOrigin, LogLevel};

// Reuse the `/prompt` roster resolver so built-ins (hidden ones included via
// exact-case) and custom agents resolve identically to `/prompt <agent>`.
use crate::app::prompt::{AgentResolution, resolve_agent};

impl App {
    /// Handle `/spawn [help]|<agent> <prompt...>`.
    pub(crate) fn handle_spawn_command(&mut self, args: &str) {
        let args = args.trim();

        if args.is_empty() || matches!(args, "help" | "--help" | "-h") {
            self.append_assistant_text(
                "From: /spawn help\n\n## /spawn - Detached background sub-agent\n\n\
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
                "From: /spawn\nUsage: `/spawn <agent> <prompt...>` - see `/spawn help`.",
            );
            self.status = "spawn: usage".to_string();
            return;
        };
        let agent_name = agent_name.trim();
        let prompt = prompt.trim();

        if prompt.is_empty() {
            self.append_assistant_text(
                "From: /spawn\nUsage: `/spawn <agent> <prompt...>` - the prompt must not be empty.",
            );
            self.status = "spawn: usage".to_string();
            return;
        }

        // Validate the agent name against built-ins + loaded custom agents.
        // `builtin_agents()` returns a static slice and `custom_agent_defs`
        // is only borrowed - validation needs no clones at all.
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
                    format!("spawn: unknown agent '{agent_name}' - available: {list}"),
                );
                self.append_assistant_text(&format!(
                    "From: /spawn\n## [warn] Unknown agent `{agent_name}`\n\n\
                     Available agents: {list}\n"
                ));
                return;
            }
        }

        // D3: registration on the shared bounded launch set replaces the
        // hand-rolled `Mutex<Option<Result<..>>>` slot. Every TUI launch path
        // registers here, so `/spawn` is no longer a special case and a second
        // Enter in the same tick cannot race two launches.
        let launch = match self.begin_subagent_launch(LaunchOrigin::SpawnCommand, agent_name) {
            Ok(launch) => launch,
            Err(refusal) => {
                self.status = format!("[warn] spawn: {refusal}");
                self.push_log_no_agent(LogLevel::Warn, format!("spawn: {refusal}"));
                return;
            }
        };

        let Some(parent_sid) = self.session_id.clone() else {
            // The gate in `execute_slash_command_inner` already created a
            // session; reaching here with none is a bug - fail visibly rather
            // than holding the launch slot forever.
            self.release_subagent_launch(launch.id);
            self.status = "[warn] spawn: no active session".to_string();
            return;
        };

        let working_dir = self.cwd_path.clone();
        let agent_label = agent_name.to_string();

        // A session with no manager is reported by the shared work future as a
        // failure outcome, so the launch slot is released by the normal poll
        // rather than by a second hand-written guard here.
        let agent_manager = self.session_processor.agent_manager.get().cloned();
        let prompt_owned = prompt.to_string();
        let model_override = self.selected_model.clone();

        let work = detached_launch_work(
            agent_manager,
            parent_sid,
            agent_label.clone(),
            prompt_owned,
            model_override,
            working_dir,
        );
        dispatch_launch(run_subagent_launch(
            launch,
            work,
            std::sync::Arc::clone(&self.subagent_launch_results),
        ));

        self.status = format!("[wait] spawn: launching {agent_label} ...");
        self.push_log_no_agent(
            LogLevel::Info,
            format!("spawn: launching detached {agent_label}"),
        );
    }
}
