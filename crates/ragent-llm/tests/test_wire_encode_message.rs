//! Tests for the one message encoder and its four result carriers (FR-006).
//!
//! FR-006 asks the active provider's encoder to emit the carrier its profile
//! declares, with an id byte-identical to the one the model returned. Before this
//! encoder every provider module wrote its own copy of the message loop, so the
//! four carriers existed in seven places and only three of them agreed on what an
//! id was called.
//!
//! Three groups of cases run here:
//!
//! 1. **One case per carrier**, pinning the shape each family actually puts on the
//!    wire - the `role:"tool"` message, the `tool_result` block, the
//!    `functionResponse` part, the `function_call_output` item.
//! 2. **The id survives every carrier**, asserted by encoding the same canonical
//!    conversation with each declared profile and reading the id back out of the
//!    position that profile's `id_field` names.
//! 3. **The dialects the encoder absorbed**, so a future edit cannot quietly put
//!    a copy back: the Converse role policy, the HuggingFace name prefix, native
//!    Ollama's name-keyed result and image rules, the Responses input list that
//!    carries tool history (FR-018's `WireLoss` refusal for a construct the input
//!    shape has no slot for - an image part or a message role), and the two
//!    dialect switches that died with the loops (`strict_single_text_collapse` and
//!    Copilot's unconditional collapse).
//!
//! A fourth group pins the conversation-level form. Six of the seven provider
//! modules call it with no context at all, so a body built from it must equal the
//! concatenation of the per-message calls, and a construct the encoder refuses
//! must refuse the whole conversation rather than shorten it.

use std::collections::HashMap;
use std::sync::Arc;

use ragent_llm::llm::{ChatContent, ChatMessage, ChatRequest, ContentPart};
use ragent_llm::provider::wire::ToolWireProfile;
use ragent_llm::provider::wire::encode::{
    MessageContext, encode_message, encode_message_fallible, encode_messages, tool_names_by_id,
};
use ragent_llm::provider::wire::profile_table::{
    ANTHROPIC_MESSAGES, BEDROCK_CONVERSE, GEMINI, OLLAMA_NATIVE, OPENAI_CHAT, OPENAI_RESPONSES,
    PROVIDER_WIRE,
};
use ragent_llm::provider::wire::{IdField, ResultCarrier};
use serde_json::Value;

/// One tool call, as the agent loop records it: a non-empty id, a name, decoded
/// input.
fn tool_use(id: &str, name: &str) -> ContentPart {
    ContentPart::ToolUse {
        id: id.to_string(),
        name: name.to_string(),
        input: serde_json::json!({ "path": "notes.txt" }),
    }
}

/// One tool result answering [`tool_use`].
fn tool_result(id: &str, text: &str) -> ContentPart {
    ContentPart::ToolResult {
        tool_use_id: id.to_string(),
        content: Arc::from(text),
        is_error: false,
    }
}

fn message(role: &str, parts: Vec<ContentPart>) -> ChatMessage {
    ChatMessage {
        role: role.to_string(),
        content: ChatContent::Parts(parts),
    }
}

fn text_message(role: &str, text: &str) -> ChatMessage {
    ChatMessage {
        role: role.to_string(),
        content: ChatContent::Text(text.to_string()),
    }
}

/// The canonical pair every carrier case starts from: an assistant turn with one
/// tool call and the user turn answering it.
fn conversation() -> Vec<ChatMessage> {
    vec![
        text_message("user", "read notes.txt"),
        message(
            "assistant",
            vec![
                ContentPart::Text {
                    text: "reading".to_string(),
                },
                tool_use("call_ab12", "Read"),
            ],
        ),
        message("user", vec![tool_result("call_ab12", "alpha")]),
    ]
}

/// One tool result recorded as an error (FR-014): the loop refuses a denied or
/// failed call with `ToolCallStatus::Error`, and history derives the flag from
/// it.
fn error_result() -> ContentPart {
    ContentPart::ToolResult {
        tool_use_id: "call_ab12".to_string(),
        content: Arc::from("Permission denied"),
        is_error: true,
    }
}

// ---------------------------------------------------------------------------
// 1. One case per carrier
// ---------------------------------------------------------------------------

#[test]
fn test_tool_role_message_carries_the_id_in_the_profile_id_field() {
    let ctx = MessageContext::default();
    let result = encode_message(
        OPENAI_CHAT,
        &message("tool", vec![tool_result("call_ab12", "alpha")]),
        &ctx,
    );

    assert_eq!(result.len(), 1, "one `role:\"tool\"` message per result");
    assert_eq!(result[0]["role"], "tool");
    // Read through the profile's own `id_field` rather than the literal, so this
    // case states "the id is in the key the profile names" and not "the key is
    // `tool_call_id`": the literal is the encoder's to write, and the source scan
    // below is what proves no provider module writes its own.
    let id_key = match OPENAI_CHAT.id_field {
        IdField::Key(key) => key,
        IdField::Synthesised => panic!("the OpenAI family keys its result by id"),
    };
    assert_eq!(result[0][id_key], "call_ab12");
    assert_eq!(result[0]["content"], "alpha");
    assert_eq!(
        result[0].as_object().expect("an object").len(),
        3,
        "the canonical dialect writes role, id and content and nothing else: {}",
        result[0]
    );
}

#[test]
fn test_tool_role_message_mirrors_the_tool_name_only_when_asked() {
    // Local Ollama over `/v1/chat/completions` satisfies both the OpenAI and the
    // native wire shape by writing the name beside the id.
    let request = conversation();
    let names = tool_names_by_id(&request);
    let quiet = MessageContext::default();
    let mirrored = MessageContext {
        tool_names: Some(&names),
        mirror_tool_name: true,
        ..MessageContext::default()
    };
    let result = message("tool", vec![tool_result("call_ab12", "alpha")]);

    let without = encode_message(OPENAI_CHAT, &result, &quiet);
    assert!(without[0].get("tool_name").is_none());

    let with = encode_message(OPENAI_CHAT, &result, &mirrored);
    assert_eq!(with[0]["tool_name"], "Read");
    assert!(
        with[0].get("tool_call_id").is_some(),
        "the id is still written: the name is a mirror, not a replacement: {}",
        with[0]
    );
    assert!(
        without[0].get("tool_name").is_none(),
        "without the mirror the entry is the canonical three fields: {}",
        without[0]
    );
}

#[test]
fn test_user_tool_result_block_wraps_the_result_in_a_user_turn() {
    let ctx = MessageContext::default();
    let result = encode_message(
        ANTHROPIC_MESSAGES,
        &message("user", vec![tool_result("call_ab12", "alpha")]),
        &ctx,
    );

    assert_eq!(result.len(), 1);
    assert_eq!(result[0]["role"], "user");
    assert_eq!(result[0]["content"][0]["type"], "tool_result");
    assert_eq!(result[0]["content"][0]["tool_use_id"], "call_ab12");
    assert_eq!(result[0]["content"][0]["content"], "alpha");
}

