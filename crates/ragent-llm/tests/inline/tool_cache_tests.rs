//! Inline tests for `tool_cache.rs` - compiled inside the module tree via the
//! `#[path]` delegation left in the source file (ANTIPAT M2 inline-test
//! migration; no behaviour change).
//!
//! These drive the *cache*: its key, its invalidation, the slice it hands a
//! caller, and the clause FR-003 actually states - a hit is byte-identical to a
//! miss. The envelope *shapes* are pinned by `tests/test_wire_encode.rs`,
//! because the bytes now come from `wire::encode::encode_tools` rather than from
//! a builder in this module (FR-003, T-004); what is asserted here is that the
//! cache adds a key and nothing else.
//!
//! Every case takes [`cache_lock`]. The cache is a process-global `OnceLock`, so
//! in the `--lib` binary every test in this file shares one map with every other
//! test in the binary; one of these cases clears it, and
//! `cached_tools_hit_is_byte_identical_to_a_fresh_encode` asserts on two
//! consecutive calls. Without the lock the clearing case can land between that
//! pair and turn the second call into a miss. Before consolidation each file was
//! its own binary, where a file-local mutex was enough - the note is here so the
//! next person does not "simplify" it away.
//!
//! One other test in this binary touches the same map:
//! `providers::bedrock::tests::test_converse_tool_config`, which reads a Bedrock
//! entry and asserts nothing about identity or emptiness. An interleaved clear
//! only makes it a miss, and by FR-003 a miss is byte-identical to the hit it
//! would have received, so it does not need this lock.

use super::*;

/// Serialise the cache cases against each other within the `--lib` binary.
fn cache_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

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

/// Read the payload slice the caller assigns, whatever shape it is.
///
/// The Bedrock payload is the whole `{"tools":[...]}` object and the other four
/// are the inner array; this is the same distinction `encode_tools` makes, used
/// here so a case can talk about "the payload" without naming a format.
fn payload_of(cached: &CachedTools, envelope: ToolEnvelope) -> serde_json::Value {
    match envelope {
        ToolEnvelope::BedrockToolSpec => cached.bedrock_tool_config_object(),
        _ => cached.openai_tools_array(),
    }
}

