//! Project routes (spec `projects`, T-012).
//!
//! `GET /projects`, `POST /projects`, `GET /projects/{id}`,
//! `DELETE /projects/{id}` and `GET /projects/{id}/sessions`, plus the
//! `project_id` form of `POST /sessions`. A project directory is subject to
//! the same containment rule as a session directory (SEC-062).

use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use ragent_agent::project::{Project, ProjectManager};
use serde::Deserialize;

use super::{
    AppState, SessionResponse, check_rate_limit, error_response, internal_error_response,
    permitted_directory, serialize_response,
};

fn manager(state: &AppState) -> ProjectManager {
    ProjectManager::new(Arc::clone(&state.storage), Arc::clone(&state.event_bus))
}

/// `GET /projects` - the listed projects, most recently updated first.
pub(super) async fn list_projects(
    State(state): State<AppState>,
) -> (StatusCode, Json<serde_json::Value>) {
    let manager = manager(&state);
    match tokio::task::spawn_blocking(move || manager.list()).await {
        Ok(Ok(projects)) => serialize_response(projects, "list_projects"),
        Ok(Err(e)) => internal_error_response("list_projects", e),
        Err(e) => internal_error_response("list_projects", e),
    }
}

/// Body of `POST /projects`.
#[derive(Deserialize)]
pub(super) struct CreateProjectRequest {
    directory: String,
}

/// `POST /projects` - register the project that contains `directory`,
/// detecting its remote. Registering a directory twice returns the same
/// project.
pub(super) async fn create_project(
    State(state): State<AppState>,
    Json(body): Json<CreateProjectRequest>,
) -> Response {
    if let Some(rate_limited) = check_rate_limit(&state, "create_project").await {
        return rate_limited.into_response();
    }
    let Some(directory) = permitted_directory(&state, &body.directory).await else {
        return error_response(StatusCode::BAD_REQUEST, "invalid directory").into_response();
    };
    let manager = manager(&state);
    let resolved = tokio::task::spawn_blocking(move || {
        manager.resolve_for_dir(std::path::Path::new(&directory))
    })
    .await;
    match resolved {
        Ok(Ok(project)) => {
            let (_, body) = serialize_response(project, "create_project");
            (StatusCode::CREATED, body).into_response()
        }
        Ok(Err(e)) => internal_error_response("create_project", e).into_response(),
        Err(e) => internal_error_response("create_project (task)", e).into_response(),
    }
}

/// `GET /projects/{id}`.
pub(super) async fn get_project(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    let manager = manager(&state);
    match tokio::task::spawn_blocking(move || manager.get(&id)).await {
        Ok(Ok(Some(project))) => serialize_response(project, "get_project"),
        Ok(Ok(None)) => error_response(StatusCode::NOT_FOUND, "project not found"),
        Ok(Err(e)) => internal_error_response("get_project", e),
        Err(e) => internal_error_response("get_project", e),
    }
}

/// `DELETE /projects/{id}` - remove a project with no live sessions from
/// the list. Its files and archived sessions are kept.
pub(super) async fn remove_project(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    let storage = Arc::clone(&state.storage);
    let manager = manager(&state);
    // `Some(true)` removed, `Some(false)` refused (live sessions), `None`
    // no such project.
    let removed = tokio::task::spawn_blocking(move || {
        if storage.get_project(&id)?.is_none() {
            return Ok::<_, anyhow::Error>(None);
        }
        match manager.remove(&id) {
            Ok(()) => Ok(Some(true)),
            // The only refusal `remove` makes for an existing project.
            Err(e) if e.to_string().contains("live session") => Ok(Some(false)),
            Err(e) => Err(e),
        }
    })
    .await;
    match removed {
        Ok(Ok(Some(true))) => (StatusCode::OK, Json(serde_json::json!({ "ok": true }))),
        Ok(Ok(Some(false))) => {
            error_response(StatusCode::CONFLICT, "the project still has live sessions")
        }
        Ok(Ok(None)) => error_response(StatusCode::NOT_FOUND, "project not found"),
        Ok(Err(e)) => internal_error_response("remove_project", e),
        Err(e) => internal_error_response("remove_project", e),
    }
}

/// `GET /projects/{id}/sessions` - the project's live top-level sessions,
/// newest first.
pub(super) async fn project_sessions(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    let storage = Arc::clone(&state.storage);
    let listed = tokio::task::spawn_blocking(move || {
        if storage.get_project(&id)?.is_none() {
            return Ok::<_, anyhow::Error>(None);
        }
        let mut checkouts = storage.session_checkouts()?;
        let sessions: Vec<SessionResponse> = storage
            .list_project_sessions(&id, false)?
            .into_iter()
            .map(|row| {
                let checkout = checkouts.remove(&row.id);
                SessionResponse::new(row, checkout)
            })
            .collect();
        Ok(Some(sessions))
    })
    .await;
    match listed {
        Ok(Ok(Some(sessions))) => serialize_response(sessions, "project_sessions"),
        Ok(Ok(None)) => error_response(StatusCode::NOT_FOUND, "project not found"),
        Ok(Err(e)) => internal_error_response("project_sessions", e),
        Err(e) => internal_error_response("project_sessions", e),
    }
}

/// The `project_id` form of `POST /sessions`: a session in the project
/// worktree, or, with `branch`, in its own git worktree at that branch.
pub(super) async fn create_session_in_project(
    state: &AppState,
    project_id: String,
    branch: Option<String>,
) -> Response {
    let manager = manager(state);
    let lookup = {
        let manager = manager.clone();
        let project_id = project_id.clone();
        tokio::task::spawn_blocking(move || manager.get(&project_id)).await
    };
    let project: Project = match lookup {
        Ok(Ok(Some(project))) if project.archived_at.is_none() => project,
        Ok(Ok(_)) => {
            return error_response(StatusCode::NOT_FOUND, "project not found").into_response();
        }
        Ok(Err(e)) => return internal_error_response("create_session", e).into_response(),
        Err(e) => return internal_error_response("create_session (task)", e).into_response(),
    };
    // SEC-062 applies to the project's own directory too.
    if permitted_directory(state, &project.worktree.display().to_string())
        .await
        .is_none()
    {
        return error_response(StatusCode::BAD_REQUEST, "invalid directory").into_response();
    }
    let sessions = Arc::clone(&state.session_processor.session_manager);
    let root = Arc::clone(&state.worktree_root);
    let created = tokio::task::spawn_blocking(move || {
        manager.create_session(&sessions, &project.id, branch.as_deref(), &root)
    })
    .await;
    match created {
        Ok(Ok(session)) => {
            // `SessionManager` already published `SessionCreated`.
            (
                StatusCode::CREATED,
                Json(serde_json::json!({
                    "id": session.id,
                    "project_id": session.project_id,
                    "directory": session.directory.display().to_string(),
                    "branch": session.branch,
                    "worktree": session.worktree.map(|w| w.display().to_string()),
                })),
            )
                .into_response()
        }
        Ok(Err(e)) => {
            // A bad branch name or an existing worktree is the caller's
            // mistake; report it without host paths.
            tracing::warn!(error = %e, "session in project not created");
            error_response(
                StatusCode::BAD_REQUEST,
                "the session could not be created for that project and branch",
            )
            .into_response()
        }
        Err(e) => internal_error_response("create_session (task)", e).into_response(),
    }
}
