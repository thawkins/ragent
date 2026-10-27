//! The two canonical `ContentPart` additions of spec `uniformtools` T-001.
//!
//! [`ContentPart`] is the provider-neutral vocabulary every provider encoder
//! matches on (spec `uniformtools` D1), so both additions are type-level
//! contracts the later wire tasks read:
//!
//! * `ToolResult::is_error` - FR-014 requires every tool result to carry the
//!   flag. A profile that declares `supports_is_error` transmits it natively;
//!   every other profile's encoder derives its `ERROR: ` prefix from it. The
//!   flag therefore has to survive the JSON round trip *and* has to be written
//!   even when it is `false` (an absent flag and a `false` flag would otherwise
//!   be indistinguishable on the wire).
//! * `ContentPart::Reasoning` - FR-011 requires the signed reasoning block to be
//!   replayable for profiles that declare `echo_reasoning`. The signature is
//!   optional because only some providers supply one.
//!
//! These cases pin the serde shape only. Transmitting the flag (FR-014) is
//! T-017 and replaying the reasoning block (FR-011) is T-018; this task owns
//! the representation those two consume.

use ragent_types::llm::ContentPart;
use std::sync::Arc;

/// Serialise a part to a JSON object, asserting the variant is an object (every
/// `ContentPart` is internally tagged by `type`).
fn json_of(part: &ContentPart) -> serde_json::Map<String, serde_json::Value> {
    let value = serde_json::to_value(part).expect("serialize");
    match value {
        serde_json::Value::Object(map) => map,
        other => panic!("a ContentPart must be a JSON object, got {other:?}"),
    }
}

/// A tool result whose recorded status was an error, as history builds it.
fn error_result() -> ContentPart {
    ContentPart::ToolResult {
        tool_use_id: "call-1".to_string(),
        content: Arc::from("no such file: missing-file.txt"),
        is_error: true,
    }
}

/// FR-014: the error flag is transmitted, not inferred from the content.
///
/// A false flag is written too: the encoder reads the field, so a successful
/// result that omitted it would make "absent" and "false" two spellings of the
/// same state and let a strict reader treat a success as an error.
#[test]
fn tool_result_error_flag_reaches_the_wire_in_both_states() {
    let failed = json_of(&error_result());
    assert_eq!(failed["type"], "tool_result");
    assert_eq!(
        failed["is_error"],
        serde_json::json!(true),
        "an error result must carry the flag (FR-014)"
    );

    let succeeded = json_of(&ContentPart::ToolResult {
        tool_use_id: "call-1".to_string(),
        content: Arc::from("no such file: missing-file.txt"),
        is_error: false,
    });
    assert_eq!(
        succeeded["is_error"],
        serde_json::json!(false),
        "a success is written as an explicit false, never omitted"
    );

    // The flag is the only difference: nothing about the content changes.
    assert_eq!(failed["content"], succeeded["content"]);
}

/// The flag is a real field, not a presentation detail: both states round-trip.
#[test]
fn tool_result_error_flag_round_trips_both_states() {
    for is_error in [true, false] {
        let part = ContentPart::ToolResult {
            tool_use_id: "call-7".to_string(),
            content: Arc::from("body"),
            is_error,
        };
        let text = serde_json::to_string(&part).expect("serialize");
        let back: ContentPart = serde_json::from_str(&text).expect("deserialize");
        match back {
            ContentPart::ToolResult {
                tool_use_id,
                content,
                is_error: decoded,
            } => {
                assert_eq!(tool_use_id, "call-7");
                assert_eq!(&*content, "body");
                assert_eq!(decoded, is_error, "the flag survives the round trip");
            }
            other => panic!("expected ToolResult, got {other:?}"),
        }
    }
}

