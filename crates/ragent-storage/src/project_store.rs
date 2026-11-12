//! Project records and per-session checkout data (spec `projects`).
//!
//! A child module of [`crate::storage`] so it can use the connection and its
//! lock macros. Depends on `rusqlite`, `chrono` and `uuid`.

use std::collections::HashMap;

use anyhow::Result;
use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, params};

use super::{SessionKind, SessionRow, Storage, session_row_v5};
use crate::project::{ProjectRemoteRow, ProjectRow, SessionCheckout, default_project_name};

/// Column list matching [`project_row`].
const PROJECT_COLUMNS: &str = "id, name, worktree, provider_id, model_id, remote_kind, \
     remote_host, remote_path, remote_url, remote_name, default_branch, remote_checked, \
     created_at, updated_at, archived_at";

/// Maps a `projects` row selected with [`PROJECT_COLUMNS`].
fn project_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProjectRow> {
    let kind: Option<String> = row.get(5)?;
    let remote = match kind {
        Some(kind) => Some(ProjectRemoteRow {
            kind,
            host: row.get::<_, Option<String>>(6)?.unwrap_or_default(),
            path: row.get::<_, Option<String>>(7)?.unwrap_or_default(),
            url: row.get::<_, Option<String>>(8)?.unwrap_or_default(),
            remote_name: row
                .get::<_, Option<String>>(9)?
                .unwrap_or_else(|| "origin".to_string()),
            default_branch: row.get(10)?,
        }),
        None => None,
    };
    Ok(ProjectRow {
        id: row.get(0)?,
        name: row.get(1)?,
        worktree: row.get(2)?,
        provider_id: row.get(3)?,
        model_id: row.get(4)?,
        remote,
        remote_checked: row.get::<_, i64>(11)? != 0,
        created_at: row.get(12)?,
        updated_at: row.get(13)?,
        archived_at: row.get(14)?,
    })
}

/// Returns the project whose worktree is `worktree`, creating it when there
/// is none and restoring it when it was archived.
///
/// Takes a plain connection so the migration can call it inside its
/// transaction.
pub(super) fn ensure_project_on(conn: &Connection, worktree: &str) -> Result<ProjectRow> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO projects (id, name, worktree, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?4) ON CONFLICT(worktree) DO UPDATE SET archived_at = NULL",
        params![
            uuid::Uuid::new_v4().to_string(),
            default_project_name(worktree),
            worktree,
            now
        ],
    )?;
    let row = conn.query_row(
        &format!("SELECT {PROJECT_COLUMNS} FROM projects WHERE worktree = ?1"),
        params![worktree],
        project_row,
    )?;
    Ok(row)
}

/// Resolves the project id for a new session in `directory`.
///
/// A directory that is some session's own worktree belongs to that session's
/// project; otherwise the project is the one rooted at the directory's work
/// tree root.
pub(super) fn project_id_for_directory(conn: &Connection, directory: &str) -> Result<String> {
    let root = crate::project::project_root(directory);
    let owner: Option<String> = conn
        .query_row(
            "SELECT project_id FROM sessions WHERE worktree = ?1 AND project_id != '' LIMIT 1",
            params![root],
            |r| r.get(0),
        )
        .optional()?;
    match owner {
        Some(id) => Ok(id),
        None => Ok(ensure_project_on(conn, &root)?.id),
    }
}

impl Storage {
    /// Returns the project rooted at `worktree`, creating it if needed.
    ///
    /// # Arguments
    ///
    /// * `worktree` - canonical root of the local checkout.
    ///
    /// # Errors
    ///
    /// Returns an error if the database write or read fails.
    ///
    /// # Examples
    ///
    /// ```
    /// use ragent_storage::storage::Storage;
    ///
    /// let storage = Storage::open_in_memory().unwrap();
    /// let a = storage.ensure_project("/work/app").unwrap();
    /// let b = storage.ensure_project("/work/app").unwrap();
    /// assert_eq!(a.id, b.id);
    /// assert_eq!(a.name, "app");
    /// ```
    pub fn ensure_project(&self, worktree: &str) -> Result<ProjectRow> {
        let conn = lock_conn!(self)?;
        ensure_project_on(&conn, worktree)
    }

