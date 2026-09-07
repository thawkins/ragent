//! Loop state for goal-driven agentic runs.
//!
//! This module carries the specification of a goal-driven loop run
//! ([`LoopSpec`]) and the runtime tracker that enforces the loop's stop
//! conditions ([`LoopTracker`]). It formalises the loop contract described in
//! `specs/agentloop/SPEC.md` (FR-006, FR-013, FR-014): a loop is started with
//! a structured goal — an agent, a goal text, an optional verification
//! command, scope boundaries, read-only constraints, a tool set, and budget
//! limits — and iterates plan-act-observe until a stop condition fires.
//!
//! # Usage (`/loop`)
//!
//! ```text
//! /loop                              # interactive setup dialog
//! /loop <agent> <goal text...>       # start immediately with documented defaults
//! ```
//!
//! # Goal format
//!
//! | Field | Meaning |
//! | ----- | ------- |
//! | Success state | what must be true for the loop to complete |
//! | Verification command | runs when the model responds without tool calls; gates completion (FR-007) |
//! | Scope boundaries | path globs file operations must stay inside |
//! | Constraints | read-only path globs; writes denied as observations (FR-021) |
//! | Tool set | restricted tool surface; out-of-set calls denied (FR-008, FR-009) |
//! | Budget | max iterations (`max_steps`) and accumulated tokens (`cost_limit`) |
//!
//! # Stop conditions ([`StopCondition`])
//!
//! - `GoalAchieved`       — the model signalled completion (verification passed).
//! - `UnrecoverableError` — a stage failed unrecoverably; no retry.
//! - `BudgetExhausted`    — the step or token budget was consumed.
//! - `HumanIntervention`  — the user interrupted or denied a checkpoint.
//!
//! Dependencies: `serde` (serialisable loop specs), `globset` (already a
//! dependency of this crate for permission-rule matching; no new
//! dependencies are introduced).

use globset::{Glob, GlobMatcher};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A goal-driven agentic loop specification.
///
/// Captured when a loop is started (via the `/loop` command or HTTP
/// `POST /loop`) and consulted at every iteration. The fields mirror the
/// structured goal format of the specification: success state
/// ([`LoopSpec::goal`] plus the optional [`LoopSpec::verify_cmd`]), scope
/// boundaries ([`LoopSpec::scope`]), constraints
/// ([`LoopSpec::read_only`]), the restricted tool surface
/// ([`LoopSpec::tool_set`]), and the budget knobs
/// ([`LoopSpec::max_steps`], [`LoopSpec::cost_limit`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoopSpec {
    /// The agent preset driving the loop (e.g. `coder`, `general`).
    pub agent: String,
    /// The goal text: the success state the loop must reach.
    pub goal: String,
    /// Optional verification command whose exit status gates goal
    /// achievement (FR-007). `None` means a no-tool-call response completes
    /// the loop directly.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verify_cmd: Option<String>,
    /// Scope boundaries as glob patterns (FR-022). File operations outside
    /// these patterns are out of scope. An empty list means the working
    /// directory (no extra restriction).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scope: Vec<String>,
    /// Read-only constraints as glob patterns (FR-021). Writes to matching
    /// paths are denied so the agent cannot satisfy the goal by modifying
    /// protected files such as tests.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub read_only: Vec<String>,
    /// Restricted tool surface (FR-008). An empty list means all tools.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_set: Vec<String>,
    /// Step budget: maximum number of iterations (FR-013). `None` means the
    /// agent/config default applies.
    pub max_steps: Option<u32>,
    /// Cost budget: maximum accumulated tokens across the run (FR-014).
    /// `None` means no token-cost gate.
    pub cost_limit: Option<u64>,
    /// Whether destructive-action checkpoints are forced (FR-015).
    pub checkpoints: bool,
    /// Checkpoint-prompt timeout override in seconds (FR-015). `None` means
    /// the `loop.checkpoint_timeout_secs` config default applies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkpoint_timeout_secs: Option<u32>,
}

impl LoopSpec {
    /// Create a minimal spec: the named agent pursuing `goal` with default
    /// budgets and checkpoints on.
    pub fn new(agent: impl Into<String>, goal: impl Into<String>) -> Self {
        Self {
            agent: agent.into(),
            goal: goal.into(),
            verify_cmd: None,
            scope: Vec::new(),
            read_only: Vec::new(),
            tool_set: Vec::new(),
            max_steps: None,
            cost_limit: None,
            checkpoints: true,
            checkpoint_timeout_secs: None,
        }
    }