/// FR-011: the reasoning part carries the block text and the provider's
/// signature so a profile with `echo_reasoning` can replay the block verbatim.
#[test]
fn reasoning_part_carries_text_and_signature() {
    let part = ContentPart::Reasoning {
        text: "the file is probably absent, so read will fail".to_string(),
        signature: Some("sig-abc123".to_string()),
    };
    let json = json_of(&part);
    assert_eq!(json["type"], "reasoning");
    assert_eq!(
        json["text"],
        serde_json::json!("the file is probably absent, so read will fail")
    );
    assert_eq!(json["signature"], serde_json::json!("sig-abc123"));

    let back: ContentPart =
        serde_json::from_value(serde_json::Value::Object(json)).expect("deserialize");
    match back {
        ContentPart::Reasoning { text, signature } => {
            assert_eq!(text, "the file is probably absent, so read will fail");
            assert_eq!(signature.as_deref(), Some("sig-abc123"));
        }
        other => panic!("expected Reasoning, got {other:?}"),
    }
}

/// The signature is optional: a provider that supplies none is replayed without
/// one, and the key is omitted rather than written as `null` so the wire shape
/// of a signature-less block is the shape it had before the field existed.
#[test]
fn reasoning_part_without_a_signature_omits_the_key() {
    let part = ContentPart::Reasoning {
        text: "hmm".to_string(),
        signature: None,
    };
    let json = json_of(&part);
    assert_eq!(json["type"], "reasoning");
    assert_eq!(json["text"], serde_json::json!("hmm"));
    assert!(
        !json.contains_key("signature"),
        "a missing signature is omitted, not serialised as null: {json:?}"
    );

    // A payload that names no signature decodes to `None` - including one
    // recorded before the field existed.
    let without: ContentPart = serde_json::from_value(serde_json::json!({
        "type": "reasoning",
        "text": "hmm",
    }))
    .expect("deserialize without signature");
    let ContentPart::Reasoning { text, signature } = without else {
        panic!("expected Reasoning");
    };
    assert_eq!(text, "hmm");
    assert!(signature.is_none(), "an absent signature is None");

    // An explicit null means the same thing.
    let explicit: ContentPart = serde_json::from_value(serde_json::json!({
        "type": "reasoning",
        "text": "hmm",
        "signature": null,
    }))
    .expect("deserialize with a null signature");
    let ContentPart::Reasoning { signature, .. } = explicit else {
        panic!("expected Reasoning");
    };
    assert!(signature.is_none());
}

/// Reasoning is its own part, not a `text` block: the encoders dispatch on the
/// tag, so a reasoning block that serialised as `text` would be sent to every
/// provider as assistant output.
#[test]
fn reasoning_is_not_the_text_part() {
    let reasoning = json_of(&ContentPart::Reasoning {
        text: "same words".to_string(),
        signature: None,
    });
    let text = json_of(&ContentPart::Text {
        text: "same words".to_string(),
    });
    assert_eq!(reasoning["text"], text["text"]);
    assert_ne!(
        reasoning["type"], text["type"],
        "a reasoning block must not be indistinguishable from assistant text"
    );
}

/// D1's vocabulary is closed: the two additions complete a five-part set, and
/// each tag is the exact string every provider encoder matches on. A sixth
/// variant, or a renamed tag, fails here before a provider request does.
#[test]
fn the_canonical_part_vocabulary_is_five_tags() {
    let parts = [
        ContentPart::Text {
            text: String::new(),
        },
        ContentPart::ToolUse {
            id: String::new(),
            name: String::new(),
            input: serde_json::json!({}),
        },
        ContentPart::ToolResult {
            tool_use_id: String::new(),
            content: Arc::from(""),
            is_error: false,
        },
        ContentPart::ImageUrl { url: String::new() },
        ContentPart::Reasoning {
            text: String::new(),
            signature: None,
        },
    ];
    let tags: Vec<String> = parts
        .iter()
        .map(|part| {
            json_of(part)["type"]
                .as_str()
                .map(str::to_string)
                .expect("every part is tagged with a string")
        })
        .collect();
    assert_eq!(
        tags,
        vec![
            "text".to_string(),
            "tool_use".to_string(),
            "tool_result".to_string(),
            "image_url".to_string(),
            "reasoning".to_string(),
        ],
        "the canonical vocabulary and its wire tags (spec D1)"
    );
}