    /// Fetches a project by id, archived ones included.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub fn get_project(&self, id: &str) -> Result<Option<ProjectRow>> {
        let conn = lock_conn_read!(self)?;
        let row = conn
            .query_row(
                &format!("SELECT {PROJECT_COLUMNS} FROM projects WHERE id = ?1"),
                params![id],
                project_row,
            )
            .optional()?;
        Ok(row)
    }

    /// Fetches the project rooted at `worktree`, archived ones included.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub fn get_project_by_worktree(&self, worktree: &str) -> Result<Option<ProjectRow>> {
        let conn = lock_conn_read!(self)?;
        let row = conn
            .query_row(
                &format!("SELECT {PROJECT_COLUMNS} FROM projects WHERE worktree = ?1"),
                params![worktree],
                project_row,
            )
            .optional()?;
        Ok(row)
    }

    /// Lists the non-archived projects, most recently updated first.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub fn list_projects(&self) -> Result<Vec<ProjectRow>> {
        let conn = lock_conn_read!(self)?;
        let mut stmt = conn.prepare_cached(&format!(
            "SELECT {PROJECT_COLUMNS} FROM projects WHERE archived_at IS NULL \
             ORDER BY updated_at DESC, name"
        ))?;
        let rows = stmt
            .query_map([], project_row)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Finds a non-archived project by exact id, exact name
    /// (case-insensitive), or unique id prefix.
    ///
    /// # Arguments
    ///
    /// * `key` - what the user typed.
    ///
    /// # Returns
    ///
    /// `Ok(None)` when nothing matches.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails, or if `key` matches more than one
    /// project.
    pub fn find_project(&self, key: &str) -> Result<Option<ProjectRow>> {
        let projects = self.list_projects()?;
        if let Some(exact) = projects.iter().find(|p| p.id == key) {
            return Ok(Some(exact.clone()));
        }
        let mut matches: Vec<&ProjectRow> = projects
            .iter()
            .filter(|p| p.name.eq_ignore_ascii_case(key))
            .collect();
        if matches.is_empty() && !key.is_empty() {
            matches = projects.iter().filter(|p| p.id.starts_with(key)).collect();
        }
        match matches.as_slice() {
            [] => Ok(None),
            [one] => Ok(Some((*one).clone())),
            many => anyhow::bail!(
                "'{key}' matches {} projects; use the project id instead",
                many.len()
            ),
        }
    }

    /// Sets a project's display name.
    ///
    /// # Errors
    ///
    /// Returns an error if `name` is blank or the update fails.
    pub fn rename_project(&self, id: &str, name: &str) -> Result<()> {
        let name = name.trim();
        anyhow::ensure!(!name.is_empty(), "a project name cannot be empty");
        let conn = lock_conn!(self)?;
        conn.execute(
            "UPDATE projects SET name = ?1, updated_at = ?2 WHERE id = ?3",
            params![name, Utc::now().to_rfc3339(), id],
        )?;
        Ok(())
    }

    /// Sets or clears the base provider/model new sessions start with.
    ///
    /// # Arguments
    ///
    /// * `id` - the project.
    /// * `model` - `(provider_id, model_id)`, or `None` to clear it.
    ///
    /// # Errors
    ///
    /// Returns an error if the update fails.
    pub fn set_project_base_model(&self, id: &str, model: Option<(&str, &str)>) -> Result<()> {
        let conn = lock_conn!(self)?;
        conn.execute(
            "UPDATE projects SET provider_id = ?1, model_id = ?2, updated_at = ?3 WHERE id = ?4",
            params![
                model.map(|m| m.0),
                model.map(|m| m.1),
                Utc::now().to_rfc3339(),
                id
            ],
        )?;
        Ok(())
    }

    /// Records the result of remote detection: the remote, or `None` when
    /// the worktree has no recognised remote. Either way the project is
    /// marked as checked.
    ///
    /// # Errors
    ///
    /// Returns an error if the update fails.
    pub fn set_project_remote(&self, id: &str, remote: Option<&ProjectRemoteRow>) -> Result<()> {
        let conn = lock_conn!(self)?;
        conn.execute(
            "UPDATE projects SET remote_kind = ?1, remote_host = ?2, remote_path = ?3, \
             remote_url = ?4, remote_name = ?5, default_branch = ?6, remote_checked = 1, \
             updated_at = ?7 WHERE id = ?8",
            params![
                remote.map(|r| r.kind.as_str()),
                remote.map(|r| r.host.as_str()),
                remote.map(|r| r.path.as_str()),
                remote.map(|r| r.url.as_str()),
                remote.map(|r| r.remote_name.as_str()),
                remote.and_then(|r| r.default_branch.as_deref()),
                Utc::now().to_rfc3339(),
                id
            ],
        )?;
        Ok(())
    }

    /// Removes a project from the list. Its sessions and files are kept; the
    /// project returns when a session is next created in its worktree.
    ///
    /// # Errors
    ///
    /// Returns an error if the project still has live sessions, or if the
    /// update fails.
    pub fn archive_project(&self, id: &str) -> Result<()> {
        let conn = lock_conn!(self)?;
        let live: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sessions WHERE project_id = ?1 AND archived_at IS NULL",
            params![id],
            |r| r.get(0),
        )?;
        anyhow::ensure!(
            live == 0,
            "the project still has {live} live session(s); archive them first"
        );
        conn.execute(
            "UPDATE projects SET archived_at = ?1 WHERE id = ?2",
            params![Utc::now().to_rfc3339(), id],
        )?;
        Ok(())
    }

    /// Creates an interactive session in a named project, optionally with a
    /// session-owned checkout.
    ///
    /// # Arguments
    ///
    /// * `id` - the new session id.
    /// * `project_id` - the owning project; must exist.
    /// * `directory` - the session's working directory.
    /// * `checkout` - the session's branch and worktree, if it owns one.
    ///
    /// # Errors
    ///
    /// Returns an error if the project does not exist or the insert fails.
    ///
    /// # Examples
    ///
    /// ```
    /// use ragent_storage::SessionCheckout;
    /// use ragent_storage::storage::Storage;
    ///
    /// let storage = Storage::open_in_memory().unwrap();
    /// let project = storage.ensure_project("/work/app").unwrap();
    /// let checkout = SessionCheckout {
    ///     branch: Some("feature".into()),
    ///     worktree: Some("/work/trees/feature".into()),
    ///     pr_count: None,
    /// };
    /// storage
    ///     .create_session_in_project("s1", &project.id, "/work/trees/feature", &checkout)
    ///     .unwrap();
    /// assert_eq!(storage.session_checkout("s1").unwrap(), Some(checkout));
    /// ```
    pub fn create_session_in_project(
        &self,
        id: &str,
        project_id: &str,
        directory: &str,
        checkout: &SessionCheckout,
    ) -> Result<()> {
        let conn = lock_conn!(self)?;
        let exists: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM projects WHERE id = ?1)",
            params![project_id],
            |r| r.get(0),
        )?;
        anyhow::ensure!(exists, "project not found: {project_id}");
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO sessions \
             (id, project_id, directory, kind, branch, worktree, pr_count, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
            params![
                id,
                project_id,
                directory,
                SessionKind::Interactive.as_str(),
                checkout.branch,
                checkout.worktree,
                checkout.pr_count,
                now
            ],
        )?;
        conn.execute(
            "UPDATE projects SET archived_at = NULL, updated_at = ?1 WHERE id = ?2",
            params![now, project_id],
        )?;
        Ok(())
    }

    /// The checkout data of one session, or `None` when it does not exist.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub fn session_checkout(&self, id: &str) -> Result<Option<SessionCheckout>> {
        let conn = lock_conn_read!(self)?;
        let row = conn
            .query_row(
                "SELECT branch, worktree, pr_count FROM sessions WHERE id = ?1",
                params![id],
                |r| {
                    Ok(SessionCheckout {
                        branch: r.get(0)?,
                        worktree: r.get(1)?,
                        pr_count: r.get(2)?,
                    })
                },
            )
            .optional()?;
        Ok(row)
    }

    /// Checkout data for every non-archived session that has a branch, keyed
    /// by session id.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub fn session_checkouts(&self) -> Result<HashMap<String, SessionCheckout>> {
        let conn = lock_conn_read!(self)?;
        let mut stmt = conn.prepare_cached(
            "SELECT id, branch, worktree, pr_count FROM sessions \
             WHERE archived_at IS NULL AND branch IS NOT NULL",
        )?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    SessionCheckout {
                        branch: r.get(1)?,
                        worktree: r.get(2)?,
                        pr_count: r.get(3)?,
                    },
                ))
            })?
            .collect::<std::result::Result<_, _>>()?;
        Ok(rows)
    }

    /// Sets a session's branch and worktree.
    ///
    /// # Errors
    ///
    /// Returns an error if the update fails.
    pub fn set_session_checkout(
        &self,
        id: &str,
        branch: Option<&str>,
        worktree: Option<&str>,
    ) -> Result<()> {
        let conn = lock_conn!(self)?;
        conn.execute(
            "UPDATE sessions SET branch = ?1, worktree = ?2 WHERE id = ?3",
            params![branch, worktree, id],
        )?;
        Ok(())
    }

    /// Stores the last known pull/merge request count for every session of
    /// `project_id` on `branch`. `None` marks the count unknown.
    ///
    /// # Returns
    ///
    /// The number of sessions whose stored count changed.
    ///
    /// # Errors
    ///
    /// Returns an error if the update fails.
    pub fn set_branch_pr_count(
        &self,
        project_id: &str,
        branch: &str,
        count: Option<i64>,
    ) -> Result<usize> {
        let conn = lock_conn!(self)?;
        let changed = conn.execute(
            "UPDATE sessions SET pr_count = ?1 \
             WHERE project_id = ?2 AND branch = ?3 AND pr_count IS NOT ?1",
            params![count, project_id, branch],
        )?;
        Ok(changed)
    }

    /// The distinct branches of a project's non-archived sessions.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub fn project_session_branches(&self, project_id: &str) -> Result<Vec<String>> {
        let conn = lock_conn_read!(self)?;
        let mut stmt = conn.prepare_cached(
            "SELECT DISTINCT branch FROM sessions \
             WHERE project_id = ?1 AND archived_at IS NULL AND branch IS NOT NULL",
        )?;
        let rows = stmt
            .query_map(params![project_id], |r| r.get(0))?
            .collect::<std::result::Result<_, _>>()?;
        Ok(rows)
    }

    /// Lists a project's sessions, newest first.
    ///
    /// # Arguments
    ///
    /// * `project_id` - the project.
    /// * `archived` - `true` lists the archived sessions instead of the live
    ///   ones.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub fn list_project_sessions(
        &self,
        project_id: &str,
        archived: bool,
    ) -> Result<Vec<SessionRow>> {
        let conn = lock_conn_read!(self)?;
        let filter = if archived {
            "archived_at IS NOT NULL"
        } else {
            "archived_at IS NULL"
        };
        let mut stmt = conn.prepare_cached(&format!(
            "SELECT id, title, project_id, directory, parent_id, version, format_version, \
             created_at, updated_at, archived_at, summary FROM sessions \
             WHERE project_id = ?1 AND {filter} AND parent_id IS NULL AND kind = 'interactive' \
             ORDER BY updated_at DESC"
        ))?;
        let rows = stmt
            .query_map(params![project_id], session_row_v5)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }
}
