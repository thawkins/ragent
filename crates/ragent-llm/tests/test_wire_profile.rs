//! Tests for the declared per-provider tool-calling wire contract (FR-002).
//!
//! These pin the nine profile fields as a contract rather than as a set of
//! incidental enums: every field is asserted to be *readable* (a non-blank
//! ASCII label, for the wire matrix of FR-016) and the shape of a realistic
//! profile is pinned for one provider family per carrier, so a later encoder
//! task cannot silently redefine what "OpenAI" or "Anthropic" means here.
//!
//! The family profiles below are functions rather than `const`s on purpose: a
//! `const` would let the compiler fold every assertion to a literal, which is
//! both a lint hazard and a weaker test. The one `const` case keeps its own
//! test, so `const`-constructibility is still proven.
//!
//! No provider is contacted and no registry is built: the profile is a plain
//! value, which is what NFR-004 asks for.

use ragent_llm::provider::wire::{
    ArgsEncoding, IdField, ResultCarrier, StreamKeying, ToolEnvelope, ToolWireProfile,
};

/// A representative OpenAI-chat-completions profile.
fn openai() -> ToolWireProfile {
    ToolWireProfile {
        tool_envelope: ToolEnvelope::OpenAiFunction,
        result_carrier: ResultCarrier::ToolRoleMessage,
        id_field: IdField::Key("tool_call_id"),
        args_encoding: ArgsEncoding::JsonString,
        stream_keying: StreamKeying::ByIndex,
        supports_parallel: true,
        supports_is_error: false,
        echo_reasoning: false,
        synthetic_ids: false,
    }
}

/// A representative Anthropic Messages profile.
fn anthropic() -> ToolWireProfile {
    ToolWireProfile {
        tool_envelope: ToolEnvelope::AnthropicTool,
        result_carrier: ResultCarrier::UserToolResultBlock,
        id_field: IdField::Key("tool_use_id"),
        args_encoding: ArgsEncoding::JsonObject,
        stream_keying: StreamKeying::ByIndex,
        supports_parallel: true,
        supports_is_error: true,
        echo_reasoning: true,
        synthetic_ids: false,
    }
}

/// A representative Gemini profile - the one family that supplies no ids.
fn gemini() -> ToolWireProfile {
    ToolWireProfile {
        tool_envelope: ToolEnvelope::GeminiFunctionDeclaration,
        result_carrier: ResultCarrier::GeminiFunctionResponse,
        id_field: IdField::Synthesised,
        args_encoding: ArgsEncoding::JsonObject,
        stream_keying: StreamKeying::ByIndex,
        supports_parallel: true,
        supports_is_error: false,
        echo_reasoning: false,
        synthetic_ids: true,
    }
}

/// A representative Responses-API profile - the id-keyed stream family.
fn responses() -> ToolWireProfile {
    ToolWireProfile {
        tool_envelope: ToolEnvelope::ResponsesFlatFunction,
        result_carrier: ResultCarrier::ResponsesFunctionCallOutput,
        id_field: IdField::Key("call_id"),
        args_encoding: ArgsEncoding::JsonString,
        stream_keying: StreamKeying::ByCallId,
        supports_parallel: true,
        supports_is_error: false,
        echo_reasoning: false,
        synthetic_ids: false,
    }
}

/// A representative Bedrock Converse profile - the `toolSpec` family.
fn bedrock() -> ToolWireProfile {
    ToolWireProfile {
        tool_envelope: ToolEnvelope::BedrockToolSpec,
        result_carrier: ResultCarrier::UserToolResultBlock,
        id_field: IdField::Key("toolUseId"),
        args_encoding: ArgsEncoding::JsonObject,
        stream_keying: StreamKeying::ByIndex,
        supports_parallel: true,
        supports_is_error: true,
        echo_reasoning: false,
        synthetic_ids: false,
    }
}

#[test]
fn test_wire_profile_every_envelope_has_a_non_blank_label() {
    let table = [
        (ToolEnvelope::OpenAiFunction, "openai_function"),
        (ToolEnvelope::AnthropicTool, "anthropic_tool"),
        (
            ToolEnvelope::GeminiFunctionDeclaration,
            "gemini_function_declaration",
        ),
        (ToolEnvelope::BedrockToolSpec, "bedrock_tool_spec"),
        (
            ToolEnvelope::ResponsesFlatFunction,
            "responses_flat_function",
        ),
    ];
    let mut seen: Vec<&str> = Vec::with_capacity(table.len());
    for (envelope, expected) in table {
        let label = envelope.as_str();
        // Asserting the exact label proves both "not blank" and "printable".
        assert_eq!(label, expected, "{envelope:?} label drifted");
        assert!(
            label.is_ascii(),
            "{envelope:?} label is not ASCII (NFR-005)"
        );
        assert!(!seen.contains(&label), "{envelope:?} label is a duplicate");
        seen.push(label);
    }
}

#[test]
fn test_wire_profile_every_carrier_has_a_non_blank_label() {
    let table = [
        (ResultCarrier::ToolRoleMessage, "tool_role_message"),
        (ResultCarrier::UserToolResultBlock, "user_tool_result_block"),
        (
            ResultCarrier::GeminiFunctionResponse,
            "gemini_function_response",
        ),
        (
            ResultCarrier::ResponsesFunctionCallOutput,
            "responses_function_call_output",
        ),
    ];
    let mut seen: Vec<&str> = Vec::with_capacity(table.len());
    for (carrier, expected) in table {
        let label = carrier.as_str();
        assert_eq!(label, expected, "{carrier:?} label drifted");
        assert!(label.is_ascii(), "{carrier:?} label is not ASCII (NFR-005)");
        assert!(!seen.contains(&label), "{carrier:?} label is a duplicate");
        seen.push(label);
    }
}