    /// Whether the spec restricts file access beyond the working directory.
    pub fn has_scope(&self) -> bool {
        !self.scope.is_empty()
    }

    /// Whether the spec restricts the tool surface.
    pub fn has_tool_set(&self) -> bool {
        !self.tool_set.is_empty()
    }
}

impl Default for LoopSpec {
    fn default() -> Self {
        Self::new("general", String::new())
    }
}

/// Why a loop run terminated.
///
/// One variant per specification stop condition; the string form is the
/// termination status published on the event bus (FR-010 - FR-013 of the
/// spec's status model).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StopCondition {
    /// The model signalled completion and any verification command passed.
    GoalAchieved,
    /// A loop stage failed unrecoverably; the run stopped without retrying.
    UnrecoverableError,
    /// The step budget or token cost budget was exhausted.
    BudgetExhausted,
    /// The user interrupted the run or denied a forced checkpoint.
    HumanIntervention,
}

impl StopCondition {
    /// Stable serialisable label for this stop condition (`completed`,
    /// `error`, `budget_exhausted`, `interrupted`).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::GoalAchieved => "completed",
            Self::UnrecoverableError => "error",
            Self::BudgetExhausted => "budget_exhausted",
            Self::HumanIntervention => "interrupted",
        }
    }

    /// Parse a termination status label back into a stop condition.
    pub fn from_str_label(label: &str) -> Option<Self> {
        match label {
            "completed" => Some(Self::GoalAchieved),
            "error" => Some(Self::UnrecoverableError),
            "budget_exhausted" => Some(Self::BudgetExhausted),
            "interrupted" => Some(Self::HumanIntervention),
            _ => None,
        }
    }

    /// Whether this stop condition counts as a successful run.
    pub fn is_success(self) -> bool {
        matches!(self, Self::GoalAchieved)
    }
}

/// Runtime tracker for one loop run: enforces the budget gates
/// (FR-013, FR-014) and carries the loop's terminal stop condition.
///
/// The tracker is the single source of truth for "has the loop stopped?" —
/// once [`LoopTracker::stop`] is set, no further stage may run (FR-017:
/// no iteration after a stop condition).
#[derive(Debug, Clone)]
pub struct LoopTracker {
    /// Number of completed iterations.
    steps: u32,
    /// Accumulated token usage (input + output) across the run.
    tokens: u64,
    /// Consecutive recoverable-failure counter (FR-012 of the spec's error
    /// model); reset on every successful observation.
    consecutive_failures: u32,
    /// Per-iteration tool-call tally (FR-025): the total number of tool
    /// calls started across the run. Incremented per iteration by the
    /// session processor and published with the loop telemetry on
    /// termination.
    tool_calls: u64,
    /// Set when a stop condition fired; no stage may run afterwards.
    stopped: Option<StopCondition>,
    /// The budget limits this tracker enforces.
    max_steps: Option<u32>,
    cost_limit: Option<u64>,
}

impl LoopTracker {
    /// Create a tracker for a run bounded by `spec`'s budgets.
    pub fn new(spec: &LoopSpec) -> Self {
        Self {
            steps: 0,
            tokens: 0,
            consecutive_failures: 0,
            tool_calls: 0,
            stopped: None,
            max_steps: spec.max_steps,
            cost_limit: spec.cost_limit,
        }
    }

    /// Create a tracker with explicit budgets (test helper).
    pub fn with_budgets(max_steps: Option<u32>, cost_limit: Option<u64>) -> Self {
        Self {
            steps: 0,
            tokens: 0,
            consecutive_failures: 0,
            tool_calls: 0,
            stopped: None,
            max_steps,
            cost_limit,
        }
    }

    /// The number of completed iterations.
    pub fn steps(&self) -> u32 {
        self.steps
    }

    /// The accumulated token usage.
    pub fn tokens(&self) -> u64 {
        self.tokens
    }

    /// The total tool-call count across the run (FR-025).
    pub fn tool_calls(&self) -> u64 {
        self.tool_calls
    }

    /// The consecutive recoverable-failure counter.
    pub fn consecutive_failures(&self) -> u32 {
        self.consecutive_failures
    }

    /// The configured step budget (`None` = unbounded).
    ///
    /// Read access for the loop's budget-exhaustion reason message
    /// (spec `agentloop` T-008 / FR-013).
    pub fn max_steps(&self) -> Option<u32> {
        self.max_steps
    }

    /// The configured token cost budget (`None` = no gate).
    ///
    /// Read access for the loop's budget-exhaustion reason message
    /// (spec `agentloop` T-008 / FR-014).
    pub fn cost_limit(&self) -> Option<u64> {
        self.cost_limit
    }

