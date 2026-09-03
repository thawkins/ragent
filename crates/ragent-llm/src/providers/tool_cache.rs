//! Shared byte cache for serialised tool-definition JSON (task H2, sub-task 1).
//!
//! Providers build their request bodies from a session's ~111 [`ToolDefinition`]s
//! on *every* LLM call. The tool definitions only change when the tool registry
//! is invalidated (new tools registered, permissions change visibility), so
//! re-serialising them each turn is pure overhead.
//!
//! The serialised shape is provider-specific — OpenAI-compatible
//! (`{"type":"function","function":{...}}`), Anthropic
//! (`{"name","description","input_schema"}`), Gemini
//! (`{"name","description","parameters"}`), Bedrock (`{"toolSpec"}`) — so the
//! cache is keyed by a combination of the *format* the caller passes in and a
//! content fingerprint of the tool definitions. A process-global [`OnceLock`]
//! cache is used (rather than a field on `ChatRequest`) to keep the public
//! `LlmClient::chat` signature and `ragent-types` structs unchanged.

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::{Arc, Mutex, OnceLock};

use ragent_types::ToolDefinition;

/// The provider-specific serialised shape of a tool list.
///
/// Providers emit different JSON envelopes for the same [`ToolDefinition`]s,
/// so the byte cache must be keyed by the shape being built as well as by the
/// tool content:
/// - OpenAI-compatible: `{"type":"function","function":{name,description,parameters}}`
/// - Anthropic: `{"name","description","input_schema"}`
/// - Gemini: `{"name","description","parameters"}`
/// - Bedrock: `{"toolSpec":{name,description,inputSchema:{json:...}}}`
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
}

const HF_TOOL_PREFIX: &str = "t_";

/// Cache entry key. `format` disambiguates the provider-specific JSON shape;
/// `fingerprint` identifies the exact tool-definition set that was serialised.
type CacheKey = (ToolFormat, u64);

/// The cached serialised tool-list bytes plus the size the caller should
/// report for request-size accounting.
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
    /// Parse the inner tool-list payload (the slice between `payload_start`
    /// and `payload_end`) back into a [`serde_json::Value`].
    ///
    /// The bytes were produced by `serde_json::to_writer` in the cache
    /// builders, so a parse failure can only mean data corruption; the
    /// fallback returns the empty-tolerant `Value::Null` rather than panicking
    /// mid-request (the caller treats a missing tool list as "no tools").
    fn payload_value(&self) -> serde_json::Value {
        serde_json::from_slice(&self.bytes[self.payload_start..self.payload_end])
            .unwrap_or(serde_json::Value::Null)
    }

    /// Extract the inner tool-list array for OpenAI-compatible providers.
    ///
    /// The cached wrapper is `{"tools":[...]}`, so the payload is the `[...]`
    /// array.
    pub fn openai_tools_array(&self) -> serde_json::Value {
        self.payload_value()
    }

    /// Extract the inner tool-list array for Anthropic-format providers.
    ///
    /// The cached wrapper is `{"tools":[...]}` with `input_schema` items, so
    /// the payload is the `[...]` array.
    pub fn anthropic_tools_array(&self) -> serde_json::Value {
        self.payload_value()
    }

    /// Extract the Gemini `tools` array (`[{"functionDeclarations":[...]}]`).
    ///
    /// The cached wrapper is `{"tools":[{"functionDeclarations":[...]}]}`, so
    /// the payload is the `[{"functionDeclarations":[...]}]` array.
    pub fn gemini_tools_array(&self) -> serde_json::Value {
        self.payload_value()
    }

    /// Extract the Bedrock Converse `toolConfig` field value (`{"tools": [...]}`).
    ///
    /// The cached wrapper is `{"tools":[...]}` (each item a `toolSpec` object)
    /// and the caller assigns it directly to `body["toolConfig"]`.
    pub fn bedrock_tool_config_object(&self) -> serde_json::Value {
        self.payload_value()
    }
}

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
    let built = match format {
        ToolFormat::OpenAi => build_openai(tools),
        ToolFormat::Anthropic => build_anthropic(tools),
        ToolFormat::Gemini => build_gemini(tools),
        ToolFormat::Bedrock => build_bedrock(tools),
        ToolFormat::HuggingFace => build_huggingface(tools),
    };
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

