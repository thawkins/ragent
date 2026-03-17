//! Skill discovery and YAML frontmatter parsing.
//!
//! Skills are defined as `SKILL.md` files with YAML frontmatter delimited
//! by `---`. This module handles parsing those files into [`SkillInfo`]
//! structs and discovering them from the filesystem.
//!
//! # Frontmatter Format
//!
//! ```yaml
//! ---
//! name: deploy
//! description: Deploy the application to production
//! disable-model-invocation: true
//! allowed-tools: bash
//! context: fork
//! agent: general-purpose
//! argument-hint: "[environment]"
//! ---
//!
//! Deploy $ARGUMENTS to production...
//! ```

use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

use super::{SkillContext, SkillInfo, SkillScope};

/// Intermediate representation of the YAML frontmatter in a `SKILL.md` file.
///
/// Field names use kebab-case to match the SPEC-defined frontmatter format.
/// Includes fields from the Anthropic Agent Skills specification for
/// OpenSkills compatibility (license, compatibility, metadata).
#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct SkillFrontmatter {
    /// Display name (defaults to directory name if absent).
    name: Option<String>,
    /// What the skill does; used for auto-invocation matching.
    description: Option<String>,
    /// Hint shown during autocomplete (e.g. `"[issue-number]"`).
    #[serde(rename = "argument-hint")]
    argument_hint: Option<String>,
    /// If `true`, only the user can invoke via `/name`.
    #[serde(rename = "disable-model-invocation", default)]
    disable_model_invocation: bool,
    /// If `false`, hidden from `/` menu; only the agent can invoke.
    #[serde(rename = "user-invocable", default = "default_true")]
    user_invocable: bool,
    /// Tools the agent can use without permission when skill is active.
    /// Can be a single string or a list.
    #[serde(rename = "allowed-tools", default)]
    allowed_tools: AllowedTools,
    /// Override model when this skill is active.
    model: Option<String>,
    /// Execution context mode (`"fork"`).
    context: Option<SkillContext>,
    /// Subagent type when `context` is `fork`.
    agent: Option<String>,
    /// Hooks scoped to this skill's lifecycle (raw YAML, stored as JSON).
    hooks: Option<serde_yaml::Value>,
    /// License information (Anthropic Agent Skills spec).
    license: Option<String>,
    /// Environment compatibility notes (Anthropic Agent Skills spec).
    compatibility: Option<String>,
    /// Arbitrary key-value metadata (Anthropic Agent Skills spec).
    #[serde(default)]
    metadata: HashMap<String, String>,
}

fn default_true() -> bool {
    true
}

/// Handles the `allowed-tools` field which can be a single string or a list.
#[derive(Debug, Deserialize, Default)]
#[serde(untagged)]
enum AllowedTools {
    /// A single tool name (e.g. `allowed-tools: bash`).
    Single(String),
    /// A list of tool names (e.g. `allowed-tools: [bash, read, write]`).
    Multiple(Vec<String>),
    /// No tools specified.
    #[default]
    None,
}

impl AllowedTools {
    fn into_vec(self) -> Vec<String> {
        match self {
            AllowedTools::Single(s) => vec![s],
            AllowedTools::Multiple(v) => v,
            AllowedTools::None => Vec::new(),
        }
    }
}

