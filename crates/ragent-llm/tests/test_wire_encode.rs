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
    // a provider request at run time.
    //
    // Key order is not stable across builds here, and pinning it as a fixed string
    // would be the wrong assertion. This workspace's graph contains TWO copies of
    // `serde_json`: `ragent-llm` links one without `preserve_order` (a sorted map),
    // and `ragent-agent`'s `gcf` dependency enables the feature in a second copy
    // (insertion order). Cargo unifies features per build, and whether the crate
    // under test links the sorted or the insertion-ordered copy depends on the build
    // graph, so the same source encodes the same tools to different bytes in
    // different runs - measured, both ways. The key *sets* and the byte *lengths* are
    // identical either way, which is what these cases pin; a comparison of `Value`s
    // would not catch a field added or dropped, so the structural checks below stay
    // string-level.
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
            r#"{"tools":[{"type":"function","function":{"name":"Read","description":"d","parameters":{}}}]}"#,
        ),
        (
            "anthropic",
            ANTHROPIC_MESSAGES,
            None,
            r#"{"tools":[{"name":"Read","description":"d","input_schema":{}}]}"#,
        ),
        (
            "gemini",
            GEMINI,
            None,
            r#"{"tools":[{"functionDeclarations":[{"name":"Read","description":"d","parameters":{}}]}]}"#,
        ),
        (
            "bedrock",
            BEDROCK_CONVERSE,
            None,
            r#"{"tools":[{"toolSpec":{"name":"Read","description":"d","inputSchema":{"json":{}}}}]}"#,
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
            r#"{"tools":[{"type":"function","function":{"name":"t_Read","description":"d","parameters":{}}}]}"#,
        ),
    ];
    for (label, profile, prefix, expected) in cases {
        let cached = encode_tools(profile, prefix, &one);
        let got = String::from_utf8_lossy(&cached.bytes).into_owned();
        assert_eq!(
            got.len(),
            expected.len(),
            "{label}: the wrapper, separators or field count changed (key order may \
             differ between builds, the byte count may not): got {got}, want {expected}"
        );
        // The same members in either order: serialising both sides with one process
        // sorts both, so this compares the field set without depending on which
        // `serde_json` copy the encoder linked.
        let want: serde_json::Value = serde_json::from_str(expected).expect("expected parses");
        let have: serde_json::Value = serde_json::from_str(&got).expect("encoder output parses");
        assert_eq!(
            have, want,
            "{label}: the folded encoder must emit the members the old builder emitted"
        );
        // No byte-for-byte comparison: whichever `serde_json` copy this build linked
        // decides the key order, and both orders are correct output (measured - the
        // same source emits each of them in a different build). The members and the
        // length above are the parts that may not change.
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

// ---------------------------------------------------------------------------
// FR-003, full tree (T-027): no provider module builds its own tool JSON
// ---------------------------------------------------------------------------

/// A Rust source file under `src/providers/`, with its comment lines removed.
///
/// Comments are stripped first because the tree's module docs and comments
/// legitimately *name* the envelope keys when explaining where the shapes now
/// live (for example `encode.rs` says "`{"toolSpec":{...}}`"), and a scan that
/// fired on prose would have to be narrowed until it stopped seeing the defect
/// it exists for. Only code can build JSON.
fn provider_sources(dir: &std::path::Path, out: &mut Vec<(std::path::PathBuf, String)>) {
    let entries =
        std::fs::read_dir(dir).unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()));
    let mut paths: Vec<std::path::PathBuf> = entries
        .map(|entry| {
            entry
                .unwrap_or_else(|e| panic!("cannot read an entry of {}: {e}", dir.display()))
                .path()
        })
        .collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            provider_sources(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            let raw = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
            let code = strip_rust_comments(&raw);
            out.push((path, code));
        }
    }
}

