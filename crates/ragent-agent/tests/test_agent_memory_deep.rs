//! Deep coverage for the remaining moderate-size gaps across
//! `agent/mod.rs`, `session/history.rs`, `memory/knowledge_graph.rs`,
//! `memory/extract.rs`, `reference/resolve.rs`, and `trigger/dynamic.rs`.
//!
//! Tests only - no production code is modified. Everything that talks to
//! storage uses a real in-memory `ragent_storage::Storage`; the one HTTP path
//! (`reference::resolve::resolve_ref` for `@http://...`) is driven against an
//! in-process loopback `TcpListener`, never the public network. No real LLM
//! provider is contacted (stub providers refuse client creation).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use ragent_agent::agent::{
    AgentInfo, AgentMode, InstructionFileDiscovery, ModelRef, PromptContextComponent,
    apply_fallback_thinking, build_memory_prompt_section,
    build_system_prompt_with_storage_and_memory_and_config, default_permissions,
    fallback_thinking_for_model_ref, resolve_agent_with_customs_and_model,
    resolve_agent_with_model, resolve_default_model, skills_prompt_section,
};
use ragent_agent::llm::LlmClient;
use ragent_agent::memory::extract::{
    ExtractionEngine, SessionMessageSummary, ToolCallSummary, decay_confidence,
};
use ragent_agent::memory::knowledge_graph::{
    EntityType, RelationType, extract_entities, get_knowledge_graph, store_extraction,
};
use ragent_agent::message::{Message, MessagePart, Role, ToolCallState, ToolCallStatus};
use ragent_agent::provider::{ModelInfo, Provider, ProviderRegistry};
use ragent_agent::reference::parse::parse_refs;
use ragent_agent::reference::resolve::{resolve_all_refs, resolve_ref};
use ragent_agent::session::history::{
    detect_incomplete_file_task, history_to_chat_messages, is_permanent_llm_api_error,
    should_retry_stream_error, stream_has_meaningful_partial_output, tool_result_content_for_llm,
};
use ragent_agent::skill::{SkillInfo, SkillRegistry};
use ragent_agent::storage::Storage;
use ragent_agent::trigger::dynamic::{
    DynamicTriggerEngine, DynamicTriggerError, NoopActionDispatcher, SimpleConditionEvaluator,
    parse_trigger_request,
};
use ragent_agent::trigger::runtime::{TriggerRuntime, TriggerRuntimeConfig};
use ragent_config::{Capabilities, Config, Cost, ProviderConfig};
use ragent_types::{ThinkingConfig, ThinkingLevel};
use serde_json::json;

// ---------------------------------------------------------------------------
// Shared stubs / fixtures
// ---------------------------------------------------------------------------

/// A provider advertising one model, for offline model/thinking resolution.
struct StubProvider {
    id: &'static str,
    thinking: Option<ThinkingConfig>,
}

impl StubProvider {
    fn model(&self) -> ModelInfo {
        ModelInfo {
            id: "stub-model".to_string(),
            provider_id: self.id.to_string(),
            name: "Stub Model".to_string(),
            cost: Cost {
                input: 0.0,
                output: 0.0,
            },
            capabilities: Capabilities {
                reasoning: true,
                streaming: true,
                vision: false,
                tool_use: true,
                thinking_levels: vec![ThinkingLevel::Low],
            },
            context_window: 8_000,
            max_output: None,
            request_multiplier: None,
            thinking_config: self.thinking.clone(),
        }
    }
}

#[async_trait::async_trait]
impl Provider for StubProvider {
    fn id(&self) -> &'static str {
        self.id
    }
    fn name(&self) -> &'static str {
        "Stub"
    }
    fn default_models(&self) -> Vec<ModelInfo> {
        vec![self.model()]
    }
    async fn create_client(
        &self,
        _api_key: &str,
        _base_url: Option<&str>,
        _options: &HashMap<String, serde_json::Value>,
    ) -> anyhow::Result<Box<dyn LlmClient>> {
        anyhow::bail!("stub provider cannot create a client")
    }
    fn as_any_static(&self) -> &(dyn std::any::Any + 'static) {
        self
    }
}

fn stub_registry(id: &'static str, thinking: Option<ThinkingConfig>) -> ProviderRegistry {
    let mut registry = ProviderRegistry::new();
    registry.register(Box::new(StubProvider { id, thinking }));
    registry
}

// ---------------------------------------------------------------------------
// memory/knowledge_graph.rs
// ---------------------------------------------------------------------------

#[test]
fn kg_store_extraction_and_roundtrip_through_storage() {
    let storage = Storage::open_in_memory().expect("storage");
    let result = extract_entities("Rust uses Tokio and Docker", "workflow", &[]);
    assert!(result.entities.len() >= 2);

    let (entities, relationships) = store_extraction(&result, 42, &storage).expect("store");
    assert_eq!(entities, result.entities.len());
    assert!(relationships >= 1, "pairwise relationships expected");

    // Re-storing the same extraction increments mention counts rather than
    // creating duplicate entity rows (upsert path).
    let (entities2, _) = store_extraction(&result, 43, &storage).expect("store again");
    assert_eq!(entities2, result.entities.len());

    let graph = get_knowledge_graph(&storage).expect("read graph");
    assert_eq!(graph.entities.len(), result.entities.len());
    assert!(!graph.relationships.is_empty());
    // `From<KgEntityRow>` / `From<KgRelationshipRow>` mapping is exercised here.
    assert!(graph.entities.iter().any(|e| e.name == "Rust"));
    assert!(graph.entities.iter().any(|e| e.mention_count >= 2));
    let rel = &graph.relationships[0];
    assert!(rel.confidence > 0.0);
    assert_eq!(rel.source_memory_id, Some(42));
}

