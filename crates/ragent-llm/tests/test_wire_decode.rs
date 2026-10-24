//! Tests for the one stream tool-call accumulator (FR-004).
//!
//! FR-004 asks the accumulator to key open calls by the provider's declared
//! `stream_keying` - SSE content-block index where the provider supplies an
//! index, call id where it supplies an id - to attribute every
//! `ToolCallDelta` to the call its key names, and to produce exactly one
//! `ToolCallEnd` per call id. FR-005 (parallel calls stay independent) is the
//! reason the keying exists at all, so the interleaving cases below are the
//! centre of this file. FR-019 is the id an unnamed call gets and FR-020 the
//! reconciliation of an id the provider reuses inside one response; both are
//! pure accumulator behaviour, and the FR-020 cases include a captured
//! subscriber that counts the warning line, because "warn once per collision" is
//! the requirement's third clause and cannot be seen from the return value.
//!
//! The last two cases are the mechanical half of T-008: they read the
//! provider sources and fail if a module goes back to keeping its own block map,
//! and they check that every family in the profile table declares a keying the
//! accumulator actually implements. Both are pure source/table reads, so the whole
//! file runs without a provider (NFR-004).

use ragent_llm::llm::StreamEvent;
use ragent_llm::provider::wire::StreamKeying;
use ragent_llm::provider::wire::decode::{StreamKey, ToolCallAccumulator};
use ragent_llm::provider::wire::profile_table::{
    ANTHROPIC_MESSAGES, BEDROCK_CONVERSE, GEMINI, OPENAI_CHAT, OPENAI_RESPONSES, PROVIDER_WIRE,
};

/// The `(id, args_json)` of a delta event, or a panic naming what arrived instead.
fn delta(event: Option<StreamEvent>) -> (String, String) {
    match event {
        Some(StreamEvent::ToolCallDelta { id, args_json }) => (id, args_json),
        other => panic!("expected a ToolCallDelta, got {other:?}"),
    }
}

/// The `(id, name)` of a start event, or a panic naming what arrived instead.
fn start(event: Option<StreamEvent>) -> (String, String) {
    match event {
        Some(StreamEvent::ToolCallStart { id, name }) => (id, name),
        other => panic!("expected a ToolCallStart, got {other:?}"),
    }
}

/// The id of an end event, or a panic naming what arrived instead.
fn end(event: Option<StreamEvent>) -> String {
    match event {
        Some(StreamEvent::ToolCallEnd { id }) => id,
        other => panic!("expected a ToolCallEnd, got {other:?}"),
    }
}

#[test]
fn test_accumulator_by_index_attributes_interleaved_deltas_to_their_own_block() {
    // FR-004 + FR-005: two tool_use blocks open, then argue about which one each
    // delta belongs to. Every fragment must land on the block its frame names, so
    // neither call's arguments are a concatenation of the other's - the exact
    // failure the old single-slot and `iter().last()` trackers produced.
    let mut acc = ToolCallAccumulator::new(ANTHROPIC_MESSAGES, "anthropic");
    assert_eq!(
        start(acc.open(StreamKey::Index(1), Some("toolu_a"), "Read")),
        ("toolu_a".to_string(), "Read".to_string())
    );
    assert_eq!(
        start(acc.open(StreamKey::Index(2), Some("toolu_b"), "Grep")),
        ("toolu_b".to_string(), "Grep".to_string())
    );

    assert_eq!(
        delta(acc.delta(&StreamKey::Index(2), "{\"pattern\":")),
        ("toolu_b".to_string(), "{\"pattern\":".to_string())
    );
    assert_eq!(
        delta(acc.delta(&StreamKey::Index(1), "{\"path\":")),
        ("toolu_a".to_string(), "{\"path\":".to_string())
    );
    assert_eq!(
        delta(acc.delta(&StreamKey::Index(2), "\"fn main\"}")),
        ("toolu_b".to_string(), "\"fn main\"}".to_string())
    );
    assert_eq!(
        delta(acc.delta(&StreamKey::Index(1), "\"a.rs\"}")),
        ("toolu_a".to_string(), "\"a.rs\"}".to_string())
    );

    // Close in the reverse of the open order: the index decides, not the order.
    assert_eq!(end(acc.end(&StreamKey::Index(2))), "toolu_b");
    assert_eq!(end(acc.end(&StreamKey::Index(1))), "toolu_a");
    assert_eq!(acc.open_count(), 0);
}

