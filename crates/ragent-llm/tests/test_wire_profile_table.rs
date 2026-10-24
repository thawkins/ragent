//! Coverage test for the provider-to-profile table (FR-002).
//!
//! FR-002 has two halves. This file tests the second: a test **shall** assert
//! that the set of profiles covers every registered provider id. The first half -
//! "a client constructed without a profile shall fail to compile" - is not
//! asserted here, because no client holds a profile yet: T-003 declares the
//! contract and the table, and the encoder tasks (T-004 onward) make each client
//! take one. What makes that later step safe is the type: `ToolWireProfile` has
//! no `Default` impl and no optional constructor, so when a client does take a
//! profile there is no fallback contract it could silently inherit.
//!
//! The assertion is deliberately one-directional in the dangerous direction and
//! two-directional in the safe one:
//!
//! - every id `create_default_registry()` registers has a [`PROVIDER_WIRE`] row
//!   - this is the case that fails when someone adds a provider;
//! - every [`PROVIDER_WIRE`] row names an id that is registered - so a renamed
//!   or removed provider cannot leave a dead row behind claiming a contract
//!   nothing speaks.
//!
//! The remaining cases pin the properties that make the table a *table* rather
//! than fifteen copies of the same struct: the ids are unique and ordered, the
//! lookup agrees with the slice, every family constant is present, and the
//! profiles differ on the axes the tree actually diverges on.
//!
//! No provider is contacted and no session is created: the registry is built
//! offline (its constructors read environment variables only to *resolve* a
//! base URL, and none is asked to create a client here), which is what NFR-004
//! asks for.

use std::collections::HashSet;

use ragent_llm::provider::ProviderRegistry;
use ragent_llm::provider::create_default_registry;
use ragent_llm::provider::wire::profile_table::{
    ANTHROPIC_MESSAGES, BEDROCK_CONVERSE, GEMINI, OLLAMA_NATIVE, OPENAI_CHAT, OPENAI_RESPONSES,
    PROVIDER_WIRE, profile_for,
};
use ragent_llm::provider::wire::{
    ArgsEncoding, IdField, ResultCarrier, StreamKeying, ToolEnvelope, ToolWireProfile,
};

#[test]
fn test_wire_profile_table_covers_every_registered_provider() {
    let registry: ProviderRegistry = create_default_registry();
    let registered: HashSet<String> = registry.list().into_iter().map(|p| p.id).collect();
    assert!(
        !registered.is_empty(),
        "the default registry must register at least one provider"
    );

    for id in &registered {
        assert!(
            profile_for(id).is_some(),
            "FR-002: provider {id:?} is registered but declares no ToolWireProfile; \
             add a row to PROVIDER_WIRE in crates/ragent-llm/src/providers/wire/profile_table.rs"
        );
    }
}

#[test]
fn test_wire_profile_table_has_no_row_for_an_unregistered_provider() {
    let registry = create_default_registry();
    let registered: HashSet<String> = registry.list().into_iter().map(|p| p.id).collect();
    for (id, _) in PROVIDER_WIRE {
        assert!(
            registered.contains(*id),
            "PROVIDER_WIRE names {id:?} but no provider registers that id; \
             remove the row or register the provider"
        );
    }
}

#[test]
fn test_wire_profile_table_and_registry_have_the_same_size() {
    // Stated separately from the two subset checks above so a failure reports
    // the counts, which is the fastest way to see which side changed.
    let registry = create_default_registry();
    let registered = registry.list().len();
    assert_eq!(
        PROVIDER_WIRE.len(),
        registered,
        "PROVIDER_WIRE has {} rows but the registry registers {registered} providers",
        PROVIDER_WIRE.len()
    );
}

#[test]
fn test_wire_profile_table_ids_are_unique_and_ascending() {
    let mut seen: HashSet<&str> = HashSet::with_capacity(PROVIDER_WIRE.len());
    for (id, _) in PROVIDER_WIRE {
        assert!(seen.insert(id), "PROVIDER_WIRE repeats the id {id:?}");
    }
    for window in PROVIDER_WIRE.windows(2) {
        assert!(
            window[0].0 < window[1].0,
            "PROVIDER_WIRE must be ascending by provider id, but {:?} precedes {:?}",
            window[0].0,
            window[1].0
        );
    }
}