#[test]
fn kg_store_extraction_skips_relationships_with_unknown_endpoints() {
    // Build a result whose relationship references an entity that was not
    // extracted -> the `(Some, Some)` filter must skip it.
    let storage = Storage::open_in_memory().expect("storage");
    let result = ragent_agent::memory::knowledge_graph::ExtractionResult {
        entities: vec![ragent_agent::memory::knowledge_graph::ExtractedEntity {
            name: "Rust".to_string(),
            entity_type: EntityType::Language,
        }],
        relationships: vec![
            ragent_agent::memory::knowledge_graph::ExtractedRelationship {
                source: "Rust".to_string(),
                target: "Ghost".to_string(),
                relation_type: RelationType::Uses,
            },
        ],
    };
    let (entities, relationships) = store_extraction(&result, 1, &storage).expect("store");
    assert_eq!(entities, 1);
    assert_eq!(relationships, 0, "dangling endpoint must be skipped");
}

#[test]
fn kg_determine_relation_tag_and_type_branches() {
    // Tag-driven relation types.
    let uses = extract_entities("Rust Python", "fact", &["uses".to_string()]);
    assert_eq!(uses.relationships[0].relation_type, RelationType::Uses);

    let depends = extract_entities("Rust Python", "fact", &["dependency".to_string()]);
    assert_eq!(
        depends.relationships[0].relation_type,
        RelationType::DependsOn
    );

    let prefers = extract_entities("Rust Python", "fact", &["preferred".to_string()]);
    assert_eq!(
        prefers.relationships[0].relation_type,
        RelationType::Prefers
    );

    let avoids = extract_entities("Rust Python", "fact", &["avoid".to_string()]);
    assert_eq!(avoids.relationships[0].relation_type, RelationType::Avoids);

    // Language -> Tool yields Uses (languages are collected before tools).
    let typed = extract_entities("Rust Tokio", "fact", &[]);
    assert!(
        typed.relationships.iter().any(|r| r.source == "Rust"
            && r.target == "Tokio"
            && r.relation_type == RelationType::Uses),
        "Rust->Tokio should be Uses: {:?}",
        typed.relationships
    );

    // workflow category default -> Uses (two language entities, no tag/type rule).
    let workflow = extract_entities("Go Java", "workflow", &[]);
    assert_eq!(workflow.relationships[0].relation_type, RelationType::Uses);

    // related_to default for an unclassified category.
    let other = extract_entities("Go Java", "insight", &[]);
    assert_eq!(
        other.relationships[0].relation_type,
        RelationType::RelatedTo
    );

    // A single entity yields no relationships (len < 2 guard). "Kotlin" avoids
    // the single-letter language names ("C", "R") that would add a second hit.
    let single = extract_entities("Kotlin", "fact", &[]);
    assert_eq!(single.entities.len(), 1);
    assert!(single.relationships.is_empty());
}

#[test]
fn kg_pattern_entity_guards_and_display() {
    // The previous word is too short ("an pattern") -> skipped (len > 2 guard).
    let short_prev = extract_entities("a pattern", "fact", &[]);
    assert!(
        !short_prev
            .entities
            .iter()
            .any(|e| e.entity_type == EntityType::Pattern),
        "short preceding word must not yield a pattern entity"
    );

    // A real pattern still extracts the "<prev> <keyword>" name.
    let ok = extract_entities("TDD methodology", "fact", &[]);
    assert!(
        ok.entities
            .iter()
            .any(|e| e.name == "TDD methodology" && e.entity_type == EntityType::Pattern)
    );

    // Display / as_str round-trips for both enums.
    for t in [
        EntityType::Project,
        EntityType::Tool,
        EntityType::Language,
        EntityType::Pattern,
        EntityType::Person,
        EntityType::Concept,
    ] {
        assert_eq!(t.to_string(), t.as_str());
    }
    assert_eq!(EntityType::from_str("nope"), None);
    for r in [
        RelationType::Uses,
        RelationType::Prefers,
        RelationType::DependsOn,
        RelationType::Avoids,
        RelationType::RelatedTo,
    ] {
        assert_eq!(r.to_string(), r.as_str());
    }
    // Unknown relation strings fall back to RelatedTo.
    assert_eq!(
        RelationType::from_str_lossy("totally-unknown"),
        RelationType::RelatedTo
    );
}

// ---------------------------------------------------------------------------
// memory/extract.rs
// ---------------------------------------------------------------------------

fn extract_fixture(
    enabled: bool,
    require_confirmation: bool,
) -> (
    ExtractionEngine,
    Arc<Storage>,
    Arc<ragent_agent::event::EventBus>,
    tempfile::TempDir,
) {
    let config = ragent_config::AutoExtractConfig {
        enabled,
        require_confirmation,
    };
    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    let bus = Arc::new(ragent_agent::event::EventBus::new(256));
    let dir = tempfile::tempdir().expect("tempdir");
    (ExtractionEngine::new(config), storage, bus, dir)
}

