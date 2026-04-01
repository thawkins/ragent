//! Discovery and loading of custom OASF agent definitions.
//!
//! Custom agents are stored as `.json` (OASF) or `.md` (profile) files in two
//! standard directories, searched in priority order (project-local wins over
//! user-global):
//!
//! | Priority | Directory |
//! |----------|-----------|
//! | 1 (lower) | `~/.ragent/agents/` |
//! | 2 (higher) | `[PROJECT]/.ragent/agents/` |
//!
//! The project directory is discovered by walking up from `working_dir` until a
//! `.ragent/` directory is found or the filesystem root is reached.
//!
//! Each `.json` file must contain a valid OASF agent record with at least one
//! `ragent/agent/v1` module. `.md` files use JSON frontmatter (between `---`
//! delimiters) for metadata; the markdown body becomes the system prompt.
//!
//! Malformed files produce a non-fatal diagnostic string; the rest of the
//! custom agents continue loading normally.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::agent::oasf::{OasfAgentRecord, RAGENT_MODULE_TYPE, RagentAgentPayload};
use crate::agent::{AgentInfo, AgentMode, ModelRef};
use crate::permission::{Permission, PermissionAction, PermissionRule};

/// A successfully loaded and validated custom agent definition.
#[derive(Debug, Clone)]
pub struct CustomAgentDef {
    /// The raw OASF record as parsed from disk.
    pub record: OasfAgentRecord,
    /// Absolute path of the file this record was loaded from.
    pub source_path: PathBuf,
    /// The resolved [`AgentInfo`] ready for use by the session processor.
    pub agent_info: AgentInfo,
    /// Scope: `true` = project-local, `false` = user-global.
    pub is_project_local: bool,
}

/// Load all custom agents from the standard discovery directories.
///
/// Scans `~/.ragent/agents/` first (lower priority) then
/// `[PROJECT]/.ragent/agents/` (higher priority). When the same agent `name`
/// appears in both directories the project-local definition replaces the
/// global one.
///
/// Returns `(agents, diagnostics)`.  Diagnostics are non-fatal human-readable
/// strings describing why individual files were skipped or renamed.
pub fn load_custom_agents(working_dir: &Path) -> (Vec<CustomAgentDef>, Vec<String>) {
    let mut agents: HashMap<String, CustomAgentDef> = HashMap::new();
    let mut diagnostics: Vec<String> = Vec::new();

    // Load user-global agents first (lowest priority).
    if let Some(global_dir) = global_agents_dir() {
        scan_dir(&global_dir, false, &mut agents, &mut diagnostics);
    }

    // Load project-local agents (highest priority — overrides global).
    if let Some(project_dir) = find_project_agents_dir(working_dir) {
        scan_dir(&project_dir, true, &mut agents, &mut diagnostics);
    }

    // Return in a stable order (alphabetical by name).
    let mut result: Vec<CustomAgentDef> = agents.into_values().collect();
    result.sort_by(|a, b| a.agent_info.name.cmp(&b.agent_info.name));
    (result, diagnostics)
}

/// Return the user-global agents directory: `~/.ragent/agents/`.
///
/// Returns `None` if the home directory cannot be determined.
pub fn global_agents_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".ragent").join("agents"))
}

/// Walk up from `working_dir` to find the nearest `.ragent/agents/` directory.
///
/// Returns `None` if no `.ragent/` directory is found before the filesystem root.
pub fn find_project_agents_dir(working_dir: &Path) -> Option<PathBuf> {
    let mut current = working_dir;
    loop {
        let candidate = current.join(".ragent").join("agents");
        if candidate.is_dir() {
            return Some(candidate);
        }
        // Try just .ragent existing (agents subdir may not exist yet — skip upward)
        match current.parent() {
            Some(parent) if parent != current => current = parent,
            _ => return None,
        }
    }
}