#[test]
fn test_wire_profile_table_lookup_agrees_with_the_slice() {
    for (id, profile) in PROVIDER_WIRE {
        assert_eq!(
            profile_for(id),
            Some(*profile),
            "profile_for({id:?}) disagrees with the PROVIDER_WIRE row"
        );
    }
    // No catch-all: an unknown id gets `None`, never a defaulted contract.
    assert_eq!(profile_for(""), None);
    assert_eq!(profile_for("no_such_provider"), None);
    // The lookup is case-sensitive, like the registry key it mirrors.
    assert_eq!(profile_for("Anthropic"), None);
}

#[test]
fn test_wire_profile_table_declares_all_six_families() {
    // Every family constant must be a row of the table; a constant nothing
    // names would be dead contract, and the wire matrix would never print it.
    let named = [
        OPENAI_CHAT,
        ANTHROPIC_MESSAGES,
        GEMINI,
        BEDROCK_CONVERSE,
        OPENAI_RESPONSES,
        OLLAMA_NATIVE,
    ];
    for (index, family) in named.iter().enumerate() {
        assert!(
            PROVIDER_WIRE.iter().any(|(_, p)| p == family),
            "family constant {index} ({:?} / {:?}) is not named by any PROVIDER_WIRE row",
            family.tool_envelope,
            family.result_carrier
        );
    }

    // The six constants are six *distinct* contracts: no two agree on all nine
    // fields, or the second would be a duplicate family rather than a family.
    for (i, a) in named.iter().enumerate() {
        for (j, b) in named.iter().enumerate().skip(i + 1) {
            assert_ne!(a, b, "family constants {i} and {j} are the same profile");
        }
    }
}

#[test]
fn test_wire_profile_table_assigns_each_provider_its_declared_family() {
    // Spot-check the assignments a reader is most likely to question: the
    // provider whose name suggests one family but speaks another, and the two
    // ids that resolve to a second family for a second model.
    assert_eq!(profile_for("anthropic"), Some(ANTHROPIC_MESSAGES));
    assert_eq!(profile_for("gemini"), Some(GEMINI));
    assert_eq!(profile_for("openai_responses"), Some(OPENAI_RESPONSES));
    assert_eq!(profile_for("ollama_cloud"), Some(OLLAMA_NATIVE));
    // Local Ollama talks `OpenAiCompat` over `/v1/chat/completions`, not the
    // native `/api/chat` shape - the two Ollama providers are different
    // families, which is the distinction the table exists to record.
    assert_eq!(profile_for("ollama"), Some(OPENAI_CHAT));
    // HuggingFace's `t_` name prefix is a transformation inside the OpenAI
    // envelope, not a seventh family.
    assert_eq!(profile_for("huggingface"), Some(OPENAI_CHAT));
    // Bedrock's row is the Converse API, the default path; the Anthropic-model
    // client holds ANTHROPIC_MESSAGES itself.
    assert_eq!(profile_for("bedrock"), Some(BEDROCK_CONVERSE));
    assert_eq!(profile_for("azure_resource"), Some(OPENAI_CHAT));
    // The router forwards the canonical request to a downstream provider, so
    // its row is the canonical chat family and is nominal.
    assert_eq!(profile_for("router"), Some(OPENAI_CHAT));
}

