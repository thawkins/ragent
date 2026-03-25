#![allow(missing_docs, unused_variables, unused_imports, dead_code, unused_mut)]

//! Phase 10.6: External tests for TUI-related skill functionality.
//!
//! Tests skill registry filtering, listing, and bundled skill properties
//! that the TUI relies on for slash-command autocomplete and display.

use ragent_core::skill::{SkillContext, SkillInfo, SkillRegistry, SkillScope};

/// Create a skill with controllable invocation flags.
fn skill_with_flags(name: &str, user_invocable: bool, disable_model: bool) -> SkillInfo {
    SkillInfo {
        name: name.to_string(),
        description: Some(format!("{name} desc")),
        body: "body".to_string(),
        scope: SkillScope::Project,
        user_invocable,
        disable_model_invocation: disable_model,
        ..Default::default()
    }
}

// ── User-invocable filtering ─────────────────────────────────────

#[test]
fn test_list_user_invocable_includes_correct_skills() {
    let mut reg = SkillRegistry::new();
    reg.register(skill_with_flags("user-both", true, false));
    reg.register(skill_with_flags("user-only", true, true));
    reg.register(skill_with_flags("agent-only", false, false));
    reg.register(skill_with_flags("disabled", false, true));

    let user_skills = reg.list_user_invocable();
    let names: Vec<&str> = user_skills.iter().map(|s| s.name.as_str()).collect();

    assert!(names.contains(&"user-both"));
    assert!(names.contains(&"user-only"));
    assert!(!names.contains(&"agent-only"));
    assert!(!names.contains(&"disabled"));
}

#[test]
fn test_list_agent_invocable_filters_correctly() {
    let mut reg = SkillRegistry::new();
    reg.register(skill_with_flags("user-both", true, false));
    reg.register(skill_with_flags("user-only", true, true));
    reg.register(skill_with_flags("agent-only", false, false));
    reg.register(skill_with_flags("disabled", false, true));

    let agent_skills = reg.list_agent_invocable();
    let names: Vec<&str> = agent_skills.iter().map(|s| s.name.as_str()).collect();

    assert!(names.contains(&"user-both"));
    assert!(names.contains(&"agent-only"));
    assert!(!names.contains(&"user-only"));
    assert!(!names.contains(&"disabled"));
}

// ── Autocomplete-relevant properties ─────────────────────────────

#[test]
fn test_skill_argument_hint_for_autocomplete() {
    let mut skill = skill_with_flags("deploy", true, false);
    skill.argument_hint = Some("<environment>".to_string());

    let mut reg = SkillRegistry::new();
    reg.register(skill);

    let skills = reg.list_user_invocable();
    let deploy = skills
        .iter()
        .find(|s| s.name == "deploy")
        .expect("find deploy");
    assert_eq!(deploy.argument_hint.as_deref(), Some("<environment>"));
}

#[test]
fn test_skill_without_argument_hint() {
    let skill = skill_with_flags("simple", true, false);

    let mut reg = SkillRegistry::new();
    reg.register(skill);

    let skills = reg.list_user_invocable();
    let simple = skills
        .iter()
        .find(|s| s.name == "simple")
        .expect("find simple");
    assert!(simple.argument_hint.is_none());
}

// ── Bundled skills TUI properties ────────────────────────────────

#[test]
fn test_bundled_batch_is_user_only() {
    let bundled = ragent_core::skill::bundled::bundled_skills();
    let batch = bundled
        .iter()
        .find(|s| s.name == "batch")
        .expect("find batch");
    assert!(batch.user_invocable);
    assert!(batch.disable_model_invocation);
}

#[test]
fn test_bundled_loop_is_user_only() {
    let bundled = ragent_core::skill::bundled::bundled_skills();
    let loopsk = bundled
        .iter()
        .find(|s| s.name == "loop")
        .expect("find loop");
    assert!(loopsk.user_invocable);
    assert!(loopsk.disable_model_invocation);
}

#[test]
fn test_bundled_simplify_is_both() {
    let bundled = ragent_core::skill::bundled::bundled_skills();
    let simplify = bundled
        .iter()
        .find(|s| s.name == "simplify")
        .expect("find simplify");
    assert!(simplify.user_invocable);
    assert!(!simplify.disable_model_invocation);
}

