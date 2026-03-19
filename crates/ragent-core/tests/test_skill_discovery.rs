//! Phase 10.4: External tests for skill discovery.
//!
//! Covers filesystem-based skill discovery including extra directories,
//! scope priority, monorepo nesting, and error resilience.

use ragent_core::skill::{SkillRegistry, SkillScope};

/// Write a minimal SKILL.md file.
fn write_skill(dir: &std::path::Path, name: &str, desc: &str) {
    let skill_dir = dir.join(name);
    std::fs::create_dir_all(&skill_dir).expect("create skill dir");
    std::fs::write(
        skill_dir.join("SKILL.md"),
        format!("---\ndescription: {desc}\n---\n{desc} body\n"),
    )
    .expect("write SKILL.md");
}

// ── Registry load with bundled skills ────────────────────────────

#[test]
fn test_load_empty_dir_has_bundled_skills() {
    let tmp = std::env::temp_dir().join("ragent_ext_test_empty");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).expect("create tmp");

    let registry = SkillRegistry::load(&tmp, &[]);

    // Should have exactly the 4 bundled skills
    assert_eq!(registry.len(), 4);
    assert!(registry.get("simplify").is_some());
    assert!(registry.get("batch").is_some());
    assert!(registry.get("debug").is_some());
    assert!(registry.get("loop").is_some());

    // All should be Bundled scope
    for skill in registry.list_all() {
        assert_eq!(skill.scope, SkillScope::Bundled);
    }

    let _ = std::fs::remove_dir_all(&tmp);
}

// ── Project skills override bundled ──────────────────────────────

#[test]
fn test_project_skill_overrides_bundled_by_name() {
    let tmp = std::env::temp_dir().join("ragent_ext_test_override");
    let _ = std::fs::remove_dir_all(&tmp);

    let proj_skills = tmp.join(".ragent").join("skills");
    write_skill(&proj_skills, "simplify", "Custom simplify");

    let registry = SkillRegistry::load(&tmp, &[]);

    let simplify = registry.get("simplify").expect("should find simplify");
    assert_eq!(simplify.scope, SkillScope::Project);
    assert_eq!(simplify.description.as_deref(), Some("Custom simplify"));

    let _ = std::fs::remove_dir_all(&tmp);
}

// ── Extra dirs discovery ─────────────────────────────────────────

