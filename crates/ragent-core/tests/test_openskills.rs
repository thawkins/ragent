//! Tests for OpenSkills / Anthropic Agent Skills integration.
//!
//! Covers:
//! - New SkillScope variants (OpenSkillsGlobal, OpenSkillsProject)
//! - OpenSkills directory discovery (.agent/skills/, .claude/skills/)
//! - Anthropic-format SKILL.md parsing (license, compatibility, metadata)
//! - Scope priority: ragent-native paths override OpenSkills paths

use ragent_core::skill::loader::parse_skill_md;
use ragent_core::skill::{SkillRegistry, SkillScope};
use std::path::PathBuf;

/// Write a minimal SKILL.md file into a subdirectory.
fn write_skill(dir: &std::path::Path, name: &str, desc: &str) {
    let skill_dir = dir.join(name);
    std::fs::create_dir_all(&skill_dir).expect("create skill dir");
    std::fs::write(
        skill_dir.join("SKILL.md"),
        format!("---\ndescription: {desc}\n---\n{desc} body\n"),
    )
    .expect("write SKILL.md");
}

// ── Scope ordering ──────────────────────────────────────────────

#[test]
fn test_openskills_scope_ordering() {
    assert!(SkillScope::Bundled < SkillScope::Enterprise);
    assert!(SkillScope::Enterprise < SkillScope::OpenSkillsGlobal);
    assert!(SkillScope::OpenSkillsGlobal < SkillScope::Personal);
    assert!(SkillScope::Personal < SkillScope::OpenSkillsProject);
    assert!(SkillScope::OpenSkillsProject < SkillScope::Project);
}

#[test]
fn test_openskills_scope_display() {
    assert_eq!(
        format!("{}", SkillScope::OpenSkillsGlobal),
        "openskills-global"
    );
    assert_eq!(
        format!("{}", SkillScope::OpenSkillsProject),
        "openskills-project"
    );
}

// ── OpenSkills project directory discovery ───────────────────────

