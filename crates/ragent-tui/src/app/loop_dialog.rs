//! Interactive `/loop` setup dialog (spec `agentloop`, task T-014).
//!
//! Implements FR-002 (the interactive setup dialog: an agent selector
//! navigable with arrow keys, goal / verification / scope / constraints /
//! tool-set text fields, a step-limit and a cost-limit field, and a
//! checkpoints toggle) and FR-004 (Esc cancels without starting anything
//! and every entered value is preserved so re-opening the dialog restores
//! the draft).
//!
//! The dialog state lives in a single [`LoopSetupState`] slot on `App`
//! (`App::loop_setup`), following the same one-`Option`-slot pattern as the
//! provider-setup dialog. Confirming composes a [`LoopSpec`] and hands it
//! to [`App::start_goal_loop`], which registers the loop with the session
//! processor and sends the goal text through the normal message path so the
//! agent loop engages immediately.
//!
//! Dependencies: `crossterm` (key events), `ragent-agent` (`LoopSpec`,
//! agent resolution), `ragent-config` (`LoopConfig` defaults), and the
//! reusable [`InputField`](crate::input_field::InputField) widget.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use crossterm::event::{KeyCode, KeyEvent};

use ragent_agent::agent::AgentInfo;
use ragent_agent::session::loop_state::LoopSpec;

use ragent_config::LoopConfig;

use crate::app::state::App;
use crate::input_field::InputField;

/// The navigable fields of the loop setup dialog, in display order (FR-002).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopSetupField {
    /// Agent picker - arrow keys select the agent that drives the loop.
    Agent,
    /// Goal text - the success state the loop must reach (required, FR-005).
    Goal,
    /// Optional verification command whose exit status gates goal
    /// achievement (FR-007).
    VerifyCmd,
    /// Optional scope boundaries as comma-separated glob patterns (FR-022).
    Scope,
    /// Optional read-only constraints as comma-separated glob patterns
    /// (FR-021).
    ReadOnly,
    /// Optional restricted tool surface as comma-separated tool names
    /// (FR-008).
    ToolSet,
    /// Step budget; blank applies the config default (FR-013).
    MaxSteps,
    /// Token cost budget; blank applies the config default (FR-014).
    CostLimit,
    /// Destructive-action checkpoints toggle (FR-015).
    Checkpoints,
}

impl LoopSetupField {
    /// The field after `self`, wrapping to the first field.
    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Self::Agent => Self::Goal,
            Self::Goal => Self::VerifyCmd,
            Self::VerifyCmd => Self::Scope,
            Self::Scope => Self::ReadOnly,
            Self::ReadOnly => Self::ToolSet,
            Self::ToolSet => Self::MaxSteps,
            Self::MaxSteps => Self::CostLimit,
            Self::CostLimit => Self::Checkpoints,
            Self::Checkpoints => Self::Agent,
        }
    }

    /// The field before `self`, wrapping to the last field.
    #[must_use]
    pub fn previous(self) -> Self {
        match self {
            Self::Agent => Self::Checkpoints,
            Self::Goal => Self::Agent,
            Self::VerifyCmd => Self::Goal,
            Self::Scope => Self::VerifyCmd,
            Self::ReadOnly => Self::Scope,
            Self::ToolSet => Self::ReadOnly,
            Self::MaxSteps => Self::ToolSet,
            Self::CostLimit => Self::MaxSteps,
            Self::Checkpoints => Self::CostLimit,
        }
    }

    /// Whether the field accepts typed text (`false` for the agent picker
    /// and the checkpoints toggle, which are driven with arrow keys).
    #[must_use]
    pub fn is_text(self) -> bool {
        !matches!(self, Self::Agent | Self::Checkpoints)
    }
}