#[test]
fn test_accumulator_emits_exactly_one_start_and_one_end_per_call() {
    // FR-004's second clause. A provider that repeats `function.name` on every
    // argument frame (OpenRouter does) must not push a second pending call with
    // the same id, and a repeated stop frame must not close a call twice.
    let mut acc = ToolCallAccumulator::new(OPENAI_CHAT, "openai");
    let key = StreamKey::Index(0);

    assert_eq!(
        start(acc.open(key.clone(), Some("call_1"), "read")),
        ("call_1".to_string(), "read".to_string())
    );
    // The same start again - no event.
    assert!(acc.open(key.clone(), Some("call_1"), "read").is_none());
    assert!(acc.open(key.clone(), None, "read").is_none());
    // A repeated name frame still carries its arguments, and they are not lost.
    assert_eq!(
        delta(acc.delta(&key, "{\"path\"")),
        ("call_1".to_string(), "{\"path\"".to_string())
    );

    assert_eq!(end(acc.end(&key)), "call_1");
    assert!(acc.end(&key).is_none(), "one end per call id");
    assert_eq!(acc.open_count(), 0);
}

#[test]
fn test_accumulator_by_call_id_keying_tracks_the_responses_family() {
    // FR-004: the Response API repeats `call_id` on every frame, so the family
    // declares `ByCallId` and the delta names the call it belongs to without an
    // index. Two calls must not merge.
    let mut acc = ToolCallAccumulator::new(OPENAI_RESPONSES, "openai_responses");
    assert_eq!(acc.keying(), StreamKeying::ByCallId);

    let first = StreamKey::CallId("fc_1".to_string());
    let second = StreamKey::CallId("fc_2".to_string());
    assert_eq!(
        start(acc.open(first.clone(), Some("fc_1"), "read")),
        ("fc_1".to_string(), "read".to_string())
    );
    assert_eq!(
        start(acc.open(second.clone(), Some("fc_2"), "Glob")),
        ("fc_2".to_string(), "Glob".to_string())
    );
    assert_eq!(
        delta(acc.delta(&second, "\"*.rs\"")),
        ("fc_2".to_string(), "\"*.rs\"".to_string())
    );
    assert_eq!(
        delta(acc.delta(&first, "\"a.rs\"")),
        ("fc_1".to_string(), "\"a.rs\"".to_string())
    );

    let ends = acc.end_all();
    // `ByCallId` keeps the order the stream opened the calls in.
    assert_eq!(ends.len(), 2);
    assert_eq!(end(ends.into_iter().next()), "fc_1");
}

#[test]
fn test_accumulator_end_all_closes_by_index_families_in_index_order() {
    // FR-005: the completed set preserves the provider's ordering. Opening block 3
    // before block 1 (as a parallel response may) must still end 1 before 3, so a
    // consumer that pairs events by sequence sees the model's order.
    let mut acc = ToolCallAccumulator::new(GEMINI, "gemini");
    acc.open(StreamKey::Index(3), Some("fc_c_0"), "c");
    acc.open(StreamKey::Index(1), Some("fc_a_0"), "a");
    acc.open(StreamKey::Index(2), Some("fc_b_0"), "b");

    let ids: Vec<String> = acc.end_all().into_iter().map(|e| end(Some(e))).collect();
    assert_eq!(ids, vec!["fc_a_0", "fc_b_0", "fc_c_0"]);
    assert_eq!(acc.open_count(), 0);
    assert!(
        acc.end_all().is_empty(),
        "a closed stream closes nothing twice"
    );
}