#[test]
fn test_wire_profile_id_field_never_labels_blank() {
    assert_eq!(IdField::Key("tool_call_id").as_str(), "tool_call_id");
    assert_eq!(IdField::Key("tool_use_id").as_str(), "tool_use_id");
    assert_eq!(IdField::Key("call_id").as_str(), "call_id");
    // A synthesising family still has a word to print, never an empty cell.
    assert_eq!(IdField::Synthesised.as_str(), "synthesised");
    assert!(IdField::Synthesised.as_str().is_ascii());
}

#[test]
fn test_wire_profile_every_encoding_and_keying_has_a_non_blank_label() {
    assert_eq!(ArgsEncoding::JsonString.as_str(), "json_string");
    assert_eq!(ArgsEncoding::JsonObject.as_str(), "json_object");
    assert_eq!(StreamKeying::ByIndex.as_str(), "by_index");
    assert_eq!(StreamKeying::ByCallId.as_str(), "by_call_id");

    for label in [
        ArgsEncoding::JsonString.as_str(),
        ArgsEncoding::JsonObject.as_str(),
        StreamKeying::ByIndex.as_str(),
        StreamKeying::ByCallId.as_str(),
    ] {
        assert!(label.is_ascii(), "{label} is not ASCII (NFR-005)");
    }
}

#[test]
fn test_wire_profile_declares_all_nine_contract_decisions() {
    // Reading every field of one profile is the test: if a field were dropped
    // from the struct this would not compile, and if a field were added
    // without a decision this test would still pass only once it is stated.
    let profile = anthropic();
    assert_eq!(profile.tool_envelope, ToolEnvelope::AnthropicTool);
    assert_eq!(profile.result_carrier, ResultCarrier::UserToolResultBlock);
    assert_eq!(profile.id_field, IdField::Key("tool_use_id"));
    assert_eq!(profile.args_encoding, ArgsEncoding::JsonObject);
    assert_eq!(profile.stream_keying, StreamKeying::ByIndex);

    // The four boolean decisions, each stated rather than folded into the
    // struct's `Debug`: parallel calls, native is_error, reasoning echo and
    // synthetic ids.
    assert!(profile.supports_parallel, "parallel calls supported");
    assert!(profile.supports_is_error, "tool_result.is_error is native");
    assert!(profile.echo_reasoning, "signed thinking is replayed");
    assert!(
        !profile.synthetic_ids,
        "tool_use.id is supplied by the model"
    );
}

#[test]
fn test_wire_profile_families_differ_only_where_they_must() {
    // The three axes that actually diverge between the two mainstream
    // families. A profile matching another on all of them would be a
    // duplicate family rather than a second wire contract.
    assert_ne!(openai().tool_envelope, anthropic().tool_envelope);
    assert_ne!(openai().result_carrier, anthropic().result_carrier);
    assert_ne!(openai().args_encoding, anthropic().args_encoding);

    // The Responses path is the only id-keyed stream family.
    assert_eq!(responses().stream_keying, StreamKeying::ByCallId);
    for other in [openai(), anthropic(), gemini(), bedrock()] {
        assert_eq!(other.stream_keying, StreamKeying::ByIndex);
    }

    // Gemini is the only family whose ids must be synthesised, and the flag
    // and the id field agree with each other (FR-019).
    assert!(gemini().synthetic_ids);
    assert_eq!(gemini().id_field, IdField::Synthesised);
    for other in [openai(), anthropic(), responses(), bedrock()] {
        assert!(!other.synthetic_ids, "{other:?} supplies its own ids");
        assert!(
            matches!(other.id_field, IdField::Key(_)),
            "{other:?} must name an id key"
        );
    }
}

#[test]
fn test_wire_profile_error_flag_and_reasoning_are_declared_per_family() {
    // FR-014: the families that cannot transmit is_error get the textual
    // marker instead, so the flag must be an explicit per-family declaration.
    assert!(anthropic().supports_is_error, "Anthropic: native is_error");
    assert!(bedrock().supports_is_error, "Bedrock: native is_error");
    for other in [openai(), gemini(), responses()] {
        assert!(
            !other.supports_is_error,
            "{other:?} needs the ERROR: marker"
        );
    }

    // FR-011: only the signed-thinking family replays reasoning.
    assert!(anthropic().echo_reasoning, "replays signed thinking");
    for other in [openai(), gemini(), responses(), bedrock()] {
        assert!(!other.echo_reasoning, "{other:?} omits reasoning");
    }
}

#[test]
fn test_wire_profile_is_const_constructible_and_copy() {
    // FR-002 requires one declared profile per family; a `const` that is
    // `Copy` is what lets a client hold it as a field and hand it to the
    // encoders without allocating or borrowing.
    const OPENAI: ToolWireProfile = ToolWireProfile {
        tool_envelope: ToolEnvelope::OpenAiFunction,
        result_carrier: ResultCarrier::ToolRoleMessage,
        id_field: IdField::Key("tool_call_id"),
        args_encoding: ArgsEncoding::JsonString,
        stream_keying: StreamKeying::ByIndex,
        supports_parallel: true,
        supports_is_error: false,
        echo_reasoning: false,
        synthetic_ids: false,
    };

    let copied = OPENAI;
    assert_eq!(copied, OPENAI);
    let envelope = |profile: ToolWireProfile| profile.tool_envelope;
    // Passing a copy, then reading `OPENAI` again, proves `Copy` not `Move`.
    assert_eq!(envelope(copied), ToolEnvelope::OpenAiFunction);
    assert_eq!(OPENAI.tool_envelope, ToolEnvelope::OpenAiFunction);
    // The runtime-built profile agrees with the `const` one field for field.
    assert_eq!(copied, openai());
}
