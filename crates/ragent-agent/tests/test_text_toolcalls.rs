//! External tests for the text tool-call recovery parser
//! (`session::text_toolcalls`), migrated from the module's inline `#[cfg(test)]`
//! block per the workspace test-organization rule.
//!
//! The module is included via the documented `#[path]` shim; shims for
//! `super::history` and `crate::` references are provided at the test root so
//! the source file compiles unchanged.

// Include the source module directly so its private helpers are reachable
// without widening production visibility.
#[path = "../src/session/text_toolcalls.rs"]
mod text_toolcalls;

// The source file's `use super::history::PendingToolCall;` resolves through
// this shim at the test root (the struct itself stays crate-private in
// production; the shim only satisfies the module's own import).
mod history {
    pub struct PendingToolCall {
        pub id: String,
        pub name: String,
        pub args_json: String,
    }
}

use serde_json::Value;

// The tag constants are private in the source module; the shim below makes
// the `use super::{...}` line resolve. Define mirror constants here for
// building fixtures.
const OPEN: &str = "\u{e5b6}call\u{e5b6}";
const CLOSE: &str = "\u{e5b6}/call\u{e5b6}";

use text_toolcalls::{blank_spans, extract_text_tool_calls_with_spans};

/// Convenience wrapper mirroring the common call shape.
fn extract_text_tool_calls(text: &str) -> Vec<history::PendingToolCall> {
    extract_text_tool_calls_with_spans(text).0
}

#[test]
fn test_extract_json_object_dialect() {
    let text =
        format!("{OPEN}\n{{\"name\": \"read\", \"arguments\": {{\"path\": \"a.rs\"}}}}\n{CLOSE}");
    let calls = extract_text_tool_calls(&text);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].name, "read");
    let args: Value = serde_json::from_str(&calls[0].args_json).unwrap();
    assert_eq!(args["path"], "a.rs");
}

#[test]
fn test_extract_xml_parameter_dialect() {
    let text = format!(
        "{OPEN}<function=read>\n<parameter=path>\nsrc/main.rs\n</parameter>\n</function>{CLOSE}"
    );
    let calls = extract_text_tool_calls(&text);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].name, "read");
    let args: Value = serde_json::from_str(&calls[0].args_json).unwrap();
    assert_eq!(args["path"], "src/main.rs");
}

#[test]
fn test_extract_xml_typed_parameter() {
    // A JSON-typed parameter payload parses to its typed form; the XML
    // dialect no longer forces every parameter to a string.
    let text = format!(
        "{OPEN}<function=write>\n<parameter=count>\n5\n</parameter>\n<parameter=flag>\ntrue\n</parameter>\n<parameter=note>\nplain prose\n</parameter>\n</function>{CLOSE}"
    );
    let calls = extract_text_tool_calls(&text);
    assert_eq!(calls.len(), 1);
    let args: Value = serde_json::from_str(&calls[0].args_json).unwrap();
    assert_eq!(args["count"], 5);
    assert_eq!(args["flag"], true);
    assert_eq!(args["note"], "plain prose");
}

#[test]
fn test_extract_xml_zero_parameter_call() {
    // A zero-parameter invocation is still a call; the registry's required-args
    // validation handles tools that genuinely need parameters.
    let text = format!("{OPEN}<function=list_files></function>{CLOSE}");
    let calls = extract_text_tool_calls(&text);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].name, "list_files");
    assert_eq!(calls[0].args_json, "{}");
}

#[test]
fn test_extract_bare_json_array() {
    let text = r#"[{"name": "bash", "arguments": {"command": "ls"}}, {"name": "think", "arguments": {"thought": "ok"}}]"#;
    let calls = extract_text_tool_calls(text);
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].name, "bash");
    assert_eq!(calls[1].name, "think");
}

#[test]
fn test_extract_bare_json_single_object() {
    let text = r#"{"name": "grep", "parameters": {"pattern": "fn main"}}"#;
    let calls = extract_text_tool_calls(text);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].name, "grep");
    let args: Value = serde_json::from_str(&calls[0].args_json).unwrap();
    assert_eq!(args["pattern"], "fn main");
}

#[test]
fn test_ordinary_prose_is_not_recovered() {
    assert!(extract_text_tool_calls("Here is how you might do it: read(path).").is_empty());
    assert!(extract_text_tool_calls("Use `{\"name\": \"read\"}` markup.").is_empty());
    // A JSON object without a tool-call shape is left alone.
    assert!(extract_text_tool_calls("{\"answer\": 42}").is_empty());
    assert!(extract_text_tool_calls("").is_empty());
}

#[test]
fn test_unparseable_markup_is_ignored() {
    let text = format!("{OPEN}\nnot json at all\n{CLOSE}");
    assert!(extract_text_tool_calls(&text).is_empty());
}

#[test]
fn test_recovered_ids_are_unique() {
    let text = r#"[{"name": "read", "arguments": {}}, {"name": "read", "arguments": {}}]"#;
    let calls = extract_text_tool_calls(text);
    assert_eq!(calls.len(), 2);
    assert_ne!(calls[0].id, calls[1].id);
}

