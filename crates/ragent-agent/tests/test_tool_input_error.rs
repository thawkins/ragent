//! Contract tests for the single refusal constructor (spec `uniformtools`
//! T-013 / FR-008, FR-009, FR-014).
//!
//! Two mistakes reach the dispatch pipeline before a tool runs: argument text
//! that will not decode (FR-008) and arguments that violate the tool's declared
//! schema (FR-009). Before `session::tool_input_error` each was built in its own
//! place, with its own sentence, its own status and its own `is_error` flag -
//! and two further sites derived the flag again. The module itself had never
//! been compiled until T-013 declared it, so these cases had never run either.
//! They pin the properties that make one constructor worth having:
//!
//! * each refusal names the tool and quotes the underlying detail verbatim, so
//!   the model can act on it;
//! * both causes report `ToolCallStatus::Error`, and the result's `is_error`
//!   flag is the same derivation the history and compaction converters use;
//! * the `anyhow` adapter the in-task gate uses renders identical text, so the
//!   two gates cannot drift apart;
//! * no other agent source states a refusal sentence, **and** both gates
//!   actually call this constructor - the negative scan alone is satisfiable by
//!   a gate that simply states no sentence, which is the defect this file was
//!   written after.

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

/// The derivation from the record to the wire exists, and it exists in every
/// history converter.
///
/// A file being free of the two wrong spellings is not the same as a file
/// deriving the flag: a converter that wrote `is_error: false` everywhere would
/// pass the scan above while making every failed call look successful. This is
/// the positive half, and it names each site so a converter that stops
/// forwarding the record's status is a red test rather than a silent regression
/// (FR-014; T-017).
#[test]
fn every_history_converter_forwards_the_record_status_to_the_wire_flag() {
    let root = workspace_root();
    for (relative, needle) in [
        // The canonical history converter: every recorded call becomes one
        // `ContentPart::ToolResult`, flagged from its own state.
        (
            "crates/ragent-agent/src/session/history.rs",
            "state.status.is_error()",
        ),
        // The compaction converter, which rebuilds a provider-facing request
        // from the stored history.
        (
            "crates/ragent-agent/src/compaction/convert.rs",
            "state.status.is_error()",
        ),
        // The tool phase's own append path, which holds the outcome's status
        // rather than the state it is about to record.
        (
            "crates/ragent-agent/src/session/processor.rs",
            "status.is_error()",
        ),
    ] {
        let path = root.join(relative);
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        assert!(
            source.contains(needle),
            "{relative} must derive the result's is_error from the recorded \
             status with `{needle}` (FR-014), not from a constant"
        );
    }
}

/// The two gates in the loop reach the refusal through the shared constructor,
/// and neither still hand-assembles its own corrective sentence.
///
/// This is the positive half of
/// [`refusals_are_constructed_only_in_the_shared_constructor`]: a file could be
/// free of the constructor's sentence merely because it never states one, which
/// would let a gate drift back to a bare `ToolCallStatus::Error` and a
/// hand-written message without the scan above noticing.
#[test]
fn both_gates_build_their_refusal_with_the_shared_constructor() {
    let root = workspace_root();
    // The refusal constructors and their adapters live in the session module. A
    // gate's status and sentence are read from the whole module rather than from
    // `processor.rs` alone, because stages 1-3 and the adapters that render their
    // outcomes were moved out of the loop body: a scan of one file would report a
    // gate as missing merely because the code moved (T-014).
    let mut source = String::new();
    for relative in [
        "crates/ragent-agent/src/session/processor.rs",
        "crates/ragent-agent/src/session/dispatch.rs",
        "crates/ragent-agent/src/session/tool_input_error.rs",
        "crates/ragent-agent/src/session/refusal_outcome.rs",
    ] {
        let path = root.join(relative);
        source.push_str(
            &std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display())),
        );
        source.push('\n');
    }

    // FR-008: the decode gate.
    assert!(
        source.contains("ToolInputError::undecodable_with_reason(")
            && source.contains("ToolInputError::undecodable("),
        "the decode gate must build its FR-008 refusal with ToolInputError, not a \
         hand-rolled sentence"
    );
    // FR-009: the in-task schema gate.
    assert!(
        source.contains("ToolInputError::schema_violation("),
        "the in-task schema gate must build its FR-009 refusal with ToolInputError"
    );
    // FR-014: the decode gate's record status is the refusal's own status, not a
    // second opinion about what a refusal means.
    assert!(
        source.contains("refusal.status()"),
        "the decode gate must take its record status from the refusal (FR-014), \
         so the sentence and the status cannot disagree"
    );
    // The two hand-rolled sentences the constructor replaced.
    for stale in [
        "Invalid tool arguments:",
        "Invalid arguments JSON for tool",
        "Arguments must be a single valid JSON object",
    ] {
        assert!(
            !source.contains(stale),
            "processor.rs still states the pre-T-013 sentence {stale:?}; the \
             refusal wording belongs to {REFUSAL_OWNER} (FR-008, FR-009)"
        );
    }
}