fn build_openai(tools: &[ToolDefinition]) -> CachedTools {
    let mut buf = Vec::with_capacity(tools.len() * 128);
    buf.extend_from_slice(br#"{"tools":["#);
    let payload_start = buf.len() - 1; // index of '['
    for (i, t) in tools.iter().enumerate() {
        if i > 0 {
            buf.push(b',');
        }
        let mut item = Vec::with_capacity(128 + t.name.len() + t.description.len());
        serde_json::to_writer(
            &mut item,
            &serde_json::json!({
                "type": "function",
                "function": {
                    "name": t.name,
                    "description": t.description,
                    "parameters": t.parameters
                }
            }),
        )
        .expect("serialise openai tool");
        buf.extend_from_slice(&item);
    }
    buf.push(b']');
    buf.push(b'}');
    let payload_end = buf.len() - 1; // exclusive index after ']'
    let byte_len = payload_end - payload_start;
    CachedTools {
        bytes: buf,
        byte_len,
        payload_start,
        payload_end,
    }
}

fn build_anthropic(tools: &[ToolDefinition]) -> CachedTools {
    let mut buf = Vec::with_capacity(tools.len() * 128);
    buf.extend_from_slice(br#"{"tools":["#);
    let payload_start = buf.len() - 1; // index of '['
    for (i, t) in tools.iter().enumerate() {
        if i > 0 {
            buf.push(b',');
        }
        let mut item = Vec::with_capacity(128 + t.name.len() + t.description.len());
        serde_json::to_writer(
            &mut item,
            &serde_json::json!({
                "name": t.name,
                "description": t.description,
                "input_schema": t.parameters
            }),
        )
        .expect("serialise anthropic tool");
        buf.extend_from_slice(&item);
    }
    buf.push(b']');
    buf.push(b'}');
    let payload_end = buf.len() - 1; // exclusive index after ']'
    let byte_len = payload_end - payload_start;
    CachedTools {
        bytes: buf,
        byte_len,
        payload_start,
        payload_end,
    }
}

fn build_gemini(tools: &[ToolDefinition]) -> CachedTools {
    let mut buf = Vec::with_capacity(tools.len() * 256);
    buf.extend_from_slice(br#"{"tools":["#);
    // The array to assign to `body["tools"]` starts at the '[' after
    // `{"tools":` (index 9) and ends before the final '}'.
    let payload_start = 9;
    for (i, t) in tools.iter().enumerate() {
        if i > 0 {
            buf.push(b',');
        }
        let mut item = Vec::with_capacity(128 + t.name.len() + t.description.len());
        serde_json::to_writer(
            &mut item,
            &serde_json::json!({
                "functionDeclarations": [{
                    "name": t.name,
                    "description": t.description,
                    "parameters": t.parameters
                }]
            }),
        )
        .expect("serialise gemini tool");
        buf.extend_from_slice(&item);
    }
    buf.push(b']');
    buf.push(b'}');
    let payload_end = buf.len() - 1; // exclusive index after ']'
    let byte_len = payload_end - payload_start;
    CachedTools {
        bytes: buf,
        byte_len,
        payload_start,
        payload_end,
    }
}

fn build_bedrock(tools: &[ToolDefinition]) -> CachedTools {
    // Converse API toolConfig field value:
    // {"tools":[{"toolSpec":{name,description,inputSchema:{json:...}}}]}
    let mut buf = Vec::with_capacity(tools.len() * 128);
    buf.extend_from_slice(br#"{"tools":["#);
    for (i, t) in tools.iter().enumerate() {
        if i > 0 {
            buf.push(b',');
        }
        let mut item = Vec::with_capacity(128 + t.name.len() + t.description.len());
        serde_json::to_writer(
            &mut item,
            &serde_json::json!({
                "toolSpec": {
                    "name": t.name,
                    "description": t.description,
                    "inputSchema": {
                        "json": t.parameters
                    }
                }
            }),
        )
        .expect("serialise bedrock tool");
        buf.extend_from_slice(&item);
    }
    buf.push(b']');
    buf.push(b'}');
    let byte_len = buf.len();
    CachedTools {
        bytes: buf,
        byte_len,
        payload_start: 0,
        payload_end: byte_len,
    }
}

fn build_huggingface(tools: &[ToolDefinition]) -> CachedTools {
    let mut buf = Vec::with_capacity(tools.len() * 128);
    buf.extend_from_slice(br#"{"tools":["#);
    let payload_start = buf.len() - 1; // index of '['
    for (i, t) in tools.iter().enumerate() {
        if i > 0 {
            buf.push(b',');
        }
        let mut item = Vec::with_capacity(128 + t.name.len() + t.description.len());
        serde_json::to_writer(
            &mut item,
            &serde_json::json!({
                "type": "function",
                "function": {
                    "name": format!("{HF_TOOL_PREFIX}{}", t.name),
                    "description": t.description,
                    "parameters": t.parameters
                }
            }),
        )
        .expect("serialise huggingface tool");
        buf.extend_from_slice(&item);
    }
    buf.push(b']');
    buf.push(b'}');
    let payload_end = buf.len() - 1; // exclusive index after ']'
    let byte_len = payload_end - payload_start;
    CachedTools {
        bytes: buf,
        byte_len,
        payload_start,
        payload_end,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_tool(name: &str) -> ToolDefinition {
        ToolDefinition {
            name: name.to_string(),
            description: format!("Use the {name} tool."),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        }
    }

    #[test]
    fn empty_openai_tools_array_is_valid() {
        let cached = build_openai(&[]);
        let value = cached.openai_tools_array();
        assert!(value.is_array());
        assert_eq!(value.as_array().unwrap().len(), 0);
    }

    #[test]
    fn openai_tools_array_matches_expected_shape() {
        let cached = build_openai(&[sample_tool("read"), sample_tool("write")]);
        let value = cached.openai_tools_array();
        let arr = value.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["type"], "function");
        assert_eq!(arr[0]["function"]["name"], "read");
    }

    #[test]
    fn empty_anthropic_tools_array_is_valid() {
        let cached = build_anthropic(&[]);
        let value = cached.anthropic_tools_array();
        assert!(value.is_array());
        assert_eq!(value.as_array().unwrap().len(), 0);
    }

    #[test]
    fn anthropic_tools_array_matches_expected_shape() {
        let cached = build_anthropic(&[sample_tool("bash")]);
        let value = cached.anthropic_tools_array();
        let arr = value.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["name"], "bash");
        assert!(arr[0].get("input_schema").is_some());
    }

    #[test]
    fn empty_gemini_tools_array_is_valid() {
        let cached = build_gemini(&[]);
        let value = cached.gemini_tools_array();
        assert!(value.is_array());
        assert_eq!(value.as_array().unwrap().len(), 0);
    }

    #[test]
    fn gemini_tools_array_has_function_declarations_wrapper() {
        let cached = build_gemini(&[sample_tool("read")]);
        let value = cached.gemini_tools_array();
        let arr = value.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert!(arr[0].get("functionDeclarations").is_some());
        let decls = arr[0]["functionDeclarations"].as_array().unwrap();
        assert_eq!(decls.len(), 1);
        assert_eq!(decls[0]["name"], "read");
    }

    #[test]
    fn empty_bedrock_tool_config_is_valid() {
        let cached = build_bedrock(&[]);
        let value = cached.bedrock_tool_config_object();
        let tools = value["tools"].as_array().unwrap();
        assert!(tools.is_empty(), "tools should be empty");
    }

    #[test]
    fn bedrock_tool_config_matches_expected_shape() {
        let cached = build_bedrock(&[sample_tool("read")]);
        let value = cached.bedrock_tool_config_object();
        let tools = value["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0]["toolSpec"]["name"], "read");
        assert!(tools[0]["toolSpec"].get("inputSchema").is_some());
    }

    #[test]
    fn huggingface_tools_get_t_prefix() {
        let cached = build_huggingface(&[sample_tool("read")]);
        let value = cached.openai_tools_array();
        let arr = value.as_array().unwrap();
        assert_eq!(arr[0]["function"]["name"], "t_read");
    }

    #[test]
    fn cached_tools_reuses_same_buffer() {
        let t = sample_tool("read");
        let a = cached_tools(ToolFormat::OpenAi, std::slice::from_ref(&t));
        let b = cached_tools(ToolFormat::OpenAi, std::slice::from_ref(&t));
        assert!(Arc::ptr_eq(&a, &b));
    }
}
