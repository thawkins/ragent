//! Tests for test_skill_parsing.rs

//! Phase 10.1: External tests for SKILL.md parsing.
//!
//! Covers edge cases in YAML frontmatter parsing and body extraction
//! that complement the inline tests in `skill/loader.rs`.

use ragent_core::skill::{SkillInfo, SkillScope};
use std::path::PathBuf;

/// Helper: parse a SKILL.md string into a SkillInfo using the public API.
fn parse(content: &str, name: &str) -> anyhow::Result<SkillInfo> {
    ragent_core::skill::loader::parse_skill_md(
        content,
        &PathBuf::from(format!("/test/skills/{name}/SKILL.md")),
        name,
        SkillScope::Project,
    )
}

// ── Frontmatter field variations ─────────────────────────────────

#[test]
fn test_parse_unicode_description() {
    let content = "---\ndescription: \"Déployer l'application 🚀\"\n---\nBody\n";
    let skill = parse(content, "deploy-fr").expect("should parse unicode description");
    assert_eq!(
        skill.description.as_deref(),
        Some("Déployer l'application 🚀")
    );
}

#[test]
fn test_parse_unicode_body() {
    let content = "---\ndescription: test\n---\n日本語のテキスト\n中文文本\nEmoji: 🦀🔧\n";
    let skill = parse(content, "unicode-body").expect("should parse unicode body");
    assert!(skill.body.contains("日本語"));
    assert!(skill.body.contains("🦀"));
}

#[test]
fn test_parse_boolean_variations_true() {
    // YAML accepts various boolean representations
    for val in &["true", "True", "TRUE"] {
        let content = format!("---\ndisable-model-invocation: {val}\n---\nBody\n");
        let skill =
            parse(&content, "bool-test").unwrap_or_else(|_| panic!("should parse boolean {val}"));
        assert!(
            skill.disable_model_invocation,
            "disable-model-invocation should be true for '{val}'"
        );
    }
}

#[test]
fn test_parse_boolean_variations_false() {
    for val in &["false", "False", "FALSE"] {
        let content = format!("---\ndisable-model-invocation: {val}\n---\nBody\n");
        let skill =
            parse(&content, "bool-test").unwrap_or_else(|_| panic!("should parse boolean {val}"));
        assert!(
            !skill.disable_model_invocation,
            "disable-model-invocation should be false for '{val}'"
        );
    }
}

#[test]
fn test_parse_user_invocable_false() {
    let content = "---\nuser-invocable: false\n---\nAgent-only skill\n";
    let skill = parse(content, "agent-only").expect("should parse");
    assert!(!skill.user_invocable);
}

#[test]
fn test_parse_both_invocation_flags_disabled() {
    let content = "---\nuser-invocable: false\ndisable-model-invocation: true\n---\nDisabled\n";
    let skill = parse(content, "disabled").expect("should parse");
    assert!(!skill.user_invocable);
    assert!(skill.disable_model_invocation);
}

#[test]
fn test_parse_context_fork() {
    let content = "---\ncontext: fork\n---\nForked skill\n";
    let skill = parse(content, "forked").expect("should parse");
    assert!(skill.is_forked());
}

#[test]
fn test_parse_context_default_not_forked() {
    let content = "---\ndescription: Normal\n---\nNormal skill\n";
    let skill = parse(content, "normal").expect("should parse");
    assert!(!skill.is_forked());
}

#[test]
fn test_parse_model_override() {
    let content = "---\nmodel: \"openai/gpt-4\"\n---\nBody\n";
    let skill = parse(content, "model-test").expect("should parse");
    assert_eq!(skill.model.as_deref(), Some("openai/gpt-4"));
}

#[test]
fn test_parse_agent_field() {
    let content = "---\nagent: planner\n---\nBody\n";
    let skill = parse(content, "agent-test").expect("should parse");
    assert_eq!(skill.agent.as_deref(), Some("planner"));
}

#[test]
fn test_parse_argument_hint() {
    let content = "---\nargument-hint: \"<filename> [--force]\"\n---\nBody\n";
    let skill = parse(content, "hint-test").expect("should parse");
    assert_eq!(skill.argument_hint.as_deref(), Some("<filename> [--force]"));
}

#[test]
fn test_parse_multiple_allowed_tools() {
    let content = "---\nallowed-tools:\n  - bash\n  - read\n  - write\n  - grep\n---\nBody\n";
    let skill = parse(content, "tools-test").expect("should parse");
    assert_eq!(skill.allowed_tools, vec!["bash", "read", "write", "grep"]);
}

#[test]
fn test_parse_single_allowed_tool_string() {
    let content = "---\nallowed-tools: bash\n---\nBody\n";
    let skill = parse(content, "single-tool").expect("should parse");
    assert_eq!(skill.allowed_tools, vec!["bash"]);
}

