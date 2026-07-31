//! Tests for mod.rs (M8/T8.4).
//! Compiled as a submodule via #[path], super::* resolves to the source module.

use super::*;

#[test]
fn test_registry_register_and_get() {
    let mut registry = SkillRegistry::new();
    let skill = SkillInfo::new("deploy", "Deploy to production");
    registry.register(skill);

    assert_eq!(registry.len(), 1);
    assert!(!registry.is_empty());
    let found = registry.get("deploy").expect("should find deploy");
    assert_eq!(found.name, "deploy");
}

#[test]
fn test_registry_get_missing() {
    let registry = SkillRegistry::new();
    assert!(registry.get("nonexistent").is_none());
}

#[test]
fn test_registry_scope_priority_higher_wins() {
    let mut registry = SkillRegistry::new();

    let mut personal = SkillInfo::new("deploy", "Personal deploy");
    personal.scope = SkillScope::Personal;
    personal.description = Some("personal version".to_string());
    registry.register(personal);

    let mut project = SkillInfo::new("deploy", "Project deploy");
    project.scope = SkillScope::Project;
    project.description = Some("project version".to_string());
    registry.register(project);

    assert_eq!(registry.len(), 1);
    let skill = registry.get("deploy").expect("should find deploy");
    assert_eq!(skill.description.as_deref(), Some("project version"));
    assert_eq!(skill.scope, SkillScope::Project);
}

#[test]
fn test_registry_scope_priority_lower_rejected() {
    let mut registry = SkillRegistry::new();

    let mut project = SkillInfo::new("deploy", "Project deploy");
    project.scope = SkillScope::Project;
    project.description = Some("project version".to_string());
    registry.register(project);

    // Lower-priority personal skill should NOT override
    let mut personal = SkillInfo::new("deploy", "Personal deploy");
    personal.scope = SkillScope::Personal;
    personal.description = Some("personal version".to_string());
    registry.register(personal);

    assert_eq!(registry.len(), 1);
    let skill = registry.get("deploy").expect("should find deploy");
    assert_eq!(skill.description.as_deref(), Some("project version"));
}

#[test]
fn test_registry_scope_priority_equal_replaces() {
    let mut registry = SkillRegistry::new();

    let mut first = SkillInfo::new("deploy", "First");
    first.scope = SkillScope::Project;
    first.description = Some("first".to_string());
    registry.register(first);

    let mut second = SkillInfo::new("deploy", "Second");
    second.scope = SkillScope::Project;
    second.description = Some("second".to_string());
    registry.register(second);

    let skill = registry.get("deploy").expect("should find deploy");
    assert_eq!(skill.description.as_deref(), Some("second"));
}

#[test]
fn test_registry_bundled_overridden_by_project() {
    let mut registry = SkillRegistry::new();

    let mut bundled = SkillInfo::new("simplify", "Bundled simplify");
    bundled.scope = SkillScope::Bundled;
    bundled.description = Some("bundled".to_string());
    registry.register(bundled);

    let mut project = SkillInfo::new("simplify", "Custom simplify");
    project.scope = SkillScope::Project;
    project.description = Some("custom".to_string());
    registry.register(project);

    let skill = registry.get("simplify").expect("should find simplify");
    assert_eq!(skill.description.as_deref(), Some("custom"));
    assert_eq!(skill.scope, SkillScope::Project);
}

#[test]
fn test_registry_list_user_invocable() {
    let mut registry = SkillRegistry::new();

    let mut visible = SkillInfo::new("visible", "Visible skill");
    visible.user_invocable = true;
    registry.register(visible);

    let mut hidden = SkillInfo::new("hidden", "Hidden skill");
    hidden.user_invocable = false;
    registry.register(hidden);

    let user_skills = registry.list_user_invocable();
    assert_eq!(user_skills.len(), 1);
    assert_eq!(user_skills[0].name, "visible");
}

#[test]
fn test_registry_list_agent_invocable() {
    let mut registry = SkillRegistry::new();

    let mut auto = SkillInfo::new("auto", "Auto-invocable");
    auto.disable_model_invocation = false;
    registry.register(auto);

    let mut manual = SkillInfo::new("manual", "Manual only");
    manual.disable_model_invocation = true;
    registry.register(manual);

    let agent_skills = registry.list_agent_invocable();
    assert_eq!(agent_skills.len(), 1);
    assert_eq!(agent_skills[0].name, "auto");
}

#[test]
fn test_registry_list_all_sorted() {
    let mut registry = SkillRegistry::new();
    registry.register(SkillInfo::new("zebra", "Z"));
    registry.register(SkillInfo::new("alpha", "A"));
    registry.register(SkillInfo::new("middle", "M"));

    let all = registry.list_all();
    assert_eq!(all.len(), 3);
    assert_eq!(all[0].name, "alpha");
    assert_eq!(all[1].name, "middle");
    assert_eq!(all[2].name, "zebra");
}