/// State of the interactive `/loop` setup dialog (FR-002).
#[derive(Debug, Clone)]
pub struct LoopSetupState {
    /// Loaded agents available as the loop driver: `(name, description)`.
    pub agents: Vec<(String, String)>,
    /// Index into `agents` of the currently selected loop agent.
    pub selected_agent: usize,
    /// The field the cursor currently sits on.
    pub active_field: LoopSetupField,
    /// Goal text (required).
    pub goal_field: InputField,
    /// Optional verification command.
    pub verify_cmd_field: InputField,
    /// Optional scope globs (comma-separated).
    pub scope_field: InputField,
    /// Optional read-only constraint globs (comma-separated).
    pub read_only_field: InputField,
    /// Optional restricted tool set (comma-separated).
    pub tool_set_field: InputField,
    /// Step budget; blank applies the config default.
    pub max_steps_field: InputField,
    /// Token cost budget; blank applies the config default.
    pub cost_limit_field: InputField,
    /// Whether destructive-action checkpoints are forced (FR-015).
    pub checkpoints: bool,
    /// Validation / start error shown inside the dialog (e.g. missing goal).
    pub error: Option<String>,
}

impl LoopSetupState {
    /// Create dialog state for `agents`, initially focused on the agent
    /// picker with `default_agent_index` highlighted.
    #[must_use]
    pub fn new(agents: Vec<(String, String)>, default_agent_index: usize) -> Self {
        let selected_agent = if agents.is_empty() {
            0
        } else {
            default_agent_index.min(agents.len() - 1)
        };
        Self {
            agents,
            selected_agent,
            active_field: LoopSetupField::Agent,
            goal_field: InputField::new(),
            verify_cmd_field: InputField::new(),
            scope_field: InputField::new(),
            read_only_field: InputField::new(),
            tool_set_field: InputField::new(),
            max_steps_field: InputField::new(),
            cost_limit_field: InputField::new(),
            checkpoints: true,
            error: None,
        }
    }

    /// The name of the currently selected loop agent (empty when no agents
    /// are loaded).
    #[must_use]
    pub fn selected_agent_name(&self) -> &str {
        self.agents
            .get(self.selected_agent)
            .map_or("", |(name, _)| name)
    }

    /// Move the agent selection down, wrapping past the last entry.
    pub fn select_next_agent(&mut self) {
        if self.agents.is_empty() {
            return;
        }
        self.selected_agent = (self.selected_agent + 1) % self.agents.len();
    }

    /// Move the agent selection up, wrapping past the first entry.
    pub fn select_previous_agent(&mut self) {
        if self.agents.is_empty() {
            return;
        }
        self.selected_agent = if self.selected_agent == 0 {
            self.agents.len() - 1
        } else {
            self.selected_agent - 1
        };
    }

    /// The text field for `field`, when it is a text field.
    #[must_use]
    pub fn field_ref(&self, field: LoopSetupField) -> Option<&InputField> {
        match field {
            LoopSetupField::Goal => Some(&self.goal_field),
            LoopSetupField::VerifyCmd => Some(&self.verify_cmd_field),
            LoopSetupField::Scope => Some(&self.scope_field),
            LoopSetupField::ReadOnly => Some(&self.read_only_field),
            LoopSetupField::ToolSet => Some(&self.tool_set_field),
            LoopSetupField::MaxSteps => Some(&self.max_steps_field),
            LoopSetupField::CostLimit => Some(&self.cost_limit_field),
            LoopSetupField::Agent | LoopSetupField::Checkpoints => None,
        }
    }

    /// The mutable text field for `field`, when it is a text field.
    pub fn field_mut(&mut self, field: LoopSetupField) -> Option<&mut InputField> {
        match field {
            LoopSetupField::Goal => Some(&mut self.goal_field),
            LoopSetupField::VerifyCmd => Some(&mut self.verify_cmd_field),
            LoopSetupField::Scope => Some(&mut self.scope_field),
            LoopSetupField::ReadOnly => Some(&mut self.read_only_field),
            LoopSetupField::ToolSet => Some(&mut self.tool_set_field),
            LoopSetupField::MaxSteps => Some(&mut self.max_steps_field),
            LoopSetupField::CostLimit => Some(&mut self.cost_limit_field),
            LoopSetupField::Agent | LoopSetupField::Checkpoints => None,
        }
    }
}

/// Split a comma-separated free-text field into trimmed, de-duplicated
/// tokens (order preserved). Empty tokens are dropped.
#[must_use]
pub fn parse_comma_list(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for token in text.split(',') {
        let token = token.trim();
        if !token.is_empty() && !out.iter().any(|existing| existing == token) {
            out.push(token.to_string());
        }
    }
    out
}

