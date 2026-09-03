#![allow(clippy::assert_is_empty)]
//! Tests for `SkillRegistry::catalog()` (T-007).

use ragent_agent::skill::{SkillCatalogEntry, SkillInfo, SkillRegistry, SkillScope};

#[test]
fn test_catalog_empty_registry() {
    let registry = SkillRegistry::new();
    assert!(registry.catalog().is_empty());
}

#[test]
fn test_catalog_derives_from_metadata_without_bodies() {
    let mut registry = SkillRegistry::new();

    let mut deploy = SkillInfo::new(
        "deploy",
        "Full deploy body that should not appear in catalog",
    );
    deploy.description = Some("Deploy the application".to_string());
    deploy.trigger = Some("/deploy".to_string());
    deploy.scope = SkillScope::Project;
    registry.register(deploy);

    let mut test = SkillInfo::new("test", "Test body");
    test.description = Some("Run tests".to_string());
    // No explicit trigger -> should fall back to name
    test.scope = SkillScope::Personal;
    registry.register(test);

    let catalog = registry.catalog();
    assert_eq!(catalog.len(), 2);

    assert_eq!(
        catalog[0],
        SkillCatalogEntry {
            name: "deploy".to_string(),
            description: "Deploy the application".to_string(),
            trigger: "/deploy".to_string(),
            scope: SkillScope::Project,
            user_invocable: true,
            agent_invocable: true,
            argument_hint: None,
        }
    );
    assert_eq!(
        catalog[1],
        SkillCatalogEntry {
            name: "test".to_string(),
            description: "Run tests".to_string(),
            trigger: "test".to_string(),
            scope: SkillScope::Personal,
            user_invocable: true,
            agent_invocable: true,
            argument_hint: None,
        }
    );
}

#[test]
fn test_catalog_sorted_by_name() {
    let mut registry = SkillRegistry::new();
    for name in ["charlie", "alpha", "bravo"] {
        let mut skill = SkillInfo::new(name, "body");
        skill.scope = SkillScope::Project;
        registry.register(skill);
    }

    let catalog = registry.catalog();
    let names: Vec<String> = catalog.iter().map(|e| e.name.clone()).collect();
    assert_eq!(
        names,
        vec![
            "alpha".to_string(),
            "bravo".to_string(),
            "charlie".to_string()
        ]
    );
}

#[test]
fn test_catalog_higher_scope_wins() {
    let mut registry = SkillRegistry::new();

    let mut bundled = SkillInfo::new("deploy", "bundled body");
    bundled.scope = SkillScope::Bundled;
    bundled.description = Some("Bundled deploy".to_string());
    bundled.trigger = Some("/deploy".to_string());
    registry.register(bundled);

    let mut project = SkillInfo::new("deploy", "project body");
    project.scope = SkillScope::Project;
    project.description = Some("Project deploy".to_string());
    project.trigger = Some("/ship".to_string());
    registry.register(project);

    let entries = registry.catalog();
    assert_eq!(entries.len(), 1);
    assert_eq!(
        entries[0],
        SkillCatalogEntry {
            name: "deploy".to_string(),
            description: "Project deploy".to_string(),
            trigger: "/ship".to_string(),
            scope: SkillScope::Project,
            user_invocable: true,
            agent_invocable: true,
            argument_hint: None,
        }
    );
}

#[test]
fn test_catalog_default_description_empty() {
    let mut registry = SkillRegistry::new();
    let mut skill = SkillInfo::new("no-desc", "body");
    skill.scope = SkillScope::Bundled;
    registry.register(skill);

    let entry = &registry.catalog()[0];
    assert_eq!(entry.description, "");
    assert_eq!(entry.trigger, "no-desc");
}

#[test]
fn test_catalog_does_not_include_bodies() {
    let mut registry = SkillRegistry::new();
    let mut skill = SkillInfo::new("secret", "classified instructions");
    skill.description = Some("A skill".to_string());
    skill.scope = SkillScope::Project;
    registry.register(skill);

    let serialized = serde_json::to_string(&registry.catalog()).unwrap();
    assert!(!serialized.contains("classified instructions"));
    assert!(serialized.contains("A skill"));
}

#[test]
fn test_catalog_does_not_touch_skill_body_files() {
    let tmp = std::env::temp_dir().join("ragent_test_catalog_no_touch");
    let _ = std::fs::remove_dir_all(&tmp);

    let secret_dir = tmp.join(".ragent").join("skills").join("secret");
    std::fs::create_dir_all(&secret_dir).unwrap();
    std::fs::write(
        secret_dir.join("SKILL.md"),
        "---\ndescription: A secret skill\n---\nSUPER_SECRET_BODY_THAT_MUST_NOT_LEAK\n",
    )
    .unwrap();

    let deploy_dir = tmp.join(".ragent").join("skills").join("deploy");
    std::fs::create_dir_all(&deploy_dir).unwrap();
    std::fs::write(
        deploy_dir.join("SKILL.md"),
        "---\ndescription: Deploy app\n---\nDeploy $ARGUMENTS to production\n",
    )
    .unwrap();

    let registry = SkillRegistry::load(&tmp, &[]);

    // Spy read: delete the SKILL.md files after the registry has been built.
    // If catalog() touched the body files it would fail or read stale data.
    std::fs::remove_file(secret_dir.join("SKILL.md")).unwrap();
    std::fs::remove_file(deploy_dir.join("SKILL.md")).unwrap();

    let catalog = registry.catalog();

    let secret_entry = catalog.iter().find(|e| e.name == "secret").unwrap();
    assert_eq!(secret_entry.description, "A secret skill");
    assert!(
        !secret_entry
            .description
            .contains("SUPER_SECRET_BODY_THAT_MUST_NOT_LEAK")
    );

    let deploy_entry = catalog.iter().find(|e| e.name == "deploy").unwrap();
    assert_eq!(deploy_entry.description, "Deploy app");

    let json = serde_json::to_string(&catalog).unwrap();
    assert!(!json.contains("SUPER_SECRET_BODY_THAT_MUST_NOT_LEAK"));
    assert!(json.contains("A secret skill"));
    assert!(json.contains("Deploy app"));

    let _ = std::fs::remove_dir_all(&tmp);
}
