//! FR-005: AgentSkills pack discovery end-to-end through [`SkillRegistry::load`].
//!
//! The AgentSkills convention is a directory of packs under `.agents/skills/`
//! (project) and `~/.agents/skills/` (user), each pack a directory holding a
//! `SKILL.md` with YAML frontmatter that declares at least `name` and
//! `description`. These tests exercise the full load path (bundled, discovered,
//! dedup) and the precedence contract from assumption A7: a name clash resolves
//! in favour of the existing (higher-scope) ragent pack.

use ragent_agent::skill::{SkillRegistry, SkillScope};
use std::path::{Path, PathBuf};

/// Create a temporary working directory with a unique name and return it.
fn temp_working_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ragent_agentskills_{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp working dir");
    dir
}

/// Write a pack: `<base>/<dir_name>/SKILL.md` with the given frontmatter body.
fn write_pack(base: &Path, dir_name: &str, frontmatter: &str) {
    let pack = base.join(dir_name);
    std::fs::create_dir_all(&pack).expect("create pack dir");
    std::fs::write(pack.join("SKILL.md"), frontmatter).expect("write SKILL.md");
}

#[test]
fn test_registry_load_discovers_project_agents_skills_pack() {
    let tmp = temp_working_dir("project_pack");

    // A project AgentSkills pack with frontmatter name + description.
    write_pack(
        &tmp.join(".agents").join("skills"),
        "agentskills-tiny",
        "---\nname: agentskills-tiny\ndescription: A trivial AgentSkills pack.\n---\n\n# Tiny\n\nReply TINY-OK\n",
    );

    let registry = SkillRegistry::load(&tmp, &[]);

    let skill = registry
        .get("agentskills-tiny")
        .expect("AgentSkills pack discovered via .agents/skills/");
    assert_eq!(
        skill.description.as_deref(),
        Some("A trivial AgentSkills pack."),
        "frontmatter description must be parsed"
    );
    assert_eq!(
        skill.scope,
        SkillScope::OpenSkillsProject,
        "project .agents/skills/ loads at openskills-project scope"
    );

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_registry_load_uses_frontmatter_name_not_directory_name() {
    let tmp = temp_working_dir("frontmatter_name");

    // Directory name differs from the frontmatter `name`; the frontmatter wins.
    write_pack(
        &tmp.join(".agents").join("skills"),
        "some-dir-name",
        "---\nname: agentskills-named\ndescription: Named via frontmatter.\n---\nBody\n",
    );

    let registry = SkillRegistry::load(&tmp, &[]);

    assert!(
        registry.get("agentskills-named").is_some(),
        "frontmatter name must be the skill id"
    );
    assert!(
        registry.get("some-dir-name").is_none(),
        "directory name must not shadow the frontmatter name"
    );

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_registry_load_agents_skills_clash_prefers_existing_pack() {
    let tmp = temp_working_dir("clash");

    // Existing ragent project pack (`.ragent/skills/`, Project scope).
    write_pack(
        &tmp.join(".ragent").join("skills"),
        "agentskills-clash",
        "---\nname: agentskills-clash\ndescription: from the existing ragent pack\n---\nragent body\n",
    );
    // AgentSkills pack with the same name (`.agents/skills/`,
    // openskills-project scope) must not shadow the existing pack (A7).
    write_pack(
        &tmp.join(".agents").join("skills"),
        "agentskills-clash",
        "---\nname: agentskills-clash\ndescription: from the AgentSkills pack\n---\nagentskills body\n",
    );

    let registry = SkillRegistry::load(&tmp, &[]);

    let skill = registry
        .get("agentskills-clash")
        .expect("clash resolves to one pack");
    assert_eq!(
        skill.description.as_deref(),
        Some("from the existing ragent pack"),
        "existing ragent pack must win the name clash"
    );
    assert_eq!(
        skill.scope,
        SkillScope::Project,
        "the winning pack is the higher-scope ragent pack"
    );

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_registry_load_agents_skills_multi_target_tree() {
    let tmp = temp_working_dir("multi_pack");

    // Multiple packs under one `.agents/skills/` root each register.
    write_pack(
        &tmp.join(".agents").join("skills"),
        "agentskills-alpha",
        "---\ndescription: Alpha pack\n---\nA\n",
    );
    write_pack(
        &tmp.join(".agents").join("skills"),
        "agentskills-bravo",
        "---\ndescription: Bravo pack\n---\nB\n",
    );

    let registry = SkillRegistry::load(&tmp, &[]);

    assert_eq!(
        registry
            .get("agentskills-alpha")
            .and_then(|s| s.description.clone()),
        Some("Alpha pack".to_string())
    );
    assert_eq!(
        registry
            .get("agentskills-bravo")
            .and_then(|s| s.description.clone()),
        Some("Bravo pack".to_string())
    );

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_registry_load_skips_agents_skills_pack_without_skill_md() {
    let tmp = temp_working_dir("no_skill_md");

    // A directory under `.agents/skills/` with no `SKILL.md` contributes nothing.
    let empty_pack = tmp.join(".agents").join("skills").join("not-a-pack");
    std::fs::create_dir_all(&empty_pack).expect("create empty pack dir");
    write_pack(
        &tmp.join(".agents").join("skills"),
        "agentskills-real",
        "---\ndescription: Real pack\n---\nBody\n",
    );

    let registry = SkillRegistry::load(&tmp, &[]);

    assert!(registry.get("agentskills-real").is_some());
    assert!(
        registry.get("not-a-pack").is_none(),
        "a directory without SKILL.md is not a pack"
    );

    let _ = std::fs::remove_dir_all(&tmp);
}

#[tokio::test]
async fn test_agentskills_pack_is_loadable_and_invocable() {
    let tmp = temp_working_dir("invoke");

    // The project AgentSkills pack from the manual test plan (TC-011).
    write_pack(
        &tmp.join(".agents").join("skills"),
        "tiny",
        "---\nname: tiny\ndescription: A trivial pack used by the manual test plan.\n---\n\n# Tiny skill\n\nWhen asked, reply with exactly: TINY-OK\n",
    );

    let registry = SkillRegistry::load(&tmp, &[]);
    let skill = registry
        .get("tiny")
        .expect("AgentSkills pack is discoverable by frontmatter name")
        .clone();
    assert_eq!(skill.scope, SkillScope::OpenSkillsProject);

    // Loading the pack (as `/skill tiny` and the `/tiny` trigger both do)
    // yields the pack's instructions, ready for injection into the session.
    let invocation = ragent_agent::skill::invoke::invoke_skill(&skill, "", "sess-1", &tmp)
        .await
        .expect("the AgentSkills pack body must load on demand");
    assert!(
        invocation.content.contains("reply with exactly: TINY-OK"),
        "the pack's instructions must be the invocation content, got: {}",
        invocation.content
    );

    let _ = std::fs::remove_dir_all(&tmp);
}