/// Parse a `SKILL.md` file's content into a [`SkillInfo`].
///
/// The file must have YAML frontmatter delimited by `---` lines. Everything
/// after the closing `---` is treated as the markdown body.
///
/// # Arguments
///
/// * `content` — Raw text content of the `SKILL.md` file.
/// * `source_path` — Absolute path to the `SKILL.md` file (for metadata).
/// * `dir_name` — Name of the skill directory (used as fallback name).
/// * `scope` — The scope from which this skill was discovered.
///
/// # Errors
///
/// Returns an error if the frontmatter is missing or cannot be parsed as YAML.
///
/// # Examples
///
/// ```
/// use ragent_core::skill::loader::parse_skill_md;
/// use ragent_core::skill::SkillScope;
/// use std::path::PathBuf;
///
/// let content = r#"---
/// name: deploy
/// description: Deploy the application
/// context: fork
/// agent: general-purpose
/// ---
///
/// Deploy $ARGUMENTS to production.
/// "#;
///
/// let skill = parse_skill_md(content, &PathBuf::from("/project/.ragent/skills/deploy/SKILL.md"), "deploy", SkillScope::Project).unwrap();
/// assert_eq!(skill.name, "deploy");
/// assert_eq!(skill.description.as_deref(), Some("Deploy the application"));
/// assert!(skill.is_forked());
/// assert_eq!(skill.body.trim(), "Deploy $ARGUMENTS to production.");
/// ```
pub fn parse_skill_md(
    content: &str,
    source_path: &Path,
    dir_name: &str,
    scope: SkillScope,
) -> anyhow::Result<SkillInfo> {
    let (frontmatter_str, body) = split_frontmatter(content)?;

    let frontmatter: SkillFrontmatter = serde_yaml::from_str(frontmatter_str)
        .map_err(|e| anyhow::anyhow!("Failed to parse SKILL.md frontmatter: {e}"))?;

    let skill_dir = source_path
        .parent()
        .unwrap_or(Path::new(""))
        .to_path_buf();

    // Name defaults to the directory name if not specified in frontmatter
    let name = frontmatter
        .name
        .unwrap_or_else(|| dir_name.to_string());

    // Validate name: lowercase, hyphens, max 64 chars
    validate_skill_name(&name)?;

    // Convert hooks from YAML to JSON for storage
    let hooks_json = frontmatter
        .hooks
        .map(|yaml_val| yaml_to_json(&yaml_val))
        .transpose()?;

    Ok(SkillInfo {
        name,
        description: frontmatter.description,
        argument_hint: frontmatter.argument_hint,
        disable_model_invocation: frontmatter.disable_model_invocation,
        user_invocable: frontmatter.user_invocable,
        allowed_tools: frontmatter.allowed_tools.into_vec(),
        model: frontmatter.model,
        context: frontmatter.context,
        agent: frontmatter.agent,
        hooks: hooks_json,
        license: frontmatter.license,
        compatibility: frontmatter.compatibility,
        metadata: frontmatter.metadata,
        source_path: source_path.to_path_buf(),
        skill_dir,
        scope,
        body: body.to_string(),
    })
}

/// Split a SKILL.md file into frontmatter and body.
///
/// Expects the file to start with `---`, followed by YAML, then a closing
/// `---` line. Returns `(frontmatter, body)`.
fn split_frontmatter(content: &str) -> anyhow::Result<(&str, &str)> {
    let trimmed = content.trim_start();

    if !trimmed.starts_with("---") {
        return Err(anyhow::anyhow!(
            "SKILL.md must start with YAML frontmatter delimited by ---"
        ));
    }

    // Find the opening delimiter
    let after_first = &trimmed[3..];
    let after_first = after_first.strip_prefix('\n').unwrap_or(
        after_first.strip_prefix("\r\n").unwrap_or(after_first),
    );

    // Find the closing delimiter
    let closing_pos = find_closing_delimiter(after_first).ok_or_else(|| {
        anyhow::anyhow!("SKILL.md frontmatter is missing closing --- delimiter")
    })?;

    let frontmatter = &after_first[..closing_pos];
    let rest = &after_first[closing_pos + 3..];
    // Strip the newline immediately after closing ---
    let body = rest
        .strip_prefix('\n')
        .unwrap_or(rest.strip_prefix("\r\n").unwrap_or(rest));

    Ok((frontmatter, body))
}

/// Find the byte offset of a closing `---` delimiter that appears at the
/// start of a line.
fn find_closing_delimiter(text: &str) -> Option<usize> {
    let mut offset = 0;
    for line in text.lines() {
        if line.trim() == "---" {
            return Some(offset);
        }
        // +1 for the newline character (handles \n; \r\n is fine since we
        // compare trimmed lines)
        offset += line.len() + 1;
    }
    None
}

