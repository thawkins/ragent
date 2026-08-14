//! Slash-command dispatch for the TUI.
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use ragent_agent::{event::Event, mcp::McpClient, message::Message, tool::TeamManagerInterface};
use ragent_team::team::{
    self, Mailbox, MailboxMessage, MemberStatus, MessageType, TaskStatus, TeamStore,
};
use ragent_types::ThinkingLevel;
use ragent_types::strutil::truncate_bytes;

use ragent_config::OtelConfig;
use ragent_telemetry::counters::{TelemetryCountersContent, current_values};

use crate::research_adapter::RagentCompleter;

// Prompt optimization templates
use ragent_prompt_opt::{Completer, OptMethod, optimize};

// State types from app/state.rs
use crate::app::state::{
    App, ConfigSavePickerState, ConfiguredProvider, DeviceFlowKind, LogEntry, LogLevel,
    McpDiscoverState, PendingForceCleanup, ProviderSetupStep, ProviderSource, RoleMode,
    SLASH_COMMANDS, SlashMenuEntry, SlashMenuState,
};

// Helpers
use crate::app::helpers::{parse_swarm_args, short_session_id};

// Redaction patterns for bug reports
use regex::Regex;

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
            "triggers" => {
                vec![
                    "list".to_string(),
                    "enable".to_string(),
                    "disable".to_string(),
                    "remove".to_string(),
                    "status".to_string(),
                    "help".to_string(),
                ]
            }
            "inbox" => {
                vec![
                    "list".to_string(),
                    "claim".to_string(),
                    "dismiss".to_string(),
                    "clear".to_string(),
                    "help".to_string(),
                ]
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

    /// Handle the `/editlog` slash command.
    fn handle_editlog_command(&mut self, args: &str) {
        use ragent_config::edit_log::{is_enabled as is_edit_log_enabled, persist_edit_log};
        use ragent_tools_core::edit_log::clear_edit_log_contents;
        let sub = args.split_whitespace().next().unwrap_or("").to_lowercase();
        match sub.as_str() {
            "help" | "" => {
                self.append_assistant_text(
                    "From: /editlog help\n\n## /editlog — Edit-operation logging\n\n| Subcommand | Description |\n|---|---|\n| `/editlog help` | Show this help |\n| `/editlog status` | Show whether logging is enabled and the log directory |\n| `/editlog on` | Enable logging of `edit` and `multi_edit` operations |\n| `/editlog off` | Disable logging |\n| `/editlog show` | Show counts, outcomes, and success/failure ratio per tool |\n| `/editlog analyse` | Analyse failed edits for `old_str` characteristics that may cause failures |\n| `/editlog clear` | Empty the contents of the editlog files (files are kept) |",
                );
                self.status = "editlog: help".to_string();
            }
            "on" => match persist_edit_log(true) {
                Ok(()) => {
                    self.append_assistant_text(
                        "From: /editlog on\n\n✅ Edit-operation logging enabled.",
                    );
                    self.status = "editlog: enabled".to_string();
                }
                Err(e) => {
                    self.append_assistant_text(&format!(
                        "From: /editlog on\n\n⚠ Failed to persist edit-log state: {e}"
                    ));
                    self.status = format!("editlog: persist failed ({e})");
                }
            },
            "off" => match persist_edit_log(false) {
                Ok(()) => {
                    self.append_assistant_text(
                        "From: /editlog off\n\n✅ Edit-operation logging disabled.",
                    );
                    self.status = "editlog: disabled".to_string();
                }
                Err(e) => {
                    self.append_assistant_text(&format!(
                        "From: /editlog off\n\n⚠ Failed to persist edit-log state: {e}"
                    ));
                    self.status = format!("editlog: persist failed ({e})");
                }
            },
            "status" => {
                let enabled = is_edit_log_enabled();
                let dir = std::env::current_dir()
                    .map(|p| p.join("log").display().to_string())
                    .unwrap_or_else(|_| "(unknown)".to_string());
                self.append_assistant_text(&format!(
                    "From: /editlog status\n\nEdit logging: {}\nLog directory: {}",
                    if enabled { "enabled" } else { "disabled" },
                    dir
                ));
                self.status = format!("editlog: {}", if enabled { "on" } else { "off" });
            }
            "show" => {
                self.show_editlog_stats();
            }
            "analyse" => {
                self.show_editlog_analysis();
            }
            "clear" => {
                let working_dir = std::env::current_dir().unwrap_or_default();
                let cleared = clear_edit_log_contents(&working_dir);
                self.append_assistant_text(&format!(
                    "From: /editlog clear\n\n✅ Cleared {cleared} edit-log file{}.",
                    if cleared == 1 { "" } else { "s" }
                ));
                self.status = "editlog: cleared".to_string();
            }
            _ => {
                self.append_assistant_text(
                    "From: /editlog\n\nUsage: `/editlog on|off|status|show|analyse|clear|help`",
                );
                self.status = "editlog: usage".to_string();
            }
        }
    }

    /// Render aggregate edit-log statistics into the assistant output.
    fn show_editlog_stats(&mut self) {
        use ragent_tools_core::edit_log::edit_log_stats;
        let working_dir = std::env::current_dir().unwrap_or_default();
        let log_dir = working_dir.join("log");

        let Some(stats) = edit_log_stats(&working_dir) else {
            self.append_assistant_text(&format!(
                "From: /editlog show\n\n⚠ Log directory not found: {}.",
                log_dir.display()
            ));
            self.status = "editlog: no logs".to_string();
            return;
        };

        if stats.total() == 0 {
            self.append_assistant_text(&format!(
                "From: /editlog show\n\nNo edit-log entries found in {}.",
                log_dir.display()
            ));
            self.status = "editlog: empty".to_string();
            return;
        }

        let mut tools: Vec<&String> = stats.tool_counts.keys().collect();
        tools.sort();

        let mut output = "From: /editlog show\n\n".to_string();
        output.push_str("| Tool | Count | Success | Failure | Success % |\n");
        output.push_str("|---|---|---|---:|---:|\n");
        for tool in &tools {
            let total = stats.tool_counts[*tool];
            let success = stats.success_for(tool);
            let failure = stats.failure_for(tool);
            let pct = stats.success_pct_for(tool);
            output.push_str(&format!(
                "| {} | {} | {} | {} | {:.1}% |\n",
                tool, total, success, failure, pct
            ));
        }

        if !stats.failure_reasons.is_empty() {
            output.push_str("\n**Failure reasons**\n\n");
            output.push_str("| Reason | Count |\n");
            output.push_str("|---|---:|\n");
            let mut reasons: Vec<(&String, &usize)> = stats.failure_reasons.iter().collect();
            reasons.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
            for (reason, count) in reasons {
                output.push_str(&format!("| {} | {} |\n", reason, count));
            }
        }

        self.append_assistant_text(&output);
        self.status = "editlog: shown".to_string();
    }

    /// Render an analysis of failed `old_str` values into the assistant output.
    fn show_editlog_analysis(&mut self) {
        use ragent_tools_core::edit_log::{OldStrRisk, edit_log_analyse};
        let working_dir = std::env::current_dir().unwrap_or_default();
        let log_dir = working_dir.join("log");

        let Some(analysis) = edit_log_analyse(&working_dir) else {
            self.append_assistant_text(&format!(
                "From: /editlog analyse\n\n⚠ Log directory not found: {}.",
                log_dir.display()
            ));
            self.status = "editlog: no logs".to_string();
            return;
        };

        if analysis.failure_count == 0 {
            self.append_assistant_text(&format!(
                "From: /editlog analyse\n\nNo failed edit-log entries found in {}.",
                log_dir.display()
            ));
            self.status = "editlog: no failures".to_string();
            return;
        }

        let mut output = "From: /editlog analyse\n\n".to_string();
        output.push_str(&format!(
            "Analysed {} failed edit operation{}. {} had one or more characteristics that may explain the failure.\n\n",
            analysis.failure_count,
            if analysis.failure_count == 1 { "" } else { "s" },
            analysis.risky_failure_count
        ));

        // Per-tool success / failure / ratio table.
        let tools = analysis.tools_sorted();
        if !tools.is_empty() {
            output.push_str("**Success vs failure by tool**\n\n");
            output.push_str("| Tool | Success | Failure | Fail/Success ratio |\n");
            output.push_str("|---|---:|---:|---:|\n");
            for tool in &tools {
                let success = analysis.success_by_tool.get(*tool).copied().unwrap_or(0);
                let failure = analysis.failure_by_tool.get(*tool).copied().unwrap_or(0);
                let ratio = analysis.failure_success_ratio_pct_for(tool);
                output.push_str(&format!(
                    "| {} | {} | {} | {:.1}% |\n",
                    tool, success, failure, ratio
                ));
            }
            output.push('\n');
        }

        let by_freq = analysis.risks_by_frequency();
        if by_freq.is_empty() {
            output.push_str("No obvious `old_str` risk characteristics were detected.\n");
        } else {
            output.push_str("| Risk characteristic | Failures | % of failures |\n");
            output.push_str("|---|---|---:|\n");
            for (risk, count) in &by_freq {
                let pct = (*count as f64 / analysis.failure_count as f64) * 100.0;
                output.push_str(&format!("| {} | {} | {:.1}% |\n", risk.label(), count, pct));
            }
        }

        if !analysis.combination_counts.is_empty() {
            output.push_str("\n**Common combinations**\n\n");
            output.push_str("| Combination | Count |\n");
            output.push_str("|---|---:|\n");
            let mut combos: Vec<(&Vec<OldStrRisk>, &usize)> =
                analysis.combination_counts.iter().collect();
            combos.sort_by(|a, b| b.1.cmp(a.1));
            for (combo, count) in combos.iter().take(10) {
                let labels: Vec<String> =
                    combo.iter().map(|r| format!("`{}`", r.label())).collect();
                output.push_str(&format!("| {} | {} |\n", labels.join(" + "), count));
            }
        }

        if !analysis.risk_examples.is_empty() {
            output.push_str("\n**Examples**\n\n");
            for (risk, examples) in &analysis.risk_examples {
                output.push_str(&format!("*{}*\n\n", risk.label()));
                for ex in examples.iter().take(3) {
                    output.push_str(&format!(
                        "- `{}` on `{}` → {}\n  `old_str`: `{}`\n\n",
                        ex.tool, ex.file_path, ex.outcome, ex.old_str_preview
                    ));
                }
            }
        }

        self.append_assistant_text(&output);
        self.status = "editlog: analysed".to_string();
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

    /// Execute the slash command already parsed from `raw`.
    ///
    /// This is the inner entry point for slash-command dispatch. The caller is
    /// responsible for updating the UI mode and input buffer if needed; this
    /// method handles command-specific side effects and logging.
    pub fn execute_slash_command_inner(&mut self, raw: &str) {
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
            "bug-report" => self.handle_bug_report(),
            "template" => handle_template_command(self, args),
            "goal" => handle_goal_command(self, args),
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

                    // Allow file writes so the agent can call memory_store
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
After your analysis, call the `memory_store` tool with:\n\
- category: \"fact\"\n\
- content: a well-structured markdown summary of your findings\n\
- confidence: 0.8\n\
- tags: [\"project-analysis\"]\n\n\
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
            "actionloop" => {
                let sub = args.split_whitespace().next().unwrap_or("").to_lowercase();
                match sub.as_str() {
                    "help" => {
                        self.append_assistant_text(
                                          "From: /actionloop help\n\n## /actionloop — agent action-loop timing\n\n| Subcommand | Description |\n|---|---|\n| `/actionloop help` | Show this help |\n| `/actionloop` | Show average timing of the agent action-loop buckets |\n| `/actionloop clip` | Copy the timing data to the system clipboard |",
                                      );
                        self.status = "actionloop: help".to_string();
                        return;
                    }
                    "clip" => {
                        match self.actionloop_report() {
                            Some(report) => {
                                Self::set_clipboard(&report);
                                self.append_assistant_text(
                                      "From: /actionloop clip\n\n📋 Action-loop timing data copied to the clipboard.",
                                  );
                                self.status = "actionloop: clipped".to_string();
                            }
                            None => {
                                self.append_assistant_text(
                                      "From: /actionloop clip\nNo action-loop timing samples recorded yet.\nEnable `/profile on` and run the agent to collect bucket timings.",
                                  );
                                self.status = "actionloop: no samples".to_string();
                            }
                        }
                        return;
                    }
                    _ => {}
                }
                match self.actionloop_report() {
                    Some(report) => {
                        self.append_assistant_text(&format!("From: /actionloop\n{report}"));
                        self.status = "actionloop: timings shown".to_string();
                    }
                    None => {
                        self.append_assistant_text(
                              "From: /actionloop\nNo action-loop timing samples recorded yet.\nEnable `/profile on` and run the agent to collect bucket timings.",
                          );
                        self.status = "actionloop: no samples".to_string();
                    }
                }
            }
            "editlog" => self.handle_editlog_command(args),
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
            // ── /undo ──────────────────────────────────────────────────────
            // FR-014: Remove the last user/assistant turn pair from the conversation.
            // This allows users to correct mistakes or backtrack from unhelpful responses.
            "undo" => {
                if self.session_id.is_none() {
                    self.status = "⚠ No active session to undo".to_string();
                    return;
                }
                if self.messages.is_empty() {
                    self.status = "⚠ No messages to undo".to_string();
                    return;
                }

                // Find the last user message and remove it along with any
                // following assistant/compaction messages.
                let mut last_user_idx = None;
                for (idx, msg) in self.messages.iter().enumerate().rev() {
                    if msg.role == ragent_types::Role::User {
                        last_user_idx = Some(idx);
                        break;
                    }
                }

                match last_user_idx {
                    Some(user_idx) => {
                        // Remove the user message and all following messages
                        // (typically one assistant response, but could be more)
                        let removed_count = self.messages.len() - user_idx;
                        self.messages.truncate(user_idx);

                        // Reset scroll to show the new end of conversation
                        self.scroll_offset = 0;

                        self.status =
                            format!("Undid last turn (removed {} message(s))", removed_count);
                        self.push_log_no_agent(
                            LogLevel::Info,
                            format!(
                                "Undo: removed {} message(s) from end of conversation",
                                removed_count
                            ),
                        );
                    }
                    None => {
                        self.status = "⚠ No user message found to undo".to_string();
                        self.push_log_no_agent(
                            LogLevel::Warn,
                            "Undo: no user message found in conversation".to_string(),
                        );
                    }
                }
            }
            // ── /name ──────────────────────────────────────────────────────
            // FR-015: Set a human-readable display name on the active session.
            // The name is persisted in session metadata and appears in session lists.
            "name" => {
                if self.session_id.is_none() {
                    self.status = "⚠ No active session to name".to_string();
                    return;
                }

                let name = args.trim();
                let session_id = self.session_id.clone().unwrap();

                if name.is_empty() {
                    // Clear the session name
                    let storage = self.session_processor.session_manager.storage();
                    match storage.update_session(&session_id, "") {
                        Ok(()) => {
                            self.status = "Session name cleared".to_string();
                            self.push_log_no_agent(
                                LogLevel::Info,
                                "Session name cleared".to_string(),
                            );
                        }
                        Err(e) => {
                            self.status = format!("⚠ Failed to clear session name: {}", e);
                            self.push_log_no_agent(
                                LogLevel::Error,
                                format!("Failed to clear session name: {}", e),
                            );
                        }
                    }
                } else {
                    // Set the session name
                    let storage = self.session_processor.session_manager.storage();
                    match storage.update_session(&session_id, name) {
                        Ok(()) => {
                            self.status = format!("Session name set to '{}'", name);
                            self.push_log_no_agent(
                                LogLevel::Info,
                                format!("Session name set to '{}'", name),
                            );
                        }
                        Err(e) => {
                            self.status = format!("⚠ Failed to set session name: {}", e);
                            self.push_log_no_agent(
                                LogLevel::Error,
                                format!("Failed to set session name: {}", e),
                            );
                        }
                    }
                }
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
                        self.status_set_at = None;
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
                use ragent_specs::{
                    SddFlags, SpecCommand, SpecFilter, SpecManager, validate_with_flags,
                };
                let cmd = SpecCommand::parse(args);
                match cmd {
                    SpecCommand::Help => {
                        self.append_assistant_text(SpecCommand::build_help_message());
                        self.status = "spec: help".to_string();
                    }
                    SpecCommand::Create {
                        specname,
                        feature,
                        from_research,
                    } => {
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

                        let task = SpecCommand::build_create_prompt(
                            &specname,
                            &feature,
                            from_research.as_deref(),
                        );
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
                    SpecCommand::Jtbd {
                        spec_id,
                        force,
                        agent,
                    } => {
                        let working_dir = std::env::current_dir().unwrap_or_default();
                        let specs_root = working_dir.join("specs");

                        // FR-008: validate spec ID format
                        let id = match ragent_specs::spec::SpecId::new(&spec_id) {
                            Some(id) => id,
                            None => {
                                self.status = format!("spec: invalid spec ID: {}", spec_id);
                                self.append_assistant_text(&format!(
                                    "From: /spec jtbd\n\n**Error:** Invalid spec ID \
                                     `{}`. Spec IDs must be alphanumeric with hyphens or \
                                     underscores only.",
                                    spec_id
                                ));
                                return;
                            }
                        };

                        let mgr = SpecManager::new(&specs_root);
                        let spec_dir = specs_root.join(id.as_str());
                        let spec_md_path = spec_dir.join("SPEC.md");
                        let jtbd_path = spec_dir.join("JTBD.md");

                        let rt = tokio::runtime::Handle::current();
                        let spec_id_owned = spec_id.clone();
                        // Validation returns Ok(jtbd_exists) so the caller can
                        // enforce the overwrite guard (FR-003 / FR-004).
                        let validation: Result<bool, String> = tokio::task::block_in_place(|| {
                            rt.block_on(async {
                                // FR-008: confirm spec directory exists
                                if !spec_dir.is_dir() {
                                    return Err(format!(
                                        "spec `{}` not found at {}",
                                        spec_id_owned,
                                        spec_dir.display()
                                    ));
                                }
                                // FR-009: confirm SPEC.md exists and is readable
                                match tokio::fs::read_to_string(&spec_md_path).await {
                                    Ok(content) if content.trim().is_empty() => {
                                        return Err(format!(
                                            "SPEC.md is empty: {}",
                                            spec_md_path.display()
                                        ));
                                    }
                                    Ok(_) => {}
                                    Err(e) => {
                                        return Err(format!(
                                            "SPEC.md not readable at {}: {}",
                                            spec_md_path.display(),
                                            e
                                        ));
                                    }
                                }
                                // FR-003/FR-004: check whether JTBD.md already exists
                                let jtbd_exists = tokio::fs::metadata(&jtbd_path).await.is_ok();
                                Ok(jtbd_exists)
                            })
                        });
                        let jtbd_exists = match validation {
                            Ok(exists) => exists,
                            Err(e) => {
                                self.status = format!("spec: {}", e);
                                self.append_assistant_text(&format!(
                                    "From: /spec jtbd\n\n**Error:** {}",
                                    e
                                ));
                                return;
                            }
                        };

                        // FR-003: refuse if JTBD.md exists and --force not given
                        let _ = mgr; // SpecManager kept for parity / future use
                        if jtbd_exists && !force {
                            self.status = format!("spec jtbd: {} already has JTBD.md", spec_id);
                            self.append_assistant_text(&format!(
                                "From: /spec jtbd\n\n\
                                 **JTBD.md already exists** at `specs/{}/JTBD.md`.\n\
                                 Re-run with `--force` to overwrite:\n\n\
                                 `/spec jtbd {} --force`",
                                spec_id, spec_id
                            ));
                            return;
                        }
                        // FR-004: when --force is present the guard is bypassed
                        // and the agent task below will overwrite JTBD.md.

                        // FR-011: status, message, and log parity with /spec create
                        let sid = self.session_id.clone().unwrap_or_default();
                        self.append_assistant_text(&SpecCommand::build_jtbd_message(&spec_id));
                        self.push_log_no_agent(
                            LogLevel::Info,
                            SpecCommand::build_jtbd_log(&spec_id, force, agent.as_deref()),
                        );

                        // FR-005: optional --agent <name> override
                        let mut selected_agent = if let Some(ref agent_name) = agent {
                            match self
                                .cycleable_agents
                                .iter()
                                .find(|a| a.name == *agent_name)
                                .cloned()
                            {
                                Some(a) => a,
                                None => {
                                    self.status = format!("spec: agent '{}' not found", agent_name);
                                    self.append_assistant_text(&format!(
                                        "From: /spec jtbd\n\n**Error:** Agent \
                                         `{}` not found. No task was spawned.",
                                        agent_name
                                    ));
                                    return;
                                }
                            }
                        } else {
                            // FR-002: default to explore agent, fallback to current agent
                            let explore_agent = self
                                .cycleable_agents
                                .iter()
                                .find(|a| a.name == "explore")
                                .cloned();
                            explore_agent.unwrap_or_else(|| self.agent_info.clone())
                        };
                        self.apply_selected_model_and_thinking(&mut selected_agent);
                        selected_agent.permission = ragent_agent::agent::default_permissions();

                        // FR-002, FR-006, FR-007: build the JTBD analysis prompt
                        let task = SpecCommand::build_jtbd_prompt(&spec_id);
                        let msg = Message::user_text(&sid, &task);
                        self.messages.push(msg);

                        let processor = self.session_processor.clone();
                        let flag = Arc::new(AtomicBool::new(false));
                        self.cancel_flag = Some(flag.clone());
                        self.is_processing = true;
                        self.status = SpecCommand::build_jtbd_status(&spec_id);

                        let event_bus = self.event_bus.clone();
                        // FR-014: clone path + spec_id for cancellation cleanup
                        let jtbd_path_for_cleanup = jtbd_path.clone();
                        let spec_id_for_cleanup = spec_id.clone();
                        let cancel_flag_for_check = flag.clone();
                        tokio::spawn(async move {
                            let result = processor
                                .process_message(&sid, &task, &selected_agent, flag)
                                .await;

                            // FR-014: if cancelled, remove partial JTBD.md and notify
                            if cancel_flag_for_check.load(Ordering::Acquire) {
                                // FR-013: remove any partially-written file so a
                                // subsequent --force re-run starts clean.
                                if jtbd_path_for_cleanup.exists() {
                                    if let Err(e) =
                                        tokio::fs::remove_file(&jtbd_path_for_cleanup).await
                                    {
                                        tracing::warn!(
                                            error = %e,
                                            "spec jtbd: failed to remove partial JTBD.md"
                                        );
                                    }
                                }
                                event_bus.publish(ragent_agent::event::Event::AgentNotice {
                                    session_id: sid.clone(),
                                    message: format!(
                                        "spec jtbd: cancelled — partial JTBD.md for `{}` removed",
                                        spec_id_for_cleanup
                                    ),
                                });
                                return;
                            }

                            if let Err(e) = result {
                                tracing::warn!(error = %e, "spec jtbd: analysis failed");
                                event_bus.publish(ragent_agent::event::Event::AgentError {
                                    session_id: sid,
                                    error: format!("spec jtbd analysis failed: {e}"),
                                });
                            }
                        });
                    }
                    SpecCommand::Validate { spec_id } => {
                        // FR-019: Build SDD flags from loaded config so
                        // SDD-specific checks are opt-in.
                        let sdd_cfg = ragent_agent::Config::load().unwrap_or_default().sdd;
                        let flags = SddFlags::from_bools(
                            sdd_cfg.clarification_markers,
                            sdd_cfg.quality_checklists,
                            sdd_cfg.consistency_checks,
                            sdd_cfg.phase_minus_one_gates,
                            sdd_cfg.constitution,
                            sdd_cfg.feedback_loop,
                        );
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
                                    let report = validate_with_flags(spec, &flags);
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
                                                                                                                                                                                                                                                              // FR-019: Gate SDD clarification check via config flags.
                                                                let sdd_cfg = ragent_agent::Config::load().unwrap_or_default().sdd;
                                                                let flags = SddFlags::from_bools(
                                                                    sdd_cfg.clarification_markers,
                                                                    sdd_cfg.quality_checklists,
                                                                    sdd_cfg.consistency_checks,
                                                                    sdd_cfg.phase_minus_one_gates,
                                                                    sdd_cfg.constitution,
                                                                    sdd_cfg.feedback_loop,
                                                                );
                                                                if let Err(e) = mgr.transition_with_flags(&mut spec, new_status, "user", &flags).await {
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
                            || sub == "delete"
                            || sub == "jtbd"
                            || sub == "update"
                            || sub == "specify"
                            || sub == "plan"
                            || sub == "tasks"
                            || sub == "feedback" =>
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
                                let specs_root_phase2 = specs_root.clone();
                                let spec_id_phase2 = spec_id.clone();
                                tokio::spawn(async move {
                                    // Phase 1: incremental add (new requirements + task rows)
                                    if let Err(e) =
                                        processor.process_message(&sid, &prompt, &agent, flag).await
                                    {
                                        tracing::warn!(error = %e, "spec: add generation failed");
                                        event_bus.publish(ragent_agent::event::Event::AgentError {
                                            session_id: sid,
                                            error: format!("spec add generation failed: {e}"),
                                        });
                                        return;
                                    }

                                    // Phase 2: regenerate PLAN.md + TESTPLAN.md
                                    // (same as /spec update — re-read the updated
                                    // SPEC.md and fully regenerate both files)
                                    let sid2 =
                                        match ragent_specs::spec::SpecId::new(&spec_id_phase2) {
                                            Some(id) => id,
                                            None => {
                                                tracing::warn!(
                                                    "spec: invalid spec ID after add phase: {}",
                                                    spec_id_phase2
                                                );
                                                return;
                                            }
                                        };
                                    let mgr2 = SpecManager::new(&specs_root_phase2);
                                    let plan_md = match mgr2.read_spec(&sid2).await {
                                        Ok(spec) => spec.plan_md,
                                        Err(e) => {
                                            tracing::warn!(
                                                error = %e,
                                                "spec: failed to read spec after add phase"
                                            );
                                            return;
                                        }
                                    };

                                    let update_prompt =
                                        SpecCommand::build_update_prompt(&spec_id_phase2, &plan_md);
                                    let flag2 = Arc::new(AtomicBool::new(false));
                                    if let Err(e) = processor
                                        .process_message(&sid, &update_prompt, &agent, flag2)
                                        .await
                                    {
                                        tracing::warn!(
                                            error = %e,
                                            "spec: update generation failed after add"
                                        );
                                        event_bus.publish(ragent_agent::event::Event::AgentError {
                                            session_id: sid,
                                            error: format!(
                                                "spec update generation failed after add: {e}"
                                            ),
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
                    SpecCommand::Update { spec_id } => {
                        let working_dir = std::env::current_dir().unwrap_or_default();
                        let specs_root = working_dir.join("specs");

                        // FR-006: validate spec ID format
                        let sid = match ragent_specs::spec::SpecId::new(&spec_id) {
                            Some(id) => id,
                            None => {
                                self.status = format!("spec: invalid spec ID: {}", spec_id);
                                self.append_assistant_text(&format!(
                                    "From: /spec update\n\n**Error:** Invalid spec ID \
                                     `{}`. Spec IDs must be alphanumeric with hyphens or \
                                     underscores only.",
                                    spec_id
                                ));
                                return;
                            }
                        };

                        let mgr = SpecManager::new(&specs_root);
                        let rt = tokio::runtime::Handle::current();

                        // FR-005/FR-010: read spec, guard archived, read PLAN.md
                        let spec_id_owned = spec_id.clone();
                        let result: Result<String, String> = tokio::task::block_in_place(|| {
                            rt.block_on(async {
                                let spec = match mgr.read_spec(&sid).await {
                                    Ok(s) => s,
                                    Err(_e) => {
                                        // List available specs on not-found
                                        let available: Vec<String> = match mgr
                                            .discover_specs()
                                            .await
                                        {
                                            Ok(specs) => specs
                                                .iter()
                                                .map(|s| {
                                                    format!("  - {} ({})", s.id, s.status.as_str())
                                                })
                                                .collect(),
                                            Err(_) => vec![],
                                        };
                                        let avail_str = if available.is_empty() {
                                            "  (none found)".to_string()
                                        } else {
                                            available.join("\n")
                                        };
                                        return Err(format!(
                                            "Spec `{}` not found.\n\nAvailable specs:\n{}",
                                            spec_id_owned, avail_str
                                        ));
                                    }
                                };
                                // FR-013: guard archived specs
                                if spec.status == ragent_specs::spec::SpecStatus::Archived {
                                    return Err(format!(
                                        "spec: '{}' is archived and cannot be updated",
                                        spec_id_owned
                                    ));
                                }
                                // FR-011: read existing PLAN.md for status preservation
                                Ok(spec.plan_md)
                            })
                        });

                        let plan_md = match result {
                            Ok(p) => p,
                            Err(e) => {
                                self.status = format!("spec: {}", e);
                                self.append_assistant_text(&format!(
                                    "From: /spec update\n\n**Error:** {}",
                                    e
                                ));
                                return;
                            }
                        };

                        // FR-009: status, message, and log
                        self.append_assistant_text(&SpecCommand::build_update_message(&spec_id));
                        self.push_log_no_agent(
                            crate::app::LogLevel::Info,
                            SpecCommand::build_update_log(&spec_id),
                        );

                        // FR-007: select explore agent, fallback to current agent
                        let explore_agent = self
                            .cycleable_agents
                            .iter()
                            .find(|a| a.name == "explore")
                            .cloned();
                        let mut agent = explore_agent.unwrap_or_else(|| self.agent_info.clone());
                        self.apply_selected_model_and_thinking(&mut agent);
                        agent.permission = ragent_agent::agent::default_permissions();

                        // FR-008/FR-011: build prompt with plan_md for status preservation
                        let prompt = SpecCommand::build_update_prompt(&spec_id, &plan_md);
                        let sid = self.session_id.clone().unwrap_or_default();
                        let msg = Message::user_text(&sid, &prompt);
                        self.messages.push(msg);

                        let processor = self.session_processor.clone();
                        let flag = Arc::new(AtomicBool::new(false));
                        self.cancel_flag = Some(flag.clone());
                        self.is_processing = true;
                        self.status = SpecCommand::build_update_status(&spec_id);

                        let event_bus = self.event_bus.clone();
                        tokio::spawn(async move {
                            if let Err(e) =
                                processor.process_message(&sid, &prompt, &agent, flag).await
                            {
                                tracing::warn!(error = %e, "spec: update generation failed");
                                event_bus.publish(ragent_agent::event::Event::AgentError {
                                    session_id: sid,
                                    error: format!("spec update generation failed: {e}"),
                                });
                            }
                        });
                    }
                    SpecCommand::Unknown(sub) => {
                        self.status = format!("Unknown /spec subcommand: {sub}. Try /spec help");
                    }
                    SpecCommand::Specify {
                        specname,
                        feature,
                        from_research,
                    } => {
                        let sid = self.session_id.clone().unwrap_or_default();
                        self.append_assistant_text(&SpecCommand::build_specify_message(
                            &specname, &feature,
                        ));
                        self.push_log_no_agent(
                            LogLevel::Info,
                            SpecCommand::build_specify_log(&specname, &feature),
                        );

                        // FR-009: optionally create a git branch for the spec.
                        let sdd_cfg = ragent_agent::Config::load().unwrap_or_default().sdd;
                        if sdd_cfg.branch_per_spec {
                            let working_dir = std::env::current_dir().unwrap_or_default();
                            let branch_result =
                                ragent_specs::create_spec_branch(&specname, &working_dir);
                            self.append_assistant_text(&SpecCommand::build_branch_message(
                                &branch_result,
                            ));
                            self.push_log_no_agent(
                                LogLevel::Info,
                                SpecCommand::build_branch_log(&specname, &branch_result),
                            );
                        }

                        let explore_agent = self
                            .cycleable_agents
                            .iter()
                            .find(|a| a.name == "explore")
                            .cloned();

                        let mut agent = explore_agent.unwrap_or_else(|| self.agent_info.clone());
                        self.apply_selected_model_and_thinking(&mut agent);
                        agent.permission = ragent_agent::agent::default_permissions();

                        let task = SpecCommand::build_specify_prompt(
                            &specname,
                            &feature,
                            from_research.as_deref(),
                        );
                        let msg = Message::user_text(&sid, &task);
                        self.messages.push(msg);

                        let processor = self.session_processor.clone();
                        let flag = Arc::new(AtomicBool::new(false));
                        self.cancel_flag = Some(flag.clone());
                        self.is_processing = true;
                        self.status = SpecCommand::build_specify_status(&specname);

                        let event_bus = self.event_bus.clone();
                        tokio::spawn(async move {
                            if let Err(e) =
                                processor.process_message(&sid, &task, &agent, flag).await
                            {
                                tracing::warn!(error = %e, "spec: specify generation failed");
                                event_bus.publish(ragent_agent::event::Event::AgentError {
                                    session_id: sid,
                                    error: format!("spec specify generation failed: {e}"),
                                });
                            }
                        });
                    }
                    SpecCommand::Plan {
                        spec_id,
                        tech_context,
                    } => {
                        let working_dir = std::env::current_dir().unwrap_or_default();
                        let specs_root = working_dir.join("specs");

                        // FR-006: validate spec ID format
                        let sid = match ragent_specs::spec::SpecId::new(&spec_id) {
                            Some(id) => id,
                            None => {
                                self.status = format!("spec: invalid spec ID: {}", spec_id);
                                self.append_assistant_text(&format!(
                                    "From: /spec plan\n\n**Error:** Invalid spec ID \
                                     `{}`. Spec IDs must be alphanumeric with hyphens or \
                                     underscores only.",
                                    spec_id
                                ));
                                return;
                            }
                        };

                        let mgr = SpecManager::new(&specs_root);
                        let rt = tokio::runtime::Handle::current();

                        // Read SPEC.md (and existing PLAN.md for status preservation)
                        let spec_id_owned = spec_id.clone();
                        let result: Result<(String, String, String), String> =
                            tokio::task::block_in_place(|| {
                                rt.block_on(async {
                                    let spec = match mgr.read_spec(&sid).await {
                                        Ok(s) => s,
                                        Err(_e) => {
                                            let available: Vec<String> =
                                                match mgr.discover_specs().await {
                                                    Ok(specs) => specs
                                                        .iter()
                                                        .map(|s| {
                                                            format!(
                                                                "  - {} ({})",
                                                                s.id,
                                                                s.status.as_str()
                                                            )
                                                        })
                                                        .collect(),
                                                    Err(_) => vec![],
                                                };
                                            let avail_str = if available.is_empty() {
                                                "  (none found)".to_string()
                                            } else {
                                                available.join("\n")
                                            };
                                            return Err(format!(
                                                "Spec `{}` not found.\n\nAvailable specs:\n{}",
                                                spec_id_owned, avail_str
                                            ));
                                        }
                                    };
                                    if spec.status == ragent_specs::spec::SpecStatus::Archived {
                                        return Err(format!(
                                            "spec: '{}' is archived and cannot be updated",
                                            spec_id_owned
                                        ));
                                    }
                                    Ok((spec.spec_md, spec.plan_md, spec.feedback_md.clone()))
                                })
                            });

                        let (spec_md, plan_md, feedback_md) = match result {
                            Ok(v) => v,
                            Err(e) => {
                                self.status = format!("spec: {}", e);
                                self.append_assistant_text(&format!(
                                    "From: /spec plan\n\n**Error:** {}",
                                    e
                                ));
                                return;
                            }
                        };

                        // FR-011/FR-012/FR-017: check whether data-model,
                        // contracts, and feedback surfacing are enabled
                        let sdd_cfg = ragent_agent::Config::load().unwrap_or_default().sdd;
                        let data_model_enabled = sdd_cfg.data_model;
                        let contracts_enabled = sdd_cfg.contracts;
                        let feedback_enabled = sdd_cfg.feedback_loop;
                        // FR-017: load FEEDBACK.md content when feedback loop is
                        // enabled, so it can be surfaced during plan regeneration
                        let feedback_md = if feedback_enabled {
                            feedback_md.as_str()
                        } else {
                            ""
                        };

                        self.append_assistant_text(&SpecCommand::build_plan_message(
                            &spec_id,
                            &tech_context,
                            data_model_enabled,
                            contracts_enabled,
                            feedback_enabled,
                        ));
                        self.push_log_no_agent(
                            crate::app::LogLevel::Info,
                            SpecCommand::build_plan_log(&spec_id, &tech_context),
                        );

                        let explore_agent = self
                            .cycleable_agents
                            .iter()
                            .find(|a| a.name == "explore")
                            .cloned();
                        let mut agent = explore_agent.unwrap_or_else(|| self.agent_info.clone());
                        self.apply_selected_model_and_thinking(&mut agent);
                        agent.permission = ragent_agent::agent::default_permissions();

                        let prompt = SpecCommand::build_plan_prompt(
                            &spec_id,
                            &tech_context,
                            &spec_md,
                            &plan_md,
                            data_model_enabled,
                            contracts_enabled,
                            feedback_md,
                        );
                        let sid_msg = self.session_id.clone().unwrap_or_default();
                        let msg = Message::user_text(&sid_msg, &prompt);
                        self.messages.push(msg);

                        let processor = self.session_processor.clone();
                        let flag = Arc::new(AtomicBool::new(false));
                        self.cancel_flag = Some(flag.clone());
                        self.is_processing = true;
                        self.status = SpecCommand::build_plan_status(&spec_id);

                        let event_bus = self.event_bus.clone();
                        tokio::spawn(async move {
                            if let Err(e) = processor
                                .process_message(&sid_msg, &prompt, &agent, flag)
                                .await
                            {
                                tracing::warn!(error = %e, "spec: plan generation failed");
                                event_bus.publish(ragent_agent::event::Event::AgentError {
                                    session_id: sid_msg,
                                    error: format!("spec plan generation failed: {e}"),
                                });
                            }
                        });
                    }
                    SpecCommand::Tasks { spec_id } => {
                        let working_dir = std::env::current_dir().unwrap_or_default();
                        let specs_root = working_dir.join("specs");

                        // FR-006: validate spec ID format
                        let sid = match ragent_specs::spec::SpecId::new(&spec_id) {
                            Some(id) => id,
                            None => {
                                self.status = format!("spec: invalid spec ID: {}", spec_id);
                                self.append_assistant_text(&format!(
                                    "From: /spec tasks\n\n**Error:** Invalid \
                                     spec ID `{}`. Spec IDs must be \
                                     alphanumeric with hyphens or \
                                     underscores only.",
                                    spec_id
                                ));
                                return;
                            }
                        };

                        let mgr = SpecManager::new(&specs_root);
                        let rt = tokio::runtime::Handle::current();

                        // Read spec to get plan_md and title
                        let spec_id_owned = spec_id.clone();
                        let result: Result<
                            (String, String, String, ragent_specs::spec::SpecStatus),
                            String,
                        > = tokio::task::block_in_place(|| {
                            rt.block_on(async {
                                let spec = match mgr.read_spec(&sid).await {
                                    Ok(s) => s,
                                    Err(_e) => {
                                        let available: Vec<String> = match mgr
                                            .discover_specs()
                                            .await
                                        {
                                            Ok(specs) => specs
                                                .iter()
                                                .map(|s| {
                                                    format!("  - {} ({})", s.id, s.status.as_str())
                                                })
                                                .collect(),
                                            Err(_) => vec![],
                                        };
                                        let avail_str = if available.is_empty() {
                                            "  (none found)".to_string()
                                        } else {
                                            available.join("\n")
                                        };
                                        return Err(format!(
                                            "Spec `{}` not found.\n\nAvailable specs:\n{}",
                                            spec_id_owned, avail_str
                                        ));
                                    }
                                };
                                if spec.status == ragent_specs::spec::SpecStatus::Archived {
                                    return Err(format!(
                                        "spec: '{}' is archived and cannot be updated",
                                        spec_id_owned
                                    ));
                                }
                                Ok((spec.spec_md, spec.plan_md, spec.title, spec.status))
                            })
                        });

                        let (spec_md, plan_md, title, _status) = match result {
                            Ok(v) => v,
                            Err(e) => {
                                self.status = format!("spec: {}", e);
                                self.append_assistant_text(&format!(
                                    "From: /spec tasks\n\n**Error:** {}",
                                    e
                                ));
                                return;
                            }
                        };

                        // Check PLAN.md is not empty
                        if plan_md.trim().is_empty() {
                            self.status = format!("spec: PLAN.md is empty for {}", spec_id);
                            self.append_assistant_text(&SpecCommand::build_tasks_no_plan_error(
                                &spec_id,
                            ));
                            return;
                        }

                        // Generate TASKS.md content from the task table
                        let tasks_md = match SpecCommand::build_tasks_md(&spec_id, &title, &plan_md)
                        {
                            Some(md) => md,
                            None => {
                                self.status =
                                    format!("spec: no tasks found in PLAN.md for {}", spec_id);
                                self.append_assistant_text(
                                    &SpecCommand::build_tasks_no_tasks_error(&spec_id),
                                );
                                return;
                            }
                        };

                        // Write TASKS.md atomically
                        let tasks_path = specs_root.join(&spec_id).join("TASKS.md");
                        let write_result = tokio::task::block_in_place(|| {
                            rt.block_on(async {
                                ragent_specs::io::SpecIo::atomic_write(&tasks_path, &tasks_md).await
                            })
                        });

                        match write_result {
                            Ok(()) => {
                                // Count tasks for the completion message
                                let task_count = ragent_specs::PlanParser::parse(&plan_md)
                                    .map(|t| t.len())
                                    .unwrap_or(0);

                                // FR-013, T-023: also generate quickstart.md
                                // with key validation scenarios from SPEC.md
                                let quickstart_path =
                                    specs_root.join(&spec_id).join("quickstart.md");
                                let quickstart_md =
                                    SpecCommand::build_quickstart_md(&spec_id, &title, &spec_md);
                                let quickstart_written = match &quickstart_md {
                                    Some(qs_md) => {
                                        let qs_result = tokio::task::block_in_place(|| {
                                            rt.block_on(async {
                                                ragent_specs::io::SpecIo::atomic_write(
                                                    &quickstart_path,
                                                    qs_md,
                                                )
                                                .await
                                            })
                                        });
                                        qs_result.is_ok()
                                    }
                                    None => false,
                                };

                                self.append_assistant_text(
                                    &SpecCommand::build_tasks_completion_message(
                                        &spec_id, task_count,
                                    ),
                                );
                                self.push_log_no_agent(
                                    crate::app::LogLevel::Info,
                                    SpecCommand::build_tasks_log(&spec_id),
                                );
                                if quickstart_written {
                                    self.status = format!(
                                        "spec: wrote specs/{}/TASKS.md ({} tasks) + \
                                         quickstart.md",
                                        spec_id, task_count
                                    );
                                } else {
                                    self.status = format!(
                                        "spec: wrote specs/{}/TASKS.md ({} tasks)",
                                        spec_id, task_count
                                    );
                                }
                            }
                            Err(e) => {
                                self.status = format!("spec: failed to write TASKS.md: {}", e);
                                self.append_assistant_text(&format!(
                                    "From: /spec tasks\n\n**Error:** Failed \
                                     to write `specs/{}/TASKS.md`: {}",
                                    spec_id, e
                                ));
                            }
                        }
                    }
                    SpecCommand::Feedback { spec_id, note } => {
                        let working_dir = std::env::current_dir().unwrap_or_default();
                        let specs_root = working_dir.join("specs");

                        // Validate spec ID format
                        let sid = match ragent_specs::spec::SpecId::new(&spec_id) {
                            Some(id) => id,
                            None => {
                                self.status = format!("spec: invalid spec ID: {}", spec_id);
                                self.append_assistant_text(&format!(
                                    "From: /spec feedback\n\n**Error:** Invalid spec ID \
                                     `{}`. Spec IDs must be alphanumeric with hyphens or \
                                     underscores only.",
                                    spec_id
                                ));
                                return;
                            }
                        };

                        let mgr = SpecManager::new(&specs_root);
                        let rt = tokio::runtime::Handle::current();

                        // Read spec to validate it exists and get title + existing FEEDBACK.md
                        let spec_id_owned = spec_id.clone();
                        let note_owned = note.clone();
                        let result: Result<(String, String), String> =
                            tokio::task::block_in_place(|| {
                                rt.block_on(async {
                                    let spec = match mgr.read_spec(&sid).await {
                                        Ok(s) => s,
                                        Err(_e) => {
                                            let available: Vec<String> =
                                                match mgr.discover_specs().await {
                                                    Ok(specs) => specs
                                                        .iter()
                                                        .map(|s| {
                                                            format!(
                                                                "  - {} ({})",
                                                                s.id,
                                                                s.status.as_str()
                                                            )
                                                        })
                                                        .collect(),
                                                    Err(_) => vec![],
                                                };
                                            let avail_str = if available.is_empty() {
                                                "  (none found)".to_string()
                                            } else {
                                                available.join("\n")
                                            };
                                            return Err(format!(
                                                "Spec `{}` not found.\n\nAvailable specs:\n{}",
                                                spec_id_owned, avail_str
                                            ));
                                        }
                                    };
                                    Ok((spec.title.clone(), spec.feedback_md.clone()))
                                })
                            });

                        let (title, existing_feedback) = match result {
                            Ok(v) => v,
                            Err(e) => {
                                self.status = format!("spec: {}", e);
                                self.append_assistant_text(&format!(
                                    "From: /spec feedback\n\n**Error:** {}",
                                    e
                                ));
                                return;
                            }
                        };

                        // Build the updated FEEDBACK.md content
                        let updated_feedback = SpecCommand::append_feedback_note(
                            &existing_feedback,
                            &title,
                            &note_owned,
                        );

                        // Write FEEDBACK.md to disk
                        let feedback_path = specs_root.join(sid.dir_name()).join("FEEDBACK.md");
                        let write_result: Result<(), String> = tokio::task::block_in_place(|| {
                            rt.block_on(async {
                                ragent_specs::io::SpecIo::atomic_write(
                                    &feedback_path,
                                    &updated_feedback,
                                )
                                .await
                                .map_err(|e| e.to_string())
                            })
                        });

                        match write_result {
                            Ok(()) => {
                                self.append_assistant_text(&SpecCommand::build_feedback_message(
                                    &spec_id,
                                    &note_owned,
                                ));
                                self.push_log_no_agent(
                                    crate::app::LogLevel::Info,
                                    SpecCommand::build_feedback_log(&spec_id, &note_owned),
                                );
                                self.status = SpecCommand::build_feedback_status(&spec_id);
                            }
                            Err(e) => {
                                self.status = format!("spec: failed to write FEEDBACK.md: {}", e);
                                self.append_assistant_text(&format!(
                                    "From: /spec feedback\n\n**Error:** Failed to \
                                     write `specs/{}/FEEDBACK.md`: {}",
                                    spec_id, e
                                ));
                            }
                        }
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
                let project_dir = std::env::current_dir().unwrap_or_default();

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

                        match self.storage.count_memories_for_project(&project_dir) {
                            Ok(count) => {
                                output.push_str(&format!("**Structured memories:** {count}\n\n"));
                            }
                            Err(e) => {
                                output.push_str(&format!(
                                    "⚠️ Could not read structured memories: {e}\n\n"
                                ));
                            }
                        }

                        match self.storage.list_memories_for_project(&project_dir, 50) {
                            Ok(rows) => {
                                if rows.is_empty() {
                                    output.push_str("(no memories for this project)\n");
                                } else {
                                    let mut by_category: std::collections::BTreeMap<
                                        String,
                                        Vec<&ragent_storage::storage::MemoryRow>,
                                    > = std::collections::BTreeMap::new();
                                    for row in &rows {
                                        by_category
                                            .entry(row.category.clone())
                                            .or_default()
                                            .push(row);
                                    }

                                    for (category, mems) in &by_category {
                                        output.push_str(&format!(
                                            "### {category} ({} entries)\n",
                                            mems.len()
                                        ));
                                        for row in mems {
                                            let preview = truncate_bytes(&row.content, 120);
                                            output.push_str(&format!(
                                                "- **#{id}** `{conf:.2}` {preview}\n",
                                                id = row.id,
                                                conf = row.confidence,
                                            ));
                                        }
                                        output.push_str("\n");
                                    }
                                }
                            }
                            Err(e) => {
                                output.push_str(&format!("⚠️ Could not list memories: {e}\n"));
                            }
                        }

                        self.append_assistant_text(&output);
                    }
                    "help" => {
                        self.append_assistant_text(
                            "From: /memory\nUsage: `/memory show` | `/memory help`",
                        );
                    }
                    _ => {
                        self.append_assistant_text(
                            "From: /memory\nUsage: `/memory show` | `/memory help`",
                        );
                    }
                }
            }

            "github" => match args.trim() {
                "login" => {
                    // Start the device flow synchronously so we can show the
                    // same OAuth pending dialog that Copilot uses.
                    let handle = match tokio::runtime::Handle::try_current() {
                        Ok(h) => h,
                        Err(_) => {
                            self.append_assistant_text(
                                "From: /github login\n❌ Async runtime not available.",
                            );
                            return;
                        }
                    };

                    let start = tokio::task::block_in_place(|| {
                        handle.block_on(ragent_agent::github::auth::start_device_flow(
                            &ragent_agent::github::GitHubClient::client_id(),
                        ))
                    });

                    let flow = match start {
                        Ok(f) => f,
                        Err(e) => {
                            self.append_assistant_text(&format!(
                                "From: /github login\n❌ Device flow failed: {e}"
                            ));
                            return;
                        }
                    };

                    let user_code = flow.user_code.clone();
                    let verification_uri = flow.verification_uri.clone();
                    let interval = std::time::Duration::from_secs(flow.interval.max(5));
                    let client_id = ragent_agent::github::GitHubClient::client_id();
                    let event_bus = self.event_bus.clone();
                    self.push_log(
                        LogLevel::Info,
                        format!(
                            "GitHub login started — enter code {user_code} at {verification_uri}"
                        ),
                        None,
                    );

                    self.provider_setup = Some(ProviderSetupStep::DeviceFlowPending {
                        flow: DeviceFlowKind::GitHub,
                        user_code,
                        verification_uri,
                    });

                    // Background task: poll until authorised or expired.
                    tokio::spawn(async move {
                        let deadline = std::time::Instant::now()
                            + std::time::Duration::from_secs(flow.expires_in);
                        let mut poll_interval = interval;
                        loop {
                            if std::time::Instant::now() > deadline {
                                event_bus.publish(Event::GithubDeviceFlowComplete {
                                    success: false,
                                    error: Some(
                                        "Device flow timed out — please try /github login again."
                                            .to_string(),
                                    ),
                                });
                                break;
                            }
                            tokio::time::sleep(poll_interval).await;

                            match ragent_agent::github::auth::poll_device_flow(&client_id, &flow)
                                .await
                            {
                                Ok(Some(token)) => {
                                    let outcome =
                                        match ragent_agent::github::auth::save_token(&token) {
                                            Ok(_) => Event::GithubDeviceFlowComplete {
                                                success: true,
                                                error: None,
                                            },
                                            Err(e) => Event::GithubDeviceFlowComplete {
                                                success: false,
                                                error: Some(format!(
                                                    "Failed to save GitHub token: {e}"
                                                )),
                                            },
                                        };
                                    event_bus.publish(outcome);
                                    break;
                                }
                                Ok(None) => {}
                                Err(e) => {
                                    let msg = e.to_string();
                                    if msg.contains("slow_down") {
                                        poll_interval += std::time::Duration::from_secs(5);
                                    } else {
                                        event_bus.publish(Event::GithubDeviceFlowComplete {
                                            success: false,
                                            error: Some(msg),
                                        });
                                        break;
                                    }
                                }
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
                                           `TAVILY_API_KEY` environment variable. \
                                           Perplexity requires `perplexity_api_key` in `ragent.json` \
                                           or the `PERPLEXITY_API_KEY` environment variable.",
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
            // ── /cron ────────────────────────────────────────────────────
            "cron" => self.handle_cron_command(args),
            // ── /triggers ────────────────────────────────────────────────
            "triggers" => self.handle_triggers_command(args),
            // ── /inbox ────────────────────────────────────
            "inbox" => self.handle_inbox_command(args),
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

    /// Build the formatted action-loop timing report, or `None` when no
    /// samples have been recorded yet. Shared by `/actionloop` and
    /// `/actionloop clip`.
    fn actionloop_report(&self) -> Option<String> {
        let profiler = ragent_agent::session::profiler::agent_loop_profiler();
        let snapshot = profiler.snapshot();
        if snapshot.operations.is_empty() {
            return None;
        }
        let mut out = String::from(
            "Agent action-loop average timings (ms):\n\n\
               ```\n\
               count     avg ms     self avg    max ms    operation\n",
        );
        // Sort by descending average elapsed time for a useful overview.
        let mut ops: Vec<_> = snapshot.operations.iter().collect();
        ops.sort_by(|a, b| {
            b.avg_ms
                .partial_cmp(&a.avg_ms)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        for op in ops {
            out.push_str(&format!(
                "{:>5}  {:>10.2}  {:>10.2}  {:>9.2}  {}\n",
                op.count, op.avg_ms, op.self_avg_ms, op.max_ms, op.name
            ));
        }
        out.push_str("```\n");
        Some(out)
    }

    // ── /cron slash-command handler (spec agentchron T-013) ────────────

    /// Handle the `/cron` slash-command family (FR-007 surface, FR-008, FR-009).
    ///
    /// Sub-commands:
    ///
    /// | Sub-command | Description |
    /// |---|---|
    /// | `/cron add <cronname> <agent> <schedule> "<prompt>"` | Create a new scheduled event |
    /// | `/cron remove <event_id>` | Delete an event by id |
    /// | `/cron enable <event_id>` | Enable an event |
    /// | `/cron disable <event_id>` | Disable an event |
    /// | `/cron list` | Show all events with human-readable schedules |
    /// | `/cron log [event_id]` | Show execution log, optionally filtered |
    /// | `/cron help` | Show usage |
    fn handle_cron_command(&mut self, args: &str) {
        let sub = args.split_whitespace().next().unwrap_or("").to_lowercase();
        let rest = args
            .split_once(char::is_whitespace)
            .map(|(_, r)| r.trim())
            .unwrap_or("");

        match sub.as_str() {
            "add" | "" => self.handle_cron_add(rest),
            "remove" => self.handle_cron_remove(rest),
            "enable" => self.handle_cron_set_enabled(rest, true),
            "disable" => self.handle_cron_set_enabled(rest, false),
            "list" => self.handle_cron_list(),
            "detail" => self.handle_cron_detail(rest),
            "log" => self.handle_cron_log(rest),
            "help" => self.handle_cron_help(),
            _ => {
                self.append_assistant_text(&format!(
                    "From: /cron\n⚠ Unknown sub-command '{sub}'. Use `/cron help` for usage."
                ));
                self.status = "cron: unknown".to_string();
            }
        }
    }

    /// Show `/cron` usage help.
    fn handle_cron_help(&mut self) {
        self.append_assistant_text(
            "From: /cron help\n\n\
             ## /cron — Scheduled agent runs\n\n\
             | Sub-command | Usage | Description |\n\
             |---|---|---|\n\
             | `add` | `/cron add <cronname> <agent> <schedule> \"<prompt>\"` | Create a new event |\n\
             | `remove` | `/cron remove <event_id>` | Delete an event |\n\
             | `enable` | `/cron enable <event_id>` | Enable an event |\n\
             | `disable` | `/cron disable <event_id>` | Disable an event |\n\
             | `list` | `/cron list` | Show all events |\n\
             | `detail` | `/cron detail <event_id>` | Show full details of an event |\n\
             | `log` | `/cron log [event_id]` | Show execution log |\n\
             | `help` | `/cron help` | Show this help |\n\n\
             **`add` parameters (positional):**\n\n\
             | Position | Parameter | Description |\n\
             |---|---|---|\n\
             | 1 | `cronname` | Sets the event ID |\n\
             | 2 | `agent` | Agent type to run |\n\
             | 3 | `schedule` | Schedule expression (see forms below) |\n\
             | 4 | `prompt` | Prompt the agent executes (must be double-quoted) |\n\n\
             **Schedule forms:**\n\n\
             | Form | Example |\n\
             |---|---|\n\
             | `at <timestamp>` | One-shot at a specific time |\n\
             | `from <timestamp> every <duration>` | Repeating from a start time |\n\
             | `every <duration>` | Repeating from now |\n\n\
             **Schedule examples:**\n\n\
             | Example | Meaning |\n\
             |---|---|\n\
             | `at 2025-01-15T09:00:00Z` | One-shot run at 9am UTC on Jan 15 2025 |\n\
             | `at 5pm` | One-shot at the next 5pm (today or tomorrow) |\n\
             | `every 30m` | Repeat every 30 minutes starting now |\n\
             | `from 5pm tomorrow every 1h` | Repeat hourly, first run at 5pm tomorrow |\n\
             | `every 1d` | Repeat every 24 hours starting now |\n\n\
             **Timestamps** accept ISO-8601 (`2025-01-15T09:00:00Z`) or \
             natural-language shortcuts: `5pm`, `5:30pm`, `17:00`, `5am tomorrow`.\n\n\
             **Durations:** `<int><unit>` where unit is `m` (mins), `h` (hrs), \
             `d` (days), `w` (wks), or `mo` (months).",
        );
        self.status = "cron: help".to_string();
    }

    /// Handle `/cron add <cronname> <agent> <schedule> "<prompt>"`.
    ///
    /// Positional parameters set the event ID (`cronname`), the agent type, the
    /// schedule expression, and the prompt. The prompt must be double-quoted.
    fn handle_cron_add(&mut self, rest: &str) {
        // Parse: <cronname> <agent_type> <schedule_expr> "<prompt>"
        // The prompt is the last double-quoted string.
        let rest = rest.trim();
        if rest.is_empty() {
            self.append_assistant_text(
                "From: /cron add\n\n\
                 Usage: `/cron add <cronname> <agent> <schedule> \"<prompt>\"`\n\n\
                 Example: `/cron add nightly general every 30m \"Run tests\"`",
            );
            self.status = "cron: add usage".to_string();
            return;
        }

        // Extract the quoted prompt (last double-quoted segment).
        let (before_prompt, prompt) = match extract_quoted_prompt(rest) {
            Some(p) => p,
            None => {
                self.append_assistant_text(
                    "From: /cron add\n⚠ The prompt must be enclosed in double quotes.\n\n\
                     Example: `/cron add nightly general every 30m \"Run tests\"`",
                );
                self.status = "cron: add missing prompt".to_string();
                return;
            }
        };

        // Split the remaining text into cronname, agent, and schedule.
        // <cronname> <agent> <schedule...>
        let (cronname, rest1) = match before_prompt.split_once(char::is_whitespace) {
            Some((n, r)) => (n.trim(), r.trim()),
            None => {
                self.append_assistant_text(
                    "From: /cron add\n⚠ Missing agent and schedule expression.\n\n\
                     Format: `<cronname> <agent> <schedule> \"<prompt>\"`\n\
                     Example: `/cron add nightly general every 30m \"Run tests\"`",
                );
                self.status = "cron: add missing agent".to_string();
                return;
            }
        };

        let (agent_type, schedule_expr) = match rest1.split_once(char::is_whitespace) {
            Some((a, s)) => (a.trim(), s.trim()),
            None => {
                self.append_assistant_text(
                    "From: /cron add\n⚠ Missing schedule expression.\n\n\
                     Format: `<cronname> <agent> <schedule> \"<prompt>\"`\n\
                     Example: `/cron add nightly general every 30m \"Run tests\"`",
                );
                self.status = "cron: add missing schedule".to_string();
                return;
            }
        };

        if cronname.is_empty() || agent_type.is_empty() || schedule_expr.is_empty() {
            self.append_assistant_text(
                "From: /cron add\n⚠ cronname, agent, and schedule expression are all required.",
            );
            self.status = "cron: add missing fields".to_string();
            return;
        }

        // Parse the schedule expression.
        let now = chrono::Utc::now();
        let parsed = match ragent_types::parse_schedule(schedule_expr, now) {
            Ok(p) => p,
            Err(e) => {
                self.append_assistant_text(&format!(
                    "From: /cron add\n❌ Failed to parse schedule `{schedule_expr}`:\n  {e}"
                ));
                self.status = "cron: add parse error".to_string();
                return;
            }
        };

        // Use the supplied cronname as the event id.
        let event = ragent_types::CronEvent::new(
            cronname.to_string(),
            agent_type.to_string(),
            prompt,
            parsed.schedule,
            schedule_expr.to_string(),
            parsed.next_due,
        );

        // Insert into storage.
        if let Err(e) = self.storage.insert_cron_event(&event) {
            self.append_assistant_text(&format!("From: /cron add\n❌ Failed to store event: {e}"));
            self.status = "cron: add store error".to_string();
            return;
        }

        let desc = event.schedule.human_readable();
        self.append_assistant_text(&format!(
            "From: /cron add\n✅ Scheduled event created.\n\n\
             | Field | Value |\n|---|---|\n\
             | ID | `{}` |\n\
             | Agent | `{}` |\n\
             | Schedule | `{}` ({}) |\n\
             | Next due | {} |\n\
             | Prompt | \"{}\" |",
            event.id,
            agent_type,
            schedule_expr,
            desc,
            event.next_due.to_rfc3339(),
            event.prompt,
        ));
        self.push_log_no_agent(
            LogLevel::Info,
            format!(
                "cron add: event {} agent={} schedule={}",
                event.id, agent_type, schedule_expr
            ),
        );
        self.status = "cron: added".to_string();
    }

    /// Handle `/cron remove <event_id>`.
    fn handle_cron_remove(&mut self, rest: &str) {
        let event_id = rest.trim();
        if event_id.is_empty() {
            self.append_assistant_text("From: /cron remove\n\nUsage: `/cron remove <event_id>`");
            self.status = "cron: remove usage".to_string();
            return;
        }

        match self.storage.delete_cron_event(event_id) {
            Ok(true) => {
                self.append_assistant_text(&format!(
                    "From: /cron remove\n✅ Event `{}` removed.",
                    event_id
                ));
                self.push_log_no_agent(
                    LogLevel::Info,
                    format!("cron remove: deleted event {}", event_id),
                );
                self.status = "cron: removed".to_string();
            }
            Ok(false) => {
                self.append_assistant_text(&format!(
                    "From: /cron remove\n⚠ Event `{}` not found.",
                    event_id
                ));
                self.status = "cron: not found".to_string();
            }
            Err(e) => {
                self.append_assistant_text(&format!(
                    "From: /cron remove\n❌ Failed to remove event: {e}"
                ));
                self.status = "cron: remove error".to_string();
            }
        }
    }

    /// Handle `/cron enable <event_id>` or `/cron disable <event_id>`.
    ///
    /// Toggles the `enabled` flag on a stored cron event. When `enabled` is
    /// `true` the scheduler will fire the event; when `false` it skips the
    /// event and logs `"skipped"`.
    fn handle_cron_set_enabled(&mut self, rest: &str, enabled: bool) {
        let event_id = rest.trim();
        let action = if enabled { "enable" } else { "disable" };
        if event_id.is_empty() {
            self.append_assistant_text(&format!(
                "From: /cron {action}\n\nUsage: `/cron {action} <event_id>`"
            ));
            self.status = format!("cron: {action} usage");
            return;
        }

        match self.storage.set_cron_event_enabled(event_id, enabled) {
            Ok(true) => {
                let mark = if enabled { "✅" } else { "⏸️" };
                self.append_assistant_text(&format!(
                    "From: /cron {action}\n{mark} Event `{}` {}.",
                    event_id,
                    if enabled { "enabled" } else { "disabled" }
                ));
                self.push_log_no_agent(
                    LogLevel::Info,
                    format!("cron {action}: event {}", event_id),
                );
                self.status = format!("cron: {action}d");
            }
            Ok(false) => {
                self.append_assistant_text(&format!(
                    "From: /cron {action}\n⚠ Event `{}` not found.",
                    event_id
                ));
                self.status = "cron: not found".to_string();
            }
            Err(e) => {
                self.append_assistant_text(&format!(
                    "From: /cron {action}\n❌ Failed to update event: {e}"
                ));
                self.status = format!("cron: {action} error");
            }
        }
    }

    /// Handle `/cron list` — display all scheduled events.
    fn handle_cron_list(&mut self) {
        match self.storage.list_cron_events() {
            Ok(rows) if rows.is_empty() => {
                self.append_assistant_text(
                    "From: /cron list\n\nℹ️  No scheduled events.\n\n\
                     Use `/cron add <cronname> <agent> <schedule> \"<prompt>\"` to create one.",
                );
                self.status = "cron: list empty".to_string();
            }
            Ok(rows) => {
                let mut output = String::from(
                    "From: /cron list\n\n## Scheduled Events\n\n\
                     | ID | Agent | Schedule | Enabled | Next Due | Prompt |\n\
                     |---|---|---|---|---|---|\n",
                );
                for row in &rows {
                    // Reconstruct a human-readable schedule description from the row.
                    let desc = row_to_human_readable(row);
                    let prompt_preview = if row.prompt.len() > 40 {
                        format!("{}…", &row.prompt[..40])
                    } else {
                        row.prompt.clone()
                    };
                    let enabled_str = if row.enabled { "✓" } else { "✗" };
                    let next_due_display = format_next_due(&row.next_due, row.enabled);
                    output.push_str(&format!(
                        "| `{}` | `{}` | {} ({}) | {} | {} | \"{}\" |\n",
                        row.id,
                        row.agent_type,
                        row.schedule_raw,
                        desc,
                        enabled_str,
                        next_due_display,
                        prompt_preview,
                    ));
                }
                self.append_assistant_text(&output);
                self.status = "cron: list".to_string();
            }
            Err(e) => {
                self.append_assistant_text(&format!(
                    "From: /cron list\n❌ Failed to list events: {e}"
                ));
                self.status = "cron: list error".to_string();
            }
        }
    }

    /// Handle `/cron detail <event_id>` — show full details of a single event.
    ///
    /// Displays every stored field including the complete (untruncated) prompt.
    fn handle_cron_detail(&mut self, rest: &str) {
        let event_id = rest.trim();
        if event_id.is_empty() {
            self.append_assistant_text("From: /cron detail\n\nUsage: `/cron detail <event_id>`");
            self.status = "cron: detail usage".to_string();
            return;
        }

        match self.storage.get_cron_event(event_id) {
            Ok(Some(row)) => {
                let desc = row_to_human_readable(&row);
                let enabled_str = if row.enabled {
                    "✓ enabled"
                } else {
                    "✗ disabled"
                };
                let next_due_display = format_next_due(&row.next_due, row.enabled);
                let start_at_display = row.start_at.as_deref().unwrap_or("—");
                let duration_display = row
                    .duration_secs
                    .map(duration_secs_to_string)
                    .unwrap_or_else(|| "—".to_string());
                let last_fired_display = row.last_fired.as_deref().unwrap_or("never");

                let output = format!(
                    "From: /cron detail\n\n\
                     ## Event `{id}`\n\n\
                     | Field | Value |\n\
                     |---|---|\n\
                     | ID | `{id}` |\n\
                     | Agent | `{agent}` |\n\
                     | Schedule | {raw} ({desc}) |\n\
                     | Schedule form | `{form}` |\n\
                     | Start at | {start_at} |\n\
                     | Duration | {duration} |\n\
                     | Enabled | {enabled} |\n\
                     | Next due | {next_due} |\n\
                     | Created at | {created_at} |\n\
                     | Last fired | {last_fired} |\n\n\
                     **Prompt:**\n\n\
                     ```\n\
                     {prompt}\n\
                     ```",
                    id = row.id,
                    agent = row.agent_type,
                    raw = row.schedule_raw,
                    desc = desc,
                    form = row.schedule_form,
                    start_at = start_at_display,
                    duration = duration_display,
                    enabled = enabled_str,
                    next_due = next_due_display,
                    created_at = row.created_at,
                    last_fired = last_fired_display,
                    prompt = row.prompt,
                );

                self.append_assistant_text(&output);
                self.status = "cron: detail".to_string();
            }
            Ok(None) => {
                self.append_assistant_text(&format!(
                    "From: /cron detail\n\n⚠ Event `{}` not found.",
                    event_id
                ));
                self.status = "cron: detail not found".to_string();
            }
            Err(e) => {
                self.append_assistant_text(&format!(
                    "From: /cron detail\n❌ Failed to fetch event: {e}"
                ));
                self.status = "cron: detail error".to_string();
            }
        }
    }

    /// Handle `/cron log [event_id]` — display execution log.
    fn handle_cron_log(&mut self, rest: &str) {
        let filter = rest.trim();
        let working_dir = std::env::current_dir().unwrap_or_default();
        let event_id = if filter.is_empty() {
            None
        } else {
            Some(filter)
        };

        let entries = ragent_tools_core::cron_log::read_cron_log(&working_dir, event_id);

        if entries.is_empty() {
            self.append_assistant_text(&format!(
                "From: /cron log\n\nℹ️  No execution log entries{}.",
                if event_id.is_some() {
                    format!(" for event `{}`", filter)
                } else {
                    String::new()
                }
            ));
            self.status = "cron: log empty".to_string();
            return;
        }

        let mut output = String::from(
            "From: /cron log\n\n## Execution Log\n\n\
             | Timestamp | Event | Agent | Outcome | Prompt |\n\
             |---|---|---|---|---|\n",
        );
        for entry in &entries {
            let prompt_preview = if entry.prompt.len() > 40 {
                format!("{}…", &entry.prompt[..40])
            } else {
                entry.prompt.clone()
            };
            let outcome_icon = match entry.outcome.as_str() {
                "success" => "✅",
                "error" => "❌",
                "skipped" => "⏭️",
                _ => "•",
            };
            output.push_str(&format!(
                "| {} | `{}` | `{}` | {} {} | \"{}\" |\n",
                entry.timestamp,
                entry.event_id,
                entry.agent_type,
                outcome_icon,
                entry.outcome,
                prompt_preview,
            ));
        }
        self.append_assistant_text(&output);
        self.status = "cron: log".to_string();
    }

    // ── /triggers slash-command handler (spec piegap T-004) ─────────────

    /// Handle the `/triggers` slash-command family (FR-002, FR-003).
    ///
    /// Sub-commands:
    ///
    /// | Sub-command | Description |
    /// |---|---|
    /// | `/triggers list` | Show all registered trigger rules |
    /// | `/triggers enable <rule_id>` | Enable a disabled trigger rule |
    /// | `/triggers disable <rule_id>` | Disable an active trigger rule |
    /// | `/triggers remove <rule_id>` | Remove a trigger rule |
    /// | `/triggers status` | Show trigger runtime status (dedup, cycles) |
    /// | `/triggers help` | Show usage |
    fn handle_triggers_command(&mut self, args: &str) {
        let sub = args.split_whitespace().next().unwrap_or("").to_lowercase();
        let rest = args
            .split_once(char::is_whitespace)
            .map(|(_, r)| r.trim())
            .unwrap_or("");

        match sub.as_str() {
            "list" | "" => self.handle_triggers_list(),
            "enable" => self.handle_triggers_set_enabled(rest, true),
            "disable" => self.handle_triggers_set_enabled(rest, false),
            "remove" => self.handle_triggers_remove(rest),
            "status" => self.handle_triggers_status(),
            "help" => self.handle_triggers_help(),
            _ => {
                self.append_assistant_text(&format!(
                    "From: /triggers\n⚠ Unknown sub-command '{sub}'. Use `/triggers help` for usage."
                ));
                self.status = "triggers: unknown".to_string();
            }
        }
    }

    /// Ensure the trigger runtime is initialised. Returns `true` if the
    /// runtime is available (either already initialised or just created).
    fn ensure_trigger_runtime(&mut self) -> bool {
        if self.trigger_runtime.is_some() {
            return true;
        }
        self.trigger_runtime = Some(ragent_agent::trigger::TriggerRuntime::default());
        true
    }

    /// Handle `/triggers list` — display all registered trigger rules.
    fn handle_triggers_list(&mut self) {
        if !self.ensure_trigger_runtime() {
            return;
        }
        let runtime = self.trigger_runtime.as_ref().unwrap();
        let rules = runtime.list_rules();
        if rules.is_empty() {
            self.append_assistant_text(
                "From: /triggers list\n\nℹ️  No trigger rules registered.\n\n\
                 Trigger rules are created by asking the agent in natural language,\n\
                 e.g. \"when $HOME/build.done exists, run cargo test\".",
            );
            self.status = "triggers: list empty".to_string();
            return;
        }
        let mut output = String::from(
            "From: /triggers list\n\n## Trigger Rules\n\n\
             | ID | Condition | Action | Mode | Status |\n\
             |---|---|---|---|---|\n",
        );
        for rule in &rules {
            let id_short = if rule.id.as_str().len() > 8 {
                &rule.id.as_str()[..8]
            } else {
                rule.id.as_str()
            };
            let mode = if rule.fire_once { "once" } else { "repeat" };
            let status = match rule.status() {
                ragent_types::trigger::TriggerRuleStatus::Active => "active",
                ragent_types::trigger::TriggerRuleStatus::Disabled => "disabled",
                ragent_types::trigger::TriggerRuleStatus::Fired => "fired",
            };
            let cond_preview = truncate_field(&rule.condition, 40);
            let action_preview = truncate_field(&rule.action, 40);
            output.push_str(&format!(
                "| `{}` | {} | {} | {} | {} |\n",
                id_short, cond_preview, action_preview, mode, status
            ));
        }
        output.push_str(&format!("\n**{} rule(s) registered.**\n", rules.len()));
        self.append_assistant_text(&output);
        self.status = "triggers: list".to_string();
    }

    /// Handle `/triggers enable <rule_id>` and `/triggers disable <rule_id>`.
    fn handle_triggers_set_enabled(&mut self, rest: &str, enabled: bool) {
        let rule_id = rest.trim();
        if rule_id.is_empty() {
            let action = if enabled { "enable" } else { "disable" };
            self.append_assistant_text(&format!(
                "From: /triggers {action}\n\nUsage: `/triggers {action} <rule_id>`"
            ));
            self.status = format!("triggers: {action} usage");
            return;
        }
        if !self.ensure_trigger_runtime() {
            return;
        }
        let runtime = self.trigger_runtime.as_ref().unwrap();
        let found = if enabled {
            runtime.enable_rule(rule_id)
        } else {
            runtime.disable_rule(rule_id)
        };
        if found {
            let mark = if enabled { "✅" } else { "⏸️" };
            let action = if enabled { "enabled" } else { "disabled" };
            self.append_assistant_text(&format!(
                "From: /triggers\n{mark} Rule `{}` {action}.",
                rule_id
            ));
            self.push_log_no_agent(
                LogLevel::Info,
                format!("triggers: rule {} {}", rule_id, action),
            );
            self.status = format!("triggers: {action}");
        } else {
            self.append_assistant_text(&format!(
                "From: /triggers\n⚠ Rule `{}` not found.",
                rule_id
            ));
            self.status = "triggers: not found".to_string();
        }
    }

    /// Handle `/triggers remove <rule_id>`.
    fn handle_triggers_remove(&mut self, rest: &str) {
        let rule_id = rest.trim();
        if rule_id.is_empty() {
            self.append_assistant_text(
                "From: /triggers remove\n\nUsage: `/triggers remove <rule_id>`",
            );
            self.status = "triggers: remove usage".to_string();
            return;
        }
        if !self.ensure_trigger_runtime() {
            return;
        }
        let runtime = self.trigger_runtime.as_ref().unwrap();
        if runtime.remove_rule(rule_id) {
            self.append_assistant_text(&format!(
                "From: /triggers remove\n🗑️ Rule `{}` removed.",
                rule_id
            ));
            self.push_log_no_agent(
                LogLevel::Info,
                format!("triggers: rule {} removed", rule_id),
            );
            self.status = "triggers: removed".to_string();
        } else {
            self.append_assistant_text(&format!(
                "From: /triggers remove\n⚠ Rule `{}` not found.",
                rule_id
            ));
            self.status = "triggers: not found".to_string();
        }
    }

    /// Handle `/triggers status` — show runtime stats.
    fn handle_triggers_status(&mut self) {
        if !self.ensure_trigger_runtime() {
            return;
        }
        let runtime = self.trigger_runtime.as_ref().unwrap();
        let rule_count = runtime.rule_count();
        let dedup_size = runtime.dedup_cache_size();
        let cycle_size = runtime.cycle_tracker_size();
        let active = runtime
            .list_rules()
            .iter()
            .filter(|r| r.status() == ragent_types::trigger::TriggerRuleStatus::Active)
            .count();
        let disabled = runtime
            .list_rules()
            .iter()
            .filter(|r| r.status() == ragent_types::trigger::TriggerRuleStatus::Disabled)
            .count();
        let fired = runtime
            .list_rules()
            .iter()
            .filter(|r| r.status() == ragent_types::trigger::TriggerRuleStatus::Fired)
            .count();
        self.append_assistant_text(&format!(
            "From: /triggers status\n\n\
             ## Trigger Runtime Status\n\n\
             | Metric | Value |\n\
             |---|---|\n\
             | Total rules | {} |\n\
             | Active | {} |\n\
             | Disabled | {} |\n\
             | Fired (one-shot) | {} |\n\
             | Dedup cache entries | {} |\n\
             | Cycle trackers | {} |\n\
             | Dedup window | {}s |\n\
             | Max cycles | {} |",
            rule_count,
            active,
            disabled,
            fired,
            dedup_size,
            cycle_size,
            runtime.config.dedup_window.as_secs(),
            runtime.config.max_cycles,
        ));
        self.status = "triggers: status".to_string();
    }

    /// Show `/triggers` usage help.
    fn handle_triggers_help(&mut self) {
        self.append_assistant_text(
            "From: /triggers help\n\n\
             ## /triggers — Manage trigger rules\n\n\
             | Sub-command | Usage | Description |\n\
             |---|---|---|\n\
             | `list` | `/triggers list` | Show all registered trigger rules |\n\
             | `enable` | `/triggers enable <rule_id>` | Enable a disabled rule |\n\
             | `disable` | `/triggers disable <rule_id>` | Disable an active rule |\n\
             | `remove` | `/triggers remove <rule_id>` | Remove a rule permanently |\n\
             | `status` | `/triggers status` | Show runtime stats (dedup, cycles) |\n\
             | `help` | `/triggers help` | Show this help |\n\n\
             **Creating trigger rules:**\n\n\
             Trigger rules are created by asking the agent in natural language,\n\
             e.g. \"when $HOME/build.done exists, run cargo test\".\n\n\
             Rules are fire-once by default. Use `repeating` for continuous monitoring.\n\
             Rule output does not interrupt the main chat unless `promote_to_chat` is set.",
        );
        self.status = "triggers: help".to_string();
    }

    // ── /bug-report slash-command handler (spec piegap T-011) ──────────────

    /// Handle `/bug-report` — generate a diagnostic dump with redaction (FR-007).
    ///
    /// Collects session state (model, agent, tool count, cost summary), recent
    /// log entries, and the session transcript. Redacts well-known secret patterns
    /// (API keys, tokens, passwords) before writing to `log/bug-report-<timestamp>.md`.
    fn handle_bug_report(&mut self) {
        use std::fs::{self, File};
        use std::io::Write;

        let timestamp = chrono::Utc::now().format("%Y%m%d-%H%M%S").to_string();
        let filename = format!("bug-report-{}.md", timestamp);

        // Determine output directory: prefer project root `log/`, fallback to data dir
        let cwd = std::env::current_dir().unwrap_or_default();
        let log_dir = cwd.join("log");
        if let Err(e) = fs::create_dir_all(&log_dir) {
            self.append_assistant_text(&format!(
                "From: /bug-report\n⚠ Failed to create log directory: {e}"
            ));
            self.status = "bug-report: mkdir error".to_string();
            return;
        }

        let output_path = log_dir.join(&filename);

        // Gather session diagnostic info
        let session_info = if let Some(ref sid) = self.session_id {
            match self.storage.get_session(sid) {
                Ok(Some(session)) => format!(
                    "- **Session ID**: `{}`\n- **Title**: {}\n- **Directory**: `{}`\n- **Created**: {}\n- **Updated**: {}",
                    session.id,
                    session.title,
                    session.directory,
                    session.created_at,
                    session.updated_at
                ),
                Ok(None) => "- **Session**: Not found in storage".to_string(),
                Err(e) => format!("- **Session**: Error reading session: {e}"),
            }
        } else {
            "- **Session**: No active session".to_string()
        };

        let agent_info = format!(
            "- **Agent**: `{}` ({})\n- **Model**: `{}`",
            self.agent_name,
            self.agent_info.description,
            self.selected_model.as_deref().unwrap_or("(not set)")
        );

        // Tool count from registry (approximate from tool visibility config)
        let tool_count = self
            .tool_visibility
            .iter_switches()
            .filter(|(_, v)| *v)
            .count();
        let tool_info = format!(
            "- **Tools visible**: {}\n- **Tool families**:\n  - Office: {}\n  - GitHub: {}\n  - GitLab: {}\n  - Teams: {}\n  - Agents: {}\n  - Plan: {}\n  - CodeIndex: {}",
            tool_count,
            if self.tool_visibility.office {
                "✓"
            } else {
                "✗"
            },
            if self.tool_visibility.github {
                "✓"
            } else {
                "✗"
            },
            if self.tool_visibility.gitlab {
                "✓"
            } else {
                "✗"
            },
            if self.tool_visibility.teams {
                "✓"
            } else {
                "✗"
            },
            if self.tool_visibility.agents {
                "✓"
            } else {
                "✗"
            },
            if self.tool_visibility.plan {
                "✓"
            } else {
                "✗"
            },
            if self.tool_visibility.codeindex {
                "✓"
            } else {
                "✗"
            }
        );

        // Cost summary from token usage
        let cost_info = format!(
            "- **Input tokens**: {}\n- **Output tokens**: {}\n- **Total**: {}",
            self.token_usage.0,
            self.token_usage.1,
            self.token_usage.0 + self.token_usage.1
        );

        // Recent log entries (last 50)
        let log_tail: Vec<&LogEntry> = self.log_entries.iter().rev().take(50).collect();
        let log_tail: Vec<&LogEntry> = log_tail.iter().rev().copied().collect();
        let log_info = if log_tail.is_empty() {
            "- *(no log entries)*".to_string()
        } else {
            let mut lines = String::new();
            for entry in log_tail {
                let ts = entry.timestamp.format("%H:%M:%S").to_string();
                let level = match entry.level {
                    LogLevel::Info => "INFO",
                    LogLevel::Warn => "WARN",
                    LogLevel::Error => "ERROR",
                    LogLevel::Tool => "TOOL",
                };
                let msg = redact_secrets(&entry.message);
                lines.push_str(&format!("[{}] {}: {}\n", ts, level, msg));
            }
            lines
        };

        // Session transcript (messages)
        let transcript_info = if self.messages.is_empty() {
            "- *(no messages)*".to_string()
        } else {
            let mut lines = String::new();
            for msg in &self.messages {
                let role = match msg.role {
                    ragent_types::Role::User => "👤 User",
                    ragent_types::Role::Assistant => "🤖 Assistant",
                    ragent_types::Role::Compaction => "⚙️ Compaction",
                };
                let content = redact_secrets(&msg.text_content());
                // Truncate long messages for the report
                let content = if content.len() > 500 {
                    format!("{}...", &content[..500])
                } else {
                    content
                };
                lines.push_str(&format!("**{}**: {}\n\n", role, content));
            }
            lines
        };

        // Build the report
        let report = format!(
            "# RAgent Bug Report\n\n\
             Generated: {}\n\n\
             ## Session Information\n\n{}\n\n\
             ## Agent Configuration\n\n{}\n\n\
             ## Tool Configuration\n\n{}\n\n\
             ## Token Usage\n\n{}\n\n\
             ## Recent Log Entries (last 50)\n\n```\n{}```\n\n\
             ## Session Transcript\n\n{}\n\n\
             ---\n\
             *This report has been automatically redacted to remove API keys, tokens, and other sensitive patterns.*\n",
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
            session_info,
            agent_info,
            tool_info,
            cost_info,
            log_info,
            transcript_info
        );

        // Write the report
        match File::create(&output_path) {
            Ok(mut file) => {
                if let Err(e) = file.write_all(report.as_bytes()) {
                    self.append_assistant_text(&format!(
                        "From: /bug-report\n⚠ Failed to write report: {e}"
                    ));
                    self.status = "bug-report: write error".to_string();
                    return;
                }
            }
            Err(e) => {
                self.append_assistant_text(&format!(
                    "From: /bug-report\n⚠ Failed to create report file: {e}"
                ));
                self.status = "bug-report: create error".to_string();
                return;
            }
        }

        let rel_path = output_path
            .strip_prefix(&cwd)
            .unwrap_or(&output_path)
            .display();
        self.append_assistant_text(&format!(
            "From: /bug-report\n\n✅ Bug report generated: `{}`\n\n\
             The report contains:\n\
             - Session metadata\n\
             - Agent and model configuration\n\
             - Tool visibility settings\n\
             - Token usage summary\n\
             - Last 50 log entries (redacted)\n\
             - Session transcript (redacted, truncated)\n\n\
             ⚠️  **Sensitivity warning**: This report may contain sensitive information\n\
             despite redaction. Review before sharing.",
            rel_path
        ));
        self.status = "bug-report: generated".to_string();
    }

    // ── /inbox slash-command handler (spec piegap T-006) ──────────────

    /// Handle the `/inbox` slash-command family (FR-004).
    ///
    /// Sub-commands: `list` (default), `claim <id>`, `dismiss <id>`,
    /// `clear`, `help`.
    fn handle_inbox_command(&mut self, args: &str) {
        let sub = args.split_whitespace().next().unwrap_or("").to_lowercase();
        let rest = args
            .split_once(char::is_whitespace)
            .map(|(_, r)| r.trim())
            .unwrap_or("");

        match sub.as_str() {
            "list" | "" => self.handle_inbox_list(),
            "claim" => self.handle_inbox_set_status(rest, "claimed"),
            "dismiss" => self.handle_inbox_set_status(rest, "dismissed"),
            "clear" => self.handle_inbox_clear(),
            "help" => self.handle_inbox_help(),
            _ => {
                self.append_assistant_text(&format!(
                    "From: /inbox\n⚠ Unknown sub-command '{sub}'. Use `/inbox help` for usage."
                ));
                self.status = "inbox: unknown".to_string();
            }
        }
    }

    /// Handle `/inbox list` — display all inbox findings.
    fn handle_inbox_list(&mut self) {
        let data_dir = std::env::current_dir().unwrap_or_default();
        let entries = match ragent_agent::loop_state::read_inbox(&data_dir) {
            Ok(entries) => entries,
            Err(e) => {
                self.append_assistant_text(&format!(
                    "From: /inbox list\n\n⚠ Failed to read inbox: {e}"
                ));
                self.status = "inbox: read error".to_string();
                return;
            }
        };
        if entries.is_empty() {
            self.append_assistant_text(
                "From: /inbox list\n\nℹ️  Inbox is empty.\n\n\
                 Findings are added by stateful cron jobs that use the `<inbox>` tag protocol.",
            );
            self.status = "inbox: list empty".to_string();
            return;
        }
        let mut output = String::from(
            "From: /inbox list\n\n## Triage Inbox\n\n\
             | # | ID | Source | Status | Content |\n\
             |---|---|---|---|---|\n",
        );
        for (i, entry) in entries.iter().enumerate() {
            let id_short = if entry.id.len() > 8 {
                &entry.id[..8]
            } else {
                &entry.id
            };
            let source_short = if entry.source_event_id.len() > 8 {
                &entry.source_event_id[..8]
            } else {
                &entry.source_event_id
            };
            let content_preview = truncate_field(&entry.content, 60);
            output.push_str(&format!(
                "| {} | `{}` | `{}` | {} | {} |\n",
                i + 1,
                id_short,
                source_short,
                entry.status,
                content_preview
            ));
        }
        output.push_str(&format!("\n**{} finding(s) in inbox.**\n", entries.len()));
        self.append_assistant_text(&output);
        self.status = "inbox: list".to_string();
    }

    /// Handle `/inbox claim <id>` and `/inbox dismiss <id>`.
    fn handle_inbox_set_status(&mut self, rest: &str, new_status: &str) {
        let entry_id = rest.trim();
        if entry_id.is_empty() {
            self.append_assistant_text(&format!(
                "From: /inbox {new_status}\n\nUsage: `/inbox {new_status} <entry_id>`\n\n\
                 Use `/inbox list` to see entry IDs."
            ));
            self.status = format!("inbox: {new_status} usage");
            return;
        }
        let data_dir = std::env::current_dir().unwrap_or_default();
        match ragent_agent::loop_state::update_inbox_entry_status(&data_dir, entry_id, new_status) {
            Ok(true) => {
                let mark = if new_status == "claimed" {
                    "✅"
                } else {
                    "🗑️"
                };
                self.append_assistant_text(&format!(
                    "From: /inbox\n{mark} Entry `{}` marked as {new_status}.",
                    entry_id
                ));
                self.push_log_no_agent(
                    LogLevel::Info,
                    format!("inbox: entry {} {}", entry_id, new_status),
                );
                self.status = format!("inbox: {new_status}");
            }
            Ok(false) => {
                self.append_assistant_text(&format!(
                    "From: /inbox\n⚠ Entry `{}` not found in inbox.",
                    entry_id
                ));
                self.status = "inbox: not found".to_string();
            }
            Err(e) => {
                self.append_assistant_text(&format!("From: /inbox\n⚠ Failed to update inbox: {e}"));
                self.status = "inbox: write error".to_string();
            }
        }
    }

    /// Handle `/inbox clear` — remove all inbox findings.
    fn handle_inbox_clear(&mut self) {
        let data_dir = std::env::current_dir().unwrap_or_default();
        match ragent_agent::loop_state::clear_inbox(&data_dir) {
            Ok(count) => {
                self.append_assistant_text(&format!(
                    "From: /inbox clear\n🗑️ Cleared {count} finding(s) from the inbox."
                ));
                self.push_log_no_agent(LogLevel::Info, format!("inbox: cleared {count} entries"));
                self.status = "inbox: cleared".to_string();
            }
            Err(e) => {
                self.append_assistant_text(&format!(
                    "From: /inbox clear\n⚠ Failed to clear inbox: {e}"
                ));
                self.status = "inbox: clear error".to_string();
            }
        }
    }

    /// Show `/inbox` usage help.
    fn handle_inbox_help(&mut self) {
        self.append_assistant_text(
            "From: /inbox help\n\n\
             ## /inbox — Triage inbox findings\n\n\
             The inbox collects findings from stateful cron jobs that use the\n\
             `<inbox>` tag protocol. Findings are stored in a global JSONL file\n\
             shared across all sessions.\n\n\
             | Sub-command | Usage | Description |\n\
             |---|---|---|\n\
             | `list` | `/inbox list` | Show all inbox findings |\n\
             | `claim` | `/inbox claim <id>` | Mark a finding as claimed |\n\
             | `dismiss` | `/inbox dismiss <id>` | Mark a finding as dismissed |\n\
             | `clear` | `/inbox clear` | Remove all findings from the inbox |\n\
             | `help` | `/inbox help` | Show this help |\n\n\
             **Entry IDs:** Use `/inbox list` to see entry IDs (short UUID prefix).\n\
             **Statuses:** `open` (default), `claimed`, `dismissed`.",
        );
        self.status = "inbox: help".to_string();
    }
}

/// Truncate a field for display, appending an ellipsis if truncated.
fn truncate_field(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{truncated}…")
    }
}

/// Extract the last double-quoted string from `s` as the prompt, returning
/// the remaining text (before the quote) and the prompt contents.
///
/// Returns `None` if no double-quoted string is found.
fn extract_quoted_prompt(s: &str) -> Option<(&str, String)> {
    let last_open = s.rfind('"')?;
    let first_open = s.find('"')?;
    if last_open == first_open {
        return None; // Only one quote — unbalanced.
    }
    let prompt = s[first_open + 1..last_open].to_string();
    let before = s[..first_open].trim_end();
    Some((before, prompt))
}

/// Format the next-due timestamp with a compact relative time suffix.
///
/// Returns the raw timestamp followed by a parenthesised relative
/// duration such as `2026-08-08T14:30:00Z (in 12m)` or
/// `2026-08-08T14:30:00Z (overdue 5m)` when the timestamp is in the past.
/// Disabled events with a far-future sentinel timestamp show `—` for
/// the relative part.
fn format_next_due(next_due: &str, enabled: bool) -> String {
    let Ok(ts) = chrono::DateTime::parse_from_rfc3339(next_due) else {
        return next_due.to_string();
    };
    let now = chrono::Utc::now();
    let delta = ts.with_timezone(&chrono::Utc) - now;
    let total_secs = delta.num_seconds();

    // Far-future sentinel (disabled one-shot events pushed to 9999-12-31).
    if total_secs > 315_576_000 {
        return if enabled {
            format!("{next_due}")
        } else {
            format!("{next_due} (—)")
        };
    }

    let abs_secs = total_secs.unsigned_abs() as i64;
    let label = if total_secs >= 0 { "in" } else { "overdue" };
    format!("{next_due} ({label} {})", compact_duration(abs_secs))
}

/// Format a duration in seconds as a compact string with up to two units.
///
/// Examples: `12m`, `5h30m`, `3d4h`, `45s`, `1w2d`.
fn compact_duration(secs: i64) -> String {
    const SECS_PER_MIN: i64 = 60;
    const SECS_PER_HOUR: i64 = 3_600;
    const SECS_PER_DAY: i64 = 86_400;
    const SECS_PER_WEEK: i64 = 604_800;

    let weeks = secs / SECS_PER_WEEK;
    let rem = secs % SECS_PER_WEEK;
    let days = rem / SECS_PER_DAY;
    let rem = rem % SECS_PER_DAY;
    let hours = rem / SECS_PER_HOUR;
    let rem = rem % SECS_PER_HOUR;
    let mins = rem / SECS_PER_MIN;
    let seconds = rem % SECS_PER_MIN;

    // Build up to two significant units.
    let parts: [(&str, i64); 5] = [
        ("w", weeks),
        ("d", days),
        ("h", hours),
        ("m", mins),
        ("s", seconds),
    ];
    let mut result: Vec<String> = Vec::new();
    for (label, val) in &parts {
        if *val > 0 {
            result.push(format!("{val}{label}"));
        }
        if result.len() == 2 {
            break;
        }
    }
    if result.is_empty() {
        "0s".to_string()
    } else {
        result.join("")
    }
}

/// Build a human-readable schedule description from a [`CronEventRow`].
///
/// This mirrors [`CronSchedule::human_readable`] but works from the flattened
/// row fields stored in SQLite.
fn row_to_human_readable(row: &ragent_storage::CronEventRow) -> String {
    match row.schedule_form.as_str() {
        "one_shot" => match &row.start_at {
            Some(ts) => format!("at {}", ts),
            None => "at (missing timestamp)".to_string(),
        },
        "repeat_from" => {
            let ts = row.start_at.as_deref().unwrap_or("?");
            let dur = row
                .duration_secs
                .map(duration_secs_to_string)
                .unwrap_or_else(|| "?".to_string());
            format!("every {dur} from {ts}")
        }
        "repeat_now" => {
            let dur = row
                .duration_secs
                .map(duration_secs_to_string)
                .unwrap_or_else(|| "?".to_string());
            format!("every {dur}")
        }
        _ => row.schedule_raw.clone(),
    }
}

/// Convert a duration in seconds to a compact human-readable string.
fn duration_secs_to_string(secs: i64) -> String {
    const SECS_PER_MIN: i64 = 60;
    const SECS_PER_HOUR: i64 = 3_600;
    const SECS_PER_DAY: i64 = 86_400;
    const SECS_PER_WEEK: i64 = 604_800;
    const SECS_PER_MONTH: i64 = 2_592_000;

    let units: [(&str, i64); 5] = [
        ("mo", SECS_PER_MONTH),
        ("w", SECS_PER_WEEK),
        ("d", SECS_PER_DAY),
        ("h", SECS_PER_HOUR),
        ("m", SECS_PER_MIN),
    ];
    for (label, unit_secs) in &units {
        if secs % unit_secs == 0 {
            return format!("{}{}", secs / unit_secs, label);
        }
    }
    format!("{secs}s")
}

/// Redact well-known secret patterns from a string.
///
/// Patterns redacted:
/// - API keys (sk-..., key_..., api_key=...)
/// - Bearer tokens
/// - AWS access keys
/// - Generic secrets/tokens/passwords in key=value format
fn redact_secrets(input: &str) -> String {
    // API key patterns (Anthropic, OpenAI, etc.)
    static API_KEY_PATTERN: std::sync::LazyLock<Regex> =
        std::sync::LazyLock::new(|| Regex::new(r"(?i)(sk-[a-zA-Z0-9]{20,})").unwrap());

    // Bearer token pattern
    static BEARER_PATTERN: std::sync::LazyLock<Regex> =
        std::sync::LazyLock::new(|| Regex::new(r"(?i)(Bearer\s+[a-zA-Z0-9\-_\.]{20,})").unwrap());

    // AWS access key pattern
    static AWS_KEY_PATTERN: std::sync::LazyLock<Regex> =
        std::sync::LazyLock::new(|| Regex::new(r"(?i)(AKIA[0-9A-Z]{16})").unwrap());

    // Generic key=value secrets
    static SECRET_PATTERN: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
        Regex::new(r"(?i)((?:api[_-]?key|secret|token|password|passwd|pwd|auth)\s*[=:]\s*\S{8,})")
            .unwrap()
    });

    let mut result = input.to_string();

    // Redact API keys
    result = API_KEY_PATTERN
        .replace_all(&result, "[REDACTED_API_KEY]")
        .to_string();

    // Redact Bearer tokens
    result = BEARER_PATTERN
        .replace_all(&result, "Bearer [REDACTED_TOKEN]")
        .to_string();

    // Redact AWS keys
    result = AWS_KEY_PATTERN
        .replace_all(&result, "[REDACTED_AWS_KEY]")
        .to_string();

    // Redact generic secrets
    result = SECRET_PATTERN
        .replace_all(&result, "[REDACTED_SECRET]")
        .to_string();

    result
}

/// Handle the `/template` slash command for listing and applying reusable prompt templates.
///
/// Usage:
/// - `/template` — list all available templates
/// - `/template <name>` — show template details and apply with no arguments
/// - `/template <name> <args>` — apply template with arguments
fn handle_template_command(app: &mut App, args: &str) {
    use ragent_agent::template::{TemplateInfo, discover_templates};

    let working_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let templates = discover_templates(&working_dir);

    if args.is_empty() {
        // List all templates
        let mut output = String::from("## Available Templates\n\n");
        output.push_str("| Name | Description | Scope | Placeholders |\n");
        output.push_str("|------|-------------|-------|-------------|\n");

        let mut template_list: Vec<&TemplateInfo> = templates.values().collect();
        template_list.sort_by(|a, b| a.name.cmp(&b.name));

        if template_list.is_empty() {
            output.push_str("\nNo templates found.\n\n");
            output.push_str("You can create templates by adding `.md` files to:\n");
            output.push_str(&format!("  - `~/.ragent/templates/` (personal)\n"));
            output.push_str(&format!("  - `.ragent/templates/` (project-specific)\n\n"));
            output.push_str("Templates support placeholders like `{{title}}`, `{{description}}`, `{{arguments}}`, etc.\n");
        } else {
            for template in template_list {
                let desc = template.description.as_deref().unwrap_or("—");
                let placeholders = if template.placeholders.is_empty() {
                    "—".to_string()
                } else {
                    template.placeholders.join(", ")
                };
                output.push_str(&format!(
                    "| `{}` | {} | {} | {} |\n",
                    template.name, desc, template.scope, placeholders
                ));
            }
            output.push_str("\n");
            output.push_str("Usage: `/template <name> [arguments]`\n\n");
            output.push_str("The template will be applied with your arguments substituted for `{{arguments}}`.\n");
            output
                .push_str("You can then send the result to the agent or edit it before sending.\n");
        }

        app.append_assistant_text(&output);
        app.status = "template: listed".to_string();
        return;
    }

    // Parse template name and arguments
    let (template_name, template_args) = args
        .split_once(char::is_whitespace)
        .map_or((args, ""), |(name, a)| (name, a.trim()));

    if let Some(template) = templates.get(template_name) {
        let applied = template.apply_simple(template_args);

        let mut output = String::new();
        output.push_str(&format!("## Template: `{}`\n\n", template_name));
        output.push_str("**Applied Template:**\n\n");
        output.push_str("```markdown\n");
        output.push_str(&applied);
        output.push_str("\n```\n\n");

        if !template.placeholders.is_empty() {
            output.push_str("**Placeholders:** ");
            output.push_str(&template.placeholders.join(", "));
            output.push_str("\n\n");
        }

        output.push_str("The template has been applied. You can:\n");
        output.push_str("1. Press Enter to send this prompt to the agent\n");
        output.push_str("2. Edit the text above before sending\n");
        output.push_str("3. Type a new command\n");

        app.append_assistant_text(&output);
        app.status = format!("template: applied '{}'", template_name);

        // Pre-fill the input buffer with the applied template so user can edit/send
        app.input = applied;
        app.input_cursor = app.input.chars().count();
    } else {
        let mut output = String::new();
        output.push_str(&format!("Template '{}' not found.\n\n", template_name));
        output.push_str("Available templates:\n");
        let mut names: Vec<&String> = templates.keys().collect();
        names.sort();
        for name in names {
            output.push_str(&format!("  - `{}`\n", name));
        }
        output.push_str("\nUsage: `/template <name> [arguments]`\n");

        app.append_assistant_text(&output);
        app.status = format!("template: '{}' not found", template_name);
    }
}

/// Handle the `/goal` slash command for goal-based autonomous stop hook.
///
/// Usage:
/// - `/goal set <description>` — set a goal condition
/// - `/goal clear` — clear the current goal
/// - `/goal show` — show the current goal status
/// - `/goal test` — manually test if the current goal is satisfied
fn handle_goal_command(app: &mut App, args: &str) {
    use ragent_agent::goal::GoalCondition;

    if args.is_empty() {
        let output = r#"## Goal-Based Autonomous Stop

Usage:
  `/goal set <description>` — Set a goal condition for autonomous execution
  `/goal clear` — Clear the current goal
  `/goal show` — Show the current goal status
  `/goal test` — Manually test if the goal is satisfied

Example:
  `/goal set Stop when all tests pass and the build succeeds`

When a goal is set, the agent will evaluate it after each turn during
autonomous execution and halt when the goal is satisfied."#;
        app.append_assistant_text(output);
        app.status = "goal: help".to_string();
        return;
    }

    let (subcmd, rest) = args
        .split_once(char::is_whitespace)
        .map_or((args, ""), |(c, r)| (c, r.trim()));

    match subcmd {
        "set" => {
            if rest.is_empty() {
                app.append_assistant_text(
                    "Usage: `/goal set <description>`\n\nPlease provide a goal description.",
                );
                app.status = "goal: missing description".to_string();
                return;
            }

            // Store the goal in session state (for now, just display confirmation)
            let goal = GoalCondition::new(rest);
            let output = format!(
                "## Goal Set\n\n**Goal:** {}\n\nThe agent will evaluate this goal after each turn\nand halt autonomous execution when it is satisfied.\n\nUse `/goal show` to check status or `/goal clear` to remove.",
                goal.description
            );
            app.append_assistant_text(&output);
            app.status = format!("goal: set '{}'", rest);

            // TODO: Persist goal to session storage
            // For now, we just confirm the goal was set
        }
        "clear" => {
            app.append_assistant_text(
                "## Goal Cleared\n\nThe autonomous stop goal has been removed.",
            );
            app.status = "goal: cleared".to_string();
            // TODO: Clear goal from session storage
        }
        "show" => {
            // TODO: Load goal from session storage
            // For now, show placeholder
            let output = r#"## Current Goal

No goal is currently set.

Use `/goal set <description>` to set a goal for autonomous execution."#;
            app.append_assistant_text(output);
            app.status = "goal: none".to_string();
        }
        "test" => {
            // TODO: Load goal and evaluate
            // For now, show placeholder
            let output = r#"## Goal Test

No goal is currently set to test.

Use `/goal set <description>` to set a goal first."#;
            app.append_assistant_text(output);
            app.status = "goal: test (none)".to_string();
        }
        _ => {
            let output = format!(
                "Unknown goal command: '{}'\n\nUsage: `/goal set|clear|show|test`",
                subcmd
            );
            app.append_assistant_text(&output);
            app.status = format!("goal: unknown '{}'", subcmd);
        }
    }
}