    /// The stop condition if the loop has terminated, else `None`.
    pub fn stop_condition(&self) -> Option<StopCondition> {
        self.stopped
    }

    /// Whether the loop has terminated (no stage may run afterwards).
    pub fn is_stopped(&self) -> bool {
        self.stopped.is_some()
    }

    /// Budget gate (FR-013, FR-014): evaluated BEFORE sending an LLM
    /// request. Returns the breaching stop condition when the step count has
    /// reached `max_steps` or the token tally has reached `cost_limit`.
    pub fn budget_breach(&self) -> Option<StopCondition> {
        if let Some(max) = self.max_steps {
            if self.steps >= max {
                return Some(StopCondition::BudgetExhausted);
            }
        }
        if let Some(limit) = self.cost_limit {
            if self.tokens >= limit {
                return Some(StopCondition::BudgetExhausted);
            }
        }
        None
    }

    /// Record the start of an iteration: increments the step counter.
    ///
    /// Returns `false` (and records nothing) when the loop is already
    /// stopped or the budget would be breached by starting another step.
    pub fn begin_step(&mut self) -> bool {
        if self.is_stopped() || self.budget_breach().is_some() {
            return false;
        }
        self.steps += 1;
        true
    }

    /// Record token usage observed for the last LLM exchange (FR-014):
    /// adds `input_tokens + output_tokens` to the tally.
    pub fn record_tokens(&mut self, input_tokens: u64, output_tokens: u64) {
        self.tokens = self
            .tokens
            .saturating_add(input_tokens.saturating_add(output_tokens));
    }

    /// Record the tool calls started during an iteration (FR-025): adds
    /// `count` to the per-run tool-call tally. Does not affect the step
    /// counter — call [`LoopTracker::begin_step`] for that.
    pub fn record_tool_calls(&mut self, count: u64) {
        self.tool_calls = self.tool_calls.saturating_add(count);
    }

    /// Record a successful observation: resets the consecutive-failure
    /// counter.
    pub fn record_success(&mut self) {
        self.consecutive_failures = 0;
    }

    /// Record a recoverable failure; returns `true` when the consecutive
    /// count now exceeds the retry allowance (3), signalling the loop should
    /// stop with [`StopCondition::UnrecoverableError`].
    pub fn record_failure(&mut self) -> bool {
        self.consecutive_failures = self.consecutive_failures.saturating_add(1);
        self.consecutive_failures > 3
    }

    /// Stop the loop with `condition`. Idempotent: the first condition wins;
    /// later calls are ignored (FR-017). Returns the effective condition.
    pub fn stop(&mut self, condition: StopCondition) -> StopCondition {
        if let Some(existing) = self.stopped {
            return existing;
        }
        self.stopped = Some(condition);
        condition
    }
}

/// Compile a glob pattern list into matchers; invalid patterns are skipped
/// (they cannot match anything).
fn compile_globs(patterns: &[String]) -> Vec<GlobMatcher> {
    patterns
        .iter()
        .filter_map(|p| Glob::new(p).ok().map(|g| g.compile_matcher()))
        .collect()
}

/// Restriction checks for a [`LoopSpec`] against concrete tool invocations.
///
/// These helpers are the shared predicates the permission layer will consult
/// in later tasks (T-009): tool-set restriction (FR-008/FR-009), scope
/// boundaries (FR-022), and read-only constraints (FR-021).
impl LoopSpec {
    /// Whether `tool_name` is allowed under this spec's tool set
    /// (FR-008). `true` when no tool set is configured.
    pub fn allows_tool(&self, tool_name: &str) -> bool {
        !self.has_tool_set() || self.tool_set.iter().any(|t| t == tool_name)
    }

    /// Whether `path` (as a string, relative or absolute) is inside this
    /// spec's scope boundaries (FR-022). `true` when no scope is configured.
    pub fn path_in_scope(&self, path: &str) -> bool {
        if !self.has_scope() {
            return true;
        }
        let matchers = compile_globs(&self.scope);
        matchers.iter().any(|m| m.is_match(path))
    }

