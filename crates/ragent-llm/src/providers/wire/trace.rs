//! The opt-in payload trace (spec `uniformtools` FR-017).
//!
//! Where `tools.wire_trace` is enabled in the config, every provider writes the
//! exact request body it is about to send to
//! `target/temp/wire-trace/<session>-<step>.json`, with every credential
//! redacted through the shared sanitizer. The switch is a process-wide
//! [`AtomicBool`] that defaults to `false`, so a build that never sets it
//! writes nothing at all - no directory, no file - and the whole facility costs
//! one relaxed load per provider send.
//!
//! ## Why the switch is process-wide
//!
//! The body only exists inside each provider's `build_request_body`, which is a
//! method on the concrete client, so the recording has to happen at that call
//! site. Threading a per-request flag to fifteen clients would mean either a new
//! `ChatRequest` field (and every one of its construction sites) or a magic key
//! in `ChatRequest::options` that a provider could accidentally serialise onto
//! the wire. A process-wide switch set once per turn by the agent loop from
//! `session_config.tools.wire_trace` costs nothing on the default path and cannot
//! leak into a request body. The trade-off is documented rather than hidden: two
//! concurrent sessions with different settings share the last value the loop
//! wrote, which is acceptable for a diagnostic switch whose only effect is that
//! an additional file appears under `target/temp/`.
//!
//! ## What a trace file contains
//!
//! The body alone - the object with the `tools` array and the `messages` array -
//! so the file can be diffed against a provider's documented request shape. It is
//! redacted twice: once by key name (any credential-named field, so an
//! `api_key`-style entry cannot survive under a plausible name) and once by value
//! (the shared [`ragent_types::sanitize::redact_json_value`] pass, which catches a
//! credential that sits under an innocuous key). Every byte is ASCII, because a
//! trace is read in a terminal and NFR-005 applies to it.
//!
//! Writing is best effort: a trace that cannot be written logs a warning and the
//! turn proceeds. The trace observes the request; it never gates it.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use serde_json::Value;

use crate::llm::ChatRequest;

/// Directory under the working directory that trace files are written to.
const DEFAULT_ROOT: &str = "target/temp/wire-trace";

/// Bounds the redaction walk so a deeply nested (potentially provider-supplied)
/// value cannot make redaction expensive. A real request body is shallow.
const MAX_REDACT_DEPTH: usize = 16;

/// The process-wide switch (FR-017). Default `false`: the facility is inert
/// until the agent loop sets it from `tools.wire_trace`.
static ENABLED: AtomicBool = AtomicBool::new(false);

/// The trace root, overridable once per process by [`set_root`].
fn root_cell() -> &'static OnceLock<PathBuf> {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();
    &ROOT
}

/// Per-session send counter, so file names carry the step number.
fn step_counters() -> &'static Mutex<HashMap<String, u64>> {
    static COUNTERS: OnceLock<Mutex<HashMap<String, u64>>> = OnceLock::new();
    COUNTERS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Enable or disable the payload trace (FR-017).
///
/// The agent loop calls this once per turn from `tools.wire_trace`, so a config
/// change takes effect on the next turn. It is idempotent and lock-free.
pub fn set_enabled(enabled: bool) {
    ENABLED.store(enabled, Ordering::Relaxed);
}

/// Whether the payload trace is currently enabled.
///
/// Providers call [`record_body`], which short-circuits on this; a test asserts
/// the gate so the "inert by default" half of FR-017 is observable.
#[must_use]
pub fn is_enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// Point the trace at `root` for this process.
///
/// A test hook: the value is set once and applies process-wide so a test run
/// writes under `target/temp/` and never into a checked-in tree. Production code
/// never calls it, which is why the default ([`DEFAULT_ROOT`]) is what ships.
#[doc(hidden)]
pub fn set_root(root: impl Into<PathBuf>) {
    // INTENTIONAL: first set wins; a second call is a no-op so two tests cannot
    // fight over one process-wide root.
    let _ = root_cell().set(root.into());
}

