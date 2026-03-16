//! Phase 10.5: External tests for skill invocation.
//!
//! Covers end-to-end invocation logic, forked skill metadata,
//! model/tool overrides, and invocation control flags.

use ragent_core::skill::invoke::{invoke_skill, SkillInvocation};
use ragent_core::skill::{SkillContext, SkillInfo, SkillScope};
use std::path::Path;

/// Default working dir for tests.
fn wd() -> &'static Path {
    Path::new("/tmp")
}

/// Create a test skill with specified properties.
fn make_skill(name: &str, body: &str) -> SkillInfo {
    SkillInfo {
        name: name.to_string(),
        description: Some(format!("{name} skill")),
        body: body.to_string(),
        scope: SkillScope::Project,
        user_invocable: true,
        ..Default::default()
    }
}

fn make_forked_skill(name: &str, body: &str) -> SkillInfo {
    SkillInfo {
        name: name.to_string(),
        description: Some(format!("{name} skill")),
        body: body.to_string(),
        scope: SkillScope::Project,
        user_invocable: true,
        context: Some(SkillContext::Fork),
        agent: Some("coder".to_string()),
        model: Some("openai/gpt-4".to_string()),
        ..Default::default()
    }
}

// ── Basic invocation ─────────────────────────────────────────────

#[tokio::test]
async fn test_invoke_simple_body() {
    let skill = make_skill("greet", "Hello, welcome to the project!");
    let result = invoke_skill(&skill, "", "sess-1", wd()).await.expect("invoke");
    assert_eq!(result.content, "Hello, welcome to the project!");
    assert_eq!(result.skill_name, "greet");
    assert!(!result.forked);
}

#[tokio::test]
async fn test_invoke_with_argument_substitution() {
    let skill = make_skill("deploy", "Deploy $0 to $1");
    let result = invoke_skill(&skill, "frontend staging", "sess-1", wd())
        .await
        .expect("invoke");
    assert_eq!(result.content, "Deploy frontend to staging");
}

#[tokio::test]
async fn test_invoke_with_all_args() {
    let skill = make_skill("run", "Execute: $ARGUMENTS");
    let result = invoke_skill(&skill, "npm test --watch", "sess-1", wd())
        .await
        .expect("invoke");
    assert_eq!(result.content, "Execute: npm test --watch");
}

#[tokio::test]
async fn test_invoke_with_dynamic_context() {
    let skill = make_skill("info", "Version: !`echo v1.0.0`");
    let result = invoke_skill(&skill, "", "sess-1", wd()).await.expect("invoke");
    assert_eq!(result.content, "Version: v1.0.0");
}

#[tokio::test]
async fn test_invoke_combined_args_and_context() {
    let skill = make_skill("combined", "File: $0 Date: !`echo 2026-01-01`");
    let result = invoke_skill(&skill, "main.rs", "sess-1", wd())
        .await
        .expect("invoke");
    assert_eq!(result.content, "File: main.rs Date: 2026-01-01");
}

#[tokio::test]
async fn test_invoke_session_id_substitution() {
    let skill = make_skill("meta", "Session: ${RAGENT_SESSION_ID}");
    let result = invoke_skill(&skill, "", "my-unique-session", wd())
        .await
        .expect("invoke");
    assert_eq!(result.content, "Session: my-unique-session");
}

#[tokio::test]
async fn test_invoke_empty_body() {
    let skill = make_skill("empty", "");
    let result = invoke_skill(&skill, "ignored", "sess-1", wd())
        .await
        .expect("invoke");
    assert_eq!(result.content, "");
}

#[tokio::test]
async fn test_invoke_no_args_needed() {
    let skill = make_skill("static", "Always the same output");
    let result = invoke_skill(&skill, "", "sess-1", wd()).await.expect("invoke");
    assert_eq!(result.content, "Always the same output");
}

// ── Forked skill metadata ────────────────────────────────────────

#[tokio::test]
async fn test_invoke_forked_metadata() {
    let skill = make_forked_skill("analyze", "Analyze $ARGUMENTS");
    let result = invoke_skill(&skill, "the codebase", "sess-1", wd())
        .await
        .expect("invoke");

    assert!(result.forked);
    assert_eq!(result.fork_agent.as_deref(), Some("coder"));
    assert_eq!(result.model_override.as_deref(), Some("openai/gpt-4"));
    assert_eq!(result.content, "Analyze the codebase");
}

#[tokio::test]
async fn test_invoke_non_forked_no_agent() {
    let skill = make_skill("normal", "Normal body");
    let result = invoke_skill(&skill, "", "sess-1", wd()).await.expect("invoke");

    assert!(!result.forked);
    assert!(result.fork_agent.is_none());
}

#[tokio::test]
async fn test_invoke_forked_default_agent() {
    let mut skill = make_skill("forked-default", "Body");
    skill.context = Some(SkillContext::Fork);
    // No agent specified — should use default

    let result = invoke_skill(&skill, "", "sess-1", wd()).await.expect("invoke");
    assert!(result.forked);
    assert!(result.fork_agent.is_none(), "no agent field when not set on skill");
}

// ── Tool restrictions in invocation ──────────────────────────────

#[tokio::test]
async fn test_invoke_with_allowed_tools() {
    let mut skill = make_skill("restricted", "Do stuff");
    skill.allowed_tools = vec!["bash".to_string(), "read".to_string()];

    let result = invoke_skill(&skill, "", "sess-1", wd()).await.expect("invoke");
    assert_eq!(result.allowed_tools, vec!["bash", "read"]);
}

#[tokio::test]
async fn test_invoke_no_tool_restrictions() {
    let skill = make_skill("unrestricted", "Do anything");
    let result = invoke_skill(&skill, "", "sess-1", wd()).await.expect("invoke");
    assert!(result.allowed_tools.is_empty());
}

// ── Invocation result structure ──────────────────────────────────

#[tokio::test]
async fn test_invocation_result_fields() {
    let mut skill = make_forked_skill("full", "Body: $0");
    skill.allowed_tools = vec!["grep".to_string()];
    skill.argument_hint = Some("<pattern>".to_string());

    let result = invoke_skill(&skill, "TODO", "sess-42", wd())
        .await
        .expect("invoke");

    assert_eq!(result.skill_name, "full");
    assert_eq!(result.content, "Body: TODO");
    assert!(result.forked);
    assert_eq!(result.fork_agent.as_deref(), Some("coder"));
    assert_eq!(result.model_override.as_deref(), Some("openai/gpt-4"));
    assert_eq!(result.allowed_tools, vec!["grep"]);
}

// ── Format helpers ───────────────────────────────────────────────

#[test]
fn test_format_skill_message() {
    let invocation = SkillInvocation {
        skill_name: "deploy".to_string(),
        content: "Deploy to production".to_string(),
        forked: false,
        fork_agent: None,
        model_override: None,
        allowed_tools: vec![],
    };

    let msg = ragent_core::skill::invoke::format_skill_message(&invocation);
    assert!(msg.contains("deploy"));
    assert!(msg.contains("Deploy to production"));
}

#[test]
fn test_format_forked_result() {
    let result = ragent_core::skill::invoke::ForkedSkillResult {
        skill_name: "analyze".to_string(),
        forked_session_id: "sess-fork-42".to_string(),
        response: "Found 3 issues".to_string(),
    };

    let formatted = ragent_core::skill::invoke::format_forked_result(&result);
    assert!(formatted.contains("analyze"));
    assert!(formatted.contains("Found 3 issues"));
}
