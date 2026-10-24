//! Tests for the one tool-definition encoder (FR-003).
//!
//! FR-003 asks for two things of the encoder: every provider family's tool list
//! comes from the *same* function, and a cache hit is byte-identical to a miss.
//! This file covers the first directly - one case per wire envelope, pinning the
//! shape each family actually puts on the wire - and the second by comparing
//! `cached_tools` (the cache) against `encode_tools` (a fresh encode) for every
//! profile.
//!
//! Nothing here contacts a provider or builds a session: `encode_tools` is a pure
//! function over `&[ToolDefinition]`, which is what NFR-004 asks for.
//!
//! The last case is the only one in this binary that touches the process-global tool
//! cache (the cache's own tests live inline in `tool_cache.rs` and therefore run in
//! the `--lib` binary), so it needs no lock here: there is no second caller to race.
//!
//! The `ResponsesFlatFunction` case was pinned *before* T-005 routed
//! `openai_responses.rs` through the encoder, so the shape the Responses API
//! expects was recorded as a contract rather than inferred from the module the task
//! then changed. That is deliberate: a shape asserted only after its last caller
//! moves is a shape asserted against itself. T-005 has since landed, so the case now
//! also guards the live call site.

use ragent_llm::provider::tool_cache::{ToolFormat, cached_tools, invalidate_tool_cache};
use ragent_llm::provider::wire::encode::encode_tools;
use ragent_llm::provider::wire::profile_table::{
    ANTHROPIC_MESSAGES, BEDROCK_CONVERSE, GEMINI, OPENAI_CHAT, OPENAI_RESPONSES,
};
use ragent_types::ToolDefinition;

/// One tool with a non-trivial schema, so a test can assert the parameters
/// reached the wire rather than only that a name did.
fn tool(name: &str) -> ToolDefinition {
    ToolDefinition {
        name: name.to_string(),
        description: format!("Description of {name}."),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string" }
            },
            "required": ["path"]
        }),
    }
}

/// Two tools - enough that the comma separator between entries is exercised.
fn tools() -> Vec<ToolDefinition> {
    vec![tool("Read"), tool("Write")]
}

#[test]
fn test_encode_tools_openai_function_envelope() {
    let cached = encode_tools(OPENAI_CHAT, None, &tools());
    let array = cached.openai_tools_array();
    let items = array.as_array().expect("the payload is an array");
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["type"], "function");
    assert_eq!(items[0]["function"]["name"], "Read");
    assert_eq!(items[0]["function"]["description"], "Description of Read.");
    assert!(
        items[0]["function"]["parameters"]["properties"]["path"].is_object(),
        "the parameter schema must reach the wire: {}",
        items[0]["function"]["parameters"]
    );
    // The envelope is nested, so no flattened `name` sits beside `type`.
    assert!(
        items[0].get("name").is_none(),
        "the OpenAI envelope nests the name under `function`"
    );
}

#[test]
fn test_encode_tools_anthropic_tool_envelope() {
    let cached = encode_tools(ANTHROPIC_MESSAGES, None, &tools());
    let items = cached.anthropic_tools_array();
    let items = items.as_array().expect("the payload is an array");
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["name"], "Read");
    assert_eq!(items[0]["description"], "Description of Read.");
    assert!(
        items[0]["input_schema"]["properties"]["path"].is_object(),
        "Anthropic carries the schema under `input_schema`: {}",
        items[0]
    );
    // The Anthropic item is flat: no `type`, no `function` wrapper.
    assert!(items[0].get("function").is_none());
    assert!(items[0].get("type").is_none());
}

#[test]
fn test_encode_tools_gemini_function_declaration_envelope() {
    let cached = encode_tools(GEMINI, None, &tools());
    let items = cached.gemini_tools_array();
    let items = items.as_array().expect("the payload is an array");
    assert_eq!(items.len(), 2, "one wrapper object per tool");
    let decls = items[0]["functionDeclarations"]
        .as_array()
        .expect("each entry holds one declaration");
    assert_eq!(decls.len(), 1);
    assert_eq!(decls[0]["name"], "Read");
    assert!(
        decls[0]["parameters"]["properties"]["path"].is_object(),
        "Gemini names the schema `parameters` inside the declaration: {}",
        decls[0]
    );
}