fn candidate_count(rx: &mut tokio::sync::broadcast::Receiver<ragent_agent::event::Event>) -> usize {
    let mut n = 0;
    while let Ok(event) = rx.try_recv() {
        if let ragent_agent::event::Event::MemoryCandidateExtracted { .. } = event {
            n += 1;
        }
    }
    n
}

#[test]
fn extract_content_pattern_branches_for_every_detector() {
    let (engine, storage, bus, dir) = extract_fixture(true, true);
    let mut rx = bus.subscribe();

    // thiserror + tracing + serde + #[test] all in one edit.
    let file = dir.path().join("src").join("lib.rs");
    engine.on_tool_result(
        "write",
        &json!({
            "path": file.to_string_lossy(),
            "content": "thiserror::Error\ntracing::info!(\"x\")\n#[derive(Debug)] struct S; let _: Serialize;\n#[test] fn t() {}"
        }),
        "ok",
        true,
        "s1",
        &storage,
        &bus,
        dir.path(),
    );

    let n = candidate_count(&mut rx);
    assert_eq!(n, 1, "a single Rust edit should yield one candidate");
}

#[test]
fn extract_python_typescript_and_markdown_tags() {
    let (engine, storage, bus, dir) = extract_fixture(true, true);
    let mut rx = bus.subscribe();

    // Python file: tag `python`; no content_parts -> generic parent fallback.
    engine.on_tool_result(
        "write",
        &json!({ "path": dir.path().join("pkg/app.py").to_string_lossy() }),
        "ok",
        true,
        "s1",
        &storage,
        &bus,
        dir.path(),
    );
    // TypeScript file: tag `typescript`.
    engine.on_tool_result(
        "write",
        &json!({ "path": dir.path().join("web/app.tsx").to_string_lossy() }),
        "ok",
        true,
        "s1",
        &storage,
        &bus,
        dir.path(),
    );
    // Markdown doc: tag `documentation`, content_parts empty -> fallback.
    engine.on_tool_result(
        "write",
        &json!({ "path": dir.path().join("docs/guide.md").to_string_lossy() }),
        "ok",
        true,
        "s1",
        &storage,
        &bus,
        dir.path(),
    );

    // All three should be emitted (distinct content each).
    assert_eq!(candidate_count(&mut rx), 3);
}

#[test]
fn extract_root_level_file_without_content_yields_no_candidate() {
    let (engine, storage, bus, dir) = extract_fixture(true, true);
    let mut rx = bus.subscribe();

    // A bare file in the working dir root: parent is "." -> returns None.
    engine.on_tool_result(
        "write",
        &json!({ "path": dir.path().join("notes.md").to_string_lossy() }),
        "ok",
        true,
        "s1",
        &storage,
        &bus,
        dir.path(),
    );
    assert_eq!(candidate_count(&mut rx), 0);
}

#[test]
fn extract_edit_aliases_multi_edit_and_create() {
    let (engine, storage, bus, dir) = extract_fixture(true, true);
    let mut rx = bus.subscribe();

    // `multi_edit` alias with a `file_text` payload exercises the third
    // content-field branch.
    engine.on_tool_result(
        "multi_edit",
        &json!({
            "path": dir.path().join("src/deep/mod.rs").to_string_lossy(),
            "file_text": "anyhow::Result<()>"
        }),
        "ok",
        true,
        "s1",
        &storage,
        &bus,
        dir.path(),
    );
    assert_eq!(candidate_count(&mut rx), 1);
}

#[test]
fn extract_track_failure_caps_at_twenty_and_resolves() {
    let (engine, storage, bus, dir) = extract_fixture(true, true);
    let mut rx = bus.subscribe();

    // 21 failures -> the tracker's 20-entry cap must drop the oldest.
    for i in 0..21 {
        engine.on_tool_result(
            "bash",
            &json!({ "command": format!("make {i}") }),
            "boom",
            false,
            "cap-session",
            &storage,
            &bus,
            dir.path(),
        );
    }
    // A later success resolves against the most recent failure.
    engine.on_tool_result(
        "bash",
        &json!({ "command": "make 21" }),
        "fine",
        true,
        "cap-session",
        &storage,
        &bus,
        dir.path(),
    );

    assert_eq!(candidate_count(&mut rx), 1);
}

#[test]
fn extract_dedup_against_stored_memory_suppresses_candidate() {
    let (engine, storage, bus, dir) = extract_fixture(true, true);
    let mut rx = bus.subscribe();

    // Seed a memory whose content is exactly the candidate an edit would
    // produce -> the FTS/word-overlap dedup path returns true.
    let rel = "settings.toml";
    storage
        .create_memory(
            &format!("Configuration file: {rel}"),
            "pattern",
            "seed",
            0.5,
            "",
            "s0",
            &[],
        )
        .expect("seed");

    engine.on_tool_result(
        "write",
        &json!({ "path": dir.path().join(rel).to_string_lossy() }),
        "ok",
        true,
        "s1",
        &storage,
        &bus,
        dir.path(),
    );
    assert_eq!(
        candidate_count(&mut rx),
        0,
        "candidate duplicating a stored memory must be suppressed"
    );
}

