//! Inline tests for `invoke.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

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
        trigger: None,
        allow_dynamic_context: false,
        source_path: PathBuf::from(format!("/skills/{name}/SKILL.md")),
        skill_dir: PathBuf::from(format!("/skills/{name}")),
        scope: SkillScope::Project,
        body: body.to_string(),
        body_cache: crate::skill::default_body_cache(),
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
    // Default allow_dynamic_context is false - commands should NOT execute.
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