/// Validate that a skill name follows the naming rules.
fn validate_skill_name(name: &str) -> anyhow::Result<()> {
    if name.is_empty() {
        return Err(anyhow::anyhow!("Skill name cannot be empty"));
    }
    if name.len() > 64 {
        return Err(anyhow::anyhow!(
            "Skill name '{}' exceeds 64 characters",
            name
        ));
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(anyhow::anyhow!(
            "Skill name '{}' must contain only lowercase letters, digits, and hyphens",
            name
        ));
    }
    Ok(())
}

/// Convert a `serde_yaml::Value` to a `serde_json::Value`.
fn yaml_to_json(yaml: &serde_yaml::Value) -> anyhow::Result<serde_json::Value> {
    // Round-trip through string serialization for correctness
    let json_str = serde_json::to_string(&yaml)?;
    let json_val: serde_json::Value = serde_json::from_str(&json_str)?;
    Ok(json_val)
}

/// Discover skills from standard locations and optional extra directories.
///
/// Scans in order (lowest → highest priority):
///
/// 1. OpenSkills global: `~/.agent/skills/*/SKILL.md`, `~/.claude/skills/*/SKILL.md`
/// 2. Personal: `~/.ragent/skills/*/SKILL.md`
/// 3. Extra directories from config `skill_dirs` (treated as Personal scope)
/// 4. OpenSkills project: `{working_dir}/.agent/skills/*/SKILL.md`, `{working_dir}/.claude/skills/*/SKILL.md`
/// 5. Project: `{working_dir}/.ragent/skills/*/SKILL.md`
/// 6. Monorepo: nested `.ragent/skills/` in subdirectories of `working_dir`
///
/// Higher-scope skills override lower-scope skills when names conflict.
/// Ragent-native paths always take precedence over OpenSkills paths at the
/// same level (global or project).
///
/// Individual parse failures are logged as warnings and skipped.
pub fn discover_skills(working_dir: &Path, extra_dirs: &[String]) -> Vec<SkillInfo> {
    let mut skills = Vec::new();

    // OpenSkills global: ~/.agent/skills/ and ~/.claude/skills/
    if let Some(home) = dirs::home_dir() {
        for dir_name in &[".agent", ".claude"] {
            let openskills_dir = home.join(dir_name).join("skills");
            if openskills_dir.is_dir() {
                load_skills_from_dir(&openskills_dir, SkillScope::OpenSkillsGlobal, &mut skills);
            }
        }
    }

    // Personal skills: ~/.ragent/skills/*/SKILL.md
    if let Some(home) = dirs::home_dir() {
        let personal_dir = home.join(".ragent").join("skills");
        if personal_dir.is_dir() {
            load_skills_from_dir(&personal_dir, SkillScope::Personal, &mut skills);
        }
    }

    // Extra directories from config (treated as Personal scope so project
    // skills can still override them)
    for dir in extra_dirs {
        let path = Path::new(dir);
        if path.is_dir() {
            load_skills_from_dir(path, SkillScope::Personal, &mut skills);
        } else {
            tracing::warn!("Configured skill_dirs entry does not exist: {dir}");
        }
    }

    // OpenSkills project: .agent/skills/ and .claude/skills/
    for dir_name in &[".agent", ".claude"] {
        let openskills_dir = working_dir.join(dir_name).join("skills");
        if openskills_dir.is_dir() {
            load_skills_from_dir(&openskills_dir, SkillScope::OpenSkillsProject, &mut skills);
        }
    }

    // Project skills: {working_dir}/.ragent/skills/*/SKILL.md
    let project_dir = working_dir.join(".ragent").join("skills");
    if project_dir.is_dir() {
        load_skills_from_dir(&project_dir, SkillScope::Project, &mut skills);
    }

    // Monorepo support: scan first-level subdirectories for nested .ragent/skills/
    if let Ok(entries) = std::fs::read_dir(working_dir) {
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if path.is_dir() {
                let nested_skills = path.join(".ragent").join("skills");
                if nested_skills.is_dir() {
                    load_skills_from_dir(&nested_skills, SkillScope::Project, &mut skills);
                }
            }
        }
    }

    skills
}

