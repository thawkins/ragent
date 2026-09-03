//! Tests for loader.rs (M8/T8.4).
//! Compiled as a submodule via #[path], super::* resolves to the source module.

use super::*;
use std::path::PathBuf;

#[test]
fn test_parse_minimal_frontmatter() {
    let content = "---\n---\nHello world\n";
    let skill = parse_skill_md(
        content,
        &PathBuf::from("/test/skills/hello/SKILL.md"),
        "hello",
        SkillScope::Project,
    )
    .expect("should parse minimal frontmatter");

    assert_eq!(skill.name, "hello");
    assert!(skill.description.is_none());
    assert!(skill.user_invocable);
    assert!(!skill.disable_model_invocation);
    assert!(
        skill.allowed_tools.is_empty(),
        "allowed_tools should default empty"
    );
    assert_eq!(skill.body.trim(), "Hello world");
    assert_eq!(skill.scope, SkillScope::Project);
}

#[test]
fn test_parse_full_frontmatter() {
    let content = r#"---
name: deploy
description: Deploy the application to production
disable-model-invocation: true
allowed-tools:
  - bash
  - read
context: fork
agent: general-purpose
argument-hint: "[environment]"
model: "anthropic:claude-sonnet-4-20250514"
---

Deploy $ARGUMENTS to production:

1. Run the test suite
2. Build the release binary
"#;
    let skill = parse_skill_md(
        content,
        &PathBuf::from("/project/.ragent/skills/deploy/SKILL.md"),
        "deploy",
        SkillScope::Project,
    )
    .expect("should parse full frontmatter");

    assert_eq!(skill.name, "deploy");
    assert_eq!(
        skill.description.as_deref(),
        Some("Deploy the application to production")
    );
    assert!(skill.disable_model_invocation);
    assert!(skill.user_invocable);
    assert_eq!(skill.allowed_tools, vec!["bash", "read"]);
    assert_eq!(skill.context, Some(SkillContext::Fork));
    assert_eq!(skill.agent.as_deref(), Some("general-purpose"));
    assert_eq!(skill.argument_hint.as_deref(), Some("[environment]"));
    assert_eq!(
        skill.model.as_deref(),
        Some("anthropic:claude-sonnet-4-20250514")
    );
    assert!(skill.body.contains("Deploy $ARGUMENTS to production"));
    assert!(skill.body.contains("Run the test suite"));
}

#[test]
fn test_parse_single_allowed_tool() {
    let content = "---\nallowed-tools: bash\n---\nBody\n";
    let skill = parse_skill_md(
        content,
        &PathBuf::from("/test/skills/test/SKILL.md"),
        "test",
        SkillScope::Project,
    )
    .expect("should parse single allowed tool");

    assert_eq!(skill.allowed_tools, vec!["bash"]);
}

#[test]
fn test_parse_user_invocable_false() {
    let content = "---\nuser-invocable: false\n---\nAgent-only skill\n";
    let skill = parse_skill_md(
        content,
        &PathBuf::from("/test/skills/internal/SKILL.md"),
        "internal",
        SkillScope::Project,
    )
    .expect("should parse user-invocable false");

    assert!(!skill.user_invocable);
    assert!(!skill.is_user_invocable());
    assert!(skill.is_agent_invocable());
}

#[test]
fn test_parse_name_from_directory() {
    let content = "---\ndescription: A test skill\n---\nBody\n";
    let skill = parse_skill_md(
        content,
        &PathBuf::from("/test/skills/my-skill/SKILL.md"),
        "my-skill",
        SkillScope::Personal,
    )
    .expect("should use directory name as skill name");

    assert_eq!(skill.name, "my-skill");
    assert_eq!(skill.scope, SkillScope::Personal);
}

#[test]
fn test_parse_name_override() {
    let content = "---\nname: custom-name\n---\nBody\n";
    let skill = parse_skill_md(
        content,
        &PathBuf::from("/test/skills/dir-name/SKILL.md"),
        "dir-name",
        SkillScope::Project,
    )
    .expect("should use frontmatter name over directory name");

    assert_eq!(skill.name, "custom-name");
}

#[test]
fn test_parse_no_frontmatter() {
    let content = "Just plain markdown without frontmatter";
    let result = parse_skill_md(
        content,
        &PathBuf::from("/test/skills/bad/SKILL.md"),
        "bad",
        SkillScope::Project,
    );

    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("must start with YAML frontmatter")
    );
}