#[test]
fn test_wire_profile_table_families_differ_where_the_tree_diverges() {
    // The divergences the specification names, each asserted against the
    // declared constants rather than against a test-local copy.
    assert_eq!(OPENAI_CHAT.args_encoding, ArgsEncoding::JsonString);
    assert_eq!(ANTHROPIC_MESSAGES.args_encoding, ArgsEncoding::JsonObject);
    assert_eq!(OLLAMA_NATIVE.args_encoding, ArgsEncoding::JsonObject);
    assert_eq!(OPENAI_RESPONSES.args_encoding, ArgsEncoding::JsonString);

    assert_eq!(OPENAI_RESPONSES.stream_keying, StreamKeying::ByCallId);
    assert_eq!(OLLAMA_NATIVE.stream_keying, StreamKeying::ByCallId);
    for family in [OPENAI_CHAT, ANTHROPIC_MESSAGES, GEMINI, BEDROCK_CONVERSE] {
        assert_eq!(family.stream_keying, StreamKeying::ByIndex);
    }

    // FR-019: the families that supply no id agree on both fields.
    for family in [GEMINI, OLLAMA_NATIVE] {
        assert!(family.synthetic_ids);
        assert_eq!(family.id_field, IdField::Synthesised);
    }
    for family in [
        OPENAI_CHAT,
        ANTHROPIC_MESSAGES,
        BEDROCK_CONVERSE,
        OPENAI_RESPONSES,
    ] {
        assert!(!family.synthetic_ids);
        assert!(matches!(family.id_field, IdField::Key(_)));
    }

    // FR-014: the two families that can transmit the flag natively, and the
    // four that need the textual `ERROR: ` marker instead.
    for (family, expected) in [
        (ANTHROPIC_MESSAGES, true),
        (BEDROCK_CONVERSE, true),
        (OPENAI_CHAT, false),
        (GEMINI, false),
        (OPENAI_RESPONSES, false),
        (OLLAMA_NATIVE, false),
    ] {
        assert_eq!(
            family.supports_is_error, expected,
            "{family:?}: supports_is_error must be declared, not inherited"
        );
    }

    // FR-011: signed reasoning is replayed by one family only.
    for (family, expected) in [
        (ANTHROPIC_MESSAGES, true),
        (OPENAI_CHAT, false),
        (GEMINI, false),
        (BEDROCK_CONVERSE, false),
        (OPENAI_RESPONSES, false),
        (OLLAMA_NATIVE, false),
    ] {
        assert_eq!(
            family.echo_reasoning, expected,
            "{family:?}: echo_reasoning must be declared, not inherited"
        );
    }
}

#[test]
fn test_wire_profile_table_every_row_is_fully_declared() {
    // A row is a contract only if every cell prints (NFR-005, TC-002). Reading
    // each field through its label is the assertion: no profile can carry an
    // empty id key or a blank family name.
    for (id, profile) in PROVIDER_WIRE {
        assert!(
            !profile.tool_envelope.as_str().is_empty(),
            "{id:?} has a blank tool envelope"
        );
        assert!(
            !profile.result_carrier.as_str().is_empty(),
            "{id:?} has a blank result carrier"
        );
        assert!(
            !profile.id_field.as_str().is_empty(),
            "{id:?} has a blank id field"
        );
        assert!(
            !profile.args_encoding.as_str().is_empty(),
            "{id:?} has a blank argument encoding"
        );
        assert!(
            !profile.stream_keying.as_str().is_empty(),
            "{id:?} has a blank stream keying"
        );
        for label in [
            profile.tool_envelope.as_str(),
            profile.result_carrier.as_str(),
            profile.id_field.as_str(),
            profile.args_encoding.as_str(),
            profile.stream_keying.as_str(),
        ] {
            assert!(
                label.is_ascii(),
                "{id:?} prints a non-ASCII cell {label:?} (NFR-005)"
            );
        }
    }
}

#[test]
fn test_wire_profile_table_envelopes_and_carriers_agree_with_the_family() {
    // The envelope/carrier pair identifies a family; a row whose pair no
    // family constant declares would be a contract invented at the call site.
    let declared: Vec<(ToolEnvelope, ResultCarrier)> = [
        OPENAI_CHAT,
        ANTHROPIC_MESSAGES,
        GEMINI,
        BEDROCK_CONVERSE,
        OPENAI_RESPONSES,
        OLLAMA_NATIVE,
    ]
    .iter()
    .map(|p| (p.tool_envelope, p.result_carrier))
    .collect();

    for (id, profile) in PROVIDER_WIRE {
        let pair = (profile.tool_envelope, profile.result_carrier);
        assert!(
            declared.contains(&pair),
            "{id:?} declares the envelope/carrier pair {pair:?}, which no family constant uses"
        );
    }
}

#[test]
fn test_wire_profile_table_profiles_are_copy_and_const() {
    // `PROVIDER_WIRE` is a `const` slice of `Copy` values, which is what lets a
    // client hold a profile by value and hand it to an encoder without
    // allocating. Reading a row twice proves the copy did not move out.
    fn is_copy(profile: ToolWireProfile) -> ToolWireProfile {
        profile
    }
    for (id, profile) in PROVIDER_WIRE {
        let first = is_copy(*profile);
        let second = is_copy(*profile);
        assert_eq!(first, second, "{id:?} does not behave as Copy");
        assert_eq!(Some(first), profile_for(id));
    }
}
