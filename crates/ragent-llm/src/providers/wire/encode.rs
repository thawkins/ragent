//! The one encoder every provider family shares: tool *definitions* (FR-003) and
//! the tool *constructs* inside a message (FR-006).
//!
//! ## Definitions
//!
//! A provider **shall not** construct its own tool array, so the serialised
//! shape is derived here from the family's
//! [`ToolWireProfile::tool_envelope`] rather than from a per-provider builder.
//! The five envelopes the tree speaks - `OpenAiFunction`, `AnthropicTool`,
//! `GeminiFunctionDeclaration`, `BedrockToolSpec` and `ResponsesFlatFunction` -
//! are written in exactly one `match`, so an envelope has one definition in the
//! whole workspace:
//!
//! - `OpenAiFunction` - `{"type":"function","function":{name,description,parameters}}`
//! - `AnthropicTool` - `{"name","description","input_schema"}`
//! - `GeminiFunctionDeclaration` - one entry of `tools[].functionDeclarations[]`
//! - `BedrockToolSpec` - `{"toolSpec":{name,description,inputSchema:{json:...}}}`
//! - `ResponsesFlatFunction` - `{"type":"function","name","description","parameters"}`
//!
//! Every family then wraps the list in `{"tools":[...]}`. The five differ in the
//! *item* shape and, for Bedrock, in which slice of the buffer the caller
//! assigns: the Converse `toolConfig` value is the whole `{"tools":[...]}`
//! object, while the other four assign the inner array to a `tools` field. That
//! difference is one `match` on the envelope, not five builders.
//!
//! The one transformation that is deliberately *not* part of the contract is the
//! HuggingFace router's `t_` tool-name prefix: the profile table records it as a
//! name rewrite *inside* the OpenAI envelope rather than a different envelope, so
//! it is passed in as `name_prefix` and applied to the name alone. This keeps one
//! meaning per provider family (FR-002) without a second envelope constant that
//! differs only in a string.
//!
//! The encoder is a pure function over `&[ToolDefinition]` (NFR-004): it reads no
//! environment, holds no lock and contacts nothing, which is what lets the cache
//! in `tool_cache.rs` store its output byte-for-byte and serve a hit identical to a
//! miss (FR-003).

use std::borrow::Cow;

use ragent_types::{ChatContent, ChatMessage, ContentPart, ToolDefinition};

use super::{ArgsEncoding, IdField, ResultCarrier, ToolEnvelope, ToolWireProfile};

/// The wrapper object every family emits its tool list under.
///
/// It is the same for all five envelopes; only Bedrock's caller keeps the wrapper
/// (it assigns the object to `toolConfig`) while the other four assign the inner
/// array (`WRAPPER_PREFIX.len() - 1` is that array's opening `[`).
const WRAPPER_PREFIX: &[u8] = br#"{"tools":["#;

/// The HuggingFace router's tool-name prefix (`t_`), applied inside the OpenAI
/// envelope rather than by a sixth envelope constant.
///
/// The router rejects tool names containing common substrings (`read`, `Write`,
/// `search`, `list`, `open`, `memo`, `pdf`, `todo`, ...) in streaming mode, so every
/// name is prefixed on the way out and stripped on the way back in
/// (`huggingface.rs` owns the strip, since it is a *response* rewrite).
pub const HF_TOOL_PREFIX: &str = "t_";

/// A first-pass capacity per tool, so the common case does not reallocate.
///
/// This is a hint only: the serialised length varies with each definition's
/// description and parameter schema, and `Vec` grows to fit. It is a constant
/// rather than a per-envelope number because a reallocation on the first encode
/// of a tool set is invisible next to the cache hit that follows it (NFR-001).
const ITEM_BYTES: usize = 128;

/// The encoded tool list plus the slice of it the caller assigns to its body.
///
/// The bytes are produced by [`encode_tools`] and cached by
/// `crate::provider::tool_cache::cached_tools`; a hit returns the same `Arc`, so a
/// caller may read the payload on every turn without re-serialising the
/// definitions.
pub struct CachedTools {
    /// The full serialised JSON `Vec` that the caller can `.extend_from_slice`
    /// into a pre-sized request-body buffer, or pass to `RequestBuilder::body`.
    pub bytes: Vec<u8>,
    /// The pre-serialised byte length of the tool-list fragment, used by
    /// request-size estimation to avoid re-serialising the definitions.
    pub byte_len: usize,
    /// Byte offset of the inner tool list/array that should be assigned to the
    /// request body (inclusive).
    payload_start: usize,
    /// Byte offset immediately after the inner tool list/array (exclusive).
    payload_end: usize,
}

impl CachedTools {
    /// Parse the payload slice (the bytes between `payload_start` and
    /// `payload_end`) back into a [`serde_json::Value`].
    ///
    /// The bytes were produced by `serde_json::to_writer` in [`encode_tools`], so a
    /// parse failure can only mean data corruption; the fallback returns the
    /// empty-tolerant `Value::Null` rather than panicking mid-request (the caller
    /// treats a missing tool list as "no tools").
    fn payload_value(&self) -> serde_json::Value {
        serde_json::from_slice(&self.bytes[self.payload_start..self.payload_end])
            .unwrap_or(serde_json::Value::Null)
    }

