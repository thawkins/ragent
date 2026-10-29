//! The `new_agent` tool - spawns a sub-agent to perform a focused task.
//!
//! Supports both synchronous (blocking) and background (non-blocking) modes.
//! Background tasks publish [`Event::SubagentComplete`] when finished.
//!
//! An optional `timeout_secs` applies a per-call wall-clock bound to the
//! spawned task. The bound **tightens** the configured stall timeout and can
//! never extend it (FR-017); an exceeded bound terminates the task with
//! [`TaskStatus::TimedOut`], and the bound is reported in the task's detail
//! view (FR-034).

use anyhow::Result;
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};

/// Machine-readable reason for a spawn refused by the background limit
/// (FR-010, FR-029). The `AgentManager` prefixes the same token to its own
/// error, so a caller sees one code whichever layer refused.
pub const REASON_LIMIT_REACHED: &str = "limit_reached";
/// Machine-readable reason for a task id that names no task for this session
/// (FR-010).
pub const REASON_TASK_NOT_FOUND: &str = "task_not_found";
/// Machine-readable reason for a task that exists but belongs to another
/// session (FR-010).
pub const REASON_WRONG_SESSION: &str = "wrong_session";
/// Machine-readable reason for an execution contract that does not exist
/// (`detached: true` without `background: true`) (FR-010).
pub const REASON_CONTRACT_REFUSED: &str = "contract_refused";
/// Machine-readable reason for a call made while a team context is active, so
/// `new_agent` is the wrong tool (FR-009, FR-010).
pub const REASON_TEAM_CONTEXT_ACTIVE: &str = "team_context_active";
/// Machine-readable reason for a per-call `timeout_secs` that is not a positive
/// integer (FR-010, FR-017).
pub const REASON_INVALID_TIMEOUT_SECS: &str = "invalid_timeout_secs";
/// Machine-readable reason for a tool that needs the `AgentManager` and was
/// called where none is available (FR-010).
pub const REASON_MANAGER_UNAVAILABLE: &str = "manager_unavailable";

/// The execution contract a `new_agent` call selects.
///
/// Exactly three contracts exist (FR-001). They are derived once from the two
/// declared booleans (`background`, `detached`) so the rest of the tool reasons
/// about one named value instead of two independent flags, and so the illegal
/// `detached: true` without `background: true` combination is refused rather
/// than silently degraded to [`Execution::Foreground`] (FR-003).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Execution {
    /// `background` absent or `false`: `new_agent` blocks and returns the
    /// sub-agent's response inline.
    Foreground,
    /// `background: true`, `detached` absent or `false`: listed by
    /// `list_agents`, awaitable by `wait_agents`, result injected on completion.
    Background,
    /// `background: true, detached: true`: excluded from `list_agents` and
    /// `wait_agents`, result never injected.
    Detached,
}

impl Execution {
    /// Derives the execution contract from the two declared booleans.
    ///
    /// # Errors
    ///
    /// Returns an error when `detached` is `true` and `background` is absent or
    /// `false`: there is no detached-foreground contract, so the call is
    /// refused with a message naming both parameters and the two legal
    /// combinations. The error carries the machine-readable reason
    /// [`REASON_CONTRACT_REFUSED`] (FR-010).
    fn derive(background: bool, detached: bool) -> Result<Self> {
        match (background, detached) {
            (false, false) => Ok(Self::Foreground),
            (true, false) => Ok(Self::Background),
            (true, true) => Ok(Self::Detached),
            (false, true) => Err(super::param_schema::reason_error(
                REASON_CONTRACT_REFUSED,
                "Invalid execution contract: `detached: true` requires `background: true`. \
                 There is no detached-foreground contract - `detached` only modifies a \
                 background task. Use `background: true, detached: false` for a background \
                 task whose result is injected back into this session, or \
                 `background: true, detached: true` for a fire-and-forget task, or omit both \
                 parameters for a blocking foreground task.",
            )),
        }
    }

    /// Whether this contract returns immediately with a task id.
    fn is_async(self) -> bool {
        !matches!(self, Self::Foreground)
    }

    /// Whether this contract excludes the task from `list_agents` / `wait_agents`
    /// and from result injection.
    fn is_detached(self) -> bool {
        matches!(self, Self::Detached)
    }
}

