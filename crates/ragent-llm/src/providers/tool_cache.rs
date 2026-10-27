//! Shared byte cache for serialised tool-definition JSON (task H2, sub-task 1).
//!
//! Providers build their request bodies from a session's ~111 [`ToolDefinition`]s
//! on *every* LLM call. The tool definitions only change when the tool registry
//! is invalidated (new tools registered, the tool set changes), so
//! re-serialising them each turn is pure overhead.
//!
//! The serialised shape is provider-specific and is derived in exactly one
//! place - [`encode_tools`] in [`crate::provider::wire::encode`] - from the
//! family's declared [`ToolWireProfile`]. This module adds the caching on top:
//! the cache is keyed by the [`ToolFormat`] the caller passes in and a content
//! fingerprint of the tool definitions, so a hit returns the very same `Arc` a
//! miss would have built (FR-003). A process-global [`OnceLock`] cache is used
//! (rather than a field on `ChatRequest`) to keep the public `LlmClient::chat`
//! signature and `ragent-types` structs unchanged.

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::{Arc, Mutex, OnceLock};

use ragent_types::ToolDefinition;

use crate::provider::wire::ToolEnvelope;
use crate::provider::wire::ToolWireProfile;
pub use crate::provider::wire::encode::CachedTools;
use crate::provider::wire::encode::{HF_TOOL_PREFIX, encode_tools, error_marked};
use crate::provider::wire::profile_table::{
    ANTHROPIC_MESSAGES, BEDROCK_CONVERSE, GEMINI, OPENAI_CHAT, OPENAI_RESPONSES,
};

/// The provider-specific serialised shape of a tool list.
///
/// Providers emit different JSON envelopes for the same [`ToolDefinition`]s,
/// so the byte cache must be keyed by the shape being built as well as by the
/// tool content:
/// - OpenAI-compatible: `{"type":"function","function":{name,description,parameters}}`
/// - Anthropic: `{"name","description","input_schema"}`
/// - Gemini: `{"name","description","parameters"}`
/// - Bedrock: `{"toolSpec":{name,description,inputSchema:{json:...}}}`
/// - OpenAI Responses: the flattened function shape (no nested `function`)
///
/// The enum is only a *cache key* and a name for a shape; the shape itself comes
/// from [`ToolFormat::format_profile`], so this type cannot drift from the
/// encoder's envelope (FR-003).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ToolFormat {
    /// OpenAI function-call wire format.
    OpenAi,
    /// Anthropic tool wire format.
    Anthropic,
    /// Gemini tool wire format.
    Gemini,
    /// Bedrock toolSpec wire format.
    Bedrock,
    /// HuggingFace reuses the OpenAI `{"type":"function",...}` envelope but
    /// prefixes every tool name with `t_` (see `huggingface.rs`), so it needs
    /// its own serialised entries.
    HuggingFace,
    /// OpenAI Responses API: the flattened function shape (no nested
    /// `function` object), so it needs its own serialised entries.
    OpenAiResponses,
}

impl ToolFormat {
    /// Every format the cache can key, for coverage assertions that must be
    /// total over the enum (FR-003).
    pub const ALL: &[Self] = &[
        Self::OpenAi,
        Self::Anthropic,
        Self::Gemini,
        Self::Bedrock,
        Self::HuggingFace,
        Self::OpenAiResponses,
    ];

    /// The declared wire profile this cache key encodes with.
    ///
    /// HuggingFace is an OpenAI-shaped family that rewrites names, so it shares
    /// [`OPENAI_CHAT`] and adds [`ToolFormat::name_prefix`] rather than a sixth
    /// envelope that differs only in a string.
    pub const fn format_profile(self) -> ToolWireProfile {
        match self {
            Self::OpenAi | Self::HuggingFace => OPENAI_CHAT,
            Self::Anthropic => ANTHROPIC_MESSAGES,
            Self::Gemini => GEMINI,
            Self::Bedrock => BEDROCK_CONVERSE,
            Self::OpenAiResponses => OPENAI_RESPONSES,
        }
    }

