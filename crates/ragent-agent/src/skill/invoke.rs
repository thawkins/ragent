//! Skill invocation logic.
//!
//! Combines argument substitution and dynamic context injection to produce
//! the final processed skill content ready for injection into a conversation.
//! Supports both inline invocation (content injected into current conversation)
//! and forked invocation (content run in an isolated sub-session).

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use super::SkillInfo;
use super::args::substitute_args;
use super::context::inject_dynamic_context;

/// Parse a model reference in either `provider/model` or `provider:model` format.
#[must_use]
pub fn parse_model_ref(model_str: &str) -> Option<crate::agent::ModelRef> {
    model_str
        .split_once('/')
        .or_else(|| model_str.split_once(':'))
        .map(|(provider, model)| crate::agent::ModelRef {
            provider_id: provider.to_string(),
            model_id: model.to_string(),
        })
}

/// Resolve the agent used for an inline skill invocation in the current session.
///
/// The active session model overrides built-in agent defaults unless the agent
/// profile explicitly pinned its model. An explicit skill-level `model:` then
/// takes highest priority over both. The skill's `allowed_tools` are enforced
/// for the duration of the skill run by setting them on the returned agent.
#[must_use]
pub fn resolve_inline_skill_agent(
    base_agent: &Arc<crate::agent::AgentInfo>,
    active_model: Option<&str>,
    skill_model: Option<&str>,
    allowed_tools: &[String],
) -> Arc<crate::agent::AgentInfo> {
    let mut changed = false;
    let mut new_model = None;

    if (!base_agent.model_pinned || base_agent.model.is_none())
        && let Some(model_str) = active_model
        && let Some(model_ref) = parse_model_ref(model_str)
    {
        new_model = Some(model_ref);
        changed = true;
    }

    if let Some(model_str) = skill_model
        && let Some(model_ref) = parse_model_ref(model_str)
    {
        new_model = Some(model_ref);
        changed = true;
    }

    let needs_tool_restriction = !allowed_tools.is_empty();
    if !changed && !needs_tool_restriction {
        return Arc::clone(base_agent);
    }

    let mut agent = Arc::clone(base_agent);
    if changed {
        Arc::make_mut(&mut agent).model = new_model;
    }
    if needs_tool_restriction {
        Arc::make_mut(&mut agent).allowed_tools = Some(allowed_tools.to_vec());
    }
    agent
}

/// Result of invoking a skill, containing the processed content and metadata.
#[derive(Debug, Clone)]
pub struct SkillInvocation {
    /// The skill name that was invoked.
    pub skill_name: String,
    /// The fully processed skill body (arguments substituted, commands executed).
    pub content: String,
    /// Whether this skill should run in a forked subagent context.
    pub forked: bool,
    /// The agent type to use for forked execution (e.g. `"explore"`, `"general"`).
    pub fork_agent: Option<String>,
    /// Model override for this skill, if any.
    pub model_override: Option<String>,
    /// Tools allowed without permission when this skill is active.
    pub allowed_tools: Vec<String>,
}

/// Invoke a skill by processing its body with argument substitution and
/// dynamic context injection.
///
/// This is the main entry point for skill invocation. It:
/// 1. Substitutes argument placeholders (`$ARGUMENTS`, `$0`, etc.)
/// 2. Executes dynamic context commands (`` !`command` ``)
/// 3. Returns a [`SkillInvocation`] with the processed content and metadata
///
/// # Arguments
///
/// * `skill` - The skill definition to invoke.
/// * `args` - Raw argument string (e.g. for `/deploy staging`, this is `"staging"`).
/// * `session_id` - The current session identifier.
/// * `working_dir` - The directory in which to execute dynamic context commands.
///
/// # Errors
///
/// Returns an error if dynamic context injection fails critically.
///
/// # Examples
///
/// ```no_run
/// # async fn example() -> anyhow::Result<()> {
/// use ragent_agent::skill::{SkillInfo, invoke::invoke_skill};
/// use std::path::Path;
///
/// let mut skill = SkillInfo::new("deploy", "Deploy $ARGUMENTS to production");
/// skill.description = Some("Deploy the application".to_string());
///
/// let result = invoke_skill(&skill, "staging", "sess-1", Path::new("/project")).await?;
/// assert_eq!(result.content, "Deploy staging to production");
/// assert_eq!(result.skill_name, "deploy");
/// # Ok(())
/// # }
/// ```
pub async fn invoke_skill(
    skill: &SkillInfo,
    args: &str,
    session_id: &str,
    working_dir: &Path,
) -> anyhow::Result<SkillInvocation> {
    tracing::info!(
        skill = %skill.name,
        args = %args,
        forked = skill.is_forked(),
        "Invoking skill"
    );

    let body = skill.body_or_load().await?;

    // 1. Substitute argument placeholders
    let substituted = substitute_args(&body, args, session_id, &skill.skill_dir);

    // 2. Execute dynamic context injection only when the skill opts in.
    let content = if skill.allow_dynamic_context {
        inject_dynamic_context(&substituted, working_dir).await?
    } else {
        substituted
    };

    Ok(SkillInvocation {
        skill_name: skill.name.clone(),
        content,
        forked: skill.is_forked(),
        fork_agent: skill.agent.clone(),
        model_override: skill.model.clone(),
        allowed_tools: skill.allowed_tools.clone(),
    })
}