    /// Extract the inner tool-list array for OpenAI-compatible providers.
    ///
    /// The encoded wrapper is `{"tools":[...]}`, so the payload is the `[...]`
    /// array.
    pub fn openai_tools_array(&self) -> serde_json::Value {
        self.payload_value()
    }

    /// Extract the inner tool-list array for the Responses API.
    ///
    /// The encoded wrapper is `{"tools":[...]}` and each item is a flattened
    /// function object, so the payload is the `[...]` array - the same slice as
    /// [`Self::openai_tools_array`], named for the shape that asked for it. The
    /// two item *shapes* differ; the slice the caller assigns does not, and only
    /// the encoder decides which offsets those are.
    pub fn responses_tools_array(&self) -> serde_json::Value {
        self.payload_value()
    }

    /// Extract the inner tool-list array for Anthropic-format providers.
    ///
    /// The encoded wrapper is `{"tools":[...]}` with `input_schema` items, so the
    /// payload is the `[...]` array.
    pub fn anthropic_tools_array(&self) -> serde_json::Value {
        self.payload_value()
    }

    /// Extract the Gemini `tools` array (`[{"functionDeclarations":[...]}]`).
    ///
    /// The encoded wrapper is `{"tools":[{"functionDeclarations":[...]}]}`, so the
    /// payload is the `[{"functionDeclarations":[...]}]` array.
    pub fn gemini_tools_array(&self) -> serde_json::Value {
        self.payload_value()
    }

    /// Extract the Bedrock Converse `toolConfig` field value (`{"tools": [...]}`).
    ///
    /// The Bedrock payload is the whole `{"tools":[...]}` object - each item a
    /// `toolSpec` - because the caller assigns it directly to `body["toolConfig"]`.
    pub fn bedrock_tool_config_object(&self) -> serde_json::Value {
        self.payload_value()
    }
}

/// Serialise `tools` into the tool-list bytes `profile` puts on the wire (FR-003).
///
/// The envelope comes from [`ToolWireProfile::tool_envelope`]; `name_prefix` is the
/// one name rewrite a family applies inside that envelope (HuggingFace's `t_`), so
/// pass `None` for every other family. The result is byte-identical on every call
/// with the same arguments, which is what makes the cache's hit equal to its miss.
///
/// # Examples
///
/// ```
/// use ragent_llm::provider::wire::encode::encode_tools;
/// use ragent_llm::provider::wire::profile_table::{ANTHROPIC_MESSAGES, OPENAI_CHAT};
/// use ragent_types::ToolDefinition;
///
/// let tools = [ToolDefinition {
///     name: "Read".into(),
///     description: "Read a file".into(),
///     parameters: serde_json::json!({"type": "object", "properties": {}}),
/// }];
///
/// let openai = encode_tools(OPENAI_CHAT, None, &tools).openai_tools_array();
/// assert_eq!(openai[0]["function"]["name"], "Read");
///
/// let anthropic = encode_tools(ANTHROPIC_MESSAGES, None, &tools).anthropic_tools_array();
/// assert_eq!(anthropic[0]["name"], "Read");
/// assert!(anthropic[0].get("input_schema").is_some());
/// ```
#[must_use]
pub fn encode_tools(
    profile: ToolWireProfile,
    name_prefix: Option<&str>,
    tools: &[ToolDefinition],
) -> CachedTools {
    let mut buf = Vec::with_capacity(tools.len() * ITEM_BYTES);
    buf.extend_from_slice(WRAPPER_PREFIX);
    let array_start = buf.len() - 1; // index of the opening '['
    for (index, tool) in tools.iter().enumerate() {
        let name: Cow<'_, str> = match name_prefix {
            Some(prefix) => Cow::Owned(format!("{prefix}{}", tool.name)),
            None => Cow::Borrowed(tool.name.as_str()),
        };
        push_tool(
            &mut buf,
            index,
            &item_json(profile.tool_envelope, tool, &name),
        );
    }
    buf.push(b']');
    buf.push(b'}');
    // Bedrock assigns the whole `{"tools":[...]}` object to `toolConfig`; the other
    // four assign the inner array, which ends just before the closing '}'.
    let (payload_start, payload_end) = match profile.tool_envelope {
        ToolEnvelope::BedrockToolSpec => (0, buf.len()),
        ToolEnvelope::OpenAiFunction
        | ToolEnvelope::AnthropicTool
        | ToolEnvelope::GeminiFunctionDeclaration
        | ToolEnvelope::ResponsesFlatFunction => (array_start, buf.len() - 1),
    };
    let byte_len = payload_end - payload_start;
    CachedTools {
        bytes: buf,
        byte_len,
        payload_start,
        payload_end,
    }
}