/// Load all skills from a directory containing skill subdirectories.
///
/// Each subdirectory must contain a `SKILL.md` file. Subdirectories without
/// `SKILL.md` are silently skipped.
fn load_skills_from_dir(skills_dir: &Path, scope: SkillScope, out: &mut Vec<SkillInfo>) {
    let entries = match std::fs::read_dir(skills_dir) {
        Ok(entries) => entries,
        Err(e) => {
            tracing::warn!("Failed to read skills directory {}: {e}", skills_dir.display());
            return;
        }
    };

    for entry in entries.filter_map(Result::ok) {
        let skill_dir = entry.path();
        if !skill_dir.is_dir() {
            continue;
        }

        let skill_md = skill_dir.join("SKILL.md");
        if !skill_md.is_file() {
            continue;
        }

        let dir_name = skill_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        match std::fs::read_to_string(&skill_md) {
            Ok(content) => match parse_skill_md(&content, &skill_md, dir_name, scope) {
                Ok(skill) => {
                    tracing::debug!(
                        "Loaded skill '{}' from {} (scope: {})",
                        skill.name,
                        skill_md.display(),
                        scope
                    );
                    out.push(skill);
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to parse {}: {e}",
                        skill_md.display()
                    );
                }
            },
            Err(e) => {
                tracing::warn!(
                    "Failed to read {}: {e}",
                    skill_md.display()
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
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
        assert!(skill.allowed_tools.is_empty());
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
        assert!(result.unwrap_err().to_string().contains("exceeds 64 characters"));
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

        assert!(skill.body.is_empty());
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
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "deploy");
        assert_eq!(skills[0].scope, SkillScope::Project);
        assert_eq!(skills[0].description.as_deref(), Some("Deploy app"));
        assert!(skills[0].is_forked());
        assert_eq!(skills[0].agent.as_deref(), Some("general-purpose"));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_discover_skills_empty_working_dir() {
        let tmp = std::env::temp_dir().join("ragent_test_discover_empty");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).expect("create temp dir");

        let skills = discover_skills(&tmp, &[]);
        assert!(skills.is_empty());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_discover_skills_nonexistent_dir() {
        let skills = discover_skills(Path::new("/nonexistent/path/that/should/not/exist"), &[]);
        assert!(skills.is_empty());
    }

    #[test]
    fn test_discover_skills_monorepo_nested() {
        let tmp = std::env::temp_dir().join("ragent_test_discover_monorepo");
        let _ = std::fs::remove_dir_all(&tmp);

        // Subdirectory with its own .ragent/skills/
        let nested_dir = tmp.join("backend").join(".ragent").join("skills").join("api-test");
        std::fs::create_dir_all(&nested_dir).expect("create nested skill dir");
        std::fs::write(
            nested_dir.join("SKILL.md"),
            "---\ndescription: Run API tests\n---\nTest the API\n",
        )
        .expect("write nested SKILL.md");

        let skills = discover_skills(&tmp, &[]);
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "api-test");
        assert_eq!(skills[0].scope, SkillScope::Project);

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
        assert_eq!(skills.len(), 3);

        let names: Vec<&str> = skills.iter().map(|s| s.name.as_str()).collect();
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
        std::fs::write(skills_dir.join("not-a-dir.md"), "---\n---\nBody\n")
            .expect("write file");

        // Create a valid skill directory
        let valid_dir = skills_dir.join("valid");
        std::fs::create_dir_all(&valid_dir).expect("create valid dir");
        std::fs::write(valid_dir.join("SKILL.md"), "---\n---\nBody\n")
            .expect("write SKILL.md");

        let skills = discover_skills(&tmp, &[]);
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "valid");

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
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "deploy");
        // Verify the skill_dir points to the skill directory (containing scripts/, templates/)
        assert!(skills[0].skill_dir.ends_with("deploy"));

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

        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "custom-lint");
        assert_eq!(skills[0].description.as_deref(), Some("Custom linter"));
        assert_eq!(skills[0].scope, SkillScope::Personal);

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
        assert!(skills.is_empty());

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
