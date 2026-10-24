//! Contract tests for `decode_tool_arguments` (spec `uniformtools` FR-007).
//!
//! The helper is the only decode of accumulated tool-call argument text in the
//! workspace, so its three accepted shapes (object, JSON string, empty /
//! whitespace) and its refusals are pinned here rather than re-derived in each
//! caller's tests.

use ragent_types::tool_args::{ToolArgDecodeError, decode_tool_arguments};
use serde_json::json;

#[test]
fn object_passes_through_unchanged() {
    let decoded = decode_tool_arguments(r#"{"path":"a.rs","limit":3}"#).expect("object");
    assert_eq!(decoded, json!({"path": "a.rs", "limit": 3}));
}

#[test]
fn empty_object_is_accepted() {
    let decoded = decode_tool_arguments("{}").expect("empty object");
    assert_eq!(decoded, json!({}));
}

#[test]
fn json_string_decoding_to_an_object_is_accepted() {
    // The double-encoded form a server emits when it puts the argument object
    // inside a string field instead of making it the field's value.
    let decoded = decode_tool_arguments(r#""{\"path\":\"a.rs\"}""#).expect("encoded string");
    assert_eq!(decoded, json!({"path": "a.rs"}));
}

#[test]
fn empty_and_whitespace_decode_to_an_empty_object() {
    for raw in ["", " ", "\n", "\t \r\n "] {
        let decoded =
            decode_tool_arguments(raw).unwrap_or_else(|e| panic!("{raw:?} must decode: {e}"));
        assert_eq!(decoded, json!({}), "raw {raw:?}");
    }
}

#[test]
fn empty_json_string_decodes_to_an_empty_object() {
    // A string that holds nothing is a call with no arguments, matching the
    // empty-text rule rather than an "encodes nothing" error.
    assert_eq!(
        decode_tool_arguments(r#""""#).expect("empty string"),
        json!({})
    );
}

#[test]
fn surrounding_whitespace_is_trimmed_at_both_layers() {
    assert_eq!(
        decode_tool_arguments("  {\"a\":1}  ").expect("outer"),
        json!({"a": 1})
    );
    assert_eq!(
        decode_tool_arguments("  \" {\\\"a\\\":1} \"  ").expect("inner"),
        json!({"a": 1})
    );
}

#[test]
fn invalid_json_reports_the_parse_error() {
    let err = decode_tool_arguments("{not json").expect_err("must fail");
    match &err {
        ToolArgDecodeError::InvalidJson { detail } => {
            assert!(!detail.is_empty(), "the parse error text is quoted");
        }
        other => panic!("expected InvalidJson, got {other:?}"),
    }
    // Display quotes the parse error so FR-008's corrective message can reuse it.
    assert!(
        err.to_string().contains("not valid JSON"),
        "display names the failure: {err}"
    );
}

#[test]
fn non_object_json_is_an_error_naming_what_was_found() {
    for (raw, kind) in [
        ("[1, 2]", "an array"),
        ("42", "a number"),
        ("true", "a boolean"),
        ("null", "null"),
    ] {
        match decode_tool_arguments(raw) {
            Err(ToolArgDecodeError::NotAnObject { found }) => assert_eq!(found, kind, "raw {raw}"),
            other => panic!("{raw} must be NotAnObject, got {other:?}"),
        }
    }
}

#[test]
fn encoded_string_holding_a_non_object_is_refused() {
    // A JSON string whose payload parses but is not an object.
    match decode_tool_arguments(r#""[1,2]""#) {
        Err(ToolArgDecodeError::EncodedStringNotAnObject { detail }) => {
            assert!(
                detail.contains("an array"),
                "detail names the kind: {detail}"
            );
        }
        other => panic!("expected EncodedStringNotAnObject, got {other:?}"),
    }
}

#[test]
fn encoded_string_holding_garbage_is_refused() {
    // A well-formed JSON string whose *payload* is not JSON at all.
    let raw = serde_json::to_string("{\"a\":").expect("encode the payload");
    match decode_tool_arguments(&raw) {
        Err(ToolArgDecodeError::EncodedStringNotAnObject { detail }) => {
            assert!(!detail.is_empty(), "the inner parse error is quoted");
        }
        other => panic!("expected EncodedStringNotAnObject, got {other:?}"),
    }
}

#[test]
fn decode_error_display_never_produces_empty_text() {
    // The corrective message embeds the Display form, so it must never be
    // blank for any refusal path.
    for raw in ["{not json", "[1]", "42", "null", r#""[1]""#, r#""oops"#] {
        let err = decode_tool_arguments(raw).expect_err("must fail");
        assert!(
            !err.to_string().trim().is_empty(),
            "raw {raw:?} produced blank display"
        );
    }
}
