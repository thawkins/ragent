//! `skill_manage` — Runtime management of the skill registry.
//!
//! Implements JCODEPLAN M8 T-071: lets the agent inspect, load, reload, and
//! invoke skills on demand without restarting the session. Works against the
//! canonical [`crate::skill::SkillRegistry`], reusing its scope-precedence
//! rules (bundled → enterprise → OpenSkills → personal → project).
//!
//! # Actions
//!
//! - `list`     — enumerate registered skills (metadata only)
//! - `read`     — return a skill's processed prompt/body (with optional
//!                argument substitution)
//! - `load`     — (re)discover skills and return the target skill's prompt,
//!                the same content the model would receive on a `/skill`
//!                invocation
//! - `reload`   — drop all cached skill bodies and re-discover from disk;
//!                returns a summary of what changed
//!
//! Skill bodies are loaded lazily per SPEC §3.19 / §21.1 — this tool forces
//! the lazy load and returns the fully processed prompt (arguments
//! substituted, dynamic context injected when allowed).

use anyhow::{Context, Result};
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};

/// Tool for inspecting and hot-loading skills at runtime.
pub struct SkillManageTool;

#[async_trait::async_trait]
impl Tool for SkillManageTool {
    fn name(&self) -> &'static str {
        "skill_manage"
    }

    fn description(&self) -> &'static str {
        "Manage the skill registry at runtime: list available skills, read a \
         skill's prompt, load (discover + invoke) a skill by name, or reload \
         all skills from disk. Actions: list, read, load, reload."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["list", "read", "load", "reload"],
                    "description": "Operation to perform"
                },
                "name": {
                    "type": "string",
                    "description": "Skill name (required for read/load)"
                },
                "arguments": {
                    "type": "string",
                    "description": "Arguments substituted into the skill body ($ARGUMENTS and friends) for read/load"
                },
                "scope": {
                    "type": "string",
                    "enum": [
                        "bundled", "enterprise", "openskills-global", "personal",
                        "openskills-project", "project"
                    ],
                    "description": "Optional scope filter for list"
                },
                "include_bodies": {
                    "type": "boolean",
                    "description": "When true, list includes each skill's full prompt body (default: false — metadata only)"
                }
            },
            "required": ["action"]
        })
    }

    fn permission_category(&self) -> &'static str {
        "skill:manage"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let action = input["action"]
            .as_str()
            .context("Missing required 'action' parameter")?;

        let skill_dirs = ctx
            .config
            .as_ref()
            .map(|c| c.skill_dirs.clone())
            .unwrap_or_default();

        match action {
            "list" => action_list(&input, ctx, &skill_dirs).await,
            "read" => action_read(&input, ctx, &skill_dirs).await,
            "load" => action_load(&input, ctx, &skill_dirs).await,
            "reload" => action_reload(&input, ctx, &skill_dirs).await,
            other => Err(anyhow::anyhow!(
                "Unknown skill_manage action '{other}'. Valid actions: list, read, load, reload"
            )),
        }
    }
}

/// Build a fresh registry for the given working directory.
fn build_registry(ctx: &ToolContext, extra_dirs: &[String]) -> crate::skill::SkillRegistry {
    crate::skill::SkillRegistry::load(&ctx.working_dir, extra_dirs)
}