/// Parse an optional numeric limit. Blank or non-numeric input yields
/// `None` so the caller can apply its documented default (FR-013, FR-014).
#[must_use]
pub fn parse_optional_u64(text: &str) -> Option<u64> {
    text.trim().parse::<u64>().ok()
}

/// Compose a [`LoopSpec`] from the dialog's entered values.
///
/// Blank limit fields apply the supplied `defaults` (the effective
/// `loop` config section), and list fields are parsed as comma-separated
/// globs. Unparsable numeric input is treated as blank.
///
/// # Errors
///
/// Returns an error naming the missing field when the goal is empty or
/// whitespace-only (FR-005) - a loop cannot start without a goal.
#[must_use]
pub fn build_spec_from_state(
    state: &LoopSetupState,
    defaults: &LoopConfig,
) -> Result<LoopSpec, String> {
    let goal = state.goal_field.text().trim();
    if goal.is_empty() {
        return Err("missing field: goal - a loop cannot start without a goal".to_string());
    }
    let mut spec = LoopSpec::new(state.selected_agent_name(), goal);
    let verify_cmd = state.verify_cmd_field.text().trim();
    if !verify_cmd.is_empty() {
        spec.verify_cmd = Some(verify_cmd.to_string());
    }
    spec.scope = parse_comma_list(state.scope_field.text());
    spec.read_only = parse_comma_list(state.read_only_field.text());
    spec.tool_set = parse_comma_list(state.tool_set_field.text());
    spec.max_steps = Some(
        parse_optional_u64(state.max_steps_field.text()).map_or(defaults.max_steps, |steps| {
            u32::try_from(steps).unwrap_or(defaults.max_steps)
        }),
    );
    spec.cost_limit = parse_optional_u64(state.cost_limit_field.text()).or(defaults.cost_limit);
    spec.checkpoints = state.checkpoints;
    Ok(spec)
}

/// One-shot `/loop` command-line overrides (FR-003 one-shot flags).
///
/// Each field is `Some` only when the corresponding flag was given on the
/// command line; unset fields keep the config/dialog defaults.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LoopOverrides {
    /// `--max-steps N`: override the step budget (FR-013).
    pub max_steps: Option<u32>,
    /// `--cost_limit N` (or `--cost-limit N`): override the token-cost
    /// budget (FR-014).
    pub cost_limit: Option<u64>,
    /// `--timeout N`: override the checkpoint-prompt timeout in seconds
    /// (FR-015).
    pub timeout: Option<u32>,
}

/// Parse one-shot `/loop` flags out of a whitespace-split token list.
///
/// Recognised flags (with ` ` or `=` value forms):
/// - `--max-steps <n>` — step-budget override
/// - `--cost_limit <n>` / `--cost-limit <n>` — token-cost override
/// - `--timeout <n>` — checkpoint-prompt timeout override (seconds)
///
/// Flag tokens (and their values) are removed from the token list; the
/// remaining tokens keep their order so the caller can take the first as
/// the agent and the rest as the goal.
///
/// # Errors
///
/// Returns an error naming the offending flag when a flag value is missing
/// or not a non-negative integer.
pub fn parse_loop_flags(tokens: &[String]) -> Result<(Vec<String>, LoopOverrides), String> {
    let mut remaining = Vec::with_capacity(tokens.len());
    let mut overrides = LoopOverrides::default();
    let mut i = 0;
    while i < tokens.len() {
        let token = tokens[i].as_str();
        let (name, inline_value) = match token.split_once('=') {
            Some((n, v)) => (n, Some(v.to_string())),
            None => (token, None),
        };
        let flag = name.trim_start_matches('-').replace('_', "-");
        let value = match (flag.as_str(), inline_value) {
            ("max-steps" | "cost-limit" | "timeout", Some(v)) => v,
            ("max-steps" | "cost-limit" | "timeout", None) => {
                let Some(v) = tokens.get(i + 1) else {
                    return Err(format!("missing value for --{flag}"));
                };
                i += 1;
                v.clone()
            }
            _ => {
                remaining.push(tokens[i].clone());
                i += 1;
                continue;
            }
        };
        let parsed: Result<u64, _> = value.parse();
        match (flag.as_str(), parsed) {
            ("max-steps", Ok(v)) => {
                overrides.max_steps =
                    Some(u32::try_from(v).map_err(|_| {
                        format!("invalid --max-steps value: {v} (exceeds u32 range)")
                    })?);
            }
            ("cost-limit", Ok(v)) => {
                overrides.cost_limit = Some(v);
            }
            ("timeout", Ok(v)) => {
                overrides.timeout =
                    Some(u32::try_from(v).map_err(|_| {
                        format!("invalid --timeout value: {v} (exceeds u32 seconds)")
                    })?);
            }
            (flag, _) => return Err(format!("invalid value for --{flag}: {value}")),
        }
        i += 1;
    }
    Ok((remaining, overrides))
}