#[test]
fn test_converse_tool_result_block_keys_by_tool_use_id() {
    let ctx = MessageContext::default();
    let result = encode_message(
        BEDROCK_CONVERSE,
        &message("user", vec![tool_result("call_ab12", "alpha")]),
        &ctx,
    );

    assert_eq!(result[0]["role"], "user");
    assert_eq!(
        result[0]["content"][0]["toolResult"]["toolUseId"],
        "call_ab12"
    );
    assert_eq!(
        result[0]["content"][0]["toolResult"]["content"][0]["text"],
        "alpha"
    );
    assert_eq!(
        result[0]["content"][0]["toolResult"]["status"], "success",
        "Converse states a status on every result; FR-014/T-017 turns it into the \
         part's `is_error`"
    );
}

// ---------------------------------------------------------------------------
// FR-014/T-017: `is_error` - transmitted natively where the carrier has a flag,
// carried as the literal `ERROR: ` prefix where it does not
// ---------------------------------------------------------------------------

#[test]
fn test_anthropic_tool_result_transmits_is_error_natively() {
    let ctx = MessageContext::default();
    let result = encode_message(
        ANTHROPIC_MESSAGES,
        &message("user", vec![error_result()]),
        &ctx,
    );

    let block = &result[0]["content"][0];
    assert_eq!(block["type"], "tool_result");
    assert_eq!(block["tool_use_id"], "call_ab12");
    assert_eq!(block["content"], "Permission denied");
    assert_eq!(
        block["is_error"], true,
        "FR-014: Anthropic's carrier has a native flag"
    );
    assert!(
        !block["content"]
            .as_str()
            .expect("text")
            .starts_with("ERROR: "),
        "a profile with a native flag never also prefixes the content"
    );
}

#[test]
fn test_converse_tool_result_transmits_is_error_as_status_error() {
    let ctx = MessageContext::default();
    let result = encode_message(
        BEDROCK_CONVERSE,
        &message("user", vec![error_result()]),
        &ctx,
    );

    let block = &result[0]["content"][0]["toolResult"];
    assert_eq!(block["toolUseId"], "call_ab12");
    assert_eq!(block["content"][0]["text"], "Permission denied");
    assert_eq!(
        block["status"], "error",
        "FR-014: Converse carries the flag as its native `status` field"
    );
}

#[test]
fn test_openai_tool_role_message_prefixes_an_error_with_the_marker() {
    let ctx = MessageContext::default();
    let result = encode_message(OPENAI_CHAT, &message("tool", vec![error_result()]), &ctx);

    assert_eq!(result[0]["role"], "tool");
    assert_eq!(result[0]["tool_call_id"], "call_ab12");
    assert_eq!(
        result[0]["content"], "ERROR: Permission denied",
        "FR-014: no native flag on `role:\"tool\"`, so the content takes the prefix"
    );
    assert!(
        result[0].get("is_error").is_none(),
        "the OpenAI shape has no error key to write"
    );
}

#[test]
fn test_native_ollama_tool_role_message_prefixes_an_error_with_the_marker() {
    let request = conversation();
    let names = tool_names_by_id(&request);
    let ctx = MessageContext {
        tool_names: Some(&names),
        ..MessageContext::default()
    };
    let result = encode_message(OLLAMA_NATIVE, &message("user", vec![error_result()]), &ctx);

    assert_eq!(result[0]["role"], "tool");
    assert_eq!(result[0]["name"], "Read");
    assert_eq!(
        result[0]["content"], "ERROR: Permission denied",
        "FR-014: the name-keyed native shape has no error flag either"
    );
}

#[test]
fn test_gemini_function_response_prefixes_an_error_with_the_marker() {
    let request = conversation();
    let names = tool_names_by_id(&request);
    let ctx = MessageContext {
        tool_names: Some(&names),
        ..MessageContext::default()
    };
    let result = encode_message(GEMINI, &message("user", vec![error_result()]), &ctx);

    let response = &result[0]["parts"][0]["functionResponse"];
    assert_eq!(response["name"], "Read");
    assert_eq!(
        response["response"]["result"], "ERROR: Permission denied",
        "FR-014: the function response's `result` carries the prefix"
    );
}

#[test]
fn test_responses_function_call_output_prefixes_an_error_with_the_marker() {
    let ctx = MessageContext::default();
    let result = encode_message(
        OPENAI_RESPONSES,
        &message("user", vec![error_result()]),
        &ctx,
    );

    assert_eq!(result[0]["type"], "function_call_output");
    assert_eq!(result[0]["call_id"], "call_ab12");
    assert_eq!(
        result[0]["output"], "ERROR: Permission denied",
        "FR-014: the Responses output item has no error flag"
    );
}

#[test]
fn test_a_successful_result_is_never_flagged_or_prefixed() {
    // The change is a flag, not a general edit: a success must reach every
    // carrier byte-identical to how it was before FR-014.
    let ctx = MessageContext::default();

    let anthropic = encode_message(
        ANTHROPIC_MESSAGES,
        &message("user", vec![tool_result("call_ab12", "alpha")]),
        &ctx,
    );
    assert_eq!(anthropic[0]["content"][0]["content"], "alpha");
    assert_eq!(anthropic[0]["content"][0]["is_error"], false);

    let converse = encode_message(
        BEDROCK_CONVERSE,
        &message("user", vec![tool_result("call_ab12", "alpha")]),
        &ctx,
    );
    assert_eq!(converse[0]["content"][0]["toolResult"]["status"], "success");

    for profile in [OPENAI_CHAT, GEMINI, OPENAI_RESPONSES, OLLAMA_NATIVE] {
        let message = message("tool", vec![tool_result("call_ab12", "alpha")]);
        let request = conversation();
        let names = tool_names_by_id(&request);
        let ctx = MessageContext {
            tool_names: Some(&names),
            ..MessageContext::default()
        };
        let out = encode_message(profile, &message, &ctx);
        let joined = serde_json::to_string(&out).expect("encodes");
        assert!(
            !joined.contains("ERROR: "),
            "a successful result never carries the prefix ({}): {joined}",
            profile.tool_envelope.as_str()
        );
    }
}

#[test]
fn test_gemini_function_response_names_the_tool_it_answers() {
    // FR-006 calls the Gemini carrier out by name because it has no id slot at
    // all: the id-to-name map is the only way the name reaches the wire.
    let request = conversation();
    let names = tool_names_by_id(&request);
    let ctx = MessageContext {
        tool_names: Some(&names),
        ..MessageContext::default()
    };
    let result = encode_message(
        GEMINI,
        &message("user", vec![tool_result("call_ab12", "alpha")]),
        &ctx,
    );

    assert_eq!(result[0]["role"], "user");
    assert_eq!(
        result[0]["parts"][0]["functionResponse"]["name"], "Read",
        "the carrier names the tool, recovered from the id the conversation carried"
    );
    assert_eq!(
        result[0]["parts"][0]["functionResponse"]["response"]["result"],
        "alpha"
    );
    assert!(
        result[0]["parts"][0].get("functionResponse").is_some(),
        "the Gemini carrier is a part, not a message field: {}",
        result[0]
    );
}

