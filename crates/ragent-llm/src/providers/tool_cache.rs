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
use crate::provider::wire::encode::{HF_TOOL_PREFIX, encode_tools};
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
    pub const ALL: &[ToolFormat] = &[
        ToolFormat::OpenAi,
        ToolFormat::Anthropic,
        ToolFormat::Gemini,
        ToolFormat::Bedrock,
        ToolFormat::HuggingFace,
        ToolFormat::OpenAiResponses,
    ];

    /// The declared wire profile this cache key encodes with.
    ///
    /// HuggingFace is an OpenAI-shaped family that rewrites names, so it shares
    /// [`OPENAI_CHAT`] and adds [`ToolFormat::name_prefix`] rather than a sixth
    /// envelope that differs only in a string.
    pub const fn format_profile(self) -> ToolWireProfile {
        match self {
            ToolFormat::OpenAi | ToolFormat::HuggingFace => OPENAI_CHAT,
            ToolFormat::Anthropic => ANTHROPIC_MESSAGES,
            ToolFormat::Gemini => GEMINI,
            ToolFormat::Bedrock => BEDROCK_CONVERSE,
            ToolFormat::OpenAiResponses => OPENAI_RESPONSES,
        }
    }

    /// The name rewrite this family applies inside its envelope, if any.
    pub const fn name_prefix(self) -> Option<&'static str> {
        match self {
            ToolFormat::HuggingFace => Some(HF_TOOL_PREFIX),
            ToolFormat::OpenAi
            | ToolFormat::Anthropic
            | ToolFormat::Gemini
            | ToolFormat::Bedrock
            | ToolFormat::OpenAiResponses => None,
        }
    }

    /// The wire envelope this format resolves to (the profile table's own
    /// vocabulary, so the cache and the encoder cannot disagree about the
    /// mapping).
    pub const fn tool_envelope(self) -> ToolEnvelope {
        self.format_profile().tool_envelope
    }
}

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