// ── Body extraction ──────────────────────────────────────────────

#[test]
fn test_parse_body_with_triple_dashes_inside() {
    let content = "---\ndescription: test\n---\nBefore\n\n---\n\nAfter dashes\n";
    let skill = parse(content, "dashes").expect("should parse");
    // The --- in the body should be preserved
    assert!(skill.body.contains("---"));
    assert!(skill.body.contains("After dashes"));
}

#[test]
fn test_parse_whitespace_only_body() {
    let content = "---\ndescription: test\n---\n   \n  \n";
    let skill = parse(content, "ws-body").expect("should parse");
    assert!(skill.body.trim().is_empty());
}

#[test]
fn test_parse_multiline_body_preserves_formatting() {
    let content = "---\ndescription: test\n---\n## Step 1\n\n```bash\necho hello\n```\n\n- Item A\n- Item B\n";
    let skill = parse(content, "md-body").expect("should parse");
    assert!(skill.body.contains("## Step 1"));
    assert!(skill.body.contains("```bash"));
    assert!(skill.body.contains("- Item A"));
}

#[test]
fn test_parse_body_with_yaml_like_content() {
    let content = "---\ndescription: test\n---\nHere is some YAML:\n```yaml\nkey: value\nlist:\n  - item1\n  - item2\n```\n";
    let skill = parse(content, "yaml-body").expect("should parse");
    assert!(skill.body.contains("key: value"));
}

// ── Error cases ──────────────────────────────────────────────────

#[test]
fn test_parse_no_frontmatter_delimiters() {
    let content = "Just plain text without any frontmatter\n";
    let _result = parse(content, "no-fm");
    assert!(result.is_err());
}

#[test]
fn test_parse_only_opening_delimiter() {
    let content = "---\ndescription: test\nNo closing delimiter\n";
    let _result = parse(content, "unclosed");
    assert!(result.is_err());
}

#[test]
fn test_parse_empty_string() {
    let _result = parse("", "empty");
    assert!(result.is_err());
}

#[test]
fn test_parse_invalid_yaml_in_frontmatter() {
    let content = "---\n: invalid : yaml : here\n  bad indent\n---\nBody\n";
    // Should either error or parse with defaults — depends on serde_yaml behavior
    // The key point is it doesn't panic
    let _ = parse(content, "invalid-yaml");
}

#[test]
fn test_parse_name_validation_uppercase() {
    let content = "---\nname: MySkill\n---\nBody\n";
    let _result = parse(content, "myskill");
    assert!(result.is_err(), "uppercase names should be rejected");
}

#[test]
fn test_parse_name_validation_spaces() {
    let content = "---\nname: \"my skill\"\n---\nBody\n";
    let _result = parse(content, "my-skill");
    assert!(result.is_err(), "names with spaces should be rejected");
}

#[test]
fn test_parse_name_validation_special_chars() {
    let content = "---\nname: my_skill@v2\n---\nBody\n";
    let _result = parse(content, "my-skill");
    assert!(
        result.is_err(),
        "names with special chars should be rejected"
    );
}

// ── Scope assignment ─────────────────────────────────────────────

#[test]
fn test_parse_scope_preserved() {
    let content = "---\ndescription: test\n---\nBody\n";

    let personal = ragent_core::skill::loader::parse_skill_md(
        content,
        &PathBuf::from("/home/user/.ragent/skills/test/SKILL.md"),
        "test",
        SkillScope::Personal,
    )
    .expect("should parse personal");
    assert_eq!(personal.scope, SkillScope::Personal);

    let project = ragent_core::skill::loader::parse_skill_md(
        content,
        &PathBuf::from("/project/.ragent/skills/test/SKILL.md"),
        "test",
        SkillScope::Project,
    )
    .expect("should parse project");
    assert_eq!(project.scope, SkillScope::Project);
}

// ── Hooks ────────────────────────────────────────────────────────

#[test]
fn test_parse_hooks_converted_to_json() {
    let content = r#"---
description: with hooks
hooks:
  pre:
    command: echo starting
  post:
    command: echo done
---
Body
"#;
    let skill = parse(content, "hooks-test").expect("should parse hooks");
    assert!(skill.hooks.is_some());
    let hooks = skill.hooks.as_ref().expect("hooks should exist");
    assert!(hooks.get("pre").is_some());
    assert!(hooks.get("post").is_some());
}

#[test]
fn test_parse_no_hooks() {
    let content = "---\ndescription: no hooks\n---\nBody\n";
    let skill = parse(content, "no-hooks").expect("should parse");
    assert!(skill.hooks.is_none());
}