/// The directory trace files are written to ([`DEFAULT_ROOT`] unless overridden).
#[must_use]
pub fn root() -> PathBuf {
    root_cell()
        .get()
        .cloned()
        .unwrap_or_else(|| PathBuf::from(DEFAULT_ROOT))
}

/// The file [`record_body`] would write for this send, or `None` while the trace
/// is disabled.
///
/// Split out of [`record_body`] so the "inert" half of FR-017 is observable
/// without a filesystem: a disabled trace has no destination at all. While
/// enabled it consumes one step number for the request's session, exactly as a
/// write would, so call it only when the send is about to happen.
#[must_use]
pub fn next_path(request: &ChatRequest) -> Option<PathBuf> {
    if !is_enabled() {
        return None;
    }
    Some(path_for(request))
}

/// The destination for `request`, consuming one step number for its session.
///
/// Private on purpose: the step counter only advances for a call that is really
/// going to write, so an "is the trace on" question can never skip a step.
fn path_for(request: &ChatRequest) -> PathBuf {
    let session = session_component(request.session_id.as_deref());
    let step = next_step(&session);
    root().join(format!("{session}-{step:04}.json"))
}

/// Write `body` - the request body a provider is about to send for `request` -
/// to the trace directory (FR-017).
///
/// Does nothing, and touches the filesystem not at all, while [`is_enabled`] is
/// `false`. When enabled it writes `<session>-<step>.json`, where `<step>` is a
/// per-session counter starting at `0001`, so a retried send gets its own file
/// (a retry is a second request on the wire, and seeing both is the point).
///
/// The body is redacted by key name and then by value, and every non-ASCII
/// character is escaped, so the file is safe to paste into a terminal.
///
/// # Errors
///
/// None are returned: a trace that cannot be written logs a warning and the turn
/// continues. Tracing observes the request and must never block it.
pub fn record_body(request: &ChatRequest, body: &Value) {
    if !is_enabled() {
        return;
    }
    let path = path_for(request);
    let dir = root();
    if let Err(error) = std::fs::create_dir_all(&dir) {
        tracing::warn!(
            path = %dir.display(),
            error = %error,
            "wire trace: cannot create the trace directory; the turn continues untraced"
        );
        return;
    }
    let text = render(body);
    if let Err(error) = write_atomic(&path, &text) {
        tracing::warn!(
            path = %path.display(),
            error = %error,
            "wire trace: cannot write the trace file; the turn continues untraced"
        );
    }
}

/// Make one component of a trace file name ASCII and filesystem-safe.
///
/// A session id is a UUID in production, but it arrives from storage and from
/// callers, so anything that is not an ASCII alphanumeric, `-` or `_` becomes
/// `_`. An absent id renders as `no-session`. The result is never empty, which is
/// what keeps the file name ASCII (NFR-005).
fn session_component(session_id: Option<&str>) -> String {
    let Some(session_id) = session_id else {
        return String::from("no-session");
    };
    let cleaned: String = session_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if cleaned.is_empty() {
        String::from("no-session")
    } else {
        cleaned
    }
}

/// Reserve the next send number for `session`.
fn next_step(session: &str) -> u64 {
    let mut counters = step_counters()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let slot = counters.entry(session.to_string()).or_insert(0);
    *slot += 1;
    *slot
}

/// Render a body as indented, redacted, ASCII-only JSON.
fn render(body: &Value) -> String {
    let mut value = body.clone();
    redact(&mut value);
    let pretty = serde_json::to_string_pretty(&value)
        .unwrap_or_else(|error| format!("{{\"trace_error\": \"{error}\"}}"));
    escape_non_ascii(&pretty)
}

/// Redact every credential in `value`, by key name and then by value.
///
/// The key-name pass runs first so a credential-named field cannot survive at
/// all; the value pass then catches a credential that sits under an innocuous
/// key (an MCP server's `env` entry, a header map, a URL query string) by
/// matching the shapes the shared sanitizer recognises anywhere in a string.
fn redact(value: &mut Value) {
    redact_named_keys(value, 0);
    redact_matched_values(value, 0);
}

