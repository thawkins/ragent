//! Web interface module for AIWiki - HTTP routes and handlers.
//!
//! Provides web-based wiki browsing, editing, search, and visualization
//! through axum HTTP routes. Served at `/aiwiki/*` path.

use crate::Aiwiki;
use axum::extract::{Path, Query, State};
use axum::response::{Html, IntoResponse, Redirect};
use axum::routing::get;
use std::path::PathBuf;
use std::sync::Arc;

mod templates;
pub use templates::SearchResult;

mod handlers;
pub use handlers::{PageInfo, list_wiki_pages, get_page_content, save_page_content};

mod search;
pub use search::*;

mod graph;
pub use graph::*;

/// State shared across web handlers.
#[derive(Clone)]
pub struct WebState {
    /// Project root directory (for loading wiki).
    pub project_root: PathBuf,
}

impl WebState {
    /// Create new web state.
    pub fn new(project_root: impl Into<PathBuf>) -> Self {
        Self {
            project_root: project_root.into(),
        }
    }

    /// Load AIWiki instance.
    pub async fn load_wiki(&self) -> crate::Result<Aiwiki> {
        Aiwiki::new(&self.project_root).await
    }

    /// Check if wiki exists.
    pub fn wiki_exists(&self) -> bool {
        crate::Aiwiki::exists(&self.project_root)
    }
}

/// Create the AIWiki web router.
pub fn create_router(project_root: impl Into<std::path::PathBuf>) -> axum::Router {
    let state = Arc::new(WebState::new(project_root));

    axum::Router::new()
        // Main routes
        .route("/", get(home_page))
        .route("/page/{*path}", get(view_page))
        .route("/edit/{*path}", get(edit_page).post(save_page))
        .route("/search", get(search_page))
        .route("/graph", get(graph_page))
        .route("/status", get(status_page))
        // API routes
        .route("/api/pages", get(api_pages))
        .route("/api/search", get(api_search))
        .route("/api/graph", get(api_graph))
        .route("/api/status", get(api_status))
        // Static assets
        .route("/static/{*path}", get(serve_static))
        .with_state(state)
}

/// Query parameters for search.
#[derive(Debug, serde::Deserialize)]
pub struct SearchQuery {
    pub q: Option<String>,
    #[serde(rename = "type")]
    pub page_type: Option<String>,
}

/// Query parameters for graph.
#[derive(Debug, serde::Deserialize)]
pub struct GraphQuery {
    pub filter: Option<String>,
}

/// Default home page - redirects to wiki index or shows setup page.
async fn home_page(State(state): State<Arc<WebState>>) -> impl IntoResponse {
    if !state.wiki_exists() {
        return Html(templates::render_setup_page()).into_response();
    }

    // Try to load index.md, otherwise show directory listing
    match state.load_wiki().await {
        Ok(wiki) => {
            let index_path = wiki.path("wiki").join("index.md");
            if index_path.exists() {
                Redirect::to("/aiwiki/page/index.md").into_response()
            } else {
                Html(templates::render_index_page(&wiki)).into_response()
            }
        }
        Err(_) => Html(templates::render_error_page(
            "Failed to load AIWiki",
            "The wiki exists but could not be loaded. Check the configuration."
        )).into_response(),
    }
}

/// View a wiki page.
async fn view_page(
    State(state): State<Arc<WebState>>,
    Path(path): Path<String>,
) -> impl IntoResponse {
    if !state.wiki_exists() {
        return Html(templates::render_setup_page()).into_response();
    }

    match state.load_wiki().await {
        Ok(wiki) => {
            let page_path = wiki.path("wiki").join(&path);

            if !page_path.exists() {
                return Html(templates::render_not_found_page(&path,
                    &wiki
                )).into_response();
            }

            // If the path is a directory, show a listing of its pages.
            if page_path.is_dir() {
                let listing = build_directory_listing(&page_path, &path).await;
                let html = templates::render_directory_page(&path, &listing, &wiki);
                return Html(html).into_response();
            }

            // Read and render the markdown
            match tokio::fs::read_to_string(&page_path).await {
                Ok(content) => {
                    let html = templates::render_markdown_page(&path, &content, &wiki
                    );
                    Html(html).into_response()
                }
                Err(e) => Html(templates::render_error_page(
                    "Failed to read page",
                    &format!("Error reading {}: {}", path, e)
                )).into_response(),
            }
        }
        Err(e) => Html(templates::render_error_page(
            "Failed to load AIWiki",
            &e.to_string()
        )).into_response(),
    }
}