    /// Whether `path` is protected by a read-only constraint (FR-021).
    pub fn path_is_read_only(&self, path: &str) -> bool {
        let matchers = compile_globs(&self.read_only);
        matchers.iter().any(|m| m.is_match(path))
    }
}
/// Tools that mutate the workspace or protected state (FR-021).
///
/// Includes canonical tool names and their legacy aliases. `bash` is
/// special-cased: its sub-commands are scanned for write-ish commands instead
/// of matching the tool name.
pub const LOOP_WRITE_TOOLS: &[&str] = &[
    "write",
    "create",
    "edit",
    "multi_edit",
    "multiedit",
    "patch",
    "apply_patch",
    "append_to_file",
    "update_file",
    "write_file",
    "rm",
    "move_file",
    "copy_file",
    "make_directory",
    "memory_store",
    "memory_forget",
    "memory_replace",
    "memory_write",
    "office_write",
    "libre_write",
    "pdf_write",
];

/// Bash commands treated as filesystem writes for read-only-constraint
/// checks (best-effort, matching the permission layer's best-effort bash
/// splitting). `sed` is handled separately (in-place flag); `git` and
/// package managers are left to the checkpoint layer.
const BASH_WRITE_COMMANDS: &[&str] = &[
    "rm", "rmdir", "mv", "cp", "ln", "mkdir", "touch", "truncate", "shred", "dd", "tee", "chmod",
    "chown", "chattr", "install",
];

/// Mandatory safety tools that stay available even when a loop tool set is
/// configured (FR-008): control/introspection plus the memory tools.
pub const LOOP_ALWAYS_ALLOWED_TOOLS: &[&str] = &[
    "think",
    "ask_user",
    "agent_complete",
    "model_info",
    "memory_store",
    "memory_recall",
    "memory_forget",
    "conversation_search",
    "session_search",
];

/// Parameter names that carry a filesystem path in tool input JSON.
const PATH_PARAM_NAMES: &[&str] = &[
    "path",
    "file_path",
    "source",
    "destination",
    "target",
    "dir",
    "directory",
    "paths",
    "file",
];

/// Extract candidate filesystem paths from tool input JSON for scope /
/// read-only checks.
///
/// Top-level path-named parameters (`path`, `file_path`, `source`, ...) and
/// every object nested under an `edits`-style array contribute their
/// path-named values, so `multi_edit`-style batch inputs are checked too.
fn candidate_paths(input: &Value) -> Vec<String> {
    let mut found = Vec::new();
    let collect_value = |value: &Value, out: &mut Vec<String>| match value {
        Value::String(s) => out.push(s.clone()),
        Value::Array(items) => {
            for item in items {
                if let Value::String(s) = item {
                    out.push(s.clone());
                }
            }
        }
        _ => {}
    };
    for name in PATH_PARAM_NAMES {
        if let Some(value) = input.get(*name) {
            collect_value(value, &mut found);
        }
    }
    // Batch-edit inputs: collect path-named parameters from each edit object
    // (e.g. `multi_edit` / `multiedit` `{"edits": [{"file_path": ...}]}`).
    if let Some(edits) = input.get("edits").and_then(Value::as_array) {
        for edit in edits {
            if let Value::Object(map) = edit {
                for name in PATH_PARAM_NAMES {
                    if let Some(value) = map.get(*name) {
                        collect_value(value, &mut found);
                    }
                }
            }
        }
    }
    found
}

/// Extract path-looking tokens from a bash command string for scope /
/// read-only checks. Only tokens containing a `/` (or `~`) are considered
/// paths; flags (`-rf`, `--force`) and plain words are skipped.
fn bash_path_tokens(command: &str) -> Vec<String> {
    command
        .split_whitespace()
        .filter(|token| token.contains('/') || token.starts_with('~'))
        .map(|token| {
            token
                .trim_matches(|c: char| c == '"' || c == '\'' || c == ';')
                .to_string()
        })
        .filter(|token| !token.is_empty())
        .collect()
}

/// Sub-commands of a bash command string (best-effort split on
/// `&&`/`||`/`;`, mirroring the permission layer's approach), trimmed.
fn bash_sub_commands(command: &str) -> Vec<String> {
    command
        .split(['&', '|', ';'])
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect()
}

/// Whether one bash sub-command mutates the filesystem.
///
/// Best-effort heuristic: file-mutating command names count unconditionally;
/// `sed` counts when an in-place flag (`-i`) is present; output redirection
/// (`>`, `>>`) counts. Package managers / build tools and `git` are NOT
/// treated as path writes here — destructive commands are the concern of the
/// checkpoint layer (spec T-010), not the read-only path constraint.
fn bash_sub_command_is_write(sub_command: &str) -> bool {
    let tokens: Vec<&str> = sub_command.split_whitespace().collect();
    let Some(name) = tokens.first() else {
        return false;
    };
    if BASH_WRITE_COMMANDS.contains(name) {
        return true;
    }
    if *name == "sed" && tokens.iter().any(|t| *t == "-i" || t.starts_with("-i")) {
        return true;
    }
    sub_command.contains('>')
}

