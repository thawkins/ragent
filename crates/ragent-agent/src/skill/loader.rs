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
/// `OpenSkills` compatibility (license, compatibility, metadata).
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
    /// Whether this skill allows `!`command`` dynamic context injection.
    #[serde(rename = "allow-dynamic-context", default)]
    allow_dynamic_context: bool,
    /// Trigger phrase for invoking the skill (e.g. `/deploy`).
    trigger: Option<String>,
}
const fn default_true() -> bool {
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
            Self::Single(s) => vec![s],
            Self::Multiple(v) => v,
            Self::None => Vec::new(),
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
/// use ragent_agent::skill::loader::parse_skill_md;
/// use ragent_agent::skill::SkillScope;
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
    parse_skill_md_inner(content, source_path, dir_name, scope, true)
}

/// Parse a `SKILL.md` file's metadata without loading the markdown body.
///
/// This is used by skill discovery so that the registry can hold a compact
/// catalog of skills without reading the full instructions into memory. The
/// body can be loaded later via [`SkillInfo::body_or_load`].
pub(crate) fn parse_skill_md_metadata(
    content: &str,
    source_path: &Path,
    dir_name: &str,
    scope: SkillScope,
) -> anyhow::Result<SkillInfo> {
    parse_skill_md_inner(content, source_path, dir_name, scope, false)
}

pub(crate) fn parse_skill_md_inner(
    content: &str,
    source_path: &Path,
    dir_name: &str,
    scope: SkillScope,
    include_body: bool,
) -> anyhow::Result<SkillInfo> {
    let (frontmatter_str, body) = split_frontmatter(content)?;

    let frontmatter: SkillFrontmatter = serde_yaml::from_str(frontmatter_str)
        .map_err(|e| anyhow::anyhow!("Failed to parse SKILL.md frontmatter: {e}"))?;

    let skill_dir = source_path.parent().unwrap_or(Path::new("")).to_path_buf();

    // Name defaults to the directory name if not specified in frontmatter
    let name = frontmatter.name.unwrap_or_else(|| dir_name.to_string());

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
        trigger: frontmatter.trigger,
        allow_dynamic_context: frontmatter.allow_dynamic_context,
        source_path: source_path.to_path_buf(),
        skill_dir,
        scope,
        body: if include_body {
            body.to_string()
        } else {
            String::new()
        },
        body_cache: super::default_body_cache(),
    })
}

/// Extract the markdown body from the content of a `SKILL.md` file.
///
/// Skips the YAML frontmatter delimited by `---` lines and returns only the
/// markdown instructions.
pub(crate) fn extract_body(content: &str) -> anyhow::Result<&str> {
    split_frontmatter(content).map(|(_, body)| body)
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
    let after_first = after_first
        .strip_prefix('\n')
        .unwrap_or(after_first.strip_prefix("\r\n").unwrap_or(after_first));

    // Find the closing delimiter
    let closing_pos = find_closing_delimiter(after_first)
        .ok_or_else(|| anyhow::anyhow!("SKILL.md frontmatter is missing closing --- delimiter"))?;

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
        return Err(anyhow::anyhow!("Skill name '{name}' exceeds 64 characters"));
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(anyhow::anyhow!(
            "Skill name '{name}' must contain only lowercase letters, digits, and hyphens"
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
/// 1. `OpenSkills` global: `~/.agent/skills/*/SKILL.md`, `~/.claude/skills/*/SKILL.md`
/// 2. Personal: `~/.ragent/skills/*/SKILL.md`
/// 3. Extra directories from config `skill_dirs` (treated as Personal scope)
/// 4. `OpenSkills` project: `{working_dir}/.agent/skills/*/SKILL.md`, `{working_dir}/.claude/skills/*/SKILL.md`
/// 5. Project: `{working_dir}/.ragent/skills/*/SKILL.md`
/// 6. Monorepo: nested `.ragent/skills/` in subdirectories of `working_dir`
///
/// Higher-scope skills override lower-scope skills when names conflict.
/// Ragent-native paths always take precedence over `OpenSkills` paths at the
/// same level (global or project).
///
/// Individual parse failures are logged as warnings and skipped.
///
/// # Errors
///
/// This function does not return errors. File system errors and parse failures
/// are logged as warnings and the corresponding skills are skipped. The function
/// always returns a vector, which may be empty if no valid skills are found.
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
            tracing::warn!(
                "Failed to read skills directory {}: {e}",
                skills_dir.display()
            );
            return;
        }
    };

    for entry in entries.filter_map(Result::ok) {
        let skill_path = entry.path();
        if !skill_path.is_dir() {
            continue;
        }

        let skill_md = skill_path.join("SKILL.md");
        if !skill_md.is_file() {
            continue;
        }

        let dir_name = skill_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        match std::fs::read_to_string(&skill_md) {
            Ok(content) => match parse_skill_md_metadata(&content, &skill_md, dir_name, scope) {
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
                    tracing::warn!("Failed to parse {}: {e}", skill_md.display());
                }
            },
            Err(e) => {
                tracing::warn!("Failed to read {}: {e}", skill_md.display());
            }
        }
    }
}

#[cfg(test)]
#[path = "../../tests/inline/skill_loader.rs"]
mod tests_tests;