/// Apply [`LoopOverrides`] to a freshly built [`LoopSpec`].
///
/// Only flags the user actually passed are applied; unset overrides leave
/// the spec's own defaults (config fallbacks) untouched.
pub fn apply_loop_overrides(spec: &mut LoopSpec, overrides: &LoopOverrides) {
    if let Some(max_steps) = overrides.max_steps {
        spec.max_steps = Some(max_steps);
    }
    if let Some(cost_limit) = overrides.cost_limit {
        spec.cost_limit = Some(cost_limit);
    }
    if let Some(timeout) = overrides.timeout {
        spec.checkpoint_timeout_secs = Some(timeout);
    }
}

/// Open (or re-open) the interactive loop setup dialog (FR-002, FR-004).
///
/// When the dialog was previously cancelled with `Esc`, the draft values are
/// restored so nothing the user typed is lost (FR-004). A fresh dialog is
/// pre-filled with the config's loop defaults (step limit, cost limit) and
/// the currently selected agent.
pub fn open_loop_setup(app: &mut App) {
    if let Some(draft) = app.loop_setup_draft.take() {
        // FR-004: re-opening after Esc restores every entered value.
        app.loop_setup = Some(draft);
        app.status = "loop: setup restored".to_string();
        return;
    }
    let agents: Vec<(String, String)> = app
        .cycleable_agents
        .iter()
        .map(|a| (a.name.clone(), a.description.clone()))
        .collect();
    let defaults = loop_config_defaults();
    let mut state = LoopSetupState::new(agents, app.current_agent_index);
    state.max_steps_field = InputField::with_text(defaults.max_steps.to_string());
    if let Some(cost_limit) = defaults.cost_limit {
        state.cost_limit_field = InputField::with_text(cost_limit.to_string());
    }
    app.loop_setup = Some(state);
    app.status = "loop: set a goal to start".to_string();
}

/// Handle a key event while the loop setup dialog is open (FR-002, FR-004).
///
/// Key behaviour:
/// - `Esc` closes the dialog WITHOUT starting a loop and preserves every
///   entered value in `App::loop_setup_draft` for re-open (FR-004).
/// - `Tab` / `BackTab` cycle the active field.
/// - Arrow keys navigate the agent picker and toggle the checkpoints
///   switch; on text fields they move to the previous / next field.
/// - `Space` toggles the checkpoints switch when it is focused.
/// - `Enter` validates and starts the loop; a missing goal returns to the
///   dialog with an error naming the field (FR-005).
pub fn handle_loop_setup_key(app: &mut App, key: KeyEvent) {
    if key.code == KeyCode::Esc {
        // FR-004: cancel without starting; keep the entered values.
        app.loop_setup_draft = app.loop_setup.take();
        app.status = "loop: setup cancelled (values kept)".to_string();
        return;
    }
    let Some(mut state) = app.loop_setup.take() else {
        return;
    };
    let is_text = state.active_field.is_text();
    match key.code {
        KeyCode::Enter => {
            let defaults = loop_config_defaults();
            match build_spec_from_state(&state, &defaults) {
                Ok(spec) => {
                    app.loop_setup_draft = None;
                    app.start_goal_loop(spec);
                }
                Err(missing) => {
                    state.error = Some(missing);
                    app.loop_setup = Some(state);
                }
            }
            return;
        }
        KeyCode::Tab => {
            state.active_field = state.active_field.next();
        }
        KeyCode::BackTab => {
            state.active_field = state.active_field.previous();
        }
        KeyCode::Up => match state.active_field {
            LoopSetupField::Agent => state.select_previous_agent(),
            LoopSetupField::Checkpoints => state.checkpoints = !state.checkpoints,
            field => state.active_field = field.previous(),
        },
        KeyCode::Down => match state.active_field {
            LoopSetupField::Agent => state.select_next_agent(),
            LoopSetupField::Checkpoints => state.checkpoints = !state.checkpoints,
            field => state.active_field = field.next(),
        },
        KeyCode::Char(' ') if state.active_field == LoopSetupField::Checkpoints => {
            state.checkpoints = !state.checkpoints;
        }
        _ => {
            if is_text && let Some(text_field) = state.field_mut(state.active_field) {
                text_field.handle_key(key);
                state.error = None;
            }
        }
    }
    app.loop_setup = Some(state);
}