#[test]
fn test_encode_tools_bedrock_tool_spec_envelope() {
    let cached = encode_tools(BEDROCK_CONVERSE, None, &tools());
    let config = cached.bedrock_tool_config_object();
    let specs = config["tools"]
        .as_array()
        .expect("the payload is the whole toolConfig object");
    assert_eq!(specs.len(), 2);
    assert_eq!(specs[0]["toolSpec"]["name"], "Read");
    assert_eq!(specs[0]["toolSpec"]["description"], "Description of Read.");
    assert!(
        specs[0]["toolSpec"]["inputSchema"]["json"]["properties"]["path"].is_object(),
        "Bedrock wraps the schema in inputSchema.json: {}",
        specs[0]
    );
}

#[test]
fn test_encode_tools_responses_flat_function_envelope() {
    let cached = encode_tools(OPENAI_RESPONSES, None, &tools());
    let items = cached.responses_tools_array();
    let items = items.as_array().expect("the payload is an array");
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["type"], "function");
    // Flattened: the name sits beside `type`, not under a `function` object.
    assert_eq!(items[0]["name"], "Read");
    assert_eq!(items[0]["description"], "Description of Read.");
    assert!(
        items[0]["parameters"]["properties"]["path"].is_object(),
        "the Responses item carries `parameters` at the top level: {}",
        items[0]
    );
    assert!(
        items[0].get("function").is_none(),
        "the Responses envelope has no nested `function` object"
    );
}

#[test]
fn test_encode_tools_name_prefix_applies_to_the_name_only() {
    // HuggingFace's `t_` rewrite is a name transformation inside the OpenAI
    // envelope (FR-002), so the prefix must not touch the description or schema.
    let plain = encode_tools(OPENAI_CHAT, None, &tools());
    let prefixed = encode_tools(OPENAI_CHAT, Some("t_"), &tools());
    let plain_items = plain.openai_tools_array();
    let prefixed_items = prefixed.openai_tools_array();
    assert_eq!(prefixed_items[0]["function"]["name"], "t_Read");
    assert_eq!(
        plain_items[0]["function"]["description"], prefixed_items[0]["function"]["description"],
        "the prefix must not reach the description"
    );
    assert_eq!(
        plain_items[0]["function"]["parameters"], prefixed_items[0]["function"]["parameters"],
        "the prefix must not reach the parameter schema"
    );
}

#[test]
fn test_encode_tools_is_deterministic_across_calls() {
    // The properties the cache depends on: same input, same bytes, regardless of
    // how many entries were encoded before.
    let first = encode_tools(OPENAI_CHAT, None, &tools());
    let _ = encode_tools(GEMINI, None, &[]);
    let second = encode_tools(OPENAI_CHAT, None, &tools());
    assert_eq!(first.bytes, second.bytes);
    assert_eq!(first.byte_len, second.byte_len);
}

#[test]
fn test_encode_tools_empty_list_is_an_empty_valid_payload() {
    for (label, profile) in [
        ("openai", OPENAI_CHAT),
        ("anthropic", ANTHROPIC_MESSAGES),
        ("gemini", GEMINI),
        ("bedrock", BEDROCK_CONVERSE),
        ("responses", OPENAI_RESPONSES),
    ] {
        let cached = encode_tools(profile, None, &[]);
        let payload = match profile.tool_envelope {
            ragent_llm::provider::wire::ToolEnvelope::BedrockToolSpec => {
                cached.bedrock_tool_config_object()
            }
            ragent_llm::provider::wire::ToolEnvelope::GeminiFunctionDeclaration => {
                cached.gemini_tools_array()
            }
            ragent_llm::provider::wire::ToolEnvelope::AnthropicTool => {
                cached.anthropic_tools_array()
            }
            ragent_llm::provider::wire::ToolEnvelope::OpenAiFunction => cached.openai_tools_array(),
            ragent_llm::provider::wire::ToolEnvelope::ResponsesFlatFunction => {
                cached.responses_tools_array()
            }
        };
        assert!(
            !payload.is_null(),
            "{label}: an empty tool list must still parse back, not fall back to Null"
        );
        let items = match profile.tool_envelope {
            ragent_llm::provider::wire::ToolEnvelope::BedrockToolSpec => {
                payload["tools"].as_array().map(Vec::len)
            }
            _ => payload.as_array().map(Vec::len),
        };
        assert_eq!(
            items,
            Some(0),
            "{label}: the list must be empty, not absent"
        );
    }
}

