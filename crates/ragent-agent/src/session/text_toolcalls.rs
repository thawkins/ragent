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
//!
//! The helpers are `pub(crate)` although only `processor` calls them; clippy's
//! nursery lint prefers `pub` inside a crate-private module, but the narrower
//! visibility documents the real call surface (mirrors `app/helpers.rs`).
#![allow(clippy::redundant_pub_crate)]

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
/// Start tag of the plain-ASCII tool-call dialect some models emit when
/// their pseudo-XML markup arrives decoded as literal text.
const TOOLCALL_OPEN: &str = "<toolcall>";
/// End tag of the plain-ASCII dialect in its usual closing form.
const TOOLCALL_CLOSE_SLASH: &str = "</toolcall>";
/// Bare end tag variant: the same close tag with the slash dropped. Some
/// decoding layers emit `<toolcall>` for the closing tag when the markup
/// arrives as plain text, so a name may sit between two identical tags.
const TOOLCALL_CLOSE_PLAIN: &str = "<toolcall>";
/// Start tag of the underscore-spelled tool-call dialect some chat templates
/// emit when their markup arrives decoded as literal text (Mistral/GLM/Kimi
/// style: `tool_call` with an underscore instead of the Qwen `call`).
const TOOL_CALL_UNDERSCORE_OPEN: &str = "<tool_call>";
/// End tag of the underscore-spelled dialect.
const TOOL_CALL_UNDERSCORE_CLOSE: &str = "<tool_call>";

/// Recognised (open, close) tag pairs, tried in order until one yields calls.
/// The final pair has identical open/close tags: some decoding layers emit
/// `<toolcall>` for the closing tag too (the slash is dropped when the
/// markup arrives as plain text), so a name may sit between two identical
/// tags.
const DIALECT_TAGS: [(&str, &str); 5] = [
    (TOOL_CALL_OPEN, TOOL_CALL_CLOSE),
    (FUNCTION_CALLS_OPEN, FUNCTION_CALLS_CLOSE),
    (TOOLCALL_OPEN, TOOLCALL_CLOSE_SLASH),
    (TOOLCALL_OPEN, TOOLCALL_CLOSE_PLAIN),
    (TOOL_CALL_UNDERSCORE_OPEN, TOOL_CALL_UNDERSCORE_CLOSE),
];

/// Extracts tool calls from assistant text that carries tool-call markup.
///
/// Recognised dialects, in order:
///
/// 1. private-tag blocks whose payload is a JSON object with `name` and
///    `arguments`/`input`/`parameters` keys (Qwen-style dialect),
/// 2. `function=name` blocks with `parameter=key` elements (XML dialect),
/// 3. plain-ASCII `<toolcall>` blocks carrying any of the above payloads, or
///    a bare tool name alone (the dialect that appears when a model's
///    pseudo-XML markup is decoded to literal text), or
/// 4. when no markup is present and the whole trimmed response parses as a
///    JSON array of `{"name": ..., "arguments": ...}` objects (or a single
///    such object), the bare-JSON form.
///
/// Returns an empty vector when nothing recognisable is present.
///
/// Also reports the byte spans in `text` that produced the recovered calls so
/// callers can blank the recovered markup from the conversation history
/// (otherwise the model sees its own narration twice: once as text and once
/// as the tool_use part). Only markup blocks carry spans - the bare-JSON
/// dialect consumes the whole response, which callers replace wholesale if
/// they care.
pub(crate) fn extract_text_tool_calls_with_spans(
    text: &str,
) -> (Vec<PendingToolCall>, Vec<(usize, usize)>) {
    for (open_tag, close_tag) in DIALECT_TAGS {
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
pub(crate) fn blank_spans(buffer: &mut String, spans: &[(usize, usize)]) {
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
///
/// A body that fails direct parsing gets one retry with a single balanced
/// backtick pair stripped: chat templates frequently quote the invocation in
/// a fenced code block and the raw fenced body alone never parses. On
/// success the call is recovered with the SAME span (tags included), so
/// blanking wipes the whole block including its fence.
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
        } else if let Some(fenced) = strip_balanced_backticks(body) {
            if let Some(call) = parse_markup_body(fenced) {
                calls.push(call);
                spans.push((open_start, block_end));
            }
        }
        cursor = block_end;
    }
    (calls, spans)
}

/// Strips ONE balanced backtick pair from around `body` when the body starts
/// and ends with a grave accent (optionally as a triple-backtick fence).
///
/// A fenced block may carry a short language tag (` ```json `): when the
/// post-fence content begins with a lone identifier word on its own line and
/// the call payload starts on the following line, the tag line is skipped
/// too. The accent pair must wrap the whole remaining content - any interior
/// structure stays untouched. Returns `None` when there is no balanced pair,
/// so non-fenced prose bodies never match.
fn strip_balanced_backticks(body: &str) -> Option<&str> {
    let trimmed = body.trim();
    let inner = if let Some(rest) = trimmed.strip_prefix("```") {
        let rest = rest.strip_suffix("```")?;
        // Drop a lone lowercase-ish language tag line ("json", "rust", ...).
        match rest.find('\n') {
            Some(first_line_end) => {
                let (tag, after) = rest.split_at(first_line_end);
                let is_tag = !tag.is_empty()
                    && tag.len() <= 16
                    && tag.chars().all(|c| c.is_ascii_alphanumeric());
                if is_tag && after[1..].trim_start().starts_with('{') {
                    &after[1..]
                } else {
                    rest
                }
            }
            None => rest,
        }
    } else {
        trimmed.strip_prefix('`')?.strip_suffix('`')?
    };
    let stripped = inner.trim();
    (stripped.len() < trimmed.len()).then_some(stripped)
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
    if let Some(call) = parse_xml_tool_call(body) {
        return Some(call);
    }
    // Bare tool name only (e.g. a decoded `<toolcall>Read</toolcall>`
    // invocation with no arguments). Conservative: the body must be a single
    // identifier token, so an argument-bearing or prose body never matches.
    bare_tool_name(body).map(|name| make_pending_call(&name, Value::Object(serde_json::Map::new())))
}

/// Parses `function=name` plus `parameter` element markup.
fn parse_xml_tool_call(body: &str) -> Option<PendingToolCall> {
    let function_line = body
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with("<function="))?;
    // Name ends at the FIRST '>' after the prefix - a zero-parameter block
    // (`<function=list_files></function>`) has the close tag on the same
    // line, so `strip_suffix('>')` would swallow it and corrupt the name
    // into "list_files></function".
    let after_prefix = function_line.strip_prefix("<function=")?;
    let name_end = after_prefix.find('>')?;
    let name = after_prefix[..name_end].trim().to_string();
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

/// Accepts a body consisting of exactly one identifier token (a Rust/JSON
/// name shape: letters, digits, underscores and hyphens, not starting with a
/// digit or hyphen). Everything else - prose, punctuation, paths - is None.
fn bare_tool_name(body: &str) -> Option<String> {
    let mut chars = body.chars();
    let first = chars.next()?;
    if !(first.is_ascii_alphabetic() || first == '_') {
        return None;
    }
    if !chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        return None;
    }
    Some(body.to_string())
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
