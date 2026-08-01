//! Slash-command dispatch for the TUI.
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use ragent_agent::{event::Event, mcp::McpClient, message::Message, tool::TeamManagerInterface};
use ragent_team::team::{
    self, Mailbox, MailboxMessage, MemberStatus, MessageType, TaskStatus, TeamStore,
};
use ragent_types::ThinkingLevel;

use ragent_config::OtelConfig;
use ragent_telemetry::counters::{TelemetryCountersContent, current_values};

use crate::research_adapter::RagentCompleter;

// Prompt optimization templates
use ragent_prompt_opt::{Completer, OptMethod, optimize};

// State types from app/state.rs
use crate::app::state::{
    App, ConfigSavePickerState, ConfiguredProvider, LogLevel, McpDiscoverState,
    PendingForceCleanup, ProviderSetupStep, ProviderSource, RoleMode, SLASH_COMMANDS,
    SlashMenuEntry, SlashMenuState,
};

// Helpers
use crate::app::helpers::{parse_swarm_args, short_session_id};

// Re-export status types from theme

impl App {
    pub(crate) fn get_command_suggestions(&self, trigger: &str) -> Vec<String> {
        match trigger {
            "team" | "teams" => {
                // Suggest team-related subcommands
                vec![
                    "create".to_string(),
                    "open".to_string(),
                    "close".to_string(),
                    "delete".to_string(),
                    "list".to_string(),
                    "spawn".to_string(),
                    "message".to_string(),
                    "tasks".to_string(),
                    "cleanup".to_string(),
                ]
            }
            "memory" => {
                // Suggest memory categories
                vec![
                    "search".to_string(),
                    "add".to_string(),
                    "forget".to_string(),
                    "config".to_string(),
                ]
            }
            "agent" | "agents" => {
                // Suggest agent-related subcommands
                vec!["list".to_string(), "switch".to_string()]
            }
            "thinking" => {
                vec![
                    "auto".to_string(),
                    "off".to_string(),
                    "low".to_string(),
                    "medium".to_string(),
                    "high".to_string(),
                ]
            }
            "codeindex" => {
                vec!["on".to_string(), "off".to_string(), "sync".to_string()]
            }
            "tools" => vec![
                "show".to_string(),
                "office".to_string(),
                "github".to_string(),
                "gitlab".to_string(),
                "codeindex".to_string(),
                "help".to_string(),
            ],
            "theme" => {
                vec![
                    "toggle".to_string(),
                    "light".to_string(),
                    "dark".to_string(),
                ]
            }
            "mouse" => {
                vec!["on".to_string(), "off".to_string()]
            }
            "websearch" => {
                vec!["show".to_string(), "test".to_string(), "help".to_string()]
            }
            "status" => {
                vec!["clear".to_string()]
            }
            "help" => {
                vec![
                    "team".to_string(),
                    "memory".to_string(),
                    "agent".to_string(),
                    "ui".to_string(),
                    "accessibility".to_string(),
                ]
            }
            "spec" => {
                vec![
                    "help".to_string(),
                    "create".to_string(),
                    "list".to_string(),
                    "search".to_string(),
                    "validate".to_string(),
                    "status".to_string(),
                    "task".to_string(),
                ]
            }
            "router" => {
                vec![
                    "on".to_string(),
                    "off".to_string(),
                    "status".to_string(),
                    "tiers".to_string(),
                    "weights".to_string(),
                    "boundaries".to_string(),
                    "test".to_string(),
                    "stats".to_string(),
                    "reload".to_string(),
                    "help".to_string(),
                ]
            }
            "config" => {
                vec!["show".to_string(), "save".to_string(), "list".to_string()]
            }
            "init" => {
                vec!["config".to_string()]
            }
            _ => Vec::new(),
        }
    }

    /// Refresh the slash-command autocomplete menu based on the current input
    /// buffer. Closes the menu once a space is typed (so subcommand args are
    /// not obscured) and filters built-in + agent commands by prefix.
    pub fn update_slash_menu(&mut self) {
        if let Some(filter) = self.input.strip_prefix('/') {
            // If the user has typed a space after the command, close the menu
            // so it doesn't obstruct subcommand arguments.
            if filter.contains(' ') {
                self.slash_menu = None;
                return;
            }

            let needle = filter.to_lowercase();

            // Collect builtin command matches
            let mut matches: Vec<SlashMenuEntry> = SLASH_COMMANDS
                .iter()
                .filter(|cmd| {
                    needle.is_empty()
                        || cmd.trigger.starts_with(&needle)
                        || cmd.description.to_lowercase().contains(&needle)
                })
                .map(|cmd| {
                    // Build context-aware suggestions based on command type
                    let suggestions = self.get_command_suggestions(cmd.trigger);
                    let parameter_hint = self.get_parameter_hint(cmd.trigger);

                    SlashMenuEntry {
                        trigger: cmd.trigger.to_string(),
                        description: cmd.description.to_string(),
                        is_skill: false,
                        suggestions,
                        parameter_hint,
                    }
                })
                .collect();
            // Collect user-invocable skill matches
            let working_dir = std::env::current_dir().unwrap_or_default();
            let skill_dirs = ragent_agent::Config::load()
                .map(|c| c.skill_dirs)
                .unwrap_or_default();
            let registry = ragent_agent::skill::SkillRegistry::load(&working_dir, &skill_dirs);
            for skill in registry.list_user_invocable() {
                let desc = skill
                    .description
                    .as_deref()
                    .unwrap_or("(skill)")
                    .to_string();
                let hint = skill
                    .argument_hint
                    .as_deref()
                    .map(|h| format!(" — {h}"))
                    .unwrap_or_default();

                // Skip if a builtin command has the same trigger
                if matches.iter().any(|m| m.trigger == skill.name) {
                    continue;
                }

                if needle.is_empty()
                    || skill.name.starts_with(&needle)
                    || desc.to_lowercase().contains(&needle)
                {
                    matches.push(SlashMenuEntry {
                        trigger: skill.name.clone(),
                        description: format!("{desc}{hint}"),
                        is_skill: true,
                        suggestions: Vec::new(),
                        parameter_hint: skill.argument_hint.clone(),
                    });
                }
            }

            // Sort alphabetically by trigger so the list is predictable.
            matches.sort_by(|a, b| a.trigger.cmp(&b.trigger));

            // Select the entry whose trigger best matches the typed input:
            // prefer an exact match, then the first entry whose trigger starts
            // with the needle, then fall back to index 0.
            let selected = if matches.is_empty() {
                0
            } else if let Some(exact) = matches.iter().position(|m| m.trigger == needle) {
                exact
            } else if let Some(prefix) = matches.iter().position(|m| m.trigger.starts_with(&needle))
            {
                prefix
            } else {
                0
            };

            self.slash_menu = Some(SlashMenuState {
                matches,
                selected,
                filter: filter.to_string(),
            });
        } else {
            self.slash_menu = None;
        }
    }

    /// Execute a slash command from the raw input string (with or without the
    /// leading `/`). Single entry/single exit: logs the invocation, delegates
    /// to the inner implementation, then logs completion and any output lines.

    /// Return the path of the project or global config file that should be
    /// mutated for telemetry changes. Prefer the project file when a project
    /// config was loaded; otherwise fall back to the global config file.
    fn telemetry_config_source_path(&self) -> Option<std::path::PathBuf> {
        let cfg = ragent_agent::Config::load().unwrap_or_default();
        let was_project = cfg.config_paths.iter().any(|p| {
            p.file_name().is_some_and(|f| f == "ragent.json")
                && p.parent()
                    .and_then(|parent| parent.file_name())
                    .is_some_and(|f| f == ".ragent")
        });
        if was_project {
            Some(
                std::env::current_dir()
                    .unwrap_or_default()
                    .join(".ragent/ragent.json"),
            )
        } else {
            dirs::config_dir().map(|d| d.join("ragent/ragent.json"))
        }
    }

    /// Atomically update the source `ragent.json` with a new `telemetry.otel`
    /// block, preserving all other keys. The destination file is chosen based
    /// on whether a project or global config was loaded.
    pub(crate) fn save_telemetry_otel(&self, otel: &OtelConfig) -> Result<(), String> {
        let path = self
            .telemetry_config_source_path()
            .ok_or_else(|| "no telemetry config destination found".to_string())?;
        crate::app::state::atomic_config_update(&path, |json| {
            // Ensure the top-level "telemetry" object exists.
            if !json.get("telemetry").map_or(false, |v| v.is_object()) {
                json.as_object_mut()
                    .expect("json is an object")
                    .insert("telemetry".to_string(), serde_json::json!({}));
            }
            let telemetry = json
                .as_object_mut()
                .and_then(|obj| obj.get_mut("telemetry"))
                .and_then(|v| v.as_object_mut())
                .expect("telemetry object");
            let value =
                serde_json::to_value(otel).map_err(|e| format!("serialise otel config: {e}"))?;
            telemetry.insert("otel".to_string(), value);
            Ok(())
        })
    }

    /// Enable or disable telemetry in the source config and invalidate the
    /// session-processor config cache so the next loop picks up the change.
    ///
    /// In addition to persisting the new `telemetry.otel.enabled` flag to
    /// `ragent.json`, this reconfigures the live [`TelemetrySubsystem`] in
    /// place so the change takes effect immediately:
    ///
    /// - `/telemetry off` shuts down the running meter provider, which stops
    ///   the periodic OTLP reader and therefore the "Failed to export
    ///   metrics" log noise that would otherwise continue until restart.
    /// - `/telemetry on` builds a fresh provider from the persisted config.
    fn set_telemetry_enabled(&mut self, enabled: bool) {
        let mut cfg = ragent_agent::Config::load().unwrap_or_default();
        cfg.telemetry.otel.enabled = enabled;
        match self.save_telemetry_otel(&cfg.telemetry.otel) {
            Ok(()) => {
                self.session_processor.invalidate_config_cache();
                // Reconfigure the live subsystem so the toggle takes effect
                // immediately (shut down the old provider on `off`, build a
                // fresh one on `on`) rather than waiting for a restart.
                if let Err(e) = self
                    .session_processor
                    .telemetry
                    .reconfigure(cfg.telemetry.otel)
                {
                    self.push_log_no_agent(
                        LogLevel::Error,
                        format!("telemetry reconfigure failed: {e}"),
                    );
                }
            }
            Err(e) => {
                self.push_log_no_agent(
                    LogLevel::Error,
                    format!("telemetry enable/disable failed: {e}"),
                );
            }
        }
    }

    /// Build the markdown help message for the `/telemetry counters` command.
    ///
    /// The live in-memory counter/gauge values are read from
    /// [`ragent_telemetry::counters::current_values`] and appended to each
    /// line so the user can see not only what each metric means, but also its
    /// current value inside the running process.
    fn telemetry_counters_help() -> String {
        Self::telemetry_counters_content().markdown()
    }

    /// Build the structured counter/gauge content shared by `/telemetry counters`
    /// and the live telemetry side panel. Returning a content builder avoids
    /// duplicating the metric definitions and keeps the panel and the chat
    /// output in sync.
    pub(crate) fn telemetry_counters_content() -> TelemetryCountersContent {
        use std::fmt::Write;

        let values = current_values();

        let mut out = String::from("From: /telemetry counters\n\n## Telemetry counters\n\n");

        let fmt_u64 = |v: u64| v.to_string();
        let fmt_i64 = |v: i64| v.to_string();
        let fmt_f64 = |v: f64| format!("{v:.2}");

        let usage: [(&str, &str, &str, String); 9] = [
            (
                "ragent.llm.requests",
                "Counter",
                "Total LLM requests (tagged by model/provider)",
                fmt_u64(values.llm_requests),
            ),
            (
                "ragent.sessions.active",
                "UpDownCounter",
                "Currently active sessions",
                fmt_i64(values.sessions_active),
            ),
            (
                "ragent.sessions.total",
                "Counter",
                "Total sessions created",
                fmt_u64(values.sessions_total),
            ),
            (
                "ragent.messages.user",
                "Counter",
                "User messages submitted",
                fmt_u64(values.messages_user),
            ),
            (
                "ragent.tool.invocations",
                "Counter",
                "Tool invocations",
                fmt_u64(values.tool_invocations),
            ),
            (
                "ragent.agents.active",
                "UpDownCounter",
                "Currently active sub-agents",
                fmt_i64(values.agents_active),
            ),
            (
                "ragent.agents.completed",
                "Counter",
                "Completed sub-agents",
                fmt_u64(values.agents_completed),
            ),
            (
                "ragent.subagent.spawns",
                "Counter",
                "Sub-agent spawn events",
                fmt_u64(values.subagent_spawns),
            ),
            (
                "ragent.team.members",
                "UpDownCounter",
                "Current team members",
                fmt_i64(values.team_members),
            ),
        ];
        let performance: [(&str, &str, &str, String); 7] = [
            (
                "ragent.llm.duration",
                "Histogram",
                "LLM call wall-clock duration (ms, tagged by model/provider)",
                fmt_f64(values.llm_duration_last),
            ),
            (
                "ragent.llm.time_to_first_token",
                "Histogram",
                "Time to first token (ms, tagged by model)",
                fmt_f64(values.llm_ttft_last),
            ),
            (
                "ragent.tool.duration",
                "Histogram",
                "Tool execution duration (ms, tagged by tool.name)",
                fmt_f64(values.tool_duration_last),
            ),
            (
                "ragent.agent_loop.duration",
                "Histogram",
                "Agent loop iteration duration (ms)",
                fmt_f64(values.agent_loop_duration_last),
            ),
            (
                "ragent.agent_loop.iterations",
                "Histogram",
                "Iterations in a completed agent loop",
                fmt_u64(values.agent_loop_iterations_last),
            ),
            (
                "ragent.session.duration",
                "Histogram",
                "Session wall-clock duration (ms)",
                fmt_f64(values.session_duration_last),
            ),
            (
                "ragent.tool.permission_wait",
                "Histogram",
                "Time waiting for user permission (ms)",
                fmt_f64(values.tool_permission_wait_last),
            ),
        ];
        let cost: [(&str, &str, &str, String); 8] = [
            (
                "ragent.tokens.input",
                "Counter",
                "Input tokens (tagged by model)",
                fmt_u64(values.tokens_input),
            ),
            (
                "ragent.tokens.output",
                "Counter",
                "Output tokens (tagged by model)",
                fmt_u64(values.tokens_output),
            ),
            (
                "ragent.tokens.cache_read",
                "Counter",
                "Cache-read tokens (tagged by model)",
                fmt_u64(values.tokens_cache_read),
            ),
            (
                "ragent.tokens.cache_write",
                "Counter",
                "Cache-write tokens (tagged by model)",
                fmt_u64(values.tokens_cache_write),
            ),
            (
                "ragent.cost.estimated",
                "Counter",
                "Estimated cost in USD (tagged by model/provider)",
                fmt_f64(values.cost_estimated),
            ),
            (
                "ragent.cost.session",
                "Histogram",
                "Estimated cost per session",
                fmt_f64(values.cost_session_last),
            ),
            (
                "ragent.rate_limit.requests_pct",
                "Gauge",
                "Request quota percentage (tagged by provider)",
                fmt_f64(values.rate_limit_requests_pct),
            ),
            (
                "ragent.rate_limit.tokens_pct",
                "Gauge",
                "Token quota percentage (tagged by provider)",
                fmt_f64(values.rate_limit_tokens_pct),
            ),
        ];
        let effectiveness: [(&str, &str, &str, String); 10] = [
            (
                "ragent.errors.total",
                "Counter",
                "Total errors (tagged by component)",
                fmt_u64(values.errors_total),
            ),
            (
                "ragent.timeouts.total",
                "Counter",
                "Total timeout events",
                fmt_u64(values.timeouts_total),
            ),
            (
                "ragent.permission.denied",
                "Counter",
                "Permission denials (tagged by tool.name)",
                fmt_u64(values.permission_denied),
            ),
            (
                "ragent.permission.approved",
                "Counter",
                "Permission approvals (tagged by tool.name)",
                fmt_u64(values.permission_approved),
            ),
            (
                "ragent.context.compressions",
                "Counter",
                "Context compression invocations",
                fmt_u64(values.context_compressions),
            ),
            (
                "ragent.context.compression_ratio",
                "Gauge",
                "Context compression before/after ratio (%)",
                fmt_f64(values.context_compression_ratio_last),
            ),
            (
                "ragent.tool.calls_per_session",
                "Histogram",
                "Tool calls per session",
                fmt_u64(values.tool_calls_per_session_last),
            ),
            (
                "ragent.task.completions",
                "Counter",
                "Completed sub-agent and team tasks",
                fmt_u64(values.task_completions),
            ),
            (
                "ragent.retries.llm",
                "Counter",
                "LLM retry attempts (tagged by model)",
                fmt_u64(values.retries_llm),
            ),
            (
                "ragent.snapshot.restores",
                "Counter",
                "Snapshot undo restores",
                fmt_u64(values.snapshot_restores),
            ),
        ];

        let write_group = |out: &mut String, title: &str, group: &[(&str, &str, &str, String)]| {
            let _ = writeln!(out, "### {title}");
            for (name, kind, desc, value) in group {
                let _ = writeln!(out, "- `{name}` — **{value}** — *{kind}* — {desc}");
            }
            let _ = writeln!(out);
        };

        write_group(&mut out, "Usage metrics", &usage);
        write_group(&mut out, "Performance metrics", &performance);
        write_group(&mut out, "Cost metrics", &cost);
        write_group(&mut out, "Effectiveness metrics", &effectiveness);

        out.push_str(
            "### Logs
",
        );
        out.push_str(
            "No counters configured — log signals are planned.

",
        );
        out.push_str(
            "### Traces
",
        );
        out.push_str(
            "No counters configured — trace signals are planned.
",
        );

        TelemetryCountersContent {
            usage: usage
                .iter()
                .map(|(n, k, d, v)| (n.to_string(), k.to_string(), d.to_string(), v.clone()))
                .collect(),
            performance: performance
                .iter()
                .map(|(n, k, d, v)| (n.to_string(), k.to_string(), d.to_string(), v.clone()))
                .collect(),
            cost: cost
                .iter()
                .map(|(n, k, d, v)| (n.to_string(), k.to_string(), d.to_string(), v.clone()))
                .collect(),
            effectiveness: effectiveness
                .iter()
                .map(|(n, k, d, v)| (n.to_string(), k.to_string(), d.to_string(), v.clone()))
                .collect(),
            markdown: out,
        }
    }

    /// Dispatch the `/telemetry` slash-command family.
    fn handle_telemetry_command(&mut self, args: &str) {
        let sub = args.split_whitespace().next().unwrap_or("");
        match sub {
            "help" | "" => {
                self.append_assistant_text(
                    "From: /telemetry help

## /telemetry — Telemetry management

| Subcommand | Description |
|---|---|
| `/telemetry help` | Show this help |
| `/telemetry on` | Enable OpenTelemetry metrics export |
| `/telemetry off` | Disable OpenTelemetry metrics export |
| `/telemetry setup` | Open a TUI dialog to configure endpoint, protocol, interval, timeout, and internal port |
| `/telemetry counters` | List all available metric counters, grouped by category |",
                );
                self.status = "telemetry: help".to_string();
            }
            "on" => {
                self.set_telemetry_enabled(true);
                self.append_assistant_text(
                    "From: /telemetry on

✅ Telemetry enabled.",
                );
                self.status = "telemetry: enabled".to_string();
            }
            "off" => {
                self.set_telemetry_enabled(false);
                self.append_assistant_text(
                    "From: /telemetry off

✅ Telemetry disabled.",
                );
                self.status = "telemetry: disabled".to_string();
            }
            "setup" => {
                let cfg = ragent_agent::Config::load().unwrap_or_default();
                let otel = cfg.telemetry.otel;
                self.provider_setup = Some(ProviderSetupStep::TelemetrySetup {
                    endpoint_field: crate::input_field::InputField::with_text(&otel.endpoint),
                    protocol: otel.protocol,
                    interval_field: crate::input_field::InputField::with_text(
                        otel.export_interval_seconds.to_string(),
                    ),
                    timeout_field: crate::input_field::InputField::with_text(
                        otel.export_timeout_seconds.to_string(),
                    ),
                    port_field: crate::input_field::InputField::with_text(
                        otel.internal_port
                            .map(|p| p.to_string())
                            .unwrap_or_default(),
                    ),
                    active_field: 0,
                    error: None,
                });
                self.status = "telemetry: setup".to_string();
            }
            "counters" => {
                self.append_assistant_text(&Self::telemetry_counters_help());
                self.status = "telemetry: counters".to_string();
            }
            _ => {
                self.append_assistant_text(
                    "From: /telemetry

Usage: `/telemetry help|on|off|setup|counters`",
                );
                self.status = "telemetry: usage".to_string();
            }
        }
    }

    /// Entry point for all slash commands. Logs invocation, records the raw
    /// command in input history, dispatches to the inner implementation, and
    /// emits a "Finished" log entry once complete (unless a background task
    /// is still pending).
    pub fn execute_slash_command(&mut self, raw: &str) {
        // Top-level wrapper: single entry and single exit. Log invocation and
        // call the inner implementation which may return early. On return,
        // log completion and number of assistant output lines added.
        let stripped = raw.strip_prefix('/').unwrap_or(raw).trim();
        let (cmd, args) = stripped
            .split_once(char::is_whitespace)
            .map_or((stripped, ""), |(c, a)| (c, a.trim()));
        let start_lines = self.assistant_output_lines();
        self.push_log_no_agent(LogLevel::Info, format!("Executing /{} {}", cmd, args));

        // Retain the raw slash command in input history so users can recall it later.
        self.add_to_history(raw.to_string());

        // Call the original implementation moved to an inner function.
        self.execute_slash_command_inner(raw);

        // If the command spawned an async task (status begins with ⏳), defer
        // the "Finished" log entry — poll_pending_opt will emit it once the
        // background work completes.
        if self.status.starts_with('⏳') {
            return;
        }

        // Arm the status auto-expiry timer so the indicator transitions to
        // "ready" after a short grace period (see poll_status_expiry).
        self.arm_status_expiry();

        let end_lines = self.assistant_output_lines();
        let added = end_lines.saturating_sub(start_lines);
        self.push_log_no_agent(
            LogLevel::Info,
            format!("Finished /{} {} — {} lines output", cmd, args, added),
        );
    }

