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
//!
//! Three cases pin properties of the *declaration* rather than of a value,
//! because they are compile-time properties of the type: that there is no
//! `Default` and no optional field anywhere a client could inherit a contract
//! from (FR-002's "a client constructed without a profile **shall** fail to
//! compile"), that the declared field set is exactly the nine the requirement
//! names, and that the derives a client depends on (`Copy`, `Eq`, `Hash`,
//! `Send`, `Sync`) are present and behave. `tests/test_wire_profile_table.rs`
//! records that it deliberately does not assert the first of these; this is
//! where it is asserted.

use ragent_llm::provider::wire::{
    ArgsEncoding, IdField, ResultCarrier, StreamKeying, ToolEnvelope, ToolWireProfile,
};

/// The source text of `wire/mod.rs`, which is where the contract is declared.
fn profile_declaration() -> String {
    use std::path::PathBuf;

    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("providers")
        .join("wire")
        .join("mod.rs");
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

/// The body of the `ToolWireProfile` struct (between its braces).
fn profile_struct_body(source: &str) -> &str {
    let start = source
        .find("pub struct ToolWireProfile {")
        .expect("ToolWireProfile must be declared in wire/mod.rs");
    let rest = &source[start..];
    let end = rest
        .find("\n}")
        .expect("the ToolWireProfile declaration must close at column 0");
    &rest[..end]
}

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

#[test]
fn test_wire_profile_declares_exactly_the_nine_named_decisions() {
    // FR-002 names the nine decisions the profile must cover. This reads the
    // declaration and pins the field set exactly: a field added without a
    // decision, or one of the nine silently dropped, fails here rather than in
    // whichever encoder later reads a field that no longer exists.
    let source = profile_declaration();
    let body = profile_struct_body(&source);

    let mut declared: Vec<&str> = Vec::new();
    for line in body.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("pub ") else {
            continue;
        };
        let Some((name, _)) = rest.split_once(':') else {
            continue;
        };
        declared.push(name.trim());
    }

    assert_eq!(
        declared,
        vec![
            "tool_envelope",
            "result_carrier",
            "id_field",
            "args_encoding",
            "stream_keying",
            "supports_parallel",
            "supports_is_error",
            "echo_reasoning",
            "synthetic_ids",
        ],
        "FR-002 names exactly these nine decisions, in this order"
    );

    // Every field is required: an `Option<...>` would let a client omit a
    // decision and inherit a plausible default, which is the one thing FR-002
    // forbids.
    assert!(
        !body.contains("Option<"),
        "no profile field may be optional - a client must state every decision (FR-002)"
    );
    // A profile is a contract, not configuration: nothing here is deserialised
    // from `ragent.json`, so no serde attribute may appear on it.
    assert!(
        !body.contains("serde"),
        "a profile is a declared contract, not a config value (FR-002)"
    );

    // Each field's own documentation names the requirement that consumes it, so
    // the declaration stays a map of the feature rather than a bag of flags.
    for requirement in [
        "FR-003", "FR-004", "FR-006", "FR-010", "FR-011", "FR-014", "FR-019",
    ] {
        assert!(
            body.contains(requirement),
            "no field documents {requirement}; every decision names the requirement it serves"
        );
    }
}

#[test]
fn test_wire_profile_has_no_default_impl_anywhere_in_the_crate() {
    // FR-002's compile-time clause, as far as a test can reach it: a client
    // constructed without a profile must fail to compile, which holds only while
    // no defaulted profile exists for such a client to fall back on. The
    // declaration is scanned for a `Default` derive and every source file in the
    // crate for an `impl Default for ToolWireProfile` or a
    // `ToolWireProfile::default()`, so the escape cannot be added quietly in a
    // later task.
    use std::path::{Path, PathBuf};

    let source = profile_declaration();
    let struct_at = source
        .find("pub struct ToolWireProfile {")
        .expect("the declaration must exist");
    // The attributes immediately above the declaration, i.e. the contiguous run
    // of `#[...]` lines after the last doc comment. Comments are skipped because
    // the declaration's own documentation talks about the `Default` impl it does
    // not have.
    let derive_line = source[..struct_at]
        .lines()
        .rev()
        .map(str::trim)
        .take_while(|line| line.starts_with("#["))
        .find(|line| line.starts_with("#[derive("))
        .expect("the declaration must carry a derive list");
    assert!(
        !derive_line.contains("Default"),
        "ToolWireProfile must not derive Default - a defaulted profile is a silent, \
         wrong contract for every client that forgot to declare one (FR-002); got \
         {derive_line:?}"
    );

    fn rust_sources(dir: &Path, found: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                rust_sources(&path, found);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                found.push(path);
            }
        }
    }

    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut sources = Vec::new();
    rust_sources(&src, &mut sources);
    assert!(!sources.is_empty(), "the crate must contain source files");

    for path in sources {
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        for retired in [
            "impl Default for ToolWireProfile",
            "ToolWireProfile::default()",
        ] {
            assert!(
                !text.contains(retired),
                "{} introduces `{retired}`; a client without a profile must fail to \
                 compile rather than inherit one (FR-002)",
                path.display()
            );
        }
    }
}