#[test]
fn test_cached_tools_hit_equals_a_fresh_encode_for_every_format() {
    // FR-003: "a cache hit is byte-identical to a miss". Asserted for every format
    // the cache can key, against the same fresh encode this file uses, so the
    // format-to-profile mapping in `tool_cache.rs` cannot drift from the encoder.
    let tools = tools();
    let cases = [
        (ToolFormat::OpenAi, OPENAI_CHAT, None),
        (ToolFormat::Anthropic, ANTHROPIC_MESSAGES, None),
        (ToolFormat::Gemini, GEMINI, None),
        (ToolFormat::Bedrock, BEDROCK_CONVERSE, None),
        (ToolFormat::HuggingFace, OPENAI_CHAT, Some("t_")),
        (ToolFormat::OpenAiResponses, OPENAI_RESPONSES, None),
    ];
    for (format, profile, prefix) in cases {
        invalidate_tool_cache();
        let fresh = encode_tools(profile, prefix, &tools);
        let cold = cached_tools(format, &tools);
        let warm = cached_tools(format, &tools);
        assert_eq!(
            cold.bytes, fresh.bytes,
            "{format:?}: a cache miss must be byte-identical to a fresh encode"
        );
        assert_eq!(
            warm.bytes, fresh.bytes,
            "{format:?}: a cache hit must be byte-identical to a fresh encode"
        );
    }
    invalidate_tool_cache();
}

#[test]
fn test_every_profile_a_provider_can_serve_is_reachable_through_the_cache() {
    // FR-003 in the form "for every profile": the cache must be able to *ask for*
    // the bytes of every envelope the provider table declares, so a family cannot
    // end up with a profile no cache key resolves to. T-005 routed the Responses
    // API through the cache, so the exception this case used to carry is gone and
    // the assertion is now total over the provider table.
    use ragent_llm::provider::wire::ToolEnvelope;
    use ragent_llm::provider::wire::profile_table::PROVIDER_WIRE;
    use std::collections::HashSet;

    let cached_envelopes: HashSet<ToolEnvelope> = ToolFormat::ALL
        .iter()
        .map(|format| format.tool_envelope())
        .collect();

    for (id, profile) in PROVIDER_WIRE {
        assert!(
            cached_envelopes.contains(&profile.tool_envelope),
            "{id} declares {:?} but no ToolFormat resolves to that envelope, so its \
             tool list cannot be served from the cache",
            profile.tool_envelope
        );
    }

    // And the other direction: no key resolves to a profile the table does not use,
    // which is how a key that no client can reach would show up.
    for envelope in &cached_envelopes {
        assert!(
            PROVIDER_WIRE
                .iter()
                .any(|(_, profile)| profile.tool_envelope == *envelope),
            "{envelope:?} has a cache key but no provider declares it, so nothing can \
             ask for those bytes"
        );
    }
}