/// Edit page form.
async fn edit_page(
    State(state): State<Arc<WebState>>,
    Path(path): Path<String>,
) -> impl IntoResponse {
    if !state.wiki_exists() {
        return Html(templates::render_setup_page()).into_response();
    }

    match state.load_wiki().await {
        Ok(wiki) => {
            let page_path = wiki.path("wiki").join(&path);

            // Read existing content or start blank
            let content = match tokio::fs::read_to_string(&page_path).await {
                Ok(c) => c,
                Err(_) => {
                    // New page - create template
                    templates::create_page_template(&path
                    )
                }
            };

            Html(templates::render_edit_page(&path, &content, &wiki
            )).into_response()
        }
        Err(e) => Html(templates::render_error_page(
            "Failed to load AIWiki",
            &e.to_string()
        )).into_response(),
    }
}

/// Save edited page.
async fn save_page(
    State(state): State<Arc<WebState>>,
    Path(path): Path<String>,
    body: String,
) -> impl IntoResponse {
    if !state.wiki_exists() {
        return Redirect::to("/aiwiki").into_response();
    }

    match state.load_wiki().await {
        Ok(wiki) => {
            let page_path = wiki.path("wiki").join(&path);

            // Ensure parent directory exists
            if let Some(parent) = page_path.parent() {
                let _ = tokio::fs::create_dir_all(parent).await;
            }

            // Save content
            match tokio::fs::write(&page_path, &body).await {
                Ok(_) => Redirect::to(&format!("/aiwiki/page/{}", path)).into_response(),
                Err(e) => Html(templates::render_error_page(
                    "Failed to save page",
                    &format!("Error saving {}: {}", path, e)
                )).into_response(),
            }
        }
        Err(_) => Redirect::to("/aiwiki").into_response(),
    }
}

/// Search page.
async fn search_page(
    State(state): State<Arc<WebState>>,
    Query(query): Query<SearchQuery>,
) -> impl IntoResponse {
    if !state.wiki_exists() {
        return Html(templates::render_setup_page()).into_response();
    }

    let search_term = query.q.unwrap_or_default();
    let page_type = query.page_type;

    match state.load_wiki().await {
        Ok(wiki) => {
            let results = search::search_wiki(&wiki, &search_term, page_type
            ).await.unwrap_or_default();

            Html(templates::render_search_page(&search_term, &results, &wiki
            )).into_response()
        }
        Err(e) => Html(templates::render_error_page(
            "Failed to load AIWiki",
            &e.to_string()
        )).into_response(),
    }
}

/// Graph visualization page.
async fn graph_page(
    State(state): State<Arc<WebState>>,
    Query(query): Query<GraphQuery>,
) -> impl IntoResponse {
    if !state.wiki_exists() {
        return Html(templates::render_setup_page()).into_response();
    }

    Html(templates::render_graph_page(query.filter.as_deref())).into_response()
}

/// Status dashboard page.
async fn status_page(State(state): State<Arc<WebState>>) -> impl IntoResponse {
    if !state.wiki_exists() {
        return Html(templates::render_setup_page()).into_response();
    }

    match state.load_wiki().await {
        Ok(wiki) => {
            Html(templates::render_status_page(&wiki
            )).into_response()
        }
        Err(e) => Html(templates::render_error_page(
            "Failed to load AIWiki",
            &e.to_string()
        )).into_response(),
    }
}

/// API: List all pages.
async fn api_pages(State(state): State<Arc<WebState>>) -> impl IntoResponse {
    match state.load_wiki().await {
        Ok(wiki) => {
            match handlers::list_wiki_pages(&wiki).await {
                Ok(pages) => axum::Json(pages).into_response(),
                Err(_) => axum::Json(Vec::<handlers::PageInfo>::new()).into_response(),
            }
        }
        Err(_) => axum::Json(Vec::<handlers::PageInfo>::new()).into_response(),
    }
}

/// API: Search results (JSON).
async fn api_search(
    State(state): State<Arc<WebState>>,
    Query(query): Query<SearchQuery>,
) -> impl IntoResponse {
    let search_term = query.q.unwrap_or_default();
    let page_type = query.page_type;

    match state.load_wiki().await {
        Ok(wiki) => {
            let results = search::search_wiki(&wiki, &search_term, page_type
            ).await.unwrap_or_default();
            axum::Json(results).into_response()
        }
        Err(_) => axum::Json(Vec::<templates::SearchResult>::new()).into_response(),
    }
}