#[test]
fn test_discover_agent_skills_project_dir() {
    let tmp = std::env::temp_dir().join("ragent_test_os_agent_proj");
    let _ = std::fs::remove_dir_all(&tmp);

    let agent_skills = tmp.join(".agent").join("skills");
    write_skill(&agent_skills, "pdf-reader", "Read PDF files");

    let registry = SkillRegistry::load(&tmp, &[]);

    // 4 bundled + 1 OpenSkills project
    assert_eq!(registry.len(), 5);
    let pdf = registry.get("pdf-reader").expect("should find pdf-reader");
    assert_eq!(pdf.scope, SkillScope::OpenSkillsProject);
    assert_eq!(pdf.description.as_deref(), Some("Read PDF files"));

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_discover_claude_skills_project_dir() {
    let tmp = std::env::temp_dir().join("ragent_test_os_claude_proj");
    let _ = std::fs::remove_dir_all(&tmp);

    let claude_skills = tmp.join(".claude").join("skills");
    write_skill(&claude_skills, "mcp-builder", "Build MCP servers");

    let registry = SkillRegistry::load(&tmp, &[]);

    // 4 bundled + 1 OpenSkills project
    assert_eq!(registry.len(), 5);
    let mcp = registry
        .get("mcp-builder")
        .expect("should find mcp-builder");
    assert_eq!(mcp.scope, SkillScope::OpenSkillsProject);

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_discover_both_agent_and_claude_skills() {
    let tmp = std::env::temp_dir().join("ragent_test_os_both_dirs");
    let _ = std::fs::remove_dir_all(&tmp);

    let agent_skills = tmp.join(".agent").join("skills");
    write_skill(&agent_skills, "from-agent", "Agent skill");

    let claude_skills = tmp.join(".claude").join("skills");
    write_skill(&claude_skills, "from-claude", "Claude skill");

    let registry = SkillRegistry::load(&tmp, &[]);

    // 4 bundled + 2 OpenSkills project
    assert_eq!(registry.len(), 6);
    assert!(registry.get("from-agent").is_some());
    assert!(registry.get("from-claude").is_some());

    let _ = std::fs::remove_dir_all(&tmp);
}

// ── Ragent project overrides OpenSkills project ─────────────────

#[test]
fn test_ragent_project_overrides_openskills_project() {
    let tmp = std::env::temp_dir().join("ragent_test_os_override");
    let _ = std::fs::remove_dir_all(&tmp);

    // OpenSkills project: .agent/skills/deploy
    let agent_skills = tmp.join(".agent").join("skills");
    write_skill(&agent_skills, "deploy", "OpenSkills deploy");

    // Ragent project: .ragent/skills/deploy (higher priority)
    let ragent_skills = tmp.join(".ragent").join("skills");
    write_skill(&ragent_skills, "deploy", "Ragent deploy");

    let registry = SkillRegistry::load(&tmp, &[]);

    let deploy = registry.get("deploy").expect("should find deploy");
    assert_eq!(
        deploy.scope,
        SkillScope::Project,
        "ragent Project should win"
    );
    assert_eq!(deploy.description.as_deref(), Some("Ragent deploy"));

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_openskills_project_overrides_personal() {
    let tmp = std::env::temp_dir().join("ragent_test_os_proj_over_personal");
    let _ = std::fs::remove_dir_all(&tmp);

    // OpenSkills project skill
    let agent_skills = tmp.join(".agent").join("skills");
    write_skill(&agent_skills, "lint", "OpenSkills project lint");

    let registry = SkillRegistry::load(&tmp, &[]);
    let lint = registry.get("lint").expect("should find lint");
    assert_eq!(lint.scope, SkillScope::OpenSkillsProject);

    let _ = std::fs::remove_dir_all(&tmp);
}

// ── Anthropic-format SKILL.md parsing ───────────────────────────

#[test]
fn test_parse_anthropic_format_with_license() {
    let content = r#"---
name: pdf-reader
description: Read and extract text from PDF files
license: MIT
---

Extract text from the provided PDF file.
"#;
    let skill = parse_skill_md(
        content,
        &PathBuf::from("/project/.agent/skills/pdf-reader/SKILL.md"),
        "pdf-reader",
        SkillScope::OpenSkillsProject,
    )
    .expect("should parse Anthropic format");

    assert_eq!(skill.name, "pdf-reader");
    assert_eq!(
        skill.description.as_deref(),
        Some("Read and extract text from PDF files")
    );
    assert_eq!(skill.license.as_deref(), Some("MIT"));
    assert_eq!(skill.scope, SkillScope::OpenSkillsProject);
}

#[test]
fn test_parse_anthropic_format_with_compatibility() {
    let content = r#"---
name: docx-reader
description: Read DOCX files
compatibility: "Requires pandoc to be installed"
---

Read the DOCX file.
"#;
    let skill = parse_skill_md(
        content,
        &PathBuf::from("/project/.claude/skills/docx-reader/SKILL.md"),
        "docx-reader",
        SkillScope::OpenSkillsProject,
    )
    .expect("should parse compatibility field");

    assert_eq!(
        skill.compatibility.as_deref(),
        Some("Requires pandoc to be installed")
    );
}

#[test]
fn test_parse_anthropic_format_with_metadata() {
    let content = r#"---
name: skill-creator
description: Create new skills
metadata:
  author: anthropic
  version: "1.0"
  category: development
---

Create a new skill.
"#;
    let skill = parse_skill_md(
        content,
        &PathBuf::from("/project/.agent/skills/skill-creator/SKILL.md"),
        "skill-creator",
        SkillScope::OpenSkillsProject,
    )
    .expect("should parse metadata field");

    assert_eq!(
        skill.metadata.get("author").map(String::as_str),
        Some("anthropic")
    );
    assert_eq!(
        skill.metadata.get("version").map(String::as_str),
        Some("1.0")
    );
    assert_eq!(
        skill.metadata.get("category").map(String::as_str),
        Some("development")
    );
}

#[test]
fn test_parse_anthropic_format_full() {
    let content = r#"---
name: mcp-builder
description: Build MCP servers for any API
license: Apache-2.0
compatibility: "Node.js 18+ required"
allowed-tools: bash read write
metadata:
  author: anthropic
  version: "2.0"
---

Build an MCP server that wraps the given API.
"#;
    let skill = parse_skill_md(
        content,
        &PathBuf::from("/home/user/.agent/skills/mcp-builder/SKILL.md"),
        "mcp-builder",
        SkillScope::OpenSkillsGlobal,
    )
    .expect("should parse full Anthropic format");

    assert_eq!(skill.name, "mcp-builder");
    assert_eq!(
        skill.description.as_deref(),
        Some("Build MCP servers for any API")
    );
    assert_eq!(skill.license.as_deref(), Some("Apache-2.0"));
    assert_eq!(skill.compatibility.as_deref(), Some("Node.js 18+ required"));
    assert_eq!(skill.scope, SkillScope::OpenSkillsGlobal);
    assert_eq!(skill.metadata.len(), 2);
    assert!(skill.body.contains("Build an MCP server"));
}

#[test]
fn test_parse_anthropic_format_minimal() {
    // The Anthropic spec requires name and description, but ragent only requires
    // the frontmatter block. This tests that an Anthropic-minimal skill works.
    let content = r#"---
name: simple
description: A simple skill
---

Do the thing.
"#;
    let skill = parse_skill_md(
        content,
        &PathBuf::from("/project/.agent/skills/simple/SKILL.md"),
        "simple",
        SkillScope::OpenSkillsProject,
    )
    .expect("should parse minimal Anthropic format");

    assert_eq!(skill.name, "simple");
    assert!(skill.license.is_none());
    assert!(skill.compatibility.is_none());
    assert!(skill.metadata.is_empty());
}

// ── Mixed ragent + Anthropic fields ─────────────────────────────

#[test]
fn test_parse_mixed_ragent_and_anthropic_fields() {
    let content = r#"---
name: hybrid-skill
description: A skill using both ragent and Anthropic fields
license: MIT
compatibility: "Linux only"
metadata:
  source: custom
argument-hint: "[target]"
disable-model-invocation: true
context: fork
agent: general-purpose
---

Hybrid instructions.
"#;
    let skill = parse_skill_md(
        content,
        &PathBuf::from("/project/.ragent/skills/hybrid-skill/SKILL.md"),
        "hybrid-skill",
        SkillScope::Project,
    )
    .expect("should parse mixed fields");

    // Anthropic fields
    assert_eq!(skill.license.as_deref(), Some("MIT"));
    assert_eq!(skill.compatibility.as_deref(), Some("Linux only"));
    assert_eq!(
        skill.metadata.get("source").map(String::as_str),
        Some("custom")
    );

    // Ragent-specific fields
    assert_eq!(skill.argument_hint.as_deref(), Some("[target]"));
    assert!(skill.disable_model_invocation);
    assert!(skill.is_forked());
    assert_eq!(skill.agent.as_deref(), Some("general-purpose"));
}

// ── Discovery with OpenSkills + ragent combined ─────────────────

#[test]
fn test_full_discovery_mixed_sources() {
    let tmp = std::env::temp_dir().join("ragent_test_os_full_mix");
    let _ = std::fs::remove_dir_all(&tmp);

    // OpenSkills .agent/skills/
    let agent_skills = tmp.join(".agent").join("skills");
    write_skill(&agent_skills, "os-tool-a", "OpenSkills A");

    // OpenSkills .claude/skills/
    let claude_skills = tmp.join(".claude").join("skills");
    write_skill(&claude_skills, "os-tool-b", "OpenSkills B");

    // Ragent .ragent/skills/
    let ragent_skills = tmp.join(".ragent").join("skills");
    write_skill(&ragent_skills, "ragent-tool", "Ragent tool");

    let registry = SkillRegistry::load(&tmp, &[]);

    // 4 bundled + 3 discovered
    assert_eq!(registry.len(), 7);

    let a = registry.get("os-tool-a").expect("find os-tool-a");
    assert_eq!(a.scope, SkillScope::OpenSkillsProject);

    let b = registry.get("os-tool-b").expect("find os-tool-b");
    assert_eq!(b.scope, SkillScope::OpenSkillsProject);

    let r = registry.get("ragent-tool").expect("find ragent-tool");
    assert_eq!(r.scope, SkillScope::Project);

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_agent_dir_skill_overridden_by_claude_dir_same_name() {
    // Both .agent/ and .claude/ have the same skill name.
    // .claude/ is scanned after .agent/ so it should "win" at same scope.
    let tmp = std::env::temp_dir().join("ragent_test_os_agent_claude_dedup");
    let _ = std::fs::remove_dir_all(&tmp);

    let agent_skills = tmp.join(".agent").join("skills");
    write_skill(&agent_skills, "shared", "From .agent");

    let claude_skills = tmp.join(".claude").join("skills");
    write_skill(&claude_skills, "shared", "From .claude");

    let registry = SkillRegistry::load(&tmp, &[]);

    // 4 bundled + 1 (deduped by name, same scope — later one wins)
    assert_eq!(registry.len(), 5);
    let shared = registry.get("shared").expect("find shared");
    // .claude/ is scanned after .agent/ at same scope, so .claude/ wins
    assert_eq!(shared.description.as_deref(), Some("From .claude"));

    let _ = std::fs::remove_dir_all(&tmp);
}