#[test]
fn test_registry_multiple_skills() {
    let mut registry = SkillRegistry::new();
    registry.register(SkillInfo::new("deploy", "Deploy"));
    registry.register(SkillInfo::new("test", "Test"));
    registry.register(SkillInfo::new("lint", "Lint"));

    assert_eq!(registry.len(), 3);
    assert!(registry.get("deploy").is_some());
    assert!(registry.get("test").is_some());
    assert!(registry.get("lint").is_some());
}

#[test]
fn test_registry_empty() {
    let registry = SkillRegistry::new();
    assert!(registry.is_empty());
    assert_eq!(registry.len(), 0);
    assert!(registry.list_all().is_empty());
    assert!(registry.list_user_invocable().is_empty());
    assert!(registry.list_agent_invocable().is_empty());
}

#[test]
fn test_registry_load_empty_dir() {
    let tmp = std::env::temp_dir().join("ragent_test_load_empty");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).expect("create temp dir");

    let registry = SkillRegistry::load(&tmp, &[]);
    // Only bundled skills when no discovered project skills exist
    assert_eq!(registry.bundled_count(), 4);
    assert!(registry.get("simplify").is_some());
    assert!(registry.get("batch").is_some());
    assert!(registry.get("debug").is_some());
    assert!(registry.get("loop").is_some());
    // No project-scoped skills discovered from this working dir
    assert!(registry.get("deploy").is_none());
    assert!(registry.get("lint").is_none());

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_registry_load_project_skills() {
    let tmp = std::env::temp_dir().join("ragent_test_load_project");
    let _ = std::fs::remove_dir_all(&tmp);

    let skills_dir = tmp.join(".ragent").join("skills");
    let deploy_dir = skills_dir.join("deploy");
    std::fs::create_dir_all(&deploy_dir).expect("create deploy dir");
    std::fs::write(
        deploy_dir.join("SKILL.md"),
        "---\ndescription: Deploy app\n---\nDeploy it\n",
    )
    .expect("write SKILL.md");

    let lint_dir = skills_dir.join("lint");
    std::fs::create_dir_all(&lint_dir).expect("create lint dir");
    std::fs::write(
        lint_dir.join("SKILL.md"),
        "---\ndescription: Run linter\n---\nLint code\n",
    )
    .expect("write SKILL.md");

    let registry = SkillRegistry::load(&tmp, &[]);
    // 4 bundled skills are always present
    assert_eq!(registry.bundled_count(), 4);

    let deploy = registry.get("deploy").expect("should find deploy");
    assert_eq!(deploy.description.as_deref(), Some("Deploy app"));
    assert_eq!(deploy.scope, SkillScope::Project);

    let lint = registry.get("lint").expect("should find lint");
    assert_eq!(lint.description.as_deref(), Some("Run linter"));

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_registry_load_skips_dirs_without_skill_md() {
    let tmp = std::env::temp_dir().join("ragent_test_load_no_md");
    let _ = std::fs::remove_dir_all(&tmp);

    let skills_dir = tmp.join(".ragent").join("skills");
    let empty_dir = skills_dir.join("empty-skill");
    std::fs::create_dir_all(&empty_dir).expect("create empty skill dir");

    // Also create a file that's NOT SKILL.md
    std::fs::write(empty_dir.join("README.md"), "Not a skill").expect("write readme");

    let registry = SkillRegistry::load(&tmp, &[]);
    // Only bundled skills (no discovered project skills from this dir)
    assert_eq!(registry.bundled_count(), 4);
    assert!(registry.get("empty-skill").is_none());

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_registry_load_skips_malformed_skills() {
    let tmp = std::env::temp_dir().join("ragent_test_load_malformed");
    let _ = std::fs::remove_dir_all(&tmp);

    let skills_dir = tmp.join(".ragent").join("skills");
    let bad_dir = skills_dir.join("bad");
    std::fs::create_dir_all(&bad_dir).expect("create bad dir");
    std::fs::write(bad_dir.join("SKILL.md"), "No frontmatter here!").expect("write bad SKILL.md");

    // Also add a good skill
    let good_dir = skills_dir.join("good");
    std::fs::create_dir_all(&good_dir).expect("create good dir");
    std::fs::write(
        good_dir.join("SKILL.md"),
        "---\ndescription: A good skill\n---\nGood body\n",
    )
    .expect("write good SKILL.md");

    let registry = SkillRegistry::load(&tmp, &[]);
    // 4 bundled skills are always present; bad is skipped, good is discovered
    assert_eq!(registry.bundled_count(), 4);
    assert!(registry.get("good").is_some());
    assert!(registry.get("bad").is_none());

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_registry_load_monorepo_nested() {
    let tmp = std::env::temp_dir().join("ragent_test_load_monorepo");
    let _ = std::fs::remove_dir_all(&tmp);

    // Root-level skill
    let root_skills = tmp.join(".ragent").join("skills").join("root-skill");
    std::fs::create_dir_all(&root_skills).expect("create root skill dir");
    std::fs::write(
        root_skills.join("SKILL.md"),
        "---\ndescription: Root skill\n---\nRoot body\n",
    )
    .expect("write root SKILL.md");

    // Nested package skill
    let pkg_skills = tmp
        .join("packages")
        .join("frontend")
        .join(".ragent")
        .join("skills")
        .join("frontend-deploy");
    std::fs::create_dir_all(&pkg_skills).expect("create nested skill dir");
    std::fs::write(
        pkg_skills.join("SKILL.md"),
        "---\ndescription: Frontend deploy\n---\nDeploy frontend\n",
    )
    .expect("write nested SKILL.md");

    // Need to create the "packages" dir at first level
    // The monorepo scan only looks one level deep from working_dir
    // packages/frontend/.ragent/skills/ means we scan packages/ -> frontend/ -> .ragent/skills/
    // But our code only goes one level: working_dir/*/. ragent/skills/
    // So packages/ won't match unless packages/.ragent/skills/ exists
    // Let me put the nested skill at the correct depth

    let nested_skills = tmp
        .join("frontend")
        .join(".ragent")
        .join("skills")
        .join("frontend-deploy");
    let _ = std::fs::remove_dir_all(tmp.join("packages"));
    std::fs::create_dir_all(&nested_skills).expect("create nested skill dir");
    std::fs::write(
        nested_skills.join("SKILL.md"),
        "---\ndescription: Frontend deploy\n---\nDeploy frontend\n",
    )
    .expect("write nested SKILL.md");

    let registry = SkillRegistry::load(&tmp, &[]);
    // 4 bundled skills are always present; 2 project skills discovered
    assert_eq!(registry.bundled_count(), 4);
    assert!(registry.get("root-skill").is_some());
    assert!(registry.get("frontend-deploy").is_some());

    let _ = std::fs::remove_dir_all(&tmp);
}

#[tokio::test]
async fn test_registry_load_project_overrides_bundled() {
    let tmp = std::env::temp_dir().join("ragent_test_load_override_bundled");
    let _ = std::fs::remove_dir_all(&tmp);

    // Create a project skill named "simplify" that overrides the bundled one
    let skills_dir = tmp.join(".ragent").join("skills").join("simplify");
    std::fs::create_dir_all(&skills_dir).expect("create skill dir");
    std::fs::write(
        skills_dir.join("SKILL.md"),
        "---\ndescription: Custom simplify\n---\nMy custom simplify instructions\n",
    )
    .expect("write SKILL.md");

    let registry = SkillRegistry::load(&tmp, &[]);

    let simplify = registry.get("simplify").expect("should find simplify");
    assert_eq!(
        simplify.description.as_deref(),
        Some("Custom simplify"),
        "Project skill should override bundled skill"
    );
    assert_eq!(simplify.scope, SkillScope::Project);
    assert!(
        simplify
            .body_or_load()
            .await
            .expect("should load body")
            .contains("My custom simplify")
    );

    let _ = std::fs::remove_dir_all(&tmp);
}

#[tokio::test]
async fn test_registry_load_with_extra_dirs() {
    let tmp = std::env::temp_dir().join("ragent_test_load_extra_dirs");
    let _ = std::fs::remove_dir_all(&tmp);

    let extra_dir = tmp.join("shared_skills");
    let skill_dir = extra_dir.join("shared-tool");
    std::fs::create_dir_all(&skill_dir).expect("create skill dir");
    std::fs::write(
        skill_dir.join("SKILL.md"),
        "---\ndescription: Shared tool\n---\nShared body\n",
    )
    .expect("write SKILL.md");

    let work_dir = tmp.join("project");
    std::fs::create_dir_all(&work_dir).expect("create work dir");

    let extra = vec![extra_dir.to_string_lossy().to_string()];
    let registry = SkillRegistry::load(&work_dir, &extra);

    // 4 bundled skills are always present; 1 extra skill discovered
    assert_eq!(registry.bundled_count(), 4);
    let shared = registry
        .get("shared-tool")
        .expect("should find shared-tool");
    assert_eq!(shared.description.as_deref(), Some("Shared tool"));

    let _ = std::fs::remove_dir_all(&tmp);
}
