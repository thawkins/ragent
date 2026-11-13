//! Binding a request the agent just created to its session (spec
//! SESSIONMENU T4.3).

use std::path::PathBuf;
use std::sync::Arc;

use ragent_storage::{SessionKind, SessionRequest, Storage};
use ragent_tools_vcs::ToolContext;
use ragent_tools_vcs::binding::{bind_created, web_url_host};

fn created(number: u64, web_url: &str) -> SessionRequest {
    SessionRequest {
        session_id: String::new(),
        forge: "github".to_string(),
        host: String::new(),
        repo: "acme/app".to_string(),
        number,
        title: "New".to_string(),
        web_url: web_url.to_string(),
        head_branch: "feature".to_string(),
        state: "open".to_string(),
    }
}

fn ctx(session_id: &str, storage: Option<Arc<Storage>>) -> ToolContext {
    ToolContext {
        session_id: session_id.to_string(),
        working_dir: PathBuf::from("/nonexistent"),
        storage: storage.map(|s| s as Arc<dyn ragent_tools_vcs::storage::StorageBackend>),
        config: None,
    }
}

#[test]
fn test_request_binding_web_url_host() {
    assert_eq!(
        web_url_host("https://GitHub.com/acme/app/pull/1").as_deref(),
        Some("github.com")
    );
    assert_eq!(
        web_url_host("https://gitlab.example.com:8443/g/p/-/merge_requests/2").as_deref(),
        Some("gitlab.example.com:8443")
    );
    assert_eq!(web_url_host("not a url"), None);
}

#[test]
fn test_request_binding_created_request_binds_top_level_session() {
    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    storage
        .create_session("lead", "/nonexistent")
        .expect("lead");
    storage
        .create_session_with("sub", "/nonexistent", Some("lead"), SessionKind::Subagent)
        .expect("sub");

    bind_created(
        &ctx("sub", Some(Arc::clone(&storage))),
        created(7, "https://github.com/acme/app/pull/7"),
    );

    let bound = storage
        .session_request("lead")
        .expect("read")
        .expect("bound to the top-level session");
    assert_eq!(bound.number, 7);
    assert_eq!(bound.host, "github.com");
    assert_eq!(bound.session_id, "lead");
}

#[test]
fn test_request_binding_skipped_without_storage_or_number() {
    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    storage.create_session("s", "/nonexistent").expect("s");
    // No storage: nothing to do, and no panic.
    bind_created(
        &ctx("s", None),
        created(1, "https://github.com/acme/app/pull/1"),
    );
    // A create response without a number is not bound.
    bind_created(
        &ctx("s", Some(Arc::clone(&storage))),
        created(0, "https://github.com/acme/app/pull/0"),
    );
    assert!(storage.session_request("s").expect("read").is_none());
}
