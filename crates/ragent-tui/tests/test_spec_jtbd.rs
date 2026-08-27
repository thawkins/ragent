//! Integration tests for the `/spec jtbd` slash command (spec `jtbdresearch`, T-009).
//!
//! Covers the dispatch path, the JTBD.md overwrite guard (FR-003), the
//! `--force` bypass (FR-004), error handling for unknown / invalid specs
//! (FR-008), agent-override validation (FR-005), and status / logging parity
//! with `/spec create` (FR-011).

use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use ragent_agent::{
    agent,
    event::EventBus,
    permission::PermissionChecker,
    provider,
    session::{SessionManager, processor::SessionProcessor},
    storage::Storage,
    tool,
};
use ragent_tui::App;
use ragent_tui::app::LogLevel;

// ── App construction helpers (mirrors test_slash_commands.rs) ──────────────

/// Build an [`App`] backed by an in-memory database.
fn make_app() -> App {
    let event_bus = Arc::new(EventBus::default());
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    let provider_registry = Arc::new(provider::create_default_registry());
    let tool_registry = Arc::new(tool::create_default_registry());
    let permission_checker = Arc::new(parking_lot::RwLock::new(PermissionChecker::new(vec![])));
    let session_manager = Arc::new(SessionManager::new(storage.clone(), event_bus.clone()));
    let session_processor = Arc::new(SessionProcessor {
        session_manager,
        provider_registry: provider_registry.clone(),
        tool_registry,
        permission_checker,
        event_bus: event_bus.clone(),
        agent_manager: std::sync::OnceLock::new(),
        bg_service: std::sync::OnceLock::new(),
        team_manager: std::sync::OnceLock::new(),
        mcp_client: std::sync::OnceLock::new(),
        code_index: std::sync::OnceLock::new(),
        extraction_engine: std::sync::OnceLock::new(),
        stream_config: ragent_agent::StreamConfig::default(),
        active_spec: tokio::sync::RwLock::new(None),
        spec_manager: std::sync::OnceLock::new(),
        cached_tool_definitions: parking_lot::RwLock::new(None),
        cached_tool_names: parking_lot::RwLock::new(None),
        cached_tool_definition_bytes: parking_lot::RwLock::new(None),
        llm_client_cache: parking_lot::RwLock::new(std::collections::HashMap::new()),
        cached_config: parking_lot::Mutex::new(None),
        team_context_cache: std::sync::Arc::new(parking_lot::RwLock::new(
            std::collections::HashMap::new(),
        )),
        auto_approve: false,
        system_prompt_cache: parking_lot::RwLock::new(None),
        skill_body_cache: std::sync::Arc::new(std::sync::RwLock::new(
            std::collections::HashMap::new(),
        )),
        read_timestamps: std::sync::Arc::new(std::sync::RwLock::new(
            std::collections::HashMap::new(),
        )),
        telemetry: std::sync::Arc::new(ragent_agent::telemetry::TelemetrySubsystem::disabled()),
        activity_log: std::sync::OnceLock::new(),
    });
    let agent_info =
        agent::resolve_agent("general", &Default::default()).expect("resolve general agent");

    App::new(
        event_bus,
        storage,
        provider_registry,
        session_processor,
        Arc::unwrap_or_clone(agent_info),
        false,
        std::path::PathBuf::new(),
    )
}

// ── CWD helpers ────────────────────────────────────────────────────────────

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