#[test]
fn test_accumulator_drops_a_delta_that_names_no_open_call() {
    // FR-004: a delta is attributed to the call its key names. When the key names
    // no call, there is nothing to attribute it to - inventing a call would
    // dispatch a tool the model never asked for.
    let mut acc = ToolCallAccumulator::new(ANTHROPIC_MESSAGES, "anthropic");
    assert!(acc.delta(&StreamKey::Index(7), "{\"x\":1}").is_none());
    assert_eq!(acc.open_count(), 0);
    // Nor does an unopened key close anything.
    assert!(acc.end(&StreamKey::Index(7)).is_none());
}

#[test]
fn test_accumulator_synthesises_provider_counter_name_ids_for_unnamed_calls() {
    // FR-019: some OpenAI-compatible servers omit `tool_calls[].id`, so one must be
    // generated. It is `<provider>_<counter>_<name>`: the provider comes from the
    // accumulator's own label, the counter is monotonic per stream, and the name is
    // there for a human reading the id - the *record* is what the loop resolves a
    // tool from, never this string.
    let mut acc = ToolCallAccumulator::new(OPENAI_CHAT, "openai");

    // An index-keyed call gets an id even though it names only an index and a tool.
    let first = StreamKey::Index(2);
    assert_eq!(
        start(acc.open(first.clone(), None, "read")),
        ("openai_0_read".to_string(), "read".to_string())
    );
    assert_eq!(
        delta(acc.delta(&first, "{}")),
        ("openai_0_read".to_string(), "{}".to_string()),
        "every later frame of the call carries the synthesised id"
    );
    assert_eq!(end(acc.end(&first)), "openai_0_read");

    // A second unnamed call in the same stream cannot reuse the first id, even at
    // the same index and with the same tool name.
    assert_eq!(
        start(acc.open(StreamKey::Index(2), None, "read")).0,
        "openai_1_read"
    );

    // FR-019's reason for carrying the name on the record instead of inside the id:
    // an underscore in the tool's own name is not a separator.
    let mut acc = ToolCallAccumulator::new(OPENAI_CHAT, "openai");
    assert_eq!(
        start(acc.open(StreamKey::Index(0), None, "read_file")),
        ("openai_0_read_file".to_string(), "read_file".to_string()),
        "the name reaches the consumer whole"
    );

    // A frame that carries no name at all still produces a well-formed id.
    let mut acc = ToolCallAccumulator::new(OPENAI_CHAT, "openai");
    assert_eq!(
        start(acc.open(StreamKey::Index(0), None, "")).0,
        "openai_0_unnamed"
    );
}

#[test]
fn test_accumulator_pins_the_start_id_so_a_later_id_frame_cannot_split_the_call() {
    // The id is pinned when the start is emitted. A provider that streams the name
    // first and the id later (some compatible servers do) must not end up with a
    // start under one id and an end under another - the consumer pairs them.
    let mut acc = ToolCallAccumulator::new(OPENAI_CHAT, "openai");
    let key = StreamKey::Index(0);
    assert_eq!(
        start(acc.open(key.clone(), None, "read")),
        ("openai_0_read".to_string(), "read".to_string())
    );
    acc.set_id(&key, "call_late");
    assert_eq!(
        delta(acc.delta(&key, "{}")),
        ("openai_0_read".to_string(), "{}".to_string()),
        "the id a start carried is the id every later frame carries"
    );
    assert_eq!(end(acc.end(&key)), "openai_0_read");
}