/// Build the JSON object one envelope wraps a single definition in.
///
/// `name` is the *wire* name - already carrying the family's prefix when it has
/// one - so the envelope arms never concatenate a name themselves.
///
/// The object's key order is irrelevant to the output bytes: `serde_json`'s default
/// map is sorted, so every envelope serialises identically however its literal is
/// written. The fields are listed in wire order for readability only.
fn item_json(envelope: ToolEnvelope, tool: &ToolDefinition, name: &str) -> serde_json::Value {
    match envelope {
        ToolEnvelope::OpenAiFunction => serde_json::json!({
            "type": "function",
            "function": {
                "name": name,
                "description": tool.description,
                "parameters": tool.parameters
            }
        }),
        ToolEnvelope::AnthropicTool => serde_json::json!({
            "name": name,
            "description": tool.description,
            "input_schema": tool.parameters
        }),
        ToolEnvelope::GeminiFunctionDeclaration => serde_json::json!({
            "functionDeclarations": [{
                "name": name,
                "description": tool.description,
                "parameters": tool.parameters
            }]
        }),
        ToolEnvelope::BedrockToolSpec => serde_json::json!({
            "toolSpec": {
                "name": name,
                "description": tool.description,
                "inputSchema": {
                    "json": tool.parameters
                }
            }
        }),
        ToolEnvelope::ResponsesFlatFunction => serde_json::json!({
            "type": "function",
            "name": name,
            "description": tool.description,
            "parameters": tool.parameters
        }),
    }
}

/// Append one serialised tool object to `buf`, inserting the separating comma for
/// every entry after the first.
///
/// A `serde_json::Value` built from `ToolDefinition` (strings, numbers, and nested
/// schema objects) always serialises; if serialisation ever failed the writer would
/// have left `item` empty, and an empty tool object makes the whole request body
/// invalid JSON. Rather than panic mid-request, an empty serialisation is logged and
/// the entry skipped (the provider then sees one fewer tool rather than a corrupt
/// body).
fn push_tool(buf: &mut Vec<u8>, index: usize, tool: &serde_json::Value) {
    if index > 0 {
        buf.push(b',');
    }
    let mut item = Vec::with_capacity(ITEM_BYTES);
    match serde_json::to_writer(&mut item, tool) {
        Ok(()) => buf.extend_from_slice(&item),
        Err(e) => {
            tracing::warn!("tool-definition serialisation failed, skipping entry: {e}");
        }
    }
}

// ---------------------------------------------------------------------------
// Messages: the four result carriers behind one encoder (FR-006)
// ---------------------------------------------------------------------------

/// Names the tool a result answers, keyed by the call id it answers.
///
/// Two families identify a tool result by *naming* the tool rather than (or as
/// well as) keying it by id, and neither provider returns the name on the result
/// frame, so the name has to be recovered from the assistant turn that opened
/// the call. [`tool_names_by_id`] is the one definition of that scan.
pub type ToolNames = std::collections::HashMap<String, String>;

/// Build the id-to-name map a carrier needs from `messages`.
///
/// Only the families that name a tool on the wire need this
/// (`GeminiFunctionResponse`, native Ollama's `role:"tool"` message), and only
/// they build it: a scan over the whole conversation is cheap but not free, so a
/// family that never reads it does not pay for it.
#[must_use]
pub fn tool_names_by_id(messages: &[ChatMessage]) -> ToolNames {
    let mut map = ToolNames::new();
    for msg in messages {
        if let ChatContent::Parts(parts) = &msg.content {
            for part in parts {
                if let ContentPart::ToolUse { id, name, .. } = part {
                    map.insert(id.clone(), name.clone());
                }
            }
        }
    }
    map
}

/// The caller-supplied facts one [`encode_message`] call needs besides the
/// message itself.
///
/// Everything here is *per call* or *per turn*, never a wire rule: the wire rule
/// is the profile. Four families have no extra facts at all and pass
/// `MessageContext::default()`.
#[derive(Debug, Clone, Copy)]
pub struct MessageContext<'a> {
    /// The id-to-name map, for the families that name a tool on a result.
    ///
    /// `None` means "this caller's family never names a tool", so the encoder
    /// will not look one up.
    pub tool_names: Option<&'a ToolNames>,
    /// A name rewrite applied inside the family's envelope (HuggingFace's `t_`).
    ///
    /// The same rewrite [`encode_tools`] takes, applied to the tool name inside
    /// an assistant tool call so the request and the history agree.
    pub name_prefix: Option<&'a str>,
    /// Whether *this* message may carry images.
    ///
    /// Native Ollama accepts an `images` array only on the most recent user
    /// message; every other family always carries them, so the default is
    /// `true` and only that one caller lowers it.
    pub allow_images: bool,
    /// Mirror the tool name beside the call id on a `role:"tool"` result.
    ///
    /// Local Ollama over `/v1/chat/completions` asks for both the OpenAI
    /// `tool_call_id` and the native `tool_name`; no profile field records that
    /// request, so the caller states it. Inert for every other family.
    pub mirror_tool_name: bool,
}

/// The textual marker a wire shape without a native error flag carries
/// instead (FR-014).
///
/// Every family whose profile declares `supports_is_error == false` writes
/// this prefix in front of an error result's content, so an error result is
/// never indistinguishable from a successful one. The constant is exported so
/// the providers that still serialise their own message loop (their own
/// carrier, and so no error-flag knowledge of their own) mark the result with
/// the same six bytes the shared encoder uses.
pub const ERROR_PREFIX: &str = "ERROR: ";

