//! The opt-in payload trace (spec `uniformtools` FR-017).
//!
//! `tools.wire_trace` writes each step's request body to
//! `target/temp/wire-trace/<session>-<step>.json`, redacted and ASCII-only, and
//! is inert while the setting is absent or `false`. These tests pin the three
//! halves that can silently regress: the default is off *and writes nothing*, an
//! enabled trace redacts every credential shape, and every provider that builds a
//! body of its own actually records it - a provider that quietly skips the call
//! would make the trace incomplete without any error.
//!
//! The switch and the trace root are process-wide (see
//! [`ragent_llm::provider::wire::trace`]), so every test here takes
//! `crate::global_env_lock()` and gives its session a unique id. The root is
//! pointed at `target/temp/` so a test run never writes into the repository.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use ragent_llm::llm::{ChatContent, ChatMessage, ChatRequest, ToolDefinition};
use ragent_llm::provider::wire::trace;

/// The scratch tree every trace test writes under, resolved from the crate
/// manifest so it is correct regardless of the test's working directory.
fn scratch_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("temp")
        .join("wire-trace-tests")
}

/// Point the trace root at the scratch tree exactly once for this process.
///
/// `set_root` is first-set-wins by design, so the call is idempotent and every
/// test that enables the trace shares the one root.
fn use_scratch_root() {
    static INIT: OnceLock<()> = OnceLock::new();
    INIT.get_or_init(|| {
        let root = scratch_root();
        std::fs::create_dir_all(&root).expect("create the wire-trace scratch root");
        trace::set_root(root);
    });
}

/// A request carrying one tool definition, so the recorded body has both a
/// `tools` array and a `messages` array.
fn request(session_id: &str) -> ChatRequest {
    ChatRequest {
        model: "test-model".to_string(),
        messages: Arc::new(vec![ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Text("list the files in this directory".to_string()),
        }]),
        tools: Arc::new(vec![ToolDefinition {
            name: "Bash".to_string(),
            description: "Run a shell command".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": { "command": { "type": "string" } },
                "required": ["command"],
            }),
        }]),
        temperature: None,
        top_p: None,
        max_tokens: Some(64),
        system: Some(Arc::from("system")),
        options: HashMap::new(),
        session_id: Some(session_id.to_string()),
        request_id: None,
        stream_timeout_secs: None,
        thinking: None,
    }
}

/// A request with no session id, which must still name a file.
fn sessionless_request() -> ChatRequest {
    let mut built = request("unused");
    built.session_id = None;
    built
}

/// The body a provider would put on the wire for [`request`].
fn body() -> serde_json::Value {
    serde_json::json!({
        "model": "test-model",
        "tools": [{ "type": "function", "function": { "name": "Bash" } }],
        "messages": [{ "role": "user", "content": "list the files in this directory" }],
        "max_tokens": 64,
    })
}

/// Every trace file whose name starts with `prefix`.
fn trace_files(prefix: &str) -> Vec<PathBuf> {
    let dir = trace::root();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut found: Vec<PathBuf> = entries
        .filter_map(std::result::Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(prefix))
        })
        .collect();
    found.sort();
    found
}

/// Remove a test's own files, so a rerun starts from a known state without the
/// test depending on a previous run's leftovers.
fn clear(prefix: &str) {
    for path in trace_files(prefix) {
        let _ = std::fs::remove_file(&path); // INTENTIONAL: best-effort test cleanup
    }
}

/// The trace is inert by default: nothing is written and no directory is created.
#[test]
fn test_wire_trace_is_inert_by_default() {
    use_scratch_root();
    let _guard = crate::global_env_lock();
    trace::set_enabled(false);
    let session = "t023-default";
    clear(session);

    assert!(
        !trace::is_enabled(),
        "the payload trace must default to off (FR-017)"
    );
    assert!(
        trace::next_path(&request(session)).is_none(),
        "a disabled trace must have no destination at all (FR-017)"
    );
    trace::record_body(&request(session), &body());

    assert!(
        trace_files(session).is_empty(),
        "a disabled trace must write nothing at all"
    );
}