#[test]
fn extract_unquoted_failure_command_and_duplicate_resolution() {
    let (engine, storage, bus, dir) = extract_fixture(true, true);
    let mut rx = bus.subscribe();

    // A failure whose recorded input is a plain string (no surrounding
    // quotes) exercises the `unwrap_or` branch in detect_error_resolution.
    engine.on_tool_result(
        "bash",
        &json!("cargo build"),
        "boom",
        false,
        "s1",
        &storage,
        &bus,
        dir.path(),
    );
    engine.on_tool_result(
        "bash",
        &json!("cargo build"),
        "ok",
        true,
        "s1",
        &storage,
        &bus,
        dir.path(),
    );
    assert_eq!(candidate_count(&mut rx), 1);
}

#[test]
fn extract_auto_store_reads_setting_and_creates_memory() {
    let (engine, storage, bus, dir) = extract_fixture(true, false);
    let mut rx = bus.subscribe();

    engine.on_tool_result(
        "edit",
        &json!({
            "path": dir.path().join("tests/a.rs").to_string_lossy(),
            "new_str": "anyhow::Result<()>"
        }),
        "ok",
        true,
        "s1",
        &storage,
        &bus,
        dir.path(),
    );

    let memories = storage.list_memories("", 100).expect("list");
    assert_eq!(memories.len(), 1);
    let mut stored = false;
    while let Ok(event) = rx.try_recv() {
        if matches!(event, ragent_agent::event::Event::MemoryStored { .. }) {
            stored = true;
        }
    }
    assert!(stored);
}

#[test]
fn extract_session_summary_with_legacy_path_key() {
    let (engine, storage, bus, dir) = extract_fixture(true, true);
    let mut rx = bus.subscribe();

    // Use the legacy `path` key (not `file_path`) so the `.or_else` fallback
    // in extract_session_summary runs.
    let edited = dir.path().join("src").join("legacy.rs");
    let messages = vec![SessionMessageSummary {
        role: "assistant".to_string(),
        tool_calls: vec![ToolCallSummary {
            tool_name: "write".to_string(),
            input: json!({ "path": edited.to_string_lossy() }),
            success: true,
        }],
    }];
    engine.on_session_end("s1", &messages, &storage, &bus, dir.path());
    assert_eq!(candidate_count(&mut rx), 1);
}

#[test]
fn extract_decay_applies_to_aged_memory() {
    let storage = Storage::open_in_memory().expect("storage");
    storage
        .create_memory("aged fact", "fact", "test", 0.9, "", "s1", &[])
        .expect("create");

    // Age the memory by rewinding its timestamps via the raw connection.
    {
        let conn = storage.conn_lock_for_test().expect("conn");
        conn.execute(
            "UPDATE memories SET last_accessed = '2020-01-01T00:00:00Z', updated_at = '2020-01-01T00:00:00Z'",
            (),
        )
        .expect("age");
    }

    let updated = decay_confidence(&storage, 0.5, 0.1);
    assert_eq!(updated, 1, "an aged memory must decay");
    let memories = storage.list_memories("", 100).expect("list");
    assert!(memories[0].confidence < 0.9, "confidence should drop");

    // Decaying again immediately is a no-op only if the date did not move;
    // here the update path already ran once, so a second pass with an
    // unchanged confidence floor still runs the comparison branch.
    let _ = decay_confidence(&storage, 0.99, 0.1);
}

// ---------------------------------------------------------------------------
// session/history.rs
// ---------------------------------------------------------------------------

#[test]
fn history_is_permanent_llm_api_error_status_code_branches() {
    // 4xx (non-408/429) is permanent.
    assert!(is_permanent_llm_api_error(
        "HTTP status code 404: not found"
    ));
    assert!(is_permanent_llm_api_error("HTTP 401 unauthorized"));
    assert!(is_permanent_llm_api_error("API error (422): unprocessable"));
    // 408 / 429 are retryable, not permanent.
    assert!(!is_permanent_llm_api_error("HTTP 429 too many requests"));
    assert!(!is_permanent_llm_api_error("request timed out 408"));
    // 5xx is not permanent.
    assert!(!is_permanent_llm_api_error("HTTP 500 internal"));
    // Named permanent diagnostics.
    assert!(is_permanent_llm_api_error("empty/malformed event stream"));
    assert!(is_permanent_llm_api_error("no models loaded"));
    assert!(is_permanent_llm_api_error("please load the model"));
}

#[test]
fn history_should_retry_stream_error_various_messages() {
    // Retryable transient messages before any output.
    for msg in [
        "connection reset by peer",
        "broken pipe",
        "unexpected eof",
        "incomplete message",
        "h2 protocol error",
        "http2 error",
        "stream ended unexpectedly",
    ] {
        assert!(
            should_retry_stream_error(msg, 0, 4, false),
            "expected retry for {msg}"
        );
    }
    // Non-retryable diagnostics.
    for msg in [
        "empty/malformed event stream",
        "model is not loaded",
        "no models loaded",
        "totally benign message",
    ] {
        assert!(
            !should_retry_stream_error(msg, 0, 4, false),
            "expected no retry for {msg}"
        );
    }
    // Early decode failure with no output is treated as fatal.
    assert!(!should_retry_stream_error(
        "error decoding response body",
        0,
        4,
        false
    ));
    // After meaningful output the caller must not retry (would lose output).
    assert!(!should_retry_stream_error(
        "error decoding response body",
        0,
        4,
        true
    ));
    // Retry budget exhausted -> no retry.
    assert!(!should_retry_stream_error("connection reset", 4, 4, false));
}

