//! Conservative recovery of tool calls from text-format model output.
//!
//! Some providers and models cannot emit native tool-call fields and instead
//! narrate the invocation as text (for example a bare JSON block or an XML
//! tool-call dialect). When the agent loop finishes a step with no native tool
//! calls, `extract_text_tool_calls` scans that text for the common markup
//! dialects and returns pending calls so the invocation is executed rather
//! than left as prose.
//!
//! The parser is deliberately narrow so ordinary prose is never mis-executed:
//! markup tags must be literally present, or the ENTIRE trimmed response must
//! itself be a tool-call JSON object/array. Anything else returns an empty
//! vector. Calls recovered here still pass through the normal dispatch
//! pipeline: loop restrictions, hooks, permission checks, and the registry.

use serde_json::Value;

use super::history::PendingToolCall;

/// Monotonic generator for synthetic tool-call ids of recovered calls.
static TEXT_CALL_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Start tag of the JSON-object tool-call dialect used by Qwen-style models.
const TOOL_CALL_OPEN: &str = "\u{e5b6}call\u{e5b6}";
/// End tag of the JSON-object tool-call dialect.
const TOOL_CALL_CLOSE: &str = "\u{e5b6}/call\u{e5b6}";
/// Start tag of the XML function-calls dialect.
const FUNCTION_CALLS_OPEN: &str = "<function_calls>";
/// End tag of the XML function-calls dialect.
const FUNCTION_CALLS_CLOSE: &str = "</function_calls>";

/// Extracts tool calls from assistant text that carries tool-call markup.
///
/// Recognised dialects, in order:
///
/// 1. private-tag blocks whose payload is a JSON object with `name` and
///    `arguments`/`input`/`parameters` keys (Qwen-style dialect),
/// 2. `function=name` blocks with `parameter=key` elements (XML dialect),
/// 3. when no markup is present and the whole trimmed response parses as a
///    JSON array of `{"name": ..., "arguments": ...}` objects (or a single
///    such object), the bare-JSON form.
///
/// Returns an empty vector when nothing recognisable is present.
///
/// Also reports the byte spans in `text` that produced the recovered calls so
/// callers can blank the recovered markup from the conversation history
/// (otherwise the model sees its own narration twice: once as text and once
/// as the tool_use part). Only markup blocks carry spans — the bare-JSON
/// dialect consumes the whole response, which callers replace wholesale if
/// they care.
pub fn extract_text_tool_calls_with_spans(
    text: &str,
) -> (Vec<PendingToolCall>, Vec<(usize, usize)>) {
    for (open_tag, close_tag) in [
        (TOOL_CALL_OPEN, TOOL_CALL_CLOSE),
        (FUNCTION_CALLS_OPEN, FUNCTION_CALLS_CLOSE),
    ] {
        let (calls, spans) = scan_blocks_with_spans(text, open_tag, close_tag);
        if !calls.is_empty() {
            return (calls, spans);
        }
    }
    let calls = extract_bare_json_calls(text);
    (calls, Vec::new())
}

/// Blanks the recovered byte spans in `buffer` with spaces, preserving every
/// byte offset and newline so downstream rendering and offsets stay valid.
pub fn blank_spans(buffer: &mut String, spans: &[(usize, usize)]) {
    if spans.is_empty() {
        return;
    }
    let mut bytes = buffer.as_bytes().to_vec();
    for (start, end) in spans {
        let (start, end) = (*start, *end);
        if start <= end
            && end <= bytes.len()
            && buffer.is_char_boundary(start)
            && buffer.is_char_boundary(end)
        {
            bytes[start..end].fill(b' ');
        }
    }
    if let Ok(replaced) = String::from_utf8(bytes) {
        *buffer = replaced;
    }
}

/// Span-aware markup scan: finds every `open_tag ... close_tag` block in
/// `text`, parses the bodies, and returns the calls plus the `(start, end)`
/// byte span of each complete block (tags included).
fn scan_blocks_with_spans(
    text: &str,
    open_tag: &str,
    close_tag: &str,
) -> (Vec<PendingToolCall>, Vec<(usize, usize)>) {
    let mut calls = Vec::new();
    let mut spans = Vec::new();
    let mut cursor = 0usize;
    while let Some(open_rel) = text[cursor..].find(open_tag) {
        let open_start = cursor + open_rel;
        let body_start = open_start + open_tag.len();
        let Some(close_rel) = text[body_start..].find(close_tag) else {
            break;
        };
        let block_end = body_start + close_rel + close_tag.len();
        let body = text[body_start..body_start + close_rel].trim();
        if let Some(call) = parse_markup_body(body) {
            calls.push(call);
            spans.push((open_start, block_end));
        }
        cursor = block_end;
    }
    (calls, spans)
}

