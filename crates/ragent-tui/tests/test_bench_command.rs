//! Regression tests for `/bench` slash commands.

use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use ragent_bench::{BenchProgressHandle, BenchRunEvent, BenchRunOutcome, BenchRunProgress};
use ragent_core::{
    agent,
    event::EventBus,
    permission::PermissionChecker,
    provider,
    session::{SessionManager, processor::SessionProcessor},
    storage::Storage,
    tool,
};
use ragent_tui::App;

fn make_app() -> App {
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    let event_bus = Arc::new(EventBus::default());
    let provider_registry = Arc::new(provider::create_default_registry());
    let tool_registry = Arc::new(tool::create_default_registry());
    let permission_checker = Arc::new(tokio::sync::RwLock::new(PermissionChecker::new(vec![])));
    let session_manager = Arc::new(SessionManager::new(storage.clone(), event_bus.clone()));
    let session_processor = Arc::new(SessionProcessor {
        session_manager,
        provider_registry: provider_registry.clone(),
        tool_registry,
        permission_checker,
        event_bus: event_bus.clone(),
        task_manager: std::sync::OnceLock::new(),
        team_manager: std::sync::OnceLock::new(),
        mcp_client: std::sync::OnceLock::new(),
        code_index: std::sync::OnceLock::new(),
        extraction_engine: std::sync::OnceLock::new(),
        stream_config: ragent_core::config::StreamConfig::default(),
        auto_approve: false,
    });
    let agent_info =
        agent::resolve_agent("general", &Default::default()).expect("resolve general agent");

    App::new(
        event_bus,
        storage,
        provider_registry,
        session_processor,
        agent_info,
        false,
    )
}

struct CwdGuard(std::path::PathBuf);

impl Drop for CwdGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.0);
    }
}

fn cwd_test_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn enter_temp_project_dir() -> (tempfile::TempDir, CwdGuard) {
    let original = std::env::current_dir().expect("cwd");
    let temp = tempfile::tempdir().expect("tempdir");
    std::env::set_current_dir(temp.path()).expect("set cwd");
    std::fs::create_dir_all(temp.path().join(".ragent")).expect("create .ragent");
    (temp, CwdGuard(original))
}

fn configured_app() -> App {
    let mut app = make_app();
    app.session_id = Some("bench-test-session".to_string());
    app.selected_model = Some("anthropic/claude-sonnet-4-20250514".to_string());
    app.bench_mock_outputs = Some(vec![
        "def solve():\n    return 'bench-output'".to_string(),
        "def solve():\n    return 'bench-output-2'".to_string(),
    ]);
    app
}

