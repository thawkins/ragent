//! Skill discovery, loading, argument substitution, and invocation.
//!
//! Skills enhance the agent's ability to perform specialized tasks by bundling
//! instructions, scripts, and resources into reusable packages. Each skill is
//! defined by a `SKILL.md` file with YAML frontmatter for configuration and a
//! markdown body containing the instructions.
//!
//! # Skill Structure
//!
//! ```text
//! .ragent/skills/
//!   deploy/
//!     SKILL.md            # Skill instructions and frontmatter (required)
//!     scripts/            # Helper scripts the skill can invoke
//!     templates/          # Template files for the agent to fill in
//!     examples/           # Example outputs showing expected format
//!     resources/          # Reference materials
//! ```
//!
//! # Skill Scopes
//!
//! | Scope              | Path                                   | Applies To                    |
//! |--------------------|----------------------------------------|-------------------------------|
//! | Enterprise         | Managed settings                       | All users in organization     |
//! | OpenSkills Global  | `~/.agent/skills/`, `~/.claude/skills/`| OpenSkills ecosystem (global) |
//! | Personal           | `~/.ragent/skills/<name>/SKILL.md`     | All projects for this user    |
//! | OpenSkills Project | `.agent/skills/`, `.claude/skills/`    | OpenSkills ecosystem (project)|
//! | Project            | `.ragent/skills/<name>/SKILL.md`        | This project only             |
//!
//! Higher-priority scopes override lower ones when names conflict.
//! Ragent-native paths always take precedence over OpenSkills paths at
//! the same level (global or project).

pub mod args;
pub mod bundled;
pub mod context;
pub mod invoke;
pub mod loader;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Execution context mode for a skill.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SkillContext {
    /// Run in a forked subagent context with isolated conversation history.
    Fork,
}

impl std::fmt::Display for SkillContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Fork => write!(f, "fork"),
        }
    }
}

/// Resolution scope indicating where a skill was loaded from.
///
/// Higher-numbered scopes take precedence over lower ones when names conflict.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum SkillScope {
    /// Bundled with ragent (lowest priority).
    Bundled = 0,
    /// Enterprise-managed settings.
    Enterprise = 1,
    /// `OpenSkills` global directories (`~/.agent/skills/`, `~/.claude/skills/`).
    OpenSkillsGlobal = 2,
    /// User-level skill from `~/.ragent/skills/`.
    Personal = 3,
    /// `OpenSkills` project directories (`.agent/skills/`, `.claude/skills/`).
    OpenSkillsProject = 4,
    /// Project-level skill from `.ragent/skills/`.
    Project = 5,
}

impl std::fmt::Display for SkillScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bundled => write!(f, "bundled"),
            Self::Enterprise => write!(f, "enterprise"),
            Self::OpenSkillsGlobal => write!(f, "openskills-global"),
            Self::Personal => write!(f, "personal"),
            Self::OpenSkillsProject => write!(f, "openskills-project"),
            Self::Project => write!(f, "project"),
        }
    }
}