/// Recursively scan `dir` for `.json` and `.md` agent files and insert
/// validated agents into `agents`. Existing entries are replaced when
/// `is_project_local` is `true` (project-local wins).
fn scan_dir(
    dir: &Path,
    is_project_local: bool,
    agents: &mut HashMap<String, CustomAgentDef>,
    diagnostics: &mut Vec<String>,
) {
    if !dir.is_dir() {
        return;
    }

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(err) => {
            diagnostics.push(format!(
                "could not read agents directory {}: {}",
                dir.display(),
                err
            ));
            return;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            // Recurse into subdirectories
            scan_dir(&path, is_project_local, agents, diagnostics);
            continue;
        }

        let ext = path.extension().and_then(|e| e.to_str());
        let loader: fn(&Path, bool) -> Result<CustomAgentDef, String> = match ext {
            Some("json") => load_agent_file,
            Some("md") => load_agent_profile,
            _ => continue,
        };

        match loader(&path, is_project_local) {
            Ok(def) => {
                let key = def.agent_info.name.clone();
                // Project-local always wins; global only inserts if not already present.
                if is_project_local || !agents.contains_key(&key) {
                    agents.insert(key, def);
                }
            }
            Err(err) => {
                diagnostics.push(format!("{}: {}", path.display(), err));
            }
        }
    }
}

/// Parse and validate a single agent JSON file, returning a [`CustomAgentDef`].
///
/// # Errors
///
/// Returns a human-readable error string when the file cannot be read, parsed,
/// or fails validation.
fn load_agent_file(path: &Path, is_project_local: bool) -> Result<CustomAgentDef, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("could not read file: {}", e))?;

    let record: OasfAgentRecord = serde_json::from_str(&content)
        .map_err(|e| format!("JSON parse error: {}", e))?;

    record_to_agent_info(&record, path).map(|agent_info| CustomAgentDef {
        record: record.clone(),
        source_path: path.to_path_buf(),
        agent_info,
        is_project_local,
    })
}

// ── Markdown agent profiles (.md) ────────────────────────────────────────────

/// JSON frontmatter fields for a `.md` agent profile.
///
/// The markdown body (everything after the closing `---`) becomes the
/// system prompt.  All fields except `name` and `description` are
/// optional and fall back to sensible defaults.
#[derive(Debug, Clone, serde::Deserialize)]
struct ProfileFrontmatter {
    /// Agent name (kebab-case, no spaces).
    name: String,
    /// One-line description.
    description: String,
    /// `"primary"`, `"subagent"`, or `"all"` (default `"all"`).
    mode: Option<String>,
    /// Model in `"provider:model"` format.
    model: Option<String>,
    /// Max agentic-loop iterations.
    max_steps: Option<u32>,
    /// Sampling temperature (0.0–2.0).
    temperature: Option<f32>,
    /// Nucleus sampling (0.0–1.0).
    top_p: Option<f32>,
    /// Hide from user-visible agent picker.
    hidden: Option<bool>,
    /// Persistent memory scope: `"user"`, `"project"`, or omit for none.
    memory: Option<String>,
    /// Permission rules — same schema as OASF `ragent/agent/v1`.
    permissions: Option<Vec<crate::agent::oasf::RagentPermissionRule>>,
    /// Skill names to preload.
    #[serde(default)]
    skills: Vec<String>,
    /// Arbitrary provider options.
    options: Option<serde_json::Value>,
}