/// A tool result's content after FR-014 marking for a profile that declares
/// `supports_is_error == false`: the `ERROR: ` prefix on an error (the content
/// is otherwise unchanged) and the content alone otherwise. Providers that
/// send the flag natively - the Anthropic Messages and Bedrock Converse
/// shapes - must not call this; their native flag is the marker.
#[must_use]
pub fn error_marked(content: &str, is_error: bool) -> String {
    if is_error {
        format!("{ERROR_PREFIX}{content}")
    } else {
        content.to_string()
    }
}

impl Default for MessageContext<'_> {
    fn default() -> Self {
        Self {
            tool_names: None,
            name_prefix: None,
            allow_images: true,
            mirror_tool_name: false,
        }
    }
}

/// The message vocabulary one wire family speaks.
///
/// It is *derived* from a profile ([`MessageFamily::of`]) rather than passed
/// beside it, so a caller cannot name a vocabulary that disagrees with its
/// declared contract. Four families share the four carriers of FR-006; the two
/// extra entries exist because the same carrier is written with a different part
/// shape in two families (`UserToolResultBlock` is Anthropic's `tool_result` and
/// Converse's `toolResult`, and `ToolRoleMessage` is OpenAI's id-keyed message
/// and native Ollama's name-keyed one).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MessageFamily {
    /// `role`/`content` messages, `tool_calls` on an assistant turn, one
    /// `role:"tool"` message per result.
    OpenAiChat,
    /// Native Ollama `/api/chat`: string content, `images` separately,
    /// `tool_calls` with object arguments and no id, `role:"tool"` by name.
    OllamaNative,
    /// Anthropic `role`/`content` blocks: `tool_use` and `tool_result`.
    AnthropicMessages,
    /// Bedrock Converse `role`/`content` blocks: `toolUse` and `toolResult`.
    BedrockConverse,
    /// Gemini `contents[].parts[]`: `functionCall` and `functionResponse`.
    GeminiContents,
    /// Responses API input items: `function_call` and `function_call_output`.
    ResponsesInput,
}

impl MessageFamily {
    /// Resolve the vocabulary a declared profile speaks.
    ///
    /// `None` means a profile was declared with a combination of envelope,
    /// carrier and argument encoding that no family implements - a defect in the
    /// profile table, which `tests/test_wire_encode_message.rs` catches by
    /// encoding a message with every declared profile.
    fn of(profile: ToolWireProfile) -> Option<Self> {
        match (
            profile.tool_envelope,
            profile.result_carrier,
            profile.args_encoding,
        ) {
            (
                ToolEnvelope::OpenAiFunction,
                ResultCarrier::ToolRoleMessage,
                ArgsEncoding::JsonString,
            ) => Some(Self::OpenAiChat),
            (
                ToolEnvelope::OpenAiFunction,
                ResultCarrier::ToolRoleMessage,
                ArgsEncoding::JsonObject,
            ) => Some(Self::OllamaNative),
            (ToolEnvelope::AnthropicTool, ResultCarrier::UserToolResultBlock, _) => {
                Some(Self::AnthropicMessages)
            }
            (ToolEnvelope::BedrockToolSpec, ResultCarrier::UserToolResultBlock, _) => {
                Some(Self::BedrockConverse)
            }
            (ToolEnvelope::GeminiFunctionDeclaration, ResultCarrier::GeminiFunctionResponse, _) => {
                Some(Self::GeminiContents)
            }
            (
                ToolEnvelope::ResponsesFlatFunction,
                ResultCarrier::ResponsesFunctionCallOutput,
                _,
            ) => Some(Self::ResponsesInput),
            _ => None,
        }
    }

    /// The carrier this vocabulary writes, so a debug build can prove the
    /// derivation agrees with the profile the caller declared.
    const fn result_carrier(self) -> ResultCarrier {
        match self {
            Self::OpenAiChat | Self::OllamaNative => ResultCarrier::ToolRoleMessage,
            Self::AnthropicMessages | Self::BedrockConverse => ResultCarrier::UserToolResultBlock,
            Self::GeminiContents => ResultCarrier::GeminiFunctionResponse,
            Self::ResponsesInput => ResultCarrier::ResponsesFunctionCallOutput,
        }
    }
}