fn cwd_lock() -> MutexGuard<'static, ()> {
    let lock = cwd_test_lock().lock();
    match lock {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

/// Create a temp directory, switch the process cwd into it, and return a
/// `(TempDir, CwdGuard)` pair.  The `TempDir` is declared **before** the
/// `CwdGuard` so that on drop the cwd is restored *before* the directory is
/// deleted (otherwise `current_dir` fails on a ghost path).
///
/// **Caller must hold `cwd_lock()`** to serialise cwd manipulation across
/// tests — this function does not acquire the lock itself (to avoid
/// double-lock deadlocks when the caller already holds it).
fn temp_cwd() -> (tempfile::TempDir, CwdGuard) {
    let original_cwd = std::env::current_dir().expect("cwd");
    let temp = tempfile::tempdir().expect("tempdir");
    std::env::set_current_dir(temp.path()).expect("set cwd");
    (temp, CwdGuard(original_cwd))
}

/// Create `specs/<id>/SPEC.md` inside the current working directory with the
/// given content.  Returns the spec directory path.
fn create_spec_dir(id: &str, spec_md_content: &str) -> std::path::PathBuf {
    let spec_dir = std::path::Path::new("specs").join(id);
    std::fs::create_dir_all(&spec_dir).expect("create spec dir");
    std::fs::write(spec_dir.join("SPEC.md"), spec_md_content).expect("write SPEC.md");
    spec_dir
}

/// Create `specs/<id>/JTBD.md` inside the current working directory.
fn create_jtbd_file(id: &str, content: &str) {
    let jtbd = std::path::Path::new("specs").join(id).join("JTBD.md");
    std::fs::write(jtbd, content).expect("write JTBD.md");
}

const SAMPLE_SPEC_MD: &str = "\
# Specification: Sample Feature

## Requirements

### FR-001: Login
**The system shall** provide a login screen.

### NFR-001: Performance
**The system shall** respond within 200ms.
";

// ── Tests ──────────────────────────────────────────────────────────────────

/// FR-008: `/spec jtbd` with no spec ID should be treated as a usage error.
/// The dispatch for `SpecCommand::Unknown("jtbd")` sets a status string but
/// does not push a message (the help text is only shown for `/spec` with no
/// subcommand at all).
#[tokio::test(flavor = "multi_thread")]
async fn test_spec_jtbd_no_args_shows_usage() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    app.execute_slash_command("/spec jtbd");

    // Usage-error subcommand → status reports the usage hint.
    assert!(
        app.status.contains("Usage") && app.status.contains("jtbd"),
        "status should report jtbd usage: {}",
        app.status
    );
    assert!(
        app.status.contains("spec help"),
        "status should suggest /spec help: {}",
        app.status
    );
    assert!(
        !app.is_processing,
        "should not start processing with no spec ID"
    );
}

/// FR-008: an invalid spec ID (contains characters other than alnum, hyphen,
/// underscore) must be rejected with a clear error status and message.
#[tokio::test(flavor = "multi_thread")]
async fn test_spec_jtbd_invalid_spec_id_rejected() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    // `bad!id` contains `!` which is invalid per SpecId::new.
    app.execute_slash_command("/spec jtbd bad!id");

    assert!(
        app.status.contains("invalid spec ID"),
        "status should report invalid spec ID: {}",
        app.status
    );
    assert!(!app.messages.is_empty(), "should produce an error message");
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("Invalid spec ID") || text.contains("invalid"),
        "message should explain the invalid ID: {text}"
    );
    assert!(
        !app.is_processing,
        "should not start processing for an invalid spec ID"
    );
}

/// FR-008: a spec ID that is valid syntactically but refers to a non-existent
/// spec directory must be rejected with a "not found" error.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_spec_jtbd_unknown_spec_not_found() {
    let _lock = cwd_lock();
    let (_temp, _guard) = temp_cwd();

    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    // `nonexistent-spec` is a valid SpecId but no specs/ dir exists for it.
    app.execute_slash_command("/spec jtbd nonexistent-spec");

    assert!(
        app.status.contains("not found"),
        "status should report spec not found: {}",
        app.status
    );
    assert!(!app.messages.is_empty(), "should produce an error message");
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("not found") || text.contains("Error"),
        "message should explain the missing spec: {text}"
    );
    assert!(
        !app.is_processing,
        "should not start processing for a missing spec"
    );
}

/// FR-009: if `SPEC.md` exists but is empty, the command must error rather than
/// silently proceeding.
#[tokio::test(flavor = "multi_thread")]
async fn test_spec_jtbd_empty_spec_md_rejected() {
    let _lock = cwd_lock();
    let (_temp, _guard) = temp_cwd();

    create_spec_dir("emptyspec", "   \n\n  ");

    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    app.execute_slash_command("/spec jtbd emptyspec");

    assert!(
        app.status.contains("empty"),
        "status should report empty SPEC.md: {}",
        app.status
    );
    assert!(
        !app.is_processing,
        "should not start processing when SPEC.md is empty"
    );
}