#[test]
fn test_parse_unclosed_frontmatter() {
    let content = "---\nname: broken\nno closing delimiter";
    let result = parse_skill_md(
        content,
        &PathBuf::from("/test/skills/broken/SKILL.md"),
        "broken",
        SkillScope::Project,
    );

    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("missing closing ---")
    );
}

#[test]
fn test_validate_name_too_long() {
    let long_name = "a".repeat(65);
    let content = format!("---\nname: {long_name}\n---\nBody\n");
    let result = parse_skill_md(
        &content,
        &PathBuf::from("/test/skills/long/SKILL.md"),
        "long",
        SkillScope::Project,
    );

    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("exceeds 64 characters")
    );
}

#[test]
fn test_validate_name_invalid_chars() {
    let content = "---\nname: My Skill!\n---\nBody\n";
    let result = parse_skill_md(
        content,
        &PathBuf::from("/test/skills/bad/SKILL.md"),
        "bad",
        SkillScope::Project,
    );

    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("must contain only lowercase")
    );
}

#[test]
fn test_parse_hooks_to_json() {
    let content = r#"---
hooks:
  PostToolUse:
    - type: command
      command: "./scripts/check.sh"
---
Body
"#;
    let skill = parse_skill_md(
        content,
        &PathBuf::from("/test/skills/hooked/SKILL.md"),
        "hooked",
        SkillScope::Project,
    )
    .expect("should parse hooks");

    assert!(skill.hooks.is_some());
    let hooks = skill.hooks.as_ref().expect("hooks should be Some");
    assert!(hooks.is_object());
    assert!(hooks.get("PostToolUse").is_some());
}

#[test]
fn test_skill_dir_set_correctly() {
    let content = "---\n---\nBody\n";
    let skill = parse_skill_md(
        content,
        &PathBuf::from("/project/.ragent/skills/deploy/SKILL.md"),
        "deploy",
        SkillScope::Project,
    )
    .expect("should set skill_dir");

    assert_eq!(
        skill.skill_dir,
        PathBuf::from("/project/.ragent/skills/deploy")
    );
    assert_eq!(
        skill.source_path,
        PathBuf::from("/project/.ragent/skills/deploy/SKILL.md")
    );
}

#[test]
fn test_is_forked() {
    let content = "---\ncontext: fork\n---\nBody\n";
    let skill = parse_skill_md(
        content,
        &PathBuf::from("/test/skills/forked/SKILL.md"),
        "forked",
        SkillScope::Project,
    )
    .expect("should parse forked skill");

    assert!(skill.is_forked());
}

#[test]
fn test_is_not_forked() {
    let content = "---\n---\nBody\n";
    let skill = parse_skill_md(
        content,
        &PathBuf::from("/test/skills/normal/SKILL.md"),
        "normal",
        SkillScope::Project,
    )
    .expect("should parse non-forked skill");

    assert!(!skill.is_forked());
}

#[test]
fn test_empty_body() {
    let content = "---\nname: empty\n---\n";
    let skill = parse_skill_md(
        content,
        &PathBuf::from("/test/skills/empty/SKILL.md"),
        "empty",
        SkillScope::Project,
    )
    .expect("should parse empty body");

    assert!(skill.body.is_empty(), "body should be empty");
}

#[test]
fn test_multiline_body() {
    let content = "---\n---\nLine 1\n\nLine 3\n\n## Heading\n\nParagraph\n";
    let skill = parse_skill_md(
        content,
        &PathBuf::from("/test/skills/multi/SKILL.md"),
        "multi",
        SkillScope::Project,
    )
    .expect("should parse multiline body");

    assert!(skill.body.contains("Line 1"));
    assert!(skill.body.contains("## Heading"));
    assert!(skill.body.contains("Paragraph"));
}