/// The output-redirection target of a bash sub-command, if any: the token
/// immediately after the last `>` (e.g. `echo x > out.log` yields
/// `out.log`). Only the redirection target is considered, so path arguments
/// of reading commands (`grep pat tests/ > /dev/null`) are not mistaken for
/// the write target.
fn bash_redirection_target(sub_command: &str) -> Option<String> {
    let index = sub_command.rfind('>')?;
    sub_command[index + 1..]
        .split_whitespace()
        .next()
        .map(|token| {
            token
                .trim_matches(|c: char| c == '"' || c == '\'')
                .to_string()
        })
        .filter(|token| !token.is_empty())
}

impl LoopSpec {
    /// Evaluate the loop's restrictions against one tool invocation.
    ///
    /// Returns `Some(reason)` — the denial observation to append to the
    /// model's context — when the call must not execute:
    ///
    /// - **tool out of scope** (FR-009): the tool is not in the configured
    ///   tool set and is not a mandatory safety tool.
    /// - **read-only constraint** (FR-021): a write tool (or a write-ish
    ///   bash sub-command) targets a path matched by the read-only globs.
    /// - **scope violation** (FR-022): a file-operation path falls outside
    ///   the configured scope boundaries.
    ///
    /// `None` means the call may proceed to the normal permission layer.
    ///
    /// Order of checks: tool set first (cheapest, most structural), then
    /// read-only constraints, then scope boundaries.
    pub fn deny_reason(&self, tool_name: &str, input: &Value) -> Option<String> {
        // FR-008/FR-009: outside the loop's tool set (mandatory safety tools
        // excepted). No configured tool set means every tool is allowed.
        if self.has_tool_set() && !self.allows_tool(tool_name) {
            let mandatory = LOOP_ALWAYS_ALLOWED_TOOLS.contains(&tool_name);
            if !mandatory {
                return Some(format!(
                    "scope violation: tool out of scope — tool '{tool_name}' is \
                     outside the loop's tool set (allowed: {}). Continue with \
                     the tools configured for this loop.",
                    self.tool_set.join(", ")
                ));
            }
        }

        // FR-021/FR-022 path checks apply to explicit path parameters, and
        // for bash to path-looking tokens in the command string.
        let paths = if tool_name == "bash" {
            input
                .get("command")
                .and_then(Value::as_str)
                .map(bash_path_tokens)
                .unwrap_or_default()
        } else {
            candidate_paths(input)
        };

        if tool_name == "bash" {
            // FR-021: a filesystem-mutating bash sub-command whose write
            // target matches a read-only constraint is denied. Scope checks
            // below still apply to every path token in the command.
            if !self.read_only.is_empty() {
                let command = input.get("command").and_then(Value::as_str).unwrap_or("");
                for sub_command in bash_sub_commands(command) {
                    let targets: Vec<String> = if bash_sub_command_is_write(&sub_command) {
                        bash_path_tokens(&sub_command)
                    } else {
                        Vec::new()
                    };
                    let mut candidates = targets;
                    if let Some(target) = bash_redirection_target(&sub_command) {
                        candidates.push(target);
                    }
                    for path in &candidates {
                        if self.path_is_read_only(path) {
                            return Some(format!(
                                "constraint violation: path '{path}' is read-only \
                                 for this loop (read-only constraints: {}). The goal \
                                 must be satisfied without modifying protected paths.",
                                self.read_only.join(", ")
                            ));
                        }
                    }
                }
            }
        } else if LOOP_WRITE_TOOLS.contains(&tool_name) && !self.read_only.is_empty() {
            for path in &paths {
                if self.path_is_read_only(path) {
                    return Some(format!(
                        "constraint violation: path '{path}' is read-only for this \
                         loop (read-only constraints: {}). The goal must be satisfied \
                         without modifying protected paths.",
                        self.read_only.join(", ")
                    ));
                }
            }
        }

        // FR-022: every file-operation path must be inside the scope
        // boundaries (when configured).
        if self.has_scope() {
            for path in &paths {
                if !self.path_in_scope(path) {
                    return Some(format!(
                        "scope violation: path '{path}' is outside the loop's scope \
                         boundaries ({}). Restrict file operations to the configured \
                         scope.",
                        self.scope.join(", ")
                    ));
                }
            }
        }

        None
    }
}