    /// The name rewrite this family applies inside its envelope, if any.
    pub const fn name_prefix(self) -> Option<&'static str> {
        match self {
            Self::HuggingFace => Some(HF_TOOL_PREFIX),
            Self::OpenAi
            | Self::Anthropic
            | Self::Gemini
            | Self::Bedrock
            | Self::OpenAiResponses => None,
        }
    }

    /// The wire envelope this format resolves to (the profile table's own
    /// vocabulary, so the cache and the encoder cannot disagree about the
    /// mapping).
    pub const fn tool_envelope(self) -> ToolEnvelope {
        self.format_profile().tool_envelope
    }

    /// The declared wire profile this cache key encodes *messages* with
    /// (FR-014, T-017).
    ///
    /// [`ToolFormat::format_profile`] is the profile that encodes this key's
    /// *definitions*, and for five of the six formats the two are the same
    /// object. Two differ, and each difference is a wire fact rather than a
    /// convenience:
    ///
    /// * `HuggingFace` sends OpenAI's chat-completions message shape as-is; only
    ///   its tool *names* are rewritten (`t_`), and names are a definition
    ///   concern, so a message it encodes needs no HuggingFace-specific contract.
    ///   Its result carrier is OpenAI's, and with the same
    ///   `supports_is_error == false`, so the marker is chosen identically - the
    ///   distinction is recorded here so a later rename of `HF_TOOL_PREFIX`'s
    ///   scope cannot silently move an `ERROR: ` prefix onto the wrong side.
    /// * `OpenAiResponses` differs in fact, not only in name: its
    ///   `OPENAI_RESPONSES` profile declares `IdField::Key` where OpenAI's chat
    ///   completions `synthetic_ids` is `true`, and its tool *results* are
    ///   `function_call_output` input items rather than `role:"tool"` messages.
    ///
    /// No format is left undefined: a body and its history cannot use two
    /// contracts, so the pair is total over the enum rather than defaulted.
    pub const fn message_profile(self) -> ToolWireProfile {
        match self {
            Self::OpenAi | Self::HuggingFace => OPENAI_CHAT,
            Self::Anthropic => ANTHROPIC_MESSAGES,
            Self::Gemini => GEMINI,
            Self::Bedrock => BEDROCK_CONVERSE,
            Self::OpenAiResponses => OPENAI_RESPONSES,
        }
    }

    /// Whether this format's declared message profile transmits a tool result's
    /// `is_error` flag natively, or needs the `ERROR: ` marker instead (FR-014).
    ///
    /// It is [`ToolFormat::message_profile`]'s `supports_is_error` and nothing
    /// else, so a dialect that assembles its own `messages` array (local Ollama,
    /// which cannot call the shared encoder because it adds a `tool_name` key)
    /// marks a result by the same rule the encoder does rather than by a second
    /// opinion.
    pub const fn transmits_is_error_natively(self) -> bool {
        self.message_profile().supports_is_error
    }

    /// The name rewrite this family applies inside *its definitions'* envelope
    /// ([`ToolFormat::name_prefix`]), if any.
    ///
    /// HuggingFace rewrites the name in its tool list. Its *messages* are
    /// OpenAI's, where a tool call carries no name at all and a tool result is
    /// keyed by the call id the model already returned - so a name rewrite there
    /// would edit the history for no benefit and could only break the id/name
    /// pairing. This is the one place the definition rewrite is deliberately not
    /// carried into the message layer (FR-014's marking is not affected: both
    /// formats it could concern are non-native).
    pub const fn message_name_prefix(self) -> Option<&'static str> {
        match self {
            Self::HuggingFace
            | Self::OpenAi
            | Self::Anthropic
            | Self::Gemini
            | Self::Bedrock
            | Self::OpenAiResponses => None,
        }
    }

    /// A tool result's content as this format writes it (FR-014): the content
    /// unchanged where the flag goes on the wire, and the literal `ERROR: `
    /// prefix in front of it otherwise.
    ///
    /// The marking itself is [`error_marked`], so the dialect path and the
    /// encoder path produce the same bytes; what this method adds is the *choice*
    /// of whether the marker is needed, taken from the declared profile.
    #[must_use]
    pub fn result_content(self, content: &str, is_error: bool) -> String {
        error_marked(content, is_error && !self.transmits_is_error_natively())
    }

    /// The body key this format's definitions are assigned to (see
    /// [`ToolFormat::body_key`], which derives it from the declared envelope).
    pub const fn body_key(self) -> &'static str {
        match self.tool_envelope() {
            ToolEnvelope::BedrockToolSpec => BEDROCK_TOOLS_KEY,
            ToolEnvelope::OpenAiFunction
            | ToolEnvelope::AnthropicTool
            | ToolEnvelope::GeminiFunctionDeclaration
            | ToolEnvelope::ResponsesFlatFunction => TOOLS_KEY,
        }
    }
}