/// One file per step, named with the session id and the step number.
#[test]
fn test_wire_trace_writes_one_file_per_step() {
    use_scratch_root();
    let _guard = crate::global_env_lock();
    let session = "t023-steps";
    clear(session);
    trace::set_enabled(true);

    let request = request(session);
    let body = body();
    trace::record_body(&request, &body);
    trace::record_body(&request, &body);

    let files = trace_files(session);
    assert_eq!(
        files.len(),
        2,
        "one trace file per recorded send: {files:?}"
    );
    let names: Vec<String> = files
        .iter()
        .map(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
                .to_string()
        })
        .collect();
    assert_eq!(names[0], format!("{session}-0001.json"));
    assert_eq!(names[1], format!("{session}-0002.json"));

    // Disabling stops further writes: a trace set back to `false` leaves the
    // directory untouched.
    trace::set_enabled(false);
    trace::record_body(&request, &body);
    assert_eq!(
        trace_files(session).len(),
        2,
        "a trace switched off must not gain another file"
    );
}

/// The recorded file carries the request body, with both the `tools` array and
/// the `messages` array, so a request shape can be diffed against the docs.
#[test]
fn test_wire_trace_records_the_tools_and_messages_arrays() {
    use_scratch_root();
    let _guard = crate::global_env_lock();
    let session = "t023-body";
    clear(session);
    trace::set_enabled(true);

    trace::record_body(&request(session), &body());
    trace::set_enabled(false);

    let files = trace_files(session);
    assert_eq!(files.len(), 1, "expected exactly one trace file");
    let text = std::fs::read_to_string(&files[0]).expect("read the trace file");
    let parsed: serde_json::Value = serde_json::from_str(&text).expect("the trace is valid JSON");

    assert_eq!(parsed["model"], "test-model");
    assert_eq!(
        parsed["tools"][0]["function"]["name"], "Bash",
        "the tools array must be recorded verbatim"
    );
    assert_eq!(
        parsed["messages"][0]["content"], "list the files in this directory",
        "the messages array must be recorded verbatim"
    );
}

/// Every credential is redacted: a value under a credential-named key, a
/// credential-shaped value under an innocuous key, and a `Bearer` token.
#[test]
fn test_wire_trace_redacts_every_credential() {
    use_scratch_root();
    let _guard = crate::global_env_lock();
    let session = "t023-redaction";
    clear(session);
    trace::set_enabled(true);

    // Real-shaped credentials: the shared sanitizer's patterns have length
    // floors (`sk-` needs 20+ characters, `Bearer` 20+), so a short placeholder
    // would pass this test without being redacted by anything.
    const SK: &str = "sk-live-abcdefghijklmnopqrstuvwxyz0123456789";
    const BEARER: &str = "Bearer abcdefghijklmnopqrstuvwxyz0123456789";
    const GITHUB: &str = "ghp_abcdefghijklmnopqrstuvwxyz0123456789";
    // An innocuous key whose value is a credential: only the value pass can
    // catch this, so it is what proves that pass runs.
    const INNOCUOUS_KEY: &str = "x-note";

    let mut body = body();
    body["api_key"] = serde_json::json!(SK);
    body["headers"] = serde_json::json!({
        "Authorization": BEARER,
        INNOCUOUS_KEY: GITHUB,
    });
    trace::record_body(&request(session), &body);
    trace::set_enabled(false);

    let files = trace_files(session);
    assert_eq!(files.len(), 1);
    let text = std::fs::read_to_string(&files[0]).expect("read the trace file");

    for secret in [SK, BEARER, GITHUB] {
        assert!(
            !text.contains(secret),
            "credential `{secret}` survived redaction: {text}"
        );
    }
    assert!(
        text.contains(ragent_types::sanitize::REDACTION_PLACEHOLDER),
        "a redacted trace names the shared placeholder: {text}"
    );
}

/// The file is ASCII-only (NFR-005): a non-ASCII message is escaped rather than
/// written raw, so the file can be pasted into a terminal.
#[test]
fn test_wire_trace_output_is_ascii() {
    use_scratch_root();
    let _guard = crate::global_env_lock();
    let session = "t023-ascii";
    clear(session);
    trace::set_enabled(true);

    let mut body = body();
    body["messages"][0]["content"] = serde_json::json!("cafe\u{301} na\u{ef}ve \u{1f600}");
    trace::record_body(&request(session), &body);
    trace::set_enabled(false);

    let files = trace_files(session);
    assert_eq!(files.len(), 1);
    let bytes = std::fs::read(&files[0]).expect("read the trace file");
    let offending: Vec<u8> = bytes.iter().copied().filter(|byte| *byte >= 0x80).collect();
    assert!(
        offending.is_empty(),
        "the trace file must be pure ASCII (NFR-005); found {offending:?}"
    );
    let text = String::from_utf8(bytes).expect("ASCII is valid UTF-8");
    assert!(
        text.contains("\\u"),
        "a non-ASCII character must be escaped, not dropped: {text}"
    );

    let name = files[0]
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    assert!(
        name.is_ascii(),
        "the trace file name must be ASCII (NFR-005): {name}"
    );
}

