//! Tests for `/codeindex skillgen` subcommand and the `skillgen` module.
//!
//! Verifies the graphify skill is written with the correct file structure
//! (SKILL.md + 8 reference files), and that existing files are overwritten
//! on re-run. Tests the inner `generate_graphify_skill_in` function directly
//! to avoid manipulating HOME (which would require `unsafe`).

use std::fs;
use std::sync::Arc;

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

/// Build an [`App`] backed by an in-memory database (mirrors test_slash_commands.rs).
fn make_app() -> App {
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    let event_bus = Arc::new(EventBus::default());
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
        last_message_finish_reason: tokio::sync::RwLock::new(std::collections::HashMap::new()),
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

/// Expected reference file names.
const EXPECTED_REFS: &[&str] = &[
    "add-watch.md",
    "exports.md",
    "extraction-spec.md",
    "github-and-merge.md",
    "hooks.md",
    "query.md",
    "transcribe.md",
    "update.md",
];

#[test]
fn test_skillgen_writes_skill_files() {
    let temp = tempfile::tempdir().expect("tempdir");
    let skills_dir = temp.path().join(".ragent").join("skills");

    let result =
        ragent_tui::app::skillgen::generate_graphify_skill_in(&skills_dir).expect("generate skill");

    assert_eq!(result.files_written, 9, "1 SKILL.md + 8 references");
    assert_eq!(
        result.dest,
        skills_dir.join("graphify"),
        "dest should be <skills_dir>/graphify"
    );
    assert!(
        !result.already_existed,
        "skill dir should not have existed before"
    );

    // SKILL.md must exist and have the graphify frontmatter.
    let skill_md = result.dest.join("SKILL.md");
    assert!(skill_md.is_file(), "SKILL.md should exist");
    let content = fs::read_to_string(&skill_md).expect("read SKILL.md");
    assert!(
        content.starts_with("---\nname: graphify"),
        "SKILL.md should start with graphify frontmatter, got: {}",
        &content[..50]
    );

    // All 8 reference files must exist and be non-empty.
    let refs_dir = result.dest.join("references");
    assert!(refs_dir.is_dir(), "references/ dir should exist");
    for name in EXPECTED_REFS {
        let path = refs_dir.join(name);
        assert!(path.is_file(), "reference file {name} should exist");
        let content =
            fs::read_to_string(&path).unwrap_or_else(|e| panic!("read reference {name}: {e}"));
        assert!(!content.is_empty(), "reference {name} should not be empty");
    }
}

#[test]
fn test_skillgen_overwrites_existing() {
    let temp = tempfile::tempdir().expect("tempdir");
    let skills_dir = temp.path().join(".ragent").join("skills");

    // Run once.
    let result1 =
        ragent_tui::app::skillgen::generate_graphify_skill_in(&skills_dir).expect("first run");
    assert!(!result1.already_existed);

    let skill_md = result1.dest.join("SKILL.md");
    let first_content = fs::read_to_string(&skill_md).expect("read first SKILL.md");

    // Run again — should report already_existed.
    let result2 =
        ragent_tui::app::skillgen::generate_graphify_skill_in(&skills_dir).expect("second run");
    assert!(
        result2.already_existed,
        "second run should report already_existed"
    );

    let second_content = fs::read_to_string(&skill_md).expect("read second SKILL.md");

    // Content should be identical (idempotent overwrite).
    assert_eq!(
        first_content, second_content,
        "second run should overwrite with same content"
    );
    assert_eq!(
        result2.files_written, 9,
        "second run should also write 9 files"
    );
}

#[test]
fn test_codeindex_skillgen_help_lists_subcommand() {
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());

    app.execute_slash_command("/codeindex help");

    let text = app.messages.last().expect("help message").text_content();
    assert!(
        text.contains("skillgen"),
        "help should mention skillgen, got: {text}"
    );
}

#[test]
fn test_codeindex_skillgen_unknown_fallback_lists_subcommand() {
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());

    // An unknown subcommand should show the usage string which now lists skillgen.
    app.execute_slash_command("/codeindex unknownxyz");

    let text = app.messages.last().expect("usage message").text_content();
    assert!(
        text.contains("skillgen"),
        "usage fallback should mention skillgen, got: {text}"
    );
}