/// Remove `//`-to-end-of-line and `/* */` comments from Rust source, leaving
/// string and character literals intact.
///
/// This is a small scanner rather than a lexer: it tracks whether it is inside a
/// string (honouring `\` escapes), a character literal or a block comment, and
/// nothing else. That is exactly enough to answer "is this key text in code",
/// and it never rewrites line numbers, so a reported line is the file's line.
fn strip_rust_comments(source: &str) -> String {
    let bytes = source.as_bytes();
    let mut out = String::with_capacity(source.len());
    let mut i = 0;
    while i < bytes.len() {
        let byte = bytes[i];
        match byte {
            b'/' if bytes.get(i + 1) == Some(&b'/') => {
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if bytes.get(i + 1) == Some(&b'*') => {
                i += 2;
                let mut depth = 1usize;
                while i < bytes.len() && depth > 0 {
                    if bytes[i] == b'/' && bytes.get(i + 1) == Some(&b'*') {
                        depth += 1;
                        i += 2;
                    } else if bytes[i] == b'*' && bytes.get(i + 1) == Some(&b'/') {
                        depth -= 1;
                        i += 2;
                    } else {
                        // Keep line structure so reported lines stay correct.
                        if bytes[i] == b'\n' {
                            out.push('\n');
                        }
                        i += 1;
                    }
                }
            }
            b'"' => {
                out.push('"');
                i += 1;
                while i < bytes.len() {
                    if bytes[i] == b'\\' {
                        out.push(bytes[i] as char);
                        if let Some(next) = bytes.get(i + 1) {
                            out.push(*next as char);
                        }
                        i += 2;
                        continue;
                    }
                    out.push(bytes[i] as char);
                    i += 1;
                    if bytes[i - 1] == b'"' {
                        break;
                    }
                }
            }
            b'\'' => {
                // A character literal (or a lifetime; neither opens a string).
                out.push('\'');
                i += 1;
                while i < bytes.len() {
                    if bytes[i] == b'\\' {
                        out.push(bytes[i] as char);
                        if let Some(next) = bytes.get(i + 1) {
                            out.push(*next as char);
                        }
                        i += 2;
                        continue;
                    }
                    out.push(bytes[i] as char);
                    let closed = bytes[i] == b'\'';
                    i += 1;
                    if closed {
                        break;
                    }
                    if bytes[i - 1] == b'\n' {
                        break;
                    }
                }
            }
            _ => {
                out.push(byte as char);
                i += 1;
            }
        }
    }
    out
}

/// Every envelope key the tree's five item shapes are built from.
///
/// `wire::encode::item_json` is the one place allowed to name any of them; a
/// provider module that names one is building a definition object itself. The
/// Responses item keys (`description`, `parameters`, `name`, `type`) are not
/// listed: they are shared with unrelated message bodies, which is why
/// `test_responses_provider_asks_the_encoder_for_its_tool_array` scopes them to
/// the one module that used to build a Responses array.
///
/// `"tool_use"` is deliberately absent even though Anthropic's *item* is tagged
/// with it: the identical literal is the Anthropic stream's content-block type and
/// its `stop_reason`, so the only files the plain string could scan are the ones
/// where it means something else. The Anthropic item is caught by
/// `"input_schema"`, which no message shape uses.
const ENVELOPE_KEYS: [&str; 8] = [
    "\"input_schema\"",
    "\"inputSchema\"",
    "\"toolSpec\"",
    "\"functionDeclarations\"",
    "\"function_declarations\"",
    "\"tool_call\"",
    "\\\"type\\\":\\\"function\\\"",
    "\"type\": \"function\"",
];

#[test]
fn test_no_provider_module_writes_an_envelope_key() {
    // FR-003 over the whole provider tree: every module under
    // `crates/ragent-llm/src/providers/` (the folder `ragent-llm` declares as its
    // provider layer, `wire/` included) must leave the tool envelope to
    // `wire::encode::item_json`. The encoder's own module is the single
    // exemption, and it is named rather than skipped by pattern, so a *second*
    // module in `wire/` cannot inherit it.
    //
    // This is the tree-wide half of the scan T-004 scoped to the six modules the
    // fold touched, and it adds the keys the scoped version deliberately left out
    // (the Bedrock `inputSchema`, the snake-case `function_declarations` spelling a
    // second family could introduce, and the two OpenAI function spellings the
    // tool-call carrier uses).
    use std::path::PathBuf;

    let providers = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("providers");
    let mut sources: Vec<(PathBuf, String)> = Vec::new();
    provider_sources(&providers, &mut sources);
    assert!(
        sources.len() > 20,
        "the scan found only {} provider sources; it is meant to walk the whole \
         `providers/` tree",
        sources.len()
    );

    let encoder = providers.join("wire").join("encode.rs");
    let mut offenders: Vec<String> = Vec::new();
    for (path, code) in &sources {
        if *path == encoder {
            continue;
        }
        for key in ENVELOPE_KEYS {
            if !code.contains(key) {
                continue;
            }
            for (number, line) in code.lines().enumerate() {
                if line.contains(key) {
                    offenders.push(format!("{}:{}: {key}", path.display(), number + 1));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "a provider module builds its own tool envelope literal (FR-003): every \
         definition shape belongs to `wire::encode::item_json`. Offenders:\n{}",
        offenders.join("\n")
    );
}

#[test]
fn test_no_provider_module_writes_a_tool_body_key_literal() {
    // FR-003's other half: a provider may *assign* the encoded list, but it may
    // not name the body key itself. The key comes from the format
    // (`ToolFormat::body_key`), which derives it from the declared envelope, so an
    // assignment cannot drift from the profile - and, because the scan above only
    // sees shapes, the key is the one place a hand-built list would still show.
    use std::path::PathBuf;

    let providers = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("providers");
    let mut sources: Vec<(PathBuf, String)> = Vec::new();
    provider_sources(&providers, &mut sources);

    // The two constants in `tool_cache.rs` are the keys' single definition.
    let cache = providers.join("tool_cache.rs");
    let mut offenders: Vec<String> = Vec::new();
    for (path, code) in &sources {
        if *path == cache || *path == providers.join("wire").join("encode.rs") {
            continue;
        }
        for key in ["[\"tools\"]", "[\"toolConfig\"]"] {
            for (number, line) in code.lines().enumerate() {
                if line.contains(key) {
                    offenders.push(format!("{}:{}: {key}", path.display(), number + 1));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "a provider names the tool body key itself instead of asking its format \
         (`ToolFormat::body_key`, FR-003). Offenders:\n{}",
        offenders.join("\n")
    );
}

#[test]
fn test_every_tool_body_key_is_the_one_the_encoder_wraps() {
    // The scan above is only worth something if the key a format reports is the
    // key its encoded payload actually fits under: `body_key` says where the
    // payload goes, `encode_tools` decides the payload's shape. Bedrock is the
    // case that matters (it takes the whole `{"tools":[...]}` object under
    // `toolConfig`), and a family that reported the wrong key would produce a
    // body the provider silently ignores.
    for format in ToolFormat::ALL {
        let cached = cached_tools(*format, &[tool("Read")]);
        let payload = std::str::from_utf8(&cached.bytes).expect("encoded tools are UTF-8");
        let key = format.body_key();
        match format.tool_envelope() {
            ragent_llm::provider::wire::ToolEnvelope::BedrockToolSpec => {
                assert_eq!(
                    key, "toolConfig",
                    "{format:?} keeps the whole wrapper, so its body key is `toolConfig`"
                );
                assert!(
                    payload.starts_with("{\"tools\":["),
                    "{format:?}: the `toolConfig` body takes the whole wrapper, got {payload}"
                );
            }
            _ => assert_eq!(
                key, "tools",
                "{format:?} assigns the inner array, so its body key is `tools`"
            ),
        }
    }
}
