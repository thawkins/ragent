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
/// takes highest priority over both.
#[must_use]
pub fn resolve_inline_skill_agent(
    base_agent: &crate::agent::AgentInfo,
    active_model: Option<&str>,
    skill_model: Option<&str>,
) -> crate::agent::AgentInfo {
    let mut agent = base_agent.clone();

    if (!agent.model_pinned || agent.model.is_none())
        && let Some(model_str) = active_model
        && let Some(model_ref) = parse_model_ref(model_str)
    {
        agent.model = Some(model_ref);
    }

    if let Some(model_str) = skill_model
        && let Some(model_ref) = parse_model_ref(model_str)
    {
        agent.model = Some(model_ref);
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
/// * `skill` — The skill definition to invoke.
/// * `args` — Raw argument string (e.g. for `/deploy staging`, this is `"staging"`).
/// * `session_id` — The current session identifier.
/// * `working_dir` — The directory in which to execute dynamic context commands.
///
/// # Errors
///
/// Returns an error if dynamic context injection fails critically.
///
/// # Examples
///
/// ```no_run
/// # async fn example() -> anyhow::Result<()> {
/// use ragent_core::skill::{SkillInfo, invoke::invoke_skill};
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

    // 1. Substitute argument placeholders
    let substituted = substitute_args(&skill.body, args, session_id, &skill.skill_dir);

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
/// use ragent_core::skill::invoke::{SkillInvocation, format_skill_message};
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
/// * `invocation` — The already-processed skill invocation (from [`invoke_skill`]).
/// * `processor` — The session processor to use for the sub-session.
/// * `parent_session_id` — The session from which this fork originates.
/// * `working_dir` — Working directory for the forked session.
/// * `cancel_flag` — Cancellation flag shared with the caller.
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
) -> anyhow::Result<crate::agent::AgentInfo> {
    let agent_name = invocation.fork_agent.as_deref().unwrap_or("general");
    let config = crate::Config::default();
    let mut agent = crate::agent::resolve_agent(agent_name, &config)?;
    agent.mode = crate::agent::AgentMode::Subagent;

    if (!agent.model_pinned || agent.model.is_none())
        && let Some(model_ref) = active_model
    {
        agent.model = Some(model_ref.clone());
    }

    if let Some(model_str) = invocation.model_override.as_deref()
        && let Some(model_ref) = parse_model_ref(model_str)
    {
        agent.model = Some(model_ref);
    }

    Ok(agent)
}

/// Format the result of a forked skill execution as a message for the
/// parent conversation, so the main agent can see the subagent's output.
///
/// # Examples
///
/// ```
/// use ragent_core::skill::invoke::{ForkedSkillResult, format_forked_result};
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
mod tests {
    use super::*;
    use crate::skill::{SkillContext, SkillScope};
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn test_skill(name: &str, body: &str) -> SkillInfo {
        SkillInfo {
            name: name.to_string(),
            description: Some(format!("{name} skill")),
            argument_hint: None,
            disable_model_invocation: false,
            user_invocable: true,
            allowed_tools: Vec::new(),
            model: None,
            context: None,
            agent: None,
            hooks: None,
            license: None,
            compatibility: None,
            metadata: HashMap::new(),
            allow_dynamic_context: false,
            source_path: PathBuf::from(format!("/skills/{name}/SKILL.md")),
            skill_dir: PathBuf::from(format!("/skills/{name}")),
            scope: SkillScope::Project,
            body: body.to_string(),
        }
    }

    #[tokio::test]
    async fn test_invoke_simple_skill() {
        let skill = test_skill("greet", "Hello $ARGUMENTS!");
        let result = invoke_skill(&skill, "world", "s1", Path::new("/tmp"))
            .await
            .expect("should invoke");

        assert_eq!(result.content, "Hello world!");
        assert_eq!(result.skill_name, "greet");
        assert!(!result.forked);
        assert!(result.fork_agent.is_none());
    }

    #[tokio::test]
    async fn test_invoke_with_dynamic_context() {
        let mut skill = test_skill("info", "Version: !`echo 1.0.0`");
        skill.allow_dynamic_context = true;
        let result = invoke_skill(&skill, "", "s1", Path::new("/tmp"))
            .await
            .expect("should invoke");

        assert_eq!(result.content, "Version: 1.0.0");
    }

    #[tokio::test]
    async fn test_invoke_with_args_and_context() {
        let mut skill = test_skill("deploy", "Deploy $0 at !`date +%Y`");
        skill.allow_dynamic_context = true;
        let result = invoke_skill(&skill, "staging", "s1", Path::new("/tmp"))
            .await
            .expect("should invoke");

        assert!(result.content.starts_with("Deploy staging at "));
        // Year should be a 4-digit number
        let year_part = result
            .content
            .strip_prefix("Deploy staging at ")
            .unwrap_or("");
        assert!(
            year_part.parse::<u32>().is_ok(),
            "Expected year, got: {year_part}"
        );
    }

    #[tokio::test]
    async fn test_invoke_preserves_fork_metadata() {
        let mut skill = test_skill("review", "Review the code");
        skill.context = Some(SkillContext::Fork);
        skill.agent = Some("explore".to_string());
        skill.model = Some("anthropic:claude-haiku".to_string());
        skill.allowed_tools = vec!["read".to_string(), "grep".to_string()];

        let result = invoke_skill(&skill, "", "s1", Path::new("/tmp"))
            .await
            .expect("should invoke");

        assert!(result.forked);
        assert_eq!(result.fork_agent.as_deref(), Some("explore"));
        assert_eq!(
            result.model_override.as_deref(),
            Some("anthropic:claude-haiku")
        );
        assert_eq!(result.allowed_tools, vec!["read", "grep"]);
    }

    #[tokio::test]
    async fn test_invoke_skips_dynamic_context_when_disabled() {
        // Default allow_dynamic_context is false — commands should NOT execute.
        let skill = test_skill("info", "Version: !`echo 1.0.0`");
        assert!(!skill.allow_dynamic_context);
        let result = invoke_skill(&skill, "", "s1", Path::new("/tmp"))
            .await
            .expect("should invoke");
        // The !`echo 1.0.0` pattern remains unprocessed.
        assert_eq!(result.content, "Version: !`echo 1.0.0`");
    }

    #[tokio::test]
    async fn test_invoke_no_args_no_context() {
        let skill = test_skill("simplify", "Review recently changed files");
        let result = invoke_skill(&skill, "", "s1", Path::new("/tmp"))
            .await
            .expect("should invoke");

        assert_eq!(result.content, "Review recently changed files");
    }

    #[tokio::test]
    async fn test_invoke_session_id_substitution() {
        let skill = test_skill("debug", "Session: ${RAGENT_SESSION_ID}");
        let result = invoke_skill(&skill, "", "my-session-42", Path::new("/tmp"))
            .await
            .expect("should invoke");

        assert_eq!(result.content, "Session: my-session-42");
    }

    #[test]
    fn test_format_skill_message() {
        let invocation = SkillInvocation {
            skill_name: "deploy".to_string(),
            content: "Deploy staging to production".to_string(),
            forked: false,
            fork_agent: None,
            model_override: None,
            allowed_tools: vec![],
        };

        let msg = format_skill_message(&invocation);
        assert_eq!(msg, "[Skill: /deploy]\n\nDeploy staging to production");
    }

    #[test]
    fn test_format_skill_message_multiline() {
        let invocation = SkillInvocation {
            skill_name: "review".to_string(),
            content: "Step 1: Read code\nStep 2: Find issues\nStep 3: Report".to_string(),
            forked: false,
            fork_agent: None,
            model_override: None,
            allowed_tools: vec![],
        };

        let msg = format_skill_message(&invocation);
        assert!(msg.starts_with("[Skill: /review]\n\n"));
        assert!(msg.contains("Step 1: Read code"));
    }

    #[test]
    fn test_format_forked_result() {
        let result = ForkedSkillResult {
            skill_name: "review".to_string(),
            forked_session_id: "fork-abc".to_string(),
            response: "Found 3 issues in the code.".to_string(),
        };

        let msg = format_forked_result(&result);
        assert_eq!(
            msg,
            "[Forked Skill Result: /review]\n\nFound 3 issues in the code."
        );
    }

    #[test]
    fn test_format_forked_result_multiline() {
        let result = ForkedSkillResult {
            skill_name: "audit".to_string(),
            forked_session_id: "fork-xyz".to_string(),
            response: "Issue 1: Missing validation\nIssue 2: SQL injection risk".to_string(),
        };

        let msg = format_forked_result(&result);
        assert!(msg.starts_with("[Forked Skill Result: /audit]\n\n"));
        assert!(msg.contains("Missing validation"));
        assert!(msg.contains("SQL injection risk"));
    }

    #[test]
    fn test_forked_skill_result_struct() {
        let result = ForkedSkillResult {
            skill_name: "deploy".to_string(),
            forked_session_id: "sess-fork-1".to_string(),
            response: "Deployed successfully".to_string(),
        };

        assert_eq!(result.skill_name, "deploy");
        assert_eq!(result.forked_session_id, "sess-fork-1");
        assert_eq!(result.response, "Deployed successfully");
    }

    #[tokio::test]
    async fn test_invoke_forked_skill_sets_metadata() {
        let mut skill = test_skill("review", "Review the code in $ARGUMENTS");
        skill.context = Some(SkillContext::Fork);
        skill.agent = Some("explore".to_string());
        skill.model = Some("anthropic/claude-haiku".to_string());
        skill.allowed_tools = vec!["read".to_string(), "grep".to_string()];

        let invocation = invoke_skill(&skill, "src/main.rs", "s1", Path::new("/tmp"))
            .await
            .expect("should invoke");

        assert!(invocation.forked);
        assert_eq!(invocation.fork_agent.as_deref(), Some("explore"));
        assert_eq!(
            invocation.model_override.as_deref(),
            Some("anthropic/claude-haiku")
        );
        assert_eq!(invocation.allowed_tools, vec!["read", "grep"]);
        assert_eq!(invocation.content, "Review the code in src/main.rs");
    }

    #[test]
    fn test_invocation_default_agent_fallback() {
        // When fork_agent is None, the forked execution should default to "general"
        let invocation = SkillInvocation {
            skill_name: "test".to_string(),
            content: "test content".to_string(),
            forked: true,
            fork_agent: None,
            model_override: None,
            allowed_tools: vec![],
        };

        let agent_name = invocation.fork_agent.as_deref().unwrap_or("general");
        assert_eq!(agent_name, "general");
    }

    #[test]
    fn test_invocation_agent_specified() {
        let invocation = SkillInvocation {
            skill_name: "test".to_string(),
            content: "test content".to_string(),
            forked: true,
            fork_agent: Some("explore".to_string()),
            model_override: None,
            allowed_tools: vec![],
        };

        let agent_name = invocation.fork_agent.as_deref().unwrap_or("general");
        assert_eq!(agent_name, "explore");
    }
}