// ---------------------------------------------------------------------------
// FR-011: the reasoning signature survives the round trip (T-018)
// ---------------------------------------------------------------------------

/// The signature that the provider streamed has to reach the record the history
/// converter replays, or the echoed block is unsigned and the provider either
/// rejects it or cannot verify it originated the reasoning.
///
/// The two halves are checked as source facts because they are what the loop
/// writes and what the converter reads; the wire shape they feed is asserted by
/// `crates/ragent-llm/tests/test_wire_encode_message.rs`
/// (`..._replays_the_signed_thinking_block_before_the_call`) and the stream
/// event by `providers::anthropic::tests`.
#[test]
fn the_reasoning_signature_is_recorded_and_replayed_from_history() {
    let root = workspace_root();

    // 1. The loop records the streamed signature beside the reasoning text.
    let processor =
        std::fs::read_to_string(root.join("crates/ragent-agent/src/session/processor.rs"))
            .expect("read processor.rs");
    assert!(
        processor.contains("signature: llm_result.reasoning_signature.clone()"),
        "the loop must record the streamed signature with the reasoning text \
         (FR-011), not the text alone"
    );

    // 2. Both history converters forward it to the wire part.
    for relative in [
        "crates/ragent-agent/src/session/history.rs",
        "crates/ragent-agent/src/compaction/convert.rs",
    ] {
        let path = root.join(relative);
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        assert!(
            source.contains("ContentPart::Reasoning {")
                && source.contains("signature: signature.clone()"),
            "{relative} must forward the reasoning signature to the wire part \
             (FR-011); a converter that drops it makes every echoed block \
             unsigned"
        );
    }

    // 3. The omission itself belongs to the encoder, not to history: a
    //    converter that decided *per provider* which blocks to forward would
    //    re-implement `echo_reasoning` outside the profile table.
    let history = std::fs::read_to_string(root.join("crates/ragent-agent/src/session/history.rs"))
        .expect("read history.rs");
    assert!(
        !history.contains("echo_reasoning"),
        "the `echo_reasoning` decision belongs to the profile-driven encoder \
         (FR-011), not to the history converter"
    );
}

/// The canonical record keeps the two halves apart: a reasoning part serialises
/// `text` and `signature`, and a signature-less block omits the key rather than
/// writing `null`.
///
/// The stored `parts` JSON is what a resumed session replays, so the signature
/// has to survive storage for the echo to work after a restart.
#[test]
fn a_persisted_reasoning_part_keeps_its_signature() {
    use ragent_types::message::MessagePart;

    let signed = MessagePart::Reasoning {
        text: "weigh the options".to_string(),
        signature: Some("EqQBCkYIBxgCKkA".to_string()),
    };
    let json = serde_json::to_value(&signed).expect("serialise");
    assert_eq!(json["type"], "reasoning");
    assert_eq!(json["text"], "weigh the options");
    assert_eq!(
        json["signature"], "EqQBCkYIBxgCKkA",
        "the signature is half of the block FR-011 replays"
    );

    let back: MessagePart = serde_json::from_value(json).expect("deserialise");
    match back {
        MessagePart::Reasoning { text, signature } => {
            assert_eq!(text, "weigh the options");
            assert_eq!(signature.as_deref(), Some("EqQBCkYIBxgCKkA"));
        }
        other => panic!("expected a reasoning part, got {other:?}"),
    }

    let unsigned = MessagePart::Reasoning {
        text: "no signature".to_string(),
        signature: None,
    };
    let json = serde_json::to_value(&unsigned).expect("serialise");
    assert!(
        json.get("signature").is_none(),
        "an unsigned block gains no `signature` key: {json}"
    );
    // A record written before the field existed still loads, as unsigned.
    let legacy: MessagePart = serde_json::from_str(r#"{"type":"reasoning","text":"old"}"#)
        .expect("a pre-signature record still deserialises");
    assert!(matches!(
        legacy,
        MessagePart::Reasoning {
            signature: None,
            ..
        }
    ));
}