/// Show the `/loop help` usage block (FR-001).
///
/// Documents every loop command form, dialog field and key binding, the
/// one-shot defaults, the stop conditions, the interrupt path, the
/// rollback offer, and the `loop` config section knobs.
pub fn show_loop_help(app: &mut App) {
    app.append_assistant_text(
        r"From: /loop help

## /loop - Goal-driven agent loops

A loop runs an agent towards a stated goal until a stop condition fires:
`completed` (goal achieved and any verification command passed), `error`
(unrecoverable failure or the retry allowance ran out), `budget_exhausted`
(step or token budget spent), or `interrupted` (you pressed Esc/Ctrl+X or
denied a forced checkpoint).

### Commands

| Form | Description |
|---|---|
| `/loop` | Open the interactive setup dialog |
| `/loop <agent> <goal text...>` | Start immediately with defaults |
| `/loop <agent> --max-steps N --cost_limit N --timeout N <goal...>` | Start with one-shot budget overrides |
| `/loop help` | Show this help |

### One-shot flags

| Flag | Purpose |
|---|---|
| `--max-steps N` | Override the step budget for this run |
| `--cost_limit N` (or `--cost-limit N`) | Override the token-cost budget for this run |
| `--timeout N` | Override the checkpoint-prompt timeout (seconds) for this run |

Flags accept `--flag value` or `--flag=value` and may appear anywhere in the
command; they apply to that one run only.

### One-shot defaults

`/loop <agent> <goal>` starts with the config step limit (default 512), the
config cost limit (none by default), checkpoints on, and no verification
command, scope, read-only or tool-set restriction.

### Setup dialog fields

| Field | Purpose |
|---|---|
| Agent | Agent preset driving the loop (arrow keys to select) |
| Goal | The success state the loop must reach (required) |
| Verify cmd | Command whose exit status must pass for the goal to count as achieved (e.g. `cargo test -q`) |
| Scope | Comma-separated glob patterns; file ops outside them are denied |
| Read-only | Comma-separated glob patterns; writes to matching paths are denied (protect tests, docs, ...) |
| Tool set | Comma-separated tool names; only these tools may be called |
| Max steps | Iteration budget; blank applies the config default (512) |
| Cost limit | Token budget; blank applies the config default (none) |
| Checkpoints | Force a snapshot checkpoint before destructive tool calls |

### Dialog keys

| Key | Action |
|---|---|
| `Tab` / `Shift+Tab` | Next / previous field |
| `Up` / `Down` | Move agent selection, toggle checkpoints, or change field |
| `Space` | Toggle the checkpoints switch (when focused) |
| `Enter` | Validate and start the loop (a missing goal re-opens with an error) |
| `Esc` | Cancel without starting; entered values are kept for re-open |

### During a run

| Action | Effect |
|---|---|
| `Esc` / `Ctrl+X` | Request an interrupt; the loop stops at the next inter-stage safe point with status `interrupted` |
| Checkpoint prompt | Appears before destructive tool calls; timeout counts as denial |
| After termination | Enter rolls back to the pre-loop snapshot, Esc keeps the changes |

### Config (ragent.json)

| Key | Default | Purpose |
|---|---|---|
| `loop.max_steps` | 512 | Default iteration budget |
| `loop.cost_limit` | none | Default token-cost budget |
| `loop.error_retry_allowance` | 3 | Consecutive recoverable failures tolerated |
| `loop.checkpoints` | true | Force checkpoints before destructive tool calls |
| `loop.checkpoint_timeout_secs` | 120 | Seconds before a checkpoint prompt counts as denial |

See docs/howtos/loopprogramming.md for worked examples.",
    );
    app.status = "loop: help".to_string();
}