#[test]
fn test_accumulator_holds_a_whole_deferred_call_until_it_is_flushed() {
    // Gemini hands over a complete `functionCall` in one frame and must not emit it
    // before reading the `finishReason` that may share that frame (F2). The call
    // waits, does not count as open, and is emitted once.
    let mut acc = ToolCallAccumulator::new(GEMINI, "gemini");
    acc.defer(
        "get_weather".to_string(),
        "{\"location\":\"London\"}".to_string(),
    );
    acc.defer("read_file".to_string(), "{\"path\":\"a.rs\"}".to_string());
    assert_eq!(acc.deferred_count(), 2);
    assert_eq!(acc.open_count(), 0, "deferred calls are not open");

    let events = acc.flush_deferred();
    assert_eq!(events.len(), 6, "start, delta and end per call");
    assert_eq!(acc.deferred_count(), 0);
    assert!(acc.flush_deferred().is_empty(), "a flush drains the buffer");

    let starts: Vec<(String, String)> = events
        .iter()
        .filter_map(|e| match e {
            StreamEvent::ToolCallStart { id, name } => Some((id.clone(), name.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(starts.len(), 2);
    // FR-019: the accumulator mints the id, so Gemini's own `fc_...` spelling is
    // gone and the name travels beside the id rather than inside it.
    assert_eq!(
        starts[0],
        (
            "gemini_0_get_weather".to_string(),
            "get_weather".to_string()
        )
    );
    assert_eq!(
        starts[1],
        ("gemini_1_read_file".to_string(), "read_file".to_string())
    );
    let ends = events
        .iter()
        .filter(|e| matches!(e, StreamEvent::ToolCallEnd { .. }))
        .count();
    assert_eq!(ends, 2, "one end per deferred call");
}

#[test]
fn test_stream_key_from_frame_prefers_the_index_and_falls_back_to_the_id() {
    // The one place a provider decides which key its frame names. An index wins
    // when present; otherwise a non-empty id; otherwise the frame names no call.
    assert_eq!(
        StreamKey::from_frame(Some(3), Some("toolu_1")),
        Some(StreamKey::Index(3))
    );
    assert_eq!(
        StreamKey::from_frame(None, Some("toolu_1")),
        Some(StreamKey::CallId("toolu_1".to_string()))
    );
    assert_eq!(StreamKey::from_frame(None, Some("")), None);
    assert_eq!(StreamKey::from_frame(None, None), None);
}

#[test]
fn test_every_profile_keying_is_served_by_the_accumulator() {
    // The table is the declaration and the accumulator is the implementation; a
    // family declaring a keying the accumulator does not carry a key for would be
    // a contract with no code behind it.
    for (provider, profile) in PROVIDER_WIRE {
        let acc = ToolCallAccumulator::new(*profile, provider);
        assert_eq!(
            acc.keying(),
            profile.stream_keying,
            "{provider} declares {:?} but the accumulator was built with {:?}",
            profile.stream_keying,
            acc.keying()
        );
        assert!(
            matches!(
                profile.stream_keying,
                StreamKeying::ByIndex | StreamKeying::ByCallId
            ),
            "{provider} declares a keying the accumulator does not implement"
        );
    }
    // Bedrock's Converse path and the Anthropic Messages shape behind the same
    // provider id are both index-keyed; a key-based one exists only for the
    // Responses and native-Ollama families.
    assert_eq!(BEDROCK_CONVERSE.stream_keying, StreamKeying::ByIndex);
    assert_eq!(ANTHROPIC_MESSAGES.stream_keying, StreamKeying::ByIndex);
}

#[test]
fn test_the_provider_modules_no_longer_keep_their_own_tool_call_maps() {
    // The mechanical half of T-008: every module that parses a provider stream
    // delegates its tool-call bookkeeping to the accumulator, so none of them may
    // still hold a block map or a single active-call slot of its own. The four the
    // task names come first; the four OpenAI-family siblings - each a copy of the
    // same `HashMap<index, id>` plus drain-at-finish loop - are included because
    // FR-004 asks for one accumulator, not one per module that happens to be
    // listed (T-004 folded its sibling encoders for the same reason), and T-011
    // added the Azure Resource Anthropic branch, the eleventh copy of that loop.
    use std::fs;
    use std::path::PathBuf;

    let providers = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("providers");
    let folded = [
        "anthropic.rs",
        "bedrock.rs",
        "gemini.rs",
        "sse.rs",
        "copilot.rs",
        "huggingface.rs",
        "ollama.rs",
        "ollama_cloud.rs",
        // T-011: the Azure Resource provider's Anthropic branch is an eleventh copy
        // of the same Anthropic-Messages loop. It held a `BTreeMap<u64, String>` that
        // never minted an id for a block the server left unnamed and skipped that
        // block's end, so an FR-019/FR-020 defect there was invisible to the
        // accumulator.
        "azure_resource.rs",
    ];
    for name in folded {
        let path = providers.join(name);
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        assert!(
            source.contains("ToolCallAccumulator"),
            "{} does not use the shared accumulator (FR-004)",
            path.display()
        );
        for retired in [
            "tool_call_blocks",
            "tool_call_ids",
            "tool_call_args",
            "active_tool_call_id",
            "pending_tool_calls",
            "started_tool_call_indices",
            "open_tool_calls",
        ] {
            assert!(
                !source.contains(retired),
                "{} still keeps its own tool-call bookkeeping in `{retired}`; the \
                 open-call map belongs to `wire::decode::ToolCallAccumulator` (FR-004)",
                path.display()
            );
        }
    }
}

#[test]
fn test_no_stream_attributes_a_frame_to_the_call_opened_last() {
    // FR-005. The single-slot answer is wrong the moment a response carries two
    // tool calls, so the "it must mean the call I opened most recently" fallback
    // cannot exist in the accumulator either: a provider either names the call
    // (index or id) or its frame is dropped. This reads the sources rather than
    // calling them, because the failure it prevents is a *caller* reaching for the
    // fallback, and the two helpers that offered it are gone.
    use std::fs;
    use std::path::PathBuf;

    let providers = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("providers");
    for relative in [
        "wire/decode.rs",
        "bedrock.rs",
        "anthropic.rs",
        "gemini.rs",
        "sse.rs",
    ] {
        let path = providers.join(relative);
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        for retired in ["delta_last", "end_last", "open.last_mut()"] {
            assert!(
                !source.contains(retired),
                "{} still carries the last-opened attribution `{retired}`; a frame \
                 that names no open call is dropped, not folded onto whichever call \
                 happens to be open (FR-005)",
                path.display()
            );
        }
    }
}

#[test]
fn test_a_converse_block_keys_by_content_block_index_not_by_id() {
    // FR-005: the Converse frames repeat `contentBlockIndex` on every event of a
    // block and only carry `toolUseId` on the start. Keying by index is therefore
    // what makes a fragment attributable before any id has been seen, and two
    // blocks whose starts arrive back to back cannot be confused with each other.
    let mut acc = ToolCallAccumulator::new(BEDROCK_CONVERSE, "bedrock");
    let first = StreamKey::from_frame(Some(0), Some("tu_a"));
    let second = StreamKey::from_frame(Some(1), Some("tu_b"));
    assert_eq!(first, Some(StreamKey::Index(0)));
    assert_eq!(second, Some(StreamKey::Index(1)));

    let first = first.expect("index 0");
    let second = second.expect("index 1");
    assert_eq!(
        start(acc.open(first.clone(), Some("tu_a"), "Read")).0,
        "tu_a"
    );
    assert_eq!(
        start(acc.open(second.clone(), Some("tu_b"), "Grep")).0,
        "tu_b"
    );

    assert_eq!(delta(acc.delta(&second, "{\"pattern\":")).0, "tu_b");
    assert_eq!(delta(acc.delta(&first, "{\"path\":")).0, "tu_a");
    assert_eq!(delta(acc.delta(&second, "\"x\"}")).0, "tu_b");
    assert_eq!(delta(acc.delta(&first, "\"a.rs\"}")).0, "tu_a");

    assert_eq!(end(acc.end(&second)), "tu_b");
    assert_eq!(end(acc.end(&first)), "tu_a");
    assert!(acc.end(&first).is_none(), "one end per block");
}

#[test]
fn test_accumulator_rekeys_a_duplicate_call_id_and_keeps_the_first() {
    // FR-020: a local model asked to give two calls the same id will do exactly
    // that, and an OpenAI-compatible server replaying a cached frame can too. The
    // consumer keys its pending calls by id, so a second start under the same id
    // would overwrite the first and only one of the two results would ever come
    // back - so the first call keeps the provider's id and the later one is
    // re-keyed to a fresh FR-019 id.
    let mut acc = ToolCallAccumulator::new(OPENAI_CHAT, "openai");
    let first = StreamKey::Index(0);
    let second = StreamKey::Index(1);

    assert_eq!(
        start(acc.open(first.clone(), Some("call_dup"), "read")),
        ("call_dup".to_string(), "read".to_string()),
        "the first call to claim an id keeps it"
    );
    assert_eq!(
        start(acc.open(second.clone(), Some("call_dup"), "read")),
        ("openai_0_read".to_string(), "read".to_string()),
        "the later call is re-keyed to a synthetic id, not silently dropped"
    );

    // Each call still carries its own arguments, which is the whole point: two
    // distinct results, each attributable to its own call.
    assert_eq!(
        delta(acc.delta(&first, "{\"path\":\"a\"}")),
        ("call_dup".to_string(), "{\"path\":\"a\"}".to_string())
    );
    assert_eq!(
        delta(acc.delta(&second, "{\"path\":\"b\"}")),
        ("openai_0_read".to_string(), "{\"path\":\"b\"}".to_string())
    );

    let ids: Vec<String> = acc.end_all().into_iter().map(|e| end(Some(e))).collect();
    assert_eq!(ids, vec!["call_dup", "openai_0_read"]);
}

#[test]
fn test_accumulator_rekeys_a_duplicate_id_used_by_two_sequential_calls() {
    // FR-020 does not say "two calls open at once": an id that has already been
    // ended in this response is still spent, because both calls reach the same
    // history. Keeping the ledger across `end` is what makes the second result
    // attributable.
    let mut acc = ToolCallAccumulator::new(ANTHROPIC_MESSAGES, "anthropic");
    let first = StreamKey::Index(0);
    assert_eq!(
        start(acc.open(first.clone(), Some("toolu_dup"), "Read")).0,
        "toolu_dup"
    );
    assert_eq!(end(acc.end(&first)), "toolu_dup");

    let second = StreamKey::Index(1);
    assert_eq!(
        start(acc.open(second.clone(), Some("toolu_dup"), "Grep")).0,
        "anthropic_0_Grep",
        "a closed call's id is still spent for this response"
    );
    assert_eq!(end(acc.end(&second)), "anthropic_0_Grep");
    assert_eq!(acc.open_count(), 0);
}

#[test]
fn test_accumulator_rekeys_a_collision_between_a_minted_id_and_a_provider_id() {
    // The re-key walks the counter past every id this response has used, so a
    // provider that happens to spell ids exactly like ours cannot make the
    // replacement itself a duplicate (the second, less obvious, half of FR-020).
    let mut acc = ToolCallAccumulator::new(OPENAI_CHAT, "openai");
    let first = StreamKey::Index(0);
    let second = StreamKey::Index(1);
    assert_eq!(
        start(acc.open(first.clone(), Some("openai_0_read"), "read")).0,
        "openai_0_read",
        "a provider id is kept byte for byte, even one shaped like ours"
    );
    assert_eq!(
        start(acc.open(second.clone(), Some("openai_1_read"), "read")).0,
        "openai_1_read"
    );

    // A third call repeats the first id. Re-keying it mints from counter 0, whose
    // first two candidates (`openai_0_read`, `openai_1_read`) are both taken - so the
    // mint must keep walking to `openai_2_read`. Without that walk the replacement
    // would be the duplicate it was meant to resolve.
    let third = StreamKey::Index(2);
    assert_eq!(
        start(acc.open(third.clone(), Some("openai_0_read"), "read")).0,
        "openai_2_read",
        "the replacement is not itself a duplicate"
    );

    // All three are distinct, so all three results come back.
    let ids: Vec<String> = acc.end_all().into_iter().map(|e| end(Some(e))).collect();
    assert_eq!(ids, vec!["openai_0_read", "openai_1_read", "openai_2_read"]);
}

#[test]
fn test_accumulator_does_not_rekey_a_repeated_frame_of_the_same_call() {
    // FR-004's idempotence and FR-020 must not fight: a provider that repeats the
    // whole start frame every step names the *same* call, so `open` returns no
    // second event and no collision is reported. A call-id-keyed family is the
    // clearest case - there the id is the key.
    let mut acc = ToolCallAccumulator::new(OPENAI_RESPONSES, "openai_responses");
    let key = StreamKey::CallId("fc_same".to_string());
    assert_eq!(
        start(acc.open(key.clone(), Some("fc_same"), "read")).0,
        "fc_same"
    );
    assert!(
        acc.open(key.clone(), Some("fc_same"), "read").is_none(),
        "a repeated frame is not a second call"
    );

    let index_keyed = StreamKey::Index(0);
    assert_eq!(
        start(acc.open(index_keyed.clone(), Some("call_1"), "read")).0,
        "call_1"
    );
    assert!(
        acc.open(index_keyed.clone(), Some("call_1"), "read")
            .is_none()
    );
    assert_eq!(acc.open_count(), 2, "two calls, not three");
}

#[test]
fn test_accumulator_warns_one_line_per_collision_and_not_per_frame() {
    // FR-020's third clause is "log one warning per collision" - not one per frame,
    // and not one per re-key. The return value cannot show that, so this case
    // captures the subscriber's events and counts the WARN lines: a provider that
    // repeats the frame every step must add nothing, a second collision of a
    // different id must add exactly one.
    use std::sync::{Arc, Mutex};

    #[derive(Debug, Default)]
    struct Fields {
        message: String,
        pairs: Vec<(String, String)>,
    }

    impl tracing::field::Visit for Fields {
        fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
            if field.name() == "message" {
                self.message = value.to_string();
            } else {
                self.pairs
                    .push((field.name().to_string(), value.to_string()));
            }
        }

        fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
            let rendered = format!("{value:?}");
            if field.name() == "message" {
                self.message = rendered;
            } else {
                self.pairs.push((field.name().to_string(), rendered));
            }
        }
    }

    #[derive(Clone, Default)]
    struct Capture(Arc<Mutex<Vec<(tracing::Level, Fields)>>>);

    impl tracing::Subscriber for Capture {
        fn enabled(&self, metadata: &tracing::Metadata<'_>) -> bool {
            *metadata.level() <= tracing::Level::WARN
        }

        fn new_span(&self, _span: &tracing::span::Attributes<'_>) -> tracing::span::Id {
            tracing::span::Id::from_u64(1)
        }

        fn record(&self, _span: &tracing::span::Id, _values: &tracing::span::Record<'_>) {}

        fn record_follows_from(&self, _span: &tracing::span::Id, _follows: &tracing::span::Id) {}

        fn event(&self, event: &tracing::Event<'_>) {
            let mut fields = Fields::default();
            event.record(&mut fields);
            self.0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push((*event.metadata().level(), fields));
        }

        fn enter(&self, _span: &tracing::span::Id) {}
        fn exit(&self, _span: &tracing::span::Id) {}
    }

    let capture = Capture::default();
    let warns = || -> Vec<(tracing::Level, Fields)> {
        capture
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .filter(|(level, _)| *level == tracing::Level::WARN)
            .map(|(level, fields)| {
                (
                    *level,
                    Fields {
                        message: fields.message.clone(),
                        pairs: fields.pairs.clone(),
                    },
                )
            })
            .collect()
    };

    let subscriber = capture.clone();
    tracing::subscriber::with_default(subscriber, || {
        // The callsite may have been registered before any subscriber existed, so
        // the interest cache is rebuilt rather than left at `never`.
        tracing::callsite::rebuild_interest_cache();

        let mut acc = ToolCallAccumulator::new(OPENAI_CHAT, "openai");
        acc.open(StreamKey::Index(0), Some("call_dup"), "read");
        acc.open(StreamKey::Index(1), Some("call_dup"), "read");
        // The provider repeats the same two frames on the following steps.
        for _ in 0..3 {
            acc.open(StreamKey::Index(0), Some("call_dup"), "read");
            acc.open(StreamKey::Index(1), Some("call_dup"), "read");
        }
        // A second, different collision is a second collision.
        acc.open(StreamKey::Index(2), Some("call_other"), "read");
        acc.open(StreamKey::Index(3), Some("call_other"), "read");
        // Calls that collide with nothing say nothing.
        acc.open(StreamKey::Index(4), Some("call_solo"), "read");

        let warnings = warns();
        assert_eq!(
            warnings.len(),
            2,
            "one warning per collision, not one per frame: {warnings:?}"
        );
        assert!(
            warnings[0].1.message.contains("reused a tool-call id")
                && warnings[0].1.message.contains("re-keyed"),
            "the warning says what happened: {}",
            warnings[0].1.message
        );
        assert!(
            warnings.iter().any(|(_, f)| f
                .pairs
                .iter()
                .any(|(k, v)| k == "duplicate_id" && v == "call_dup")),
            "the warning names the duplicated id: {warnings:?}"
        );
        assert!(
            warnings.iter().any(|(_, f)| f
                .pairs
                .iter()
                .any(|(k, v)| k == "duplicate_id" && v == "call_other")),
            "the second collision names its own id: {warnings:?}"
        );

        // And the two duplicate calls still reach the consumer under distinct ids.
        let mut ids: Vec<String> = acc.end_all().into_iter().map(|e| end(Some(e))).collect();
        ids.sort();
        assert_eq!(ids.len(), 5, "five calls dispatched: {ids:?}");
        assert!(ids.contains(&"call_dup".to_string()));
        assert!(ids.contains(&"openai_0_read".to_string()));
    });
}

#[test]
fn test_no_module_invents_a_tool_call_id_or_recovers_a_name_from_one() {
    // FR-019's unwanted behaviour has two halves, and both are mechanical: no
    // provider may mint a call id of its own spelling (Gemini's `fc_<name>_<n>`,
    // Ollama Cloud's `ollama_cloud_tc_<index>`, the old `tc_<index>`), and no
    // module may read a tool name out of an id by splitting it. The accumulator is
    // the one place a synthetic id exists, so a `format!("..._{}...")` naming a
    // call id elsewhere is the defect - not a style preference.
    use std::fs;
    use std::path::PathBuf;

    let providers = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("providers");
    for relative in [
        "gemini.rs",
        "ollama_cloud.rs",
        "ollama.rs",
        "sse.rs",
        "copilot.rs",
        "huggingface.rs",
        "anthropic.rs",
        "bedrock.rs",
        "azure_resource.rs",
        "openai_responses.rs",
        "wire/encode.rs",
    ] {
        let path = providers.join(relative);
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        for retired in ["format!(\"fc_", "format!(\"tc_", "_tc_{"] {
            assert!(
                !source.contains(retired),
                "{} still mints its own tool-call id with `{retired}`; the synthetic \
                 id FR-019 asks for is `<provider>_<counter>_<name>` and only \
                 `wire::decode::ToolCallAccumulator` may construct one",
                path.display()
            );
        }
        // `split('_')` / `rsplit('_')` on a call id is the name-recovery this
        // requirement forbids: a tool called `read_file` would be resolved as
        // `read`. `wire/encode.rs` is the file that used to do it.
        for retired in ["tool_use_id.split", "tool_use_id.rsplit", ".id.split('_')"] {
            assert!(
                !source.contains(retired),
                "{} still recovers a tool name by splitting a call id (`{retired}`); \
                 the name travels beside the id on the canonical record (FR-019)",
                path.display()
            );
        }
    }
}