#[test]
fn test_extra_dirs_skills_discovered() {
    let tmp = std::env::temp_dir().join("ragent_ext_test_extra");
    let _ = std::fs::remove_dir_all(&tmp);

    let work_dir = tmp.join("project");
    std::fs::create_dir_all(&work_dir).expect("create work dir");

    let extra = tmp.join("shared");
    write_skill(&extra, "shared-lint", "Shared linter");

    let extra_dirs = vec![extra.to_string_lossy().to_string()];
    let registry = SkillRegistry::load(&work_dir, &extra_dirs);

    // 4 bundled + 1 shared
    assert_eq!(registry.len(), 5);
    let lint = registry
        .get("shared-lint")
        .expect("should find shared-lint");
    assert_eq!(lint.scope, SkillScope::Personal);

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_project_overrides_extra_dir_skill() {
    let tmp = std::env::temp_dir().join("ragent_ext_test_extra_override");
    let _ = std::fs::remove_dir_all(&tmp);

    let work_dir = tmp.join("project");
    let proj_skills = work_dir.join(".ragent").join("skills");
    write_skill(&proj_skills, "deploy", "Project deploy");

    let extra = tmp.join("shared");
    write_skill(&extra, "deploy", "Shared deploy");

    let extra_dirs = vec![extra.to_string_lossy().to_string()];
    let registry = SkillRegistry::load(&work_dir, &extra_dirs);

    let deploy = registry.get("deploy").expect("should find deploy");
    assert_eq!(
        deploy.scope,
        SkillScope::Project,
        "project should override extra dir"
    );
    assert_eq!(deploy.description.as_deref(), Some("Project deploy"));

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_extra_dirs_nonexistent_ignored() {
    let tmp = std::env::temp_dir().join("ragent_ext_test_noexist");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).expect("create tmp");

    let extra_dirs = vec!["/definitely/does/not/exist/12345".to_string()];
    let registry = SkillRegistry::load(&tmp, &extra_dirs);

    // Should still have bundled skills, no crash
    assert_eq!(registry.len(), 4);

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_multiple_extra_dirs() {
    let tmp = std::env::temp_dir().join("ragent_ext_test_multi_extra");
    let _ = std::fs::remove_dir_all(&tmp);

    let work_dir = tmp.join("project");
    std::fs::create_dir_all(&work_dir).expect("create work dir");

    let extra1 = tmp.join("team-skills");
    write_skill(&extra1, "team-lint", "Team linter");

    let extra2 = tmp.join("org-skills");
    write_skill(&extra2, "org-deploy", "Org deploy");

    let extra_dirs = vec![
        extra1.to_string_lossy().to_string(),
        extra2.to_string_lossy().to_string(),
    ];
    let registry = SkillRegistry::load(&work_dir, &extra_dirs);

    // 4 bundled + 2 extra
    assert_eq!(registry.len(), 6);
    assert!(registry.get("team-lint").is_some());
    assert!(registry.get("org-deploy").is_some());

    let _ = std::fs::remove_dir_all(&tmp);
}

// ── Monorepo nested discovery ────────────────────────────────────

#[test]
fn test_monorepo_discovers_nested_skills() {
    let tmp = std::env::temp_dir().join("ragent_ext_test_mono");
    let _ = std::fs::remove_dir_all(&tmp);

    // Root skill
    let root_skills = tmp.join(".ragent").join("skills");
    write_skill(&root_skills, "root-build", "Root build");

    // Nested package skill (one level deep)
    let nested_skills = tmp.join("frontend").join(".ragent").join("skills");
    write_skill(&nested_skills, "fe-test", "Frontend test");

    let registry = SkillRegistry::load(&tmp, &[]);

    // 4 bundled + 2 discovered
    assert_eq!(registry.len(), 6);
    assert!(registry.get("root-build").is_some());
    assert!(registry.get("fe-test").is_some());

    let _ = std::fs::remove_dir_all(&tmp);
}

// ── Malformed skills resilience ──────────────────────────────────

#[test]
fn test_malformed_skill_skipped_others_loaded() {
    let tmp = std::env::temp_dir().join("ragent_ext_test_malformed");
    let _ = std::fs::remove_dir_all(&tmp);

    let skills_dir = tmp.join(".ragent").join("skills");

    // Good skill
    write_skill(&skills_dir, "good", "A good skill");

    // Bad skill (no frontmatter)
    let bad_dir = skills_dir.join("bad");
    std::fs::create_dir_all(&bad_dir).expect("create bad dir");
    std::fs::write(bad_dir.join("SKILL.md"), "No frontmatter here").expect("write bad");

    let registry = SkillRegistry::load(&tmp, &[]);

    // 4 bundled + 1 good (bad is skipped)
    assert_eq!(registry.len(), 5);
    assert!(registry.get("good").is_some());
    assert!(registry.get("bad").is_none());

    let _ = std::fs::remove_dir_all(&tmp);
}

// ── Listing methods ──────────────────────────────────────────────

#[test]
fn test_list_all_sorted_alphabetically() {
    let tmp = std::env::temp_dir().join("ragent_ext_test_sorted");
    let _ = std::fs::remove_dir_all(&tmp);

    let skills_dir = tmp.join(".ragent").join("skills");
    write_skill(&skills_dir, "zebra", "Zebra skill");
    write_skill(&skills_dir, "alpha", "Alpha skill");

    let registry = SkillRegistry::load(&tmp, &[]);
    let all = registry.list_all();
    let names: Vec<&str> = all.iter().map(|s| s.name.as_str()).collect();

    // Should be sorted: alpha, batch, debug, loop, simplify, zebra
    for i in 1..names.len() {
        assert!(
            names[i - 1] <= names[i],
            "Skills should be sorted: '{}' should come before '{}'",
            names[i - 1],
            names[i]
        );
    }

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_list_user_invocable_filters_correctly() {
    let tmp = std::env::temp_dir().join("ragent_ext_test_invocable");
    let _ = std::fs::remove_dir_all(&tmp);

    let skills_dir = tmp.join(".ragent").join("skills");

    // User-invocable skill (default)
    write_skill(&skills_dir, "user-ok", "User invocable");

    // Agent-only skill
    let agent_dir = skills_dir.join("agent-only");
    std::fs::create_dir_all(&agent_dir).expect("create agent skill dir");
    std::fs::write(
        agent_dir.join("SKILL.md"),
        "---\nuser-invocable: false\ndescription: Agent only\n---\nBody\n",
    )
    .expect("write SKILL.md");

    let registry = SkillRegistry::load(&tmp, &[]);
    let user_skills = registry.list_user_invocable();

    assert!(
        user_skills.iter().any(|s| s.name == "user-ok"),
        "user-ok should be in user-invocable list"
    );
    assert!(
        !user_skills.iter().any(|s| s.name == "agent-only"),
        "agent-only should NOT be in user-invocable list"
    );

    let _ = std::fs::remove_dir_all(&tmp);
}
