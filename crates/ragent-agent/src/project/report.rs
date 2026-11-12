//! Plain-text and JSON reports of projects and their sessions, for the
//! `ragent project` and `ragent session list` commands (spec `projects`,
//! T-011).
//!
//! The functions return the report; printing is left to the CLI.

use anyhow::Result;
use ragent_storage::{ProjectSessions, SessionCheckout, Storage};
use serde_json::{Value, json};

use super::Project;

/// The session groups to report: every project, or only the one `filter`
/// names (by id, name or unique id prefix).
fn groups(storage: &Storage, filter: Option<&str>) -> Result<Vec<ProjectSessions>> {
    let groups = storage.list_sessions_grouped()?;
    let Some(key) = filter else {
        return Ok(groups);
    };
    let project = storage
        .find_project(key)?
        .ok_or_else(|| anyhow::anyhow!("no project named '{key}'"))?;
    Ok(groups
        .into_iter()
        .filter(|g| g.project_id == project.id)
        .collect())
}

/// First eight characters of an id.
fn short(id: &str) -> &str {
    id.get(..8).unwrap_or(id)
}

/// One line describing a project's remote, or an empty string.
fn remote_text(project: &Project) -> String {
    project
        .remote
        .as_ref()
        .map_or_else(String::new, |r| format!("{} {}", r.kind, r.path))
}

/// Lists live top-level sessions under a heading per project.
///
/// Each session line holds the short id, title, branch (when the session
/// has one, with its pull/merge request count when known) and last update.
///
/// # Arguments
///
/// * `storage` - the session store.
/// * `filter` - limit the report to this project (id, name or id prefix).
///
/// # Errors
///
/// Returns an error if storage fails or `filter` names no project.
pub fn grouped_sessions_text(storage: &Storage, filter: Option<&str>) -> Result<String> {
    let groups = groups(storage, filter)?;
    let checkouts = storage.session_checkouts()?;
    if groups.iter().all(|g| g.sessions.is_empty()) {
        return Ok("No sessions found\n".to_string());
    }
    let mut text = String::new();
    for group in groups.iter().filter(|g| !g.sessions.is_empty()) {
        let project = Project::from(group.project.clone());
        let remote = remote_text(&project);
        text.push_str(&format!(
            "{} ({}){}\n",
            project.name,
            project.worktree.display(),
            if remote.is_empty() {
                String::new()
            } else {
                format!(" [{remote}]")
            }
        ));
        for session in &group.sessions {
            let branch = checkouts
                .get(&session.id)
                .map(branch_text)
                .unwrap_or_default();
            text.push_str(&format!(
                "  {} | {} | {}{}\n",
                short(&session.id),
                session.title,
                if branch.is_empty() {
                    String::new()
                } else {
                    format!("{branch} | ")
                },
                session.updated_at
            ));
        }
    }
    Ok(text)
}

/// `branch` or `branch (N requests)`.
fn branch_text(checkout: &SessionCheckout) -> String {
    match (&checkout.branch, checkout.pr_count) {
        (Some(branch), Some(n)) if n > 0 => format!("{branch} ({n} requests)"),
        (Some(branch), _) => branch.clone(),
        (None, _) => String::new(),
    }
}

/// The grouped session listing as JSON: an array of
/// `{ project, sessions: [...] }`.
///
/// # Errors
///
/// Returns an error if storage fails or `filter` names no project.
pub fn grouped_sessions_json(storage: &Storage, filter: Option<&str>) -> Result<Value> {
    let groups = groups(storage, filter)?;
    let checkouts = storage.session_checkouts()?;
    let groups: Vec<Value> = groups
        .into_iter()
        .map(|group| {
            let sessions: Vec<Value> = group
                .sessions
                .iter()
                .map(|s| {
                    let checkout = checkouts.get(&s.id);
                    json!({
                        "id": s.id,
                        "title": s.title,
                        "directory": s.directory,
                        "branch": checkout.and_then(|c| c.branch.clone()),
                        "worktree": checkout.and_then(|c| c.worktree.clone()),
                        "requests": checkout.and_then(|c| c.pr_count),
                        "created_at": s.created_at,
                        "updated_at": s.updated_at,
                    })
                })
                .collect();
            json!({ "project": Project::from(group.project), "sessions": sessions })
        })
        .collect();
    Ok(Value::Array(groups))
}

/// One line per project: name, worktree, remote, base model and the number
/// of live sessions.
///
/// # Errors
///
/// Returns an error if storage fails.
pub fn projects_text(storage: &Storage) -> Result<String> {
    let groups = storage.list_sessions_grouped()?;
    if groups.is_empty() {
        return Ok("No projects found\n".to_string());
    }
    let mut text = String::new();
    for group in groups {
        let count = group.sessions.len();
        let project = Project::from(group.project);
        text.push_str(&format!(
            "{} | {} | {} | {} | {} | {count} session(s)\n",
            short(&project.id),
            project.name,
            project.worktree.display(),
            remote_text(&project),
            project.base_model_label().unwrap_or_default(),
        ));
    }
    Ok(text)
}

/// A multi-line description of one project.
#[must_use]
pub fn project_text(project: &Project, live_sessions: usize) -> String {
    let remote = project.remote.as_ref().map_or_else(
        || "none".to_string(),
        |r| {
            format!(
                "{} {} on {}{}",
                r.kind,
                r.path,
                r.host,
                r.default_branch
                    .as_deref()
                    .map(|b| format!(" (default branch {b})"))
                    .unwrap_or_default()
            )
        },
    );
    format!(
        "Project:    {}\nId:         {}\nWorktree:   {}\nRemote:     {remote}\n\
         Base model: {}\nSessions:   {live_sessions}\n",
        project.name,
        project.id,
        project.worktree.display(),
        project
            .base_model_label()
            .unwrap_or_else(|| "none".to_string()),
    )
}