/// FR-003: if `JTBD.md` already exists and `--force` is not supplied, the
/// command must refuse and tell the user to re-run with `--force`.
#[tokio::test(flavor = "multi_thread")]
async fn test_spec_jtbd_guard_refuses_existing_without_force() {
    let _lock = cwd_lock();
    let (_temp, _guard) = temp_cwd();

    create_spec_dir("myspec", SAMPLE_SPEC_MD);
    create_jtbd_file("myspec", "# Existing JTBD\nOld content");

    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    app.execute_slash_command("/spec jtbd myspec");

    // Guard status: "spec jtbd: <id> already has JTBD.md"
    assert!(
        app.status.contains("already has JTBD.md"),
        "status should indicate existing JTBD.md: {}",
        app.status
    );
    assert!(
        !app.is_processing,
        "should not start processing when JTBD.md exists without --force"
    );
    assert!(!app.messages.is_empty(), "should produce a guard message");
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("JTBD.md already exists"),
        "message should explain the guard: {text}"
    );
    assert!(
        text.contains("--force"),
        "message should suggest --force: {text}"
    );

    // The existing JTBD.md must be preserved (not overwritten).
    let preserved = std::fs::read_to_string("specs/myspec/JTBD.md").expect("read JTBD.md");
    assert!(
        preserved.contains("Existing JTBD"),
        "existing JTBD.md must be preserved: {preserved}"
    );
}

/// FR-004: `--force` bypasses the guard and starts the JTBD generation task.
#[tokio::test(flavor = "multi_thread")]
async fn test_spec_jtbd_force_bypasses_guard() {
    let _lock = cwd_lock();
    let (_temp, _guard) = temp_cwd();

    create_spec_dir("myspec", SAMPLE_SPEC_MD);
    create_jtbd_file("myspec", "# Existing JTBD\nOld content");

    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    app.execute_slash_command("/spec jtbd myspec --force");

    // FR-011: status should match build_jtbd_status pattern
    assert!(
        app.status.contains("spec jtbd") && app.status.contains("myspec"),
        "status should indicate jtbd generation for myspec: {}",
        app.status
    );
    assert!(
        app.is_processing,
        "should start processing with --force even when JTBD.md exists"
    );
}

/// FR-002 / FR-011: a valid spec without an existing JTBD.md should start the
/// generation task with status, message, and log parity matching `/spec create`.
#[tokio::test(flavor = "multi_thread")]
async fn test_spec_jtbd_starts_generation() {
    let _lock = cwd_lock();
    let (_temp, _guard) = temp_cwd();

    create_spec_dir("validspec", SAMPLE_SPEC_MD);

    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    app.execute_slash_command("/spec jtbd validspec");

    // FR-011: status parity — should be a "spec jtbd: …" status string
    assert!(
        app.status.contains("spec jtbd") && app.status.contains("validspec"),
        "status should indicate jtbd generation: {}",
        app.status
    );
    assert!(
        app.is_processing,
        "should set is_processing for a valid spec"
    );

    // FR-011: message parity — should contain build_jtbd_message content
    assert!(
        !app.messages.is_empty(),
        "should push the jtbd task message"
    );
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("validspec"),
        "message should mention the spec ID: {text}"
    );
    assert!(text.contains("JTBD"), "message should mention JTBD: {text}");

    // FR-011: log parity — an Info-level log entry referencing JTBD should exist
    let has_jtbd_log = app
        .log_entries
        .iter()
        .any(|e| e.level == LogLevel::Info && e.message.contains("JTBD"));
    assert!(
        has_jtbd_log,
        "should push an Info log entry mentioning JTBD"
    );
}

/// FR-005: `--agent <name>` with a name not in the cycleable agents list must
/// be rejected with an error and no task should be spawned.
#[tokio::test(flavor = "multi_thread")]
async fn test_spec_jtbd_unknown_agent_rejected() {
    let _lock = cwd_lock();
    let (_temp, _guard) = temp_cwd();

    create_spec_dir("validspec", SAMPLE_SPEC_MD);

    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    app.execute_slash_command("/spec jtbd validspec --agent nonexistent-agent");

    assert!(
        app.status.contains("agent") && app.status.contains("not found"),
        "status should report agent not found: {}",
        app.status
    );
    assert!(
        !app.is_processing,
        "should not start processing when agent is not found"
    );
    assert!(!app.messages.is_empty(), "should produce an error message");
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("not found") || text.contains("Error"),
        "message should explain the unknown agent: {text}"
    );
}

/// FR-005: `--agent <name>` with a valid built-in agent name should start the
/// generation task (agent override accepted).
#[tokio::test(flavor = "multi_thread")]
async fn test_spec_jtbd_valid_agent_override_starts() {
    let _lock = cwd_lock();
    let (_temp, _guard) = temp_cwd();

    create_spec_dir("validspec", SAMPLE_SPEC_MD);

    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    // `general` is always in the built-in agent roster.
    app.execute_slash_command("/spec jtbd validspec --agent general");

    assert!(
        app.status.contains("spec jtbd") && app.status.contains("validspec"),
        "status should indicate jtbd generation: {}",
        app.status
    );
    assert!(
        app.is_processing,
        "should start processing with a valid agent override"
    );
}