/// Encode one canonical message into the entries `profile`'s family puts on the
/// wire (FR-006).
///
/// One message expands to zero, one, or several entries:
///
/// - zero when the family has no slot for the message at all (Converse accepts
///   only `user` and `assistant` turns, so a `system` or `tool` message is
///   dropped);
/// - one for every family except the two that unpack results and the Responses
///   input list;
/// - one per tool result for the OpenAI family, which carries each result as its
///   own `role:"tool"` message, and one item per tool construct for the Responses
///   input list.
///
/// The result carrier comes from [`ToolWireProfile::result_carrier`], the id key
/// from [`ToolWireProfile::id_field`] and the argument encoding from
/// [`ToolWireProfile::args_encoding`], so the four carriers of FR-006 - and the
/// id inside each - are written here once rather than in seven provider modules.
///
/// Two constructs are *not* carried yet, and both are recorded rather than
/// silently handled: an image part on the Responses input path, and a signed
/// reasoning part for a profile that declares `echo_reasoning`. FR-018's
/// `WireLoss` refusal is T-007 and reasoning replay is T-018.
///
/// # Examples
///
/// ```
/// use ragent_llm::llm::{ChatContent, ChatMessage, ContentPart};
/// use ragent_llm::provider::wire::encode::{MessageContext, encode_message};
/// use ragent_llm::provider::wire::profile_table::{ANTHROPIC_MESSAGES, OPENAI_CHAT};
///
/// let result = ChatMessage {
///     role: "tool".to_string(),
///     content: ChatContent::Parts(vec![ContentPart::ToolResult {
///         tool_use_id: "call_1".to_string(),
///         content: "42".into(),
///         is_error: false,
///     }]),
/// };
/// let ctx = MessageContext::default();
///
/// // The OpenAI family keys the result by `tool_call_id` on its own message.
/// let openai = encode_message(OPENAI_CHAT, &result, &ctx);
/// assert_eq!(openai[0]["tool_call_id"], "call_1");
///
/// // The Anthropic family wraps it in a `tool_result` block in a user turn.
/// let anthropic = encode_message(ANTHROPIC_MESSAGES, &result, &ctx);
/// assert_eq!(anthropic[0]["content"][0]["type"], "tool_result");
/// ```
#[must_use]
pub fn encode_message(
    profile: ToolWireProfile,
    msg: &ChatMessage,
    ctx: &MessageContext<'_>,
) -> Vec<serde_json::Value> {
    let Some(family) = MessageFamily::of(profile) else {
        // A declared profile with no message vocabulary is a build defect, not a
        // request condition: the coverage test fails the build for it. In a
        // release build the message is omitted with an error rather than
        // panicking mid-request.
        tracing::error!(
            envelope = profile.tool_envelope.as_str(),
            carrier = profile.result_carrier.as_str(),
            "no message vocabulary for this wire profile; omitting the message"
        );
        return Vec::new();
    };
    debug_assert_eq!(
        family.result_carrier(),
        profile.result_carrier,
        "the message family derived from a profile must write that profile's carrier"
    );
    match family {
        MessageFamily::OpenAiChat => openai_chat_message(profile, msg, ctx),
        MessageFamily::OllamaNative => ollama_native_message(profile, msg, ctx),
        MessageFamily::AnthropicMessages => anthropic_message(msg, ctx),
        MessageFamily::BedrockConverse => converse_message(msg, ctx),
        MessageFamily::GeminiContents => gemini_message(profile, msg, ctx),
        MessageFamily::ResponsesInput => responses_items(profile, msg, ctx),
    }
}

/// The family's name rewrite, applied wherever a tool name is written.
fn wire_name<'n>(ctx: &MessageContext<'_>, name: &'n str) -> Cow<'n, str> {
    match ctx.name_prefix {
        Some(prefix) => Cow::Owned(format!("{prefix}{name}")),
        None => Cow::Borrowed(name),
    }
}

/// Render a tool call's arguments in the encoding the profile declares.
fn args_json(encoding: ArgsEncoding, input: &serde_json::Value) -> serde_json::Value {
    match encoding {
        ArgsEncoding::JsonString => serde_json::Value::String(input.to_string()),
        ArgsEncoding::JsonObject => input.clone(),
    }
}

/// A tool result's content with the `ERROR: ` prefix applied when it would
/// otherwise be lost (FR-014).
///
/// Returns `false` for the second tuple element when the prefix was *not*
/// needed (the profile transmits the flag natively, or the result is not an
/// error), so a carrier can decide whether to also write its native flag
/// without re-checking the profile.
fn mark_error(profile: ToolWireProfile, content: &str, is_error: bool) -> (String, bool) {
    match (profile.supports_is_error, is_error) {
        // The flag is on the wire already; the content is the result alone.
        (true, _) | (_, false) => (content.to_string(), profile.supports_is_error),
        // No native flag on this shape: the text marker carries it instead.
        (false, true) => (format!("{ERROR_PREFIX}{content}"), false),
    }
}

/// The `content` field of an ordinary OpenAI-family message.
///
/// A single text part collapses to a bare string, which is what every
/// OpenAI-compatible endpoint accepts most readily. The collapse is deliberately
/// narrow: only a *text* part becomes a string, because collapsing anything else
/// would write `"content": null` and lose the part (the two provider copies this
/// replaced disagreed on exactly that case, one of them producing the null).
fn openai_content(parts: &[ContentPart]) -> serde_json::Value {
    let rendered: Vec<serde_json::Value> = parts
        .iter()
        .filter_map(|part| match part {
            ContentPart::Text { text } => Some(serde_json::json!({
                "type": "text",
                "text": text
            })),
            ContentPart::ImageUrl { url } => Some(serde_json::json!({
                "type": "image_url",
                "image_url": { "url": url }
            })),
            ContentPart::ToolUse { .. }
            | ContentPart::ToolResult { .. }
            | ContentPart::Reasoning { .. } => None,
        })
        .collect();
    match rendered.as_slice() {
        [] => serde_json::json!([]),
        [only] if only["type"] == "text" => only["text"].clone(),
        _ => serde_json::json!(rendered),
    }
}