#[test]
fn test_discover_skills_from_project_dir() {
    let tmp = std::env::temp_dir().join("ragent_test_discover_project");
    let _ = std::fs::remove_dir_all(&tmp);

    let skills_dir = tmp.join(".ragent").join("skills");
    let deploy_dir = skills_dir.join("deploy");
    std::fs::create_dir_all(&deploy_dir).expect("create deploy dir");
    std::fs::write(
        deploy_dir.join("SKILL.md"),
        "---\ndescription: Deploy app\ncontext: fork\nagent: general-purpose\n---\nDeploy it\n",
    )
    .expect("write deploy SKILL.md");

    let skills = discover_skills(&tmp, &[]);
    let project_skills: Vec<_> = skills
        .into_iter()
        .filter(|s| s.scope == SkillScope::Project)
        .collect();
    assert_eq!(project_skills.len(), 1);
    assert_eq!(project_skills[0].name, "deploy");
    assert_eq!(project_skills[0].scope, SkillScope::Project);
    assert_eq!(project_skills[0].description.as_deref(), Some("Deploy app"));
    assert!(project_skills[0].is_forked());
    assert_eq!(project_skills[0].agent.as_deref(), Some("general-purpose"));

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_discover_skills_empty_working_dir() {
    let tmp = std::env::temp_dir().join("ragent_test_discover_empty");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).expect("create temp dir");

    let skills = discover_skills(&tmp, &[]);
    assert!(!skills.into_iter().any(|s| s.scope == SkillScope::Project));

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_discover_skills_nonexistent_dir() {
    let skills = discover_skills(Path::new("/nonexistent/path/that/should/not/exist"), &[]);
    assert!(!skills.into_iter().any(|s| s.scope == SkillScope::Project));
}

#[test]
fn test_discover_skills_monorepo_nested() {
    let tmp = std::env::temp_dir().join("ragent_test_discover_monorepo");
    let _ = std::fs::remove_dir_all(&tmp);

    // Subdirectory with its own .ragent/skills/
    let nested_dir = tmp
        .join("backend")
        .join(".ragent")
        .join("skills")
        .join("api-test");
    std::fs::create_dir_all(&nested_dir).expect("create nested skill dir");
    std::fs::write(
        nested_dir.join("SKILL.md"),
        "---\ndescription: Run API tests\n---\nTest the API\n",
    )
    .expect("write nested SKILL.md");

    let skills = discover_skills(&tmp, &[]);
    let project_skills: Vec<_> = skills
        .into_iter()
        .filter(|s| s.scope == SkillScope::Project)
        .collect();
    assert_eq!(project_skills.len(), 1);
    assert_eq!(project_skills[0].name, "api-test");
    assert_eq!(project_skills[0].scope, SkillScope::Project);

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_discover_skills_multiple() {
    let tmp = std::env::temp_dir().join("ragent_test_discover_multi");
    let _ = std::fs::remove_dir_all(&tmp);

    let skills_dir = tmp.join(".ragent").join("skills");
    for name in &["deploy", "lint", "test-all"] {
        let dir = skills_dir.join(name);
        std::fs::create_dir_all(&dir).expect("create skill dir");
        std::fs::write(
            dir.join("SKILL.md"),
            format!("---\ndescription: {name} skill\n---\nRun {name}\n"),
        )
        .expect("write SKILL.md");
    }

    let skills = discover_skills(&tmp, &[]);
    let project_skills: Vec<_> = skills
        .into_iter()
        .filter(|s| s.scope == SkillScope::Project)
        .collect();
    assert_eq!(project_skills.len(), 3);

    let names: Vec<&str> = project_skills.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"deploy"));
    assert!(names.contains(&"lint"));
    assert!(names.contains(&"test-all"));

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_discover_skills_skips_non_directories() {
    let tmp = std::env::temp_dir().join("ragent_test_discover_skip_files");
    let _ = std::fs::remove_dir_all(&tmp);

    let skills_dir = tmp.join(".ragent").join("skills");
    std::fs::create_dir_all(&skills_dir).expect("create skills dir");

    // Create a regular file inside skills/ (not a directory)
    std::fs::write(skills_dir.join("not-a-dir.md"), "---\n---\nBody\n").expect("write file");

    // Create a valid skill directory
    let valid_dir = skills_dir.join("valid");
    std::fs::create_dir_all(&valid_dir).expect("create valid dir");
    std::fs::write(valid_dir.join("SKILL.md"), "---\n---\nBody\n").expect("write SKILL.md");

    let skills = discover_skills(&tmp, &[]);
    let project_skills: Vec<_> = skills
        .into_iter()
        .filter(|s| s.scope == SkillScope::Project)
        .collect();
    assert_eq!(project_skills.len(), 1);
    assert_eq!(project_skills[0].name, "valid");

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_discover_skills_with_extra_files() {
    let tmp = std::env::temp_dir().join("ragent_test_discover_extras");
    let _ = std::fs::remove_dir_all(&tmp);

    let skill_dir = tmp.join(".ragent").join("skills").join("deploy");
    let scripts_dir = skill_dir.join("scripts");
    let templates_dir = skill_dir.join("templates");
    std::fs::create_dir_all(&scripts_dir).expect("create scripts dir");
    std::fs::create_dir_all(&templates_dir).expect("create templates dir");

    std::fs::write(
        skill_dir.join("SKILL.md"),
        "---\ndescription: Deploy with scripts\n---\nDeploy\n",
    )
    .expect("write SKILL.md");
    std::fs::write(scripts_dir.join("deploy.sh"), "#!/bin/bash\necho deploy")
        .expect("write script");
    std::fs::write(templates_dir.join("config.toml"), "[server]\nport = 8080")
        .expect("write template");

    let skills = discover_skills(&tmp, &[]);
    let project_skills: Vec<_> = skills
        .into_iter()
        .filter(|s| s.scope == SkillScope::Project)
        .collect();
    assert_eq!(project_skills.len(), 1);
    assert_eq!(project_skills[0].name, "deploy");
    // Verify the skill_dir points to the skill directory (containing scripts/, templates/)
    assert!(project_skills[0].skill_dir.ends_with("deploy"));

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_discover_skills_extra_dirs() {
    let tmp = std::env::temp_dir().join("ragent_test_discover_extra");
    let _ = std::fs::remove_dir_all(&tmp);

    // Create an extra dir with a skill
    let extra_dir = tmp.join("extra_skills");
    let custom_dir = extra_dir.join("custom-lint");
    std::fs::create_dir_all(&custom_dir).expect("create custom skill dir");
    std::fs::write(
        custom_dir.join("SKILL.md"),
        "---\ndescription: Custom linter\n---\nRun the custom linter\n",
    )
    .expect("write SKILL.md");

    // Working dir has no skills
    let work_dir = tmp.join("project");
    std::fs::create_dir_all(&work_dir).expect("create work dir");

    let extra = vec![extra_dir.to_string_lossy().to_string()];
    let skills = discover_skills(&work_dir, &extra);
    let personal_skills: Vec<_> = skills
        .into_iter()
        .filter(|s| s.scope == SkillScope::Personal)
        .collect();

    assert_eq!(personal_skills.len(), 1);
    assert_eq!(personal_skills[0].name, "custom-lint");
    assert_eq!(
        personal_skills[0].description.as_deref(),
        Some("Custom linter")
    );
    assert_eq!(personal_skills[0].scope, SkillScope::Personal);

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_discover_skills_extra_dirs_overridden_by_project() {
    let tmp = std::env::temp_dir().join("ragent_test_discover_extra_override");
    let _ = std::fs::remove_dir_all(&tmp);

    // Extra dir has a skill called "deploy"
    let extra_dir = tmp.join("extra_skills");
    let extra_deploy = extra_dir.join("deploy");
    std::fs::create_dir_all(&extra_deploy).expect("create extra deploy dir");
    std::fs::write(
        extra_deploy.join("SKILL.md"),
        "---\ndescription: Extra deploy\n---\nExtra deploy body\n",
    )
    .expect("write extra SKILL.md");

    // Project also has a skill called "deploy"
    let work_dir = tmp.join("project");
    let proj_deploy = work_dir.join(".ragent").join("skills").join("deploy");
    std::fs::create_dir_all(&proj_deploy).expect("create project deploy dir");
    std::fs::write(
        proj_deploy.join("SKILL.md"),
        "---\ndescription: Project deploy\n---\nProject deploy body\n",
    )
    .expect("write project SKILL.md");

    let extra = vec![extra_dir.to_string_lossy().to_string()];
    let skills = discover_skills(&work_dir, &extra);

    // Both are returned; the registry handles dedup by scope priority
    // Extra dir skill is Personal scope, project skill is Project scope
    let extra_skill = skills.iter().find(|s| s.scope == SkillScope::Personal);
    let proj_skill = skills.iter().find(|s| s.scope == SkillScope::Project);
    assert!(extra_skill.is_some(), "should find extra dir skill");
    assert!(proj_skill.is_some(), "should find project skill");

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_discover_skills_extra_dirs_nonexistent() {
    let tmp = std::env::temp_dir().join("ragent_test_discover_extra_noexist");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).expect("create tmp");

    let extra = vec!["/nonexistent/skill/dir/12345".to_string()];
    let skills = discover_skills(&tmp, &extra);
    assert!(!skills.into_iter().any(|s| s.scope == SkillScope::Project));

    let _ = std::fs::remove_dir_all(&tmp);
}