/// API: Graph data (JSON).
async fn api_graph(
    State(state): State<Arc<WebState>>,
    Query(query): Query<GraphQuery>,
) -> impl IntoResponse {
    match state.load_wiki().await {
        Ok(wiki) => {
            let graph = graph::build_graph(&wiki, query.filter.as_deref()).await;
            axum::Json(graph).into_response()
        }
        Err(_) => axum::Json(serde_json::json!({
                "nodes": [],
                "links": []
            })).into_response(),
    }
}

/// API: Status (JSON).
async fn api_status(State(state): State<Arc<WebState>>) -> impl IntoResponse {
    match state.load_wiki().await {
        Ok(wiki) => {
            let stats = wiki.state.stats();
            let status = serde_json::json!({
                "name": wiki.config.name,
                "enabled": wiki.config.enabled,
                "version": wiki.config.version,
                "sync_mode": format!("{:?}", wiki.config.sync_mode),
                "llm_model": wiki.config.llm_model,
                "total_sources": stats.total_sources,
                "total_pages": stats.total_pages,
                "last_sync": stats.last_sync,
            });
            axum::Json(status).into_response()
        }
        Err(e) => axum::Json(serde_json::json!({
                "error": e.to_string()
            })).into_response(),
    }
}

/// Serve static assets.
async fn serve_static(
    Path(path): Path<String>,
) -> impl IntoResponse {
    // Embed static assets in binary
    let content = match path.as_str() {
        "css/aiwiki.css" => Some((templates::CSS_CONTENT, "text/css")),
        "js/aiwiki.js" => Some((templates::JS_CONTENT, "application/javascript")),
        _ => None,
    };

    match content {
        Some((data, content_type)) => {
            (
                [(axum::http::header::CONTENT_TYPE, content_type)],
                data
            ).into_response()
        }
        None => (
            axum::http::StatusCode::NOT_FOUND,
            "Not found"
        ).into_response(),
    }
}

/// An entry in a wiki directory listing.
pub struct DirectoryEntry {
    /// Display title (from frontmatter or filename).
    pub title: String,
    /// Link path relative to /aiwiki/page/.
    pub link: String,
    /// True if this is a subdirectory.
    pub is_dir: bool,
}

/// Build a listing of .md files and subdirectories inside `dir_path`.
async fn build_directory_listing(
    dir_path: &std::path::Path,
    rel_prefix: &str,
) -> Vec<DirectoryEntry> {
    let mut entries = Vec::new();
    let mut rd = match tokio::fs::read_dir(dir_path).await {
        Ok(rd) => rd,
        Err(_) => return entries,
    };
    while let Ok(Some(entry)) = rd.next_entry().await {
        let path = entry.path();
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        if name.starts_with('.') || name == "log.md" {
            continue;
        }
        if path.is_dir() {
            entries.push(DirectoryEntry {
                title: name.clone(),
                link: format!("{}/{}", rel_prefix.trim_end_matches('/'), name),
                is_dir: true,
            });
        } else if name.ends_with(".md") {
            let title = extract_title_from_file(&path).await
                .unwrap_or_else(|| name.trim_end_matches(".md").replace('-', " "));
            entries.push(DirectoryEntry {
                title,
                link: format!("{}/{}", rel_prefix.trim_end_matches('/'), name),
                is_dir: false,
            });
        }
    }
    entries.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
    entries
}

/// Extract title from a markdown file's frontmatter or first heading.
async fn extract_title_from_file(path: &std::path::Path) -> Option<String> {
    let content = tokio::fs::read_to_string(path).await.ok()?;
    // Check YAML frontmatter for title
    if content.starts_with("---") {
        if let Some(end) = content[3..].find("---") {
            let fm = &content[3..3 + end];
            for line in fm.lines() {
                let line = line.trim();
                if let Some(rest) = line.strip_prefix("title:") {
                    let t = rest.trim().trim_matches('"').trim_matches('\'');
                    if !t.is_empty() {
                        return Some(t.to_string());
                    }
                }
            }
        }
    }
    // Fallback: first # heading
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("# ") {
            let t = rest.trim();
            if !t.is_empty() {
                return Some(t.to_string());
            }
        }
    }
    None
}