/// Format a skill invocation result as a user message suitable for the LLM.
///
/// Wraps the processed skill content with context about the skill being invoked
/// so the agent understands the instruction source.
///
/// # Examples
///
/// ```
/// use ragent_agent::skill::invoke::{SkillInvocation, format_skill_message};
///
/// let invocation = SkillInvocation {
///     skill_name: "deploy".to_string(),
///     content: "Deploy staging to production".to_string(),
///     forked: false,
///     fork_agent: None,
///     model_override: None,
///     allowed_tools: vec![],
/// };
///
/// let msg = format_skill_message(&invocation);
/// assert!(msg.contains("deploy"));
/// assert!(msg.contains("Deploy staging to production"));
/// ```
#[must_use]
pub fn format_skill_message(invocation: &SkillInvocation) -> String {
    format!(
        "[Skill: /{}]\n\n{}",
        invocation.skill_name, invocation.content
    )
}

/// Result of a forked skill execution.
#[derive(Debug, Clone)]
pub struct ForkedSkillResult {
    /// The skill that was invoked.
    pub skill_name: String,
    /// The ID of the forked session that was created.
    pub forked_session_id: String,
    /// The assistant's response from the forked session.
    pub response: String,
}

/// Execute a skill in a forked subagent context.
///
/// Creates an isolated sub-session with fresh message history, resolves the
/// agent specified by the skill (defaulting to `"general"`), applies any model
/// override, and runs the processed skill content through the agent loop.
///
/// The forked session's assistant response is returned so the caller can
/// inject a summary back into the parent conversation.
///
/// # Arguments
///
/// * `invocation` - The already-processed skill invocation (from [`invoke_skill`]).
/// * `processor` - The session processor to use for the sub-session.
/// * `parent_session_id` - The session from which this fork originates.
/// * `working_dir` - Working directory for the forked session.
/// * `cancel_flag` - Cancellation flag shared with the caller.
///
/// # Errors
///
/// Returns an error if session creation fails, the agent cannot be resolved,
/// or the LLM call fails.
pub async fn invoke_forked_skill(
    invocation: &SkillInvocation,
    processor: &crate::session::processor::SessionProcessor,
    parent_session_id: &str,
    working_dir: &std::path::Path,
    cancel_flag: Arc<AtomicBool>,
    active_model: Option<crate::agent::ModelRef>,
) -> anyhow::Result<ForkedSkillResult> {
    tracing::info!(
        skill = %invocation.skill_name,
        parent_session = %parent_session_id,
        agent = ?invocation.fork_agent,
        model = ?invocation.model_override,
        "Executing forked skill in sub-session"
    );

    // 1. Create an isolated forked session
    let forked_session = processor
        .session_manager
        .create_session(working_dir.to_path_buf())?;
    let forked_sid = forked_session.id.clone();

    tracing::debug!(
        forked_session = %forked_sid,
        parent_session = %parent_session_id,
        "Created forked session for skill"
    );

    // 2. Resolve the subagent
    let agent = resolve_forked_skill_agent(invocation, active_model.as_ref())?;

    // 4. Format the skill content as the initial prompt
    let prompt = format_skill_message(invocation);

    // 5. Run the skill content through the agent loop in the forked session
    let response_msg = processor
        .process_message(&forked_sid, &prompt, &agent, cancel_flag)
        .await?;

    let response_text = response_msg.text_content();

    tracing::info!(
        skill = %invocation.skill_name,
        forked_session = %forked_sid,
        response_len = response_text.len(),
        "Forked skill execution complete"
    );

    Ok(ForkedSkillResult {
        skill_name: invocation.skill_name.clone(),
        forked_session_id: forked_sid,
        response: response_text,
    })
}

/// Resolve the agent used for a forked skill invocation.
///
/// Starts from the skill's requested agent (defaulting to `general`), inherits
/// the active session model when the target agent is not model-pinned, and then
/// applies any explicit skill-level model override.
///
/// # Errors
///
/// Returns an error if the target agent cannot be resolved.
pub fn resolve_forked_skill_agent(
    invocation: &SkillInvocation,
    active_model: Option<&crate::agent::ModelRef>,
) -> anyhow::Result<Arc<crate::agent::AgentInfo>> {
    let agent_name = invocation.fork_agent.as_deref().unwrap_or("general");
    let config = crate::Config::default();
    let mut agent = crate::agent::resolve_agent(agent_name, &config)?;
    Arc::make_mut(&mut agent).mode = crate::agent::AgentMode::Subagent;

    if (!agent.model_pinned || agent.model.is_none())
        && let Some(model_ref) = active_model
    {
        Arc::make_mut(&mut agent).model = Some(model_ref.clone());
    }

    if let Some(model_str) = invocation.model_override.as_deref()
        && let Some(model_ref) = parse_model_ref(model_str)
    {
        Arc::make_mut(&mut agent).model = Some(model_ref);
    }

    if !invocation.allowed_tools.is_empty() {
        Arc::make_mut(&mut agent).allowed_tools = Some(invocation.allowed_tools.clone());
    }

    Ok(agent)
}

/// Format the result of a forked skill execution as a message for the
/// parent conversation, so the main agent can see the subagent's output.
///
/// # Examples
///
/// ```
/// use ragent_agent::skill::invoke::{ForkedSkillResult, format_forked_result};
///
/// let result = ForkedSkillResult {
///     skill_name: "review".to_string(),
///     forked_session_id: "fork-123".to_string(),
///     response: "Found 3 issues in the code.".to_string(),
/// };
///
/// let msg = format_forked_result(&result);
/// assert!(msg.contains("review"));
/// assert!(msg.contains("Found 3 issues"));
/// ```
#[must_use]
pub fn format_forked_result(result: &ForkedSkillResult) -> String {
    format!(
        "[Forked Skill Result: /{}]\n\n{}",
        result.skill_name, result.response
    )
}

#[cfg(test)]
#[path = "../tests/inline/invoke_tests.rs"]
mod tests;