#[test]
fn test_bundled_debug_is_both() {
    let bundled = ragent_core::skill::bundled::bundled_skills();
    let debug = bundled
        .iter()
        .find(|s| s.name == "debug")
        .expect("find debug");
    assert!(debug.user_invocable);
    assert!(!debug.disable_model_invocation);
}

// ── Scope display ────────────────────────────────────────────────

#[test]
fn test_scope_display_strings() {
    assert_eq!(format!("{}", SkillScope::Bundled), "bundled");
    assert_eq!(format!("{}", SkillScope::Enterprise), "enterprise");
    assert_eq!(format!("{}", SkillScope::Personal), "personal");
    assert_eq!(format!("{}", SkillScope::Project), "project");
}

// ── Registry sorted listing for menu ─────────────────────────────

#[test]
fn test_list_all_sorted_for_menu() {
    let mut reg = SkillRegistry::new();
    reg.register(skill_with_flags("zebra", true, false));
    reg.register(skill_with_flags("alpha", true, false));
    reg.register(skill_with_flags("middle", true, false));

    let all = reg.list_all();
    let names: Vec<&str> = all.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, vec!["alpha", "middle", "zebra"]);
}

#[test]
fn test_empty_registry_lists() {
    let reg = SkillRegistry::new();
    assert!(reg.list_all().is_empty());
    assert!(reg.list_user_invocable().is_empty());
    assert!(reg.list_agent_invocable().is_empty());
}

// ── Scope priority for autocomplete dedup ────────────────────────

#[test]
fn test_scope_priority_project_overrides_personal() {
    let personal = SkillInfo {
        name: "deploy".to_string(),
        description: Some("Personal deploy".to_string()),
        body: "personal body".to_string(),
        scope: SkillScope::Personal,
        user_invocable: true,
        ..Default::default()
    };
    let project = SkillInfo {
        name: "deploy".to_string(),
        description: Some("Project deploy".to_string()),
        body: "project body".to_string(),
        scope: SkillScope::Project,
        user_invocable: true,
        ..Default::default()
    };

    let mut reg = SkillRegistry::new();
    reg.register(personal);
    reg.register(project);

    assert_eq!(reg.len(), 1);
    let deploy = reg.get("deploy").expect("find deploy");
    assert_eq!(deploy.scope, SkillScope::Project);
    assert_eq!(deploy.description.as_deref(), Some("Project deploy"));
}

#[test]
fn test_scope_priority_bundled_overridden_by_all() {
    let bundled = SkillInfo {
        name: "test-skill".to_string(),
        description: Some("Bundled".to_string()),
        body: "bundled body".to_string(),
        scope: SkillScope::Bundled,
        user_invocable: true,
        ..Default::default()
    };
    let personal = SkillInfo {
        name: "test-skill".to_string(),
        description: Some("Personal".to_string()),
        body: "personal body".to_string(),
        scope: SkillScope::Personal,
        user_invocable: true,
        ..Default::default()
    };

    let mut reg = SkillRegistry::new();
    reg.register(bundled);
    reg.register(personal);

    let skill = reg.get("test-skill").expect("find skill");
    assert_eq!(skill.scope, SkillScope::Personal);
}

#[test]
fn test_lower_scope_cannot_override_higher() {
    let project = SkillInfo {
        name: "test-skill".to_string(),
        description: Some("Project".to_string()),
        body: "project body".to_string(),
        scope: SkillScope::Project,
        user_invocable: true,
        ..Default::default()
    };
    let personal = SkillInfo {
        name: "test-skill".to_string(),
        description: Some("Personal".to_string()),
        body: "personal body".to_string(),
        scope: SkillScope::Personal,
        user_invocable: true,
        ..Default::default()
    };

    let mut reg = SkillRegistry::new();
    reg.register(project);
    reg.register(personal); // Should NOT override

    let skill = reg.get("test-skill").expect("find skill");
    assert_eq!(
        skill.scope,
        SkillScope::Project,
        "lower scope should not override higher"
    );
}

// ── Skill forked detection ───────────────────────────────────────

#[test]
fn test_is_forked_detection() {
    let mut skill = skill_with_flags("forked", true, false);
    skill.context = Some(SkillContext::Fork);
    assert!(skill.is_forked());

    let normal = skill_with_flags("normal", true, false);
    assert!(!normal.is_forked());
}
