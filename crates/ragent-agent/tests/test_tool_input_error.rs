//! Contract tests for the single refusal constructor (spec `uniformtools`
//! T-013 / FR-008, FR-009, FR-014).
//!
//! Two mistakes reach the dispatch pipeline before a tool runs: argument text
//! that will not decode (FR-008) and arguments that violate the tool's declared
//! schema (FR-009). Before `session::tool_input_error` each was built in its own
//! place, with its own sentence, its own status and its own `is_error` flag -
//! and two further sites derived the flag again. These cases pin the properties
//! that make one constructor worth having:
//!
//! * each refusal names the tool and quotes the underlying detail verbatim, so
//!   the model can act on it;
//! * both causes report `ToolCallStatus::Error`, and the result's `is_error`
//!   flag is the same derivation the history and compaction converters use;
//! * the `anyhow` adapter the in-task gate uses renders identical text, so the
//!   two gates cannot drift apart.

use ragent_agent::message::ToolCallStatus;
use ragent_agent::session::tool_input_error::ToolInputError;
use ragent_types::ToolArgDecodeError;

/// A decode failure names the tool, quotes the parser, instructs a resend, and
/// says the tool did not run (FR-008).
#[test]
fn undecodable_refusal_names_quotes_and_instructs() {
    let decode_error = ragent_types::tool_args::decode_tool_arguments("{not valid json")
        .expect_err("the payload is not JSON");
    let refusal = ToolInputError::undecodable("read", &decode_error);
    let text = refusal.to_string();

    assert_eq!(refusal.tool(), "read", "the refusal names the tool");
    assert!(
        text.contains("tool 'read'"),
        "the sentence the model reads names the tool: {text}"
    );
    // FR-008: the parse error is quoted, not paraphrased.
    assert!(
        text.contains(&decode_error.to_string()),
        "the corrective text quotes the decoder's own message: {text}"
    );
    assert!(
        text.contains("resend"),
        "the model is told to resend the call: {text}"
    );
    assert!(
        text.contains("not executed"),
        "an unexecuted call must say so: {text}"
    );
}

/// A schema violation names the tool and states the offending parameter and its
/// declared type (FR-009).
#[test]
fn schema_refusal_names_the_parameter_and_its_type() {
    let detail = ragent_tools_core::schema::validate_required_args(
        &serde_json::json!({
            "type": "object",
            "properties": {"destination": {"type": "string"}},
            "required": ["destination"]
        }),
        &serde_json::json!({}),
    )
    .expect_err("the required parameter is absent");
    let refusal = ToolInputError::schema_violation("move_file", &detail);
    let text = refusal.to_string();

    assert_eq!(refusal.tool(), "move_file");
    assert!(
        text.contains("tool 'move_file'"),
        "the sentence names the tool: {text}"
    );
    assert!(
        text.contains("'destination'"),
        "the sentence names the offending parameter: {text}"
    );
    assert!(
        text.contains("string"),
        "the sentence states the declared type: {text}"
    );
    assert!(
        text.contains("not executed"),
        "a refused call must say it did not run: {text}"
    );
}

/// Both refusals are recorded as errors, and their `is_error` flag is the
/// status type's own derivation (FR-014) - never a second opinion.
#[test]
fn every_refusal_is_an_error_by_the_status_derivation() {
    let decode_error = ToolArgDecodeError::NotAnObject { found: "an array" };
    let refusals = [
        ToolInputError::undecodable("read", &decode_error),
        ToolInputError::schema_violation("read", "missing required parameter 'path'"),
    ];
    for refusal in &refusals {
        assert_eq!(
            refusal.status(),
            ToolCallStatus::Error,
            "a call that never ran is recorded as an error, not as a completion"
        );
        assert!(refusal.is_error(), "an error result carries the flag");
        assert_eq!(
            refusal.is_error(),
            refusal.status().is_error(),
            "the flag and the status are one derivation"
        );
    }
}