#[test]
fn test_multibyte_content_survives_extraction() {
    let payload = "{\"name\": \"edit\", \"arguments\": {\"new_str\": \"caf\\u00e9 \\u2615 \\u2014 \\u00e9migr\\u00e9\"}}";
    let text = format!("{OPEN}{payload}{CLOSE}");
    let calls = extract_text_tool_calls(&text);
    assert_eq!(calls.len(), 1);
    let args: Value = serde_json::from_str(&calls[0].args_json).unwrap();
    assert_eq!(
        args["new_str"],
        "caf\u{e9} \u{2615} \u{2014} \u{e9}migr\u{e9}"
    );
}

#[test]
fn test_string_form_arguments_pass_through() {
    let text = r#"{"name": "bash", "arguments": "{\"command\": \"ls\"}"}"#;
    let calls = extract_text_tool_calls(text);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].args_json, r#"{"command": "ls"}"#);
}

#[test]
fn test_mixed_array_is_rejected() {
    let text = r#"[{"name": "read", "arguments": {}}, {"nope": true}]"#;
    assert!(extract_text_tool_calls(text).is_empty());
}

#[test]
fn test_non_call_json_is_not_a_call() {
    // A bare JSON scalar/array that is not a tool-call shape is prose.
    assert!(extract_text_tool_calls("[1, 2, 3]").is_empty());
    assert!(extract_text_tool_calls("null").is_empty());
    assert!(extract_text_tool_calls(r#"{"name": null}"#).is_empty());
}

#[test]
fn test_spans_cover_each_block_and_blanking_preserves_offsets() {
    let text = format!(
        "before {OPEN}{{\"name\": \"read\", \"arguments\": {{}}}}{CLOSE} middle {OPEN}<function=ls></function>{CLOSE} tail"
    );
    let (calls, spans) = extract_text_tool_calls_with_spans(&text);
    assert_eq!(calls.len(), 2);
    assert_eq!(spans.len(), 2);
    for (start, end) in &spans {
        assert!(text.is_char_boundary(*start) && text.is_char_boundary(*end));
        assert!(*start < *end && *end <= text.len());
        // Each span starts with the open tag and ends with the close tag.
        assert!(text[*start..].starts_with(OPEN));
        assert!(text[..*end].ends_with(CLOSE));
    }
    // Blanking removes the markup but keeps every byte offset valid.
    let mut buffer = text.clone();
    blank_spans(&mut buffer, &spans);
    assert_eq!(buffer.len(), text.len());
    for (start, end) in &spans {
        assert!(buffer[*start..*end].chars().all(|c| c == ' '));
    }
    assert!(extract_text_tool_calls(&buffer).is_empty());
}

// ---------------------------------------------------------------------------
// Plain-ASCII `<toolcall>` dialect
// ---------------------------------------------------------------------------

const ASCII_OPEN: &str = "<toolcall>";
const ASCII_CLOSE: &str = "</toolcall>";
const ASCII_PLAIN: &str = "<toolcall>";

#[test]
fn test_ascii_dialect_name_only() {
    // The exact user-reported shape: the markup decoded to literal text and
    // the invocation carries no arguments.
    let text = format!("{ASCII_OPEN}Read{ASCII_CLOSE}");
    let calls = extract_text_tool_calls(&text);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].name, "Read");
    assert_eq!(calls[0].args_json, "{}");
}

#[test]
fn test_ascii_dialect_plain_close_tag() {
    // The closing form with the slash dropped: two identical tags.
    let text = format!("{ASCII_OPEN}Read{ASCII_PLAIN}");
    let calls = extract_text_tool_calls(&text);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].name, "Read");
}

#[test]
fn test_ascii_dialect_json_payload() {
    let text = format!(
        "{ASCII_OPEN}\n{{\"name\": \"read_file\", \"arguments\": {{\"path\": \"src/x.rs\"}}}}\n{ASCII_CLOSE}"
    );
    let calls = extract_text_tool_calls(&text);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].name, "read_file");
    let args: Value = serde_json::from_str(&calls[0].args_json).unwrap();
    assert_eq!(args["path"], "src/x.rs");
}

#[test]
fn test_ascii_dialect_xml_parameter_payload() {
    let text = format!(
        "{ASCII_OPEN}<function=Bash>\n<parameter=command>\ncargo test\n</parameter>\n</function>{ASCII_CLOSE}"
    );
    let calls = extract_text_tool_calls(&text);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].name, "Bash");
    let args: Value = serde_json::from_str(&calls[0].args_json).unwrap();
    assert_eq!(args["command"], "cargo test");
}

#[test]
fn test_ascii_dialect_multiple_calls_and_spans() {
    let text = format!("A{ASCII_OPEN}read{ASCII_CLOSE} B{ASCII_OPEN}write{ASCII_CLOSE} C");
    let (calls, spans) = extract_text_tool_calls_with_spans(&text);
    assert_eq!(calls.len(), 2);
    assert_eq!(spans.len(), 2);
    assert!(text[spans[0].0..].starts_with(ASCII_OPEN));
    assert!(text[..spans[0].1].ends_with(ASCII_CLOSE));
    // Blanking wipes both blocks and no second pass recovers anything.
    let mut buffer = text.clone();
    blank_spans(&mut buffer, &spans);
    assert!(extract_text_tool_calls(&buffer).is_empty());
}

