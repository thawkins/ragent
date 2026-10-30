//! Tests for `/research cluster` slash-command parsing and validation.

#[path = "support/mod.rs"]
mod support;

use std::sync::MutexGuard;

use ragent_tui::App;

/// A test [`App`] built on the shared crate-wide fixture.
///
/// Picks a model that resolves without runtime discovery so the cluster
/// command can build a `ModelRef` and proceed to the async phase.
fn make_app() -> App {
    let mut app = support::make_app();
    app.selected_model = Some("gemini/gemini-2.0-flash".to_string());
    app.selected_model_ctx_window = Some(1_048_576);
    app
}

struct CwdGuard {
    prev: std::path::PathBuf,
    #[allow(dead_code)]
    lock: MutexGuard<'static, ()>,
    #[allow(dead_code)]
    temp: Option<tempfile::TempDir>,
}

impl Drop for CwdGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.prev);
    }
}

/// The process-wide cwd/env lock (see `tests/all.rs`).
fn cwd_lock() -> MutexGuard<'static, ()> {
    crate::global_env_lock()
}

fn enter_with_cwd() -> CwdGuard {
    let lock = cwd_lock();
    let prev = std::env::current_dir().expect("current dir");
    let temp = tempfile::TempDir::new().expect("tempdir");
    std::env::set_current_dir(temp.path()).expect("set_current_dir");
    CwdGuard {
        prev,
        lock,
        temp: Some(temp),
    }
}

#[tokio::test]
async fn test_research_cluster_valid_sources() {
    let _guard = enter_with_cwd();
    std::fs::create_dir_all("research/foo/sources").expect("create dirs");
    std::fs::write("research/foo/sources/web-01.md", "body").expect("write source");

    let mut app = make_app();
    app.session_id = Some("s1".to_string());
    app.execute_slash_command("/research cluster foo").await;

    assert!(app.status.starts_with("[wait]"), "status: {}", app.status);
    assert!(
        app.status.contains("reading sources"),
        "status: {}",
        app.status
    );
    let text = app.messages.last().unwrap().text_content();
    assert!(text.contains("Request accepted for `foo`"), "{text}");
    assert!(text.contains("force=false"), "{text}");
}

#[tokio::test]
async fn test_research_cluster_valid_sources_force() {
    let _guard = enter_with_cwd();
    std::fs::create_dir_all("research/bar/sources").expect("create dirs");
    std::fs::write("research/bar/sources/web-01.md", "body").expect("write source");

    let mut app = make_app();
    app.session_id = Some("s2".to_string());
    app.execute_slash_command("/research cluster bar --force")
        .await;

    assert!(app.status.starts_with("[wait]"), "status: {}", app.status);
    assert!(
        app.status.contains("reading sources"),
        "status: {}",
        app.status
    );
    let text = app.messages.last().unwrap().text_content();
    assert!(text.contains("force=true"), "{text}");
}

#[tokio::test]
async fn test_research_cluster_missing_folder() {
    let _guard = enter_with_cwd();

    let mut app = make_app();
    app.session_id = Some("s3".to_string());

    app.execute_slash_command("/research cluster missing").await;

    assert_eq!(app.status, "research: cluster 'missing' folder missing");
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("research folder `research/missing` does not exist"),
        "{text}"
    );
}

#[tokio::test]
async fn test_research_cluster_no_sources_folder() {
    let _guard = enter_with_cwd();
    std::fs::create_dir_all("research/no-sources").expect("create dirs");

    let mut app = make_app();
    app.session_id = Some("s4".to_string());

    app.execute_slash_command("/research cluster no-sources")
        .await;

    assert_eq!(app.status, "research: cluster 'no-sources' no sources");
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("`research/no-sources/sources/` folder not found"),
        "{text}"
    );
}

#[tokio::test]
async fn test_research_cluster_empty_sources() {
    let _guard = enter_with_cwd();
    std::fs::create_dir_all("research/empty/sources").expect("create dirs");

    let mut app = make_app();
    app.session_id = Some("s5".to_string());

    app.execute_slash_command("/research cluster empty").await;

    assert_eq!(app.status, "research: cluster 'empty' empty sources");
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("`research/empty/sources/` is empty"),
        "{text}"
    );
}

#[tokio::test]
async fn test_research_cluster_rejects_existing_concepts_without_force() {
    let _guard = enter_with_cwd();
    std::fs::create_dir_all("research/existing/sources").expect("create dirs");
    std::fs::write("research/existing/sources/web-01.md", "body").expect("write source");
    std::fs::write("research/existing/CONCEPTS.md", "old").expect("write concepts");

    let mut app = make_app();
    app.session_id = Some("s6".to_string());

    app.execute_slash_command("/research cluster existing")
        .await;

    assert_eq!(app.status, "research: cluster 'existing' already clustered");
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("`research/existing/CONCEPTS.md` already exists"),
        "{text}"
    );
    assert!(text.contains("`--force`"), "{text}");
}

#[tokio::test]
async fn test_research_cluster_force_overwrites_existing_concepts() {
    let _guard = enter_with_cwd();
    std::fs::create_dir_all("research/forced/sources").expect("create dirs");
    std::fs::write("research/forced/sources/web-01.md", "body").expect("write source");
    std::fs::write("research/forced/CONCEPTS.md", "old").expect("write concepts");

    let mut app = make_app();
    app.session_id = Some("s7".to_string());
    app.execute_slash_command("/research cluster forced --force")
        .await;

    assert!(app.status.starts_with("[wait]"), "status: {}", app.status);
    assert!(
        app.status.contains("reading sources"),
        "status: {}",
        app.status
    );
    let text = app.messages.last().unwrap().text_content();
    assert!(text.contains("force=true"), "{text}");
    assert!(text.contains("Request accepted for `forced`"), "{text}");
}