/// The `toolConfig` body key Bedrock Converse takes the whole `{"tools":[...]}`
/// object under.
///
/// It is a constant rather than a literal at the call site so that FR-003's guard
/// (`tests/test_wire_encode.rs`, T-027) can scan every module under `providers/`
/// for the key text without exempting the one module entitled to name it.
pub const BEDROCK_TOOLS_KEY: &str = "toolConfig";

/// The `tools` body key the other four families assign the inner array to.
///
/// Same rule as [`BEDROCK_TOOLS_KEY`]: named once here, so a provider that writes
/// the literal instead is the one place the scan has to allow, and it does not.
pub const TOOLS_KEY: &str = "tools";

/// Cache entry key. `format` disambiguates the provider-specific JSON shape;
/// `fingerprint` identifies the exact tool-definition set that was serialised.
type CacheKey = (ToolFormat, u64);

static TOOL_CACHE: OnceLock<Mutex<HashMap<CacheKey, Arc<CachedTools>>>> = OnceLock::new();

/// Return the process-global tool-JSON cache.
fn cache() -> &'static Mutex<HashMap<CacheKey, Arc<CachedTools>>> {
    TOOL_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Compute a cheap content fingerprint of the tool definitions so the cache
/// key changes exactly when the tool set changes.
fn fingerprint(tools: &[ToolDefinition]) -> u64 {
    let mut hasher = DefaultHasher::new();
    for t in tools {
        t.name.hash(&mut hasher);
        t.description.hash(&mut hasher);
        t.parameters.hash(&mut hasher);
    }
    hasher.finish()
}

/// Get the cached serialised tool list for `format` + `tools`, or serialise and
/// store it on a miss. The returned [`CachedTools`] is shared (via `Arc`) so
/// every turn reuses the same buffer instead of re-allocating.
///
/// The bytes always come from [`encode_tools`], so a hit is byte-identical to a
/// miss by construction rather than by two implementations agreeing (FR-003).
pub fn cached_tools(format: ToolFormat, tools: &[ToolDefinition]) -> Arc<CachedTools> {
    let key = (format, fingerprint(tools));
    let cached = cache()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(&key)
        .cloned();
    if let Some(hit) = cached {
        return hit;
    }
    // Serialise outside the lock so concurrent misses don't contend.
    let built = encode_tools(format.format_profile(), format.name_prefix(), tools);
    let entry = Arc::new(built);
    let mut guard = cache()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(existing) = guard.get(&key) {
        return existing.clone();
    }
    guard.insert(key, entry.clone());
    entry
}

/// Invalidate the whole tool cache. Called when the session's tool registry is
/// invalidated (the same invalidation that drives `cached_tool_definitions`).
pub fn invalidate_tool_cache() {
    cache()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clear();
}

/// Extract tool-call arguments from an OpenAI-style `function` object,
/// accepting both wire forms:
/// - the streaming string form (`"arguments": "{\"a\":1}"`, built up across
///   delta frames), and
/// - the whole-object form (`"arguments": {"a":1}`) emitted by llama.cpp /
///   vLLM-class servers that echo the full function object per frame.
///
/// Returns `None` when the field is absent. Serialisation failure of the
/// object form is logged and mapped to `None` rather than silently dropping
/// the call's arguments with no diagnostic.
pub(crate) fn tool_arguments_json(function: &serde_json::Value) -> Option<String> {
    match function.get("arguments") {
        Some(serde_json::Value::String(s)) => Some(s.clone()),
        Some(obj @ serde_json::Value::Object(_)) => match serde_json::to_string(obj) {
            Ok(s) => Some(s),
            Err(e) => {
                tracing::warn!("tool-call arguments serialisation failed: {e}");
                None
            }
        },
        _ => None,
    }
}

#[cfg(test)]
#[path = "../../tests/inline/tool_cache_tests.rs"]
mod tests;
