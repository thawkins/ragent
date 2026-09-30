//! Inline tests for `bundled.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;

#[test]
fn test_bundled_skills_count() {
    let skills = bundled_skills();
    assert_eq!(skills.len(), 4);
}

#[test]
fn test_bundled_skill_names() {
    let skills = bundled_skills();
    let names: Vec<&str> = skills.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"simplify"));
    assert!(names.contains(&"batch"));
    assert!(names.contains(&"debug"));
    assert!(names.contains(&"loop"));
}

#[test]
fn test_bundled_skills_scope() {
    for skill in bundled_skills() {
        assert_eq!(
            skill.scope,
            SkillScope::Bundled,
            "Skill '{}' should have Bundled scope",
            skill.name
        );
    }
}

#[test]
fn test_bundled_skills_have_descriptions() {
    for skill in bundled_skills() {
        assert!(
            skill.description.is_some(),
            "Skill '{}' should have a description",
            skill.name
        );
    }
}

#[test]
fn test_bundled_skills_user_invocable() {
    for skill in bundled_skills() {
        assert!(
            skill.user_invocable,
            "Skill '{}' should be user-invocable",
            skill.name
        );
    }
}

#[test]
fn test_simplify_skill() {
    let skill = simplify_skill();
    assert_eq!(skill.name, "simplify");
    assert!(skill.body.contains("git diff"));
    assert!(!skill.disable_model_invocation);
    assert_eq!(skill.argument_hint.as_deref(), Some("[all|output_path]"));
}

#[test]
fn test_batch_skill() {
    let skill = batch_skill();
    assert_eq!(skill.name, "batch");
    assert!(skill.body.contains("$ARGUMENTS"));
    assert!(skill.disable_model_invocation, "batch is user-only");
    assert_eq!(skill.argument_hint.as_deref(), Some("<instruction>"));
}

#[test]
fn test_debug_skill() {
    let skill = debug_skill();
    assert_eq!(skill.name, "debug");
    assert!(skill.body.contains("$ARGUMENTS"));
    assert!(!skill.disable_model_invocation);
    assert_eq!(skill.argument_hint.as_deref(), Some("[description]"));
}

#[test]
fn test_loop_skill() {
    let skill = loop_skill();
    assert_eq!(skill.name, "loop");
    assert!(skill.body.contains("$ARGUMENTS"));
    assert!(skill.disable_model_invocation, "loop is user-only");
    assert_eq!(skill.argument_hint.as_deref(), Some("[interval] <prompt>"));
}

#[test]
fn test_bundled_skills_have_nonempty_bodies() {
    for skill in bundled_skills() {
        assert!(
            !skill.body.is_empty(),
            "Skill '{}' should have a non-empty body",
            skill.name
        );
    }
}

#[test]
fn test_bundled_skills_have_allowed_tools() {
    for skill in bundled_skills() {
        assert!(
            !skill.allowed_tools.is_empty(),
            "Skill '{}' should have at least one allowed tool",
            skill.name
        );
        assert!(
            skill.allowed_tools.contains(&"bash".to_string()),
            "Skill '{}' should allow bash",
            skill.name
        );
    }
}