/// Run the shared [`ragent_types::sanitize::redact_secrets`] matcher over every
/// string in `value`.
///
/// [`ragent_types::sanitize::redact_json_value`] redacts a whole string *value*,
/// which is right for a log field and wrong for a request body: a body's
/// `system` prompt or tool description legitimately mentions `api_key=...` or a
/// `Bearer` header shape while explaining one, and blanking the whole string
/// would destroy the document the trace exists to show. Matching inside the
/// string is the same shared pattern set and the same placeholder - only the
/// whole-value replacement is dropped - so the trace redacts exactly what the
/// sanitizer redacts, and nothing else.
fn redact_matched_values(value: &mut Value, depth: usize) {
    if depth > MAX_REDACT_DEPTH {
        return;
    }
    match value {
        Value::String(text) => *text = ragent_types::sanitize::redact_secrets(text),
        Value::Array(items) => {
            for item in items.iter_mut() {
                redact_matched_values(item, depth + 1);
            }
        }
        Value::Object(map) => {
            for entry in map.values_mut() {
                redact_matched_values(entry, depth + 1);
            }
        }
        _ => {}
    }
}

/// Replace the value of every credential-named key with the shared placeholder.
///
/// Only string values are replaced: a tool's JSON *schema* legitimately has a
/// property called `token`, and masking that object would destroy the very
/// document the trace exists to show. Recursion continues into every non-secret
/// value so a credential nested under an innocuous key is still reached by the
/// value pass in [`redact`].
fn redact_named_keys(value: &mut Value, depth: usize) {
    if depth > MAX_REDACT_DEPTH {
        return;
    }
    match value {
        Value::Object(map) => {
            for (key, entry) in map.iter_mut() {
                let present = entry.as_str().is_some_and(|s| {
                    !s.trim().is_empty() && s != ragent_types::sanitize::REDACTION_PLACEHOLDER
                });
                if ragent_types::sanitize::is_secret_json_key(key) && present {
                    *entry =
                        Value::String(ragent_types::sanitize::REDACTION_PLACEHOLDER.to_string());
                } else {
                    redact_named_keys(entry, depth + 1);
                }
            }
        }
        Value::Array(items) => {
            for item in items.iter_mut() {
                redact_named_keys(item, depth + 1);
            }
        }
        _ => {}
    }
}

/// Rewrite every non-ASCII character as a JSON `\uXXXX` escape (NFR-005).
///
/// A non-ASCII character can only appear inside a JSON string literal, where the
/// escape is equivalent, so the file stays valid JSON while every byte is ASCII.
fn escape_non_ascii(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut buffer = [0u16; 2];
    for character in text.chars() {
        if character.is_ascii() {
            out.push(character);
        } else {
            for unit in character.encode_utf16(&mut buffer) {
                out.push_str(&format!("\\u{unit:04x}"));
            }
        }
    }
    out
}

/// Write `contents` to `path` through a temporary file in the same directory.
///
/// A crash mid-write would otherwise leave a truncated trace that looks like a
/// truncated request, so the file is built beside its destination and renamed
/// into place.
fn write_atomic(path: &Path, contents: &str) -> std::io::Result<()> {
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let temporary = path.with_file_name(format!(".{file_name}.{}.tmp", std::process::id()));
    std::fs::write(&temporary, contents)?;
    match std::fs::rename(&temporary, path) {
        Ok(()) => Ok(()),
        Err(error) => {
            // Clean up the partial temporary file; the rename failure itself is
            // the error the caller reports.
            if let Err(cleanup) = std::fs::remove_file(&temporary) {
                tracing::debug!(
                    path = %temporary.display(),
                    error = %cleanup,
                    "wire trace: cannot remove a temporary file after a failed rename"
                );
            }
            Err(error)
        }
    }
}