/// Complete definition of a skill, parsed from a `SKILL.md` file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillInfo {
    /// Unique identifier for the skill (defaults to directory name).
    /// Lowercase, hyphens, max 64 characters.
    pub name: String,
    /// What the skill does; used for auto-invocation matching.
    pub description: Option<String>,
    /// Hint shown during autocomplete (e.g. `"[environment]"`).
    pub argument_hint: Option<String>,
    /// If `true`, only the user can invoke via `/name`; the agent cannot auto-invoke.
    #[serde(default)]
    pub disable_model_invocation: bool,
    /// If `false`, hidden from `/` menu; only the agent can invoke.
    #[serde(default = "default_true")]
    pub user_invocable: bool,
    /// Tools the agent can use without permission when this skill is active.
    #[serde(default)]
    pub allowed_tools: Vec<String>,
    /// Override model when this skill is active (e.g. `"anthropic:claude-sonnet-4-20250514"`).
    pub model: Option<String>,
    /// Set to `Fork` to run in a forked subagent context.
    pub context: Option<SkillContext>,
    /// Subagent type when `context` is `Fork` (e.g. `"explore"`, `"general-purpose"`).
    pub agent: Option<String>,
    /// Hooks scoped to this skill's lifecycle. Stored as raw JSON until the
    /// hooks system (SPEC §3.17) is implemented.
    pub hooks: Option<serde_json::Value>,
    /// License information (Anthropic Agent Skills spec).
    pub license: Option<String>,
    /// Environment compatibility notes (Anthropic Agent Skills spec).
    pub compatibility: Option<String>,
    /// Arbitrary key-value metadata (Anthropic Agent Skills spec).
    #[serde(default)]
    pub metadata: HashMap<String, String>,
    /// Trigger phrase used for skill catalog and auto-invocation matching.
    /// Defaults to the skill name when not specified in frontmatter.
    pub trigger: Option<String>,
    /// Whether this skill allows `!`command`` dynamic context injection.
    /// Defaults to `false` — skills must opt in explicitly via
    /// `allow_dynamic_context: true` in the YAML frontmatter.
    #[serde(default)]
    pub allow_dynamic_context: bool,
    /// Absolute path to the `SKILL.md` file this skill was loaded from.
    #[serde(skip)]
    pub source_path: PathBuf,
    /// Directory containing the `SKILL.md` file (used for `${RAGENT_SKILL_DIR}`).
    #[serde(skip)]
    pub skill_dir: PathBuf,
    /// Where this skill was discovered.
    pub scope: SkillScope,
    /// Markdown body after the YAML frontmatter (the skill instructions).
    ///
    /// For skills discovered from disk this is left empty until the skill is
    /// invoked; use [`SkillInfo::body_or_load`] to read it on demand.
    #[serde(skip)]
    pub body: String,
    /// On-demand body cache. Populated by [`SkillInfo::body_or_load`] so that
    /// repeated invocations avoid re-reading the `SKILL.md` file from disk.
    #[serde(skip, default = "default_body_cache")]
    body_cache: Arc<Mutex<Option<String>>>,
}

fn default_body_cache() -> Arc<Mutex<Option<String>>> {
    Arc::new(Mutex::new(None))
}

const fn default_true() -> bool {
    true
}

impl SkillInfo {
    /// Creates a new skill with the given name and body, using default values.
    ///
    /// # Errors
    ///
    /// This function does not return errors. It always constructs a valid `SkillInfo`
    /// with default values for all optional fields.
    ///
    /// # Examples
    ///
    /// ```
    /// use ragent_agent::skill::SkillInfo;
    ///
    /// let skill = SkillInfo::new("deploy", "Deploy $ARGUMENTS to production");
    /// assert_eq!(skill.name, "deploy");
    /// assert!(skill.user_invocable);
    /// assert!(!skill.disable_model_invocation);
    /// ```
    pub fn new(name: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
            argument_hint: None,
            disable_model_invocation: false,
            user_invocable: true,
            allowed_tools: Vec::new(),
            model: None,
            context: None,
            agent: None,
            hooks: None,
            license: None,
            compatibility: None,
            metadata: HashMap::new(),
            trigger: None,
            allow_dynamic_context: false,
            source_path: PathBuf::new(),
            skill_dir: PathBuf::new(),
            scope: SkillScope::Project,
            body: body.into(),
            body_cache: default_body_cache(),
        }
    }

    /// Returns the skill body, loading it from [`source_path`] on demand if it
    /// has not been loaded yet.
    ///
    /// For skills constructed with an in-memory body (e.g. bundled skills or
    /// [`SkillInfo::new`]), the stored body is returned directly. For skills
    /// loaded from disk with an empty body field, this method reads the
    /// `SKILL.md` file from [`source_path`], caches it, and returns the
    /// content. Subsequent calls return the cached copy without touching disk.
    ///
    /// # Errors
    ///
    /// Returns an error if the body is not present, [`source_path`] is empty,
    /// or the file cannot be read.
    pub async fn body_or_load(&self) -> anyhow::Result<String> {
        if !self.body.is_empty() {
            return Ok(self.body.clone());
        }

        let mut cache = self.body_cache.lock().await;
        if let Some(body) = cache.as_ref() {
            return Ok(body.clone());
        }

        if self.source_path.as_os_str().is_empty() {
            return Err(anyhow::anyhow!(
                "Skill '{}' has no source path and no loaded body",
                self.name
            ));
        }

        let content = tokio::fs::read_to_string(&self.source_path)
            .await
            .map_err(|e| {
                anyhow::anyhow!(
                    "Failed to load skill body for '{}' from {}: {e}",
                    self.name,
                    self.source_path.display()
                )
            })?;

        let body = loader::extract_body(&content)
            .map_err(|e| anyhow::anyhow!("Failed to extract body for '{}': {e}", self.name))?;

        *cache = Some(body.to_string());
        Ok(body.to_string())
    }

    /// Returns `true` if the user can invoke this skill via `/name`.
    ///
    /// # Errors
    ///
    /// This function does not return errors. It returns a boolean based on the
    /// `user_invocable` field value.
    #[must_use]
    pub const fn is_user_invocable(&self) -> bool {
        self.user_invocable
    }

    /// Returns `true` if the agent can auto-invoke this skill.
    ///
    /// # Errors
    ///
    /// This function does not return errors. It returns a boolean based on the
    /// inverse of the `disable_model_invocation` field.
    #[must_use]
    pub const fn is_agent_invocable(&self) -> bool {
        !self.disable_model_invocation
    }

    /// Returns `true` if this skill runs in a forked subagent context.
    ///
    /// # Errors
    ///
    /// This function does not return errors. It returns `true` if the context
    /// is set to `SkillContext::Fork`, `false` otherwise.
    #[must_use]
    pub fn is_forked(&self) -> bool {
        self.context.as_ref() == Some(&SkillContext::Fork)
    }
}