/// OpenAI chat-completions messages: `tool_calls` on an assistant turn, then one
/// `role:"tool"` message per result.
fn openai_chat_message(
    profile: ToolWireProfile,
    msg: &ChatMessage,
    ctx: &MessageContext<'_>,
) -> Vec<serde_json::Value> {
    let ChatContent::Parts(parts) = &msg.content else {
        return vec![serde_json::json!({
            "role": msg.role,
            "content": match &msg.content {
                ChatContent::Text(text) => serde_json::json!(text),
                ChatContent::Parts(_) => serde_json::json!([]),
            },
        })];
    };
    let tool_calls: Vec<serde_json::Value> = parts
        .iter()
        .filter_map(|part| match part {
            ContentPart::ToolUse { id, name, input } => Some(serde_json::json!({
                "id": id,
                "type": "function",
                "function": {
                    "name": wire_name(ctx, name),
                    "arguments": args_json(profile.args_encoding, input)
                }
            })),
            _ => None,
        })
        .collect();
    if !tool_calls.is_empty() {
        return vec![serde_json::json!({
            "role": "assistant",
            "tool_calls": tool_calls
        })];
    }
    let results: Vec<&ContentPart> = parts
        .iter()
        .filter(|part| matches!(part, ContentPart::ToolResult { .. }))
        .collect();
    if !results.is_empty() {
        return results
            .iter()
            .map(|part| {
                let ContentPart::ToolResult {
                    tool_use_id,
                    content,
                    is_error,
                } = part
                else {
                    // `results` holds only tool results; a changed filter must not
                    // panic here.
                    return serde_json::Value::Null;
                };
                // FR-014: `role:"tool"` has no error flag, so the profile
                // declares `false` and the content takes the `ERROR: ` prefix.
                let (content, _) = mark_error(profile, content, *is_error);
                let mut entry = serde_json::Map::new();
                entry.insert("role".to_string(), serde_json::json!("tool"));
                if let IdField::Key(key) = profile.id_field {
                    entry.insert(key.to_string(), serde_json::json!(tool_use_id));
                }
                entry.insert("content".to_string(), serde_json::json!(content));
                if ctx.mirror_tool_name
                    && let Some(name) = ctx
                        .tool_names
                        .and_then(|names| names.get(tool_use_id.as_str()))
                {
                    entry.insert("tool_name".to_string(), serde_json::json!(name));
                }
                serde_json::Value::Object(entry)
            })
            .filter(|entry| !entry.is_null())
            .collect();
    }
    vec![serde_json::json!({
        "role": msg.role,
        "content": openai_content(parts)
    })]
}

/// Native Ollama `/api/chat`: the text of a message always reaches the wire as a
/// string, images travel beside it, tool calls carry object arguments, and a
/// result names its tool instead of keying it by id.
fn ollama_native_message(
    profile: ToolWireProfile,
    msg: &ChatMessage,
    ctx: &MessageContext<'_>,
) -> Vec<serde_json::Value> {
    let (content, images) = match &msg.content {
        ChatContent::Text(text) => (text.clone(), Vec::new()),
        ChatContent::Parts(parts) => {
            let mut texts: Vec<&str> = Vec::new();
            let mut images: Vec<&str> = Vec::new();
            for part in parts {
                match part {
                    ContentPart::Text { text } => texts.push(text),
                    ContentPart::ImageUrl { url } if ctx.allow_images => {
                        // The native endpoint takes raw base64, without the
                        // `data:<mime>;base64,` prefix.
                        images.push(match url.find(";base64,") {
                            Some(at) => &url[at + 8..],
                            None => url.as_str(),
                        });
                    }
                    // A historical image is dropped so a non-vision model is not
                    // rejected for an attachment from an earlier turn.
                    ContentPart::ImageUrl { .. }
                    | ContentPart::ToolUse { .. }
                    | ContentPart::ToolResult { .. }
                    | ContentPart::Reasoning { .. } => {}
                }
            }
            (texts.join("\n"), images)
        }
    };

    let ChatContent::Parts(parts) = &msg.content else {
        return vec![serde_json::json!({ "role": msg.role, "content": content })];
    };
    let tool_calls: Vec<serde_json::Value> = parts
        .iter()
        .filter_map(|part| match part {
            ContentPart::ToolUse { name, input, .. } => Some(serde_json::json!({
                "function": {
                    "name": wire_name(ctx, name),
                    "arguments": args_json(profile.args_encoding, input)
                }
            })),
            _ => None,
        })
        .collect();
    if !tool_calls.is_empty() {
        return vec![serde_json::json!({
            "role": "assistant",
            "content": "",
            "tool_calls": tool_calls
        })];
    }
    let results: Vec<&ContentPart> = parts
        .iter()
        .filter(|part| matches!(part, ContentPart::ToolResult { .. }))
        .collect();
    if !results.is_empty() {
        return results
            .iter()
            .filter_map(|part| {
                let ContentPart::ToolResult {
                    tool_use_id,
                    content,
                    is_error,
                } = part
                else {
                    return None;
                };
                // The native shape has no id slot: it names the tool, and the map
                // is the only place the name survives. It is never derived from
                // the id, and an id the history no longer carries goes out
                // unnamed rather than guessed (FR-019). It also has no error flag,
                // so an error carries the FR-014 `ERROR: ` prefix.
                let (content, _) = mark_error(profile, content, *is_error);
                let name = ctx
                    .tool_names
                    .and_then(|names| names.get(tool_use_id.as_str()))
                    .map_or("", String::as_str);
                Some(serde_json::json!({
                    "role": "tool",
                    "name": name,
                    "content": content
                }))
            })
            .collect();
    }
    let mut entry = serde_json::json!({ "role": msg.role, "content": content });
    if !images.is_empty() {
        entry["images"] = serde_json::json!(images);
    }
    vec![entry]
}