#[test]
fn history_stream_has_meaningful_partial_output_branches() {
    assert!(stream_has_meaningful_partial_output("hello", "", false));
    assert!(stream_has_meaningful_partial_output("", "reasoning", false));
    assert!(stream_has_meaningful_partial_output("", "", true));
    assert!(!stream_has_meaningful_partial_output("   ", "\n", false));
}

#[tokio::test]
async fn history_tool_result_marker_and_error_fallbacks() {
    let with_error = Message::new(
        "s1",
        Role::Assistant,
        vec![MessagePart::ToolCall {
            tool: "bash".to_string(),
            call_id: "c1".to_string(),
            state: Box::new(ToolCallState {
                status: ToolCallStatus::Error,
                input: json!({ "command": "false" }),
                output: None,
                error: Some("boom".to_string()),
                duration_ms: None,
            }),
        }],
    );
    let no_output = Message::new(
        "s1",
        Role::Assistant,
        vec![MessagePart::ToolCall {
            tool: "read".to_string(),
            call_id: "c2".to_string(),
            state: Box::new(ToolCallState {
                status: ToolCallStatus::Completed,
                input: json!({ "path": "x" }),
                output: None,
                error: None,
                duration_ms: None,
            }),
        }],
    );

    let chat = history_to_chat_messages(&[with_error, no_output]).await;
    let joined: String = chat
        .iter()
        .map(|m| format!("{:?}", m.content))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(joined.contains("boom"), "error text must surface: {joined}");
    assert!(
        joined.contains("(no output)"),
        "missing tool result must emit the marker: {joined}"
    );
}

#[tokio::test]
async fn history_sync_path_and_compaction_role() {
    let user = Message::new(
        "s1",
        Role::User,
        vec![MessagePart::Text { text: "hi".into() }],
    );
    let compaction = Message::new(
        "s1",
        Role::Compaction,
        vec![MessagePart::Text {
            text: "summary".into(),
        }],
    );
    let chat = history_to_chat_messages(&[user, compaction]).await;
    assert_eq!(chat.len(), 2);
    assert_eq!(chat[0].role, "user");
    assert_eq!(chat[1].role, "assistant");
}

#[test]
fn history_tool_result_metadata_line_key_variants() {
    let big = "x".repeat(20_000);
    for meta in [
        json!({ "line_count": 7u64 }),
        json!({ "lines": 9u64 }),
        json!({ "other": 1 }),
    ] {
        let out = tool_result_content_for_llm("read", &big, Some(&meta));
        assert!(out.contains("tool result truncated for context"));
    }
}

#[test]
fn history_tool_result_truncation_is_multibyte_safe() {
    // Multibyte content beyond the head budget must truncate on a char
    // boundary, never mid-codepoint.
    let big: String = "é".repeat(30_000);
    let out = tool_result_content_for_llm("read", &big, None);
    assert!(out.contains("tool result truncated for context"));
    // Round-tripping through `String` proves no invalid UTF-8 boundary.
    let _ = out.chars().count();
}

#[test]
fn history_exempt_tool_without_results_metadata_uses_head_tail() {
    // A wait_agents payload over the inline budget with metadata that lacks a
    // `results` array falls back to head+tail.
    let big = "y".repeat(40_000);
    let out = tool_result_content_for_llm("wait_agents", &big, Some(&json!({ "foo": 1 })));
    assert!(out.contains("middle omitted"), "fallback marker expected");
}

#[test]
fn history_detect_incomplete_file_task_more_verbs() {
    assert!(detect_incomplete_file_task("save output.csv please", &[]));
    assert!(detect_incomplete_file_task("produce a summary.pdf", &[]));
    assert!(detect_incomplete_file_task("write the README.md", &[]));
    // A trailing dot with no extension is not a filename.
    assert!(!detect_incomplete_file_task("create a file.", &[]));
    // A hyphenated extension-less token is not a filename.
    assert!(!detect_incomplete_file_task("make it work", &[]));
}

// ---------------------------------------------------------------------------
// reference/resolve.rs
// ---------------------------------------------------------------------------

/// Serve a single canned HTTP response on a loopback port, returning the URL.
fn serve_once(response: &'static str) -> String {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 2048];
            let _ = stream.read(&mut buf);
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.flush();
        }
    });
    format!("http://{addr}/")
}

#[tokio::test]
async fn resolve_url_plain_text_success() {
    let url = serve_once(
        "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 5\r\nConnection: close\r\n\r\nhello",
    );
    let parsed = parse_refs(&format!("@{url}"));
    assert!(matches!(
        parsed[0].kind,
        ragent_agent::reference::parse::FileRef::Url(_)
    ));
    let resolved = resolve_ref(&parsed[0], std::path::Path::new("."))
        .await
        .expect("resolve url");
    assert!(resolved.content.contains("hello"));
    assert!(resolved.resolved_path.is_none());
}

#[tokio::test]
async fn resolve_url_html_is_converted() {
    let body = "<html><body><p>Hello <b>World</b></p></body></html>";
    let response: &'static str = Box::leak(
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .into_boxed_str(),
    );
    let url = serve_once(response);
    let parsed = parse_refs(&format!("@{url}"));
    let resolved = resolve_ref(&parsed[0], std::path::Path::new("."))
        .await
        .expect("resolve html url");
    assert!(
        resolved.content.contains("Hello") && resolved.content.contains("World"),
        "html text should survive conversion: {}",
        resolved.content
    );
}