#[test]
fn test_encode_tools_bytes_match_the_pre_fold_builders() {
    // The fold must be behaviour-preserving: these are the exact bytes the five
    // hand-written builders produced before `encode_tools` replaced them, captured
    // from the old `build_*` functions. Pinning them makes a refactor that changes
    // key order, adds a field or drops the comma separator fail here rather than in
    // a provider request at run time. (`serde_json`'s default map sorts keys, so the
    // order is stable without the `preserve_order` feature.)
    let one = [ToolDefinition {
        name: "Read".to_string(),
        description: "d".to_string(),
        parameters: serde_json::json!({}),
    }];
    let cases = [
        (
            "openai",
            OPENAI_CHAT,
            None,
            r#"{"tools":[{"function":{"description":"d","name":"Read","parameters":{}},"type":"function"}]}"#,
        ),
        (
            "anthropic",
            ANTHROPIC_MESSAGES,
            None,
            r#"{"tools":[{"description":"d","input_schema":{},"name":"Read"}]}"#,
        ),
        (
            "gemini",
            GEMINI,
            None,
            r#"{"tools":[{"functionDeclarations":[{"description":"d","name":"Read","parameters":{}}]}]}"#,
        ),
        (
            "bedrock",
            BEDROCK_CONVERSE,
            None,
            r#"{"tools":[{"toolSpec":{"description":"d","inputSchema":{"json":{}},"name":"Read"}}]}"#,
        ),
        (
            "responses",
            OPENAI_RESPONSES,
            None,
            r#"{"tools":[{"description":"d","name":"Read","parameters":{},"type":"function"}]}"#,
        ),
        (
            "huggingface",
            OPENAI_CHAT,
            Some("t_"),
            r#"{"tools":[{"function":{"description":"d","name":"t_Read","parameters":{}},"type":"function"}]}"#,
        ),
    ];
    for (label, profile, prefix, expected) in cases {
        let cached = encode_tools(profile, prefix, &one);
        assert_eq!(
            String::from_utf8_lossy(&cached.bytes),
            expected,
            "{label}: the folded encoder must emit the bytes the old builder emitted"
        );
    }
}

#[test]
fn test_folded_provider_modules_no_longer_write_an_envelope_literal() {
    // FR-003's second test clause: no provider module may hold a hand-built
    // envelope literal. This is the scoped half - the six modules the fold and
    // T-005 cover - so the assertion lands with the change rather than after it.
    // T-027 owns the full-tree version of the same scan, over every file under
    // `providers/` and every envelope key.
    use std::fs;
    use std::path::PathBuf;

    let providers = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("providers");
    let folded = [
        "anthropic.rs",
        "bedrock.rs",
        "gemini.rs",
        "openai_compat.rs",
        "huggingface.rs",
        "openai_responses.rs",
    ];
    let envelope_keys = [
        "\"input_schema\"",
        "\"toolSpec\"",
        "\"functionDeclarations\"",
    ];
    for name in folded {
        let path = providers.join(name);
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        for key in envelope_keys {
            assert!(
                !source.contains(key),
                "{} still writes the tool envelope key {key}; the envelope belongs \
                 to `wire::encode::encode_tools` (FR-003)",
                path.display()
            );
        }
    }
}

#[test]
fn test_responses_provider_asks_the_encoder_for_its_tool_array() {
    // T-005: the Responses client used to build a `Vec<Value>` of flattened
    // function items inline, which is precisely the state FR-003 forbids ("a
    // provider shall not construct its own tool array"). This scans the module for
    // the two literals that construction needed - the item's `type` and its
    // top-level `parameters` - so a re-introduced array fails here rather than
    // being noticed in a request at run time. It is separate from the case above
    // because those keys are the Responses *item* shape, not a wrapper key the
    // other five families share.
    use std::fs;
    use std::path::PathBuf;

    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("providers")
        .join("openai_responses.rs");
    let source =
        fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    for key in ["\"parameters\":", "\"type\": \"function\""] {
        assert!(
            !source.contains(key),
            "{} writes the flattened function item literal {key}; the Responses tool \
             array belongs to `wire::encode::encode_tools` through \
             `ToolFormat::OpenAiResponses` (FR-003)",
            path.display()
        );
    }
    assert!(
        source.contains("cached_tools(ToolFormat::OpenAiResponses"),
        "{} must serve its tool array from the shared cache (FR-003)",
        path.display()
    );
}