#[test]
fn test_responses_function_call_output_is_an_input_item_keyed_by_call_id() {
    let ctx = MessageContext::default();
    let result = encode_message(
        OPENAI_RESPONSES,
        &message("user", vec![tool_result("call_ab12", "alpha")]),
        &ctx,
    );

    assert_eq!(result.len(), 1);
    assert_eq!(result[0]["type"], "function_call_output");
    assert_eq!(result[0]["call_id"], "call_ab12");
    assert_eq!(result[0]["output"], "alpha");
    assert!(
        result[0].get("content").is_none(),
        "the Responses item has an `output`, not a `content`: {}",
        result[0]
    );
}

// ---------------------------------------------------------------------------
// 2. The id survives every carrier, for every declared profile
// ---------------------------------------------------------------------------

/// Every profile a registered provider declares, deduplicated by profile value so
/// a family served by eight providers is exercised once.
fn every_declared_profile() -> Vec<(&'static str, ragent_llm::provider::wire::ToolWireProfile)> {
    let mut seen: Vec<(&'static str, ragent_llm::provider::wire::ToolWireProfile)> = Vec::new();
    for (provider, profile) in PROVIDER_WIRE {
        if !seen.iter().any(|(_, existing)| existing == profile) {
            seen.push((provider, *profile));
        }
    }
    seen
}

/// Every position a carrier can put a call id in, read back out as a string.
fn ids_in(value: &Value, carrier: ResultCarrier) -> Vec<String> {
    let mut ids = Vec::new();
    match carrier {
        ResultCarrier::ToolRoleMessage | ResultCarrier::ResponsesFunctionCallOutput => {
            if let Some(object) = value.as_object() {
                for key in ["tool_call_id", "call_id"] {
                    if let Some(id) = object.get(key).and_then(Value::as_str) {
                        ids.push(id.to_string());
                    }
                }
            }
        }
        ResultCarrier::UserToolResultBlock => {
            let blocks = match value.get("content").and_then(Value::as_array) {
                Some(blocks) => blocks.clone(),
                None => Vec::new(),
            };
            for block in blocks {
                for key in ["tool_use_id", "toolUseId"] {
                    if let Some(id) = block.get(key).and_then(Value::as_str) {
                        ids.push(id.to_string());
                    }
                    if let Some(id) = block
                        .get("toolResult")
                        .and_then(|r| r.get(key))
                        .and_then(Value::as_str)
                    {
                        ids.push(id.to_string());
                    }
                }
            }
        }
        ResultCarrier::GeminiFunctionResponse => {
            // The Gemini carrier has no id slot; FR-006 exempts it by naming the
            // tool instead, so there is nothing to read back here.
        }
    }
    ids
}

#[test]
fn test_every_declared_profile_writes_its_carrier_and_keeps_the_id() {
    let request = conversation();
    let names = tool_names_by_id(&request);
    let ctx = MessageContext {
        tool_names: Some(&names),
        ..MessageContext::default()
    };

    for (provider, profile) in every_declared_profile() {
        let encoded = encode_message(
            profile,
            &message("user", vec![tool_result("call_ab12", "alpha")]),
            &ctx,
        );
        assert!(
            !encoded.is_empty(),
            "{provider}: a tool result must reach the wire in the declared carrier \
             {:?}, not vanish",
            profile.result_carrier
        );
        let joined = encoded
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(" ");
        match profile.result_carrier {
            ResultCarrier::GeminiFunctionResponse => {
                assert!(
                    joined.contains("functionResponse"),
                    "{provider}: the Gemini carrier is a `functionResponse` part: {joined}"
                );
                assert!(
                    joined.contains("\"Read\""),
                    "{provider}: the Gemini carrier names the tool: {joined}"
                );
            }
            carrier => {
                let ids: Vec<String> = encoded.iter().flat_map(|v| ids_in(v, carrier)).collect();
                match profile.id_field {
                    IdField::Key(key) => assert_eq!(
                        ids,
                        vec!["call_ab12".to_string()],
                        "{provider}: the id must reach the wire once, under {key}: {joined}"
                    ),
                    IdField::Synthesised => assert!(
                        ids.is_empty(),
                        "{provider}: a synthesised-id profile has no id slot to write: {joined}"
                    ),
                }
            }
        }
    }
}

#[test]
fn test_the_id_on_a_carrier_is_byte_identical_to_the_id_on_the_call() {
    // FR-006's second clause, stated as an equality rather than a shape: whatever a
    // provider hands back is what the encoder must write, with no normalisation,
    // trimming, re-casing or re-keying on the way.
    let awkward = "call_Ab-12:x/._9 ";
    let ctx = MessageContext::default();
    for profile in [
        OPENAI_CHAT,
        ANTHROPIC_MESSAGES,
        BEDROCK_CONVERSE,
        OPENAI_RESPONSES,
    ] {
        let encoded = encode_message(
            profile,
            &message("user", vec![tool_result(awkward, "alpha")]),
            &ctx,
        );
        let ids: Vec<String> = encoded
            .iter()
            .flat_map(|v| ids_in(v, profile.result_carrier))
            .collect();
        assert_eq!(
            ids,
            vec![awkward.to_string()],
            "{}: the carrier must carry the id the model returned, byte for byte",
            profile.result_carrier.as_str()
        );
    }
}

#[test]
fn test_the_assistant_turn_carries_the_tool_call_with_its_arguments() {
    // The call side of the same requirement: every family must emit the invocation
    // as well as the answer, or the model cannot match one to the other.
    let ctx = MessageContext::default();
    let call = message("assistant", vec![tool_use("call_ab12", "Read")]);

    let openai = encode_message(OPENAI_CHAT, &call, &ctx);
    assert_eq!(openai[0]["tool_calls"][0]["id"], "call_ab12");
    assert_eq!(
        openai[0]["tool_calls"][0]["function"]["arguments"],
        serde_json::json!({"path": "notes.txt"}).to_string(),
        "an OpenAI family argument is an escaped JSON string"
    );

    let anthropic = encode_message(ANTHROPIC_MESSAGES, &call, &ctx);
    assert_eq!(anthropic[0]["content"][0]["type"], "tool_use");
    assert_eq!(anthropic[0]["content"][0]["input"]["path"], "notes.txt");

    let gemini = encode_message(GEMINI, &call, &ctx);
    assert_eq!(
        gemini[0]["role"], "model",
        "Gemini calls the assistant `model`"
    );
    assert_eq!(
        gemini[0]["parts"][0]["functionCall"]["args"]["path"],
        "notes.txt"
    );

    let responses = encode_message(OPENAI_RESPONSES, &call, &ctx);
    assert_eq!(responses[0]["type"], "function_call");
    assert_eq!(responses[0]["call_id"], "call_ab12");
    assert_eq!(
        responses[0]["arguments"],
        "{\"path\":\"notes.txt\"}".to_string()
    );
}

// ---------------------------------------------------------------------------
// 3. The dialects the encoder absorbed
// ---------------------------------------------------------------------------

#[test]
fn test_converse_drops_a_role_it_has_no_slot_for() {
    // The Converse body carries `system` in its own top-level field and accepts
    // only `user` and `assistant` turns, so the encoder - not the caller - decides
    // a message has nowhere to go.
    let ctx = MessageContext::default();
    assert!(
        encode_message(BEDROCK_CONVERSE, &text_message("system", "be brief"), &ctx).is_empty(),
        "the Converse encoder owns the system-prompt role policy"
    );
    assert!(
        encode_message(BEDROCK_CONVERSE, &text_message("tool", "orphan"), &ctx).is_empty(),
        "an unknown role has no Converse slot"
    );
    assert_eq!(
        encode_message(BEDROCK_CONVERSE, &text_message("user", "hi"), &ctx)[0]["role"],
        "user"
    );
}

#[test]
fn test_the_name_prefix_rewrites_the_name_inside_the_call() {
    // HuggingFace's `t_` prefix is a name rewrite inside the OpenAI envelope, not a
    // sixth envelope (see `encode_tools`). The encoder applies it to the name and to
    // nothing else - not the id, not the arguments.
    let ctx = MessageContext {
        name_prefix: Some("t_"),
        ..MessageContext::default()
    };
    let encoded = encode_message(
        OPENAI_CHAT,
        &message("assistant", vec![tool_use("call_ab12", "Read")]),
        &ctx,
    );
    assert_eq!(encoded[0]["tool_calls"][0]["function"]["name"], "t_Read");
    assert_eq!(encoded[0]["tool_calls"][0]["id"], "call_ab12");
    assert_eq!(
        encoded[0]["tool_calls"][0]["function"]["arguments"],
        serde_json::json!({"path": "notes.txt"}).to_string()
    );
}

#[test]
fn test_native_ollama_writes_object_arguments_and_a_name_keyed_result() {
    let request = conversation();
    let names = tool_names_by_id(&request);
    let ctx = MessageContext {
        tool_names: Some(&names),
        ..MessageContext::default()
    };

    let call = encode_message(
        OLLAMA_NATIVE,
        &message("assistant", vec![tool_use("call_ab12", "Read")]),
        &ctx,
    );
    assert_eq!(
        call[0]["tool_calls"][0]["function"]["arguments"]["path"], "notes.txt",
        "the native shape carries arguments as an object, not a string"
    );
    assert!(
        call[0]["tool_calls"][0].get("id").is_none(),
        "the native call has no id wrapper: {}",
        call[0]
    );

    let result = encode_message(
        OLLAMA_NATIVE,
        &message("tool", vec![tool_result("call_ab12", "alpha")]),
        &ctx,
    );
    assert_eq!(result[0]["role"], "tool");
    assert_eq!(
        result[0]["name"], "Read",
        "the native result names the tool"
    );
    assert!(
        result[0].get("tool_call_id").is_none(),
        "and carries no id: {}",
        result[0]
    );
}

#[test]
fn test_native_ollama_strips_the_data_uri_and_keeps_images_to_the_turn_allowed() {
    let image = |url: &str| {
        message(
            "user",
            vec![ContentPart::ImageUrl {
                url: url.to_string(),
            }],
        )
    };
    let allowed = MessageContext::default();
    let dropped = MessageContext {
        allow_images: false,
        ..MessageContext::default()
    };

    let kept = encode_message(
        OLLAMA_NATIVE,
        &image("data:image/png;base64,OLDB64"),
        &allowed,
    );
    assert_eq!(
        kept[0]["images"][0], "OLDB64",
        "the native endpoint takes raw base64 without the data-URI prefix"
    );

    let historical = encode_message(
        OLLAMA_NATIVE,
        &image("data:image/png;base64,OLDB64"),
        &dropped,
    );
    assert!(
        historical[0].get("images").is_none(),
        "a historical image is dropped so a non-vision model is not rejected: {}",
        historical[0]
    );
}

#[test]
fn test_native_ollama_content_is_always_a_string() {
    // Native Ollama declares `content` as a Go `string`. Any other JSON value - an
    // image-only user turn, or a part list holding nothing but constructs - makes the
    // endpoint reject the entire request with `json: cannot unmarshal array into Go
    // struct field ... of type string`, which is what a session that had posted an
    // image then saw on every later turn.
    let ctx = MessageContext::default();
    let image_only = message(
        "user",
        vec![ContentPart::ImageUrl {
            url: "data:image/png;base64,AAA".to_string(),
        }],
    );
    let encoded = encode_message(OLLAMA_NATIVE, &image_only, &ctx);
    assert_eq!(
        encoded[0]["content"],
        serde_json::json!(""),
        "an image-only turn still writes a string content: {}",
        encoded[0]
    );
    assert_eq!(encoded[0]["images"][0], "AAA");

    // Two text parts concatenate into one string rather than an array of blocks.
    let two_texts = message(
        "user",
        vec![
            ContentPart::Text {
                text: "first".to_string(),
            },
            ContentPart::Text {
                text: "second".to_string(),
            },
        ],
    );
    let encoded = encode_message(OLLAMA_NATIVE, &two_texts, &ctx);
    assert_eq!(encoded[0]["content"], serde_json::json!("first\nsecond"));
}

#[test]
fn test_responses_input_carries_tool_history_instead_of_only_text() {
    // FR-018's subject is that the Responses path used to drop every non-text part.
    // The carrier now reaches the wire; the `WireLoss` policy for the constructs
    // this shape genuinely has no slot for is
    // `test_a_role_the_responses_input_list_has_no_item_for_is_refused` and
    // `test_a_refused_construct_refuses_the_whole_conversation`.
    let ctx = MessageContext::default();
    let request = conversation();
    let input: Vec<Value> = request
        .iter()
        .flat_map(|msg| encode_message(OPENAI_RESPONSES, msg, &ctx))
        .collect();

    let kinds: Vec<&str> = input
        .iter()
        .filter_map(|item| item.get("type").and_then(Value::as_str))
        .collect();
    assert_eq!(
        kinds,
        vec!["function_call", "function_call_output"],
        "both halves of the tool exchange reach the input list: {input:?}"
    );
    // Entries: the user turn, the assistant turn's text item, then the call and its
    // output. The text item is a `role`/`content` entry like any other message.
    assert_eq!(input[0]["role"], "user");
    assert_eq!(input[1]["role"], "assistant");
    assert_eq!(
        input[1]["content"][0]["text"], "reading",
        "the assistant text beside the call is not lost either"
    );
}

#[test]
fn test_a_text_only_message_reaches_every_family() {
    // The cheapest total check: no profile may silently emit nothing for ordinary
    // conversation. A family that dropped a plain user turn would still pass every
    // carrier case above while being useless in a session.
    let ctx = MessageContext::default();
    for (provider, profile) in every_declared_profile() {
        let encoded = encode_message(profile, &text_message("user", "hello"), &ctx);
        assert_eq!(
            encoded.len(),
            1,
            "{provider}: a user turn must produce exactly one entry"
        );
        let joined = encoded[0].to_string();
        assert!(
            joined.contains("hello"),
            "{provider}: the text must reach the wire: {joined}"
        );
    }
}

#[test]
fn test_no_provider_module_writes_a_result_carrier_literal() {
    // The structural half of FR-006: the four carriers have one definition in the
    // tree, so a provider module may not hold any of their wire keys. This is a
    // source scan rather than a behavioural test because the failure it guards
    // against is a *second copy* that happens to be correct today - no request
    // would reveal it, and the next provider to copy it would be wrong.
    use std::fs;
    use std::path::PathBuf;

    let providers = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("providers");
    let modules = [
        "anthropic.rs",
        "bedrock.rs",
        "gemini.rs",
        "huggingface.rs",
        "ollama_cloud.rs",
        "openai_compat.rs",
        "openai_responses.rs",
    ];
    for name in modules {
        let path = providers.join(name);
        assert!(
            path.is_file(),
            "the scan reads {name}, so a renamed module would quietly pass: {}",
            path.display()
        );
    }
    // Every key that names one of the four carriers on the request side. The
    // response side reads the same keys back (`toolUse` on a Converse delta,
    // `tool_calls` in an SSE frame), which is a decoder's job and is why the scan
    // cannot simply name the string.
    let request_carriers = [
        ("\"tool_result\"", "the Anthropic result carrier"),
        ("\"toolResult\"", "the Converse result carrier"),
        ("\"functionResponse\"", "the Gemini result carrier"),
        ("\"function_call_output\"", "the Responses result carrier"),
        ("\"tool_call_id\":", "the OpenAI result-carrier id"),
    ];
    for name in modules {
        let path = providers.join(name);
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        for (key, what) in request_carriers {
            // The scan names the key itself, so a hit is reported with the line it
            // is on: the failure is then the copy to delete, not a hunt for it.
            if let Some(line) = source
                .lines()
                .position(|text| text.contains(key))
                .map(|index| index + 1)
            {
                panic!(
                    "{}:{line} still writes {key}, which is {what}; every request-side \
                     carrier belongs to `wire::encode::encode_message` (FR-006)",
                    path.display()
                );
            }
        }
    }
}

#[test]
fn test_the_encoder_is_the_only_place_the_carriers_are_written() {
    // The other direction: the one encoder must actually implement all four, or the
    // scan above could be satisfied by deleting the constructs entirely.
    use std::fs;
    use std::path::PathBuf;

    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("providers")
        .join("wire")
        .join("encode.rs");
    let source =
        fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    for key in [
        "\"tool_call_id\"",
        "\"tool_result\"",
        "\"toolUseId\"",
        "\"functionResponse\"",
        "\"function_call_output\"",
    ] {
        assert!(
            source.contains(key),
            "{} must define {key}: it is the one encoder for the four result \
             carriers (FR-006)",
            path.display()
        );
    }
}

#[test]
fn test_tool_names_by_id_scans_every_turn_once() {
    // The helper exists so two carriers can name a tool they were only told the id
    // of. It must see a call in any turn, keep the first id when a stream repeated
    // it, and return nothing for a conversation with no calls.
    let mut turns = conversation();
    turns.push(message("assistant", vec![tool_use("call_second", "Glob")]));

    let map = tool_names_by_id(&turns);
    assert_eq!(map.len(), 2);
    assert_eq!(map.get("call_ab12").map(String::as_str), Some("Read"));
    assert_eq!(map.get("call_second").map(String::as_str), Some("Glob"));

    assert_eq!(
        tool_names_by_id(&[text_message("user", "no calls here")]).len(),
        0
    );
}

#[test]
fn test_a_gemini_result_never_recovers_a_tool_name_by_splitting_the_id() {
    // FR-019's unwanted behaviour. The name a `functionResponse` carries comes from
    // the canonical record that opened the call, never from the id: an id is an
    // opaque handle, and `read_file_0` split on `_` names the tool `read`. A result
    // whose call the history no longer carries therefore goes out unnamed - a
    // visibly missing name, logged - rather than under a plausible wrong one.
    // (Making that gap a hard refusal is FR-018, T-007.)
    let ctx = MessageContext::default();
    let encoded = encode_message(
        GEMINI,
        &message("user", vec![tool_result("orphan_tool_0", "alpha")]),
        &ctx,
    );
    assert_eq!(
        encoded[0]["parts"][0]["functionResponse"]["name"], "",
        "no prefix of `orphan_tool_0` is a tool name the record ever carried: {}",
        encoded[0]
    );

    // The same carrier with the calling turn present names the tool the record
    // named, including when the tool's own name contains an underscore.
    let mut turns = vec![message("assistant", vec![tool_use("call_1", "read_file")])];
    turns.push(message("user", vec![tool_result("call_1", "alpha")]));
    let names = tool_names_by_id(&turns);
    let ctx = MessageContext {
        tool_names: Some(&names),
        ..MessageContext::default()
    };
    let encoded = encode_message(GEMINI, &turns[1], &ctx);
    assert_eq!(
        encoded[0]["parts"][0]["functionResponse"]["name"], "read_file",
        "the name comes from the record, whole"
    );
}

#[test]
fn test_a_message_with_several_results_becomes_one_entry_per_result() {
    // The OpenAI carrier is the one family whose single canonical message expands to
    // several wire entries: the protocol has no array of results on one message.
    let ctx = MessageContext::default();
    let encoded = encode_message(
        OPENAI_CHAT,
        &message(
            "tool",
            vec![tool_result("call_a", "one"), tool_result("call_b", "two")],
        ),
        &ctx,
    );

    assert_eq!(encoded.len(), 2, "one `role:\"tool\"` message per result");
    assert_eq!(encoded[0]["tool_call_id"], "call_a");
    assert_eq!(encoded[1]["tool_call_id"], "call_b");
    assert_eq!(encoded[1]["content"], "two");

    // Converse, by contrast, keeps them as blocks of one user turn - and note the
    // role: the Converse body accepts only `user` and `assistant` turns, which is
    // what the history converter emits for a tool result (see
    // `test_converse_drops_a_role_it_has_no_slot_for`).
    let converse = encode_message(
        BEDROCK_CONVERSE,
        &message(
            "user",
            vec![tool_result("call_a", "one"), tool_result("call_b", "two")],
        ),
        &ctx,
    );
    assert_eq!(converse.len(), 1);
    assert_eq!(
        converse[0]["content"].as_array().map(Vec::len),
        Some(2),
        "the block carrier holds both results in one turn"
    );
}

#[test]
fn test_an_empty_conversation_encodes_to_nothing() {
    // A guard against an encoder that manufactures an empty message: a request with
    // no history must produce no entries, not one blank turn.
    let ctx = MessageContext::default();
    let empty: Vec<ChatMessage> = Vec::new();
    let names = tool_names_by_id(&empty);
    assert!(names.is_empty());

    for (_, profile) in every_declared_profile() {
        assert_eq!(
            empty
                .iter()
                .flat_map(|msg| encode_message(profile, msg, &ctx))
                .count(),
            0
        );
    }
}

#[test]
fn test_the_two_openai_families_differ_in_argument_encoding_and_result_key() {
    // The pair most likely to be conflated: both are `role:"tool"`, both use the
    // OpenAI function envelope, and the profile table separates them only by
    // `args_encoding`, `id_field` and `stream_keying`. The encoder must honour all
    // three of the request-side ones.
    assert_eq!(OPENAI_CHAT.tool_envelope, OLLAMA_NATIVE.tool_envelope);
    assert_eq!(
        OPENAI_CHAT.result_carrier, OLLAMA_NATIVE.result_carrier,
        "the two share a carrier name and nothing else"
    );

    let names: HashMap<String, String> = [("call_ab12".to_string(), "Read".to_string())].into();
    let ctx = MessageContext {
        tool_names: Some(&names),
        ..MessageContext::default()
    };
    let call = message("assistant", vec![tool_use("call_ab12", "Read")]);

    let chat = encode_message(OPENAI_CHAT, &call, &ctx);
    let native = encode_message(OLLAMA_NATIVE, &call, &ctx);
    assert!(
        chat[0]["tool_calls"][0]["function"]["arguments"].is_string(),
        "the chat family escapes the arguments into a string"
    );
    assert!(
        native[0]["tool_calls"][0]["function"]["arguments"].is_object(),
        "the native family sends the arguments as an object"
    );
}

#[test]
fn test_the_responses_client_body_uses_the_encoder_for_its_input_list() {
    // The case above drives the encoder directly, which proves the *carrier* exists
    // but not that the Responses client reaches it. This one drives the client's own
    // body builder, so the historic text-only filter - which dropped every tool part
    // and so dropped the whole tool exchange from the conversation - fails here.
    use ragent_llm::provider::openai_responses::ResponsesApiClient;

    let client = ResponsesApiClient::new("test-key", "https://api.openai.com");
    let request = ragent_llm::llm::ChatRequest {
        model: "gpt-5.6".to_string(),
        messages: Arc::new(conversation()),
        tools: Arc::new(Vec::new()),
        temperature: None,
        top_p: None,
        max_tokens: None,
        system: None,
        options: HashMap::new(),
        session_id: None,
        request_id: None,
        stream_timeout_secs: None,
        thinking: None,
    };

    let body = client
        .build_request_body(&request)
        .expect("the Responses body builds");
    let input = body["input"].as_array().expect("an input array");
    let kinds: Vec<&str> = input
        .iter()
        .filter_map(|item| item.get("type").and_then(Value::as_str))
        .collect();
    assert_eq!(
        kinds,
        vec!["function_call", "function_call_output"],
        "the Responses body must carry the tool exchange the conversation holds: \
         {input:?}"
    );
    // Entries: the user turn, the assistant turn's text item, the call, the output.
    let call = input
        .iter()
        .find(|item| item["type"] == "function_call")
        .expect("the call item");
    let output = input
        .iter()
        .find(|item| item["type"] == "function_call_output")
        .expect("the output item");
    assert_eq!(call["call_id"], "call_ab12");
    assert_eq!(output["call_id"], "call_ab12");
    assert_eq!(call["name"], "Read", "the call names the tool it invokes");
    assert_eq!(output["output"], "alpha");
}

#[test]
fn test_the_responses_client_refuses_to_build_a_body_it_would_shorten() {
    // FR-018 says "shall not issue the request", and the client's `chat` uses `?`
    // on this builder, so the refusal has to leave the *body builder* as an error
    // rather than as a body with fewer entries. The construct used here is a role
    // the input list has no item for, because it is the one a session can reach
    // without a provider that sends images.
    use ragent_llm::provider::openai_responses::ResponsesApiClient;

    let client = ResponsesApiClient::new("test-key", "https://api.openai.com");
    let mut messages = conversation();
    messages.push(text_message("tool", "orphan output"));
    let request = ragent_llm::llm::ChatRequest {
        model: "gpt-5.6".to_string(),
        messages: Arc::new(messages),
        tools: Arc::new(Vec::new()),
        temperature: None,
        top_p: None,
        max_tokens: None,
        system: None,
        options: HashMap::new(),
        session_id: None,
        request_id: None,
        stream_timeout_secs: None,
        thinking: None,
    };

    match client.build_request_body(&request) {
        Err(ragent_types::RagentError::WireLoss {
            ref construct,
            ref provider,
        }) => {
            assert_eq!(construct, "role:tool");
            assert_eq!(provider, "openai_responses");
        }
        other => panic!(
            "the Responses client must refuse a history it cannot carry whole \
             instead of sending the rest (FR-018): {other:?}"
        ),
    }

    // The same history with a role the input list accepts still builds, so the
    // refusal is the construct's and not a wider "always error" state.
    let ok_request = ragent_llm::llm::ChatRequest {
        model: "gpt-5.6".to_string(),
        messages: Arc::new(conversation()),
        ..request
    };
    assert!(
        client.build_request_body(&ok_request).is_ok(),
        "a carryable history still builds a body"
    );
}

// ---------------------------------------------------------------------------
// 4. The conversation-level form, and the dialects that died with the loops
// ---------------------------------------------------------------------------

/// The name of the provider whose OpenAI-compatible dialect the Copilot case
/// drives, so the body builder is exercised rather than a copy of it.
#[test]
fn test_encode_messages_equals_the_per_message_calls_for_every_profile() {
    // Six of the seven provider modules hand their whole history to the
    // conversation-level form with `MessageContext::default()`, so it must be
    // exactly the per-message loop it replaced - same entries, same order. A
    // family that started dropping or reordering a message here would change every
    // one of those requests at once.
    let request = conversation();
    for (provider, profile) in every_declared_profile() {
        let ctx = MessageContext::default();
        let mut expected: Vec<Value> = Vec::new();
        for msg in &request {
            expected.extend(encode_message(profile, msg, &ctx));
        }
        let actual = encode_messages(profile, &request, &ctx)
            .unwrap_or_else(|e| panic!("{provider}: the history encodes: {e}"));
        assert_eq!(
            actual, expected,
            "{provider}: the conversation form must be the per-message loop"
        );
        assert!(
            !actual.is_empty(),
            "{provider}: a conversation with a text turn encodes to something"
        );
    }
}

#[test]
fn test_a_refused_construct_refuses_the_whole_conversation() {
    // FR-018: the Responses input list has no image slot. The refusal must not
    // arrive as a shortened history - the caller may not issue that request - and
    // must not leak into the six families that carry images fine.
    let image = ChatMessage {
        role: "user".to_string(),
        content: ChatContent::Parts(vec![ContentPart::ImageUrl {
            url: "data:image/png;base64,AAA".to_string(),
        }]),
    };
    let mut history = conversation();
    history.push(image);

    let refused = encode_messages(OPENAI_RESPONSES, &history, &MessageContext::default());
    assert!(
        matches!(
            refused,
            Err(ragent_types::RagentError::WireLoss { ref construct, ref provider })
                if construct == "ImageUrl" && provider == "openai_responses"
        ),
        "the Responses input list must refuse an image rather than strip it: {refused:?}"
    );

    for profile in [
        OPENAI_CHAT,
        ANTHROPIC_MESSAGES,
        BEDROCK_CONVERSE,
        GEMINI,
        OLLAMA_NATIVE,
    ] {
        assert!(
            encode_messages(profile, &history, &MessageContext::default()).is_ok(),
            "a profile that carries images must not refuse the same history"
        );
    }
}

#[test]
fn test_a_role_the_responses_input_list_has_no_item_for_is_refused() {
    // The second construct the Responses input shape cannot carry is the message
    // *role*: it accepts `user`, `assistant`, `system` and `developer` and has no
    // item for anything else. `ChatMessage::role` is a `String`, so an unknown role
    // reaches the encoder, and the input list used to answer it by emitting nothing
    // - a silently shortened history, which is exactly what FR-018 forbids. The
    // refusal has to name the role and the provider so the failure says what was
    // lost and who could not carry it.
    let ctx = MessageContext::default();
    let mut history = conversation();
    history.push(text_message("tool", "orphan output"));

    let refused = encode_messages(OPENAI_RESPONSES, &history, &ctx);
    match refused {
        Err(ragent_types::RagentError::WireLoss {
            ref construct,
            ref provider,
        }) => {
            assert_eq!(
                construct, "role:tool",
                "the error must name the role it cannot carry"
            );
            assert_eq!(
                provider, "openai_responses",
                "the error must name the provider that cannot carry it"
            );
        }
        other => panic!(
            "a role the Responses input list has no item for must be refused rather \
             than dropped (FR-018): {other:?}"
        ),
    }

    // Every role the shape *does* have an item for still carries, so the refusal is
    // the shape's limit and not a blanket refusal of the conversation-level form.
    let carried = encode_messages(OPENAI_RESPONSES, &conversation(), &ctx)
        .expect("the canonical conversation carries");
    assert!(
        !carried.is_empty(),
        "the canonical history encodes to something"
    );
    for role in ["user", "assistant", "system", "developer"] {
        assert!(
            encode_message(OPENAI_RESPONSES, &text_message(role, "hi"), &ctx)[0]["role"] == role,
            "the Responses input list has an item for the `{role}` role"
        );
    }

    // And the other six families keep their own role policies: an unknown role is a
    // dropped turn there, which is what each of them already did with `system` or
    // `tool` before the encoder was shared. Widening this refusal to them would be a
    // behaviour change no requirement asks for.
    for profile in [
        OPENAI_CHAT,
        ANTHROPIC_MESSAGES,
        BEDROCK_CONVERSE,
        GEMINI,
        OLLAMA_NATIVE,
    ] {
        assert!(
            encode_messages(profile, &history, &MessageContext::default()).is_ok(),
            "only the Responses input list refuses an unknown role"
        );
    }
}

#[test]
fn test_a_single_text_part_collapses_and_an_image_never_does() {
    // The rule Copilot's `strict_single_text_collapse` flag used to choose between
    // is now the only rule: one *text* part collapses to a bare string, one image
    // part stays in its array. The old unconditional collapse in Copilot's copy
    // wrote `"content": null` here and lost the image.
    let ctx = MessageContext::default();
    let single_text = ChatMessage {
        role: "user".to_string(),
        content: ChatContent::Parts(vec![ContentPart::Text {
            text: "just text".to_string(),
        }]),
    };
    let single_image = ChatMessage {
        role: "user".to_string(),
        content: ChatContent::Parts(vec![ContentPart::ImageUrl {
            url: "data:image/png;base64,AAA".to_string(),
        }]),
    };

    let collapsed = encode_message(OPENAI_CHAT, &single_text, &ctx);
    assert_eq!(collapsed[0]["content"], "just text");

    let kept = encode_message(OPENAI_CHAT, &single_image, &ctx);
    assert!(
        kept[0]["content"].is_array(),
        "a single image part must stay in an array, not collapse to null: {kept:?}"
    );
    assert_eq!(kept[0]["content"][0]["type"], "image_url");
}

#[test]
fn test_copilot_and_openai_differ_only_in_stream_options() {
    // `strict_single_text_collapse` was Copilot's one dialect switch on the message
    // loop; with the loop shared there is nothing left to set, so the two specs
    // differ in the base body alone. Pin that, so a future field added here is a
    // deliberate dialect rather than a resurrected copy.
    use ragent_llm::provider::openai_compat::OpenAiCompatSpec;

    let openai = OpenAiCompatSpec::openai();
    let copilot = OpenAiCompatSpec::copilot();
    assert_eq!(openai.openai_envelope, copilot.openai_envelope);
    assert_eq!(openai.tool_result_name, copilot.tool_result_name);
    assert_ne!(
        openai.stream_options_in_base, copilot.stream_options_in_base,
        "stream_options is the one difference Copilot keeps"
    );
}

#[test]
fn test_a_local_ollama_result_carries_both_the_id_and_the_name() {
    // `mirror_tool_name` is the one context fact the OpenAI-compatible builder
    // still has to pass: local Ollama asks for the OpenAI `tool_call_id` and the
    // native `tool_name` on the same result message.
    use ragent_llm::provider::openai_compat::{OpenAiCompatSpec, build_openai_messages};

    let mut request = ChatRequest {
        model: "llama3".to_string(),
        messages: Arc::new(conversation()),
        tools: Arc::new(Vec::new()),
        temperature: None,
        top_p: None,
        max_tokens: None,
        system: None,
        options: HashMap::new(),
        session_id: None,
        request_id: None,
        stream_timeout_secs: None,
        thinking: None,
    };
    let mirroring = build_openai_messages(&request, &OpenAiCompatSpec::ollama());
    let plain = build_openai_messages(&request, &OpenAiCompatSpec::openai());

    let result = mirroring
        .iter()
        .find(|entry| entry["role"] == "tool")
        .expect("the result becomes its own message");
    assert_eq!(result["tool_call_id"], "call_ab12");
    assert_eq!(
        result["tool_name"], "Read",
        "local Ollama asks for the name beside the id"
    );
    let plain_result = plain
        .iter()
        .find(|entry| entry["role"] == "tool")
        .expect("the result becomes its own message");
    assert!(
        plain_result.get("tool_name").is_none(),
        "canonical OpenAI takes the id alone: {plain_result:?}"
    );

    // The system prompt is still this builder's own dialect, not the encoder's.
    let system: Arc<str> = Arc::from("be brief");
    request.system = Some(system);
    let with_system = build_openai_messages(&request, &OpenAiCompatSpec::openai());
    assert_eq!(with_system[0]["role"], "system");
    assert_eq!(with_system[0]["content"], "be brief");
}

#[test]
fn test_the_encoder_refuses_a_profile_its_family_table_does_not_cover() {
    // `MessageFamily::of` returns `None` for a profile combination no family
    // implements - a defect in the profile table rather than a request condition.
    // The encoder must omit the message and log, not guess a shape, and the
    // coverage case above is what fails the build for it.
    let invented = ToolWireProfile {
        result_carrier: ResultCarrier::GeminiFunctionResponse,
        ..OPENAI_CHAT
    };
    let message = text_message("user", "hello");
    let ctx = MessageContext::default();
    assert!(
        encode_message(invented, &message, &ctx).is_empty(),
        "an uncovered envelope/carrier pair writes nothing"
    );
    let empty: Vec<Value> = Vec::new();
    assert_eq!(
        encode_message_fallible(invented, &message, &ctx)
            .expect("omitting a message is not a wire loss"),
        empty,
        "an uncovered pair writes nothing"
    );
    // The real Gemini profile does cover its own carrier, so the case above is
    // about the combination and not about Gemini.
    assert_ne!(encode_message(GEMINI, &message, &ctx), empty);
}

// ---------------------------------------------------------------------------
// FR-011: reasoning echo (T-018)
// ---------------------------------------------------------------------------

/// One signed reasoning block, as the agent loop records it after the provider
/// streamed the thinking text and its signature.
fn signed_reasoning(text: &str, signature: &str) -> ContentPart {
    ContentPart::Reasoning {
        text: text.to_string(),
        signature: Some(signature.to_string()),
    }
}

/// FR-011: the assistant turn that answers a tool call must carry the signed
/// thinking block back, before the `tool_use` it decided.
///
/// Anthropic requires both halves: the reasoning text and the provider's
/// signature over it, in that order relative to the call.
#[test]
fn test_anthropic_replays_the_signed_thinking_block_before_the_call() {
    let turn = message(
        "assistant",
        vec![
            signed_reasoning("weigh the options", "EqQBCkYIBxgCKkA"),
            tool_use("call_ab12", "Read"),
        ],
    );
    let encoded = encode_message(ANTHROPIC_MESSAGES, &turn, &MessageContext::default());
    let blocks = encoded[0]["content"]
        .as_array()
        .expect("the assistant turn is a block list");
    assert_eq!(
        blocks[0],
        serde_json::json!({
            "type": "thinking",
            "thinking": "weigh the options",
            "signature": "EqQBCkYIBxgCKkA"
        }),
        "the signed block is replayed first, byte for byte"
    );
    assert_eq!(blocks[1]["type"], "tool_use");
    assert_eq!(
        blocks[1]["id"], "call_ab12",
        "the call the thinking decided follows it"
    );
}

/// FR-011's other half: a profile that declares `echo_reasoning == false`
/// carries on omitting reasoning, so this task moved no byte on those paths.
#[test]
fn test_a_profile_without_echo_reasoning_still_omits_the_block() {
    let turn = message(
        "assistant",
        vec![
            signed_reasoning("weigh the options", "EqQBCkYIBxgCKkA"),
            tool_use("call_ab12", "Read"),
        ],
    );
    for (label, profile) in every_declared_profile() {
        if profile.echo_reasoning {
            continue;
        }
        let encoded = encode_message(profile, &turn, &MessageContext::default());
        let rendered = serde_json::Value::Array(encoded).to_string();
        assert!(
            !rendered.contains("thinking") && !rendered.contains("weigh the options"),
            "{label}: reasoning is omitted without `echo_reasoning`: {rendered}"
        );
        assert!(
            rendered.contains("Read"),
            "{label}: omitting reasoning must not drop the call: {rendered}"
        );
    }
    // And the one family that does declare it writes the block, so the loop
    // above is about the declaration rather than about a broken fixture.
    let echoed = encode_message(ANTHROPIC_MESSAGES, &turn, &MessageContext::default());
    assert_eq!(echoed[0]["content"][0]["type"], "thinking");
}

/// A block recorded without a signature is replayed without the key: an empty
/// `signature` would claim the provider signed a block it did not (T-001's
/// omission rule, which FR-011's replay must preserve).
#[test]
fn test_an_unsigned_reasoning_block_is_replayed_without_a_signature() {
    let turn = message(
        "assistant",
        vec![
            ContentPart::Reasoning {
                text: "no signature here".to_string(),
                signature: None,
            },
            tool_use("call_ab12", "Read"),
        ],
    );
    let encoded = encode_message(ANTHROPIC_MESSAGES, &turn, &MessageContext::default());
    let block = &encoded[0]["content"][0];
    assert_eq!(block["type"], "thinking");
    assert_eq!(block["thinking"], "no signature here");
    assert!(
        block.get("signature").is_none(),
        "an unsigned block gains no `signature` key: {block}"
    );
}

/// The whole conversation form replays it too, so the provider module that calls
/// `encode_messages` gets the echo without a second implementation.
#[test]
fn test_the_conversation_encoder_replays_the_signed_block() {
    let mut messages = conversation();
    messages[1] = message(
        "assistant",
        vec![
            signed_reasoning("reading notes", "sig-1"),
            ContentPart::Text {
                text: "reading".to_string(),
            },
            tool_use("call_ab12", "Read"),
        ],
    );
    let encoded = encode_messages(ANTHROPIC_MESSAGES, &messages, &MessageContext::default())
        .expect("a signed block is a construct Anthropic carries");
    let assistant = encoded
        .iter()
        .find(|entry| entry["role"] == "assistant")
        .expect("the assistant turn");
    assert_eq!(
        assistant["content"][0],
        serde_json::json!({
            "type": "thinking",
            "thinking": "reading notes",
            "signature": "sig-1"
        })
    );
    assert_eq!(assistant["content"][2]["type"], "tool_use");
}