/// Parse a `.md` agent profile with JSON frontmatter and return a
/// [`CustomAgentDef`].
///
/// Format:
/// ```markdown
/// ---
/// {
///   "name": "my-agent",
///   "description": "Does useful things"
/// }
/// ---
///
/// You are a helpful agent that ...
/// ```
///
/// # Errors
///
/// Returns a human-readable string when parsing or validation fails.
fn load_agent_profile(path: &Path, is_project_local: bool) -> Result<CustomAgentDef, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("could not read file: {}", e))?;

    let (frontmatter, body) = parse_json_frontmatter(&content)
        .ok_or_else(|| "missing JSON frontmatter (expected --- delimiters)".to_string())?;

    let fm: ProfileFrontmatter = serde_json::from_str(frontmatter)
        .map_err(|e| format!("frontmatter JSON parse error: {}", e))?;

    let system_prompt = body.trim().to_string();
    if system_prompt.is_empty() {
        return Err("markdown body (system_prompt) must not be empty".to_string());
    }

    // Build a RagentAgentPayload and reuse the existing validation path.
    let payload = crate::agent::oasf::RagentAgentPayload {
        system_prompt,
        mode: fm.mode,
        max_steps: fm.max_steps,
        temperature: fm.temperature,
        top_p: fm.top_p,
        model: fm.model,
        skills: fm.skills,
        permissions: fm.permissions,
        hidden: fm.hidden,
        memory: fm.memory,
        options: fm.options,
    };

    // Synthesise a minimal OASF record so the rest of the pipeline is happy.
    let record = OasfAgentRecord {
        name: fm.name.clone(),
        description: fm.description.clone(),
        version: "1.0.0".to_string(),
        schema_version: "0.7.0".to_string(),
        authors: Vec::new(),
        created_at: None,
        skills: Vec::new(),
        domains: Vec::new(),
        locators: Vec::new(),
        modules: vec![crate::agent::oasf::OasfModule {
            module_type: crate::agent::oasf::RAGENT_MODULE_TYPE.to_string(),
            payload: serde_json::to_value(&payload).unwrap_or_default(),
        }],
    };

    record_to_agent_info(&record, path).map(|agent_info| CustomAgentDef {
        record,
        source_path: path.to_path_buf(),
        agent_info,
        is_project_local,
    })
}

/// Extract JSON frontmatter delimited by `---` lines from markdown text.
///
/// Returns `(frontmatter_json, body)` or `None` if delimiters are missing.
fn parse_json_frontmatter(text: &str) -> Option<(&str, &str)> {
    let trimmed = text.trim_start();
    if !trimmed.starts_with("---") {
        return None;
    }
    // Skip past the opening "---" line.
    let after_open = trimmed.strip_prefix("---")?.trim_start_matches(|c: char| c == '-');
    let after_open = after_open.strip_prefix('\n').or_else(|| after_open.strip_prefix("\r\n"))?;
    // Find the closing "---".
    let close_pos = after_open.find("\n---")?;
    let frontmatter = after_open[..close_pos].trim();
    let rest_start = close_pos + 4; // skip "\n---"
    // Skip any trailing dashes and newline on the closing delimiter.
    let rest = after_open[rest_start..].trim_start_matches('-');
    let body = rest.strip_prefix('\n').or_else(|| rest.strip_prefix("\r\n")).unwrap_or(rest);
    Some((frontmatter, body))
}