/// Parses the inner payload of one markup block.
fn parse_markup_body(body: &str) -> Option<PendingToolCall> {
    // JSON-object dialect: the payload is a JSON object.
    if let Ok(parsed) = serde_json::from_str::<Value>(body)
        && let Some(call) = tool_call_from_object(&parsed)
    {
        return Some(call);
    }
    // XML dialect: `function=name` with `parameter` elements.
    parse_xml_tool_call(body)
}

/// Parses `function=name` plus `parameter` element markup.
fn parse_xml_tool_call(body: &str) -> Option<PendingToolCall> {
    let function_line = body
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with("<function="))?;
    let name = function_line
        .strip_prefix("<function=")?
        .strip_suffix('>')?
        .trim()
        .to_string();
    if name.is_empty() {
        return None;
    }
    let mut object = serde_json::Map::new();
    let mut rest = body;
    while let Some(param_start) = rest.find("<parameter=") {
        let after_tag = &rest[param_start + "<parameter=".len()..];
        let Some(tag_end) = after_tag.find('>') else {
            break;
        };
        let key = after_tag[..tag_end].trim().to_string();
        let value_start = tag_end + 1;
        let Some(close_rel) = after_tag[value_start..].find("</parameter>") else {
            break;
        };
        let value = after_tag[value_start..value_start + close_rel].trim();
        // JSON-typed parameter payloads (numbers, booleans, nested objects)
        // parse to their typed form; anything else stays a string so prose
        // values are never mis-typed. The registry's required-args validation
        // produces the corrective error when a typed payload is wrong.
        let typed = serde_json::from_str::<Value>(value)
            .unwrap_or_else(|_| Value::String(value.to_string()));
        object.insert(key, typed);
        rest = &after_tag[value_start + close_rel + "</parameter>".len()..];
    }
    // A zero-parameter invocation (e.g. `<function=list_files></function>`) is
    // still a call; the registry's required-args validation handles tools that
    // genuinely need parameters.
    Some(make_pending_call(&name, Value::Object(object)))
}

/// Parses the whole trimmed response as bare tool-call JSON.
fn extract_bare_json_calls(text: &str) -> Vec<PendingToolCall> {
    let trimmed = text.trim();
    if !(trimmed.starts_with('{') || trimmed.starts_with('[')) {
        return Vec::new();
    }
    let Ok(parsed) = serde_json::from_str::<Value>(trimmed) else {
        return Vec::new();
    };
    match parsed {
        Value::Array(items) => {
            if items.is_empty() {
                return Vec::new();
            }
            let calls: Vec<PendingToolCall> =
                items.iter().filter_map(tool_call_from_object).collect();
            if calls.len() != items.len() {
                // A mixed array (some items are not tool calls) is treated as
                // prose, not a partial invocation.
                return Vec::new();
            }
            calls
        }
        single => tool_call_from_object(&single).into_iter().collect(),
    }
}

/// Builds a `PendingToolCall` from a `{"name": ..., "arguments": ...}` object.
///
/// Returns `None` for objects without a usable `name`.
fn tool_call_from_object(value: &Value) -> Option<PendingToolCall> {
    let object = value.as_object()?;
    let name = object.get("name").and_then(Value::as_str)?.trim();
    if name.is_empty() {
        return None;
    }
    Some(make_pending_call(name, object_arguments(object)))
}

/// Extracts the arguments payload from a tool-call object, accepting the
/// common key aliases and both object and pre-serialised string forms.
fn object_arguments(object: &serde_json::Map<String, Value>) -> Value {
    for key in ["arguments", "input", "parameters"] {
        if let Some(args) = object.get(key) {
            return match args {
                Value::String(s) => Value::String(s.clone()),
                Value::Null => Value::Object(serde_json::Map::new()),
                other => other.clone(),
            };
        }
    }
    Value::Object(serde_json::Map::new())
}

/// Wraps `args` into the pending call's `args_json` string form.
fn make_pending_call(name: &str, args: Value) -> PendingToolCall {
    let args_json = match args {
        Value::String(s) => s,
        other => other.to_string(),
    };
    PendingToolCall {
        id: format!(
            "text-{}",
            TEXT_CALL_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ),
        name: name.to_string(),
        args_json,
    }
}
