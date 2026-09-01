//! Shared helpers for listing installed team blueprints.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use ragent_types::strutil::truncate_bytes;

/// Information about an installed team blueprint.
#[derive(Debug, Clone)]
pub struct BlueprintInfo {
    /// Directory name of the blueprint.
    pub name: String,
    /// Absolute path to the blueprint directory.
    pub path: PathBuf,
    /// Where the blueprint was discovered: `"project"` or `"global"`.
    pub scope: String,
}

/// Discover all installed team blueprints.
///
/// Searches project-local `.ragent/blueprints/teams/` first (walking up from
/// `working_dir`), then falls back to `~/.ragent/blueprints/teams/`. Project-local
/// blueprints take precedence over global blueprints with the same name.
pub fn list_installed_blueprints(working_dir: &Path) -> Vec<BlueprintInfo> {
    let mut out: Vec<BlueprintInfo> = Vec::new();
    let mut seen_names: HashSet<String> = HashSet::new();

    // Walk up to find project .ragent/blueprints/teams/
    let mut cur_opt: Option<&Path> = Some(working_dir);
    while let Some(cur) = cur_opt {
        let bp_root = cur.join(".ragent").join("blueprints").join("teams");
        if bp_root.is_dir() {
            collect_blueprints(&bp_root, "project", &mut seen_names, &mut out);
            break;
        }
        cur_opt = cur.parent();
    }

    // Global fallback
    if let Some(home) = dirs::home_dir() {
        let bp_root = home.join(".ragent").join("blueprints").join("teams");
        if bp_root.is_dir() {
            collect_blueprints(&bp_root, "global", &mut seen_names, &mut out);
        }
    }

    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn collect_blueprints(
    root: &Path,
    scope: &str,
    seen_names: &mut HashSet<String>,
    out: &mut Vec<BlueprintInfo>,
) {
    let entries = match std::fs::read_dir(root) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if !seen_names.insert(name.clone()) {
            continue;
        }
        out.push(BlueprintInfo {
            name,
            path,
            scope: scope.to_string(),
        });
    }
}

/// Count entries in a JSON array file, returning 0 if the file is missing or invalid.
fn count_json_array_entries(path: &Path) -> usize {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
        .and_then(|v| v.as_array().map(|a| a.len()))
        .unwrap_or(0)
}

/// Extract a short description from a blueprint README.md.
///
/// Returns the first non-empty line that is not a markdown heading, or `"-"`
/// when no description can be read.
fn blueprint_description(path: &Path) -> String {
    std::fs::read_to_string(path.join("README.md"))
        .ok()
        .and_then(|raw| {
            raw.lines()
                .find(|l| !l.trim().is_empty() && !l.starts_with('#'))
                .map(|l| l.trim().to_string())
        })
        .unwrap_or_else(|| "-".to_string())
}

/// Render a detailed summary of a single blueprint, or `None` if not found.
pub fn render_blueprint_detail(
    blueprints: &[BlueprintInfo],
    name: &str,
    command_label: &str,
) -> Option<String> {
    let bp = blueprints.iter().find(|bp| bp.name == name)?;
    let mut output = format!(
        "From: {command_label} {name}\n\n## Blueprint: `{name}`\n\n**Scope:** {}  \n**Path:** `{}`\n\n",
        bp.scope,
        bp.path.display()
    );

    // README.md
    if let Ok(readme) = std::fs::read_to_string(bp.path.join("README.md")) {
        output.push_str("### Description\n\n");
        output.push_str(&readme);
        output.push_str("\n\n");
    }

    // Teammates from spawn-prompts.json
    if let Ok(raw) = std::fs::read_to_string(bp.path.join("spawn-prompts.json")) {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&raw) {
            if let Some(items) = val.as_array() {
                output.push_str("### Teammates\n\n");
                output.push_str("| Name | Type | Prompt |\n");
                output.push_str("|------|------|--------|\n");
                for item in items {
                    let tname = item
                        .get("teammate_name")
                        .or_else(|| item.get("args").and_then(|a| a.get("teammate_name")))
                        .and_then(|v| v.as_str())
                        .unwrap_or("auto");
                    let atype = item
                        .get("agent_type")
                        .or_else(|| item.get("args").and_then(|a| a.get("agent_type")))
                        .and_then(|v| v.as_str())
                        .unwrap_or("general");
                    let prompt = item
                        .get("prompt")
                        .or_else(|| item.get("args").and_then(|a| a.get("prompt")))
                        .and_then(|v| v.as_str())
                        .unwrap_or("-");
                    let prompt_short = truncate_bytes(prompt, 77);
                    output.push_str(&format!("| `{}` | {} | {} |\n", tname, atype, prompt_short));
                }
                output.push('\n');
            }
        }
    }

    // Seed tasks from task-seed.json
    if let Ok(raw) = std::fs::read_to_string(bp.path.join("task-seed.json")) {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&raw) {
            if let Some(items) = val.as_array() {
                output.push_str("### Seed Tasks\n\n");
                output.push_str("| Title | Description |\n");
                output.push_str("|-------|-------------|\n");
                for item in items {
                    let title = item
                        .get("title")
                        .or_else(|| item.get("input").and_then(|a| a.get("title")))
                        .or_else(|| item.get("args").and_then(|a| a.get("title")))
                        .and_then(|v| v.as_str())
                        .unwrap_or("-");
                    let desc = item
                        .get("description")
                        .or_else(|| item.get("input").and_then(|a| a.get("description")))
                        .or_else(|| item.get("args").and_then(|a| a.get("description")))
                        .and_then(|v| v.as_str())
                        .unwrap_or("-");
                    output.push_str(&format!("| {} | {} |\n", title, desc));
                }
                output.push('\n');
            }
        }
    }

    output.push_str(&format!("**Usage:** `/team create {name}`\n"));
    Some(output)
}

/// Render a markdown list/table of all installed blueprints.
pub fn render_blueprint_list(blueprints: &[BlueprintInfo], command_label: &str) -> String {
    let mut output = format!("From: {command_label}\n\n## Installed Team Blueprints\n\n");
    if blueprints.is_empty() {
        output.push_str(
            "No blueprints found.\n\nInstall blueprints to:\n\
             - `[project]/.ragent/blueprints/teams/<name>/`\n\
             - `~/.ragent/blueprints/teams/<name>/`\n",
        );
        return output;
    }

    output.push_str("| Blueprint | Scope | Teammates | Tasks | Description |\n");
    output.push_str("|-----------|-------|-----------|-------|-------------|\n");

    for bp in blueprints {
        let teammate_count = count_json_array_entries(&bp.path.join("spawn-prompts.json"));
        let task_count = count_json_array_entries(&bp.path.join("task-seed.json"));
        let desc = blueprint_description(&bp.path);
        output.push_str(&format!(
            "| `{}` | {} | {} | {} | {} |\n",
            bp.name, bp.scope, teammate_count, task_count, desc
        ));
    }
    output
}