#[tokio::test]
async fn resolve_url_http_error_bails() {
    let url = serve_once(
        "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
    );
    let parsed = parse_refs(&format!("@{url}"));
    let err = resolve_ref(&parsed[0], std::path::Path::new("."))
        .await
        .expect_err("500 must error");
    assert!(err.to_string().contains("HTTP 500"), "err: {err}");
}

#[tokio::test]
async fn resolve_all_refs_url_tag_and_fuzzy_path_tag() {
    let dir = tempfile::tempdir().expect("tempdir");

    // Fuzzy resolution to an existing project file -> `<file path=...>` tag
    // via the `resolved_path` Some branch.
    std::fs::create_dir_all(dir.path().join("src")).expect("mkdir");
    std::fs::write(dir.path().join("src/unique_widget.rs"), "content\n").expect("write");

    let (out, refs) = resolve_all_refs("@unique_widget", dir.path())
        .await
        .expect("fuzzy resolve");
    assert_eq!(refs.len(), 1);
    assert!(out.contains("<referenced_files>"));
    assert!(out.contains("<file path=\""), "out: {out}");
}

#[tokio::test]
async fn resolve_directory_skips_generated_dirs_and_lists_files() {
    let dir = tempfile::tempdir().expect("tempdir");
    let tree = dir.path().join("proj");
    std::fs::create_dir_all(tree.join("node_modules")).expect("mkdir");
    std::fs::create_dir_all(tree.join("target")).expect("mkdir");
    std::fs::create_dir_all(tree.join(".hidden")).expect("mkdir");
    std::fs::write(tree.join("node_modules/pkg.js"), "x").expect("write");
    std::fs::write(tree.join("main.rs"), "fn main() {}").expect("write");

    let parsed = parse_refs(&format!("@{}/proj/", dir.path().display()));
    let resolved = resolve_ref(&parsed[0], dir.path()).await.expect("resolve");
    assert!(resolved.content.contains("main.rs"));
    assert!(
        resolved.content.contains("skipped"),
        "generated dirs must be marked skipped: {}",
        resolved.content
    );
    assert!(
        !resolved.content.contains(".hidden"),
        "hidden dirs are filtered: {}",
        resolved.content
    );
}

// ---------------------------------------------------------------------------
// trigger/dynamic.rs
// ---------------------------------------------------------------------------

fn trigger_engine(
    evaluator: Arc<SimpleConditionEvaluator>,
    dispatcher: Arc<NoopActionDispatcher>,
) -> DynamicTriggerEngine {
    let runtime = TriggerRuntime::new(TriggerRuntimeConfig {
        dedup_window: Duration::from_secs(0),
        max_cycles: 100,
    });
    DynamicTriggerEngine::new(
        runtime,
        ragent_config::TriggerConfig::default(),
        evaluator,
        dispatcher,
    )
}

/// Dispatcher that always fails, to exercise the dispatch-error branch.
struct FailingDispatcher;

#[async_trait::async_trait]
impl ragent_agent::trigger::dynamic::ActionDispatcher for FailingDispatcher {
    async fn dispatch(&self, _action_prompt: &str, _promote_to_chat: bool) -> anyhow::Result<()> {
        anyhow::bail!("dispatch refused")
    }
}

#[test]
fn dynamic_parse_more_delimiters_and_empty_sides() {
    // `if ... then ...`
    let p = parse_trigger_request("if tests pass then deploy").expect("parse");
    assert_eq!(p.condition, "tests pass");
    assert_eq!(p.action, "deploy");

    // Empty action after a comma is a parse failure.
    assert!(matches!(
        parse_trigger_request("when x,   "),
        Err(DynamicTriggerError::ParseFailed(_))
    ));
    // Empty condition after an arrow.
    assert!(matches!(
        parse_trigger_request(" -> action"),
        Err(DynamicTriggerError::ParseFailed(_))
    ));
}

#[test]
fn dynamic_error_display_strings() {
    assert_eq!(
        format!("{}", DynamicTriggerError::Disabled),
        "trigger system is disabled"
    );
    assert_eq!(
        format!("{}", DynamicTriggerError::MaxRulesReached { max: 3 }),
        "maximum rules (3) reached, cannot add more"
    );
    assert!(format!("{}", DynamicTriggerError::ParseFailed("bad".into())).contains("bad"));
}

#[test]
fn dynamic_restore_rules_rejects_invalid_json() {
    let evaluator = Arc::new(SimpleConditionEvaluator::new());
    let dispatcher = Arc::new(NoopActionDispatcher::new());
    let engine = trigger_engine(evaluator, dispatcher);
    assert!(engine.restore_rules("not json").is_err());
}

#[tokio::test]
async fn dynamic_poll_dispatch_failure_is_not_counted() {
    let evaluator = Arc::new(SimpleConditionEvaluator::new());
    evaluator.add_matching("file exists");
    let runtime = TriggerRuntime::new(TriggerRuntimeConfig {
        dedup_window: Duration::from_secs(0),
        max_cycles: 100,
    });
    let engine = DynamicTriggerEngine::new(
        runtime,
        ragent_config::TriggerConfig::default(),
        evaluator,
        Arc::new(FailingDispatcher),
    );
    engine
        .create_rule("when file exists, run tests", false, false)
        .expect("create");

    // The dispatcher fails, so the fired count stays 0.
    assert_eq!(engine.poll_once().await, 0);
}