/// `list` — enumerate registered skills.
async fn action_list(
    input: &Value,
    ctx: &ToolContext,
    skill_dirs: &[String],
) -> Result<ToolOutput> {
    let registry = build_registry(ctx, skill_dirs);
    let scope_filter = input["scope"].as_str();
    let include_bodies = input["include_bodies"].as_bool().unwrap_or(false);

    let catalog = registry.catalog();
    let mut entries: Vec<&crate::skill::SkillCatalogEntry> = catalog.iter().collect();
    if let Some(scope) = scope_filter {
        entries.retain(|e| e.scope.to_string() == scope);
    }
    entries.sort_by(|a, b| a.name.cmp(&b.name));

    if entries.is_empty() {
        return Ok(ToolOutput {
            content: format!(
                "No skills found{}.",
                scope_filter
                    .map(|s| format!(" in scope '{s}'"))
                    .unwrap_or_default()
            ),
            metadata: Some(json!({
                "action": "list",
                "count": 0,
                "scope_filter": scope_filter,
            })),
        });
    }

    let mut lines = vec![format!("Found {} skill(s):\n", entries.len())];
    lines.push(format!(
        "| {:<24} | {:<17} | {:<5} | {:<5} | Description |",
        "Name", "Scope", "User", "Agent"
    ));
    lines.push(format!(
        "|{:-<26}|{:-<19}|{:-<7}|{:-<7}|{:-<60}",
        "", "", "", "", ""
    ));

    let mut bodies_section = String::new();
    for entry in &entries {
        let desc = ragent_types::truncate_chars(&entry.description, 60);
        lines.push(format!(
            "| {:<24} | {:<17} | {:<5} | {:<5} | {} |",
            ragent_types::truncate_chars(&entry.name, 22),
            entry.scope.to_string(),
            if entry.user_invocable { "yes" } else { "no" },
            if entry.agent_invocable { "yes" } else { "no" },
            desc
        ));
        if include_bodies
            && let Some(skill) = registry.get(&entry.name)
            && let Ok(body) = skill.body_or_load().await
        {
            bodies_section.push_str(&format!(
                "\n### `{}` ({})\n\n```\n{}\n```\n",
                entry.name, entry.scope, body
            ));
        }
    }

    let mut content = lines.join("\n");
    if !bodies_section.is_empty() {
        content.push_str(&bodies_section);
    }

    Ok(ToolOutput {
        content,
        metadata: Some(json!({
            "action": "list",
            "count": entries.len(),
            "scope_filter": scope_filter,
            "include_bodies": include_bodies,
        })),
    })
}

/// Shared read/load implementation: discover, look up, invoke, return prompt.
///
/// The only difference between the two is presentation (`read` is a plain
/// fetch; `load` emphasises that the prompt is now injected for the model).
async fn read_or_load(
    action: &str,
    input: &Value,
    ctx: &ToolContext,
    skill_dirs: &[String],
) -> Result<ToolOutput> {
    let name = input["name"]
        .as_str()
        .filter(|s| !s.is_empty())
        .with_context(|| format!("{action} requires the 'name' parameter"))?;
    let args = input["arguments"].as_str().unwrap_or("");

    // Always re-discover so `load` picks up skills added after session start.
    let registry = build_registry(ctx, skill_dirs);
    let skill = registry.get(name).ok_or_else(|| {
        let mut available: Vec<&str> = registry
            .list_all()
            .iter()
            .map(|s| s.name.as_str())
            .collect();
        available.sort_unstable();
        anyhow::anyhow!(
            "Skill '{name}' not found. Available skills: {}{}",
            if available.is_empty() {
                "(none)".to_string()
            } else {
                available.join(", ")
            },
            "\nHint: add a directory under .ragent/skills/<name>/SKILL.md (project) or \
             ~/.ragent/skills/<name>/SKILL.md (personal) and run skill_manage action=reload.",
        )
    })?;

    let invocation =
        crate::skill::invoke::invoke_skill(skill, args, &ctx.session_id, &ctx.working_dir)
            .await
            .with_context(|| format!("Failed to invoke skill '{name}'"))?;

    let header = if action == "load" {
        format!(
            "Skill `{name}` loaded ({} scope). The following prompt is now active for this turn:\n",
            skill.scope
        )
    } else {
        format!("Skill `{name}` ({} scope) prompt:\n", skill.scope)
    };

    Ok(ToolOutput {
        content: format!(
            "{}\n{}\n---\n\n{}",
            header,
            if invocation.forked {
                "*context: fork (runs in a subagent when invoked via /skill)*\n---\n\n"
            } else {
                ""
            },
            invocation.content
        ),
        metadata: Some(json!({
            "action": action,
            "name": name,
            "scope": skill.scope.to_string(),
            "description": skill.description,
            "agent_invocable": !skill.disable_model_invocation,
            "user_invocable": skill.user_invocable,
            "allowed_tools": skill.allowed_tools,
            "forked": invocation.forked,
        })),
    })
}

