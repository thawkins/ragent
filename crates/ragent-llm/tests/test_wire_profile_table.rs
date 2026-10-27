//! Coverage test for the provider-to-profile table (FR-002).
//!
//! FR-002 has two halves. This file tests the second: a test **shall** assert
//! that the set of profiles covers every registered provider id. The first half -
//! "a client constructed without a profile shall fail to compile" - is not
//! asserted here, because no client holds a profile yet: T-002 declares the
//! contract, T-003 the table, and the encoder tasks (T-004 onward) make each
//! client take one. What makes that later step safe is the type: `ToolWireProfile`
//! has no `Default` impl and no optional constructor, so when a client does take a
//! profile there is no fallback contract it could silently inherit. That property
//! is pinned in `tests/test_wire_profile.rs`
//! (`test_wire_profile_has_no_default_impl_anywhere_in_the_crate`).
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
//! lookup agrees with the slice, every declared family is listed in [`FAMILIES`]
//! and every row names one of them, and the profiles differ on the axes the tree
//! actually diverges on.
//!
//! [`FAMILIES`] is what makes the "one profile per provider family" half of the
//! task checkable rather than assumed. `PROVIDER_WIRE` is many-to-one by design -
//! eight providers share the OpenAI chat family - so a table alone cannot
//! distinguish "the same declared contract" from "fifteen literals that happen to
//! be equal today". The list names each family once, every row must be a member
//! of it, and a source scan keeps a newly declared family constant from existing
//! in the module without appearing in the list.
//!
//! No provider is contacted and no session is created: the registry is built
//! offline (its constructors read environment variables only to *resolve* a
//! base URL, and none is asked to create a client here), which is what NFR-004
//! asks for.

use std::collections::HashSet;

use ragent_llm::provider::ProviderRegistry;
use ragent_llm::provider::create_default_registry;
use ragent_llm::provider::wire::profile_table::{
    ANTHROPIC_MESSAGES, BEDROCK_CONVERSE, FAMILIES, GEMINI, OLLAMA_NATIVE, OPENAI_CHAT,
    OPENAI_RESPONSES, PROVIDER_WIRE, profile_for,
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
fn test_wire_profile_table_declares_every_family_once() {
    // `FAMILIES` is the declaration of "one profile per provider family". Every
    // family `const` must appear in it - a constant nothing lists is dead
    // contract the wire matrix (FR-016) would never print - and the constants
    // must be six *distinct* contracts: no two agree on all nine fields, or the
    // second would be a duplicate family rather than a family.
    assert_eq!(
        FAMILIES.len(),
        6,
        "the tree implements six wire formats; a seventh family is a contract \
         change, not a list entry"
    );
    for (label, family) in FAMILIES {
        assert!(
            PROVIDER_WIRE.iter().any(|(_, p)| p == family),
            "family {label:?} ({:?} / {:?}) is not named by any PROVIDER_WIRE row",
            family.tool_envelope,
            family.result_carrier
        );
    }
    for (i, (label_a, a)) in FAMILIES.iter().enumerate() {
        for (label_b, b) in FAMILIES.iter().skip(i + 1) {
            assert_ne!(
                a, b,
                "families {label_a:?} and {label_b:?} are the same profile"
            );
        }
    }

    // The labels are the family's own name in lower case, so they are ASCII,
    // unique and ascending - the properties the wire matrix (FR-016) and any
    // printed reference rely on (NFR-005).
    let mut previous: Option<&str> = None;
    for (label, _) in FAMILIES {
        assert!(!label.is_empty(), "a family label may not be blank");
        assert!(label.is_ascii(), "the family label {label:?} is not ASCII");
        assert!(
            label.chars().all(|c| c.is_ascii_lowercase() || c == '_'),
            "the family label {label:?} is not the const name in lower case"
        );
        if let Some(previous) = previous {
            assert!(
                previous < *label,
                "FAMILIES must be ascending, but {previous:?} precedes {label:?}"
            );
        }
        previous = Some(label);
    }
}

#[test]
fn test_wire_profile_table_every_row_names_a_declared_family() {
    // The coverage assertion of FR-002 in the direction the registry cannot
    // express: a row is an *assignment* of an id to a family, never a contract
    // composed at the row. A provider that spoke a seventh profile no family
    // declares would make "one profile per provider family" false while every
    // other case in this file still passed, so each row's profile must be one of
    // the `FAMILIES` entries.
    for (id, profile) in PROVIDER_WIRE {
        assert!(
            FAMILIES.iter().any(|(_, family)| family == profile),
            "{id:?} declares a profile no family declares: {:?} / {:?} / {:?}",
            profile.tool_envelope,
            profile.result_carrier,
            profile.id_field
        );
    }
}

#[test]
fn test_wire_profile_table_source_declares_no_family_outside_the_list() {
    // The mechanical half: `FAMILIES` is hand-written, so a new `pub const
    // X: ToolWireProfile` added to `profile_table.rs` would compile and pass
    // every value-level assertion above without ever being listed. This reads the
    // module and pins the two sets together, so a seventh family cannot exist in
    // the build while the list still claims six.
    use std::path::PathBuf;

    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("providers")
        .join("wire")
        .join("profile_table.rs");
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));

    let mut declared: Vec<&str> = Vec::new();
    for line in source.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("pub const ") else {
            continue;
        };
        let Some((name, rest)) = rest.split_once(':') else {
            continue;
        };
        if rest.trim_start().starts_with("ToolWireProfile") {
            declared.push(name.trim());
        }
    }

    let listed: Vec<&str> = FAMILIES.iter().map(|(label, _)| *label).collect();
    for name in &declared {
        assert!(
            listed.contains(&name.to_ascii_lowercase().as_str()),
            "`{name}` is declared in profile_table.rs but not listed in FAMILIES, so \
             the wire matrix would never print it and no row could name it (FR-002)"
        );
    }
    assert_eq!(
        declared.len(),
        listed.len(),
        "{} family constants are declared but FAMILIES lists {} families: {} vs {:?}",
        declared.len(),
        listed.len(),
        declared.join(", "),
        listed
    );
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
    // Read from `FAMILIES` rather than a list repeated here, so a seventh family
    // cannot be declared, listed, and still invisible to this case.
    let declared: Vec<(ToolEnvelope, ResultCarrier)> = FAMILIES
        .iter()
        .map(|(_, p)| (p.tool_envelope, p.result_carrier))
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