/// A session id that is absent, or carries a path separator, still names a file
/// inside the trace directory - never an empty name and never a nested path.
///
/// The step counter restarts in each process and the trace root is shared, so a
/// fixed file name can survive from an earlier run and be overwritten rather than
/// added. Both directories this test inspects are cleared first and the session id
/// carries this test's own prefix, so the claim is about the names this call
/// produced rather than about the whole directory.
#[test]
fn test_wire_trace_file_name_is_sanitised() {
    use_scratch_root();
    let _guard = crate::global_env_lock();
    // `..`, a `/` separator and every other non-alphanumeric character become
    // `_`, so the id can neither escape the root nor nest a directory inside it.
    const ESCAPING: &str = "../t023-escape/../../etc";
    const FLATTENED: &str = "___t023-escape_______etc";
    clear(FLATTENED);
    clear("no-session");
    trace::set_enabled(true);

    trace::record_body(&request(ESCAPING), &body());
    trace::record_body(&sessionless_request(), &body());
    trace::set_enabled(false);

    let escaped = trace_files(FLATTENED);
    assert_eq!(
        escaped.len(),
        1,
        "the session id must be flattened into exactly one file name: {escaped:?}"
    );
    assert_eq!(
        escaped[0].parent(),
        Some(trace::root().as_path()),
        "the flattened file must sit directly in the trace root"
    );
    let name = escaped[0]
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    assert_eq!(name, format!("{FLATTENED}-0001.json"));
    assert!(name.is_ascii(), "a trace file name must be ASCII: {name}");

    // An absent id renders as `no-session` rather than an empty name.
    let no_session = trace_files("no-session");
    assert_eq!(
        no_session.len(),
        1,
        "an absent session id must render as `no-session`: {no_session:?}"
    );
    assert_eq!(
        no_session[0].parent(),
        Some(trace::root().as_path()),
        "the `no-session` file sits directly in the trace root"
    );
}

/// Every provider that builds a request body of its own records it.
///
/// A provider that skipped the call would make the trace silently incomplete,
/// and nothing else would fail - which is exactly the kind of gap a source guard
/// catches and a runtime test cannot. The two exclusions are named with their
/// reason rather than inferred.
#[test]
fn test_every_provider_records_its_body() {
    let providers_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("providers");

    // Not exclusions of convenience: `mock_llm_client` has no wire at all, and
    // `router_client` delegates to a downstream client, whose own `chat` does the
    // recording (recording the router's own wrapper would duplicate the file).
    const NO_WIRE: &[&str] = &["mock_llm_client.rs", "router_client.rs"];

    let mut checked = 0usize;
    let mut offenders = Vec::new();
    for entry in std::fs::read_dir(&providers_dir).expect("read the providers directory") {
        let path = entry.expect("directory entry").path();
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_string();
        if path.extension().and_then(|extension| extension.to_str()) != Some("rs")
            || NO_WIRE.contains(&name.as_str())
        {
            continue;
        }
        let source = std::fs::read_to_string(&path).expect("read a provider source file");
        if !source.contains("impl LlmClient for") {
            continue;
        }
        checked += 1;
        if !source.contains("trace::record_body") {
            offenders.push(name);
        }
    }

    assert!(
        checked >= 10,
        "the guard expects to see every provider with a client; found {checked}"
    );
    assert!(
        offenders.is_empty(),
        "these providers build a body but never record it for the payload trace \
         (FR-017): {offenders:?}"
    );
}

/// The agent loop sets the switch from `tools.wire_trace` once per turn.
///
/// Without this the switch would exist and never be set, so the facility would
/// be permanently inert - a config key that silently does nothing.
#[test]
fn test_agent_loop_sets_the_switch_from_config() {
    let loop_source = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("ragent-agent")
            .join("src")
            .join("session")
            .join("loop_steps.rs"),
    )
    .expect("read the agent loop step source");

    assert!(
        loop_source.contains("trace::set_enabled(session_config.tools.wire_trace)"),
        "the loop must drive the trace switch from `tools.wire_trace` (FR-017)"
    );
}