fn all_message_text(app: &App) -> String {
    app.messages
        .iter()
        .map(|message| message.text_content())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn test_bench_list_shows_suites_and_profiles() {
    let mut app = configured_app();

    app.execute_slash_command("/bench list");

    let text = app
        .messages
        .last()
        .expect("bench list message")
        .text_content();
    assert!(text.contains("Benchmark Suites"));
    assert!(text.contains("language"));
    assert!(text.contains("local partition") || text.contains("local partitions"));
    assert!(text.contains("humaneval"));
    assert!(text.contains("mbpp"));
    assert!(text.contains("bigcode/humanevalpack"));
    assert!(text.contains("gabeorlanski/bc-mbpp"));
    assert!(text.contains("nuprl/MultiPL-E"));
    assert!(text.contains("bigcodebench"));
    assert!(text.contains("python"));
    assert!(text.contains("diff"));
    assert!(text.contains("rust"));
    assert!(text.contains("Virtual Targets"));
    assert!(text.contains("all"));
    assert!(text.contains("full"));
    assert!(text.contains("quick"));
}

#[test]
fn test_bench_init_humaneval_creates_data_root() {
    let _guard = cwd_test_lock().lock().expect("cwd lock");
    let (_temp, _cwd) = enter_temp_project_dir();
    let mut app = configured_app();

    app.execute_slash_command("/bench init humaneval");

    assert!(std::path::Path::new("benches/data/humaneval/python/manifest.json").exists());
    assert!(std::path::Path::new("benches/data/humaneval/python/dataset/cases.jsonl").exists());
    let text = all_message_text(&app);
    assert!(text.contains("Loading benchmark data for `humaneval` [python]"));
    assert!(text.contains("Loaded `humaneval` [python]"));
}

#[test]
fn test_bench_init_verify_only_reports_existing_state() {
    let _guard = cwd_test_lock().lock().expect("cwd lock");
    let (_temp, _cwd) = enter_temp_project_dir();
    let mut app = configured_app();

    app.execute_slash_command("/bench init humaneval");
    app.execute_slash_command("/bench init humaneval --verify-only");

    let text = app
        .messages
        .last()
        .expect("bench verify-only message")
        .text_content();
    assert!(text.contains("verified"));
    assert!(std::path::Path::new("benches/data/humaneval/python/manifest.json").exists());
}

#[test]
fn test_bench_init_all_creates_every_suite_root() {
    let _guard = cwd_test_lock().lock().expect("cwd lock");
    let (_temp, _cwd) = enter_temp_project_dir();
    let mut app = configured_app();

    app.execute_slash_command("/bench init all");

    let text = app
        .messages
        .last()
        .expect("bench init all message")
        .text_content();
    assert!(text.contains("✅ initialized benchmark target."));
    assert!(text.contains("[python]"));
    assert!(text.contains("humaneval"));
    assert!(text.contains("bigcodebench"));
    assert!(std::path::Path::new("benches/data/humaneval/python/manifest.json").exists());
    assert!(std::path::Path::new("benches/data/bigcodebench/python/manifest.json").exists());
}

#[test]
fn test_bench_init_full_reports_gated_support() {
    let _guard = cwd_test_lock().lock().expect("cwd lock");
    let (_temp, _cwd) = enter_temp_project_dir();
    let mut app = configured_app();

    app.execute_slash_command("/bench init full");

    let text = app
        .messages
        .last()
        .expect("bench init full message")
        .text_content();
    assert!(text.contains("not ready yet"));
    assert!(text.contains("apps"));
}

#[test]
fn test_bench_run_humaneval_creates_workbook() {
    let _guard = cwd_test_lock().lock().expect("cwd lock");
    let (_temp, _cwd) = enter_temp_project_dir();
    let mut app = configured_app();

    app.execute_slash_command("/bench init humaneval");
    app.execute_slash_command("/bench run humaneval");

    for _ in 0..50 {
        app.poll_pending_bench();
        if app.active_bench_task_id.is_none() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    assert!(
        app.active_bench_task_id.is_none(),
        "bench task should complete"
    );
    assert_eq!(app.bench_last_workbooks.len(), 1);
    assert!(
        app.bench_last_workbooks[0].exists(),
        "workbook should exist"
    );
    let text = app
        .messages
        .last()
        .expect("bench run output")
        .text_content();
    assert!(text.contains("sample(s)"));
    assert!(text.contains("generated"));
    let all_text = all_message_text(&app);
    assert!(all_text.contains("Running `humaneval` [python]"));
}

#[test]
fn test_bench_run_all_creates_workbooks() {
    let _guard = cwd_test_lock().lock().expect("cwd lock");
    let (_temp, _cwd) = enter_temp_project_dir();
    let mut app = configured_app();

    app.execute_slash_command("/bench init all");
    app.execute_slash_command("/bench run all --yes");

    for _ in 0..100 {
        app.poll_pending_bench();
        if app.active_bench_task_id.is_none() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    assert!(
        app.active_bench_task_id.is_none(),
        "bench task should complete"
    );
    assert!(
        app.bench_last_workbooks.len() > 1,
        "all target should produce multiple workbooks"
    );
    let text = app
        .messages
        .last()
        .expect("bench run all output")
        .text_content();
    assert!(text.contains("humaneval"));
    assert!(text.contains("bigcodebench"));
}

#[test]
fn test_bench_status_reports_active_run_context() {
    let _guard = cwd_test_lock().lock().expect("cwd lock");
    let (_temp, _cwd) = enter_temp_project_dir();
    let mut app = configured_app();

    app.execute_slash_command("/bench init humaneval");
    app.execute_slash_command("/bench run humaneval");
    app.execute_slash_command("/bench status");

    let text = app
        .messages
        .last()
        .expect("bench status output")
        .text_content();
    assert!(text.contains("Active Benchmark Run"));
    assert!(text.contains("Task ID"));
    assert!(text.contains("Summary"));
}

#[test]
fn test_bench_status_reports_case_progress() {
    let mut app = configured_app();
    let progress = BenchProgressHandle::default();
    progress.set(BenchRunProgress {
        suite_id: "humaneval".to_string(),
        suite_index: 1,
        total_suites: 2,
        completed_cases: 3,
        total_cases: 10,
    });

    app.active_bench_task_id = Some("bench-progress".to_string());
    app.active_bench_summary = Some("`quick` on `anthropic/test-model`".to_string());
    app.active_bench_started_at = Some(chrono::Utc::now());
    app.active_bench_progress = Some(progress);

    app.execute_slash_command("/bench status");

    let text = app
        .messages
        .last()
        .expect("bench status output")
        .text_content();
    assert!(text.contains("Progress"));
    assert!(text.contains("suite `humaneval` (1/2)"));
    assert!(text.contains("case `3/10`"));
}

#[test]
fn test_bench_run_progress_events_show_case_id_and_status() {
    let mut app = configured_app();
    let progress = BenchProgressHandle::default();
    progress.push_event(BenchRunEvent::SuiteStarted {
        suite_id: "humaneval".to_string(),
        language: "python".to_string(),
        total_cases: 1,
    });
    progress.push_event(BenchRunEvent::CaseFinished {
        suite_id: "humaneval".to_string(),
        language: "python".to_string(),
        case_id: "humaneval-sample-001".to_string(),
        status: "passed".to_string(),
    });

    app.active_bench_task_id = Some("bench-progress".to_string());
    app.active_bench_progress = Some(progress);
    app.poll_pending_bench();

    let text = all_message_text(&app);
    assert!(text.contains("Running `humaneval` [python]"));
    assert!(text.contains("humaneval-sample-001"));
    assert!(text.contains("`passed`"));
}

#[test]
fn test_bench_run_drains_final_progress_events_before_completion() {
    let mut app = configured_app();
    let progress = BenchProgressHandle::default();
    progress.push_event(BenchRunEvent::CaseFinished {
        suite_id: "mbpp".to_string(),
        language: "rust".to_string(),
        case_id: "mbpp-5".to_string(),
        status: "passed".to_string(),
    });

    app.active_bench_task_id = Some("bench-progress".to_string());
    app.active_bench_progress = Some(progress);
    if let Ok(mut guard) = app.bench_result.lock() {
        *guard = Some(Ok(BenchRunOutcome {
            message: "From: /bench run\n## Benchmark Run\n\n- done".to_string(),
            workbook_paths: Vec::new(),
            summaries: Vec::new(),
        }));
    } else {
        panic!("bench result lock");
    }

    app.poll_pending_bench();

    let text = all_message_text(&app);
    assert!(text.contains("mbpp-5"));
    assert!(text.contains("`passed`"));
    assert!(text.contains("## Benchmark Run"));
}

#[test]
fn test_bench_open_last_shows_latest_workbook_paths() {
    let _guard = cwd_test_lock().lock().expect("cwd lock");
    let (_temp, _cwd) = enter_temp_project_dir();
    let mut app = configured_app();

    app.execute_slash_command("/bench init humaneval");
    app.execute_slash_command("/bench run humaneval");

    for _ in 0..50 {
        app.poll_pending_bench();
        if app.active_bench_task_id.is_none() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    app.execute_slash_command("/bench open last");

    let text = app
        .messages
        .last()
        .expect("bench open last output")
        .text_content();
    assert!(text.contains("Latest Benchmark Results"));
    assert!(text.contains(".xlsx"));
}

#[test]
fn test_bench_cancel_requests_shutdown_for_active_run() {
    let _guard = cwd_test_lock().lock().expect("cwd lock");
    let (_temp, _cwd) = enter_temp_project_dir();
    let mut app = configured_app();

    app.execute_slash_command("/bench init humaneval");
    app.execute_slash_command("/bench run humaneval");
    app.execute_slash_command("/bench cancel");

    let text = app
        .messages
        .last()
        .expect("bench cancel output")
        .text_content();
    assert!(text.contains("Cancellation requested"));
    assert_eq!(app.status, "⏳ bench: cancellation requested");
    assert!(
        app.active_bench_cancel
            .as_ref()
            .is_some_and(|flag| flag.load(std::sync::atomic::Ordering::Relaxed))
    );
}

#[test]
fn test_bench_run_missing_data_fails_fast_with_init_hint() {
    let _guard = cwd_test_lock().lock().expect("cwd lock");
    let (_temp, _cwd) = enter_temp_project_dir();
    let mut app = configured_app();

    app.execute_slash_command("/bench run humaneval");

    let text = app
        .messages
        .last()
        .expect("bench error message")
        .text_content();
    assert!(text.contains("/bench init humaneval"));
    assert!(app.active_bench_task_id.is_none());
}