/// Spawns a sub-agent to perform a focused task.
///
/// Exactly three execution contracts exist, selected by `background` and
/// `detached`:
/// - *foreground* (`background` absent or `false`) - blocks until the
///   sub-agent finishes and returns its response inline;
/// - *background* (`background: true, detached: false`) - returns a task id
///   immediately, is listed by `list_agents`, awaitable with `wait_agents`, and
///   its result is injected into this session on completion;
/// - *detached* (`background: true, detached: true`) - fire-and-forget:
///   excluded from `list_agents` and `wait_agents`, result never injected.
///
/// `detached: true` without `background: true` names no contract and is
/// refused.
///
/// ## Parameters (TWO required)
/// - `agent` (string, **REQUIRED**) - Agent name (e.g. `"explore"`, `"build"`,
///   `"plan"`, `"general"`). Both `agent` AND `task` must be supplied -
///   omitting either will fail with "Missing required parameter: ...".
/// - `task` (string, **REQUIRED**) - The prompt/instructions for the sub-agent.
///   Be specific: include context, the exact question(s) to answer, and the
///   format you want for the answer. The sub-agent has no access to the
///   parent's history.
/// - `background` (bool, optional) - If `true`, spawns in the background and
///   returns immediately with a task ID. Use this whenever you spawn more than
///   one task in the same response so they can run concurrently. Default: `false`.
/// - `detached` (bool, optional) - Only valid with `background: true`. If
///   `true`, the task is fire-and-forget. Default: `false`.
/// - `model` (string, optional) - Model override in `provider/model` or
///   `provider:model` format. Inherits the parent session's model when omitted.
pub struct NewAgentTool;

