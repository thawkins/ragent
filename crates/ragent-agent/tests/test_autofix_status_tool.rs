//! `autofix_status` tool: watched pull/merge requests and the Auto-fix
//! watcher state (spec SESSIONMENU). No network: bindings are written
//! straight to storage and the watcher is ticked with no bindings to poll.

use std::path::PathBuf;
use std::sync::Arc;

use ragent_agent::autofix::{AutofixWatcher, HEARTBEAT_KEY, WatcherHeartbeat};
use ragent_agent::event::EventBus;
use ragent_agent::session::permissions::{
    is_hardwired_auto_approved_tool, is_write_capable_permission_category,
};
use ragent_agent::tool::{ToolContext, create_default_registry};
use ragent_storage::{SessionRequest, Storage};
use serde_json::{Value, json};

fn ctx(storage: Arc<Storage>) -> ToolContext {
    ToolContext {
        session_id: "s1".to_string(),
        working_dir: PathBuf::from("target/temp"),
        event_bus: Arc::new(EventBus::new(16)),
        storage: Some(storage),
        agent_manager: None,
        active_model: None,
        provider_registry: None,
        team_context: None,
        team_manager: None,
        code_index: None,
        bg_service: None,
        spec_manager: None,
        active_spec_id: None,
        config: None,
        allowed_roots: Vec::new(),
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        cached_team_dir: Arc::new(std::sync::Mutex::new(None)),
        permission_checker: None,
        tool_registry: ToolContext::default_tool_registry(),
        canonical_cache: Arc::new(ragent_tools_core::CanonicalPathCache::new()),
    }
}

fn bind(storage: &Storage, session: &str, forge: &str, number: u64, state: &str) {
    storage
        .create_session(session, "/nonexistent/autofix-status")
        .expect("session");
    storage
        .set_session_request(&SessionRequest {
            session_id: session.to_string(),
            forge: forge.to_string(),
            host: "example.invalid".to_string(),
            repo: "acme/app".to_string(),
            number,
            title: format!("Request {number}"),
            web_url: format!("https://example.invalid/acme/app/{number}"),
            head_branch: format!("b{number}"),
            state: state.to_string(),
        })
        .expect("bind");
}

async fn run(storage: &Arc<Storage>, input: Value) -> (String, Value) {
    let registry = create_default_registry();
    let tool = registry.get("autofix_status").expect("registered");
    let out = tool
        .execute(input, &ctx(Arc::clone(storage)))
        .await
        .expect("execute");
    (out.content, out.metadata.expect("metadata"))
}

#[test]
fn test_autofix_status_tool_is_registered_read_only_and_auto_approved() {
    let registry = create_default_registry();
    let tool = registry.get("autofix_status").expect("registered");
    assert_eq!(tool.permission_category(), "system");
    assert!(!is_write_capable_permission_category(
        tool.permission_category()
    ));
    assert!(is_hardwired_auto_approved_tool("autofix_status"));
}

#[tokio::test]
async fn test_autofix_status_tool_lists_watched_requests() {
    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    bind(&storage, "watched", "github", 7, "open");
    storage.set_session_autofix("watched", true).expect("on");
    bind(&storage, "off", "gitlab", 8, "open");
    bind(&storage, "paused", "github", 9, "open");
    storage.set_session_autofix("paused", true).expect("on");
    storage.set_autofix_paused("paused", true).expect("pause");
    bind(&storage, "merged", "github", 10, "merged");
    storage.set_session_autofix("merged", true).expect("on");

    let (text, meta) = run(&storage, json!({})).await;
    assert_eq!(meta["bound_count"], 4);
    assert_eq!(meta["watched_count"], 1);
    let requests = meta["requests"].as_array().expect("requests");
    assert_eq!(requests.len(), 1, "only watched ones by default: {meta}");
    assert_eq!(requests[0]["label"], "PR #7");
    assert_eq!(requests[0]["autofix"], "on");
    assert_eq!(requests[0]["watched"], true);
    assert!(text.contains("PR #7"), "{text}");
    assert!(!text.contains("MR !8"), "{text}");
    assert_eq!(meta["watcher"]["state"], "never_started");
    assert!(text.contains("never started"), "{text}");
    assert_eq!(meta["config"]["max_attempts"], 3);

    let (text, meta) = run(&storage, json!({ "all": true })).await;
    let modes: Vec<(&str, &str)> = meta["requests"]
        .as_array()
        .expect("requests")
        .iter()
        .map(|r| {
            (
                r["label"].as_str().unwrap_or_default(),
                r["autofix"].as_str().unwrap_or_default(),
            )
        })
        .collect();
    assert_eq!(
        modes,
        [
            ("PR #10", "on"),
            ("MR !8", "off"),
            ("PR #9", "paused"),
            ("PR #7", "on")
        ]
    );
    assert!(text.contains("MR !8") && text.contains("merged"), "{text}");
}

#[tokio::test]
async fn test_autofix_status_tool_reports_watcher_heartbeat() {
    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    let watcher = AutofixWatcher::new(
        Arc::clone(&storage),
        Arc::new(EventBus::default()),
        Arc::new(|_| None),
        &ragent_config::VcsAutofixConfig::default(),
    );
    assert_eq!(watcher.tick().await.len(), 0);

    let beat = WatcherHeartbeat::load(&storage).expect("heartbeat stored");
    assert_eq!(beat.pid, std::process::id());
    assert_eq!(beat.poll_secs, 60);
    assert!(beat.is_live(chrono::Utc::now()));

    let (text, meta) = run(&storage, json!({})).await;
    assert_eq!(meta["watcher"]["state"], "running");
    assert_eq!(meta["watcher"]["pid"], std::process::id());
    assert!(text.contains("Watcher: running"), "{text}");

    // A heartbeat older than two intervals reads as stopped.
    let stale = WatcherHeartbeat {
        last_tick_at: "2020-01-01T00:00:00+00:00".to_string(),
        ..beat
    };
    storage
        .set_setting(HEARTBEAT_KEY, &serde_json::to_string(&stale).expect("json"))
        .expect("store");
    let (_, meta) = run(&storage, json!({})).await;
    assert_eq!(meta["watcher"]["state"], "stopped");
}

#[tokio::test]
async fn test_autofix_status_tool_records_poll_problems() {
    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    bind(&storage, "s", "github", 7, "open");
    storage.set_session_autofix("s", true).expect("on");
    // No credentials for the host: the poll is skipped and reported.
    let watcher = AutofixWatcher::new(
        Arc::clone(&storage),
        Arc::new(EventBus::default()),
        Arc::new(|_| None),
        &ragent_config::VcsAutofixConfig::default(),
    );
    watcher.tick().await;

    let (text, meta) = run(&storage, json!({})).await;
    assert_eq!(meta["watcher"]["watched_last_poll"], 1);
    let problems = meta["watcher"]["problems"].as_array().expect("problems");
    assert_eq!(problems.len(), 1, "{meta}");
    assert!(
        problems[0]
            .as_str()
            .is_some_and(|p| p.contains("PR #7") && p.contains("no github credentials")),
        "{problems:?}"
    );
    assert!(text.contains("problem: PR #7"), "{text}");
}