/// Anthropic Messages content blocks: `tool_use` on an assistant turn,
/// `tool_result` keyed by `tool_use_id` in a user turn.
fn anthropic_message(msg: &ChatMessage, ctx: &MessageContext<'_>) -> Vec<serde_json::Value> {
    let content = match &msg.content {
        ChatContent::Text(text) => serde_json::json!(text),
        ChatContent::Parts(parts) => serde_json::json!(
            parts
                .iter()
                .filter_map(|part| match part {
                    ContentPart::Text { text } => Some(serde_json::json!({
                        "type": "text",
                        "text": text
                    })),
                    ContentPart::ToolUse { id, name, input } => Some(serde_json::json!({
                        "type": "tool_use",
                        "id": id,
                        "name": wire_name(ctx, name),
                        "input": input
                    })),
                    ContentPart::ToolResult {
                        tool_use_id,
                        content,
                        is_error,
                    } => Some(serde_json::json!({
                        // FR-014/T-017: Anthropic carries `is_error` natively.
                        "type": "tool_result",
                        "tool_use_id": tool_use_id,
                        "content": content,
                        "is_error": is_error
                    })),
                    ContentPart::ImageUrl { url } => Some(serde_json::json!({
                        "type": "image",
                        "source": {
                            "type": "base64",
                            "media_type": crate::provider::media::extract_mime_from_data_uri(url)
                                .unwrap_or("image/png"),
                            "data": crate::provider::media::extract_base64_from_data_uri(url)
                                .unwrap_or(url.as_str())
                        }
                    })),
                    // FR-011: replayed only for an `echo_reasoning` profile
                    // (T-018).
                    ContentPart::Reasoning { .. } => None,
                })
                .collect::<Vec<serde_json::Value>>()
        ),
    };
    vec![serde_json::json!({
        "role": msg.role,
        "content": content
    })]
}

/// Bedrock Converse content blocks: `toolUse` on an assistant turn, `toolResult`
/// keyed by `toolUseId` in a user turn.
fn converse_message(msg: &ChatMessage, ctx: &MessageContext<'_>) -> Vec<serde_json::Value> {
    // Converse accepts only these two roles; a `system` turn travels in the
    // body's own `system` field and an unexpected role has no slot at all.
    let role = match msg.role.as_str() {
        "user" => "user",
        "assistant" => "assistant",
        _ => return Vec::new(),
    };
    let parts = match &msg.content {
        ChatContent::Text(text) => vec![serde_json::json!({ "text": text })],
        ChatContent::Parts(parts) => parts
            .iter()
            .filter_map(|part| match part {
                ContentPart::Text { text } => Some(serde_json::json!({ "text": text })),
                ContentPart::ImageUrl { url } => {
                    let mime = crate::provider::media::extract_mime_from_data_uri(url)
                        .unwrap_or("image/png");
                    let data = crate::provider::media::extract_base64_from_data_uri(url)
                        .unwrap_or(url.as_str());
                    Some(serde_json::json!({
                        "image": {
                            "format": mime_to_converse_format(mime),
                            "source": { "bytes": data }
                        }
                    }))
                }
                ContentPart::ToolUse { id, name, input } => Some(serde_json::json!({
                    "toolUse": {
                        "toolUseId": id,
                        "name": wire_name(ctx, name),
                        "input": input
                    }
                })),
                ContentPart::ToolResult {
                    tool_use_id,
                    content,
                    is_error,
                } => Some(serde_json::json!({
                    // FR-014/T-017: Converse carries the part's `is_error` as its
                    // native `status` field.
                    "toolResult": {
                        "toolUseId": tool_use_id,
                        "content": [{ "text": content }],
                        "status": if *is_error { "error" } else { "success" }
                    }
                })),
                // FR-011: replayed only for an `echo_reasoning` profile (T-018).
                ContentPart::Reasoning { .. } => None,
            })
            .collect(),
    };
    vec![serde_json::json!({ "role": role, "content": parts })]
}