/// Resolve the configured `loop` section once, falling back to defaults when
/// no config can be loaded. The dialog open → confirm → dispatch sequence
/// previously re-loaded the config at each step; the M-025 cache makes each
/// call cheap, but `build_spec_from_state` still needs the same value the
/// dialog was seeded with, so both read through this one helper.
fn loop_config_defaults() -> LoopConfig {
    ragent_agent::Config::load().map_or_else(|_| LoopConfig::default(), |config| config.r#loop)
}

impl App {
    /// Start a goal-driven loop for the current session (FR-002 confirm
    /// path).
    ///
    /// Mirrors the user-message UI bookkeeping of `dispatch_user_message`
    /// (message push, status, log line, cancel flag) and then - inside ONE
    /// spawned task - registers the loop spec with the session processor
    /// and sends the goal text through the normal message path, so the loop
    /// tracker exists before the first iteration begins.
    pub(crate) fn start_goal_loop(&mut self, spec: LoopSpec) {
        if self.configured_provider.is_none() {
            self.status = "loop: no provider configured - use /provider first".to_string();
            return;
        }
        if self.selected_model.is_none() {
            self.status = "loop: no model selected - use /model first".to_string();
            return;
        }
        let Some(sid) = self.session_id.clone() else {
            self.status = "[warn] No active session".to_string();
            return;
        };

        // User-message bookkeeping (mirrors dispatch_user_message).
        let goal_text = spec.goal.clone();
        let display_text = format!("[loop {}] {}", spec.agent, goal_text);
        let msg = ragent_agent::message::Message::user_text(&sid, display_text.clone());
        self.messages.push(msg);
        self.schedule_context_snapshot_refresh();
        self.add_to_history(goal_text.clone());
        self.input.clear();
        self.input_cursor = 0;
        self.file_menu = None;
        self.set_status_working("loop running");
        self.stream_in_bytes = 0;
        self.stream_out_bytes = 0;
        self.trim_messages_if_needed();
        let truncated = crate::app::helpers::truncate_to_char_boundary(&goal_text, 120);
        self.push_log_no_agent(
            crate::app::state::LogLevel::Info,
            format!("loop started [{}]: {}", spec.agent, truncated),
        );

        let agent = self.loop_dispatch_agent(&spec);
        let processor = self.session_processor.clone();
        let flag = Arc::new(AtomicBool::new(false));
        self.cancel_flag = Some(flag.clone());
        tokio::spawn(async move {
            processor.start_loop(&sid, spec).await;
            if let Err(e) = processor
                .process_message(&sid, &goal_text, &agent, flag)
                .await
            {
                // Escalate above debug: the user already saw "loop running",
                // so a silent swallow here would leave the loop apparently
                // running while the goal message never processed.
                tracing::error!(error = %e, "Failed to process loop goal message");
            }
        });
    }

    /// Resolve the [`AgentInfo`] that will drive the loop.
    ///
    /// The spec's agent preset wins; an unknown preset name falls back to
    /// the currently selected agent so a typo never silently changes the
    /// driver.
    fn loop_dispatch_agent(&self, spec: &LoopSpec) -> AgentInfo {
        let config = self.current_config();
        if let Ok(agent) = ragent_agent::agent::resolve_agent(&spec.agent, &config) {
            let mut agent = Arc::unwrap_or_clone(agent);
            self.apply_selected_model_and_thinking(&mut agent);
            return agent;
        }
        self.prepare_agent_for_dispatch()
    }
}

#[cfg(test)]
mod loop_dialog_tests;