#[test]
fn test_wire_profile_is_send_sync_static_and_fully_derived() {
    // A profile is held as a field of a client, and `LlmClient: Send + Sync`, so
    // the value has to be shareable across the async boundary and live as long as
    // the client. The bounds are checked by the compiler here rather than assumed
    // from the field types, which is what makes them a contract.
    fn assert_shareable<T: Send + Sync + 'static>() {}
    fn assert_copy_eq_hash<T: Copy + Eq + std::hash::Hash + std::fmt::Debug>() {}
    assert_shareable::<ToolWireProfile>();
    assert_copy_eq_hash::<ToolWireProfile>();

    // The derives the bounds above stand on, read from the declaration: a
    // field-carrying struct cannot gain these by accident, and losing one is a
    // compile error in `assert_shareable` only after the fact.
    let source = profile_declaration();
    let struct_at = source
        .find("pub struct ToolWireProfile {")
        .expect("the declaration must exist");
    let derives: &str = &source[..struct_at];
    let derive_line = derives
        .lines()
        .rev()
        .find(|line| line.trim_start().starts_with("#[derive("))
        .expect("ToolWireProfile must carry a derive list");
    for expected in ["Debug", "Clone", "Copy", "PartialEq", "Eq", "Hash"] {
        assert!(
            derive_line.contains(expected),
            "ToolWireProfile must derive {expected}; got {derive_line:?}"
        );
    }

    // `Hash` is what lets a profile key a map, and two equal profiles must hash
    // equally - the property a cache keyed by profile depends on.
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let hash_of = |profile: ToolWireProfile| {
        let mut hasher = DefaultHasher::new();
        profile.hash(&mut hasher);
        hasher.finish()
    };
    assert_eq!(hash_of(openai()), hash_of(openai()));
    assert_ne!(
        hash_of(openai()),
        hash_of(anthropic()),
        "two different contracts must not collide in a profile-keyed cache"
    );
}

#[test]
fn test_wire_profile_representatives_match_the_declared_table_constants() {
    // The six representative profiles above are this file's own copies, which is
    // what lets the cases read as statements about a *family* rather than about
    // the table. That freedom is only sound while each copy still equals the
    // constant T-003 declares for it: a drifted copy would make every assertion
    // in this file true of a contract nothing speaks. This case is the binding.
    use ragent_llm::provider::wire::profile_table::{
        ANTHROPIC_MESSAGES, BEDROCK_CONVERSE, GEMINI, OPENAI_CHAT, OPENAI_RESPONSES,
    };

    for (label, local, declared) in [
        ("openai", openai(), OPENAI_CHAT),
        ("anthropic", anthropic(), ANTHROPIC_MESSAGES),
        ("gemini", gemini(), GEMINI),
        ("responses", responses(), OPENAI_RESPONSES),
        ("bedrock", bedrock(), BEDROCK_CONVERSE),
    ] {
        assert_eq!(
            local, declared,
            "the `{label}` representative in this file has drifted from the declared \
             family constant; every assertion in this file is about the copy"
        );
    }

    // The declared table states the same agreement between `synthetic_ids` and
    // `id_field` that the family copy asserts (FR-019): a profile that says it
    // supplies no ids must not also name a key to write them under, and vice
    // versa. Checked over the table so a new row cannot declare the pair
    // inconsistently.
    use ragent_llm::provider::wire::profile_table::PROVIDER_WIRE;
    for (id, profile) in PROVIDER_WIRE {
        let synthesises = matches!(profile.id_field, IdField::Synthesised);
        assert_eq!(
            profile.synthetic_ids, synthesises,
            "{id:?}: `synthetic_ids` and `id_field` must agree - a provider that omits \
             ids cannot name a key, and one that names a key cannot be id-less (FR-019)"
        );
    }
}