/// Gemini `contents[].parts[]`: `functionCall` on a model turn,
/// `functionResponse` naming the tool on the answering user turn.
fn gemini_message(
    profile: ToolWireProfile,
    msg: &ChatMessage,
    ctx: &MessageContext<'_>,
) -> Vec<serde_json::Value> {
    let role = match msg.role.as_str() {
        "assistant" => "model",
        _ => "user",
    };
    let parts = match &msg.content {
        ChatContent::Text(text) => vec![serde_json::json!({ "text": text })],
        ChatContent::Parts(parts) => parts
            .iter()
            .filter_map(|part| match part {
                ContentPart::Text { text } => Some(serde_json::json!({ "text": text })),
                ContentPart::ImageUrl { url } => Some(gemini_image_part(url)),
                ContentPart::ToolResult {
                    tool_use_id,
                    content,
                    is_error,
                } => {
                    // The carrier names the tool and has no id slot, so the name
                    // comes from the map, which was built from the canonical
                    // records that opened the calls. It is deliberately never
                    // derived from the id: an id is an opaque handle, and
                    // splitting one on `_` mis-names a tool whose own name
                    // contains an underscore (FR-019). A result whose call is no
                    // longer in the history therefore carries no name rather than
                    // a guessed one, and the gap is logged. The carrier has no
                    // error flag either (FR-014): the `ERROR: ` prefix.
                    let (content, _) = mark_error(profile, content, *is_error);
                    let name = ctx
                        .tool_names
                        .and_then(|names| names.get(tool_use_id.as_str()))
                        .map_or_else(
                            || {
                                tracing::debug!(
                                    tool_use_id = %tool_use_id,
                                    "tool result names a call the history no longer \
                                     carries; the carrier goes out unnamed rather than \
                                     guessing the tool from the id"
                                );
                                ""
                            },
                            String::as_str,
                        );
                    Some(serde_json::json!({
                        "functionResponse": {
                            "name": wire_name(ctx, name),
                            "response": { "result": content }
                        }
                    }))
                }
                ContentPart::ToolUse { name, input, .. } => Some(serde_json::json!({
                    "functionCall": {
                        "name": wire_name(ctx, name),
                        "args": args_json(profile.args_encoding, input)
                    }
                })),
                // FR-011: Gemini has no signed-reasoning channel.
                ContentPart::Reasoning { .. } => None,
            })
            .collect(),
    };
    vec![serde_json::json!({ "role": role, "parts": parts })]
}

/// A Gemini image part: inline data for a `data:` URI, and a textual note for a
/// URL the API cannot fetch itself.
fn gemini_image_part(url: &str) -> serde_json::Value {
    let Some(mime) = crate::provider::media::extract_mime_from_data_uri(url) else {
        return serde_json::json!({ "text": format!("[Image: {url}]") });
    };
    let data = crate::provider::media::extract_base64_from_data_uri(url).unwrap_or(url);
    serde_json::json!({
        "inlineData": {
            "mimeType": mime,
            "data": data
        }
    })
}

/// The Responses input list: a message item for the text content, then one item
/// per tool construct (`function_call` and `function_call_output`).
fn responses_items(
    profile: ToolWireProfile,
    msg: &ChatMessage,
    ctx: &MessageContext<'_>,
) -> Vec<serde_json::Value> {
    let Some(role) = responses_role(&msg.role) else {
        return Vec::new();
    };
    let mut items = Vec::new();
    match &msg.content {
        ChatContent::Text(text) => {
            items.push(serde_json::json!({ "role": role, "content": text }));
        }
        ChatContent::Parts(parts) => {
            let text_parts: Vec<serde_json::Value> = parts
                .iter()
                .filter_map(|part| match part {
                    ContentPart::Text { text } => Some(serde_json::json!({
                        "type": "text",
                        "text": text
                    })),
                    _ => None,
                })
                .collect();
            if !text_parts.is_empty() {
                items.push(serde_json::json!({ "role": role, "content": text_parts }));
            }
            for part in parts {
                match part {
                    ContentPart::ToolUse { id, name, input } => {
                        items.push(serde_json::json!({
                            "type": "function_call",
                            "call_id": id,
                            "name": wire_name(ctx, name),
                            "arguments": args_json(profile.args_encoding, input)
                        }));
                    }
                    ContentPart::ToolResult {
                        tool_use_id,
                        content,
                        is_error,
                    } => {
                        // FR-014: `function_call_output` has no error flag, so the
                        // content takes the `ERROR: ` prefix.
                        let (output, _) = mark_error(profile, content, *is_error);
                        items.push(serde_json::json!({
                            "type": "function_call_output",
                            "call_id": tool_use_id,
                            "output": output
                        }));
                    }
                    // A text part already reached the wire above, and an image has
                    // no item shape on the Responses input path; FR-018's
                    // `WireLoss` refusal for the image is T-007.
                    ContentPart::Text { .. }
                    | ContentPart::ImageUrl { .. }
                    | ContentPart::Reasoning { .. } => {}
                }
            }
        }
    }
    items
}

/// The roles the Responses input list accepts, or `None` for a role it has no
/// slot for.
fn responses_role(role: &str) -> Option<&'static str> {
    match role {
        "user" => Some("user"),
        "assistant" => Some("assistant"),
        "system" => Some("system"),
        "developer" => Some("developer"),
        _ => None,
    }
}

/// Converts a MIME type to the Converse API image format string.
///
/// E.g. `image/png` -> `png`, `image/jpeg` -> `jpeg`. Anything unrecognised
/// falls back to `png`, which is what the Converse request builder did before
/// this function moved here with the message encoder (FR-006).
#[must_use]
pub fn mime_to_converse_format(mime: &str) -> &str {
    match mime {
        "image/jpeg" => "jpeg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        _ => "png",
    }
}
