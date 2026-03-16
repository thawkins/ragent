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
//! | Scope      | Path                                   | Applies To                    |
//! |------------|----------------------------------------|-------------------------------|
//! | Enterprise | Managed settings                       | All users in organization     |
//! | Personal   | `~/.ragent/skills/<name>/SKILL.md`     | All projects for this user    |
//! | Project    | `.ragent/skills/<name>/SKILL.md`        | This project only             |
//!
//! Higher-priority scopes override lower ones when names conflict.

pub mod loader;
pub mod args;
pub mod context;
pub mod invoke;
pub mod bundled;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

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
            SkillContext::Fork => write!(f, "fork"),
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
    /// User-level skill from `~/.ragent/skills/`.
    Personal = 2,
    /// Project-level skill from `.ragent/skills/`.
    Project = 3,
}

impl std::fmt::Display for SkillScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SkillScope::Bundled => write!(f, "bundled"),
            SkillScope::Enterprise => write!(f, "enterprise"),
            SkillScope::Personal => write!(f, "personal"),
            SkillScope::Project => write!(f, "project"),
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
    /// Absolute path to the `SKILL.md` file this skill was loaded from.
    #[serde(skip)]
    pub source_path: PathBuf,
    /// Directory containing the `SKILL.md` file (used for `${RAGENT_SKILL_DIR}`).
    #[serde(skip)]
    pub skill_dir: PathBuf,
    /// Where this skill was discovered.
    pub scope: SkillScope,
    /// Markdown body after the YAML frontmatter (the skill instructions).
    #[serde(skip)]
    pub body: String,
}

fn default_true() -> bool {
    true
}

impl SkillInfo {
    /// Creates a new skill with the given name and body, using default values.
    ///
    /// # Examples
    ///
    /// ```
    /// use ragent_core::skill::SkillInfo;
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
            source_path: PathBuf::new(),
            skill_dir: PathBuf::new(),
            scope: SkillScope::Project,
            body: body.into(),
        }
    }

    /// Returns `true` if the user can invoke this skill via `/name`.
    pub fn is_user_invocable(&self) -> bool {
        self.user_invocable
    }

    /// Returns `true` if the agent can auto-invoke this skill.
    pub fn is_agent_invocable(&self) -> bool {
        !self.disable_model_invocation
    }

    /// Returns `true` if this skill runs in a forked subagent context.
    pub fn is_forked(&self) -> bool {
        self.context.as_ref() == Some(&SkillContext::Fork)
    }
}

impl Default for SkillInfo {
    fn default() -> Self {
        Self::new("", "")
    }
}

/// Registry of discovered skills, indexed by name.
///
/// Skills are loaded from multiple scopes (bundled, personal, project) and
/// merged so that higher-priority scopes override lower ones.
///
/// # Examples
///
/// ```
/// use ragent_core::skill::{SkillInfo, SkillRegistry, SkillScope};
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
}

impl SkillRegistry {
    /// Creates an empty skill registry.
    pub fn new() -> Self {
        Self {
            skills: HashMap::new(),
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
    /// use ragent_core::skill::SkillRegistry;
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
        let bundled_count = registry.len();

        // 2. Overlay discovered skills (personal + extra + project scope)
        let discovered = loader::discover_skills(working_dir, extra_dirs);
        let disc_count = discovered.len();

        for skill in discovered {
            registry.register(skill);
        }

        tracing::info!(
            "Skill registry loaded: {} bundled, {} discovered, {} registered (after dedup)",
            bundled_count,
            disc_count,
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
    pub fn get(&self, name: &str) -> Option<&SkillInfo> {
        self.skills.get(name)
    }

    /// Returns all skills that the user can invoke via `/name`.
    pub fn list_user_invocable(&self) -> Vec<&SkillInfo> {
        self.skills
            .values()
            .filter(|s| s.is_user_invocable())
            .collect()
    }

    /// Returns all skills that the agent can auto-invoke.
    pub fn list_agent_invocable(&self) -> Vec<&SkillInfo> {
        self.skills
            .values()
            .filter(|s| s.is_agent_invocable())
            .collect()
    }

    /// Returns all registered skills, sorted by name for deterministic output.
    pub fn list_all(&self) -> Vec<&SkillInfo> {
        let mut skills: Vec<_> = self.skills.values().collect();
        skills.sort_by(|a, b| a.name.cmp(&b.name));
        skills
    }

    /// Returns the number of registered skills.
    pub fn len(&self) -> usize {
        self.skills.len()
    }

    /// Returns `true` if no skills are registered.
    pub fn is_empty(&self) -> bool {
        self.skills.is_empty()
    }
}

#[cfg(test)]
mod tests {
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
        // Only bundled skills when no discovered skills exist
        assert_eq!(registry.len(), 4);
        assert!(registry.get("simplify").is_some());
        assert!(registry.get("batch").is_some());
        assert!(registry.get("debug").is_some());
        assert!(registry.get("loop").is_some());

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
        // 4 bundled + 2 project skills
        assert_eq!(registry.len(), 6);

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
        // Only bundled skills (no discovered skills)
        assert_eq!(registry.len(), 4);

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
        // 4 bundled + 1 good discovered (bad is skipped)
        assert_eq!(registry.len(), 5);
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
        // 4 bundled + 2 discovered (root-skill + frontend-deploy)
        assert_eq!(registry.len(), 6);
        assert!(registry.get("root-skill").is_some());
        assert!(registry.get("frontend-deploy").is_some());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_registry_load_project_overrides_bundled() {
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
        assert!(simplify.body.contains("My custom simplify"));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_registry_load_with_extra_dirs() {
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

        // 4 bundled + 1 extra
        assert_eq!(registry.len(), 5);
        let shared = registry.get("shared-tool").expect("should find shared-tool");
        assert_eq!(shared.description.as_deref(), Some("Shared tool"));

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