impl Default for SkillInfo {
    fn default() -> Self {
        Self::new("", "")
    }
}

/// A lightweight catalog entry for a skill, suitable for startup-time
/// progressive disclosure. It contains only metadata; the full skill body is
/// not included and is loaded on demand when the skill is invoked.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillCatalogEntry {
    /// Unique identifier for the skill.
    pub name: String,
    /// Human-readable description of what the skill does.
    pub description: String,
    /// Trigger phrase for invoking the skill (e.g. `/deploy`).
    /// Falls back to the skill name when no explicit trigger is configured.
    pub trigger: String,
    /// Resolution scope where the skill was discovered.
    pub scope: SkillScope,
    /// `true` if the user can invoke this skill via `/name`.
    pub user_invocable: bool,
    /// `true` if the agent (model) can auto-invoke this skill.
    pub agent_invocable: bool,
    /// Optional argument hint shown during autocomplete (e.g. `"[environment]"`).
    pub argument_hint: Option<String>,
}

/// Registry of discovered skills, indexed by name.
///
/// Skills are loaded from multiple scopes (bundled, personal, project) and
/// merged so that higher-priority scopes override lower ones.
///
/// # Examples
///
/// ```
/// use ragent_agent::skill::{SkillInfo, SkillRegistry, SkillScope};
///
/// let mut registry = SkillRegistry::new();
///
/// let mut skill = SkillInfo::new("deploy", "Deploy to production");
/// skill.scope = SkillScope::Project;
/// skill.description = Some("Deploy the application".to_string());
/// registry.register(skill);
///
/// assert!(registry.get("deploy").is_some());
/// assert_eq!(registry.list_all().len(), 1);
/// ```
#[derive(Debug, Clone, Default)]
pub struct SkillRegistry {
    skills: HashMap<String, SkillInfo>,
    bundled_count: usize,
    discovered_count: usize,
}