    pub(crate) fn execute_slash_command_inner(&mut self, raw: &str) {
        let stripped = raw.strip_prefix('/').unwrap_or(raw).trim();
        self.input.clear();
        self.input_cursor = 0;
        self.slash_menu = None;
        self.scroll_offset = 0;
        self.force_new_message = true;
        self.assert_ui_invariants();

        // Split into command and optional argument text.
        let (cmd, args) = stripped
            .split_once(char::is_whitespace)
            .map_or((stripped, ""), |(c, a)| (c, a.trim()));

        // Central session gate for slash commands.
        // Commands may still choose to bypass this (e.g. quit/exit).
        if !matches!(cmd, "quit" | "exit") && !self.ensure_session() {
            return;
        }

        match cmd {
            "telemetry" => self.handle_telemetry_command(args),
            "about" => {
                let about = format!(
                    "  ragent — AI Coding Agent\n\
                                 \n\
                                 \x20 An interactive TUI-based AI coding agent\n\
                                 \x20 supporting multiple LLM providers.\n\
                                 \n\
                                 \x20 Version:     {}\n\
                                 \x20 Built:       {}\n\
                                 \x20 Repository:  https://github.com/thawkins/ragent\n\
                                 \x20 License:     MIT\n\
                                 \n\
                                 \x20 Authors:\n\
                                 \x20   Tim Hawkins <tim.thawkins@gmail.com>\n",
                    env!("CARGO_PKG_VERSION"),
                    chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
                );
                self.append_assistant_text(&format!("From: /about\n{about}"));
                self.status = "about".to_string();
            }
            "agent" => {
                if args.is_empty() {
                    // Open the agent picker dialog
                    let custom_names: std::collections::HashSet<String> = self
                        .custom_agent_defs
                        .iter()
                        .map(|d| d.agent_info.name.clone())
                        .collect();
                    let agents: Vec<(String, String, bool)> = self
                        .cycleable_agents
                        .iter()
                        .map(|a| {
                            let is_custom =
                                custom_names.contains(&a.name) || a.name.starts_with("custom:");
                            (a.name.clone(), a.description.clone(), is_custom)
                        })
                        .collect();
                    let selected = self.current_agent_index;
                    self.provider_setup = Some(ProviderSetupStep::SelectAgent { agents, selected });
                } else {
                    // Direct switch: /agent <name>
                    if let Some(idx) = self.cycleable_agents.iter().position(|a| a.name == args) {
                        let prev = self.agent_name.clone();
                        self.current_agent_index = idx;
                        self.agent_info = self.cycleable_agents[idx].clone();
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
                    } else {
                        let available: Vec<&str> = self
                            .cycleable_agents
                            .iter()
                            .map(|a| a.name.as_str())
                            .collect();
                        self.status = format!(
                            "Unknown agent '{}'. Available: {}",
                            args,
                            available.join(", ")
                        );
                        self.push_log_no_agent(LogLevel::Warn, format!("Unknown agent: {}", args));
                    }
                }
            }
            "agents" => {
                let mut output = String::from("From: /agents\n\n**Built-in Agents**\n\n");

                let custom_names: std::collections::HashSet<String> = self
                    .custom_agent_defs
                    .iter()
                    .map(|d| d.agent_info.name.clone())
                    .collect();

                for agent in &self.cycleable_agents {
                    let is_custom =
                        custom_names.contains(&agent.name) || agent.name.starts_with("custom:");
                    if !is_custom {
                        let active = if agent.name == self.agent_name {
                            " ●"
                        } else {
                            ""
                        };
                        output.push_str(&format!(
                            "- `{}` — {}{}\n",
                            agent.name, agent.description, active
                        ));
                    }
                }

                if self.custom_agent_defs.is_empty() {
                    output.push_str(
                                      "\n**Custom Agents**\n\n*(none — place .json or .md files in .ragent/agents/ or ~/.ragent/agents/)*\n",
                                  );
                } else {
                    output.push_str("\n**Custom Agents**\n\n");
                    for def in &self.custom_agent_defs {
                        let scope = if def.is_project_local {
                            "project"
                        } else {
                            "global"
                        };
                        let name = &def.agent_info.name;
                        let desc = &def.agent_info.description;
                        let active = if *name == self.agent_name { " ●" } else { "" };
                        let fmt =
                            if def.source_path.extension().and_then(|e| e.to_str()) == Some("md") {
                                "profile"
                            } else {
                                "oasf"
                            };
                        output.push_str(&format!(
                            "- `{}` — {} [{}/{}]{}\n",
                            name, desc, scope, fmt, active
                        ));
                    }
                }

                if !self.custom_agent_diagnostics.is_empty() {
                    output.push_str("\n**Diagnostics**\n\n");
                    for diag in &self.custom_agent_diagnostics {
                        output.push_str(&format!("- ⚠ {}\n", diag));
                    }
                }

                self.append_assistant_text(&output);

                self.status = "agents".to_string();
            }
            "context" => match args.trim() {
                "refresh" => {
                    ragent_agent::agent::clear_prompt_context_cache();
                    self.append_assistant_text(
                                                        "From: /context\n🔄 Context cache cleared — next message will recompute file tree, git status, and README."
                                                    );
                    self.push_log_no_agent(LogLevel::Info, "context cache cleared".to_string());
                    self.status = "context refreshed".to_string();
                }
                _ => {
                    self.append_assistant_text(
                                                        "From: /context\nUsage: `/context refresh` — clears cached file tree, git status, and README context"
                                                    );
                }
            },

            // ── /config ──────────────────────────────────────────────────────
            "config" => match args.trim() {
                "show" => {
                    let cwd = std::env::current_dir().unwrap_or_default();
                    let home = dirs::home_dir().unwrap_or_default();
                    let data_dir = dirs::data_dir().unwrap_or_default().join("ragent");
                    let config_dir = dirs::config_dir().unwrap_or_default().join("ragent");

                    // Determine active config file paths
                    let project_config = cwd.join(".ragent").join("ragent.json");
                    let global_config = config_dir.join("ragent.json");
                    let env_config = std::env::var("RAGENT_CONFIG").ok();

                    let mut output =
                        String::from("From: /config show\n\n📂 **Application Paths**\n\n");

                    output.push_str(&format!(
                        "| {:<24} | {}\n",
                        "Working directory",
                        cwd.display()
                    ));
                    output.push_str(&format!(
                        "| {:<24} | {}\n",
                        "Data directory",
                        data_dir.display()
                    ));
                    output.push_str(&format!(
                        "| {:<24} | {}\n",
                        "Config directory",
                        config_dir.display()
                    ));

                    output.push_str("\n📄 **Config Files**\n\n");

                    let project_exists = project_config.exists();
                    let global_exists = global_config.exists();

                    output.push_str(&format!(
                        "| {:<24} | {} {}\n",
                        "Project config",
                        project_config.display(),
                        if project_exists { "✓" } else { "✗" }
                    ));
                    output.push_str(&format!(
                        "| {:<24} | {} {}\n",
                        "Global config",
                        global_config.display(),
                        if global_exists { "✓" } else { "✗" }
                    ));
                    if let Some(ref env_path) = env_config {
                        let env_exists = std::path::PathBuf::from(env_path).exists();
                        output.push_str(&format!(
                            "| {:<24} | {} {}\n",
                            "Env (RAGENT_CONFIG)",
                            env_path,
                            if env_exists { "✓" } else { "✗" }
                        ));
                    } else {
                        output.push_str(&format!(
                            "| {:<24} | {}\n",
                            "Env (RAGENT_CONFIG)", "(not set)"
                        ));
                    }

                    // Storage database
                    output.push_str(&format!(
                        "\n💾 **Storage**\n\n| {:<24} | {}\n",
                        "Database",
                        self.db_path.display()
                    ));

                    // Code index
                    let codeindex_dir = cwd.join(".ragent").join("codeindex");
                    output.push_str("\n🔍 **Code Index**\n\n");
                    output.push_str(&format!(
                        "| {:<24} | {} {}\n",
                        "Index directory",
                        codeindex_dir.display(),
                        if codeindex_dir.exists() { "✓" } else { "✗" }
                    ));

                    // Memory
                    let memory_dir = cwd.join(".ragent").join("memory");
                    let global_memory = home.join(".ragent").join("memory");
                    output.push_str("\n🧠 **Memory**\n\n");
                    output.push_str(&format!(
                        "| {:<24} | {} {}\n",
                        "Project memory",
                        memory_dir.display(),
                        if memory_dir.exists() { "✓" } else { "✗" }
                    ));
                    output.push_str(&format!(
                        "| {:<24} | {} {}\n",
                        "Global memory",
                        global_memory.display(),
                        if global_memory.exists() { "✓" } else { "✗" }
                    ));

                    // Agents
                    let project_agents = cwd.join(".ragent").join("agents");
                    let global_agents = home.join(".ragent").join("agents");
                    output.push_str("\n🤖 **Custom Agents**\n\n");
                    output.push_str(&format!(
                        "| {:<24} | {} {}\n",
                        "Project agents",
                        project_agents.display(),
                        if project_agents.exists() {
                            "✓"
                        } else {
                            "✗"
                        }
                    ));
                    output.push_str(&format!(
                        "| {:<24} | {} {}\n",
                        "Global agents",
                        global_agents.display(),
                        if global_agents.exists() { "✓" } else { "✗" }
                    ));

                    self.append_assistant_text(&output);
                    self.status = "config: show".to_string();
                }
                // ── /config save ────────────────────────────────────────────
                // FR-003: snapshot the current global ragent.json into a
                // timestamped backup inside `saves/`. The helper creates the
                // `saves/` directory if needed and writes atomically.
                "save" => match ragent_config::Config::backup_global_config(None) {
                    Ok(path) => {
                        self.append_assistant_text(&format!(
                            "From: /config save\n✅ **Saved backup to:**\n  `{}`",
                            path.display()
                        ));
                        self.push_log_no_agent(
                            LogLevel::Info,
                            format!("config save: wrote backup to {}", path.display()),
                        );
                        self.status = "config: saved".to_string();
                    }
                    Err(e) => {
                        self.append_assistant_text(&format!(
                            "From: /config save\n❌ **Failed to back up global config:**\n  \
                                 {e}\n\nNo changes were made."
                        ));
                        self.push_log_no_agent(LogLevel::Error, format!("config save: error: {e}"));
                        self.status = "config: save error".to_string();
                    }
                },
                // ── /config list ────────────────────────────────────────────
                // FR-004 / FR-006: scan the `saves/` directory for backups. If
                // none exist, show a user-facing message instead of an empty
                // picker. Otherwise open the interactive picker (key handling
                // and rendering are wired up in T-006/T-007).
                "list" => {
                    let config_dir = match ragent_config::Config::global_config_dir() {
                        Some(d) => d,
                        None => {
                            self.append_assistant_text(
                                "From: /config list\n❌ **Cannot determine the global config \
                                 directory for this platform.**",
                            );
                            self.push_log_no_agent(
                                LogLevel::Error,
                                "config list: no global config directory".to_string(),
                            );
                            self.status = "config: list error".to_string();
                            return;
                        }
                    };
                    let saves_dir = config_dir.join("saves");

                    // Collect files matching the `ragent.json.*` backup pattern,
                    // excluding the transient `.tmp` temp files written during
                    // atomic backup.
                    let mut entries: Vec<std::path::PathBuf> = match std::fs::read_dir(&saves_dir) {
                        Ok(rd) => rd
                            .filter_map(Result::ok)
                            .filter_map(|e| {
                                let name = e.file_name().to_string_lossy().to_string();
                                let path = e.path();
                                let is_tmp = std::path::Path::new(&name)
                                    .extension()
                                    .is_some_and(|ext| ext.eq_ignore_ascii_case("tmp"));
                                if name.starts_with("ragent.json.") && !is_tmp && path.is_file() {
                                    Some(path)
                                } else {
                                    None
                                }
                            })
                            .collect(),
                        Err(_) => Vec::new(),
                    };

                    if entries.is_empty() {
                        // FR-006: no saved configurations available.
                        self.append_assistant_text(&format!(
                            "From: /config list\nℹ️  **No saved configurations found.**\n\n\
                             Backups are stored in:\n  `{}`\n\nUse `/config save` to create one.",
                            saves_dir.display()
                        ));
                        self.push_log_no_agent(
                            LogLevel::Info,
                            format!("config list: no saves in {}", saves_dir.display()),
                        );
                        self.status = "config: list empty".to_string();
                    } else {
                        // Sort newest-first by modification time (FR-010).
                        entries.sort_by(|a, b| {
                            b.metadata()
                                .and_then(|m| m.modified())
                                .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
                                .cmp(
                                    &a.metadata()
                                        .and_then(|m| m.modified())
                                        .unwrap_or(std::time::SystemTime::UNIX_EPOCH),
                                )
                        });
                        let count = entries.len();
                        self.config_save_picker = Some(ConfigSavePickerState {
                            entries,
                            selected: 0,
                            scroll_offset: 0,
                            config_dir: config_dir.clone(),
                        });
                        self.append_assistant_text(&format!(
                            "From: /config list\n📋 **{} saved configuration(s) found.**\n\n\
                             Use ↑/↓ (or k/j) to select a backup, Enter to restore, Esc to \
                             cancel.",
                            count
                        ));
                        self.push_log_no_agent(
                            LogLevel::Info,
                            format!(
                                "config list: opened picker ({} backup(s) in {})",
                                count,
                                saves_dir.display()
                            ),
                        );
                        self.status = "config: list".to_string();
                    }
                }
                _ => {
                    self.append_assistant_text(
                        "From: /config\nUsage:\n  `/config show` — display all application \
                         paths\n  `/config save` — back up the global ragent.json\n  \
                         `/config list` — browse saved backups and restore one",
                    );
                    self.status = "config: usage".to_string();
                }
            },

            // ── /init ────────────────────────────────────────────────────────
            "init" => match args.trim() {
                // /init config — write a default ragent.json to the global
                // config directory (~/.config/ragent/ragent.json on Linux,
                // ~/Library/Application Support/ragent/ragent.json on macOS,
                // %APPDATA%\ragent\ragent.json on Windows).
                "config" => {
                    let config_dir = match dirs::config_dir() {
                        Some(d) => d.join("ragent"),
                        None => {
                            self.append_assistant_text(
                                "From: /init config\n❌ **Cannot determine the global config \
                                 directory for this platform.**\n\nPlease set the \
                                 `XDG_CONFIG_HOME` environment variable (Linux) or ensure \
                                 the platform config directory is available.",
                            );
                            self.push_log_no_agent(
                                LogLevel::Error,
                                "init config: no config dir".to_string(),
                            );
                            self.status = "init config: error".to_string();
                            return;
                        }
                    };
                    let config_path = config_dir.join("ragent.json");

                    if config_path.exists() {
                        self.append_assistant_text(&format!(
                            "From: /init config\n⚠️  **A global config already exists at:**\n\
                             \x20  `{}`\n\n\
                             No changes were made. Edit the file directly to modify your \
                             configuration.",
                            config_path.display()
                        ));
                        self.push_log_no_agent(
                            LogLevel::Info,
                            format!(
                                "init config: skipped (file exists at {})",
                                config_path.display()
                            ),
                        );
                        self.status = "init config: already exists".to_string();
                        return;
                    }

                    // Create the config directory and write a default config.
                    let default_config = ragent_config::Config::default();
                    let json = match serde_json::to_string_pretty(&default_config) {
                        Ok(j) => j,
                        Err(e) => {
                            self.append_assistant_text(&format!(
                                "From: /init config\n❌ **Failed to serialise default config: \
                                 {}**",
                                e
                            ));
                            self.push_log_no_agent(
                                LogLevel::Error,
                                format!("init config: serialise error: {e}"),
                            );
                            self.status = "init config: error".to_string();
                            return;
                        }
                    };

                    if let Err(e) = std::fs::create_dir_all(&config_dir) {
                        self.append_assistant_text(&format!(
                            "From: /init config\n❌ **Failed to create config directory `{}`: \
                             {}**",
                            config_dir.display(),
                            e
                        ));
                        self.push_log_no_agent(
                            LogLevel::Error,
                            format!("init config: create dir error: {}", e),
                        );
                        self.status = "init config: error".to_string();
                        return;
                    }

                    if let Err(e) = std::fs::write(&config_path, &json) {
                        self.append_assistant_text(&format!(
                            "From: /init config\n❌ **Failed to write config file `{}`: {}**",
                            config_path.display(),
                            e
                        ));
                        self.push_log_no_agent(
                            LogLevel::Error,
                            format!("init config: write error: {}", e),
                        );
                        self.status = "init config: error".to_string();
                        return;
                    }

                    self.append_assistant_text(&format!(
                        "From: /init config\n✅ **Default config created at:**\n\
                         \x20  `{}`\n\n\
                         This file contains all default settings. Edit it to configure \
                         providers, agents, permissions, memory, and more.",
                        config_path.display()
                    ));
                    self.push_log_no_agent(
                        LogLevel::Info,
                        format!(
                            "init config: wrote default config to {}",
                            config_path.display()
                        ),
                    );
                    self.status = "init config: created".to_string();
                }
                _ => {
                    let sid = self.session_id.clone().unwrap_or_default();
                    self.append_assistant_text(
                        "From: /init\n🔍 **Analysing project…**\n\n\
                         The explore agent will examine the project structure, README, build \
                         files, and test layout, then write a summary to \
                         `.ragent/memory/PROJECT_ANALYSIS.md`. Future sessions will \
                         automatically load this context.",
                    );
                    self.push_log_no_agent(
                        LogLevel::Info,
                        "init: starting project analysis".to_string(),
                    );

                    // Find the explore agent and dispatch the analysis task directly
                    // (no agent-stack push — init runs as a one-shot subagent that writes
                    // memory).
                    let explore_agent = self
                        .cycleable_agents
                        .iter()
                        .find(|a| a.name == "explore")
                        .cloned();

                    let mut agent = explore_agent.unwrap_or_else(|| {
                        // Fallback: use current agent with a suitable prompt
                        self.agent_info.clone()
                    });

                    self.apply_selected_model_and_thinking(&mut agent);

                    // Allow file writes so the agent can call memory_write
                    agent.permission = ragent_agent::agent::default_permissions();

                    let task = "\
You are performing a one-time project analysis to build persistent memory for this codebase.\n\n\
Analyse the following aspects of the project:\n\
1. Programming language(s), frameworks, and key dependencies\n\
2. Overall architecture and module structure\n\
3. Entry points and main execution flow\n\
4. Build system and how to build/test the project\n\
5. Key conventions: naming, error handling, testing patterns\n\
6. Important files a developer should know about\n\n\
After your analysis, call the `memory_write` tool with:\n\
- scope: \"project\"\n\
- path: \"PROJECT_ANALYSIS.md\"\n\
- content: a well-structured markdown summary of your findings\n\n\
Be concise but comprehensive. This will be injected into future agent sessions automatically.\
"
                    .to_string();

                    let msg = Message::user_text(&sid, &task);
                    self.messages.push(msg);

                    let processor = self.session_processor.clone();
                    let flag = Arc::new(AtomicBool::new(false));
                    self.cancel_flag = Some(flag.clone());
                    self.is_processing = true;
                    self.status = "init: analysing project…".to_string();

                    let event_bus = self.event_bus.clone();
                    tokio::spawn(async move {
                        if let Err(e) = processor.process_message(&sid, &task, &agent, flag).await {
                            tracing::warn!(error = %e, "init: analysis failed");
                            event_bus.publish(ragent_agent::event::Event::AgentError {
                                session_id: sid,
                                error: format!("init analysis failed: {e}"),
                            });
                        }
                    });
                }
            },
            "clear" => {
                self.messages.clear();
                self.scroll_offset = 0;
                self.tool_step_map.clear();
                self.last_step_per_session.clear();
                self.substep_counter_per_session.clear();
                ragent_agent::agent::clear_prompt_context_cache();
                self.status = "messages cleared".to_string();
                self.push_log_no_agent(LogLevel::Info, "Message history cleared".to_string());
            }
            "browse_refresh" => {
                self.refresh_project_files_cache();
                self.status = format!(
                    "browse index refreshed ({})",
                    self.project_files_cache_count
                );
                self.push_log_no_agent(
                    LogLevel::Info,
                    format!(
                        "@ picker index refreshed ({} entries)",
                        self.project_files_cache_count
                    ),
                );
            }
            "cancel" => {
                if args.is_empty() {
                    self.status = "⚠ Please provide a task ID prefix: /cancel <id>".to_string();
                    self.push_log_no_agent(LogLevel::Warn, "No task ID provided".to_string());
                    return;
                }

                if self
                    .active_bench_task_id
                    .as_deref()
                    .is_some_and(|task_id| task_id.starts_with(args))
                    && let Some(flag) = &self.active_bench_cancel
                {
                    flag.store(true, Ordering::Relaxed);
                    self.status = "⏳ bench: cancellation requested".to_string();
                    self.push_log_no_agent(
                        LogLevel::Info,
                        format!("Benchmark cancellation requested for {}", args),
                    );
                    return;
                }

                if let Some(task) = self.active_tasks.iter().find(|t| t.id.starts_with(args)) {
                    let task_id = task.id.clone();
                    let agent = task.agent_name.clone();
                    if let Some(idx) = self.active_tasks.iter().position(|t| t.id == task_id) {
                        self.active_tasks.remove(idx);
                    }
                    self.status = format!(
                        "Cancelled task {} ({})",
                        &task_id[..8.min(task_id.len())],
                        agent
                    );
                    self.push_log_no_agent(
                        LogLevel::Info,
                        format!(
                            "Task cancelled: {}... ({})",
                            &task_id[..8.min(task_id.len())],
                            agent
                        ),
                    );
                } else {
                    self.status = format!("No task found with ID starting with '{}'", args);
                    self.push_log_no_agent(LogLevel::Warn, format!("Task not found: {}", args));
                }
            }
            "bench" => match ragent_bench::parse_bench_command(args) {
                Ok(ragent_bench::BenchCommand::Help) => {
                    self.append_assistant_text(
                        "From: /bench\nUsage: `/bench list` | `/bench init <suite-or-all-or-full> [--full] [--language LANG] [--force-download] [--verify-only]` | `/bench show` | `/bench run <suite-or-profile-or-all> [--limit N|--cap N] [--samples K] [--subset NAME] [--release VERSION] [--scenario NAME] [--language LANG] [--temperature F] [--top-p F] [--max-tokens N] [--deterministic] [--since YYYY-MM-DD] [--until YYYY-MM-DD] [--resume] [--no-exec] [--yes]` | `/bench status` | `/bench open last` | `/bench cancel`"
                    );
                    self.status = "bench help".to_string();
                }
                Ok(ragent_bench::BenchCommand::List) => {
                    self.append_assistant_text(&self.render_bench_list());
                    self.status = "bench list".to_string();
                }
                Ok(ragent_bench::BenchCommand::Show) => {
                    self.append_assistant_text(&self.render_bench_show());
                    self.status = "bench show".to_string();
                }
                Ok(ragent_bench::BenchCommand::Status) => {
                    self.append_assistant_text(&self.render_bench_status());
                    self.status = "bench status".to_string();
                }
                Ok(ragent_bench::BenchCommand::OpenLast) => {
                    self.append_assistant_text(&self.render_bench_open_last());
                    self.status = "bench open last".to_string();
                }
                Ok(ragent_bench::BenchCommand::Cancel) => {
                    if let Some(flag) = &self.active_bench_cancel {
                        flag.store(true, Ordering::Relaxed);
                        self.status = "⏳ bench: cancellation requested".to_string();
                        self.append_assistant_text(
                            "From: /bench cancel\nCancellation requested for the active benchmark run.\n\nUse `/bench status` to watch it shut down.",
                        );
                    } else {
                        self.status = "No active benchmark run".to_string();
                        self.append_assistant_text("From: /bench cancel\nNo active benchmark run.");
                    }
                }
                Ok(ragent_bench::BenchCommand::Init {
                    target,
                    mode,
                    language,
                    force_download,
                    verify_only,
                }) => {
                    let project_root = match std::env::current_dir() {
                        Ok(path) => path,
                        Err(e) => {
                            self.status = format!("⚠ Could not resolve current directory: {e}");
                            return;
                        }
                    };
                    match ragent_bench::init_target_with_progress(
                        &project_root,
                        &target,
                        mode,
                        language.as_deref(),
                        force_download,
                        verify_only,
                        |event| {
                            self.force_new_message = true;
                            self.append_assistant_text(&self.render_bench_init_event(&event));
                        },
                    ) {
                        Ok(outcomes) => {
                            let heading = if verify_only {
                                "verified benchmark target."
                            } else {
                                "initialized benchmark target."
                            };
                            let mut message = format!("From: /bench init\n✅ {heading}\n\n");
                            for init in &outcomes {
                                let init_action = if init.verified_only {
                                    "verified"
                                } else if matches!(init.mode, ragent_bench::BenchInitMode::Full) {
                                    "initialized full dataset for"
                                } else {
                                    "initialized"
                                };
                                message.push_str(&format!(
                                    "- **`{}`** [{}] {} at `{}` (`{}`, {} case(s))\n",
                                    init.suite.id,
                                    init.language,
                                    init_action,
                                    init.data_root.display(),
                                    init.manifest.revision,
                                    init.manifest.case_count
                                ));
                            }
                            self.force_new_message = true;
                            self.append_assistant_text(&message);
                            let status_target = match &target {
                                ragent_bench::BenchInitTarget::All => "all".to_string(),
                                ragent_bench::BenchInitTarget::Full => "full".to_string(),
                                ragent_bench::BenchInitTarget::Suite(id) => id.clone(),
                            };
                            self.status = format!("bench init: {status_target}");
                        }
                        Err(e) => {
                            self.status = format!("⚠ bench init failed: {e}");
                            self.force_new_message = true;
                            self.append_assistant_text(&format!("From: /bench init\n❌ {e}"));
                        }
                    }
                }
                Ok(ragent_bench::BenchCommand::Run { target, options }) => {
                    self.start_bench_run(raw, target, options);
                }
                Err(e) => {
                    self.status = format!("⚠ {e}");
                    self.append_assistant_text(&format!("From: /bench\n❌ {e}"));
                }
            },
            "compact" => {
                let _ = self.start_compaction(false);
            }
            // `/compress` is a deprecated alias for `/compact` (FR-009).
            // It forwards to the same LLM summarisation path so existing
            // muscle memory keeps working, but `/compact` is the canonical
            // command documented in `/help`.
            "compress" => {
                self.push_log_no_agent(
                    LogLevel::Info,
                    "/compress is a deprecated alias for /compact".to_string(),
                );
                let _ = self.start_compaction(false);
            }
            "cost" => {
                let Some(output) = self.cost_summary() else {
                    self.append_assistant_text(
                        "From: /cost\nNo completed LLM responses yet for this session.\n",
                    );

                    self.status = "cost unavailable".to_string();
                    return;
                };
                self.append_assistant_text(&output);

                self.status = "cost summary".to_string();
            }
            "startup" => match &self.startup_timings {
                Some(timings) => {
                    self.append_assistant_text(&timings.format_report());
                    self.status = "startup: timings shown".to_string();
                }
                None => {
                    self.append_assistant_text(
                        "From: /startup\n⚠ No startup timings recorded for this session.",
                    );
                    self.status = "startup: unavailable".to_string();
                }
            },
            "help" => {
                let mut help_lines = String::from("From: /help\nAvailable commands:\n\n```\n");
                for cmd_def in SLASH_COMMANDS {
                    help_lines.push_str(&format!(
                        "  /{:<18} {}\n",
                        cmd_def.trigger, cmd_def.description
                    ));
                }

                // Append user-invocable skills
                let working_dir = std::env::current_dir().unwrap_or_default();
                let skill_dirs = ragent_agent::Config::load()
                    .map(|c| c.skill_dirs)
                    .unwrap_or_default();
                let registry = ragent_agent::skill::SkillRegistry::load(&working_dir, &skill_dirs);
                let skills = registry.list_user_invocable();
                if !skills.is_empty() {
                    help_lines.push_str("\nSkills:\n");
                    for skill in &skills {
                        let desc = skill.description.as_deref().unwrap_or("(no description)");
                        let hint = skill
                            .argument_hint
                            .as_deref()
                            .map(|h| format!(" {h}"))
                            .unwrap_or_default();
                        help_lines.push_str(&format!(
                            "  /{:<18} {}\n",
                            format!("{}{}", skill.name, hint),
                            desc
                        ));
                    }
                }
                help_lines.push_str("```\n");
                self.append_assistant_text(&help_lines);

                self.status = "help".to_string();
            }
            "opt" => {
                // /opt help => show markdown table of available optimization methods
                if args.is_empty() || args == "help" {
                    let table = OptMethod::help_table();
                    self.append_assistant_text(&format!("From: /opt help\n\n{}", table));

                    self.status = "opt help".to_string();
                    return;
                }

                // /opt <method> <prompt>
                let (method_str, rest) = args
                    .split_once(char::is_whitespace)
                    .map_or((args, ""), |(m, r)| (m, r.trim()));

                if rest.is_empty() {
                    self.status = "⚠ Please provide a prompt: /opt <method> <prompt>".to_string();
                    return;
                }

                let method = match method_str.parse::<OptMethod>() {
                    Ok(m) => m,
                    Err(_) => {
                        self.status = format!("⚠ Unknown optimization method: {}", method_str);
                        self.push_log_no_agent(
                            LogLevel::Warn,
                            format!("opt: unknown method '{}'", method_str),
                        );
                        return;
                    }
                };

                // Resolve provider / model from session config
                let (provider_id, model_id) = match self
                    .selected_model
                    .as_deref()
                    .and_then(|s| s.split_once('/'))
                    .map(|(p, m)| (p.to_string(), m.to_string()))
                {
                    Some(pair) => pair,
                    None => {
                        self.status =
                            "⚠ /opt requires a configured model (use /provider)".to_string();
                        return;
                    }
                };

                let registry = Arc::clone(&self.provider_registry);
                let storage = Arc::clone(&self.storage);
                let opt_result = Arc::clone(&self.opt_result);
                let user_prompt = rest.to_string();
                let method_name = method.name().to_string();

                self.status = format!("⏳ opt/{}: optimizing…", method_name);

                tokio::spawn(async move {
                    let completer = RagentCompleter {
                        registry,
                        storage,
                        provider_id,
                        model_id,
                    };
                    let outcome = optimize(method, &user_prompt, &completer)
                        .await
                        .map(|text| format!("[opt: {}]\n\n{}", method_name, text))
                        .map_err(|e| e.to_string());
                    if let Ok(mut guard) = opt_result.lock() {
                        *guard = Some(outcome);
                    } else {
                        tracing::error!("opt_result mutex poisoned, result dropped");
                    }
                });
            }
            "inputdiag" => {
                let selection = self
                    .text_selection
                    .as_ref()
                    .map(|s| format!("{:?} {:?}->{:?}", s.pane, s.anchor, s.endpoint))
                    .unwrap_or_else(|| "none".to_string());
                let context_menu = self
                    .context_menu
                    .as_ref()
                    .map(|m| format!("{:?} selected={}", m.pane, m.selected))
                    .unwrap_or_else(|| "none".to_string());
                let diag = format!(
                    "From: /inputdiag\n\
                                       Input diagnostics:\n\
                                         screen: {:?}\n\
                                         input chars: {}\n\
                                         input cursor: {}\n\
                                         slash menu: {}\n\
                                         file menu: {}\n\
                                         history picker: {}\n\
                                         selection: {}\n\
                                         context menu: {}\n\
                                         message area: {:?}\n\
                                         log area: {:?}\n\
                                         input area: {:?}\n\
                                         browse cache cwd: {:?}\n\
                                         browse cache entries: {}\n\
                                         browse cache refreshed: {:?}\n\
                                         browse menu state: {}",
                    self.current_screen,
                    self.input_len_chars(),
                    self.input_cursor,
                    self.slash_menu.is_some(),
                    self.file_menu.is_some(),
                    self.history_picker.is_some(),
                    selection,
                    context_menu,
                    self.message_area,
                    self.log_area,
                    self.input_area,
                    self.project_files_cache_cwd,
                    self.project_files_cache_count,
                    self.project_files_cache_refreshed_at,
                    self.file_menu
                        .as_ref()
                        .map(|m| format!(
                            "query='{}' dir={:?} selected={} offset={} results={}",
                            m.query,
                            m.current_dir,
                            m.selected,
                            m.scroll_offset,
                            m.matches.len()
                        ))
                        .unwrap_or_else(|| "none".to_string())
                );
                self.append_assistant_text(&diag);

                self.status = "inputdiag".to_string();
            }
            "log" => {
                self.show_log = !self.show_log;
                if self.show_log {
                    // Entering log mode: dismiss the other side panels so only
                    // one occupies the side column (FR-012).
                    self.show_profile = false;
                    self.show_todo = false;
                    self.show_memory = false;
                    self.show_telemetry = false;
                }
                self.status = if self.show_log {
                    "log panel visible".to_string()
                } else {
                    "log panel hidden".to_string()
                };
            }
            "todo" => {
                // `/todo` slash alias — toggles the TODO side panel (FR-010,
                // optional). Mirrors the `/log` alias above and the Alt+T
                // InputAction::ToggleTodo handler in input_handler.rs. The
                // actual TODO mutation commands (`/todo add`, etc.) are handled
                // by a different dispatch path; this arm only fires when `args`
                // is empty, so it never shadows those subcommands.
                if args.is_empty() {
                    self.show_todo = !self.show_todo;
                    if self.show_todo {
                        self.show_log = false;
                        self.show_profile = false;
                        self.show_memory = false;
                        self.show_telemetry = false;
                    }
                    self.status = if self.show_todo {
                        "todo panel visible".to_string()
                    } else {
                        "todo panel hidden".to_string()
                    };
                }
            }
            "telemetry_panel" => {
                // `/telemetry_panel` slash alias — toggles the Telemetry side
                // panel. Mirrors the `/todo` alias and the Alt+O
                // InputAction::ToggleTelemetry handler. This is purely a UI
                // toggle; the `/telemetry counters` command still prints the
                // same values into the chat transcript.
                self.show_telemetry = !self.show_telemetry;
                if self.show_telemetry {
                    self.show_log = false;
                    self.show_profile = false;
                    self.show_todo = false;
                    self.show_memory = false;
                }
                self.status = if self.show_telemetry {
                    "telemetry panel visible".to_string()
                } else {
                    "telemetry panel hidden".to_string()
                };
            }
            "profile" => match args {
                "on" => {
                    self.set_profile_panel_enabled(true);
                }
                "off" => {
                    self.set_profile_panel_enabled(false);
                }
                _ => {
                    self.append_assistant_text(
                        "From: /profile\nUsage: `/profile on` or `/profile off`\n",
                    );
                    self.status = "profile usage".to_string();
                }
            },
            // PERFPLAN F-4: `/perf` is an alias for `/profile` that toggles
            // the agent-loop profiler panel reading
            // `AgentLoopProfiler::snapshot()`. The panel itself already
            // existed as `/profile`; the alias matches the PERFPLAN wording.
            "perf" => match args {
                "on" => {
                    self.set_profile_panel_enabled(true);
                }
                "off" => {
                    self.set_profile_panel_enabled(false);
                }
                _ => {
                    self.append_assistant_text(
                        "From: /perf\nUsage: `/perf on` or `/perf off` (alias for /profile)\n",
                    );
                    self.status = "perf usage".to_string();
                }
            },
            "llmstats" => {
                let Some(model_ref) = self.active_model_ref_string() else {
                    self.status = "⚠ No active model selected".to_string();
                    self.push_log_no_agent(LogLevel::Warn, "llmstats: no active model".to_string());
                    return;
                };

                let Some(summary) = self.llm_stats_summary() else {
                    self.append_assistant_text(&format!(
                        "From: /llmstats\nNo completed LLM responses yet for {}.\n",
                        model_ref
                    ));

                    self.status = "llm stats unavailable".to_string();
                    return;
                };

                let output = format!(
                    "From: /llmstats\n\
                     Model: {}\n\
                     Samples: {}\n\
                     Average round-trip: {:.1} ms\n\
                     Average prompt parsing tokens/sec: {:.2}\n\
                     Average output tokens/sec: {:.2}\n",
                    model_ref,
                    summary.samples,
                    summary.avg_elapsed_ms,
                    summary.avg_prompt_tps,
                    summary.avg_output_tps
                );
                self.append_assistant_text(&output);

                self.status = "llm stats".to_string();
            }
            "history" => {
                if self.input_history.is_empty() {
                    self.status = "No input history yet".to_string();
                } else {
                    // Show newest entries first
                    let entries: Vec<String> = self.input_history.iter().rev().cloned().collect();
                    self.history_picker = Some(crate::app::state::HistoryPickerState {
                        entries,
                        selected: 0,
                        scroll_offset: 0,
                    });
                    self.input.clear();
                    self.input_cursor = 0;
                }
            }
            "model" => match args.trim() {
                "" => {
                    // If a provider is already configured, jump straight to
                    // its model list; otherwise show the provider picker.
                    if let Some(cp) = self.configured_provider.clone() {
                        if cp.id == "azure_resource" {
                            self.refresh_provider();
                            let provider =
                                ragent_agent::provider::azure_resource::AzureResourceProvider::new(
                                );
                            let entries = provider.entries();
                            if entries.is_empty() {
                                self.provider_setup = Some(ProviderSetupStep::SelectProvider {
                                    selected: 0,
                                    force_key_entry: false,
                                });
                                return;
                            }
                            let mut selected = 0usize;
                            if let Ok(Some(last)) =
                                self.storage.get_setting("azure_resource_last_selection")
                            {
                                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&last)
                                {
                                    if let Some(last_id) = parsed.get("id").and_then(|v| v.as_str())
                                    {
                                        if let Some(idx) =
                                            entries.iter().position(|e| e.id == last_id)
                                        {
                                            selected = idx;
                                        }
                                    }
                                }
                            }
                            self.provider_setup = Some(ProviderSetupStep::SelectAzureResource {
                                entries,
                                selected,
                                error: None,
                            });
                            return;
                        }
                        if cp.id == "router" {
                            let providers =
                                Self::get_configured_providers_for_router(&self.storage);
                            if providers.is_empty() {
                                self.provider_setup = Some(ProviderSetupStep::SelectProvider {
                                    selected: 0,
                                    force_key_entry: false,
                                });
                            } else {
                                self.provider_setup =
                                    Some(self.seeded_router_setup_step(providers));
                            }
                            return;
                        }
                        self.refresh_provider();
                        self.provider_setup = Some(ProviderSetupStep::LoadingModels {
                            provider_id: cp.id.clone(),
                            provider_name: cp.name.clone(),
                        });
                        self.start_model_discovery(cp.id, cp.name);
                    } else {
                        self.provider_setup = Some(ProviderSetupStep::SelectProvider {
                            selected: 0,
                            force_key_entry: false,
                        });
                    }
                }
                "show" => {
                    if !self.ensure_session() {
                        self.status = "⚠ Failed to create session".to_string();
                    } else if let Some(report) = self.active_model_metadata_report() {
                        self.append_assistant_text(&report);
                        self.status = "active model metadata".to_string();
                    } else {
                        self.status = "⚠ No active model selected".to_string();
                    }
                }
                _ => {
                    self.status = "Usage: /model [show]".to_string();
                }
            },
            "thinking" => {
                if self.selected_model.is_none() {
                    self.status = "⚠ No model selected — use /model to choose".to_string();
                    return;
                }

                let supported = self.active_thinking_levels();
                let requested = args.trim();
                if requested.is_empty() {
                    let current = self
                        .effective_thinking_level_for_agent(&self.agent_info)
                        .map(Self::thinking_level_display)
                        .unwrap_or("unknown");
                    self.append_assistant_text(&format!(
                        "From: /thinking\nCurrent: `{}`\nSupported: `{}`\n",
                        current,
                        Self::format_thinking_levels(&supported)
                    ));
                    self.status = "thinking".to_string();
                    return;
                }

                let Some(level) = Self::parse_thinking_level_setting(requested) else {
                    self.status = "Usage: /thinking [auto|off|low|medium|high]".to_string();
                    return;
                };

                if supported.is_empty() && level != ThinkingLevel::Off {
                    self.status =
                        "⚠ Active model does not support configurable thinking".to_string();
                    return;
                }
                if !supported.is_empty() && !supported.contains(&level) {
                    self.status = format!(
                        "⚠ Thinking level '{}' is not supported by the active model",
                        Self::thinking_level_display(level)
                    );
                    return;
                }

                self.persist_selected_thinking_level(level);
                self.status = format!("thinking: {}", Self::thinking_level_display(level));
            }
            "provider" => match args.trim() {
                "show" => {
                    let mut providers = Self::get_configured_providers(&self.storage);
                    if providers.is_empty() {
                        self.status = "⚠ No configured providers".to_string();
                        self.push_log_no_agent(
                            LogLevel::Warn,
                            "provider show: no configured providers".to_string(),
                        );
                    } else {
                        // Include the router virtual provider if it has a saved
                        // configuration so the cluster can be viewed inline.
                        if self.load_raw_router_config().is_some() {
                            providers.push(ConfiguredProvider {
                                id: "router".to_string(),
                                name: "Model Router".to_string(),
                                source: ProviderSource::Database,
                            });
                        }
                        self.provider_setup = Some(ProviderSetupStep::ShowProviderConfig {
                            providers,
                            selected: 0,
                        });
                    }
                }
                "router" => {
                    let providers = Self::get_configured_providers_for_router(&self.storage);
                    if providers.is_empty() {
                        self.status = "⚠ No concrete providers — configure one first".to_string();
                        self.push_log_no_agent(
                            LogLevel::Warn,
                            "provider router: no concrete providers configured".to_string(),
                        );
                    } else {
                        self.provider_setup = Some(self.seeded_router_setup_step(providers));
                    }
                }
                "" => {
                    self.provider_setup = Some(ProviderSetupStep::SelectProvider {
                        selected: 0,
                        force_key_entry: true,
                    });
                }
                _ => {
                    self.append_assistant_text(
                        "From: /provider
  Usage: /provider [show|router]
  ",
                    );
                    self.status = "provider: usage".to_string();
                }
            },
            "provider_reset" => {
                self.provider_setup = Some(ProviderSetupStep::ResetProvider { selected: 0 });
            }
            "quit" | "exit" => {
                self.is_running = false;
            }
            "reload" => {
                let sub = args.split_whitespace().next().unwrap_or("all");
                let do_agents = matches!(sub, "all" | "agents");
                let do_config = matches!(sub, "all" | "config");
                let do_mcp = matches!(sub, "all" | "mcp");
                let do_skills = matches!(sub, "all" | "skills");

                let mut report = String::from("From: /reload\n\n");

                // ── reload agents ──────────────────────────────────────────────────
                if do_agents {
                    let cwd_path = std::env::current_dir().unwrap_or_default();
                    let builtin_agents = ragent_agent::agent::create_builtin_agents();
                    let builtin_names: std::collections::HashSet<String> =
                        builtin_agents.iter().map(|a| a.name.clone()).collect();

                    let (new_defs, mut diags) =
                        ragent_agent::agent::custom::load_custom_agents(&cwd_path);

                    // Rebuild cycleable list: builtins (non-hidden) + custom
                    let mut new_cycleable: Vec<_> =
                        builtin_agents.into_iter().filter(|a| !a.hidden).collect();
                    for def in &new_defs {
                        let mut info = def.agent_info.clone();
                        if builtin_names.contains(&info.name) {
                            let new_name = format!("custom:{}", info.name);
                            diags.push(format!(
                                "custom agent '{}' collides with a built-in — loaded as '{}'",
                                info.name, new_name
                            ));
                            info.name = new_name;
                        }
                        if !info.hidden {
                            new_cycleable.push(info);
                        }
                    }

                    let prev_count = self.custom_agent_defs.len();
                    self.custom_agent_defs = new_defs;
                    self.custom_agent_diagnostics = diags.clone();
                    // Preserve current_agent_index if possible
                    let current_name = self.agent_name.clone();
                    self.current_agent_index = new_cycleable
                        .iter()
                        .position(|a| a.name == current_name)
                        .unwrap_or(0);
                    self.cycleable_agents = new_cycleable;

                    for d in &diags {
                        self.push_log_no_agent(LogLevel::Warn, format!("[reload agents] {}", d));
                    }
                    report.push_str(&format!(
                        "✓ Agents reloaded — {} custom agent(s) (was {})\n",
                        self.custom_agent_defs.len(),
                        prev_count,
                    ));
                    self.push_log_no_agent(
                        LogLevel::Info,
                        format!(
                            "reload agents: {} custom agent(s) loaded",
                            self.custom_agent_defs.len()
                        ),
                    );
                }

                // ── reload config ──────────────────────────────────────────────────
                if do_config {
                    match ragent_agent::Config::load() {
                        Ok(cfg) => {
                            // Refresh cached provider and model selections
                            self.configured_provider = Self::detect_provider(&self.storage);
                            self.selected_model =
                                self.storage.get_setting("selected_model").ok().flatten();
                            self.selected_model_ctx_window = self
                                .storage
                                .get_setting("selected_model_ctx_window")
                                .ok()
                                .flatten()
                                .and_then(|s| s.parse::<usize>().ok());
                            self.selected_thinking_level =
                                Self::load_persisted_thinking_level(self.storage.as_ref());
                            self.code_index_enabled = cfg.code_index.enabled;
                            self.sync_tool_visibility_from_config(&cfg);
                            report.push_str("✓ Config reloaded (ragent.json)\n");
                            self.push_log_no_agent(
                                LogLevel::Info,
                                "reload config: ragent.json reloaded".to_string(),
                            );
                        }
                        Err(e) => {
                            report.push_str(&format!("✗ Config reload failed: {}\n", e));
                            self.push_log_no_agent(
                                LogLevel::Warn,
                                format!("reload config failed: {}", e),
                            );
                        }
                    }
                }

                // ── reload mcp ─────────────────────────────────────────────────────
                if do_mcp {
                    match ragent_agent::Config::load() {
                        Ok(cfg) => {
                            // Rebuild the display list from config, preserving connected status
                            let mut new_servers: Vec<ragent_agent::mcp::McpServer> = Vec::new();
                            for (id, mcp_cfg) in &cfg.mcp {
                                let existing_status = self
                                    .mcp_servers
                                    .iter()
                                    .find(|s| &s.id == id)
                                    .map(|s| s.status.clone())
                                    .unwrap_or(if mcp_cfg.disabled {
                                        ragent_agent::mcp::McpStatus::Disabled
                                    } else {
                                        ragent_agent::mcp::McpStatus::Disabled
                                    });
                                let existing_tools = self
                                    .mcp_servers
                                    .iter()
                                    .find(|s| &s.id == id)
                                    .map(|s| s.tools.clone())
                                    .unwrap_or_default();
                                new_servers.push(ragent_agent::mcp::McpServer {
                                    id: id.clone(),
                                    config: mcp_cfg.clone(),
                                    status: existing_status,
                                    tools: existing_tools,
                                });
                            }
                            let prev = self.mcp_servers.len();
                            self.mcp_servers = new_servers;
                            report.push_str(&format!(
                                "✓ MCP reloaded — {} server(s) in config (was {})\n",
                                self.mcp_servers.len(),
                                prev,
                            ));
                            self.push_log_no_agent(
                                LogLevel::Info,
                                format!(
                                    "reload mcp: {} server(s) in config",
                                    self.mcp_servers.len()
                                ),
                            );
                        }
                        Err(e) => {
                            report.push_str(&format!("✗ MCP reload failed: {}\n", e));
                            self.push_log_no_agent(
                                LogLevel::Warn,
                                format!("reload mcp failed: {}", e),
                            );
                        }
                    }
                }

                // ── reload skills ──────────────────────────────────────────────────
                if do_skills {
                    // Skills are loaded on-demand from disk each time they are needed;
                    // there is no persistent cache to clear.  Just confirm to the user.
                    report.push_str(
                        "✓ Skills will be reloaded from disk on next use (no cache to clear)\n",
                    );
                    self.push_log_no_agent(
                        LogLevel::Info,
                        "reload skills: confirmed (on-demand)".to_string(),
                    );
                }

                if !matches!(sub, "all" | "agents" | "config" | "mcp" | "skills") {
                    report.push_str(&format!(
                        "Unknown subcommand '{}'. Usage: /reload [all|config|mcp|skills|agents]\n",
                        sub
                    ));
                }

                self.append_assistant_text(&report);

                self.status = "reload".to_string();
                // Reload bash lists alongside other config
                ragent_agent::bash_lists::load_from_config();
                // Reload directory lists alongside other config
                ragent_agent::dir_lists::load_from_config();
            }
            "resume" => {
                if !self.agent_halted {
                    self.status = "Nothing to resume — agent was not halted".to_string();
                    self.push_log_no_agent(LogLevel::Warn, "Nothing to resume".to_string());
                    return;
                }
                if self.session_id.is_none() {
                    self.status = "No active session".to_string();
                    return;
                }

                self.agent_halted = false;
                let Some(sid) = self.session_id.clone() else {
                    self.status = "No active session".to_string();
                    return;
                };
                let resume_text = "You were previously interrupted by the user. Continue the task from where you left off.";
                let msg = Message::user_text(&sid, resume_text);
                self.messages.push(msg);
                self.set_status_working("processing");
                self.push_log_no_agent(LogLevel::Info, "Resuming halted agent".to_string());

                let mut agent = self.agent_info.clone();
                self.apply_selected_model_and_thinking(&mut agent);

                let processor = self.session_processor.clone();
                let flag = Arc::new(AtomicBool::new(false));
                self.cancel_flag = Some(flag.clone());
                tokio::spawn(async move {
                    if let Err(e) = processor
                        .process_message(&sid, resume_text, &agent, flag)
                        .await
                    {
                        tracing::debug!(error = %e, "Failed to resume agent");
                    }
                });
            }
            "system" => {
                if args.is_empty() {
                    // Show current system prompt
                    if let Some(ref prompt) = self.agent_info.prompt {
                        self.append_assistant_text(&format!(
                            "From: /system\nCurrent system prompt:\n{prompt}"
                        ));
                    } else {
                        self.status = "No system prompt set".to_string();
                    }
                } else {
                    self.agent_info.prompt = Some(args.to_string());
                    self.status = "system prompt updated".to_string();
                    self.push_log_no_agent(
                        LogLevel::Info,
                        format!("System prompt set ({} chars)", args.len()),
                    );
                }
            }
            "tools" => {
                let parts: Vec<&str> = args.split_whitespace().collect();
                match parts.as_slice() {
                    [] | ["show"] => {
                        self.append_assistant_text(&self.render_tool_visibility_table());
                        self.status = "tools".to_string();
                    }
                    ["help"] | ["usage"] => {
                        self.append_assistant_text(
                                                  "From: /tools\nUsage: `/tools` | `/tools show` | `/tools help` | `/tools <switch>` | `/tools <switch> on|off`\n\nValid switches: `office`, `github`, `gitlab`, `teams`, `agents`, `plan`, `codeindex`, `masterfetch`, `browser`.",
                                              );
                        self.status = "tools help".to_string();
                    }
                    [switch] => {
                        if let Some(enabled) = self.tool_visibility_state(switch) {
                            self.append_assistant_text(&format!(
                                "From: /tools\n`{switch}` is currently **{}**.",
                                if enabled { "on" } else { "off" }
                            ));
                            self.status = "tools".to_string();
                        } else {
                            self.append_assistant_text(
                                                          "From: /tools\n⚠ Invalid switch. Use one of: `office`, `github`, `gitlab`, `teams`, `agents`, `plan`, `codeindex`, `masterfetch`, `browser`.",
                                                      );
                            self.status = "tools error".to_string();
                        }
                    }
                    [switch, state] => {
                        let enabled = match *state {
                            "on" | "enable" => true,
                            "off" | "disable" => false,
                            _ => {
                                self.append_assistant_text(
                                    "From: /tools\n⚠ Usage: `/tools <switch> on|off`.",
                                );
                                self.status = "tools error".to_string();
                                return;
                            }
                        };

                        if !self.set_tool_visibility_state(switch, enabled) {
                            self.append_assistant_text(
                                                          "From: /tools\n⚠ Invalid switch. Use one of: `office`, `github`, `gitlab`, `teams`, `agents`, `plan`, `codeindex`, `masterfetch`, `browser`.",
                                                      );
                            self.status = "tools error".to_string();
                            return;
                        }
                        let mut cfg = ragent_agent::Config::load().unwrap_or_default();
                        cfg.tool_visibility = self.tool_visibility.clone();
                        self.sync_tool_visibility_from_config(&cfg);

                        match cfg.save_to_source() {
                            Ok(()) => {
                                // P-2: invalidate the cached config so the next turn
                                // picks up the newly-saved file.
                                self.session_processor.invalidate_config_cache();
                                self.append_assistant_text(&format!(
                                    "From: /tools\n✅ `{switch}` visibility is now **{}**.",
                                    if enabled { "on" } else { "off" }
                                ));
                                self.status = format!(
                                    "tools: {switch} {}",
                                    if enabled { "on" } else { "off" }
                                );
                            }
                            Err(e) => {
                                self.append_assistant_text(&format!(
                                    "From: /tools\n⚠ `{switch}` visibility changed to **{}**, but saving config failed: {e}",
                                    if enabled { "on" } else { "off" }
                                ));
                                self.status = format!(
                                    "tools: {switch} {} (unsaved)",
                                    if enabled { "on" } else { "off" }
                                );
                                self.push_log_no_agent(
                                    LogLevel::Warn,
                                    format!("tools visibility save failed: {}", e),
                                );
                            }
                        }
                    }
                    _ => {
                        self.append_assistant_text(
                            "From: /tools\n⚠ Usage: `/tools` | `/tools <switch>` | `/tools <switch> on|off`.",
                        );
                        self.status = "tools error".to_string();
                    }
                }
            }

            "skills" => {
                let working_dir = std::env::current_dir().unwrap_or_default();
                let skill_dirs = ragent_agent::Config::load()
                    .map(|c| c.skill_dirs)
                    .unwrap_or_default();
                let registry = ragent_agent::skill::SkillRegistry::load(&working_dir, &skill_dirs);
                let skills = registry.list_all();

                let mut output = String::from("From: /skills\nRegistered Skills:\n\n");

                if skills.is_empty() {
                    output.push_str("  (no skills found)\n\n");
                    output.push_str("  Skills are loaded from:\n");
                    output.push_str("    Personal:  ~/.ragent/skills/<name>/SKILL.md\n");
                    output.push_str("    Project:   .ragent/skills/<name>/SKILL.md\n");
                } else {
                    // Wrap the table body in a fenced code block so the markdown
                    // pipeline preserves the per-line layout and column alignment
                    // instead of collapsing every row into a single paragraph.
                    output.push_str("```\n");
                    // Compute column widths from data
                    let col_cmd = skills
                        .iter()
                        .map(|s| {
                            let hint_len = s.argument_hint.as_ref().map_or(0, |h| h.len() + 1);
                            s.name.len() + 1 + hint_len // +1 for leading '/'
                        })
                        .max()
                        .unwrap_or(7)
                        .max(7); // "Command"
                    let col_scope = 10; // "Scope" header is 5, but values up to 10
                    let col_access = 10; // "Access" header is 6, values up to 10

                    // Header
                    output.push_str(&format!(
                        "  {:<col_cmd$}  {:<col_scope$}  {:<col_access$}  Description\n",
                        "Command",
                        "Scope",
                        "Access",
                        col_cmd = col_cmd,
                        col_scope = col_scope,
                        col_access = col_access,
                    ));
                    // Separator
                    output.push_str(&format!(
                        "  {:-<col_cmd$}  {:-<col_scope$}  {:-<col_access$}  {:-<11}\n",
                        "",
                        "",
                        "",
                        "",
                        col_cmd = col_cmd,
                        col_scope = col_scope,
                        col_access = col_access,
                    ));

                    for skill in &skills {
                        let hint = skill
                            .argument_hint
                            .as_deref()
                            .map(|h| format!(" {h}"))
                            .unwrap_or_default();
                        let cmd_col = format!("/{}{}", skill.name, hint);
                        let scope = format!("{}", skill.scope);
                        let access = match (skill.user_invocable, !skill.disable_model_invocation) {
                            (true, true) => "both",
                            (true, false) => "user-only",
                            (false, true) => "agent-only",
                            (false, false) => "disabled",
                        };
                        let desc = skill.description.as_deref().unwrap_or("(no description)");
                        output.push_str(&format!(
                            "  {:<col_cmd$}  {:<col_scope$}  {:<col_access$}  {}\n",
                            cmd_col,
                            scope,
                            access,
                            desc,
                            col_cmd = col_cmd,
                            col_scope = col_scope,
                            col_access = col_access,
                        ));
                    }
                    output.push_str(&format!("\n  {} skill(s) registered\n", skills.len()));
                    output.push_str("```\n");
                }

                self.append_assistant_text(&output);

                self.status = "skills".to_string();
            }
            "tasks" => {
                if self.active_tasks.is_empty() {
                    self.status = "No active background tasks".to_string();
                    self.push_log_no_agent(LogLevel::Info, "No active tasks".to_string());
                    return;
                }

                let mut output = String::from("From: /tasks\nActive Background Tasks:\n\n");
                output.push_str(&format!(
                    "  {:<12}  {:<20}  {:<12}  Description\n",
                    "Task ID", "Agent", "Status"
                ));
                output.push_str(&format!(
                    "  {:-<12}  {:-<20}  {:-<12}  {:-<20}\n",
                    "", "", "", ""
                ));

                for task in &self.active_tasks {
                    let task_id = format!("{}...", &task.id[..8.min(task.id.len())]);
                    let status_str = format!("{}", task.status);
                    output.push_str(&format!(
                        "  {:<12}  {:<20}  {:<12}  {}\n",
                        task_id,
                        task.agent_name,
                        status_str,
                        task.result.as_deref().unwrap_or("(running)")
                    ));
                }

                output.push_str(&format!(
                    "\nTo cancel a task, use: /cancel <task_id_prefix>\n"
                ));
                output.push_str(&format!(
                    "{} task(s) running, {} completed\n",
                    self.active_tasks
                        .iter()
                        .filter(|t| t.status == ragent_agent::task::TaskStatus::Running)
                        .count(),
                    self.active_tasks
                        .iter()
                        .filter(|t| t.status == ragent_agent::task::TaskStatus::Completed)
                        .count()
                ));

                self.append_assistant_text(&output);

                self.status = "tasks".to_string();
            }
            "mcp" => {
                let mcp_args: Vec<&str> = args.split_whitespace().collect();
                let sub = mcp_args.first().copied().unwrap_or("");
                match sub {
                    "discover" => {
                        // Run discovery synchronously using block_in_place.
                        let found = tokio::task::block_in_place(|| {
                            tokio::runtime::Handle::current().block_on(McpClient::discover())
                        });
                        // Show interactive discover dialog.
                        self.mcp_discover = Some(McpDiscoverState {
                            servers: found,
                            number_input: String::new(),
                            number_cursor: 0,
                            feedback: None,
                        });

                        return;
                    }
                    "connect" => {
                        if let Some(&id) = mcp_args.get(1) {
                            let config = ragent_agent::Config::load()
                                .ok()
                                .and_then(|c| c.mcp.get(id).cloned());
                            if let Some(_cfg) = config {
                                self.status =
                                    format!("MCP connect not yet implemented for '{}'", id);
                            } else {
                                self.status = format!("MCP '{}' not found in config", id);
                            }
                        } else {
                            self.status = "Usage: /mcp connect <id>".to_string();
                        }

                        return;
                    }
                    "disconnect" => {
                        if let Some(&id) = mcp_args.get(1) {
                            self.status =
                                format!("MCP disconnect not yet implemented for '{}'", id);
                        } else {
                            self.status = "Usage: /mcp disconnect <id>".to_string();
                        }

                        return;
                    }
                    _ => {
                        // Show all registered servers and status.
                        let mut out = String::from("From: /mcp\nMCP Servers:\n\n");
                        if self.mcp_servers.is_empty() {
                            out.push_str("  (no MCP servers configured)\n\n");
                            out.push_str("Run /mcp discover to scan for available servers.\n");
                            out.push_str("Then add them to 'mcp' in ragent.json to activate.\n");
                        } else {
                            for s in &self.mcp_servers {
                                let status_icon = match &s.status {
                                    ragent_agent::mcp::McpStatus::Connected => "🟢 connected",
                                    ragent_agent::mcp::McpStatus::Disabled => "⚪ disabled",
                                    ragent_agent::mcp::McpStatus::NeedsAuth => "🟡 needs auth",
                                    ragent_agent::mcp::McpStatus::Failed { error } => {
                                        &format!("🔴 failed: {}", error)
                                    }
                                };
                                out.push_str(&format!("  {:<18} {}\n", s.id, status_icon));
                                if !s.tools.is_empty() {
                                    out.push_str(&format!("    tools: {}\n", s.tools.len()));
                                }
                            }
                            let connected = self
                                .mcp_servers
                                .iter()
                                .filter(|s| s.status == ragent_agent::mcp::McpStatus::Connected)
                                .count();
                            out.push_str(&format!(
                                "\n{}/{} server(s) connected\n",
                                connected,
                                self.mcp_servers.len()
                            ));
                        }
                        out.push_str("\nSubcommands: /mcp discover  /mcp connect <id>  /mcp disconnect <id>\n");
                        self.append_assistant_text(&out);
                    }
                }

                self.status = "mcp".to_string();
            }
            "team" | "teams" => {
                // Split "subcommand rest-of-args"
                let (sub, rest) = args
                    .split_once(char::is_whitespace)
                    .map_or((args, ""), |(s, r)| (s.trim(), r.trim()));
                let sub = if sub.is_empty() { "status" } else { sub };
                match sub {
                    "help" => {
                        let output = "From: /team help
## /team command reference

| Command | Arguments | Description |
|---|---|---|
| `/team help` | none | Show this command reference table. |
| `/team status` | none | Show the currently active team in this session. |
| `/team show [name]` | optional `name` | Show one team in detail, or all registered teams when no name is given. |
| `/team create <blueprint> [name]` | required `blueprint`, optional `name` | Create a new project-local team (blueprint mandatory) and set it active. |
| `/team close` | none | Close the active team in this session (does not delete on disk). |
| `/team delete <name>` | required `name` | Delete a team from disk (also clears active state if it is active). |
| `/team blueprint [name]` | optional `name` | List all installed blueprints, or show details of a specific blueprint. |
| `/team message <teammate-name> <text>` | required `teammate-name`, required `text` | Send a mailbox message from lead to a teammate. |
| `/team tasks` | none | Show the task table for the active team. |
| `/team clear` | none | Clear/remove the active team task list file. |
| `/team cleanup` | none | Clean up the active team (requires no working teammates). |
| `/team focus [name]` | optional `name` | Focus on a teammate (shows output, routes input). No arg clears focus. |

Alias: `/teams ...` routes to `/team ...` (for example `/teams help`, `/teams show`).";
                        self.append_assistant_text(output);
                        self.status = "team: help".to_string();
                    }
                    "status" | "" => {
                        let mut output = String::from("From: /team status\n");
                        if let Some(team) = self.active_team.clone() {
                            self.ensure_team_manager_for_team(&team.name, None);
                            output.push_str(&format!(
                                "## Team: {} ({})\n\n",
                                team.name,
                                format!("{:?}", team.status).to_lowercase()
                            ));
                            output.push_str(&format!(
                                "  ● lead (you)  session: {}\n",
                                team.lead_session_id
                            ));
                            if self.team_members.is_empty() {
                                output.push_str(
                                    "  (no teammates yet — use team_spawn tool or /team create)\n",
                                );
                            } else {
                                for m in &self.team_members {
                                    let status = format!("{:?}", m.status).to_lowercase();
                                    let task =
                                        m.current_task_id.as_deref().unwrap_or("—").to_string();
                                    let model_str = m
                                        .model_override
                                        .as_ref()
                                        .map(|mr| format!("{}/{}", mr.provider_id, mr.model_id))
                                        .unwrap_or_else(|| "(inherited)".to_string());
                                    output.push_str(&format!(
                                        "  └ {:<18} {:<10} model:{:<30} task:{}\n",
                                        m.name, status, model_str, task
                                    ));
                                }
                            }
                            output
                                .push_str(&format!("\n{} teammate(s)\n", self.team_members.len()));
                        } else {
                            output.push_str("No active team.\n\nUse `/team create <blueprint> [name]` to start a team (blueprint required).");
                        }
                        self.append_assistant_text(&output);
                        self.status = "team: status".to_string();
                    }
                    "create" => {
                        if rest.is_empty() {
                            self.status = "Usage: /team create <blueprint> [name]".to_string();
                            return;
                        }

                        // Parse blueprint (mandatory) then optional name
                        let mut parts = rest.split_whitespace();
                        let blueprint = parts.next().unwrap_or("").to_string();
                        let mut name = parts.next().map(|s| s.to_string());

                        if blueprint.is_empty() {
                            self.status = "Usage: /team create <blueprint> [name]".to_string();
                            return;
                        }

                        // If no name provided, generate one from blueprint + timestamp
                        if name.is_none() {
                            let generated_name = format!(
                                "{}-{}",
                                blueprint,
                                chrono::Utc::now().format("%Y%m%d-%H-%M-%S")
                            );
                            name = Some(generated_name);
                        }
                        let name = name.expect("name guaranteed Some above");

                        let working_dir = std::env::current_dir().unwrap_or_default();
                        let sid = self.session_id.clone().unwrap_or_default();
                        match TeamStore::create(&name, &sid, &working_dir, true) {
                            Ok(store) => {
                                let name = store.config.name.clone();
                                let team_dir = store.dir.clone();
                                self.active_team = Some(store.config);
                                self.team_members.clear();
                                self.team_message_counts.clear();
                                self.show_teams = true;
                                self.ensure_team_manager_for_team(&name, Some(team_dir));
                                self.push_log_no_agent(
                                    LogLevel::Info,
                                    format!("🤝 Team '{}' created", name),
                                );
                                self.append_assistant_text(&format!(
                                    "From: /team create\nTeam '{}' created.\n\nUse the `team_spawn` tool to add teammates.",
                                    name
                                ));
                                self.status = format!("team: {}", name);

                                // If blueprint provided, invoke the team_create tool to apply seeding asynchronously
                                let bp = blueprint.clone();
                                if !bp.is_empty() {
                                    let session_processor = self.session_processor.clone();
                                    let event_bus = self.event_bus.clone();
                                    let storage = self.storage.clone();
                                    let working_dir_clone = working_dir.clone();
                                    let sid_clone = sid.clone();
                                    let name_clone = name.clone();
                                    // Capture the currently selected model so teammates inherit it.
                                    let active_model_clone: Option<ragent_agent::agent::ModelRef> =
                                        self.selected_model.as_deref().and_then(|s| {
                                            s.split_once('/').map(|(pid, mid)| {
                                                ragent_agent::agent::ModelRef {
                                                    provider_id: pid.to_string(),
                                                    model_id: mid.to_string(),
                                                }
                                            })
                                        });
                                    std::thread::spawn(move || {
                                        // Create a small runtime for seeding if there is no existing Tokio runtime
                                        let rt = match tokio::runtime::Runtime::new() {
                                            Ok(rt) => rt,
                                            Err(e) => {
                                                tracing::error!(
                                                    "Failed to create tokio runtime for team seed: {e}"
                                                );
                                                return;
                                            }
                                        };
                                        rt.block_on(async move {
                                                let registry = ragent_agent::tool::create_default_registry();
                                                if let Some(tool) = registry.get("team_create") {
                                                    let input = serde_json::json!({
                                                        "name": name_clone,
                                                        "project_local": true,
                                                        "blueprint": bp,
                                                    });
                                                                                                                                                                                                                      let ctx = ragent_agent::tool::ToolContext {
                                                                                                                                                                                                                          session_id: sid_clone.clone(),
                                                                                                                                                                                                                          working_dir: working_dir_clone.clone(),
                                                                                                                                                                                                                          event_bus: event_bus.clone(),
                                                                                                                                                                                                                          storage: Some(storage.clone()),
                                                                                                                                                                                                                          task_manager: None,
                                                                                                                                                                                                                          active_model: active_model_clone,
                                                                                                                                                                                                                          team_context: None,
                                                                                                                                                                                                                          team_manager: session_processor.team_manager.get().cloned().map(|tm| tm as Arc<dyn ragent_agent::tool::TeamManagerInterface>),
                                                                                                                                                                                                                          code_index: None,
                                                                                                                                                                                                                          bg_service: None,
                                                                                                                                                                                                                          spec_manager: session_processor.spec_manager.get().cloned(),
                                                                                                                                                                                                                          active_spec_id: session_processor.active_spec.read().await.clone(),
                                                                                                                                                                                                                          config: Some(Arc::new(ragent_agent::Config::load().unwrap_or_default())),
                                                                                                                                                                                                                          cached_team_dir: Arc::new(std::sync::Mutex::new(None)),
                                                                                                                                                                                                                          read_timestamps: session_processor.read_timestamps.clone(),
                                                                                                                                                                                                                      };
                                                                                                                                                                                                                      let _ = tool.execute(input, &ctx).await;                                                }
                                            });
                                    });
                                }
                            }
                            Err(e) => {
                                self.status = format!("Failed to create team: {}", e);
                                self.push_log_no_agent(
                                    LogLevel::Error,
                                    format!("team create failed: {}", e),
                                );
                            }
                        }
                    }

                    "show" => {
                        let working_dir = std::env::current_dir().unwrap_or_default();
                        if rest.is_empty() {
                            let teams = TeamStore::list_teams(&working_dir);
                            let mut output = String::from("From: /team show\n");
                            if teams.is_empty() {
                                output.push_str("No registered teams found.");
                                self.status = "team: show all (0)".to_string();
                            } else {
                                output.push_str("## Registered teams\n\n");
                                for (name, dir, is_project_local) in teams {
                                    match TeamStore::load(&dir) {
                                        Ok(store) => {
                                            let team = store.config;
                                            let scope = if is_project_local {
                                                "project"
                                            } else {
                                                "global"
                                            };
                                            output.push_str(&format!(
                                                "  ● {:<18} {:<10} lead:{} teammates:{}\n",
                                                team.name,
                                                format!("{:?}", team.status).to_lowercase(),
                                                team.lead_session_id,
                                                team.members.len()
                                            ));
                                            output.push_str(&format!(
                                                "    scope:{} path:{}\n",
                                                scope,
                                                dir.display()
                                            ));
                                        }
                                        Err(e) => {
                                            output.push_str(&format!(
                                                "  ● {} (failed to load: {})\n",
                                                name, e
                                            ));
                                        }
                                    }
                                }
                                self.status = "team: show all".to_string();
                            }
                            self.append_assistant_text(&output);
                            return;
                        }
                        match TeamStore::load_by_name(rest, &working_dir) {
                            Ok(store) => {
                                let team = store.config.clone();
                                self.ensure_team_manager_for_team(
                                    &team.name,
                                    Some(store.dir.clone()),
                                );

                                let mut output = String::from("From: /team show\n");
                                output.push_str(&format!(
                                    "## Team: {} ({})\n\n",
                                    team.name,
                                    format!("{:?}", team.status).to_lowercase()
                                ));
                                output.push_str(&format!(
                                    "  ● lead-session: {}\n",
                                    team.lead_session_id
                                ));
                                if team.members.is_empty() {
                                    output.push_str("  (no teammates yet)\n");
                                } else {
                                    for m in &team.members {
                                        let status = format!("{:?}", m.status).to_lowercase();
                                        let task =
                                            m.current_task_id.as_deref().unwrap_or("—").to_string();
                                        let sid = m.session_id.as_deref().unwrap_or("—");
                                        output.push_str(&format!(
                                            "  └ {:<18} {:<10} agent:{} session:{} task:{}\n",
                                            m.name, status, m.agent_id, sid, task
                                        ));
                                    }
                                }
                                output.push_str(&format!("\n{} teammate(s)\n", team.members.len()));
                                self.append_assistant_text(&output);
                                self.status = format!("team: show {}", team.name);
                            }
                            Err(e) => {
                                self.status = format!("Failed to load team: {e}");
                                self.push_log_no_agent(
                                    LogLevel::Error,
                                    format!("team show failed for '{}': {}", rest, e),
                                );
                            }
                        }
                    }
                    "close" => {
                        if let Some(team) = self.active_team.as_ref() {
                            let team_name = team.name.clone();
                            self.active_team = None;
                            self.team_members.clear();
                            self.team_message_counts.clear();
                            self.show_teams = false;
                            self.focused_teammate = None;
                            if self
                                .swarm_state
                                .as_ref()
                                .is_some_and(|s| s.team_name == team_name)
                            {
                                self.swarm_state = None;
                            }
                            self.push_log_no_agent(
                                LogLevel::Info,
                                format!("🤝 Team '{}' closed for this session", team_name),
                            );
                            self.append_assistant_text(&format!(
                                "From: /team close\nTeam '{}' closed for this session.",
                                team_name
                            ));
                            self.status = "team closed".to_string();
                        } else {
                            self.status = "No active team to close".to_string();
                        }
                    }
                    "delete" => {
                        if rest.is_empty() {
                            self.status = "Usage: /team delete <name>".to_string();
                            return;
                        }
                        let deleting_active = self
                            .active_team
                            .as_ref()
                            .is_some_and(|team| team.name == rest);
                        if deleting_active {
                            let active_count = self
                                .team_members
                                .iter()
                                .filter(|m| matches!(m.status, MemberStatus::Working))
                                .count();
                            if active_count > 0 {
                                self.status = format!(
                                    "{} teammate(s) still active — shut them down first",
                                    active_count
                                );
                                return;
                            }
                        }
                        let working_dir = std::env::current_dir().unwrap_or_default();
                        match TeamStore::load_by_name(rest, &working_dir) {
                            Ok(store) => match std::fs::remove_dir_all(&store.dir) {
                                Ok(_) => {
                                    if deleting_active {
                                        self.active_team = None;
                                        self.team_members.clear();
                                        self.team_message_counts.clear();
                                        self.show_teams = false;
                                        self.focused_teammate = None;
                                        if self
                                            .swarm_state
                                            .as_ref()
                                            .is_some_and(|s| s.team_name == rest)
                                        {
                                            self.swarm_state = None;
                                        }
                                    }
                                    self.push_log_no_agent(
                                        LogLevel::Info,
                                        format!("🗑️  Team '{}' deleted", rest),
                                    );
                                    self.append_assistant_text(&format!(
                                        "From: /team delete\nTeam '{}' deleted.",
                                        rest
                                    ));
                                    self.status = "team deleted".to_string();
                                }
                                Err(e) => {
                                    self.status = format!("Failed to delete team: {e}");
                                    self.push_log_no_agent(
                                        LogLevel::Error,
                                        format!("team delete failed for '{}': {}", rest, e),
                                    );
                                }
                            },
                            Err(e) => {
                                self.status = format!("Failed to load team: {e}");
                                self.push_log_no_agent(
                                    LogLevel::Error,
                                    format!("team delete failed for '{}': {}", rest, e),
                                );
                            }
                        }
                    }
                    "blueprint" | "blueprints" => {
                        let working_dir = std::env::current_dir().unwrap_or_default();

                        // Collect all blueprint directories from project-local and global paths.
                        let mut blueprint_dirs: Vec<(String, std::path::PathBuf, String)> =
                            Vec::new();
                        let mut seen_names: std::collections::HashSet<String> =
                            std::collections::HashSet::new();

                        // Walk up to find project .ragent/blueprints/teams/
                        let mut cur_opt: Option<&std::path::Path> = Some(working_dir.as_path());
                        while let Some(cur) = cur_opt {
                            let bp_root = cur.join(".ragent").join("blueprints").join("teams");
                            if bp_root.is_dir() {
                                if let Ok(entries) = std::fs::read_dir(&bp_root) {
                                    for entry in entries.flatten() {
                                        if entry.path().is_dir() {
                                            let name =
                                                entry.file_name().to_string_lossy().to_string();
                                            if seen_names.insert(name.clone()) {
                                                blueprint_dirs.push((
                                                    name,
                                                    entry.path(),
                                                    "project".to_string(),
                                                ));
                                            }
                                        }
                                    }
                                }
                                break;
                            }
                            cur_opt = cur.parent();
                        }
                        // Global blueprints
                        if let Some(home) = dirs::home_dir() {
                            let bp_root = home.join(".ragent").join("blueprints").join("teams");
                            if bp_root.is_dir() {
                                if let Ok(entries) = std::fs::read_dir(&bp_root) {
                                    for entry in entries.flatten() {
                                        if entry.path().is_dir() {
                                            let name =
                                                entry.file_name().to_string_lossy().to_string();
                                            if seen_names.insert(name.clone()) {
                                                blueprint_dirs.push((
                                                    name,
                                                    entry.path(),
                                                    "global".to_string(),
                                                ));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        blueprint_dirs.sort_by(|a, b| a.0.cmp(&b.0));

                        if rest.is_empty() {
                            // List all blueprints as a markdown table.
                            let mut output = String::from(
                                "From: /team blueprint\n\n## Installed Team Blueprints\n\n",
                            );
                            if blueprint_dirs.is_empty() {
                                output.push_str("No blueprints found.\n\nInstall blueprints to:\n- `[project]/.ragent/blueprints/teams/<name>/`\n- `~/.ragent/blueprints/teams/<name>/`\n");
                            } else {
                                output.push_str(
                                    "| Blueprint | Scope | Teammates | Tasks | Description |\n",
                                );
                                output.push_str(
                                    "|-----------|-------|-----------|-------|-------------|\n",
                                );
                                for (name, path, scope) in &blueprint_dirs {
                                    // Count teammates from spawn-prompts.json
                                    let teammate_count =
                                        std::fs::read_to_string(path.join("spawn-prompts.json"))
                                            .ok()
                                            .and_then(|raw| {
                                                serde_json::from_str::<serde_json::Value>(&raw).ok()
                                            })
                                            .and_then(|v| v.as_array().map(|a| a.len()))
                                            .unwrap_or(0);
                                    // Count tasks from task-seed.json
                                    let task_count =
                                        std::fs::read_to_string(path.join("task-seed.json"))
                                            .ok()
                                            .and_then(|raw| {
                                                serde_json::from_str::<serde_json::Value>(&raw).ok()
                                            })
                                            .and_then(|v| v.as_array().map(|a| a.len()))
                                            .unwrap_or(0);
                                    // Description from first line of README.md (skip heading)
                                    let desc = std::fs::read_to_string(path.join("README.md"))
                                        .ok()
                                        .and_then(|raw| {
                                            raw.lines()
                                                .find(|l| {
                                                    !l.trim().is_empty() && !l.starts_with('#')
                                                })
                                                .map(|l| l.trim().to_string())
                                        })
                                        .unwrap_or_else(|| "-".to_string());
                                    output.push_str(&format!(
                                        "| `{}` | {} | {} | {} | {} |\n",
                                        name, scope, teammate_count, task_count, desc
                                    ));
                                }
                            }
                            self.append_assistant_text(&output);
                            self.status = "team: blueprints".to_string();
                        } else {
                            // Show detailed summary for a specific blueprint.
                            let bp_name = rest.trim();
                            let found = blueprint_dirs.iter().find(|(n, _, _)| n == bp_name);
                            if let Some((name, path, scope)) = found {
                                let mut output = format!(
                                    "From: /team blueprint {name}\n\n## Blueprint: `{name}`\n\n**Scope:** {scope}  \n**Path:** `{}`\n\n",
                                    path.display()
                                );

                                // README.md
                                if let Ok(readme) = std::fs::read_to_string(path.join("README.md"))
                                {
                                    output.push_str("### Description\n\n");
                                    output.push_str(&readme);
                                    output.push_str("\n\n");
                                }

                                // Teammates from spawn-prompts.json
                                if let Ok(raw) =
                                    std::fs::read_to_string(path.join("spawn-prompts.json"))
                                {
                                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&raw)
                                    {
                                        if let Some(items) = val.as_array() {
                                            output.push_str("### Teammates\n\n");
                                            output.push_str("| Name | Type | Prompt |\n");
                                            output.push_str("|------|------|--------|\n");
                                            for item in items {
                                                let tname = item
                                                    .get("teammate_name")
                                                    .or_else(|| {
                                                        item.get("args")
                                                            .and_then(|a| a.get("teammate_name"))
                                                    })
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or("auto");
                                                let atype = item
                                                    .get("agent_type")
                                                    .or_else(|| {
                                                        item.get("args")
                                                            .and_then(|a| a.get("agent_type"))
                                                    })
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or("general");
                                                let prompt = item
                                                    .get("prompt")
                                                    .or_else(|| {
                                                        item.get("args")
                                                            .and_then(|a| a.get("prompt"))
                                                    })
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or("-");
                                                // Truncate long prompts for the table
                                                let prompt_short =
                                                    ragent_types::truncate_bytes(prompt, 77);
                                                output.push_str(&format!(
                                                    "| `{}` | {} | {} |\n",
                                                    tname, atype, prompt_short
                                                ));
                                            }
                                            output.push('\n');
                                        }
                                    }
                                }

                                // Tasks from task-seed.json
                                if let Ok(raw) =
                                    std::fs::read_to_string(path.join("task-seed.json"))
                                {
                                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&raw)
                                    {
                                        if let Some(items) = val.as_array() {
                                            output.push_str("### Seed Tasks\n\n");
                                            output.push_str("| Title | Description |\n");
                                            output.push_str("|-------|-------------|\n");
                                            for item in items {
                                                let title = item
                                                    .get("title")
                                                    .or_else(|| {
                                                        item.get("input")
                                                            .and_then(|a| a.get("title"))
                                                    })
                                                    .or_else(|| {
                                                        item.get("args")
                                                            .and_then(|a| a.get("title"))
                                                    })
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or("-");
                                                let desc = item
                                                    .get("description")
                                                    .or_else(|| {
                                                        item.get("input")
                                                            .and_then(|a| a.get("description"))
                                                    })
                                                    .or_else(|| {
                                                        item.get("args")
                                                            .and_then(|a| a.get("description"))
                                                    })
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or("-");
                                                output.push_str(&format!(
                                                    "| {} | {} |\n",
                                                    title, desc
                                                ));
                                            }
                                            output.push('\n');
                                        }
                                    }
                                }

                                output.push_str(&format!("**Usage:** `/team create {name}`\n"));
                                self.append_assistant_text(&output);
                                self.status = format!("team: blueprint {name}");
                            } else {
                                self.status = format!("Blueprint '{}' not found", bp_name);
                            }
                        }
                    }
                    "message" => {
                        let (name, text) = rest
                            .split_once(char::is_whitespace)
                            .map_or((rest, ""), |(n, t)| (n.trim(), t.trim()));
                        if name.is_empty() || text.is_empty() {
                            self.status = "Usage: /team message <teammate-name> <text>".to_string();
                            return;
                        }
                        let member = self.team_members.iter().find(|m| m.name == name).cloned();
                        match (self.active_team.clone(), member) {
                            (Some(team), Some(member)) => {
                                let working_dir = std::env::current_dir().unwrap_or_default();
                                match TeamStore::load_by_name(&team.name, &working_dir) {
                                    Ok(store) => {
                                        match Mailbox::open(&store.dir, &member.agent_id) {
                                            Ok(mb) => {
                                                let msg = MailboxMessage::new(
                                                    "lead",
                                                    &member.agent_id,
                                                    MessageType::Message,
                                                    text,
                                                );
                                                match mb.push(msg) {
                                                    Ok(_) => {
                                                        self.push_log_no_agent(
                                                            LogLevel::Info,
                                                            format!("📨 lead → {name}: {text}"),
                                                        );
                                                        self.status =
                                                            format!("message sent to {name}");
                                                    }
                                                    Err(e) => {
                                                        self.status =
                                                            format!("Failed to send message: {e}");
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                self.status =
                                                    format!("Failed to open mailbox: {e}");
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        self.status = format!("Failed to load team: {e}");
                                    }
                                }
                            }
                            (None, _) => {
                                self.status = "No active team".to_string();
                            }
                            (Some(_), None) => {
                                self.status = format!("Teammate '{name}' not found");
                            }
                        }
                    }
                    "tasks" => {
                        let team_opt = self.active_team.clone();
                        if let Some(team) = team_opt {
                            let working_dir = std::env::current_dir().unwrap_or_default();
                            match TeamStore::load_by_name(&team.name, &working_dir) {
                                Ok(store) => match store.task_store() {
                                    Ok(task_store) => match task_store.read() {
                                        Ok(task_list) => {
                                            let mut output = format!(
                                                "From: /team tasks\n## Tasks — team '{}'\n\n",
                                                team.name
                                            );
                                            if task_list.tasks.is_empty() {
                                                output.push_str("  (no tasks)\n");
                                            } else {
                                                output.push_str(&format!(
                                                    "  {:<12}  {:<34}  {:<12}  {}\n",
                                                    "ID", "Title", "Status", "Assignee"
                                                ));
                                                output.push_str(&format!(
                                                    "  {:-<12}  {:-<34}  {:-<12}  {:-<16}\n",
                                                    "", "", "", ""
                                                ));
                                                for task in &task_list.tasks {
                                                    let status = match task.status {
                                                        TaskStatus::Pending => "pending",
                                                        TaskStatus::InProgress => "in-progress",
                                                        TaskStatus::Completed => "completed",
                                                        TaskStatus::Cancelled => "cancelled",
                                                    };
                                                    let assignee =
                                                        task.assigned_to.as_deref().unwrap_or("—");
                                                    output.push_str(&format!(
                                                        "  {:<12}  {:<34}  {:<12}  {}\n",
                                                        task.id, task.title, status, assignee
                                                    ));
                                                }
                                            }
                                            self.append_assistant_text(&output);
                                            self.status =
                                                format!("{} task(s)", task_list.tasks.len());
                                        }
                                        Err(e) => {
                                            self.status = format!("Failed to read tasks: {e}");
                                        }
                                    },
                                    Err(e) => {
                                        self.status = format!("Failed to open task store: {e}");
                                    }
                                },
                                Err(e) => {
                                    self.status = format!("Failed to load team: {e}");
                                }
                            }
                        } else {
                            self.append_assistant_text("From: /team tasks\nNo active team.");
                            self.status = "no active team".to_string();
                        }
                    }
                    "clear" => {
                        let team_opt = self.active_team.clone();
                        if let Some(team) = team_opt {
                            let working_dir = std::env::current_dir().unwrap_or_default();
                            match TeamStore::load_by_name(&team.name, &working_dir) {
                                Ok(store) => {
                                    let tasks_path = store.dir.join("tasks.json");
                                    let cleared_count = store
                                        .task_store()
                                        .ok()
                                        .and_then(|s| s.read().ok())
                                        .map(|l| l.tasks.len())
                                        .unwrap_or(0);
                                    let clear_result = if tasks_path.exists() {
                                        std::fs::remove_file(&tasks_path)
                                    } else {
                                        Ok(())
                                    };
                                    match clear_result {
                                        Ok(_) => {
                                            self.append_assistant_text(&format!(
                                                "From: /team clear\nCleared {} task(s) for team '{}'.",
                                                cleared_count, team.name
                                            ));
                                            self.push_log_no_agent(
                                                LogLevel::Info,
                                                format!(
                                                    "🧹 Cleared {} task(s) from team '{}'",
                                                    cleared_count, team.name
                                                ),
                                            );
                                            self.status = "team tasks cleared".to_string();
                                        }
                                        Err(e) => {
                                            self.status = format!("Failed to clear tasks: {e}");
                                            self.push_log_no_agent(
                                                LogLevel::Error,
                                                format!(
                                                    "team clear failed for '{}': {}",
                                                    team.name, e
                                                ),
                                            );
                                        }
                                    }
                                }
                                Err(e) => {
                                    self.status = format!("Failed to load team: {e}");
                                }
                            }
                        } else {
                            self.append_assistant_text("From: /team clear\nNo active team.");
                            self.status = "no active team".to_string();
                        }
                    }
                    "cleanup" => {
                        let team_opt = self.active_team.clone();
                        if let Some(team) = team_opt {
                            // Guard: ensure no teammates are still working.
                            let active_count = self
                                .team_members
                                .iter()
                                .filter(|m| matches!(m.status, MemberStatus::Working))
                                .count();
                            if active_count > 0 {
                                // Build list of active teammate names
                                let active_names: Vec<String> = self
                                    .team_members
                                    .iter()
                                    .filter(|m| m.status != MemberStatus::Stopped)
                                    .map(|m| format!("{} ({})", m.name, m.agent_id))
                                    .collect();

                                // Log a warning with the list of active teammates
                                self.push_log_no_agent(
                                    LogLevel::Warn,
                                    format!(
                                        "Cannot clean up team '{}': {} teammate(s) still active: {}",
                                        team.name,
                                        active_names.len(),
                                        active_names.join(", ")
                                    ),
                                );

                                // Also show a message in the chat window listing active teammates
                                let mut msg = format!(
                                    "From: /team cleanup\nCannot clean up team '{}' because the following teammate(s) are still active:\n\n",
                                    team.name
                                );
                                for name in &active_names {
                                    msg.push_str(&format!("  - {}\n", name));
                                }
                                msg.push_str("\nYou can shut them down individually with team_shutdown_teammate, or run `/team forcecleanup` to deactivate and remove them.");
                                self.append_assistant_text(&msg);

                                self.status = format!(
                                    "{} teammate(s) still active — shut them down first",
                                    active_count
                                );
                                return;
                            }

                            let working_dir = std::env::current_dir().unwrap_or_default();
                            let team_name = team.name.clone();
                            let removed = match TeamStore::load_by_name(&team_name, &working_dir) {
                                Ok(store) => std::fs::remove_dir_all(&store.dir).is_ok(),
                                Err(_) => false,
                            };
                            self.active_team = None;
                            self.team_members.clear();
                            self.team_message_counts.clear();
                            self.show_teams = false;
                            self.focused_teammate = None;
                            if self
                                .swarm_state
                                .as_ref()
                                .is_some_and(|s| s.team_name == team_name)
                            {
                                self.swarm_state = None;
                            }
                            if removed {
                                self.push_log_no_agent(
                                    LogLevel::Info,
                                    format!("🗑️  Team '{team_name}' cleaned up"),
                                );
                                self.append_assistant_text(&format!(
                                    "From: /team cleanup\nTeam '{team_name}' cleaned up."
                                ));
                            } else {
                                self.push_log_no_agent(
                                    LogLevel::Warn,
                                    format!("Team '{team_name}' state cleared (dir not found)"),
                                );
                                self.append_assistant_text(&format!(
                                    "From: /team cleanup\nTeam '{team_name}' state cleared."
                                ));
                            }
                            self.status = "team cleaned up".to_string();
                        } else {
                            self.status = "No active team to clean up".to_string();
                        }
                    }
                    "forcecleanup" | "force-cleanup" => {
                        // Confirm with the user before destructive operation.
                        let confirm_msg = "Are you sure you want to force-cleanup the active team (deactivate teammates and remove on-disk resources)? Press Enter to confirm or Esc to cancel.";
                        let args_lower = args.to_lowercase();
                        let confirmed = args_lower.split_whitespace().any(|s| s == "confirm");
                        if !confirmed {
                            // Show interactive confirmation modal state with active members listed.
                            // Build list of active teammate names for display in modal.
                            let active_names: Vec<String> = self
                                .team_members
                                .iter()
                                .filter(|m| m.status != MemberStatus::Stopped)
                                .map(|m| format!("{} ({})", m.name, m.agent_id))
                                .collect();

                            let team_name = self
                                .active_team
                                .as_ref()
                                .map(|t| t.name.clone())
                                .unwrap_or_else(|| "<unknown>".to_string());

                            self.pending_forcecleanup = Some(PendingForceCleanup {
                                team_name: team_name.clone(),
                                active_members: active_names.clone(),
                            });

                            // Append assistant text instructing user to press Enter/Esc
                            let mut msg = format!("From: /team forcecleanup\n{}\n\n", confirm_msg);
                            if !active_names.is_empty() {
                                msg.push_str("Active teammates:\n\n");
                                for n in &active_names {
                                    msg.push_str(&format!("  - {}\n", n));
                                }
                                msg.push_str("\n");
                            }
                            msg.push_str("Press Enter to confirm or Esc to cancel.");

                            self.append_assistant_text(&msg);
                            self.push_log_no_agent(
                                LogLevel::Info,
                                "forcecleanup confirmation required (modal)".to_string(),
                            );
                            self.status = "forcecleanup confirmation required".to_string();
                            return;
                        }

                        // If confirmed, perform the force cleanup
                        let team_opt = self.active_team.clone();
                        if let Some(team) = team_opt {
                            let working_dir = std::env::current_dir().unwrap_or_default();
                            let team_name = team.name.clone();
                            match TeamStore::load_by_name(&team_name, &working_dir) {
                                Ok(mut store) => {
                                    // Attempt graceful shutdown of active teammate sessions first
                                    let mut deactivated: Vec<String> = Vec::new();
                                    for m in &mut store.config.members {
                                        if m.status != MemberStatus::Stopped {
                                            // Try to contact team manager to request shutdown if available
                                            if self.session_processor.team_manager.get().is_some() {
                                                // best-effort: ignore errors
                                                // Best-effort: request teammate to shutdown asynchronously.
                                                // Fire-and-forget via tokio::spawn; ignore result.
                                                let team_name_clone = store.config.name.clone();
                                                let m_name = m.name.clone();
                                                let m_agent_type = m.agent_type.clone();
                                                let working_dir_clone = store.dir.clone();
                                                let active_model: Option<
                                                    &ragent_agent::agent::ModelRef,
                                                > = None;
                                                if let Some(tm_arc) =
                                                    self.session_processor.team_manager.get()
                                                {
                                                    let tm = tm_arc.clone();
                                                    tokio::spawn(async move {
                                                        let _ = tm
                                                            .spawn_teammate(
                                                                &team_name_clone,
                                                                &m_name,
                                                                &m_agent_type,
                                                                "shutdown",
                                                                active_model,
                                                                None,
                                                                &working_dir_clone,
                                                            )
                                                            .await;
                                                    });
                                                }
                                            }
                                            let desc = format!("{} ({})", m.name, m.agent_id);
                                            m.status = MemberStatus::Stopped;
                                            deactivated.push(desc);
                                        }
                                    }
                                    // Persist best-effort
                                    if let Err(e) = store.save() {
                                        self.push_log_no_agent(
                                            LogLevel::Warn,
                                            format!("Failed to persist team member status before force cleanup: {}", e),
                                        );
                                    }

                                    // Remove directory
                                    let removed = std::fs::remove_dir_all(&store.dir).is_ok();

                                    // Update TUI state
                                    self.active_team = None;
                                    self.team_members.clear();
                                    self.team_message_counts.clear();
                                    self.show_teams = false;
                                    self.focused_teammate = None;
                                    if self
                                        .swarm_state
                                        .as_ref()
                                        .is_some_and(|s| s.team_name == team_name)
                                    {
                                        self.swarm_state = None;
                                    }

                                    if !deactivated.is_empty() {
                                        self.push_log_no_agent(
                                            LogLevel::Info,
                                            format!(
                                                "Deactivated teammates: {}",
                                                deactivated.join(", ")
                                            ),
                                        );
                                    }

                                    if removed {
                                        self.push_log_no_agent(
                                            LogLevel::Info,
                                            format!("🗑️  Team '{team_name}' force cleaned up"),
                                        );
                                        let mut msg = format!(
                                            "From: /team forcecleanup\nTeam '{team_name}' force cleaned up. The following teammate(s) were deactivated and removed:\n\n"
                                        );
                                        for d in &deactivated {
                                            msg.push_str(&format!("  - {}\n", d));
                                        }
                                        self.append_assistant_text(&msg);
                                    } else {
                                        self.push_log_no_agent(
                                            LogLevel::Warn,
                                            format!(
                                                "Team '{team_name}' state cleared (dir not found)"
                                            ),
                                        );
                                        let mut msg = format!(
                                            "From: /team forcecleanup\nTeam '{team_name}' state cleared. The following teammate(s) were deactivated:\n\n"
                                        );
                                        for d in &deactivated {
                                            msg.push_str(&format!("  - {}\n", d));
                                        }
                                        self.append_assistant_text(&msg);
                                    }

                                    self.status = "team force cleaned up".to_string();
                                }
                                Err(e) => {
                                    self.status = format!("Failed to force cleanup team: {}", e);
                                    self.push_log_no_agent(
                                        LogLevel::Error,
                                        format!("forcecleanup failed for '{}': {}", team_name, e),
                                    );
                                }
                            }
                        } else {
                            self.status = "No active team to clean up".to_string();
                        }
                    }
                    "focus" => {
                        if self.active_team.is_none() {
                            self.status = "No active team".to_string();
                            return;
                        }
                        if rest.is_empty() {
                            // No arg → clear focus
                            self.focused_teammate = None;
                            self.output_view = None;
                            self.append_assistant_text("From: /team focus\nTeammate focus cleared. Input returns to lead session.");
                            self.status = "team: focus cleared".to_string();
                        } else {
                            match self.focus_teammate_by_name(rest) {
                                Ok(()) => {
                                    let name = self
                                        .focused_teammate
                                        .as_ref()
                                        .and_then(|id| {
                                            self.team_members.iter().find(|m| m.agent_id == *id)
                                        })
                                        .map(|m| m.name.clone())
                                        .unwrap_or_default();
                                    self.append_assistant_text(&format!(
                                        "From: /team focus\nFocused on **{}**. Messages will be routed to this teammate.\n\nUse `/team focus` (no args) or Alt+Up/Down to change focus.\nPress Esc to close the output view.",
                                        name
                                    ));
                                }
                                Err(e) => {
                                    self.append_assistant_text(&format!(
                                        "From: /team focus\n{e}\n\nAvailable teammates: {}",
                                        self.team_members
                                            .iter()
                                            .map(|m| m.name.as_str())
                                            .collect::<Vec<_>>()
                                            .join(", ")
                                    ));
                                    self.status = format!("team focus: {e}");
                                }
                            }
                        }
                    }
                    _ => {
                        self.status = format!(
                            "Unknown /team subcommand '{}'. Usage: /team [help|status|show|create|close|delete|message|tasks|clear|cleanup|focus]",
                            sub
                        );
                        self.push_log_no_agent(
                            LogLevel::Warn,
                            format!("unknown /team subcommand: {}", sub),
                        );
                    }
                }
            }
            "todos" => {
                if !self.ensure_session() {
                    return;
                }
                let Some(session_id) = self.session_id.clone() else {
                    self.status = "No active session".to_string();
                    return;
                };
                let storage = self.session_processor.session_manager.storage();

                // Fetch todos from storage
                let status_filter = if args.is_empty() { None } else { Some(args) };
                match storage.get_todos(&session_id, status_filter) {
                    Ok(todos) => {
                        let mut output = String::from("From: /todo_list\n");
                        if todos.is_empty() {
                            output.push_str("No TODO items found");
                            if let Some(filter) = status_filter {
                                output.push_str(&format!(" with status '{filter}'"));
                            }
                            output.push_str(".\n");
                        } else {
                            output.push_str(&format!("## TODOs ({} items)\n\n", todos.len()));
                            for todo in &todos {
                                let status_icon = match todo.status.as_str() {
                                    "pending" => "⏳",
                                    "in_progress" => "🔄",
                                    "done" => "✅",
                                    "blocked" => "🚫",
                                    _ => "❓",
                                };
                                output.push_str(&format!(
                                    "- {} **{}** — {} `[{}]`\n",
                                    status_icon, todo.id, todo.title, todo.status
                                ));
                                if !todo.description.is_empty() {
                                    output.push_str(&format!("  {}\n", todo.description));
                                }
                            }
                        }
                        self.append_assistant_text(&output);

                        self.status = format!("{} todo(s)", todos.len());
                    }
                    Err(e) => {
                        self.status = format!("Failed to read todos: {}", e);
                        self.push_log_no_agent(LogLevel::Error, format!("todo_list error: {}", e));
                    }
                }
            }
            // ── /bash ────────────────────────────────────────────────────────
            "bash" => {
                let (sub, rest) = args
                    .split_once(char::is_whitespace)
                    .map_or((args, ""), |(s, r)| (s.trim(), r.trim()));

                match sub {
                    "help" | "" => {
                        let help = "\
From: /bash help

## /bash — Bash command list management

Manage the user-defined **allowlist** and **denylist** that complement the
built-in safety rules.

### Subcommands

| Command | Description |
|---------|-------------|
| `/bash add allow <cmd>` | Allow a banned command prefix (e.g. `curl`) |
| `/bash add deny <pattern>` | Block any command containing `<pattern>` |
| `/bash remove allow <cmd>` | Remove a command from the allowlist |
| `/bash remove deny <pattern>` | Remove a pattern from the denylist |
| `/bash show` | Show the current allowlist and denylist |
| `/bash help` | Show this help text |

Append `--global` to write the change to the global config
(`~/.config/ragent/ragent.json`) instead of the project `.ragent/ragent.json`.

### How it works

- **allowlist**: command prefixes that bypass the built-in banned-command \
check.  Use this to re-enable tools like `curl` without entering YOLO mode.
- **denylist**: substring patterns that always reject a command, \
supplementing the built-in denied-patterns list.

Changes are persisted immediately to `.ragent/ragent.json` and take effect at once.
";
                        self.append_assistant_text(help);
                    }
                    "show" => {
                        let allowlist = ragent_agent::bash_lists::get_allowlist();
                        let denylist = ragent_agent::bash_lists::get_denylist();
                        let safe_commands = ragent_tools_core::bash::get_safe_commands();
                        let (
                            builtin_banned,
                            builtin_denied_commands,
                            builtin_denied_cmd_patterns,
                            builtin_patterns,
                        ) = ragent_tools_core::bash::get_builtin_lists();

                        let mut out = String::from("From: /bash show\n\n## Bash command lists\n\n");

                        // Built-in safe commands (Layer 1)
                        out.push_str("### Built-in Safe Commands (Layer 1 - Auto-approved)\n");
                        out.push_str(
                            "*These commands are auto-approved without user prompting*\n\n",
                        );
                        for cmd in safe_commands {
                            out.push_str(&format!("  - `{cmd}`\n"));
                        }

                        // User-defined allowlist
                        out.push_str("\n### Allowlist (user-defined - Layer 2 exemptions)\n");
                        if allowlist.is_empty() {
                            out.push_str("  *(empty)*\n");
                        } else {
                            for entry in &allowlist {
                                out.push_str(&format!("  - `{entry}`\n"));
                            }
                        }

                        // User-defined denylist
                        out.push_str("\n### Denylist (user-defined - Layer 3 custom blocks)\n");
                        if denylist.is_empty() {
                            out.push_str("  *(empty)*\n");
                        } else {
                            for entry in &denylist {
                                out.push_str(&format!("  - `{entry}`\n"));
                            }
                        }

                        // Built-in banned commands
                        out.push_str(
                            "\n### Built-in Banned Commands (Layer 2 - Word-boundary matched)\n",
                        );
                        out.push_str("*These commands are blocked unless allowlisted or YOLO mode is enabled*\n\n");
                        for cmd in builtin_banned {
                            out.push_str(&format!("  - `{cmd}`\n"));
                        }

                        // Built-in denied commands
                        out.push_str(
                            "\n### Built-in Denied Commands (Layer 3 - Word-boundary matched)\n",
                        );
                        out.push_str("*These command names are unconditionally blocked (e.g., mkfs, insmod, useradd)*\n\n");
                        for cmd in builtin_denied_commands {
                            out.push_str(&format!("  - `{cmd}`\n"));
                        }

                        // Built-in denied command patterns
                        out.push_str("\n### Built-in Denied Command Patterns (Layer 3 - Command+args matched)\n");
                        out.push_str("*Commands with specific arguments are blocked (e.g., sudo , su -, passwd )*\n\n");
                        for pattern in builtin_denied_cmd_patterns {
                            out.push_str(&format!("  - `{pattern}`\n"));
                        }

                        // Built-in denied patterns
                        out.push_str(
                            "\n### Built-in Denied Patterns (Layer 3 - Substring matched)\n",
                        );
                        out.push_str(
                            "*Commands containing these patterns are unconditionally blocked*\n\n",
                        );
                        for pattern in builtin_patterns {
                            out.push_str(&format!("  - `{pattern}`\n"));
                        }

                        self.append_assistant_text(&out);
                    }
                    "add" | "remove" => {
                        // Parse: [allow|deny] <entry> [--global]
                        let (list_type, entry_with_flag) = rest
                            .split_once(char::is_whitespace)
                            .map_or((rest, ""), |(l, e)| (l.trim(), e.trim()));

                        let is_global = entry_with_flag.ends_with("--global");
                        let entry = if is_global {
                            entry_with_flag.trim_end_matches("--global").trim()
                        } else {
                            entry_with_flag
                        };

                        if entry.is_empty() {
                            self.append_assistant_text(&format!(
                                "From: /bash {sub}\n\nUsage: `/bash {sub} allow|deny <entry> [--global]`"
                            ));

                            return;
                        }

                        let scope = if is_global {
                            ragent_agent::bash_lists::Scope::Global
                        } else {
                            ragent_agent::bash_lists::Scope::Project
                        };
                        let scope_label = if is_global { "global" } else { "project" };
                        let config_file = if is_global {
                            "~/.config/ragent/ragent.json"
                        } else {
                            ".ragent/ragent.json"
                        };

                        match (sub, list_type) {
                            ("add", "allow") => {
                                match ragent_agent::bash_lists::add_allowlist(entry, scope) {
                                    Ok(()) => {
                                        self.append_assistant_text(&format!(
                                            "From: /bash add allow\n\n\
                                            ✅ Added `{entry}` to the **allowlist** \
                                            ({scope_label}: `{config_file}`).\n\n\
                                            Commands starting with `{entry}` will no longer \
                                            be blocked by the banned-command check."
                                        ));
                                    }
                                    Err(e) => {
                                        self.append_assistant_text(&format!(
                                            "From: /bash add allow\n\n❌ Error: {e}"
                                        ));
                                    }
                                }
                            }
                            ("add", "deny") => {
                                match ragent_agent::bash_lists::add_denylist(entry, scope) {
                                    Ok(()) => {
                                        self.append_assistant_text(&format!(
                                            "From: /bash add deny\n\n\
                                            ✅ Added `{entry}` to the **denylist** \
                                            ({scope_label}: `{config_file}`).\n\n\
                                            Any command containing `{entry}` will be rejected."
                                        ));
                                    }
                                    Err(e) => {
                                        self.append_assistant_text(&format!(
                                            "From: /bash add deny\n\n❌ Error: {e}"
                                        ));
                                    }
                                }
                            }
                            ("remove", "allow") => {
                                match ragent_agent::bash_lists::remove_allowlist(entry, scope) {
                                    Ok(true) => {
                                        self.append_assistant_text(&format!(
                                            "From: /bash remove allow\n\n\
                                            ✅ Removed `{entry}` from the **allowlist** \
                                            ({scope_label}: `{config_file}`)."
                                        ));
                                    }
                                    Ok(false) => {
                                        self.append_assistant_text(&format!(
                                            "From: /bash remove allow\n\n\
                                            ⚠️ `{entry}` was not in the {scope_label} allowlist."
                                        ));
                                    }
                                    Err(e) => {
                                        self.append_assistant_text(&format!(
                                            "From: /bash remove allow\n\n❌ Error: {e}"
                                        ));
                                    }
                                }
                            }
                            ("remove", "deny") => {
                                match ragent_agent::bash_lists::remove_denylist(entry, scope) {
                                    Ok(true) => {
                                        self.append_assistant_text(&format!(
                                            "From: /bash remove deny\n\n\
                                            ✅ Removed `{entry}` from the **denylist** \
                                            ({scope_label}: `{config_file}`)."
                                        ));
                                    }
                                    Ok(false) => {
                                        self.append_assistant_text(&format!(
                                            "From: /bash remove deny\n\n\
                                            ⚠️ `{entry}` was not in the {scope_label} denylist."
                                        ));
                                    }
                                    Err(e) => {
                                        self.append_assistant_text(&format!(
                                            "From: /bash remove deny\n\n❌ Error: {e}"
                                        ));
                                    }
                                }
                            }
                            _ => {
                                self.append_assistant_text(&format!(
                                    "From: /bash {sub}\n\n\
                                    Unknown list type `{list_type}`. Use `allow` or `deny`.\n\n\
                                    Usage: `/bash {sub} allow|deny <entry> [--global]`"
                                ));
                            }
                        }
                    }
                    _ => {
                        self.append_assistant_text(&format!(
                            "From: /bash\n\nUnknown subcommand `{sub}`. \
                            Run `/bash help` for usage."
                        ));
                    }
                }
            }
            // ── /dirs ────────────────────────────────────────────────────────
            "dirs" => {
                let (sub, rest) = args
                    .split_once(char::is_whitespace)
                    .map_or((args, ""), |(s, r)| (s.trim(), r.trim()));

                match sub {
                    "help" | "" => {
                        let help = "\
            From: /dirs help
            
            ## /dirs — Directory/file permission management
            
            Manage glob patterns for file operations that are automatically **allowed** or **denied**
            by the permission system without prompting.
            
            ### Subcommands
            
            | Command | Description |
            |---------|-------------|
            | `/dirs add allow <pattern>` | Add a glob pattern to auto-allow (e.g. `src/**/*.rs`) |
            | `/dirs add deny <pattern>` | Add a glob pattern to auto-deny (e.g. `secrets/**`) |
            | `/dirs remove allow <pattern>` | Remove a pattern from the allowlist |
            | `/dirs remove deny <pattern>` | Remove a pattern from the denylist |
            | `/dirs show` | Show current allowlist and denylist |
            | `/dirs help` | Show this help text |
            
            ### Flags
            
            - `--global` — Persist to global config (`~/.config/ragent/ragent.json`)
            - (default) — Persist to project config (`./.ragent/ragent.json`)
            
            ### Examples
            
            ```bash
            # Allow editing all Rust source files without prompting
            /dirs add allow src/**/*.rs
            
            # Deny all operations in the secrets directory
            /dirs add deny secrets/**
            
            # Show current lists
            /dirs show
            ```
            
            ### Pattern Matching
            
            Patterns use **glob syntax**:
            - `*` matches any sequence of characters (except `/`)
            - `**` matches any sequence of characters (including `/`)
            - `?` matches any single character
            - `[abc]` matches any character in the set
            
            ### Notes
            
            - Patterns are checked **before** user permission prompts
            - Denylist patterns override allowlist patterns
            - Use `/dirs show` to see active patterns
            ";
                        self.append_assistant_text(help);
                    }
                    "show" => {
                        let (builtin_allow, builtin_deny) =
                            ragent_agent::dir_lists::get_builtin_lists();
                        let user_allow = ragent_agent::dir_lists::get_allowlist();
                        let user_deny = ragent_agent::dir_lists::get_denylist();

                        let mut out = String::from("From: /dirs show\n\n");
                        out.push_str("## Directory/File Permission Lists\n\n");

                        // Built-in allowlist
                        out.push_str("### Built-in Allowlist (auto-approve)\n");
                        if builtin_allow.is_empty() {
                            out.push_str("*(empty)*\n\n");
                        } else {
                            out.push_str("*File operations matching these patterns are automatically allowed*\n\n");
                            for pattern in &builtin_allow {
                                out.push_str(&format!("  - `{pattern}`\n"));
                            }
                            out.push('\n');
                        }

                        // User allowlist
                        out.push_str("### User Allowlist (auto-approve)\n");
                        if user_allow.is_empty() {
                            out.push_str("*(empty)*\n\n");
                        } else {
                            out.push_str("*File operations matching these patterns are automatically allowed*\n\n");
                            for pattern in &user_allow {
                                out.push_str(&format!("  - `{pattern}`\n"));
                            }
                            out.push('\n');
                        }

                        // Built-in denylist
                        out.push_str("### Built-in Denylist (auto-deny)\n");
                        if builtin_deny.is_empty() {
                            out.push_str("*(empty)*\n\n");
                        } else {
                            out.push_str("*File operations matching these patterns are automatically denied*\n\n");
                            for pattern in &builtin_deny {
                                out.push_str(&format!("  - `{pattern}`\n"));
                            }
                            out.push('\n');
                        }

                        // User denylist
                        out.push_str("### User Denylist (auto-deny)\n");
                        if user_deny.is_empty() {
                            out.push_str("*(empty)*\n\n");
                        } else {
                            out.push_str("*File operations matching these patterns are automatically denied*\n\n");
                            for pattern in &user_deny {
                                out.push_str(&format!("  - `{pattern}`\n"));
                            }
                        }

                        self.append_assistant_text(&out);
                    }
                    "add" | "remove" => {
                        // Parse: [allow|deny] <pattern> [--global]
                        let (list_type, pattern_with_flag) = rest
                            .split_once(char::is_whitespace)
                            .map_or((rest, ""), |(l, p)| (l.trim(), p.trim()));

                        let is_global = pattern_with_flag.ends_with("--global");
                        let pattern = if is_global {
                            pattern_with_flag.trim_end_matches("--global").trim()
                        } else {
                            pattern_with_flag
                        };

                        if pattern.is_empty() {
                            self.append_assistant_text(&format!(
                                              "From: /dirs {sub}\n\nUsage: `/dirs {sub} allow|deny <pattern> [--global]`"
                                          ));
                            return;
                        }

                        let scope = if is_global {
                            ragent_agent::dir_lists::Scope::Global
                        } else {
                            ragent_agent::dir_lists::Scope::Project
                        };
                        let scope_label = if is_global { "global" } else { "project" };
                        let config_file = if is_global {
                            "~/.config/ragent/ragent.json"
                        } else {
                            ".ragent/ragent.json"
                        };

                        match (sub, list_type) {
                            ("add", "allow") => {
                                match ragent_agent::dir_lists::add_allowlist(pattern, scope) {
                                    Ok(()) => {
                                        self.append_assistant_text(&format!(
                                                          "From: /dirs add allow\n\n\
                                                          ✅ Added `{pattern}` to the **allowlist** \
                                                          ({scope_label}: `{config_file}`).\n\n\
                                                          File operations matching `{pattern}` will be automatically allowed."
                                                      ));
                                    }
                                    Err(e) => {
                                        self.append_assistant_text(&format!(
                                            "From: /dirs add allow\n\n❌ Error: {e}"
                                        ));
                                    }
                                }
                            }
                            ("add", "deny") => {
                                match ragent_agent::dir_lists::add_denylist(pattern, scope) {
                                    Ok(()) => {
                                        self.append_assistant_text(&format!(
                                                          "From: /dirs add deny\n\n\
                                                          ✅ Added `{pattern}` to the **denylist** \
                                                          ({scope_label}: `{config_file}`).\n\n\
                                                          File operations matching `{pattern}` will be automatically denied."
                                                      ));
                                    }
                                    Err(e) => {
                                        self.append_assistant_text(&format!(
                                            "From: /dirs add deny\n\n❌ Error: {e}"
                                        ));
                                    }
                                }
                            }
                            ("remove", "allow") => {
                                match ragent_agent::dir_lists::remove_allowlist(pattern, scope) {
                                    Ok(true) => {
                                        self.append_assistant_text(&format!(
                                                          "From: /dirs remove allow\n\n\
                                                          ✅ Removed `{pattern}` from the **allowlist** \
                                                          ({scope_label}: `{config_file}`)."
                                                      ));
                                    }
                                    Ok(false) => {
                                        self.append_assistant_text(&format!(
                                                          "From: /dirs remove allow\n\n\
                                                          ⚠️ `{pattern}` was not in the {scope_label} allowlist."
                                                      ));
                                    }
                                    Err(e) => {
                                        self.append_assistant_text(&format!(
                                            "From: /dirs remove allow\n\n❌ Error: {e}"
                                        ));
                                    }
                                }
                            }
                            ("remove", "deny") => {
                                match ragent_agent::dir_lists::remove_denylist(pattern, scope) {
                                    Ok(true) => {
                                        self.append_assistant_text(&format!(
                                                          "From: /dirs remove deny\n\n\
                                                          ✅ Removed `{pattern}` from the **denylist** \
                                                          ({scope_label}: `{config_file}`)."
                                                      ));
                                    }
                                    Ok(false) => {
                                        self.append_assistant_text(&format!(
                                                          "From: /dirs remove deny\n\n\
                                                          ⚠️ `{pattern}` was not in the {scope_label} denylist."
                                                      ));
                                    }
                                    Err(e) => {
                                        self.append_assistant_text(&format!(
                                            "From: /dirs remove deny\n\n❌ Error: {e}"
                                        ));
                                    }
                                }
                            }
                            _ => {
                                self.append_assistant_text(&format!(
                                                  "From: /dirs {sub}\n\n\
                                                  Unknown list type `{list_type}`. Use `allow` or `deny`.\n\n\
                                                  Usage: `/dirs {sub} allow|deny <pattern> [--global]`"
                                              ));
                            }
                        }
                    }
                    _ => {
                        self.append_assistant_text(&format!(
                            "From: /dirs\n\nUnknown subcommand `{sub}`. \
                                          Run `/dirs help` for usage."
                        ));
                    }
                }
            }
            "yolo" => {
                match ragent_config::yolo::toggle_persist() {
                    Ok(new_state) => {
                        let label = if new_state {
                            "ENABLED ⚠️"
                        } else {
                            "disabled"
                        };
                        let mut output = format!("From: /yolo\n## YOLO mode {label}\n\n");
                        if new_state {
                            output.push_str(
                                "All command validation is now **bypassed**:\n\
                                 - Bash denied-pattern checks — **off**\n\
                                 - Dynamic context allowlist — **off**\n\
                                 - MCP config validation — **off**\n\
                                 - Obfuscation detection — **off**\n\n\
                                 ⚠️  The agent can now execute **any** command without restriction.\n\
                                 Use `/yolo` again to re-enable safety checks.\n",
                            );
                        } else {
                            output.push_str("All safety checks have been **re-enabled**.\n");
                        }
                        self.append_assistant_text(&output);

                        self.status = format!("YOLO mode {label}");
                        self.push_log_no_agent(
                            if new_state {
                                LogLevel::Warn
                            } else {
                                LogLevel::Info
                            },
                            format!("YOLO mode {label}"),
                        );
                    }
                    Err(e) => {
                        self.status = format!("⚠ failed to persist YOLO mode: {e}");
                        self.append_assistant_text(&format!(
                            "From: /yolo\n⚠ Failed to persist YOLO mode: {e}\n"
                        ));
                        self.push_log_no_agent(
                            LogLevel::Error,
                            format!("YOLO persist failed: {e}"),
                        );
                    }
                }
                self.needs_redraw = true;
            }
            // ── /swarm ──────────────────────────────────────────────────────
            "swarm" => {
                let (sub, _rest) = args
                    .split_once(char::is_whitespace)
                    .map_or((args, ""), |(s, r)| (s.trim(), r.trim()));
                match sub {
                    "help" => {
                        let help = "\
From: /swarm help\n\
## Swarm — Fleet-Style Auto-Decomposition\n\n\
| Command | Description |\n\
|---------|-------------|\n\
| `/swarm <prompt>` | Decompose a goal into parallel subtasks and spawn a team |\n\
| `/swarm status` | Show live progress of the active swarm |\n\
| `/swarm cancel` | Cancel the active swarm and clean up |\n\
| `/swarm help` | Show this help |\n\n\
The swarm analyses your prompt, breaks it into independent subtasks with dependency \
edges, creates an ephemeral team, and orchestrates parallel execution.\n";
                        self.append_assistant_text(help);
                    }
                    "status" => {
                        self.handle_swarm_status();
                    }
                    "cancel" => {
                        self.handle_swarm_cancel();
                    }
                    _ => {
                        // /swarm <prompt> — decompose and execute
                        // Parse optional flags: --agent <type>
                        let (full_prompt, default_agent_type) = parse_swarm_args(args);

                        if full_prompt.is_empty() {
                            let help = "From: /swarm\n\nUsage: `/swarm <prompt>` — describe what you want the swarm to accomplish.\nUse `/swarm --agent <type> <prompt>` to set a default agent type for all subtasks.\nType `/swarm help` for more info.\n";
                            self.append_assistant_text(help);

                            return;
                        }

                        if self.swarm_state.is_some() {
                            self.status =
                                "⚠ A swarm is already active — use /swarm cancel first".to_string();
                            return;
                        }

                        // Store prompt and default agent type for later use when decomposition returns.
                        self.swarm_state = Some(team::SwarmState {
                            team_name: String::new(),
                            prompt: full_prompt.clone(),
                            decomposition: team::SwarmDecomposition { tasks: vec![] },
                            spawned: false,
                            completed: false,
                            default_agent_type,
                        });

                        // Require provider + model
                        let (provider_id, model_id) = match self
                            .selected_model
                            .as_deref()
                            .and_then(|s| s.split_once('/'))
                            .map(|(p, m)| (p.to_string(), m.to_string()))
                        {
                            Some(pair) => pair,
                            None => {
                                self.status =
                                    "⚠ /swarm requires a configured model — use /model".to_string();
                                return;
                            }
                        };

                        self.status = "⏳ swarm: decomposing goal…".to_string();
                        self.push_log_no_agent(
                            LogLevel::Info,
                            format!(
                                "Swarm: decomposing — {}",
                                &full_prompt[..full_prompt.len().min(80)]
                            ),
                        );

                        // Show user message in chat
                        self.append_assistant_text(&format!(
                                                                                                  "From: /swarm\n## 🐝 Swarm Decomposition\n\n\
                                                                                                  Analysing your goal and breaking it into parallel subtasks…\n\n\
                                                                                                  > {}\n",
                                                                                                  full_prompt
                                                                                              ));

                        // Spawn async LLM call for decomposition
                        let registry = Arc::clone(&self.provider_registry);
                        let storage_clone = Arc::clone(&self.storage);
                        let swarm_result = Arc::clone(&self.swarm_result);

                        tokio::spawn(async move {
                            let completer = RagentCompleter {
                                registry,
                                storage: storage_clone,
                                provider_id,
                                model_id,
                            };
                            let system = team::DECOMPOSITION_SYSTEM_PROMPT;
                            let user = team::build_decomposition_user_prompt(&full_prompt);

                            let outcome = match completer.complete(system, &user).await {
                                Ok(text) => Ok(text),
                                Err(e) => Err(e.to_string()),
                            };

                            if let Ok(mut guard) = swarm_result.lock() {
                                *guard = Some(outcome);
                            }
                        });
                    }
                }
            }

            // ── /autopilot ──────────────────────────────────────────────────
            "autopilot" => {
                let sub = args.split_whitespace().next().unwrap_or("").to_lowercase();
                match sub.as_str() {
                    "on" => {
                        // Parse optional flags: --max-tokens N  --max-time N
                        let mut token_budget: Option<u64> = None;
                        let mut time_secs: Option<u64> = None;
                        let parts: Vec<&str> = args.split_whitespace().collect();
                        let mut i = 1; // skip "on"
                        while i < parts.len() {
                            match parts[i] {
                                "--max-tokens" if i + 1 < parts.len() => {
                                    token_budget = parts[i + 1].parse().ok();
                                    i += 2;
                                }
                                "--max-time" if i + 1 < parts.len() => {
                                    time_secs = parts[i + 1].parse().ok();
                                    i += 2;
                                }
                                _ => {
                                    i += 1;
                                }
                            }
                        }
                        self.autopilot_enabled = true;
                        self.autopilot_token_budget = token_budget;
                        self.autopilot_time_limit_secs = time_secs;
                        self.autopilot_started_at = Some(std::time::Instant::now());
                        let mut msg =
                            "⚡ **Autopilot ON** — agent will run autonomously.".to_string();
                        if let Some(t) = token_budget {
                            msg.push_str(&format!(" Token budget: {t}."));
                        }
                        if let Some(s) = time_secs {
                            msg.push_str(&format!(" Time limit: {s}s."));
                        }
                        msg.push_str("\nCall `task_complete` to signal completion, or `/autopilot off` to stop.");
                        self.append_assistant_text(&format!("From: /autopilot\n{msg}"));
                        self.status = "⚡ autopilot".to_string();
                        self.push_log_no_agent(LogLevel::Info, "autopilot enabled".to_string());
                    }
                    "off" => {
                        self.autopilot_enabled = false;
                        self.autopilot_token_budget = None;
                        self.autopilot_time_limit_secs = None;
                        self.autopilot_started_at = None;
                        self.autopilot_pending_continue = None;
                        self.append_assistant_text("From: /autopilot\n⚡ **Autopilot OFF** — returning to interactive mode.");
                        self.status = "ready".to_string();
                        self.push_log_no_agent(LogLevel::Info, "autopilot disabled".to_string());
                    }
                    "status" => {
                        let state = if self.autopilot_enabled {
                            let elapsed = self
                                .autopilot_started_at
                                .map(|s| s.elapsed().as_secs())
                                .unwrap_or(0);
                            format!("⚡ Autopilot: **ON** (running for {}s)", elapsed)
                        } else {
                            "⚡ Autopilot: **OFF**".to_string()
                        };
                        self.append_assistant_text(&format!("From: /autopilot status\n{state}"));
                    }
                    _ => {
                        self.append_assistant_text(
                            "From: /autopilot\n\
                             Usage: `/autopilot on [--max-tokens N] [--max-time N]` | `off` | `status`"
                        );
                    }
                }
            }

            // ── /plan ────────────────────────────────────────────────────────
            "plan" => {
                if args.is_empty() {
                    self.append_assistant_text(
                                      "From: /plan\n\
                                       Usage: `/plan <task description>`\n\n\
                                       The plan agent will analyse the codebase and produce a plan for your task. \
                                       You will be asked to approve or reject the plan before implementation begins."
                                  );
                } else {
                    let sid = self.session_id.clone().unwrap_or_default();
                    self.execute_plan_delegation(&sid, args.to_string(), String::new());
                }
            }

            // ── /research ────────────────────────────────────────────────
            "research" => {
                self.handle_research_command(args);
            }

            // ── /spec ────────────────────────────────────────────────────────
            "spec" => {
                use ragent_specs::spec::SpecStatus;
                use ragent_specs::{SpecCommand, SpecFilter, SpecManager, validate};
                let cmd = SpecCommand::parse(args);
                match cmd {
                    SpecCommand::Help => {
                        self.append_assistant_text(SpecCommand::build_help_message());
                        self.status = "spec: help".to_string();
                    }
                    SpecCommand::Create { specname, feature } => {
                        let sid = self.session_id.clone().unwrap_or_default();
                        self.append_assistant_text(&SpecCommand::build_create_message(
                            &specname, &feature,
                        ));
                        self.push_log_no_agent(
                            LogLevel::Info,
                            SpecCommand::build_create_log(&specname, &feature),
                        );

                        let explore_agent = self
                            .cycleable_agents
                            .iter()
                            .find(|a| a.name == "explore")
                            .cloned();

                        let mut agent = explore_agent.unwrap_or_else(|| self.agent_info.clone());
                        self.apply_selected_model_and_thinking(&mut agent);
                        agent.permission = ragent_agent::agent::default_permissions();

                        let task = SpecCommand::build_create_prompt(&specname, &feature);
                        let msg = Message::user_text(&sid, &task);
                        self.messages.push(msg);

                        let processor = self.session_processor.clone();
                        let flag = Arc::new(AtomicBool::new(false));
                        self.cancel_flag = Some(flag.clone());
                        self.is_processing = true;
                        self.status = SpecCommand::build_create_status(&specname);

                        let event_bus = self.event_bus.clone();
                        tokio::spawn(async move {
                            if let Err(e) =
                                processor.process_message(&sid, &task, &agent, flag).await
                            {
                                tracing::warn!(error = %e, "spec: generation failed");
                                event_bus.publish(ragent_agent::event::Event::AgentError {
                                    session_id: sid,
                                    error: format!("spec generation failed: {e}"),
                                });
                            }
                        });
                    }
                    SpecCommand::Validate { spec_id } => {
                        let working_dir = std::env::current_dir().unwrap_or_default();
                        let specs_root = working_dir.join("specs");
                        let mgr = SpecManager::new(&specs_root);
                        let rt = tokio::runtime::Handle::current();
                        let result: Result<String, String> = tokio::task::block_in_place(|| {
                            rt.block_on(async {
                                let specs = if let Some(id_str) = spec_id {
                                    match ragent_specs::spec::SpecId::new(&id_str) {
                                        Some(id) => match mgr.read_spec(&id).await {
                                            Ok(spec) => vec![spec],
                                            Err(e) => {
                                                return Err(format!(
                                                    "spec: failed to read {}: {}",
                                                    id_str, e
                                                ));
                                            }
                                        },
                                        None => {
                                            return Err(format!(
                                                "spec: invalid spec ID: {}",
                                                id_str
                                            ));
                                        }
                                    }
                                } else {
                                    match mgr.discover_specs().await {
                                        Ok(specs) => specs,
                                        Err(e) => {
                                            return Err(format!("spec: discovery failed: {}", e));
                                        }
                                    }
                                };
                                if specs.is_empty() {
                                    return Ok("No specs found.".to_string());
                                }
                                let mut lines = vec!["From: /spec validate".to_string()];
                                for spec in &specs {
                                    let report = validate(spec);
                                    lines.push(format!("\n## Validation: `{}`", spec.id));
                                    lines.push(report.format(spec.id.as_str()));
                                }
                                Ok(lines.join("\n"))
                            })
                        });
                        match result {
                            Ok(output) => {
                                self.append_assistant_text(&output);
                                self.status = "spec: validation complete".to_string();
                            }
                            Err(e) => {
                                self.status = format!("spec: {}", e);
                                self.append_assistant_text(&format!(
                                    "From: /spec validate\n\n**Error:** {}",
                                    e
                                ));
                            }
                        }
                    }
                    SpecCommand::List { args } => {
                        let working_dir = std::env::current_dir().unwrap_or_default();
                        let specs_root = working_dir.join("specs");
                        let mgr = SpecManager::new(&specs_root);
                        let mut filter = SpecFilter::new();
                        // Parse simple --status and --prefix args
                        for token in args.split_whitespace() {
                            if let Some(val) = token.strip_prefix("--status=") {
                                if let Some(status) = SpecStatus::parse(val) {
                                    filter = filter.with_status(status);
                                }
                            } else if let Some(val) = token.strip_prefix("--prefix=") {
                                filter = filter.with_id_prefix(val);
                            } else if token == "--archived" {
                                filter = filter.with_archived();
                            }
                        }
                        let rt = tokio::runtime::Handle::current();
                        let result: Result<String, String> = tokio::task::block_in_place(|| {
                            rt.block_on(async {
                                let specs = match mgr.list_specs(&filter).await {
                                    Ok(s) => s,
                                    Err(e) => return Err(format!("spec: list failed: {}", e)),
                                };
                                if specs.is_empty() {
                                    return Ok("No specs found.".to_string());
                                }
                                let mut lines = vec!["From: /spec list".to_string(), String::new()];
                                lines.push(format!(
                                    "| {:<20} | {:<12} | {:<30} |",
                                    "ID", "Status", "Title"
                                ));
                                lines.push(
                                    "|".to_string()
                                        + &"-".repeat(22)
                                        + "|"
                                        + &"-".repeat(14)
                                        + "|"
                                        + &"-".repeat(32)
                                        + "|",
                                );
                                for spec in &specs {
                                    lines.push(format!(
                                        "| {:<20} | {:<12} | {:<30} |",
                                        spec.id.as_str(),
                                        spec.status.as_str(),
                                        spec.title.chars().take(30).collect::<String>()
                                    ));
                                }
                                Ok(lines.join("\n"))
                            })
                        });
                        match result {
                            Ok(output) => {
                                self.append_assistant_text(&output);
                                self.status = format!(
                                    "spec: {} spec(s) listed",
                                    output.lines().count().saturating_sub(4)
                                );
                            }
                            Err(e) => {
                                self.status = format!("spec: {}", e);
                            }
                        }
                    }
                    SpecCommand::Search { query } => {
                        let working_dir = std::env::current_dir().unwrap_or_default();
                        let specs_root = working_dir.join("specs");
                        let mgr = SpecManager::new(&specs_root);
                        let rt = tokio::runtime::Handle::current();
                        let result: Result<String, String> = tokio::task::block_in_place(|| {
                            rt.block_on(async {
                                let results = match mgr.search_specs(&query).await {
                                    Ok(r) => r,
                                    Err(e) => return Err(format!("spec: search failed: {}", e)),
                                };
                                if results.is_empty() {
                                    return Ok(format!("No specs found matching '{}'.", query));
                                }
                                let mut lines =
                                    vec![format!("From: /spec search '{}'", query), String::new()];
                                for r in &results {
                                    lines.push(format!(
                                        "## {} (score: {}, status: {})",
                                        r.spec.id, r.score, r.spec.status
                                    ));
                                    lines.push(format!("**{}**", r.spec.title));
                                    for snippet in &r.snippets {
                                        lines.push(format!("- {}", snippet));
                                    }
                                    lines.push(String::new());
                                }
                                Ok(lines.join("\n"))
                            })
                        });
                        match result {
                            Ok(output) => {
                                self.append_assistant_text(&output);
                                self.status = format!("spec: search complete");
                            }
                            Err(e) => {
                                self.status = format!("spec: {}", e);
                            }
                        }
                    }
                    SpecCommand::Status {
                        spec_id,
                        new_status,
                    } => {
                        let working_dir = std::env::current_dir().unwrap_or_default();
                        let specs_root = working_dir.join("specs");
                        let mgr = SpecManager::new(&specs_root);
                        let id = match ragent_specs::spec::SpecId::new(&spec_id) {
                            Some(id) => id,
                            None => {
                                self.status = format!("spec: invalid spec ID: {}", spec_id);
                                return;
                            }
                        };
                        let rt = tokio::runtime::Handle::current();
                        let result: Result<String, String> = tokio::task::block_in_place(|| {
                            rt.block_on(async {
                                                                                                                                                                                                                                                          let mut spec = match mgr.read_spec(&id).await {
                                                                                                                                                                                                                                                              Ok(s) => s,
                                                                                                                                                                                                                                                              Err(e) => return Err(format!("spec: failed to read {}: {}", spec_id, e)),
                                                                                                                                                                                                                                                          };
                                                                                                                                                                                                                                                          if let Some(new_status_str) = new_status {
                                                                                                                                                                                                                                                              let new_status = match SpecStatus::parse(&new_status_str) {
                                                                                                                                                                                                                                                                  Some(s) => s,
                                                                                                                                                                                                                                                                  None => return Err(format!("spec: unknown status '{}'", new_status_str)),
                                                                                                                                                                                                                                                              };
                                                                                                                                                                                                                                                              if let Err(e) = mgr.transition(&mut spec, new_status, "user").await {
                                                                                                                                                                                                                                                                  return Err(format!("spec: transition failed: {}", e));
                                                                                                                                                                                                                                                              }
                                                                                                                                                                                                                                                              Ok(format!(
                                                                                                                                                                                                                                                                  "From: /spec status\n\n**{}** transitioned from `{}` to `{}`.",
                                                                                                                                                                                                                                                                  spec.id, spec.status.as_str(), new_status.as_str()
                                                                                                                                                                                                                                                              ))
                                                                                                                                                                                                                                                          } else {
                                                                                                                                                                                                                                                              let next = ragent_specs::manager::next_statuses(spec.status);
                                                                                                                                                                                                                                                              let next_str = next.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ");
                                                                                                                                                                                                                                                              Ok(format!(
                                                                                                                                                                                                                                                                  "From: /spec status\n\n**{}** — current status: `{}`\nAllowed transitions: {}",
                                                                                                                                                                                                                                                                  spec.id, spec.status.as_str(), next_str
                                                                                                                                                                                                                                                              ))
                                                                                                                                                                                                                                                          }
                                                                                                                                                                                                                                                      })
                        });
                        match result {
                            Ok(output) => {
                                self.append_assistant_text(&output);
                                self.status = "spec: status updated".to_string();
                            }
                            Err(e) => {
                                self.status = format!("spec: {}", e);
                                self.append_assistant_text(&format!(
                                    "From: /spec status\n\n**Error:** {}",
                                    e
                                ));
                            }
                        }
                    }
                    SpecCommand::Task {
                        spec_id,
                        task_id,
                        new_status,
                    } => {
                        let working_dir = std::env::current_dir().unwrap_or_default();
                        let specs_root = working_dir.join("specs");
                        let mgr = SpecManager::new(&specs_root);
                        let id = match ragent_specs::spec::SpecId::new(&spec_id) {
                            Some(id) => id,
                            None => {
                                self.status = format!("spec: invalid spec ID: {}", spec_id);
                                return;
                            }
                        };
                        let rt = tokio::runtime::Handle::current();
                        let result: Result<String, String> = tokio::task::block_in_place(|| {
                            rt.block_on(async {
                                                                                                                                                                                                                                                          let mut spec = match mgr.read_spec(&id).await {
                                                                                                                                                                                                                                                              Ok(s) => s,
                                                                                                                                                                                                                                                              Err(e) => return Err(format!("spec: failed to read {}: {}", spec_id, e)),
                                                                                                                                                                                                                                                          };
                                                                                                                                                                                                                                                          if let Some(task_id_str) = task_id {
                                                                                                                                                                                                                                                              if let Some(new_status_str) = new_status {
                                                                                                                                                                                                                                                                  let new_status = match ragent_specs::spec::TaskStatus::parse(&new_status_str) {
                                                                                                                                                                                                                                                                      Some(s) => s,
                                                                                                                                                                                                                                                                      None => return Err(format!("spec: unknown task status '{}'", new_status_str)),
                                                                                                                                                                                                                                                                  };
                                                                                                                                                                                                                                                                  if let Err(e) = mgr.update_task_status(&mut spec, &task_id_str, new_status).await {
                                                                                                                                                                                                                                                                      return Err(format!("spec: task update failed: {}", e));
                                                                                                                                                                                                                                                                  }
                                                                                                                                                                                                                                                                  Ok(format!(
                                                                                                                                                                                                                                                                      "From: /spec task\n\n**{}** task `{}` updated to `{}`.",
                                                                                                                                                                                                                                                                      spec.id, task_id_str, new_status.as_str()
                                                                                                                                                                                                                                                                  ))
                                                                                                                                                                                                                                                              } else {
                                                                                                                                                                                                                                                                  // Show specific task
                                                                                                                                                                                                                                                                  let task = spec.tasks.iter().find(|t| t.id == task_id_str);
                                                                                                                                                                                                                                                                  match task {
                                                                                                                                                                                                                                                                      Some(t) => {
                                                                                                                                                                                                                                                                          let mut lines = vec![
                                                                                                                                                                                                                                                                              format!("From: /spec task\n\n## Task {} — {}", t.id, t.title),
                                                                                                                                                                                                                                                                              format!("- **Status:** {}", t.status.as_str()),
                                                                                                                                                                                                                                                                              format!("- **Effort:** {}", t.effort),
                                                                                                                                                                                                                                                                              format!("- **Priority:** {}", t.priority),
                                                                                                                                                                                                                                                                          ];
                                                                                                                                                                                                                                                                          if !t.linked_requirements.is_empty() {
                                                                                                                                                                                                                                                                              lines.push(format!("- **Requirements:** {}", t.linked_requirements.join(", ")));
                                                                                                                                                                                                                                                                          }
                                                                                                                                                                                                                                                                          if !t.dependencies.is_empty() {
                                                                                                                                                                                                                                                                              lines.push(format!("- **Dependencies:** {}", t.dependencies.join(", ")));
                                                                                                                                                                                                                                                                          }
                                                                                                                                                                                                                                                                          if let Some(ts) = t.completed_at {
                                                                                                                                                                                                                                                                              lines.push(format!("- **Completed:** {}", ts));
                                                                                                                                                                                                                                                                          }
                                                                                                                                                                                                                                                                          Ok(lines.join("\n"))
                                                                                                                                                                                                                                                                      }
                                                                                                                                                                                                                                                                      None => Err(format!("spec: task {} not found in {}", task_id_str, spec.id)),
                                                                                                                                                                                                                                                                  }
                                                                                                                                                                                                                                                              }
                                                                                                                                                                                                                                                          } else {
                                                                                                                                                                                                                                                              // List all tasks
                                                                                                                                                                                                                                                              let mut lines = vec![format!("From: /spec task\n\n## Tasks for **{}**", spec.id)];
                                                                                                                                                                                                                                                              if spec.tasks.is_empty() {
                                                                                                                                                                                                                                                                  lines.push("No tasks found.".to_string());
                                                                                                                                                                                                                                                              } else {
                                                                                                                                                                                                                                                                  lines.push("| ID | Title | Status | Effort | Priority |".to_string());
                                                                                                                                                                                                                                                                  lines.push("|----|-------|--------|--------|----------|".to_string());
                                                                                                                                                                                                                                                                  for t in &spec.tasks {
                                                                                                                                                                                                                                                                      lines.push(format!(
                                                                                                                                                                                                                                                                          "| {} | {} | {} | {} | {} |",
                                                                                                                                                                                                                                                                          t.id, t.title, t.status.as_str(), t.effort, t.priority
                                                                                                                                                                                                                                                                      ));
                                                                                                                                                                                                                                                                  }
                                                                                                                                                                                                                                                              }
                                                                                                                                                                                                                                                              Ok(lines.join("\n"))
                                                                                                                                                                                                                                                          }
                                                                                                                                                                                                                                                      })
                        });
                        match result {
                            Ok(output) => {
                                self.append_assistant_text(&output);
                                self.status = "spec: task complete".to_string();
                            }
                            Err(e) => {
                                self.status = format!("spec: {}", e);
                                self.append_assistant_text(&format!(
                                    "From: /spec task\n\n**Error:** {}",
                                    e
                                ));
                            }
                        }
                    }
                    SpecCommand::Activate { spec_id } => {
                        let working_dir = std::env::current_dir().unwrap_or_default();
                        let specs_root = working_dir.join("specs");
                        let mgr = SpecManager::new(&specs_root);
                        let id = match ragent_specs::spec::SpecId::new(&spec_id) {
                            Some(id) => id,
                            None => {
                                self.status = format!("spec: invalid spec ID: {}", spec_id);
                                return;
                            }
                        };
                        let rt = tokio::runtime::Handle::current();
                        let result: Result<_, String> = tokio::task::block_in_place(|| {
                            rt.block_on(async {
                                match mgr.read_spec(&id).await {
                                    Ok(spec) => Ok(spec),
                                    Err(e) => {
                                        Err(format!("spec: failed to read {}: {}", spec_id, e))
                                    }
                                }
                            })
                        });
                        match result {
                            Ok(spec) => {
                                self.active_spec = Some(spec_id.clone());
                                self.spec_manager = Some(Arc::new(mgr));
                                // Also set on the session processor so auto-updates work
                                let _ = rt.block_on(async {
                                    self.session_processor
                                        .active_spec
                                        .write()
                                        .await
                                        .replace(spec_id.clone())
                                });
                                // P-24: invalidate the cached spec section so the
                                // next turn re-reads the newly-activated spec.
                                self.session_processor
                                    .system_prompt_cache()
                                    .invalidate_spec_cache();
                                let _ = self
                                    .session_processor
                                    .spec_manager
                                    .set(Arc::new(SpecManager::new(&specs_root)));
                                self.append_assistant_text(&format!(                                                                                                                                                                                                                                                                                                                                                                                      "From: /spec activate\n\n✅ **{}** is now the active spec.\n\n\
                                                                                                                                                                                                                                                                                                                                                                                       Status: {}\n\
                                                                                                                                                                                                                                                                                                                                                                                       Title: {}\n\
                                                                                                                                                                                                                                                                                                                                                                                       Requirements: {}\n\
                                                                                                                                                                                                                                                                                                                                                                                       Tasks: {}\n\n\
                                                                                                                                                                                                                                                                                                                                                                                       This spec's requirements and tasks will be injected into the agent's system prompt.",
                                                                                                                                                                                                                                                                                                                                                                                      spec.id, spec.status.as_str(), spec.title, spec.requirements.len(), spec.tasks.len()
                                                                                                                                                                                                                                                                                                                                                                                  ));
                                self.status = format!("spec: {} activated", spec_id);
                            }
                            Err(e) => {
                                self.status = format!("spec: {}", e);
                                self.append_assistant_text(&format!(
                                    "From: /spec activate\n\n**Error:** {}",
                                    e
                                ));
                            }
                        }
                    }
                    SpecCommand::Deactivate => {
                        if self.active_spec.is_some() {
                            let prev = self.active_spec.take().unwrap();
                            self.spec_manager = None;
                            let rt = tokio::runtime::Handle::current();
                            let _ = rt.block_on(async {
                                self.session_processor.active_spec.write().await.take()
                            });
                            self.append_assistant_text(&format!(                                                                                                                                                                                                                                                                                                                                                                                  "From: /spec deactivate\n\n✅ Spec **{}** deactivated. Agent prompts will no longer include spec context.",
                                                                                                                                                                                                                                                                                                                                                                                  prev
                                                                                                                                                                                                                                                                                                                                                                              ));
                            self.status = "spec: deactivated".to_string();
                        } else {
                            self.append_assistant_text(
                                "From: /spec deactivate\n\nNo active spec to deactivate.",
                            );
                            self.status = "spec: no active spec".to_string();
                        }
                    }
                    SpecCommand::Delete { spec_id, yes } => {
                        let working_dir = std::env::current_dir().unwrap_or_default();
                        let specs_root = working_dir.join("specs");
                        let mgr = SpecManager::new(&specs_root);
                        let id = match ragent_specs::spec::SpecId::new(&spec_id) {
                            Some(id) => id,
                            None => {
                                self.status = format!("spec: invalid spec ID: {}", spec_id);
                                self.append_assistant_text(&format!(
                                    "From: /spec delete\n\n**Error:** invalid spec ID: {}",
                                    spec_id
                                ));
                                return;
                            }
                        };
                        if !yes {
                            self.append_assistant_text(&format!(
                                "From: /spec delete\n\nRefusing to delete specs/{} without confirmation. Re-run with `--yes` to skip this prompt.",
                                spec_id
                            ));
                            return;
                        }
                        self.status = format!("spec: deleting '{}'…", spec_id);
                        let event_bus = self.event_bus.clone();
                        let session_id = self.session_id.clone().unwrap_or_default();
                        tokio::spawn(async move {
                            let session_id_for_notice = session_id.clone();
                            match mgr.delete_spec(&id).await {
                                Ok(()) => {
                                    event_bus.publish(ragent_agent::event::Event::TextDelta {
                                        session_id,
                                        text: format!(
                                            "From: /spec delete\n\n✅ Deleted specs/{}.",
                                            spec_id
                                        ),
                                    });
                                    event_bus.publish(ragent_agent::event::Event::AgentNotice {
                                        session_id: session_id_for_notice,
                                        message: format!("spec: deleted specs/{}", spec_id),
                                    });
                                }
                                Err(e) => {
                                    event_bus.publish(ragent_agent::event::Event::TextDelta {
                                        session_id,
                                        text: format!("From: /spec delete\n\n**Error:** {}", e),
                                    });
                                }
                            }
                        });
                    }
                    SpecCommand::Coverage { spec_id } => {
                        let working_dir = std::env::current_dir().unwrap_or_default();
                        let specs_root = working_dir.join("specs");
                        let mgr = SpecManager::new(&specs_root);
                        let id = match ragent_specs::spec::SpecId::new(&spec_id) {
                            Some(id) => id,
                            None => {
                                self.status = format!("spec: invalid spec ID: {}", spec_id);
                                return;
                            }
                        };
                        let rt = tokio::runtime::Handle::current();
                        let result: Result<String, String> = tokio::task::block_in_place(|| {
                            rt.block_on(async {
                                let spec = match mgr.read_spec(&id).await {
                                    Ok(s) => s,
                                    Err(e) => {
                                        return Err(format!(
                                            "spec: failed to read {}: {}",
                                            spec_id, e
                                        ));
                                    }
                                };
                                let mut lines = vec![
                                    format!(
                                        "From: /spec coverage\n\n## Coverage Report: {}",
                                        spec.id
                                    ),
                                    String::new(),
                                    format!("**Overall Coverage:** {:.1}%", spec.coverage_pct()),
                                    String::new(),
                                ];
                                let mut req_to_completed: std::collections::HashMap<
                                    &str,
                                    Vec<&str>,
                                > = std::collections::HashMap::new();
                                let mut req_to_total: std::collections::HashMap<&str, Vec<&str>> =
                                    std::collections::HashMap::new();
                                for task in &spec.tasks {
                                    for req_id in &task.linked_requirements {
                                        req_to_total
                                            .entry(req_id.as_str())
                                            .or_default()
                                            .push(task.id.as_str());
                                        if task.status == ragent_specs::spec::TaskStatus::Completed
                                        {
                                            req_to_completed
                                                .entry(req_id.as_str())
                                                .or_default()
                                                .push(task.id.as_str());
                                        }
                                    }
                                }
                                lines.push("### Requirements".to_string());
                                for req in &spec.requirements {
                                    let completed = req_to_completed
                                        .get(req.id.as_str())
                                        .map_or(0, |v| v.len());
                                    let total =
                                        req_to_total.get(req.id.as_str()).map_or(0, |v| v.len());
                                    let covered = completed > 0 && completed == total;
                                    let symbol = if covered { "✅" } else { "⚪" };
                                    let detail = if total > 0 {
                                        format!(
                                            " ({} of {} linked tasks completed)",
                                            completed, total
                                        )
                                    } else {
                                        " (no linked tasks)".to_string()
                                    };
                                    lines.push(format!(
                                        "{} `{}` — {}{}",
                                        symbol, req.id, req.text, detail
                                    ));
                                }
                                lines.push(String::new());
                                lines.push("### Tasks".to_string());
                                for task in &spec.tasks {
                                    let reqs = if task.linked_requirements.is_empty() {
                                        "(unlinked)".to_string()
                                    } else {
                                        format!("[{}]", task.linked_requirements.join(", "))
                                    };
                                    let symbol = match task.status {
                                        ragent_specs::spec::TaskStatus::Completed => "✅",
                                        ragent_specs::spec::TaskStatus::InProgress => "🔄",
                                        ragent_specs::spec::TaskStatus::Blocked => "🚫",
                                        ragent_specs::spec::TaskStatus::Pending => "⏳",
                                    };
                                    lines.push(format!(
                                        "{} `{}` — {} ({}) {}",
                                        symbol,
                                        task.id,
                                        task.title,
                                        task.status.as_str(),
                                        reqs
                                    ));
                                }
                                Ok(lines.join("\n"))
                            })
                        });
                        match result {
                            Ok(output) => {
                                self.append_assistant_text(&output);
                                self.status = "spec: coverage complete".to_string();
                            }
                            Err(e) => {
                                self.status = format!("spec: {}", e);
                                self.append_assistant_text(&format!(
                                    "From: /spec coverage\n\n**Error:** {}",
                                    e
                                ));
                            }
                        }
                    }
                    SpecCommand::Impl {
                        spec_id,
                        task_id,
                        dry_run,
                    } => {
                        use ragent_specs::{ImplOptions, SpecImplRunner};
                        let working_dir = std::env::current_dir().unwrap_or_default();
                        let specs_root = working_dir.join("specs");

                        // Validate spec exists
                        let sid_opt = ragent_specs::spec::SpecId::new(&spec_id);
                        if sid_opt.is_none() {
                            self.status = format!("spec: invalid spec ID: {}", spec_id);
                            return;
                        }
                        let sid = sid_opt.unwrap();

                        // Check spec exists on disk and read its status
                        let mgr = SpecManager::new(&specs_root);
                        let rt = tokio::runtime::Handle::current();
                        let (spec_exists, spec_status): (
                            bool,
                            Option<ragent_specs::spec::SpecStatus>,
                        ) = tokio::task::block_in_place(|| {
                            rt.block_on(async {
                                let spec = mgr.read_spec(&sid).await;
                                match spec {
                                    Ok(s) => (true, Some(s.status)),
                                    Err(_) => (false, None),
                                }
                            })
                        });
                        if !spec_exists {
                            // List available specs with plans
                            let available: Vec<String> = tokio::task::block_in_place(|| {
                                rt.block_on(async {
                                    match mgr.discover_specs().await {
                                        Ok(specs) => specs
                                            .iter()
                                            .map(|s| {
                                                format!("  - {} ({})", s.id, s.status.as_str())
                                            })
                                            .collect(),
                                        Err(_) => vec![],
                                    }
                                })
                            });
                            let avail_str = if available.is_empty() {
                                "  (none found)".to_string()
                            } else {
                                available.join("\n")
                            };
                            self.append_assistant_text(&format!(
                                                                                                      "From: /spec impl\n\n**Error:** Spec `{}` not found.\n\nAvailable specs:\n{}",
                                                                                                      spec_id, avail_str
                                                                                                  ));
                            self.status = format!("spec: spec {} not found", spec_id);
                            return;
                        }

                        // Check if already implemented (FR-026)
                        if let Some(status) = spec_status {
                            if matches!(
                                status,
                                ragent_specs::spec::SpecStatus::Implemented
                                    | ragent_specs::spec::SpecStatus::Verified
                            ) {
                                self.append_assistant_text(&format!(
                                                          "From: /spec impl\n\n⚠️ Spec **{}** is already marked as **{}**.\n\n\
                                                           To re-implement, first transition the spec back to `approved` or `in_progress`:\n\
                                                           `/spec status {} approved`",
                                                          spec_id, status.as_str(), spec_id
                                                      ));
                                self.status = format!("spec: {} is {}", spec_id, status.as_str());
                                return;
                            }
                        }

                        // Build options
                        let mut opts = ImplOptions::new();
                        if let Some(ref tid) = task_id {
                            opts = opts.with_task(tid);
                        }
                        if dry_run {
                            opts = opts.with_dry_run();
                        }

                        // Create runner
                        let runner_result: Result<SpecImplRunner, String> =
                            tokio::task::block_in_place(|| {
                                rt.block_on(async {
                                    match SpecImplRunner::new(&spec_id, specs_root.clone(), opts)
                                        .await
                                    {
                                        Ok(r) => Ok(r),
                                        Err(e) => Err(format!("spec impl failed: {}", e)),
                                    }
                                })
                            });

                        match runner_result {
                            Ok(runner) => {
                                let is_dry_run = dry_run;
                                let result: Result<_, String> = tokio::task::block_in_place(|| {
                                    rt.block_on(async {
                                        match runner.run().await {
                                            Ok(r) => Ok(r),
                                            Err(e) => Err(format!("{}", e)),
                                        }
                                    })
                                });
                                match result {
                                    Ok(impl_result) => {
                                        // Display summary
                                        self.append_assistant_text(&impl_result.summary);

                                        if is_dry_run || impl_result.total_tasks == 0 {
                                            self.status =
                                                format!("spec: {} dry-run complete", spec_id);
                                        } else {
                                            // Drive tasks ONE AT A TIME. The previous
                                            // implementation injected the entire compound
                                            // prompt in a single `process_message` call,
                                            // which meant the agent would often stop after
                                            // the first task (considering its immediate goal
                                            // satisfied) and never reach the remaining
                                            // tasks. Instead, we store the execution order
                                            // and dispatch the first task's prompt now; the
                                            // `Event::MessageEnd` handler advances to the
                                            // next task once the agent marks the current
                                            // one `completed`.
                                            let task_ids: Vec<String> = runner
                                                .execution_order()
                                                .iter()
                                                .map(|&i| runner.tasks()[i].id.clone())
                                                .collect();
                                            let total = task_ids.len();
                                            self.spec_impl_state =
                                                Some(crate::app::state::SpecImplState {
                                                    spec_id: spec_id.clone(),
                                                    specs_root: specs_root.clone(),
                                                    task_ids,
                                                    current_rank: 1,
                                                    total,
                                                    runner: runner.clone(),
                                                });

                                            // Dispatch the first task's prompt.
                                            if let Some(prompt) = runner.task_prompt(1) {
                                                self.dispatch_spec_impl_task(
                                                    prompt, &spec_id, 1, total,
                                                );
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        self.append_assistant_text(&format!(
                                            "From: /spec impl\n\n**Error:** {}",
                                            e
                                        ));
                                        self.status = format!("spec: {}", e);
                                    }
                                }
                            }
                            Err(e) => {
                                self.append_assistant_text(&format!(
                                    "From: /spec impl\n\n**Error:** {}",
                                    e
                                ));
                                self.status = format!("spec: {}", e);
                            }
                        }
                    }
                    SpecCommand::Unknown(sub)
                        if sub == "create"
                            || sub == "validate"
                            || sub == "status"
                            || sub == "task"
                            || sub == "activate"
                            || sub == "coverage"
                            || sub == "impl"
                            || sub == "add"
                            || sub == "delete" =>
                    {
                        self.status = format!("Usage: /spec {} — try /spec help", sub);
                    }
                    SpecCommand::Add { spec_id, feature } => {
                        self.append_assistant_text(&SpecCommand::build_add_message(
                            &spec_id, &feature,
                        ));
                        self.push_log_no_agent(
                            crate::app::LogLevel::Info,
                            SpecCommand::build_add_log(&spec_id, &feature),
                        );

                        let working_dir = std::env::current_dir().unwrap_or_default();
                        let specs_root = working_dir.join("specs");
                        let mgr = SpecManager::new(&specs_root);
                        let rt = tokio::runtime::Handle::current();

                        // Load existing spec to get content and next IDs
                        let spec_id_owned = spec_id.clone();
                        let feature_owned = feature.clone();
                        let result: Result<(String, u32, u32, u32), String> =
                            tokio::task::block_in_place(|| {
                                rt.block_on(async {
                                    use ragent_specs::id_scanner;
                                    let sid = match ragent_specs::spec::SpecId::new(&spec_id_owned)
                                    {
                                        Some(id) => id,
                                        None => {
                                            return Err(format!(
                                                "spec: invalid spec ID: {}",
                                                spec_id_owned
                                            ));
                                        }
                                    };
                                    let spec = match mgr.read_spec(&sid).await {
                                        Ok(s) => s,
                                        Err(e) => {
                                            return Err(format!(
                                                "spec: failed to read {}: {}",
                                                spec_id_owned, e
                                            ));
                                        }
                                    };
                                    if spec.status == ragent_specs::spec::SpecStatus::Archived {
                                        return Err(format!(
                                            "spec: '{}' is archived and cannot be modified",
                                            spec_id_owned
                                        ));
                                    }
                                    let next_fr = id_scanner::highest_fr(&spec.spec_md) + 1;
                                    let next_nfr = id_scanner::highest_nfr(&spec.spec_md) + 1;
                                    let next_task = id_scanner::highest_task(&spec.plan_md) + 1;
                                    let prompt = SpecCommand::build_add_prompt(
                                        &spec_id_owned,
                                        &feature_owned,
                                        &spec.spec_md,
                                        &spec.plan_md,
                                        next_fr,
                                        next_nfr,
                                        next_task,
                                    );
                                    Ok((prompt, next_fr, next_nfr, next_task))
                                })
                            });

                        match result {
                            Ok((prompt, _next_fr, _next_nfr, _next_task)) => {
                                let sid = self.session_id.clone().unwrap_or_default();
                                let explore_agent = self
                                    .cycleable_agents
                                    .iter()
                                    .find(|a| a.name == "explore")
                                    .cloned();
                                let mut agent =
                                    explore_agent.unwrap_or_else(|| self.agent_info.clone());
                                self.apply_selected_model_and_thinking(&mut agent);
                                agent.permission = ragent_agent::agent::default_permissions();

                                let msg = Message::user_text(&sid, &prompt);
                                self.messages.push(msg);

                                let processor = self.session_processor.clone();
                                let flag = Arc::new(AtomicBool::new(false));
                                self.cancel_flag = Some(flag.clone());
                                self.is_processing = true;
                                self.status = SpecCommand::build_add_status(&spec_id);

                                let event_bus = self.event_bus.clone();
                                tokio::spawn(async move {
                                    if let Err(e) =
                                        processor.process_message(&sid, &prompt, &agent, flag).await
                                    {
                                        tracing::warn!(error = %e, "spec: add generation failed");
                                        event_bus.publish(ragent_agent::event::Event::AgentError {
                                            session_id: sid,
                                            error: format!("spec add generation failed: {e}"),
                                        });
                                    }
                                });
                            }
                            Err(e) => {
                                self.status = format!("spec: {}", e);
                                self.append_assistant_text(&format!(
                                    "From: /spec add\n\n**Error:** {}",
                                    e
                                ));
                            }
                        }
                    }
                    SpecCommand::Unknown(sub) => {
                        self.status = format!("Unknown /spec subcommand: {sub}. Try /spec help");
                    }
                }
            }
            // ── /mode ────────────────────────────────────────────────────────
            "mode" => {
                let sub = args.trim().to_lowercase();
                if sub.is_empty() || sub == "status" {
                    let current = self
                        .role_mode
                        .as_ref()
                        .map(|m| format!("{} {}", m.icon(), m.label()))
                        .unwrap_or_else(|| "normal (no role mode active)".to_string());
                    self.append_assistant_text(&format!(
                                                                      "From: /mode\nCurrent mode: **{current}**\n\n\
                                                                       Available modes: `architect` `coder` `reviewer` `debugger` `tester`\n\
                                                                       Use `/mode off` to return to normal mode."
                                                                  ));
                } else if sub == "off" || sub == "normal" {
                    self.role_mode = None;
                    self.status = "mode: normal".to_string();
                    self.append_assistant_text(
                        "From: /mode\n✅ Role mode cleared — back to normal mode.",
                    );
                    self.push_log_no_agent(LogLevel::Info, "role mode cleared".to_string());
                } else if let Some(mode) = RoleMode::from_str(&sub) {
                    let label = mode.label().to_string();
                    let icon = mode.icon().to_string();
                    self.role_mode = Some(mode);
                    self.status = format!("{icon} mode: {label}");
                    self.append_assistant_text(&format!(
                                                                      "From: /mode\n{icon} **{label} mode** activated.\n\
                                                                       The agent will now focus on {} tasks.",
                                                                      label
                                                                  ));
                    self.push_log_no_agent(LogLevel::Info, format!("role mode: {label}"));
                } else {
                    self.append_assistant_text(&format!(
                                                                      "From: /mode\nUnknown mode '{}'. \
                                                                       Available: `architect` `coder` `reviewer` `debugger` `tester` `off`",
                                                                      sub
                                                                  ));
                }
            }
            "memory" => {
                let project_mem = std::env::current_dir()
                    .unwrap_or_default()
                    .join(".ragent")
                    .join("memory")
                    .join("MEMORY.md");
                let project_analysis = std::env::current_dir()
                    .unwrap_or_default()
                    .join(".ragent")
                    .join("memory")
                    .join("PROJECT_ANALYSIS.md");
                let user_mem =
                    dirs::home_dir().map(|h| h.join(".ragent").join("memory").join("MEMORY.md"));

                match args.trim() {
                    "show" | "" => {
                        // Toggle the Memory side panel (FR-008), mirroring the
                        // `/log` and `/todo` slash aliases. The textual
                        // `/memory show` output is still appended to the chat
                        // so users get both the transcript and the live panel.
                        self.show_memory = !self.show_memory;
                        if self.show_memory {
                            // Entering Memory mode: dismiss the other side
                            // panels so only one occupies the side column
                            // (FR-004 mutual-exclusion policy).
                            self.show_log = false;
                            self.show_profile = false;
                            self.show_todo = false;
                            self.show_telemetry = false;
                        }
                        self.status = if self.show_memory {
                            "memory panel visible".to_string()
                        } else {
                            "memory panel hidden".to_string()
                        };

                        let mut output = String::from("From: /memory show\n\n");

                        let proj_content = std::fs::read_to_string(&project_mem)
                            .unwrap_or_else(|_| "(no project memory)".to_string());
                        output.push_str(&format!(
                            "## Project Memory ({})\n{}\n\n",
                            project_mem.display(),
                            proj_content
                        ));

                        if project_analysis.exists() {
                            let analysis =
                                std::fs::read_to_string(&project_analysis).unwrap_or_default();
                            output.push_str(&format!("## Project Analysis\n{}\n\n", analysis));
                        }

                        if let Some(path) = user_mem {
                            let user_content = std::fs::read_to_string(&path)
                                .unwrap_or_else(|_| "(no user memory)".to_string());
                            output.push_str(&format!(
                                "## User Memory ({})\n{}\n\n",
                                path.display(),
                                user_content
                            ));
                        }

                        self.append_assistant_text(&output);
                    }
                    sub if sub.starts_with("clear") => {
                        let scope = sub.strip_prefix("clear").unwrap_or("").trim();
                        let path = match scope {
                            "user" => dirs::home_dir()
                                .map(|h| h.join(".ragent").join("memory").join("MEMORY.md")),
                            _ => Some(
                                std::env::current_dir()
                                    .unwrap_or_default()
                                    .join(".ragent")
                                    .join("memory")
                                    .join("MEMORY.md"),
                            ),
                        };
                        if let Some(p) = path {
                            if p.exists() {
                                let _ = std::fs::remove_file(&p);
                                self.append_assistant_text(&format!(
                                    "From: /memory clear\nMemory cleared: {}",
                                    p.display()
                                ));
                            } else {
                                self.append_assistant_text(
                                    "From: /memory clear\nNo memory file found.",
                                );
                            }
                        }
                    }
                    _ => {
                        self.append_assistant_text(
                            "From: /memory\nUsage: `/memory show` | `/memory clear [project|user]`",
                        );
                    }
                }
            }

            "github" => match args.trim() {
                "login" => {
                    self.append_assistant_text(
                            "From: /github login\n🔐 Starting GitHub OAuth device flow…\n\nPlease wait for the authorization URL…",
                        );
                    let event_bus = self.event_bus.clone();
                    let sid = self.session_id.clone().unwrap_or_default();
                    tokio::spawn(async move {
                        let client_id = ragent_agent::github::GitHubClient::client_id();
                        let result = ragent_agent::github::auth::device_flow_login(
                                &client_id,
                                |user_code, verification_uri| {
                                    event_bus.publish(ragent_agent::event::Event::AgentError {
                                        session_id: sid.clone(),
                                        error: format!(
                                            "GitHub Login — visit: {verification_uri}\nEnter code: {user_code}\n\nWaiting for authorization…"
                                        ),
                                    });
                                },
                            )
                            .await;

                        match result {
                            Ok(token) => match ragent_agent::github::auth::save_token(&token) {
                                Ok(_) => {
                                    event_bus.publish(
                                                ragent_agent::event::Event::AgentError {
                                                    session_id: sid,
                                                    error: "✅ GitHub authentication successful! Token saved to ~/.ragent/github_token.".to_string(),
                                                },
                                            );
                                }
                                Err(e) => {
                                    event_bus.publish(ragent_agent::event::Event::AgentError {
                                        session_id: sid,
                                        error: format!("Failed to save GitHub token: {e}"),
                                    });
                                }
                            },
                            Err(e) => {
                                event_bus.publish(ragent_agent::event::Event::AgentError {
                                    session_id: sid,
                                    error: format!("GitHub login failed: {e}"),
                                });
                            }
                        }
                    });
                }
                "logout" => match ragent_agent::github::auth::delete_token() {
                    Ok(_) => self.append_assistant_text("From: /github\n✅ GitHub token removed."),
                    Err(e) => self.append_assistant_text(&format!(
                        "From: /github\n❌ Failed to remove token: {e}"
                    )),
                },
                "status" | "" => match ragent_agent::github::auth::load_token() {
                    Some(_) => {
                        self.append_assistant_text(
                                    "From: /github\n✅ GitHub token configured. (GITHUB_TOKEN env or ~/.ragent/github_token)",
                                );
                    }
                    None => {
                        self.append_assistant_text(
                                    "From: /github\n❌ No GitHub token configured.\n\nRun `/github login` to authenticate via OAuth device flow.",
                                );
                    }
                },
                _ => {
                    self.append_assistant_text(
                            "From: /github\nUsage: `/github login` | `/github logout` | `/github status`",
                        );
                }
            },

            "gitlab" => match args.trim() {
                "setup" => {
                    // Pre-fill from existing config if available
                    let (url, _user) = {
                        let storage = &self.storage;
                        let cfg = ragent_agent::gitlab::auth::load_config(storage.as_ref());
                        match cfg {
                            Some(c) => (c.instance_url, c.username),
                            None => ("https://gitlab.com".to_string(), String::new()),
                        }
                    };
                    self.provider_setup = Some(ProviderSetupStep::GitLabSetup {
                        url_input: url,
                        url_cursor: 0,
                        token_input: String::new(),
                        token_cursor: 0,
                        active_field: 0,
                        error: None,
                    });
                }
                "logout" => {
                    let storage = &self.storage;
                    let mut msgs = Vec::new();
                    if let Err(e) = ragent_agent::gitlab::auth::delete_token(storage.as_ref()) {
                        msgs.push(format!("❌ Failed to remove token: {e}"));
                    }
                    if let Err(e) = ragent_agent::gitlab::auth::delete_config(storage.as_ref()) {
                        msgs.push(format!("❌ Failed to remove config: {e}"));
                    }
                    if msgs.is_empty() {
                        self.append_assistant_text(
                            "From: /gitlab\n✅ GitLab configuration and token removed.",
                        );
                    } else {
                        self.append_assistant_text(&format!("From: /gitlab\n{}", msgs.join("\n")));
                    }
                }
                "status" | "" => {
                    let storage = &self.storage;
                    let config = ragent_agent::gitlab::auth::load_config(storage.as_ref());
                    let token = ragent_agent::gitlab::auth::load_token(storage.as_ref());
                    match (config, token) {
                        (Some(cfg), Some(_)) => {
                            self.append_assistant_text(&format!(
                                "From: /gitlab\n✅ GitLab configured\n\n\
                                 **Instance**: {}  \n\
                                 **Username**: {}  \n\
                                 **Token**: ✅ configured",
                                cfg.instance_url, cfg.username
                            ));
                        }
                        (Some(cfg), None) => {
                            self.append_assistant_text(&format!(
                                "From: /gitlab\n⚠️  GitLab partially configured\n\n\
                                 **Instance**: {}  \n\
                                 **Username**: {}  \n\
                                 **Token**: ❌ not set\n\n\
                                 Run `/gitlab setup` to complete configuration.",
                                cfg.instance_url, cfg.username
                            ));
                        }
                        _ => {
                            self.append_assistant_text(
                                "From: /gitlab\n❌ GitLab not configured.\n\n\
                                 Run `/gitlab setup` to configure instance URL, username, and token.",
                            );
                        }
                    }
                }
                _ => {
                    self.append_assistant_text(
                        "From: /gitlab\nUsage: `/gitlab setup` | `/gitlab logout` | `/gitlab status`",
                    );
                }
            },

            "update" => match args.trim() {
                "install" => {
                    self.append_assistant_text(
                        "From: /update install\n⬇️ Downloading latest release…",
                    );
                    let event_bus = self.event_bus.clone();
                    let sid = self.session_id.clone().unwrap_or_default();
                    tokio::spawn(async move {
                        match ragent_agent::updater::check_for_update().await {
                            Some(info) => match info.download_url {
                                Some(ref url) => {
                                    match ragent_agent::updater::download_and_replace(url).await {
                                        Ok(()) => {
                                            event_bus.publish(
                                                    ragent_agent::event::Event::AgentError {
                                                        session_id: sid,
                                                        error: format!(
                                                            "✅ Updated to v{}! Please restart ragent to use the new version.",
                                                            info.version
                                                        ),
                                                    },
                                                );
                                        }
                                        Err(e) => {
                                            event_bus.publish(
                                                ragent_agent::event::Event::AgentError {
                                                    session_id: sid,
                                                    error: format!("❌ Install failed: {e}"),
                                                },
                                            );
                                        }
                                    }
                                }
                                None => {
                                    event_bus.publish(ragent_agent::event::Event::AgentError {
                                            session_id: sid,
                                            error: format!(
                                                "⚠️  Update v{} found but no binary available for this platform.\n\nVisit https://github.com/thawkins/ragent/releases to download manually.",
                                                info.version
                                            ),
                                        });
                                }
                            },
                            None => {
                                event_bus.publish(ragent_agent::event::Event::AgentError {
                                    session_id: sid,
                                    error: format!(
                                        "✅ Already up to date (v{}).",
                                        ragent_agent::updater::CURRENT_VERSION
                                    ),
                                });
                            }
                        }
                    });
                }
                _ => {
                    self.append_assistant_text("From: /update\n🔍 Checking for updates…");
                    let event_bus = self.event_bus.clone();
                    let sid = self.session_id.clone().unwrap_or_default();
                    tokio::spawn(async move {
                        match ragent_agent::updater::check_for_update().await {
                            Some(info) => {
                                let notes = if info.body.is_empty() {
                                    "No release notes.".to_string()
                                } else {
                                    info.body.chars().take(500).collect::<String>()
                                };
                                event_bus.publish(ragent_agent::event::Event::AgentError {
                                        session_id: sid,
                                        error: format!(
                                            "🆕 Update available: **v{}**\n\n{}\n\nRun `/update install` to install.",
                                            info.version, notes
                                        ),
                                    });
                            }
                            None => {
                                event_bus.publish(ragent_agent::event::Event::AgentError {
                                    session_id: sid,
                                    error: format!(
                                        "✅ ragent is up to date (v{}).",
                                        ragent_agent::updater::CURRENT_VERSION
                                    ),
                                });
                            }
                        }
                    });
                }
            },

            "doctor" => {
                self.append_assistant_text("From: /doctor\n🩺 Running diagnostics…");
                let event_bus = self.event_bus.clone();
                let sid = self.session_id.clone().unwrap_or_default();
                let working_dir = std::env::current_dir().unwrap_or_default();
                tokio::spawn(async move {
                    let mut lines = vec!["From: /doctor\n# Diagnostic Report\n".to_string()];

                    // Check git
                    let git_ok = std::process::Command::new("git")
                        .args(["--version"])
                        .output()
                        .map(|o| o.status.success())
                        .unwrap_or(false);
                    lines.push(format!("{} git", if git_ok { "✅" } else { "❌" }));

                    // Check ripgrep
                    let rg_ok = std::process::Command::new("rg")
                        .arg("--version")
                        .output()
                        .map(|o| o.status.success())
                        .unwrap_or(false);
                    lines.push(format!(
                        "{} ripgrep (rg)",
                        if rg_ok {
                            "✅"
                        } else {
                            "❌ ripgrep not found — install at https://github.com/BurntSushi/ripgrep"
                        }
                    ));

                    // Check GitHub token
                    let gh_ok = ragent_agent::github::auth::load_token().is_some();
                    lines.push(format!(
                        "{} GitHub token",
                        if gh_ok {
                            "✅"
                        } else {
                            "⚠️  no GitHub token — run /github login"
                        }
                    ));

                    // Check memory dirs
                    let memory_dir_ok = if let Some(home) = dirs::home_dir() {
                        let p = home.join(".ragent").join("memory");
                        std::fs::create_dir_all(&p).is_ok()
                    } else {
                        false
                    };
                    lines.push(format!(
                        "{} memory directory (~/.ragent/memory/)",
                        if memory_dir_ok { "✅" } else { "❌" }
                    ));

                    // Check project .ragent dir
                    let project_ragent_ok =
                        std::fs::create_dir_all(working_dir.join(".ragent")).is_ok();
                    lines.push(format!(
                        "{} project .ragent/ directory",
                        if project_ragent_ok { "✅" } else { "❌" }
                    ));

                    // Check MCP config (field is `mcp`)
                    let mcp_configured = ragent_agent::Config::load()
                        .map(|c| !c.mcp.is_empty())
                        .unwrap_or(false);
                    lines.push(format!(
                        "{} MCP servers configured",
                        if mcp_configured {
                            "✅"
                        } else {
                            "ℹ️  no MCP servers configured (optional)"
                        }
                    ));

                    // Check for update
                    lines.push("\n**Checking for updates…**".to_string());
                    let update_msg = match ragent_agent::updater::check_for_update().await {
                        Some(info) => format!("⚠️  Update available: v{}", info.version),
                        None => {
                            format!(
                                "✅ Up to date (v{})",
                                ragent_agent::updater::CURRENT_VERSION
                            )
                        }
                    };
                    lines.push(update_msg);

                    lines.push("\n*Diagnostics complete.*".to_string());

                    event_bus.publish(ragent_agent::event::Event::AgentError {
                        session_id: sid,
                        error: lines.join("\n"),
                    });
                });
            }

            "webapi" => match args.trim() {
                "enable" | "start" => {
                    if self.webapi_server.is_some() {
                        let addr = self.webapi_addr.clone();
                        self.append_assistant_text(&format!(
                                "⚠️ Web API is already running at http://{addr}\n\nRun `/webapi disable` to stop it."
                            ));
                    } else {
                        use rand::Rng;
                        use rand::distr::Alphanumeric;
                        let token: String = rand::rng()
                            .sample_iter(&Alphanumeric)
                            .take(40)
                            .map(char::from)
                            .collect();
                        self.webapi_token = Some(token.clone());
                        let addr = self.webapi_addr.clone();

                        let config = ragent_agent::Config::load().unwrap_or_default();
                        let app_state = ragent_server::routes::AppState {
                            event_bus: self.event_bus.clone(),
                            config: std::sync::Arc::new(tokio::sync::RwLock::new(config)),
                            storage: self.storage.clone(),
                            session_processor: self.session_processor.clone(),
                            auth_token: token.clone(),
                            rate_limiter: std::sync::Arc::new(tokio::sync::Mutex::new(
                                std::collections::HashMap::new(),
                            )),
                            coordinator: None,
                        };

                        let addr_clone = addr.clone();
                        let handle = tokio::spawn(async move {
                            if let Err(e) =
                                ragent_server::routes::start_server(&addr_clone, app_state).await
                            {
                                tracing::error!("Web API server error: {e}");
                            }
                        });
                        self.webapi_server = Some(handle);

                        self.append_assistant_text(&format!(
                                                      "✅ **Web API enabled** at `http://{addr}`\n\n\
                                                          **Bearer Token:**\n```\n{token}\n```\n\
                                                          Include this token in all API requests (except `/health`):\n\
                                                          ```\nAuthorization: Bearer {token}\n```\n\n\
                                                          ### Example curl commands:\n\
                                                          ```bash\n\
                                                          # Health check (no auth required)\n\
                                                          curl http://{addr}/health\n\n\
                                                          # Get ragent status (requires auth)\n\
                                                          curl -H 'Authorization: Bearer {token}' http://{addr}/config\n\
                                                          ```\n\n\
                                                          Run `/webapi help` to see all endpoints."
                                                  ));
                    }
                }
                "disable" | "stop" => {
                    if let Some(handle) = self.webapi_server.take() {
                        handle.abort();
                        self.webapi_token = None;
                        self.append_assistant_text("🛑 **Web API disabled.**");
                    } else {
                        self.append_assistant_text(
                            "ℹ️ Web API is not running. Use `/webapi enable` to start it.",
                        );
                    }
                }
                "help" | "status" | "" => {
                    let base = format!("http://{}", self.webapi_addr);
                    let status = if self.webapi_server.is_some() {
                        format!("🟢 **Running** — {base}")
                    } else {
                        "🔴 **Disabled** — run `/webapi enable` to start".to_string()
                    };
                    let auth_note = if let Some(ref tok) = self.webapi_token {
                        let curl_example = format!(
                            "\n### Example curl commands:\n\
                                                ```bash\n\
                                                # Health check (no auth required)\n\
                                                curl {base}/health\n\n\
                                                # Get ragent status (requires auth)\n\
                                                curl -H 'Authorization: Bearer {tok}' {base}/config\n\
                                                ```"
                        );
                        format!(
                            "\n**Bearer Token:** `{tok}`\n\
                                                      Add `Authorization: Bearer {tok}` to all requests (except `/health`).{curl_example}"
                        )
                    } else {
                        "\n*No token set — start the server with `/webapi enable`.*".to_string()
                    };
                    self.append_assistant_text(&format!(
                            "## 🌐 Web API\n\n\
                            **Status:** {status}{auth_note}\n\n\
                            ### Endpoints\n\n\
                            | Method | Path | Description |\n\
                            |--------|------|-------------|\n\
                            | `GET` | [{base}/health]({base}/health) | Health check — no auth required |\n\
                            | `GET` | [{base}/config]({base}/config) | Get application configuration |\n\
                            | `GET` | [{base}/providers]({base}/providers) | List available LLM providers |\n\
                            | `GET` | [{base}/sessions]({base}/sessions) | List all sessions |\n\
                            | `POST` | [{base}/sessions]({base}/sessions) | Create session · body: `{{\"directory\": \"/path\"}}` |\n\
                            | `GET` | [{base}/sessions/{{id}}]({base}/sessions) | Get session details |\n\
                            | `DELETE` | [{base}/sessions/{{id}}]({base}/sessions) | Archive a session |\n\
                            | `GET` | [{base}/sessions/{{id}}/messages]({base}/sessions) | List session messages |\n\
                            | `POST` | [{base}/sessions/{{id}}/messages]({base}/sessions) | Send message · body: `{{\"content\": \"...\", \"attachments\": []}}` |\n\
                            | `POST` | [{base}/sessions/{{id}}/abort]({base}/sessions) | Abort current operation |\n\
                            | `POST` | [{base}/sessions/{{id}}/permission/{{req_id}}]({base}/sessions) | Reply to permission · body: `{{\"allow\": true}}` |\n\
                            | `GET` | [{base}/sessions/{{id}}/tasks]({base}/sessions) | List background tasks |\n\
                            | `POST` | [{base}/sessions/{{id}}/tasks]({base}/sessions) | Spawn a background task |\n\
                            | `GET` | [{base}/sessions/{{id}}/tasks/{{tid}}]({base}/sessions) | Get task status |\n\
                            | `DELETE` | [{base}/sessions/{{id}}/tasks/{{tid}}]({base}/sessions) | Cancel a task |\n\
                            | `GET` | [{base}/events]({base}/events) | SSE stream for real-time events |\n\
                            | `POST` | [{base}/opt]({base}/opt) | Optimise a prompt |\n\
                            | `GET` | [{base}/orchestrator/metrics]({base}/orchestrator/metrics) | Orchestration metrics |\n\
                            | `POST` | [{base}/orchestrator/start]({base}/orchestrator/start) | Start orchestration job |\n\
                            | `GET` | [{base}/orchestrator/jobs/{{id}}]({base}/orchestrator/jobs) | Get job status |\n\n\
                            ### Quick start\n\
                            ```bash\n\
                            # Health check\n\
                            curl {base}/health\n\n\
                            # List sessions (replace TOKEN)\n\
                            curl -H 'Authorization: Bearer TOKEN' {base}/sessions\n\n\
                            # Send a message\n\
                            curl -X POST -H 'Authorization: Bearer TOKEN' \\\n\
                              -H 'Content-Type: application/json' \\\n\
                              -d '{{\"content\": \"Hello!\"}}' \\\n\
                              {base}/sessions/SESSION_ID/messages\n\
                            ```"
                        ));
                }
                _ => {
                    self.append_assistant_text(
                        "Usage: `/webapi enable` · `/webapi disable` · `/webapi help`",
                    );
                }
            },

            "websearch" => {
                let sub = args.split_whitespace().next().unwrap_or("");
                match sub {
                    "help" => {
                        self.append_assistant_text(
                                                    "From: /websearch\n\
                                                    Web search engine diagnostics.\n\n\
                                                    Usage:\n\n\
                                                    • `/websearch show`\n\
                                                      — list all engines with enabled / in-use / failed status\n\n\
                                                    • `/websearch test`\n\
                                                      — run a live diagnostic query on each configured engine and report counts\n\n\
                                                    • `/websearch help`\n\
                                                      — show this help",
                                                );
                        self.status = "websearch: help".to_string();
                    }
                    "test" => {
                        self.status = "websearch: testing engines...".to_string();
                        let config = ragent_config::Config::load().unwrap_or_default();
                        let ctx = ragent_tools_extended::ToolContext {
                            session_id: String::new(),
                            working_dir: std::env::current_dir().unwrap_or_default(),
                            event_bus: Arc::new(ragent_agent::event::EventBus::new(16)),
                            storage: None,
                            code_index: None,
                            config: Some(Arc::new(config)),
                            read_timestamps: Arc::new(std::sync::RwLock::new(
                                std::collections::HashMap::new(),
                            )),
                        };
                        use ragent_tools_extended::masterfetch::tools::search_tool::MfSearchTool;
                        let results = tokio::task::block_in_place(|| {
                            let rt = tokio::runtime::Handle::current();
                            rt.block_on(MfSearchTool::engine_test(&ctx))
                        });
                        let mut output = String::from(
                            "From: /websearch test\n\n\
                                                                | Engine | Returned | Count |\n\
                                                                |--------|:--------:|------:|\n",
                        );
                        let mut total = 0usize;
                        for r in &results {
                            let returned = if r.returned_results {
                                "✅ yes"
                            } else {
                                "❌ no"
                            };
                            output.push_str(&format!(
                                "| {:<10} | {} | {:>5} |\n",
                                r.name, returned, r.result_count
                            ));
                            total += r.result_count;
                        }
                        output.push_str(&format!("\nTotal raw results: {total}"));
                        self.append_assistant_text(&output);
                        self.status = "websearch: engine test complete".to_string();
                    }
                    "show" | "" => {
                        let config = ragent_config::Config::load().unwrap_or_default();
                        let ctx = ragent_tools_extended::ToolContext {
                            session_id: String::new(),
                            working_dir: std::env::current_dir().unwrap_or_default(),
                            event_bus: Arc::new(ragent_agent::event::EventBus::new(16)),
                            storage: None,
                            code_index: None,
                            config: Some(Arc::new(config)),
                            read_timestamps: Arc::new(std::sync::RwLock::new(
                                std::collections::HashMap::new(),
                            )),
                        };
                        use ragent_tools_extended::masterfetch::tools::search_tool::MfSearchTool;
                        let engines = MfSearchTool::engine_status(&ctx);
                        let mut output = String::from(
                            "From: /websearch show\n\n\
                                          | Engine | Enabled | In Use | Failed |\n\
                                          |--------|:-------:|:------:|:------:|\n",
                        );
                        for e in engines {
                            let enabled = if e.enabled { "✅ yes" } else { "❌ no" };
                            let in_use = if e.in_use { "✅ yes" } else { "❌ no" };
                            let failed = if e.failed { "⚠️ yes" } else { "✅ no" };
                            output.push_str(&format!(
                                "| {:<10} | {} | {} | {} |\n",
                                e.name, enabled, in_use, failed
                            ));
                        }
                        output.push_str(
                                          "\nKeyless engines (DuckDuckGo, Brave) are always enabled. \
                                           LangSearch requires `langsearch_api_key` in `ragent.json`. \
                                           Tavily requires `tavily_api_key` in `ragent.json` or the \
                                           `TAVILY_API_KEY` environment variable.",
                                      );
                        self.append_assistant_text(&output);
                        self.status = "websearch: status shown".to_string();
                    }
                    _ => {
                        self.append_assistant_text(
                                          "From: /websearch\n\
                                          ⚠ Unknown subcommand. Use `/websearch help` for available commands.",
                                      );
                        self.status = "websearch: unknown".to_string();
                    }
                }
            }

            "mouse" => {
                let sub = args.split_whitespace().next().unwrap_or("");
                match sub {
                    "on" => {
                        self.mouse_enabled = true;
                        self.append_assistant_text(
                                                      "From: /mouse on\n✅ **Mouse support enabled.**\n\nYou can now use the mouse for scrolling, clicking, and selection."
                                                  );
                        self.status = "mouse: enabled".to_string();
                    }
                    "off" => {
                        self.mouse_enabled = false;
                        self.append_assistant_text(
                                        "From: /mouse off\n✅ **Mouse support disabled.**\n\nKeyboard-only mode active. All mouse interactions are now disabled.\n\nKeyboard shortcuts:\n• Alt+Up/Down: Focus teammates\n• Tab: Navigate UI elements\n• Enter: Select/activate\n• Esc: Close dialogs\n• Ctrl+C: Copy selection\n• Ctrl+V: Paste"
                                    );
                        self.status = "mouse: disabled (keyboard-only mode)".to_string();
                    }
                    _ => {
                        let status = if self.mouse_enabled {
                            "enabled"
                        } else {
                            "disabled"
                        };
                        self.append_assistant_text(&format!(
                                                                              "From: /mouse\n\nMouse support is currently **{}**.\n\nUsage: `/mouse on` | `/mouse off`",
                                                                              status
                                                                          ));
                        self.status = format!("mouse: {}", status);
                    }
                }
            }

            "codeindex" => {
                let sub = args.split_whitespace().next().unwrap_or("");
                match sub {
                    "on" | "enable" => {
                        self.code_index_enabled = true;
                        // Use the explicit setter so the value is marked as
                        // user-specified and persisted to the config file (it
                        // would otherwise be stripped by the default-omission
                        // behaviour and lost on restart if a global config
                        // disagrees).
                        self.tool_visibility.set_codeindex(true);
                        let mut cfg = ragent_agent::Config::load().unwrap_or_default();
                        cfg.code_index.set_enabled(true);
                        cfg.tool_visibility = self.tool_visibility.clone();
                        self.sync_tool_visibility_from_config(&cfg);
                        if self.code_index.is_some() {
                            // Already active — just ensure watcher is running
                            if self.code_index_watch_session.is_none() {
                                if let Some(ref idx) = self.code_index {
                                    match ragent_codeindex::start_watching(
                                        idx.clone(),
                                        ragent_codeindex::worker::WorkerConfig::default(),
                                    ) {
                                        Ok(session) => {
                                            self.code_index_watch_session = Some(session);
                                            self.append_assistant_text(
                                                "✅ **Code index** is already active. File watcher started.",
                                            );
                                        }
                                        Err(e) => {
                                            self.append_assistant_text(&format!(
                                                "✅ **Code index** is already active.\n\n⚠️ Could not start file watcher: {e}",
                                            ));
                                        }
                                    }
                                }
                            } else {
                                self.append_assistant_text(
                                    "✅ **Code index** is already active and enabled.",
                                );
                            }
                        } else {
                            // Create and initialize the code index
                            let cwd = std::env::current_dir().unwrap_or_default();
                            let index_config = ragent_codeindex::types::CodeIndexConfig {
                                enabled: true,
                                project_root: cwd.clone(),
                                index_dir: cwd.join(".ragent/codeindex"),
                                scan_config: ragent_codeindex::types::ScanConfig::default(),
                            };
                            match ragent_codeindex::CodeIndex::open(&index_config) {
                                Ok(idx) => {
                                    let arc_idx = Arc::new(idx);
                                    // Start watching — this performs an initial full reindex
                                    // to catch any changes made while the index was disabled.
                                    match ragent_codeindex::start_watching(
                                        arc_idx.clone(),
                                        ragent_codeindex::worker::WorkerConfig::default(),
                                    ) {
                                        Ok(session) => {
                                            self.code_index_watch_session = Some(session);
                                        }
                                        Err(e) => {
                                            tracing::warn!(error = %e, "Failed to start file watcher on codeindex enable");
                                        }
                                    }
                                    self.set_code_index(Some(arc_idx));
                                    self.append_assistant_text(
                                        "✅ **Code index:** enabled and activated. Background reindex in progress.",
                                    );
                                }
                                Err(e) => {
                                    self.append_assistant_text(&format!(
                                        "❌ **Code index:** could not open index: {e}",
                                    ));
                                }
                            }
                        }
                        match cfg.save_to_source() {
                            Ok(()) => {
                                // P-2: invalidate the cached config so the next turn
                                // picks up the newly-saved file.
                                self.session_processor.invalidate_config_cache();
                                self.status = "codeindex: on".to_string();
                            }
                            Err(e) => {
                                self.append_assistant_text(&format!(
                                    "⚠️ **Code index:** enabled in memory, but saving config failed: {e}",
                                ));
                                self.status = "codeindex: on (unsaved)".to_string();
                                self.push_log_no_agent(
                                    LogLevel::Warn,
                                    format!("codeindex enable save failed: {}", e),
                                );
                            }
                        }
                    }
                    "off" | "disable" => {
                        self.code_index_enabled = false;
                        // Use the explicit setter so `enabled: false` is marked
                        // user-specified and written to the config file. Without
                        // this, a config that previously omitted `enabled` (i.e.
                        // defaulted to true) would not persist the disable, and
                        // codeindex would silently re-enable on restart.
                        self.tool_visibility.set_codeindex(false);
                        let mut cfg = ragent_agent::Config::load().unwrap_or_default();
                        cfg.code_index.set_enabled(false);
                        cfg.tool_visibility = self.tool_visibility.clone();
                        self.sync_tool_visibility_from_config(&cfg);
                        let was_active = self.code_index.is_some();
                        // Stop the file watcher + background worker first
                        if let Some(ref mut session) = self.code_index_watch_session {
                            session.stop();
                        }
                        self.code_index_watch_session = None;
                        self.code_index = None;
                        self.code_index_stats_cache = None;
                        if was_active {
                            self.append_assistant_text(
                                "⛔ **Code index:** disabled and deactivated. Codeindex tools will no longer be available.\n\n\
                                 Use `/codeindex on` and restart to re-enable.",
                            );
                        } else {
                            self.append_assistant_text(
                                "ℹ️ **Code index:** disabled. It was not currently active.",
                            );
                        }
                        match cfg.save_to_source() {
                            Ok(()) => {
                                // P-2: invalidate the cached config so the next turn
                                // picks up the newly-saved file.
                                self.session_processor.invalidate_config_cache();
                                self.status = "codeindex: off".to_string();
                            }
                            Err(e) => {
                                self.append_assistant_text(&format!(
                                    "⚠️ **Code index:** disabled in memory, but saving config failed: {e}",
                                ));
                                self.status = "codeindex: off (unsaved)".to_string();
                                self.push_log_no_agent(
                                    LogLevel::Warn,
                                    format!("codeindex disable save failed: {}", e),
                                );
                            }
                        }
                    }
                    "show" | "status" | "" => {
                        let config_enabled = self.code_index_enabled;
                        // Check if we have an active code index with real stats
                        if let Some(ref idx) = self.code_index {
                            match idx.status() {
                                Ok(stats) => {
                                    let mut output = String::from("## Code Index Status\n\n");
                                    output.push_str(&format!(
                                        "**Enabled:** {}\n",
                                        if config_enabled {
                                            "\u{2713} yes"
                                        } else {
                                            "\u{2717} no"
                                        }
                                    ));
                                    output.push_str(&format!(
                                        "**Files indexed:** {}\n",
                                        stats.files_indexed
                                    ));
                                    output.push_str(&format!(
                                        "**Total symbols:** {}\n",
                                        stats.total_symbols
                                    ));
                                    output.push_str(&format!(
                                        "**FTS documents:** {}\n",
                                        stats.fts_doc_count
                                    ));
                                    output.push_str(&format!(
                                        "**References:** {}\n",
                                        stats.total_references
                                    ));
                                    output.push_str(&format!(
                                        "**Total size:** {:.1} KB\n",
                                        stats.total_bytes as f64 / 1024.0
                                    ));

                                    // FTS sync warning
                                    if stats.total_symbols > 0 && stats.fts_doc_count == 0 {
                                        output.push_str("\n\u{26a0}\u{fe0f} **FTS index is empty** — search will not work. Use `/codeindex rebuild` to fix.\n");
                                    } else if stats.fts_doc_count > 0
                                        && (stats.fts_doc_count as f64
                                            / stats.total_symbols.max(1) as f64)
                                            < 0.5
                                    {
                                        output.push_str(&format!(
                                            "\n\u{26a0}\u{fe0f} **FTS index may be out of sync** ({} FTS docs vs {} symbols). Use `/codeindex rebuild` to fix.\n",
                                            stats.fts_doc_count, stats.total_symbols
                                        ));
                                    }

                                    if !stats.languages.is_empty() {
                                        output.push_str("**Languages:** ");
                                        let langs: Vec<String> = stats
                                            .languages
                                            .iter()
                                            .map(|(lang, count)| format!("{lang} ({count})"))
                                            .collect();
                                        output.push_str(&langs.join(", "));
                                        output.push('\n');
                                    }

                                    if let Some(ts) = &stats.last_full_index {
                                        output.push_str(&format!("**Last full index:** {ts}\n"));
                                    }
                                    if let Some(ts) = &stats.last_incremental_update {
                                        output.push_str(&format!("**Last incremental:** {ts}\n"));
                                    }
                                    output.push_str(&format!(
                                        "**Index size:** {:.1} KB\n",
                                        stats.index_size_bytes as f64 / 1024.0
                                    ));
                                    self.append_assistant_text(&output);
                                    self.status = format!(
                                        "codeindex: {} files, {} symbols, {} FTS docs",
                                        stats.files_indexed,
                                        stats.total_symbols,
                                        stats.fts_doc_count
                                    );
                                }
                                Err(e) => {
                                    self.append_assistant_text(&format!(
                                        "## Code Index Status\n\n\u{26a0}\u{fe0f} Error reading index stats: {e}"
                                    ));
                                    self.status = "codeindex: error".to_string();
                                }
                            }
                        } else {
                            // No active code index
                            let state = if config_enabled {
                                "enabled"
                            } else {
                                "disabled"
                            };
                            self.append_assistant_text(&format!(
                                "## Code Index Status\n\n\
                                 **Enabled:** {}\n\n\
                                 Code index is not currently active. It may not yet be initialised.\n\n\
                                 Use `/codeindex on` to enable indexing, \
                                 or run `/codeindex help` for available sub-commands.",
                                if config_enabled { "\u{2713} yes" } else { "\u{2717} no" }
                            ));
                            self.status = format!("codeindex: {state}").to_string();
                        }
                    }
                    "reindex" => {
                        if let Some(idx) = self.code_index.clone() {
                            self.append_assistant_text(
                                "🔄 **Re-indexing codebase...** scanning files and extracting symbols.",
                            );
                            match idx.full_reindex() {
                                Ok(result) => {
                                    self.append_assistant_text(&format!(
                                        "✅ Re-index complete: +{} ~{} -{} files, {} symbols in {}ms.",
                                        result.files_added,
                                        result.files_updated,
                                        result.files_removed,
                                        result.symbols_extracted,
                                        result.elapsed_ms,
                                    ));
                                    self.status = format!(
                                        "codeindex: reindexed {} files",
                                        result.files_added + result.files_updated
                                    );
                                }
                                Err(e) => {
                                    self.append_assistant_text(&format!("❌ Re-index failed: {e}"));
                                    self.status = "codeindex: reindex failed".to_string();
                                }
                            }
                        } else {
                            self.append_assistant_text(
                                "⚠️ Code index is not active. Enable it first with `/codeindex on`.",
                            );
                            self.status = "codeindex: not active".to_string();
                        }
                    }
                    "lang" | "languages" => {
                        let languages = ragent_codeindex::scanner::SUPPORTED_LANGUAGES;
                        let mut output = String::from("## Supported Languages\n\n");
                        output.push_str("The code index supports the following languages:\n\n");
                        for (i, lang) in languages.iter().enumerate() {
                            if i % 5 == 0 && i > 0 {
                                output.push('\n');
                            }
                            output.push_str(&format!("`{lang}`  "));
                        }
                        output.push_str(&format!("\n\n**Total: {} languages**", languages.len()));
                        self.append_assistant_text(&output);
                        self.status = format!("codeindex: {} languages supported", languages.len());
                    }
                    "rebuild" => {
                        if let Some(idx) = self.code_index.clone() {
                            self.append_assistant_text(
                                "\u{1f504} **Rebuilding FTS index** from SQLite data...",
                            );
                            match idx.rebuild_fts() {
                                Ok(()) => {
                                    let fts_count =
                                        idx.status().map(|s| s.fts_doc_count).unwrap_or(0);
                                    self.append_assistant_text(&format!(
                                        "\u{2705} FTS rebuild complete: {} documents indexed.",
                                        fts_count,
                                    ));
                                    self.status =
                                        format!("codeindex: FTS rebuilt ({fts_count} docs)");
                                }
                                Err(e) => {
                                    self.append_assistant_text(&format!(
                                        "\u{274c} FTS rebuild failed: {e}"
                                    ));
                                    self.status = "codeindex: rebuild failed".to_string();
                                }
                            }
                        } else {
                            self.append_assistant_text(
                                "\u{26a0}\u{fe0f} Code index is not active. Enable it first with `/codeindex on`.",
                            );
                        }
                    }
                    "help" => {
                        self.append_assistant_text(
                                                  "## /codeindex \u{2014} Codebase Index Management\n\n\
                                                   | Sub-command | Description |\n\
                                                   |-------------|-------------|\n\
                                                   | `/codeindex on` | Enable codebase indexing |\n\
                                                   | `/codeindex off` | Disable codebase indexing |\n\
                                                   | `/codeindex show` | Show index status and statistics |\n\
                                                   | `/codeindex lang` | List supported languages |\n\
                                                   | `/codeindex reindex` | Trigger a full re-index |\n\
                                                   | `/codeindex rebuild` | Rebuild FTS index from SQLite |\n\
                                                   | `/codeindex help` | Show this help |\n\n\
                                                   When enabled, the agent has access to these tools:\n\
                                                   - `codeindex_search` \u{2014} Full-text search for symbols and docs\n\
                                                   - `codeindex_symbols` \u{2014} Structured symbol query\n\
                                                   - `codeindex_references` \u{2014} Find all references to a symbol\n\
                                                   - `codeindex_dependencies` \u{2014} File dependency graph\n\
                                                   - `codeindex_status` \u{2014} Index statistics\n\
                                                   - `codeindex_reindex` \u{2014} Trigger full re-index",
                                              );
                        self.status = "codeindex: help".to_string();
                    }
                    _ => {
                        self.append_assistant_text(
                            "Usage: `/codeindex on|off|show|lang|reindex|rebuild|help`",
                        );
                    }
                }
            }

            "router" => {
                let rest = args.trim();
                let sub = rest.split_whitespace().next().unwrap_or("");
                match sub {
                    "help" | "" => {
                        self.append_assistant_text(
                                                                      "From: /router\n\
                                                                       ## /router — Model Router Management\n\n\
                                                                       | Sub-command | Description |\n\
                                                                       |-------------|-------------|\n\
                                                                       | `/router on` | Enable the router (set `enabled: true`) |\n\
                                                                       | `/router off` | Disable the router (set `enabled: false`) |\n\
                                                                       | `/router status` | Show router state, current tier, and enabled/disabled status |\n\
                                                                       | `/router tiers` | Display all tier mappings and their model lists |\n\
                                                                       | `/router tier <name> set <provider>/<model>` | Set the primary model for a tier |\n\
                                                                       | `/router tier <name> add <provider>/<model>` | Append a fallback model to a tier |\n\
                                                                       | `/router tier <name> remove <provider>/<model>` | Remove a model from a tier's list |\n\
                                                                       | `/router weights` | Display the 15 dimension weights |\n\
                                                                       | `/router weights set <dimension> <value>` | Override a single dimension weight |\n\
                                                                       | `/router weights reset` | Restore built-in default weights |\n\
                                                                       | `/router boundaries` | Display the three tier boundary thresholds |\n\
                                                                       | `/router boundaries set <boundary> <value>` | Set a boundary threshold (0.0–1.0) |\n\
                                                                       | `/router test <prompt>` | Classify a prompt and show dimension scores, composite score, and selected tier |\n\
                                                                       | `/router stats` | Display cumulative routing statistics |\n\
                                                                       | `/router stats reset` | Zero out cumulative routing statistics |\n\
                                                                       | `/router reload` | Reload router config from `ragent.json` |\n\
                                                                       | `/router help` | Show this help |\n\n\
                                                                       The router analyses every prompt using a 15-dimension weighted classifier\n\
                                                                       and automatically selects the cheapest model that can satisfy the request.\n\
                                                                       Tiers: SIMPLE, MEDIUM, COMPLEX, REASONING.\n\n\
                                                                       **Prompt modifiers** — prefix your prompt to force a tier:\n\
                                                                       - `/simple`, `/medium`, `/complex`, `/max`, `/reasoning`, `/basic`, `/cheap`, `/balanced`, `/advanced`, `/think`, `/deep`\n\
                                                                       - `[simple]`, `[complex]`, etc.\n\
                                                                       - `simple mode:`, `deep mode:`, etc.",
                                                                  );
                        self.status = "router: help".to_string();
                    }
                    "reload" => {
                        // Find the config file path
                        let cwd = std::env::current_dir().unwrap_or_default();
                        let project_config = cwd.join(".ragent").join("ragent.json");
                        let config_dir = dirs::config_dir()
                            .unwrap_or_else(|| cwd.clone())
                            .join("ragent");
                        let global_config = config_dir.join("ragent.json");

                        let config_path = if project_config.exists() {
                            Some(project_config)
                        } else if global_config.exists() {
                            Some(global_config)
                        } else {
                            None
                        };

                        match config_path {
                            Some(ref path) => {
                                match std::fs::read_to_string(path) {
                                    Ok(raw) => {
                                        match serde_json::from_str::<serde_json::Value>(&raw) {
                                            Ok(json) => {
                                                let router_json = json
                                                    .get("provider")
                                                    .and_then(|p| p.get("router"));

                                                match router_json {
                                                    Some(rj) => {
                                                        match serde_json::from_value::<ragent_agent::provider::router_config::RouterConfig>(rj.clone()) {
                                                                                                                                                                      Ok(router_config) => {
                                                                                                                                                                          self.router_enabled = router_config.enabled;
                                                                                                                                                                          self.router_current_tier = None; // reset on reload
                                                                                                                                                                          self.append_assistant_text(&format!(
                                                                                                                                                                              "From: /router\n✓ Router config reloaded from {}\n  enabled: {}\n  tiers: {}\n  boundaries: {:.2}/{:.2}/{:.2}",
                                                                                                                                                                              path.display(),
                                                                                                                                                                              router_config.enabled,
                                                                                                                                                                              router_config.tiers.len(),
                                                                                                                                                                              router_config.boundaries.simple_medium,
                                                                                                                                                                              router_config.boundaries.medium_complex,
                                                                                                                                                                              router_config.boundaries.complex_reasoning,
                                                                                                                                                                          ));
                                                                                                                                                                          self.push_log_no_agent(
                                                                                                                                                                              LogLevel::Info,
                                                                                                                                                                              format!("router reload: config reloaded from {}", path.display()),
                                                                                                                                                                          );
                                                                                                                                                                          self.status = "router: reloaded".to_string();
                                                                                                                                                                      }
                                                                                                                                                                      Err(e) => {
                                                                                                                                                                          self.append_assistant_text(&format!(
                                                                                                                                                                              "From: /router\n⚠ Failed to parse provider.router: {}",
                                                                                                                                                                              e
                                                                                                                                                                          ));
                                                                                                                                                                          self.status = "router: reload parse error".to_string();
                                                                                                                                                                      }
                                                                                                                                                                  }
                                                    }
                                                    None => {
                                                        self.append_assistant_text(
                                                                                                                                                                      "From: /router\n⚠ No `provider.router` section found in ragent.json. \
                                                                                                                                                                       Add a `router` object under `provider` to configure the router.",
                                                                                                                                                                  );
                                                        self.status =
                                                            "router: no config".to_string();
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                self.append_assistant_text(&format!(
                                                                                                                                                              "From: /router\n⚠ Failed to parse ragent.json: {}",
                                                                                                                                                              e
                                                                                                                                                          ));
                                                self.status =
                                                    "router: reload parse error".to_string();
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        self.append_assistant_text(&format!(
                                            "From: /router\n⚠ Failed to read {}: {}",
                                            path.display(),
                                            e
                                        ));
                                        self.status = "router: reload read error".to_string();
                                    }
                                }
                            }
                            None => {
                                self.append_assistant_text(
                                                                                                                                              "From: /router\n⚠ No ragent.json found. Create `.ragent/ragent.json` with a `provider.router` section.",
                                                                                                                                          );
                                self.status = "router: reload failed".to_string();
                            }
                        }
                    }
                    "on" => {
                        self.router_enabled = true;
                        self.append_assistant_text(
                                                                      "From: /router\n✓ Router enabled. Prompts will be classified and routed to the appropriate model.",
                                                                  );
                        self.status = "router: on".to_string();
                    }
                    "off" => {
                        self.router_enabled = false;
                        self.append_assistant_text(
                                                                      "From: /router\n✓ Router disabled. All prompts will use the default model.",
                                                                  );
                        self.status = "router: off".to_string();
                    }
                    "status" => {
                        let enabled_str = if self.router_enabled {
                            "enabled"
                        } else {
                            "disabled"
                        };
                        let tier_str = self.router_current_tier.as_deref().unwrap_or("none");
                        self.append_assistant_text(&format!(
                                                                      "From: /router\n\
                                                                       Router status: **{}**\n\
                                                                       Current tier: {}\n\
                                                                       \n\
                                                                       Use `/router tiers` to see model mappings, or `/router test <prompt>` to classify a prompt.",
                                                                      enabled_str, tier_str,
                                                                  ));
                        self.status = "router: status".to_string();
                    }
                    "test" => {
                        let prompt = rest.strip_prefix("test").unwrap_or("").trim();
                        if prompt.is_empty() {
                            self.append_assistant_text(
                                                                          "From: /router\nUsage: `/router test <prompt>` — classify a prompt and show scores.",
                                                                      );
                        } else {
                            use ragent_agent::provider::router_classifier::{
                                AttachmentInfo, PromptClassifier,
                            };
                            use ragent_agent::provider::router_config::{
                                BoundaryConfig, WeightConfig,
                            };
                            let weights = WeightConfig::default();
                            let boundaries = BoundaryConfig::default();
                            let result = PromptClassifier::classify(
                                prompt,
                                None,
                                &weights,
                                &boundaries,
                                &AttachmentInfo::default(),
                            );
                            let mut table = String::from(
                                "From: /router\n```\nDimension                 Score\n───────────────────────────────\n",
                            );
                            for i in 0..15 {
                                let name =
                                    ragent_agent::provider::router_classifier::dimension_name(i);
                                table.push_str(&format!(
                                    "{:<26}{:.3}\n",
                                    name, result.dimension_scores[i]
                                ));
                            }
                            table.push_str(&format!("───────────────────────────────\n"));
                            table.push_str(&format!(
                                "{:<26}{:.3}\n",
                                "composite", result.composite_score
                            ));
                            table.push_str(&format!("{:<26}{}\n", "tier", result.tier));
                            if result.requires_vision {
                                table.push_str(&format!("{:<26}{}\n", "requires_vision", "true"));
                            }
                            table.push_str("```\n");
                            self.append_assistant_text(&table);
                            self.router_current_tier = Some(result.tier.to_string());
                            self.status = format!("router: test → {}", result.tier);
                        }
                    }
                    "weights" => {
                        use ragent_agent::provider::router_config::WeightConfig;
                        let sub2 = rest.strip_prefix("weights").unwrap_or("").trim();
                        if sub2.is_empty() {
                            let w = WeightConfig::default();
                            let mut table = String::from(
                                "From: /router\n```\nDimension                 Weight\n───────────────────────────────\n",
                            );
                            for i in 0..15 {
                                let name =
                                    ragent_agent::provider::router_classifier::dimension_name(i);
                                table.push_str(&format!(
                                    "{:<26}{:.2}\n",
                                    name,
                                    w.weight_by_index(i)
                                ));
                            }
                            table.push_str("```\n");
                            self.append_assistant_text(&table);
                            self.status = "router: weights".to_string();
                        } else {
                            self.append_assistant_text(
                                                                          "From: /router\n⚠ Per-dimension weight override is not yet supported. \
                                                                           Configure `provider.router.weights` in `ragent.json`.",
                                                                      );
                            self.status = "router: weights not supported".to_string();
                        }
                    }
                    "boundaries" => {
                        use ragent_agent::provider::router_config::BoundaryConfig;
                        let sub2 = rest.strip_prefix("boundaries").unwrap_or("").trim();
                        if sub2.is_empty() {
                            let b = BoundaryConfig::default();
                            self.append_assistant_text(&format!(
                                                                          "From: /router\n```\nBoundary            Threshold\n──────────────────────────\nSIMPLE → MEDIUM     {:.2}\nMEDIUM → COMPLEX    {:.2}\nCOMPLEX → REASONING {:.2}\n```",
                                                                          b.simple_medium, b.medium_complex, b.complex_reasoning,
                                                                      ));
                            self.status = "router: boundaries".to_string();
                        } else {
                            self.append_assistant_text(
                                                                          "From: /router\n⚠ Boundary override is not yet supported. \
                                                                           Configure `provider.router.boundaries` in `ragent.json`.",
                                                                      );
                            self.status = "router: boundaries not supported".to_string();
                        }
                    }
                    "tiers" => {
                        use ragent_agent::provider::router_config::{RouterConfig, Tier};
                        let config = RouterConfig::default();
                        let mut output = String::from(
                            "From: /router\n```\nTier      Provider/Model                         Timeout\n─────────────────────────────────────────────────\n",
                        );
                        for tier in Tier::all() {
                            let tc = config.tier_config(*tier);
                            for (i, entry) in tc.models.iter().enumerate() {
                                let prefix = if i == 0 {
                                    tier.to_string()
                                } else {
                                    "  fallback".to_string()
                                };
                                let timeout = tc
                                    .timeout_ms
                                    .map(|ms| format!("{}ms", ms))
                                    .unwrap_or_else(|| "default".to_string());
                                output.push_str(&format!(
                                    "{:<10}{}/{}  {}\n",
                                    prefix, entry.provider, entry.model, timeout
                                ));
                            }
                        }
                        output.push_str("```\n");
                        self.append_assistant_text(&output);
                        self.status = "router: tiers".to_string();
                    }
                    _ => {
                        self.append_assistant_text(
                                                                                                  "From: /router\n⚠ Unknown subcommand. Use `/router help` for available commands.",
                                                                                              );
                        self.status = "router: unknown".to_string();
                    }
                }
            }
            _ => {
                let working_dir = std::env::current_dir().unwrap_or_default();
                let skill_dirs = ragent_agent::Config::load()
                    .map(|c| c.skill_dirs)
                    .unwrap_or_default();
                let registry = ragent_agent::skill::SkillRegistry::load(&working_dir, &skill_dirs);
                if let Some(skill) = registry.get(cmd) {
                    if !skill.user_invocable {
                        self.status = format!("Skill '{}' is not user-invocable", cmd);
                        self.push_log_no_agent(
                            LogLevel::Warn,
                            format!("Skill /{} is not user-invocable", cmd),
                        );
                        return;
                    }
                    // Check provider/model are configured
                    if self.configured_provider.is_none() {
                        self.status =
                            "⚠ No provider configured — use /provider to set up".to_string();
                        return;
                    }
                    if self.selected_model.is_none() {
                        self.status = "⚠ No model selected — use /model to choose".to_string();
                        return;
                    }
                    // Ensure a session exists
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

                    let sid = self.session_id.clone().unwrap_or_default();
                    let skill = skill.clone();
                    let args_owned = args.to_string();
                    let processor = self.session_processor.clone();

                    let agent = ragent_agent::skill::invoke::resolve_inline_skill_agent(
                        &self.agent_info,
                        self.selected_model.as_deref(),
                        skill.model.as_deref(),
                    );

                    self.status = format!("invoking skill /{}…", cmd);
                    self.push_log_no_agent(
                        LogLevel::Info,
                        format!("Invoking skill /{} with args: {}", cmd, args),
                    );

                    // Show the skill invocation as a user message in the chat
                    let display_text = if args.is_empty() {
                        format!("/{}", cmd)
                    } else {
                        format!("/{} {}", cmd, args)
                    };
                    let user_msg = Message::user_text(&sid, &display_text);
                    self.messages.push(user_msg);
                    self.add_to_history(display_text);

                    let flag = Arc::new(AtomicBool::new(false));
                    self.cancel_flag = Some(flag.clone());
                    let working_dir = std::env::current_dir().unwrap_or_default();

                    tokio::spawn(async move {
                        match ragent_agent::skill::invoke::invoke_skill(
                            &skill,
                            &args_owned,
                            &sid,
                            &working_dir,
                        )
                        .await
                        {
                            Ok(invocation) => {
                                if invocation.forked {
                                    // Execute in an isolated sub-session
                                    match ragent_agent::skill::invoke::invoke_forked_skill(
                                        &invocation,
                                        &processor,
                                        &sid,
                                        &working_dir,
                                        flag,
                                        agent.model.clone(),
                                    )
                                    .await
                                    {
                                        Ok(result) => {
                                            tracing::info!(
                                                skill = %result.skill_name,
                                                forked_session = %result.forked_session_id,
                                                "Forked skill completed"
                                            );
                                            // The forked result is already displayed via events;
                                            // no additional process_message call needed.
                                        }
                                        Err(e) => {
                                            tracing::debug!(
                                                error = %e,
                                                "Failed to execute forked skill"
                                            );
                                        }
                                    }
                                } else {
                                    let message = ragent_agent::skill::invoke::format_skill_message(
                                        &invocation,
                                    );
                                    if let Err(e) = processor
                                        .process_message(&sid, &message, &agent, flag)
                                        .await
                                    {
                                        tracing::debug!(
                                            error = %e,
                                            "Failed to process skill message"
                                        );
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::debug!(error = %e, "Failed to invoke skill");
                            }
                        }
                    });
                } else {
                    self.status = format!("Unknown command: /{}", cmd);
                    self.push_log_no_agent(LogLevel::Warn, format!("Unknown command: /{}", cmd));
                }
            }
        }
        self.assert_ui_invariants();
    }
}