#[test]
fn dynamic_accessors_and_evaluator_helpers() {
    let evaluator = Arc::new(SimpleConditionEvaluator::new());
    evaluator.add_matching("cond-a");
    assert!(evaluator.matches("cond-a"));
    assert!(!evaluator.matches("cond-b"));

    let dispatcher = Arc::new(NoopActionDispatcher::new());
    let engine = trigger_engine(evaluator, dispatcher.clone());
    assert!(engine.is_enabled());
    assert_eq!(engine.poll_interval(), Duration::from_secs(30));
    assert!(engine.runtime_config().max_cycles > 0);
    assert_eq!(
        engine.config().max_rules,
        engine.runtime().rule_count().max(engine.config().max_rules)
    );

    let id = engine
        .create_rule_from_parts("cond", "act", true, true)
        .expect("from parts");
    assert!(engine.rule_status(id.as_str()).is_some());
    assert_eq!(engine.rule_count(), 1);
    assert!(engine.remove_rule(id.as_str()));
    assert!(!engine.remove_rule(id.as_str()));
}

// ---------------------------------------------------------------------------
// agent/mod.rs
// ---------------------------------------------------------------------------

#[test]
fn agent_skills_prompt_section_branches() {
    // Empty agent skill list + a catalog with an agent-invocable skill -> the
    // "show all agent-invocable" branch.
    let mut registry = SkillRegistry::new();
    registry.register(SkillInfo::new("foo", "foo body"));
    let agent = AgentInfo::new("general", "g");
    let section = skills_prompt_section(&registry, &agent);
    assert!(section.contains("## Available Skills"));
    assert!(section.contains("/foo"));

    // Agent with a matching skill name -> filtered branch.
    let mut agent2 = AgentInfo::new("general", "g");
    agent2.skills = vec!["foo".to_string()];
    assert!(skills_prompt_section(&registry, &agent2).contains("/foo"));

    // Agent with a non-matching skill name -> empty (is_empty branch).
    let mut agent3 = AgentInfo::new("general", "g");
    agent3.skills = vec!["missing".to_string()];
    assert_eq!(skills_prompt_section(&registry, &agent3), "");

    // An empty catalog with no agent skills -> empty.
    let empty_registry = SkillRegistry::new();
    assert_eq!(skills_prompt_section(&empty_registry, &agent), "");
}

#[test]
fn agent_instruction_discovery_format_summary_branches() {
    let none = InstructionFileDiscovery {
        searched_names: vec!["AGENTS.md".to_string()],
        working_dir: PathBuf::from("/proj"),
        global_dir: None,
        used_global_fallback: false,
        all_discovered_files: vec![],
        loaded_file: None,
    };
    let summary = none.format_summary();
    assert!(summary.contains("No instruction files found"));

    let loaded = PathBuf::from("/proj/AGENTS.md");
    let global = PathBuf::from("/home/u/.local/share/ragent");
    let discovered = InstructionFileDiscovery {
        searched_names: vec!["AGENTS.md".to_string(), "CLAUDE.md".to_string()],
        working_dir: PathBuf::from("/proj"),
        global_dir: Some(global.clone()),
        used_global_fallback: true,
        all_discovered_files: vec![loaded.clone(), global.join("AGENTS.md")],
        loaded_file: Some(loaded),
    };
    let s = discovered.format_summary();
    assert!(s.contains("Discovered 2 file(s)"));
    assert!(s.contains("[loaded]"));
    assert!(s.contains("global fallback"));
    assert!(s.contains("Additional instruction files were discovered"));
}

#[test]
fn agent_default_permissions_and_agentinfo_default() {
    let perms = default_permissions();
    assert!(!perms.is_empty());
    // The default ruleset allows reads and asks for edits.
    assert!(perms.iter().any(|r| {
        r.permission == ragent_agent::permission::Permission::Edit
            && r.action == ragent_agent::permission::PermissionAction::Ask
    }));

    let d = AgentInfo::default();
    assert_eq!(d.name, "");
    assert_eq!(d.mode, AgentMode::Primary);

    // AgentMode Display + serde round-trip.
    assert_eq!(AgentMode::Primary.to_string(), "primary");
    assert_eq!(AgentMode::Subagent.to_string(), "subagent");
    assert_eq!(AgentMode::All.to_string(), "all");
    let json = serde_json::to_string(&AgentMode::Subagent).unwrap();
    assert_eq!(json, "\"subagent\"");
    let back: AgentMode = serde_json::from_str(&json).unwrap();
    assert_eq!(back, AgentMode::Subagent);

    // ModelRef serde round-trip.
    let mref = ModelRef {
        provider_id: "p".to_string(),
        model_id: "m".to_string(),
    };
    let s = serde_json::to_string(&mref).unwrap();
    let m2: ModelRef = serde_json::from_str(&s).unwrap();
    assert_eq!(m2.provider_id, "p");
    assert_eq!(m2.model_id, "m");
}