#[test]
fn every_format_maps_to_a_distinct_profile_and_prefix_pair() {
    // The whole mapping in one assertion, so `OpenAi`/`HuggingFace` sharing a
    // profile but differing by a prefix is visible rather than implied. A cache
    // whose key resolved to a pair it did not encode with would measure one shape
    // in the TUI while the provider sent another (FR-003).
    let expected = [
        (ToolFormat::OpenAi, OPENAI_CHAT, None),
        (ToolFormat::Anthropic, ANTHROPIC_MESSAGES, None),
        (ToolFormat::Gemini, GEMINI, None),
        (ToolFormat::Bedrock, BEDROCK_CONVERSE, None),
        (ToolFormat::HuggingFace, OPENAI_CHAT, Some(HF_TOOL_PREFIX)),
        (ToolFormat::OpenAiResponses, OPENAI_RESPONSES, None),
    ];
    assert_eq!(
        expected.len(),
        ToolFormat::ALL.len(),
        "every cache key must appear in this table exactly once"
    );

    let mut seen: Vec<(ToolWireProfile, Option<&'static str>)> = Vec::new();
    for (format, profile, prefix) in expected {
        assert!(
            ToolFormat::ALL.contains(&format),
            "{format:?} is not in ALL"
        );
        assert_eq!(
            format.format_profile(),
            profile,
            "{format:?} resolves to the wrong profile"
        );
        assert_eq!(
            format.name_prefix(),
            prefix,
            "{format:?} resolves to the wrong name prefix"
        );
        // The envelope the key reports and the item it emits must agree, and the
        // pair itself must be unique - two keys sharing a pair would be one key.
        assert_eq!(format.tool_envelope(), profile.tool_envelope);
        let pair = (profile, prefix);
        assert!(
            !seen.contains(&pair),
            "{format:?} resolves to {pair:?}, which another key already uses"
        );
        seen.push(pair);
    }
    // HuggingFace is the only key that rewrites a name, and it does so inside the
    // OpenAI envelope rather than by declaring a sixth one.
    assert_eq!(
        ToolFormat::HuggingFace.tool_envelope(),
        ToolEnvelope::OpenAiFunction
    );

    // ...and each key's item really is its declared envelope, not merely labelled
    // as one. This is the folded-in half of the shape check: the byte shapes
    // themselves belong to `test_wire_encode.rs`.
    let _lock = cache_lock();
    for format in ToolFormat::ALL {
        let tools = [sample_tool("Read")];
        let cached = cached_tools(*format, &tools);
        let payload = payload_of(&cached, format.tool_envelope());
        let emitted = match format.tool_envelope() {
            ToolEnvelope::BedrockToolSpec => &payload["tools"][0],
            _ => &payload[0],
        };
        let found = match format.tool_envelope() {
            ToolEnvelope::OpenAiFunction => emitted.get("function").is_some(),
            ToolEnvelope::AnthropicTool => emitted.get("input_schema").is_some(),
            ToolEnvelope::GeminiFunctionDeclaration => {
                emitted.get("functionDeclarations").is_some()
            }
            ToolEnvelope::BedrockToolSpec => emitted.get("toolSpec").is_some(),
            ToolEnvelope::ResponsesFlatFunction => {
                emitted.get("function").is_none() && emitted.get("name").is_some()
            }
        };
        assert!(
            found,
            "{format:?} reports {:?} but emitted {emitted}",
            format.tool_envelope()
        );
        // The name rewrite lands on the name and nowhere else.
        let wire_name = emitted
            .pointer("/function/name")
            .or_else(|| emitted.get("name"))
            .or_else(|| emitted.pointer("/toolSpec/name"))
            .or_else(|| emitted.pointer("/functionDeclarations/0/name"))
            .and_then(serde_json::Value::as_str);
        let want = match format.name_prefix() {
            Some(prefix) => format!("{prefix}Read"),
            None => "Read".to_string(),
        };
        assert_eq!(wire_name, Some(want.as_str()), "{format:?} wire name");
    }
}

#[test]
fn cached_tools_reuses_same_buffer() {
    let _lock = cache_lock();
    invalidate_tool_cache();
    let t = sample_tool("Read");
    let a = cached_tools(ToolFormat::OpenAi, std::slice::from_ref(&t));
    let b = cached_tools(ToolFormat::OpenAi, std::slice::from_ref(&t));
    assert!(
        Arc::ptr_eq(&a, &b),
        "a second call over the same tools must return the same Arc, not a \
         re-serialised equal one"
    );
}

#[test]
fn cached_tools_key_separates_formats_with_the_same_tools() {
    // The key's first component. One tool slice, six keys: every format must get
    // its own entry, because the buffers hold different bytes for the same
    // definitions. Dropping the format from the key would collapse all six onto
    // whichever shape was built first - a wrong request body with no error.
    let _lock = cache_lock();
    invalidate_tool_cache();
    let tools = [sample_tool("Read")];

    let entries: Vec<(ToolFormat, Arc<CachedTools>)> = ToolFormat::ALL
        .iter()
        .map(|format| (*format, cached_tools(*format, &tools)))
        .collect();

    for i in 0..entries.len() {
        for j in (i + 1)..entries.len() {
            let (left, right) = (&entries[i], &entries[j]);
            assert!(
                !Arc::ptr_eq(&left.1, &right.1),
                "{:?} and {:?} share one cache entry, so the format is not in the key",
                left.0,
                right.0
            );
            let same_shape = left.0.format_profile() == right.0.format_profile()
                && left.0.name_prefix() == right.0.name_prefix();
            if !same_shape {
                assert_ne!(
                    left.1.bytes, right.1.bytes,
                    "{:?} and {:?} have different envelopes but identical bytes",
                    left.0, right.0
                );
            }
        }
    }

    // ...and asking again returns each one untouched by the others.
    for (format, entry) in entries {
        assert!(
            Arc::ptr_eq(&entry, &cached_tools(format, &tools)),
            "{format:?}: a later call returned another format's buffer"
        );
    }
}

#[test]
fn responses_and_chat_are_separate_cache_entries_over_the_same_payload_slice() {
    // T-005: the two OpenAI-shaped keys name different profiles, so they must be
    // different entries - while the slice each caller assigns is the same inner
    // array (`{"tools":[...]}` in both cases). Asserting both halves together is
    // what stops "they are the same shape" from collapsing into one key: the item
    // shapes differ even though the offsets do not (FR-003).
    let _lock = cache_lock();
    invalidate_tool_cache();
    let tools = [sample_tool("Read")];

    let chat = cached_tools(ToolFormat::OpenAi, &tools);
    let responses = cached_tools(ToolFormat::OpenAiResponses, &tools);
    assert!(
        !Arc::ptr_eq(&chat, &responses),
        "the two OpenAI-shaped families must not share one entry"
    );
    assert_ne!(
        chat.bytes, responses.bytes,
        "the chat and Responses envelopes must not serialise identically"
    );

    // Each `byte_len` is that buffer's own slice, minus the 10-byte `{"tools":` /
    // `}` wrapper both envelopes carry.
    for (label, cached) in [("chat", &chat), ("responses", &responses)] {
        assert_eq!(
            cached.byte_len,
            cached.bytes.len() - 10,
            "{label}: byte_len must be the inner array, not the wrapper"
        );
    }

    let items = responses.responses_tools_array();
    let first = &items.as_array().expect("the Responses payload is an array")[0];
    assert_eq!(first["name"], "Read");
    assert!(
        first.get("function").is_none(),
        "the Responses item is flattened, so it carries no nested function: {first}"
    );
    let chat_items = chat.openai_tools_array();
    let chat_first = &chat_items.as_array().expect("the chat payload is an array")[0];
    assert_eq!(chat_first["function"]["name"], "Read");
}

#[test]
fn cached_tools_key_tracks_the_tool_content() {
    // The key's second component: a fingerprint over name, description and
    // parameters in that order. Each field is changed on its own, because a
    // fingerprint that hashed only the name would still pass a test that changed
    // all three at once.
    let _lock = cache_lock();
    invalidate_tool_cache();

    let base = sample_tool("Read");
    let baseline = cached_tools(ToolFormat::OpenAi, std::slice::from_ref(&base));

    let mut renamed = base.clone();
    renamed.name = "Write".to_string();
    let mut redescribed = base.clone();
    redescribed.description = "Use the Read tool, differently.".to_string();
    let mut reschema = base.clone();
    reschema.parameters = serde_json::json!({
        "type": "object",
        "properties": { "path": { "type": "string" } },
        "required": ["path"]
    });

    for (label, variant) in [
        ("name", renamed),
        ("description", redescribed),
        ("parameters", reschema),
    ] {
        let got = cached_tools(ToolFormat::OpenAi, std::slice::from_ref(&variant));
        assert!(
            !Arc::ptr_eq(&baseline, &got),
            "changing the {label} did not change the cache key"
        );
        assert_ne!(
            baseline.bytes, got.bytes,
            "changing the {label} must change the serialised bytes"
        );
    }

    // The unchanged definitions still hit the original entry, so the fingerprint
    // is stable rather than merely sensitive.
    assert!(Arc::ptr_eq(
        &baseline,
        &cached_tools(ToolFormat::OpenAi, std::slice::from_ref(&base))
    ));
}

#[test]
fn invalidate_tool_cache_drops_every_entry() {
    // The invalidation path FR-003 relies on: after a tool-registry change the
    // map is empty, so nothing stale can be served - and because a miss and a hit
    // are the same bytes, dropping an entry is invisible to the caller.
    let _lock = cache_lock();
    invalidate_tool_cache();
    let tools = [sample_tool("Read")];
    let before: Vec<(ToolFormat, Arc<CachedTools>)> = ToolFormat::ALL
        .iter()
        .map(|format| (*format, cached_tools(*format, &tools)))
        .collect();

    invalidate_tool_cache();

    for (format, old) in before {
        let rebuilt = cached_tools(format, &tools);
        assert!(
            !Arc::ptr_eq(&old, &rebuilt),
            "{format:?}: the entry survived `invalidate_tool_cache`"
        );
        assert_eq!(
            old.bytes, rebuilt.bytes,
            "{format:?}: the rebuilt entry must carry identical bytes, so a \
             cleared cache is not observable in a request body"
        );
        assert_eq!(old.byte_len, rebuilt.byte_len, "{format:?}: byte_len");
    }

    // Invalidating an already-empty cache is not an error and leaves it empty.
    invalidate_tool_cache();
    let after = cached_tools(ToolFormat::OpenAi, &tools);
    invalidate_tool_cache();
    assert!(!Arc::ptr_eq(
        &after,
        &cached_tools(ToolFormat::OpenAi, &tools)
    ));
}

#[test]
fn empty_tool_list_is_a_valid_empty_payload() {
    // An empty registry must serialise to an empty list, not to `Value::Null`:
    // the accessors parse the payload slice, and a slice that failed to parse
    // would hand the caller `Null`, which a provider reads as an absent field.
    let _lock = cache_lock();
    for format in ToolFormat::ALL {
        let cached = cached_tools(*format, &[]);
        let payload = payload_of(&cached, format.tool_envelope());
        let list = match format.tool_envelope() {
            ToolEnvelope::BedrockToolSpec => payload["tools"].as_array().map(Vec::len),
            _ => payload.as_array().map(Vec::len),
        };
        assert_eq!(
            list,
            Some(0),
            "{format:?}: an empty tool list must serialise to an empty list, not Null"
        );
        assert!(
            !payload.is_null(),
            "{format:?}: the payload failed to parse"
        );
    }
}

#[test]
fn byte_len_counts_the_payload_the_caller_assigns() {
    // `byte_len` is what request-size estimation reads instead of re-serialising
    // the definitions (NFR-001), and the TUI measures context with it. It must be
    // the length of the slice the caller actually assigns: for the four array
    // envelopes that is the inner array, for Bedrock the whole `toolConfig`
    // object - which is exactly the offset pair `encode_tools` decides.
    let _lock = cache_lock();
    let tools = [sample_tool("Read"), sample_tool("Write")];
    for format in ToolFormat::ALL {
        let cached = cached_tools(*format, &tools);
        let serialised = serde_json::to_vec(&payload_of(&cached, format.tool_envelope()))
            .expect("the payload value serialises");
        assert_eq!(
            cached.byte_len,
            serialised.len(),
            "{format:?}: byte_len is not the length of the value the caller assigns"
        );
        match format.tool_envelope() {
            // Bedrock assigns the whole object it was handed.
            ToolEnvelope::BedrockToolSpec => assert_eq!(
                cached.byte_len,
                cached.bytes.len(),
                "the Bedrock payload is the whole encoded object"
            ),
            // The other four keep the inner array, so the payload is strictly
            // smaller than the wrapper they were handed.
            _ => assert!(
                cached.byte_len < cached.bytes.len(),
                "{format:?}: the array payload must exclude the {{\"tools\":}} wrapper"
            ),
        }
    }
}

#[test]
fn cached_tools_hit_is_byte_identical_to_a_fresh_encode() {
    // FR-003's literal clause, checked on the miss *and* on the hit for every key.
    // The comparison is against `encode_tools` with the same `(profile, prefix)`
    // pair the key resolves to, so a key that resolved to a pair it did not encode
    // with fails here rather than in a request body.
    let _lock = cache_lock();
    let tools = [sample_tool("Read"), sample_tool("Write")];
    for format in ToolFormat::ALL {
        invalidate_tool_cache();
        let fresh = encode_tools(format.format_profile(), format.name_prefix(), &tools);
        let cold = cached_tools(*format, &tools);
        let warm = cached_tools(*format, &tools);
        assert!(
            Arc::ptr_eq(&cold, &warm),
            "{format:?}: the second call must be the same entry"
        );
        assert_eq!(
            cold.bytes, fresh.bytes,
            "{format:?}: a cache miss must be byte-identical to a fresh encode"
        );
        assert_eq!(
            warm.bytes, fresh.bytes,
            "{format:?}: a cache hit must be byte-identical to a fresh encode"
        );
        assert_eq!(
            cold.byte_len, fresh.byte_len,
            "{format:?}: the payload range must survive the cache"
        );
    }
    invalidate_tool_cache();
}