/// Validate an [`OasfAgentRecord`] and convert it to an [`AgentInfo`].
///
/// # Errors
///
/// Returns a human-readable error string describing which validation rule failed.
pub fn record_to_agent_info(
    record: &OasfAgentRecord,
    source_path: &Path,
) -> Result<AgentInfo, String> {
    // ── Validate core fields ───────────────────────────────────────────────
    if record.name.is_empty() || record.name.contains(' ') {
        return Err(
            "agent name must be non-empty and contain no spaces".to_string(),
        );
    }

    if record.description.trim().is_empty() {
        return Err("description must not be empty".to_string());
    }

    // ── Extract ragent/agent/v1 module ─────────────────────────────────────
    let ragent_module = record
        .modules
        .iter()
        .find(|m| m.module_type == RAGENT_MODULE_TYPE)
        .ok_or_else(|| {
            format!("missing required module type '{}'", RAGENT_MODULE_TYPE)
        })?;

    let payload: RagentAgentPayload =
        serde_json::from_value(ragent_module.payload.clone()).map_err(|e| {
            format!("invalid '{}' payload: {}", RAGENT_MODULE_TYPE, e)
        })?;

    // ── Validate payload fields ────────────────────────────────────────────
    if payload.system_prompt.trim().is_empty() {
        return Err("system_prompt must not be empty".to_string());
    }

    if payload.system_prompt.len() > 32_768 {
        return Err(format!(
            "system_prompt too long ({} chars; max 32768)",
            payload.system_prompt.len()
        ));
    }

    let mode = match payload.mode.as_deref().unwrap_or("all") {
        "primary" => AgentMode::Primary,
        "subagent" => AgentMode::Subagent,
        "all" => AgentMode::All,
        other => {
            return Err(format!(
                "unknown mode '{}'; expected primary, subagent, or all",
                other
            ))
        }
    };

    if let Some(temp) = payload.temperature {
        if !(0.0..=2.0).contains(&temp) {
            return Err(format!(
                "temperature {} out of range [0.0, 2.0]",
                temp
            ));
        }
    }

    if let Some(top_p) = payload.top_p {
        if !(0.0..=1.0).contains(&top_p) {
            return Err(format!(
                "top_p {} out of range [0.0, 1.0]",
                top_p
            ));
        }
    }

    let model = if let Some(ref model_str) = payload.model {
        match model_str.split_once(':') {
            Some((provider, model_id)) if !provider.is_empty() && !model_id.is_empty() => {
                Some(ModelRef {
                    provider_id: provider.to_string(),
                    model_id: model_id.to_string(),
                })
            }
            _ => {
                return Err(format!(
                    "model '{}' must be in 'provider:model' format",
                    model_str
                ))
            }
        }
    } else {
        None
    };

    if let Some(steps) = payload.max_steps {
        if steps == 0 {
            return Err("max_steps must be greater than 0".to_string());
        }
    }

    // ── Parse permissions ──────────────────────────────────────────────────
    let permission = if let Some(ref rules) = payload.permissions {
        rules
            .iter()
            .map(|r| {
                let action = match r.action.as_str() {
                    "allow" => PermissionAction::Allow,
                    "deny" => PermissionAction::Deny,
                    "ask" => PermissionAction::Ask,
                    other => {
                        return Err(format!(
                            "unknown action '{}'; expected allow, deny, or ask",
                            other
                        ))
                    }
                };
                Ok(PermissionRule {
                    permission: Permission::from(r.permission.as_str()),
                    pattern: r.pattern.clone(),
                    action,
                })
            })
            .collect::<Result<Vec<_>, String>>()?
    } else {
        crate::agent::default_permissions()
    };

    // ── Parse provider options ─────────────────────────────────────────────
    let options: HashMap<String, serde_json::Value> = payload
        .options
        .as_ref()
        .and_then(|v| v.as_object())
        .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        .unwrap_or_default();

    // ── Parse memory scope ──────────────────────────────────────────────────
    let memory = match payload.memory.as_deref() {
        Some("user") => crate::team::config::MemoryScope::User,
        Some("project") => crate::team::config::MemoryScope::Project,
        Some("none") | None => crate::team::config::MemoryScope::None,
        Some(other) => {
            return Err(format!(
                "unknown memory scope '{}'; expected user, project, or none",
                other
            ));
        }
    };

    // ── Build AgentInfo ────────────────────────────────────────────────────
    // Store the raw system_prompt with template variables intact; substitution
    // happens at invocation time in build_system_prompt().
    let _ = source_path; // used only for error context by caller
    let model_pinned = model.is_some();
    let agent_info = AgentInfo {
        name: record.name.clone(),
        description: record.description.clone(),
        mode,
        hidden: payload.hidden.unwrap_or(false),
        temperature: payload.temperature,
        top_p: payload.top_p,
        model,
        prompt: Some(payload.system_prompt.clone()),
        permission,
        max_steps: Some(payload.max_steps.unwrap_or(100)),
        skills: payload.skills.clone(),
        memory,
        options,
        model_pinned,
    };

    Ok(agent_info)
}