#[test]
fn test_ascii_dialect_prose_body_is_not_recovered() {
    // A prose body between ASCII toolcall tags must not become a call.
    let text = format!("{ASCII_OPEN}Use the read tool to inspect files{ASCII_CLOSE}");
    assert!(extract_text_tool_calls(&text).is_empty());
    // An unterminated open tag is left alone.
    assert!(extract_text_tool_calls("text <toolcall> no close").is_empty());
}

#[test]
fn test_ascii_dialect_bare_name_must_be_identifier() {
    // Non-identifier bodies stay unrecovered (paths, punctuation, digits).
    let text = format!("{ASCII_OPEN}src/main.rs{ASCII_CLOSE}");
    assert!(extract_text_tool_calls(&text).is_empty());
    let text = format!("{ASCII_OPEN}9lives{ASCII_CLOSE}");
    assert!(extract_text_tool_calls(&text).is_empty());
    let text = format!("{ASCII_OPEN}-{ASCII_CLOSE}");
    assert!(extract_text_tool_calls(&text).is_empty());
}
// ---------------------------------------------------------------------------
// Underscore-spelled `tool_call` dialect (Mistral/GLM/Kimi style markup)
// ---------------------------------------------------------------------------

const UNDERSCORE_OPEN: &str = "<tool_call>";
const UNDERSCORE_CLOSE: &str = "<tool_call>";

#[test]
fn test_underscore_dialect_json_payload() {
    let text = format!(
        "{UNDERSCORE_OPEN}\n{{\"name\": \"read\", \"arguments\": {{\"path\": \"src/lib.rs\"}}}}\n{UNDERSCORE_CLOSE}"
    );
    let calls = extract_text_tool_calls(&text);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].name, "read");
    let args: Value = serde_json::from_str(&calls[0].args_json).unwrap();
    assert_eq!(args["path"], "src/lib.rs");
}

#[test]
fn test_underscore_dialect_xml_payload() {
    let text = format!(
        "{UNDERSCORE_OPEN}<function=Bash>\n<parameter=command>\ncargo build\n</parameter>\n</function>{UNDERSCORE_CLOSE}"
    );
    let calls = extract_text_tool_calls(&text);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].name, "Bash");
}

#[test]
fn test_underscore_dialect_multiple_and_spans() {
    let text =
        format!("{UNDERSCORE_OPEN}read{UNDERSCORE_CLOSE} {UNDERSCORE_OPEN}write{UNDERSCORE_CLOSE}");
    let (calls, spans) = extract_text_tool_calls_with_spans(&text);
    assert_eq!(calls.len(), 2);
    assert_eq!(spans.len(), 2);
    let mut buffer = text.clone();
    blank_spans(&mut buffer, &spans);
    assert!(extract_text_tool_calls(&buffer).is_empty());
}

// ---------------------------------------------------------------------------
// Backtick-fenced JSON bodies (a trailing fence defeats the raw parse)
// ---------------------------------------------------------------------------

#[test]
fn test_json_body_wrapped_in_backticks() {
    let text = format!(
        "{ASCII_OPEN}`{{\"name\": \"Grep\", \"arguments\": {{\"pattern\": \"fn main\"}}}}`{ASCII_CLOSE}"
    );
    let calls = extract_text_tool_calls(&text);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].name, "Grep");
    let args: Value = serde_json::from_str(&calls[0].args_json).unwrap();
    assert_eq!(args["pattern"], "fn main");
}

#[test]
fn test_json_body_wrapped_in_fenced_code_block() {
    let text = format!(
        "{ASCII_OPEN}\n```json\n{{\"name\": \"read\", \"arguments\": {{}}}}\n```\n{ASCII_CLOSE}"
    );
    let calls = extract_text_tool_calls(&text);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].name, "read");
}

#[test]
fn test_unbalanced_backtick_body_is_not_recovered() {
    // One-sided fences stay unrecovered - conservative behaviour.
    let text = format!("{ASCII_OPEN}`{{\"name\": \"read\"}}{ASCII_CLOSE}");
    assert!(extract_text_tool_calls(&text).is_empty());
    let text = format!("`not a call`");
    assert!(extract_text_tool_calls(&text).is_empty());
}

#[test]
fn test_fenced_span_covers_the_whole_block() {
    let text =
        format!("{ASCII_OPEN}`{{\"name\": \"read\", \"arguments\": {{}}}}`{ASCII_CLOSE} tail");
    let (calls, spans) = extract_text_tool_calls_with_spans(&text);
    assert_eq!(calls.len(), 1);
    assert_eq!(spans.len(), 1);
    assert!(text[spans[0].0..].starts_with(ASCII_OPEN));
    assert!(text[..spans[0].1].ends_with(ASCII_CLOSE));
    let mut buffer = text.clone();
    blank_spans(&mut buffer, &spans);
    assert!(extract_text_tool_calls(&buffer).is_empty());
}