#[test]
fn agent_fallback_thinking_resolution_paths() {
    let mref = ModelRef {
        provider_id: "stub".to_string(),
        model_id: "stub-model".to_string(),
    };

    // 1. Config per-provider thinking wins.
    let mut config = Config::default();
    config.provider.insert(
        "stub".to_string(),
        ProviderConfig {
            thinking: Some(ThinkingConfig::new(ThinkingLevel::Low)),
            ..Default::default()
        },
    );
    let empty_registry = ProviderRegistry::new();
    let from_config = fallback_thinking_for_model_ref(&config, &empty_registry, &mref);
    assert_eq!(from_config, Some(ThinkingConfig::new(ThinkingLevel::Low)));

    // 2. Model metadata thinking_config wins when config has none.
    let cfg_none = Config::default();
    let registry_with_thinking =
        stub_registry("stub", Some(ThinkingConfig::new(ThinkingLevel::High)));
    let from_model = fallback_thinking_for_model_ref(&cfg_none, &registry_with_thinking, &mref);
    assert_eq!(from_model, Some(ThinkingConfig::new(ThinkingLevel::High)));

    // 3. Model present but no thinking config -> built-in default (off).
    let registry_plain = stub_registry("stub", None);
    let defaulted = fallback_thinking_for_model_ref(&cfg_none, &registry_plain, &mref);
    assert_eq!(defaulted, Some(ThinkingConfig::off()));

    // 4. No provider at all -> still Some(off()).
    assert_eq!(
        fallback_thinking_for_model_ref(&cfg_none, &empty_registry, &mref),
        Some(ThinkingConfig::off())
    );

    // apply_fallback_thinking sets a missing thinking from the resolved model.
    let mut agent = AgentInfo::new("general", "g");
    agent.model = Some(mref.clone());
    apply_fallback_thinking(&mut agent, &cfg_none, &registry_plain);
    assert_eq!(agent.thinking, Some(ThinkingConfig::off()));

    // An agent that already has thinking is left untouched.
    let mut preset = AgentInfo::new("general", "g");
    preset.model = Some(mref.clone());
    preset.thinking = Some(ThinkingConfig::new(ThinkingLevel::Low));
    apply_fallback_thinking(&mut preset, &cfg_none, &registry_plain);
    assert_eq!(
        preset.thinking,
        Some(ThinkingConfig::new(ThinkingLevel::Low))
    );

    // No model at all -> thinking stays None.
    let mut nomodel = AgentInfo::new("general", "g");
    apply_fallback_thinking(&mut nomodel, &cfg_none, &registry_plain);
    assert!(nomodel.thinking.is_none());
}

#[test]
fn agent_default_model_resolution_paths() {
    // Empty registry -> None.
    let agent = AgentInfo::new("general", "g");
    assert!(resolve_default_model(&agent, &ProviderRegistry::new()).is_none());

    // A registry with a model -> the first provider/model pair.
    let registry = stub_registry("stub", None);
    let resolved = resolve_default_model(&agent, &registry).expect("model");
    assert_eq!(resolved.provider_id, "stub");
    assert_eq!(resolved.model_id, "stub-model");

    // An agent that already has a model keeps it.
    let mut pinned = AgentInfo::new("general", "g");
    pinned.model = Some(ModelRef {
        provider_id: "own".to_string(),
        model_id: "own-model".to_string(),
    });
    let kept = resolve_default_model(&pinned, &registry).expect("kept");
    assert_eq!(kept.provider_id, "own");
}

#[test]
fn agent_resolve_with_model_variants() {
    let config = Config::default();
    let registry = stub_registry("stub", None);

    // resolve_agent_with_model assigns a default when the builtin has none.
    let a = resolve_agent_with_model("general", &config, &registry).expect("resolve");
    assert!(a.model.is_some());

    // Empty registry leaves the model unset (no panic).
    let b =
        resolve_agent_with_model("general", &config, &ProviderRegistry::new()).expect("resolve");
    assert!(b.model.is_none());

    // With-customs variant also resolves.
    let tmp = tempfile::tempdir().expect("tempdir");
    let c = resolve_agent_with_customs_and_model("general", &config, tmp.path(), &registry)
        .expect("resolve customs");
    assert!(c.model.is_some());
}

#[test]
fn agent_build_memory_prompt_section_without_storage_is_empty() {
    let dir = tempfile::tempdir().expect("tempdir");
    assert_eq!(build_memory_prompt_section(dir.path(), None, None), "");
}

#[test]
fn agent_prompt_builder_with_explicit_config() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("f.rs"), "fn main() {}").expect("write");

    let mut agent = AgentInfo::new("general", "g");
    agent.max_steps = Some(5);
    let config = Config::default();

    let prompt = build_system_prompt_with_storage_and_memory_and_config(
        &agent,
        dir.path(),
        "tree",
        None,
        Some(""),
        Some(""),
        None,
        None,
        None,
        None,
        Some(&config),
    );
    assert!(prompt.contains("## Sub-Agent Spawning"));
    assert!(prompt.contains("## Reasoning Tool"));
}

#[tokio::test]
async fn agent_prompt_context_component_file_tree_and_agents_md() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("AGENTS.md"), "# Rules\nBe nice.\n").expect("write");

    let agents =
        ragent_agent::agent::prompt_context_component(dir.path(), PromptContextComponent::AgentsMd)
            .await;
    assert!(agents.contains("Be nice."));

    let tree =
        ragent_agent::agent::prompt_context_component(dir.path(), PromptContextComponent::FileTree)
            .await;
    assert!(tree.contains("AGENTS.md"));
}

// A trivial path assertion to make the `Path` import meaningful even if an
// earlier test is trimmed.
#[test]
fn reference_path_helper_is_stable() {
    assert_eq!(
        Path::new("a/b").file_name().and_then(|n| n.to_str()),
        Some("b")
    );
}