impl SkillRegistry {
    /// Creates an empty skill registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            skills: HashMap::new(),
            bundled_count: 0,
            discovered_count: 0,
        }
    }

    /// Discover and load all skills accessible from `working_dir`.
    ///
    /// Bundled skills are registered first at lowest priority, then discovered
    /// skills are overlaid. When names conflict, higher-priority scopes win.
    /// `extra_dirs` are additional directories to scan (from config `skill_dirs`).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use ragent_agent::skill::SkillRegistry;
    /// use std::path::Path;
    ///
    /// let registry = SkillRegistry::load(Path::new("/my/project"), &[]);
    /// println!("Loaded {} skills", registry.len());
    /// ```
    pub fn load(working_dir: &std::path::Path, extra_dirs: &[String]) -> Self {
        let mut registry = Self::new();

        // 1. Register bundled skills (lowest priority — overridable)
        for skill in bundled::bundled_skills() {
            registry.register(skill);
        }
        registry.bundled_count = registry.len();

        // 2. Overlay discovered skills (personal + extra + project scope)
        let discovered = loader::discover_skills(working_dir, extra_dirs);
        registry.discovered_count = discovered.len();

        for skill in discovered {
            registry.register(skill);
        }

        tracing::info!(
            "Skill registry loaded: {} bundled, {} discovered, {} registered (after dedup)",
            registry.bundled_count,
            registry.discovered_count,
            registry.len()
        );

        registry
    }

    /// Registers a skill. If a skill with the same name already exists, the
    /// new skill replaces it only if its scope is equal or higher priority.
    pub fn register(&mut self, skill: SkillInfo) {
        let dominated = self
            .skills
            .get(&skill.name)
            .is_none_or(|existing| skill.scope >= existing.scope);

        if dominated {
            self.skills.insert(skill.name.clone(), skill);
        }
    }

    /// Looks up a skill by name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&SkillInfo> {
        self.skills.get(name)
    }

    /// Returns all skills that the user can invoke via `/name`.
    #[must_use]
    pub fn list_user_invocable(&self) -> Vec<&SkillInfo> {
        self.skills
            .values()
            .filter(|s| s.is_user_invocable())
            .collect()
    }

    /// Returns all skills that the agent can auto-invoke.
    #[must_use]
    pub fn list_agent_invocable(&self) -> Vec<&SkillInfo> {
        self.skills
            .values()
            .filter(|s| s.is_agent_invocable())
            .collect()
    }

    /// Returns all registered skills, sorted by name for deterministic output.
    #[must_use]
    pub fn list_all(&self) -> Vec<&SkillInfo> {
        let mut skills: Vec<_> = self.skills.values().collect();
        skills.sort_by(|a, b| a.name.cmp(&b.name));
        skills
    }

    /// Returns the number of registered skills.
    #[must_use]
    pub fn len(&self) -> usize {
        self.skills.len()
    }

    /// Returns the number of bundled skills that were registered before
    /// discovered skills were overlaid.
    #[must_use]
    pub fn bundled_count(&self) -> usize {
        self.bundled_count
    }

    /// Returns the number of skills discovered on disk (before deduplication
    /// against bundled entries).
    #[must_use]
    pub fn discovered_count(&self) -> usize {
        self.discovered_count
    }

    /// Returns `true` if no skills are registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.skills.is_empty()
    }

    /// Returns a lightweight catalog of all registered skills.
    ///
    /// The catalog is derived from the already-loaded skill metadata, so it
    /// does not read `SKILL.md` bodies from disk. This makes it cheap to
    /// build at startup and suitable for progressive disclosure (display the
    /// compact list and defer full body loading until invocation).
    #[must_use]
    pub fn catalog(&self) -> Vec<SkillCatalogEntry> {
        let mut entries: Vec<SkillCatalogEntry> = self
            .skills
            .values()
            .map(|skill| SkillCatalogEntry {
                name: skill.name.clone(),
                description: skill.description.clone().unwrap_or_default(),
                trigger: skill.trigger.clone().unwrap_or_else(|| skill.name.clone()),
                scope: skill.scope,
                user_invocable: skill.user_invocable,
                agent_invocable: !skill.disable_model_invocation,
                argument_hint: skill.argument_hint.clone(),
            })
            .collect();
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        entries
    }
}

#[cfg(test)]
#[path = "../../tests/inline/skill_mod.rs"]
mod tests_tests;