#[async_trait::async_trait]
impl Tool for NewAgentTool {
    fn name(&self) -> &'static str {
        "new_agent"
    }

    /// The generated description of the tool (FR-016).
    ///
    /// It names every property in [`Self::parameters_schema`] and all three
    /// execution contracts; `test_tool_descriptions` asserts that every schema
    /// property name appears here, so a property added to the schema without a
    /// matching sentence here is a build failure (SPEC.md D1).
    fn description(&self) -> &'static str {
        "Spawn a sub-agent to perform a focused task. Use agent names like 'explore', 'build', \
         'plan', or any custom agent. \n\n\
         THREE EXECUTION CONTRACTS are selected by `background` and `detached`:\n\
         - foreground (`background` absent or false) - blocks until the sub-agent finishes and returns its response inline\n\
         - background (`background: true, detached: false`) - returns a task id immediately; listed by `list_agents`, awaitable with `wait_agents`, result injected into this session on completion\n\
         - detached (`background: true, detached: true`) - fire-and-forget; excluded from `list_agents` and `wait_agents`, result never injected\n\
         `detached: true` without `background: true` names no contract and is refused.\n\n\
         PARAMETERS:\n\
         - `agent` (string, REQUIRED) - the agent name to run\n\
         - `task` (string, REQUIRED) - the prompt/instructions for the sub-agent. BOTH `agent` and `task` must be supplied - omitting either fails the call\n\
         - `background` (bool, default false) - `true` spawns in the background and returns a task id immediately; `false` blocks the whole agent loop, so use `true` whenever you spawn more than one task in the same response\n\
         - `detached` (bool, default false) - only valid with `background: true`; `true` selects the fire-and-forget contract\n\
         - `model` (string, optional) - model override as 'provider/model' (preferred) or 'provider:model', e.g. 'anthropic/claude-sonnet-4-20250514' or 'openai:gpt-4o'. A value with no separator is ignored; omitting it inherits the session's model\n\
         - `timeout_secs` (integer, optional, >= 1) - per-call wall-clock bound in seconds for the spawned task; it may only tighten the configured stall timeout, never extend it, and exceeding it terminates the task with a `timed_out` status\n\n\
         A parameter this schema does not declare is refused, not ignored.\n\
         Example: new_agent(agent: \"explore\", task: \"Find all callers of X in src/\")"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "agent": {
                    "type": "string",
                    "description": "REQUIRED. Name of the agent to run (e.g. 'explore', 'build', 'plan', 'general'). BOTH `agent` AND `task` must be supplied - calls with only one of them will fail."
                },
                "task": {
                    "type": "string",
                    "description": "REQUIRED. The task prompt / instructions for the sub-agent. Be specific - the sub-agent has no access to the parent's history, so include all necessary context, the exact question(s) to answer, and the format you want for the answer. BOTH `agent` AND `task` must be supplied - calls with only one of them will fail."
                },
                "background": {
                    "type": "boolean",
                    "description": "If true, spawn in the background and return immediately - the agent runs concurrently. REQUIRED when spawning more than one task in the same response; background: false blocks all subsequent tool calls. Default: false (use only for a single task whose result you need immediately). Selects the background contract with `detached: false`, or the detached contract with `detached: true`."
                },
                "detached": {
                    "type": "boolean",
                    "description": "Only valid with `background: true`; a call with `detached: true` and `background` absent or false is refused. When true, the task is fire-and-forget: excluded from `list_agents`, cannot be awaited with `wait_agents`, and its completion is NOT injected into this session as a background task result. Use for side-effect-only workflows whose output nobody should consume. Default: false."
                },
                "model": {
                    "type": "string",
                    "description": "Optional model override as 'provider/model' (preferred) or 'provider:model', e.g. 'anthropic/claude-sonnet-4-20250514' or 'openai:gpt-4o'. A string with no '/' or ':' separator is ignored. If omitted, the sub-agent inherits the parent session's model."
                },
                "timeout_secs": {
                    "type": "integer",
                    "description": "Optional per-call wall-clock bound in seconds for the spawned task. It tightens the configured stall timeout and can never extend it: when both apply, the smaller wins. If the bound is exceeded the task is terminated with a `timed_out` status. Omit it and the configured stall timeout applies unchanged. Must be a positive integer.",
                    "minimum": 1
                }
            },
            "required": ["agent", "task"],
            "additionalProperties": false
        })
    }

    fn permission_category(&self) -> &'static str {
        "agent:spawn"
    }

    /// # Errors
    ///
    /// Returns an error if required parameters `agent` or `task` are missing,
    /// if the call carries a property the schema does not declare (FR-004,
    /// FR-038), if `timeout_secs` is present but is not a positive integer
    /// (FR-017, FR-034), if the calling session has an active team context
    /// (FR-009, FR-045 - the refusal names the team and the supported team
    /// tools), if `detached: true` arrives without `background: true`
    /// (FR-003), if the `AgentManager` has not been initialized, or if task
    /// spawning fails.
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        // FR-004 / FR-038: refuse an undeclared property before anything else,
        // so a misspelled flag cannot silently degrade to the default contract.
        super::param_schema::validate_tool_input(&self.parameters_schema(), &input)?;

        // FR-009 / FR-045: a precondition failure is an error, not a successful
        // result whose content is prose. The guidance text is preserved verbatim
        // so the model loses no information by the change of shape.
        if let Some(team) = ctx.team_context.as_ref() {
            let guidance = if team.is_lead {
                format!(
                    "Session '{}' is the lead of active team '{}'. \
                     Do not use `new_agent` for team delegation. Use team tools (`team_spawn`, \
                     `team_task_create`, `team_assign_task`, `team_message`) so teammate activity \
                     and output stay visible in the Teams UI.",
                    ctx.session_id, team.team_name
                )
            } else {
                format!(
                    "Session '{}' is teammate '{}' in active team '{}'. \
                     Do not use `new_agent` from a teammate session. Use team workflow tools \
                     (`team_read_messages`, `team_task_claim`, `team_task_complete`, `team_idle`) \
                     and report progress via team messaging.",
                    ctx.session_id, team.agent_id, team.team_name
                )
            };

            return Err(super::param_schema::reason_error(
                REASON_TEAM_CONTEXT_ACTIVE,
                guidance,
            ));
        }

        let agent = input
            .get("agent")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: agent"))?;

        let task = input
            .get("task")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: task"))?;

        let background = input
            .get("background")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);

        let detached = input
            .get("detached")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);

        // Derive the single named contract before doing any work, so an illegal
        // boolean combination is refused rather than silently degraded (FR-003).
        let execution = Execution::derive(background, detached)?;

        // Per-call wall-clock bound (FR-017, FR-034). Refused when it is not a
        // positive integer: a zero or negative bound would terminate every task
        // instantly, and a non-integer would be silently ignored, which is the
        // class of degradation this family is closing.
        let time_bound_secs = match input.get("timeout_secs") {
            None => None,
            Some(v) => match v.as_u64() {
                Some(n) if n > 0 => Some(n),
                _ => {
                    return Err(super::param_schema::reason_error(
                        REASON_INVALID_TIMEOUT_SECS,
                        format!(
                            "`timeout_secs` must be a positive integer number of seconds; got \
                             {v}. Omit it to leave the configured stall timeout unchanged."
                        ),
                    ));
                }
            },
        };

        let model = input
            .get("model")
            .and_then(|v| v.as_str())
            .map(std::string::ToString::to_string)
            .or_else(|| {
                // Inherit the parent session's provider/model when no explicit override
                // is given. This prevents failures when the parent uses a provider
                // (e.g. copilot) that differs from the sub-agent's hardcoded default.
                ctx.active_model
                    .as_ref()
                    .map(|m| format!("{}/{}", m.provider_id, m.model_id))
            });

        let agent_manager = ctx.agent_manager.as_ref().ok_or_else(|| {
            super::param_schema::reason_error(
                REASON_MANAGER_UNAVAILABLE,
                "Sub-agent spawning is not available in this context. \
                 AgentManager has not been initialised.",
            )
        })?;

        if execution.is_async() {
            let spawned = if execution.is_detached() {
                agent_manager
                    .spawn_detached(
                        &ctx.session_id,
                        agent,
                        task,
                        model.as_deref(),
                        &ctx.working_dir,
                        time_bound_secs,
                    )
                    .await
            } else {
                agent_manager
                    .spawn_background(
                        &ctx.session_id,
                        agent,
                        task,
                        model.as_deref(),
                        &ctx.working_dir,
                        time_bound_secs,
                    )
                    .await
            };
            // FR-010: a refusal raised by the registry (the background limit)
            // already carries a reason code; lift it into this tool's own
            // vocabulary so the caller sees one code, not the registry's.
            let entry = spawned.map_err(lift_registry_refusal)?;

            let detach_note = if execution.is_detached() {
                "\nThis task is DETACHED: it will not appear in `list_agents`, \
                 cannot be awaited with `wait_agents`, and its result will NOT \
                 be injected back into this session."
            } else {
                "\nThe task is running in the background. You will be notified \
                 when it completes via a SubagentComplete event."
            };
            // Report the bound that is actually in force, so the caller can see
            // whether its per-call value was used or clamped (FR-034).
            let bound_note = match entry.time_bound_secs {
                Some(secs) => format!("\nTime bound: {secs}s"),
                None => String::new(),
            };
            Ok(ToolOutput {
                content: format!(
                    "Background task spawned successfully.\n\
                     Task ID: {}\n\
                     Agent: {}\n\
                     Status: running{bound_note}{detach_note}",
                    entry.id, entry.agent_name
                ),
                metadata: Some(json!({
                    "task_id": entry.id,
                    "agent": entry.agent_name,
                    "background": true,
                    "detached": execution.is_detached(),
                    "status": "running",
                    "time_bound_secs": entry.time_bound_secs
                })),
            })
        } else {
            let result = agent_manager
                .spawn_sync(
                    &ctx.session_id,
                    agent,
                    task,
                    model.as_deref(),
                    &ctx.working_dir,
                    time_bound_secs,
                )
                .await
                .map_err(lift_registry_refusal)?;

            Ok(ToolOutput {
                content: result.response,
                metadata: Some(json!({
                    "task_id": result.entry.id,
                    "agent": result.entry.agent_name,
                    "background": false,
                    "status": "completed",
                    "child_session_id": result.entry.child_session_id,
                    "time_bound_secs": result.entry.time_bound_secs
                })),
            })
        }
    }
}

/// Maps a refusal raised by the task registry onto this tool's reason
/// vocabulary (FR-010).
///
/// The registry prefixes its own code (`limit_reached`) to every refusal a
/// caller could branch on; this rewrites that into the `reason_code:` shape
/// [`reason_in`](super::param_schema::reason_in) reads, leaving an uncoded
/// error untouched so nobody mistakes prose for a code.
fn lift_registry_refusal(err: anyhow::Error) -> anyhow::Error {
    let message = err.to_string();
    let Some(reason) = super::param_schema::reason_in(&message) else {
        return err;
    };
    let human = message
        .split_once(": ")
        .map(|(_, rest)| rest)
        .unwrap_or(&message);
    super::param_schema::reason_error(reason, human)
}

#[cfg(test)]
#[path = "../../tests/inline/new_agent_tests.rs"]
mod tests;