/// `read` — fetch a skill's processed prompt.
async fn action_read(
    input: &Value,
    ctx: &ToolContext,
    skill_dirs: &[String],
) -> Result<ToolOutput> {
    read_or_load("read", input, ctx, skill_dirs).await
}

/// `load` — discover + return a skill's prompt.
async fn action_load(
    input: &Value,
    ctx: &ToolContext,
    skill_dirs: &[String],
) -> Result<ToolOutput> {
    read_or_load("load", input, ctx, skill_dirs).await
}

/// `reload` — re-discover all skills and report what changed.
async fn action_reload(
    input: &Value,
    ctx: &ToolContext,
    skill_dirs: &[String],
) -> Result<ToolOutput> {
    // Build a "before" snapshot to compare against. Because the registry has
    // no shared mutable state (each `SkillRegistry::load` produces a fresh
    // copy), the pre-reload snapshot is representative of what a `list` call
    // would have shown a moment ago.
    let before = build_registry(ctx, skill_dirs);

    // Clear on-disk body caches so files edited since first load are re-read
    // on next access. Caches live on each SkillInfo, so building a fresh
    // registry below already yields uncached SkillInfo values; the explicit
    // clear covers any long-lived clones (e.g. bundled skills shared via Arc).
    for skill in before.list_all() {
        skill.clear_body_cache().await;
    }

    // Re-discover from disk (fresh registry = reload).
    let after = build_registry(ctx, skill_dirs);

    let before_names: std::collections::BTreeSet<&str> =
        before.list_all().iter().map(|s| s.name.as_str()).collect();
    let after_names: std::collections::BTreeSet<&str> =
        after.list_all().iter().map(|s| s.name.as_str()).collect();

    let added: Vec<&str> = after_names.difference(&before_names).copied().collect();
    let removed: Vec<&str> = before_names.difference(&after_names).copied().collect();

    let scope_filter = input["scope"].as_str();
    let mut catalog = after.catalog();
    if let Some(scope) = scope_filter {
        catalog.retain(|e| e.scope.to_string() == scope);
    }
    catalog.sort_by(|a, b| a.name.cmp(&b.name));

    let mut content = format!(
        "Reloaded skill registry: {} skill(s) ({} bundled baseline, {} discovered this scan).\n",
        catalog.len(),
        after.bundled_count(),
        after.discovered_count()
    );
    if !added.is_empty() {
        content.push_str(&format!(
            "\n**Added ({}):** {}",
            added.len(),
            added.join(", ")
        ));
    }
    if !removed.is_empty() {
        content.push_str(&format!(
            "\n**Removed ({}):** {}",
            removed.len(),
            removed.join(", ")
        ));
    }
    if added.is_empty() && removed.is_empty() {
        content.push_str("\n*No new or removed skills since the previous scan.*");
    }
    content.push_str("\n\n| Name                 | Scope             | Description |\n");
    content.push_str("|----------------------|-------------------|-------------|\n");
    for entry in &catalog {
        content.push_str(&format!(
            "| {:<20} | {:<17} | {} |\n",
            ragent_types::truncate_chars(&entry.name, 20),
            entry.scope,
            ragent_types::truncate_chars(&entry.description, 50)
        ));
    }

    Ok(ToolOutput {
        content,
        metadata: Some(json!({
            "action": "reload",
            "total": catalog.len(),
            "bundled": after.bundled_count(),
            "discovered": after.discovered_count(),
            "added": added,
            "removed": removed,
        })),
    })
}
