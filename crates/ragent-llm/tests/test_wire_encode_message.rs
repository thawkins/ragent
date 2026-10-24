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
//!    Ollama's name-keyed result and image rules, and the Responses input list that
//!    carries tool history at all (FR-018's dropping behaviour is what T-007
//!    replaces; this file only pins that the carrier is now *reached*).

use std::collections::HashMap;
use std::sync::Arc;

use ragent_llm::llm::{ChatContent, ChatMessage, ContentPart};
use ragent_llm::provider::wire::encode::{MessageContext, encode_message, tool_names_by_id};
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
    assert_eq!(result[0]["tool_call_id"], "call_ab12");
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
    assert_eq!(
        with[0]["tool_call_id"], "call_ab12",
        "the id is still written: the name is a mirror, not a replacement"
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
fn test_responses_input_carries_tool_history_instead_of_only_text() {
    // FR-018's subject is that the Responses path used to drop every non-text part.
    // The carrier now reaches the wire; refusing a construct it cannot carry is
    // T-007's `WireLoss` change, so this case asserts presence, not policy.
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
            assert!(
                !source.contains(key),
                "{} still writes {key}, which is {what}; every request-side carrier \
                 belongs to `wire::encode::encode_message` (FR-006)",
                path.display()
            );
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