/// The `anyhow` adapter used by the in-task gate renders the same sentence the
/// pre-dispatch gate stores, including through `{:#}` (the form the loop's
/// error path uses).
#[test]
fn anyhow_adapter_preserves_the_sentence() {
    let refusal = ToolInputError::schema_violation("read", "missing required parameter 'path'");
    let expected = refusal.to_string();
    let error = refusal.into_anyhow();

    assert_eq!(error.to_string(), expected, "Display is unchanged");
    assert_eq!(
        format!("{error:#}"),
        expected,
        "the alternate form the tool-result path uses is unchanged"
    );
    // The error also survives boxing, which is how it crosses the task
    // boundary back to the loop.
    let boxed: Box<dyn std::error::Error + Send + Sync> = Box::new(ToolInputError::undecodable(
        "read",
        &ToolArgDecodeError::InvalidJson {
            detail: "expected value".to_string(),
        },
    ));
    assert!(boxed.to_string().contains("tool 'read'"));
}

/// The two refusal kinds are distinguishable by value, so a caller that needs
/// to tell "could not decode" from "decoded but invalid" does not have to match
/// on message text.
#[test]
fn refusal_kinds_are_distinct_values() {
    let decode = ToolInputError::undecodable(
        "read",
        &ToolArgDecodeError::NotAnObject { found: "a number" },
    );
    let schema = ToolInputError::schema_violation("read", "missing required parameter 'path'");
    assert_ne!(decode, schema);
    assert!(matches!(decode, ToolInputError::Undecodable { .. }));
    assert!(matches!(schema, ToolInputError::SchemaViolation { .. }));
}

// ---------------------------------------------------------------------------
// Source guard: the refusals are built in one place.
// ---------------------------------------------------------------------------

/// The workspace root, resolved from this crate's manifest directory.
fn workspace_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("crates/<name> sits two levels below the workspace root")
        .to_path_buf()
}

/// The owner of the refusal sentences. Every corrective "the tool did not run"
/// result must be constructed through it, so a new refusal path cannot invent a
/// second wording, a second status or a second flag.
const REFUSAL_OWNER: &str = "crates/ragent-agent/src/session/tool_input_error.rs";

/// No other agent source file states a refusal sentence or hand-builds an
/// error status for a call that never ran (FR-008, FR-009, FR-014).
#[test]
fn refusals_are_constructed_only_in_the_shared_constructor() {
    let root = workspace_root();
    let src = root.join("crates/ragent-agent/src");
    let mut offenders: Vec<String> = Vec::new();
    let mut stack = vec![src];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().is_none_or(|e| e != "rs") {
                continue;
            }
            let relative = path
                .strip_prefix(&root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            if relative == REFUSAL_OWNER {
                continue;
            }
            let Ok(source) = std::fs::read_to_string(&path) else {
                continue;
            };
            // The corrective sentences themselves.
            for marker in ["resend the tool call", "resend the call with arguments"] {
                if source.contains(marker) {
                    offenders.push(format!("{relative} states a refusal sentence: {marker}"));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "a tool-call refusal must be built by {REFUSAL_OWNER} (FR-008, FR-009, \
         FR-014); found:\n{}",
        offenders.join("\n")
    );
}

/// The `is_error` flag is derived from `ToolCallStatus`, never re-derived by
/// comparing the status to `Error` at a call site.
#[test]
fn is_error_is_derived_from_the_status_type() {
    let root = workspace_root();
    for relative in [
        "crates/ragent-agent/src/session/history.rs",
        "crates/ragent-agent/src/compaction/convert.rs",
        "crates/ragent-agent/src/session/processor.rs",
    ] {
        let path = root.join(relative);
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        assert!(
            !source.contains("is_error: !success") && !source.contains("== ToolCallStatus::Error,"),
            "{relative} re-derives the is_error flag instead of calling \
             ToolCallStatus::is_error (FR-014)"
        );
    }
}
