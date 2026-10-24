//! Tests for the `tools` config section (spec `uniformtools` FR-015, FR-017).
//!
//! Covers the three keys T-020 adds: `tools.aliases` (both accepted forms and
//! the two load errors a typo produces), `tools.expose_alias_names` and
//! `tools.wire_trace`, plus their default-off behaviour, their merge rules
//! (OR for the opt-in booleans, per-spelling union for the alias entries) and
//! their serialisation form.
//!
//! None of these cases touches a provider, a tool or the filesystem: the
//! section is pure configuration, and FR-017's trace writes are T-023's.

use ragent_config::{AliasesConfig, Config, ToolsConfig};

/// Parse a whole `Config` from JSON, failing loudly with the parse error.
fn parse(json: &str) -> Config {
    serde_json::from_str(json).unwrap_or_else(|e| panic!("parse {json}: {e}"))
}

// ---------------------------------------------------------------------------
// Defaults (FR-015 "where neither is enabled, behaviour shall be unchanged")
// ---------------------------------------------------------------------------

#[test]
fn test_tools_config_default_is_entirely_off() {
    let tools = ToolsConfig::default();
    assert!(
        tools.is_default(),
        "a default section must report is_default"
    );
    assert!(
        tools.aliases.is_unspecified(),
        "an absent `aliases` key must not look like an explicit `false`"
    );
    assert!(
        !tools.aliases.is_enabled(),
        "alias resolution is off unless configured"
    );
    assert!(!tools.aliases.uses_builtin_table());
    assert!(tools.aliases.entries().is_empty());
    assert!(!tools.expose_alias_names);
    assert!(!tools.wire_trace);
}

#[test]
fn test_config_without_the_tools_key_gets_the_disabled_default() {
    let config = parse("{}");
    assert_eq!(config.tools, ToolsConfig::default());
    assert!(!config.tools.aliases.is_enabled());
    assert!(!config.tools.wire_trace);
}

#[test]
fn test_serialised_default_omits_the_tools_key() {
    let json = serde_json::to_value(Config::default()).expect("serialise config");
    assert!(
        json.get("tools").is_none(),
        "a never-configured build must not write a `tools` key: {json}"
    );
}

// ---------------------------------------------------------------------------
// `aliases` - the boolean form
// ---------------------------------------------------------------------------

#[test]
fn test_aliases_true_requests_the_builtin_table() {
    let config = parse(r#"{ "tools": { "aliases": true } }"#);
    let aliases = &config.tools.aliases;
    assert!(!aliases.is_unspecified(), "the key was present");
    assert!(aliases.is_enabled());
    assert!(
        aliases.uses_builtin_table(),
        "`true` means the built-in provider-spelling table"
    );
    assert!(
        aliases.entries().is_empty(),
        "the boolean form carries no entries of its own"
    );
    assert!(!config.tools.is_default());
}

#[test]
fn test_aliases_false_is_an_explicit_off_that_round_trips() {
    let config = parse(r#"{ "tools": { "aliases": false } }"#);
    let aliases = &config.tools.aliases;
    assert!(
        !aliases.is_unspecified(),
        "an explicit `false` is not 'absent'"
    );
    assert!(!aliases.is_enabled());
    assert!(!aliases.uses_builtin_table());

    let serialised = serde_json::to_value(&config.tools).expect("serialise tools");
    assert_eq!(
        serialised.get("aliases"),
        Some(&serde_json::json!(false)),
        "an explicit `false` must survive a round-trip rather than vanish"
    );
}

// ---------------------------------------------------------------------------
// `aliases` - the object form
// ---------------------------------------------------------------------------

#[test]
fn test_aliases_object_maps_provider_spellings_onto_ragent_names() {
    let config = parse(
        r#"{ "tools": { "aliases": { "WebFetch": "webfetch", "web_search": "websearch" } } }"#,
    );
    let aliases = &config.tools.aliases;
    assert!(aliases.is_enabled());
    assert!(
        !aliases.uses_builtin_table(),
        "the object form declares its entries instead of the built-in table"
    );
    let entries = aliases.entries();
    assert_eq!(entries.len(), 2, "both entries parsed");
    assert_eq!(
        entries.get("WebFetch").map(String::as_str),
        Some("webfetch")
    );
    assert_eq!(
        entries.get("web_search").map(String::as_str),
        Some("websearch"),
        "spellings are exact and case-sensitive: `web_search` is its own key"
    );
    assert_eq!(entries.get("websearch"), None);
}

#[test]
fn test_aliases_object_round_trips_as_an_object() {
    let config = parse(r#"{ "tools": { "aliases": { "WebFetch": "webfetch" } } }"#);
    let serialised = serde_json::to_value(&config.tools).expect("serialise tools");
    assert_eq!(
        serialised.get("aliases"),
        Some(&serde_json::json!({ "WebFetch": "webfetch" })),
        "the object form must serialise back as an object: {serialised}"
    );
}

#[test]
fn test_aliases_unknown_value_shape_is_a_load_error_naming_the_accepted_forms() {
    // A bare string is neither a boolean nor an object. It must fail loudly
    // rather than silently disabling alias resolution.
    let err = serde_json::from_str::<Config>(r#"{ "tools": { "aliases": "webfetch" } }"#)
        .expect_err("a string must not be accepted as `tools.aliases`");
    let message = err.to_string();
    assert!(
        message.contains("tools.aliases"),
        "the error must name the key: {message}"
    );
    assert!(
        message.contains("boolean") && message.contains("object"),
        "the error must name both accepted forms: {message}"
    );
    assert!(
        message.contains("a string"),
        "the error must name what was found instead: {message}"
    );
}

#[test]
fn test_aliases_entry_with_a_non_string_target_is_a_load_error_naming_the_spelling() {
    let err = serde_json::from_str::<Config>(r#"{ "tools": { "aliases": { "WebFetch": 3 } } }"#)
        .expect_err("a numeric target must not be accepted");
    let message = err.to_string();
    assert!(
        message.contains("WebFetch"),
        "the error must name the offending spelling: {message}"
    );
    assert!(
        message.contains("a number"),
        "the error must name what the target was: {message}"
    );
}

// ---------------------------------------------------------------------------
// `expose_alias_names` and `wire_trace`
// ---------------------------------------------------------------------------

#[test]
fn test_expose_alias_names_and_wire_trace_parse_independently() {
    let both = parse(r#"{ "tools": { "expose_alias_names": true, "wire_trace": true } }"#);
    assert!(both.tools.expose_alias_names);
    assert!(both.tools.wire_trace);
    assert!(
        !both.tools.aliases.is_enabled(),
        "the two switches do not imply alias resolution"
    );

    let only_trace = parse(r#"{ "tools": { "wire_trace": true } }"#);
    assert!(only_trace.tools.wire_trace);
    assert!(!only_trace.tools.expose_alias_names);
    assert!(!only_trace.tools.aliases.is_enabled());
}

#[test]
fn test_serialised_tools_section_keeps_only_the_keys_that_were_set() {
    let config = parse(r#"{ "tools": { "wire_trace": true } }"#);
    let serialised = serde_json::to_value(&config.tools).expect("serialise tools");
    assert_eq!(
        serialised,
        serde_json::json!({ "wire_trace": true }),
        "unset booleans and an absent `aliases` key must not be written: {serialised}"
    );
}

// ---------------------------------------------------------------------------
// Merge behaviour
// ---------------------------------------------------------------------------

#[test]
fn test_merge_ors_the_opt_in_booleans() {
    let base = parse(r#"{ "tools": { "aliases": true, "wire_trace": true } }"#);
    let overlay = parse(r#"{ "tools": { "expose_alias_names": true } }"#);
    let merged = Config::merge(base, overlay);

    assert!(
        merged.tools.aliases.uses_builtin_table(),
        "a layer that enabled the built-in table keeps it enabled"
    );
    assert!(
        merged.tools.wire_trace,
        "an overlay that omits `wire_trace` must not switch it off"
    );
    assert!(
        merged.tools.expose_alias_names,
        "an overlay that sets `expose_alias_names` switches it on"
    );
}

#[test]
fn test_merge_unions_alias_entries_and_the_overlay_wins_a_collision() {
    let base = parse(
        r#"{ "tools": { "aliases": { "WebFetch": "webfetch", "web_search": "websearch" } } }"#,
    );
    let overlay =
        parse(r#"{ "tools": { "aliases": { "WebFetch": "mf_fetch", "file_search": "Grep" } } }"#);
    let merged = Config::merge(base, overlay);
    let entries = merged.tools.aliases.entries();

    assert_eq!(
        entries.len(),
        3,
        "entries union by spelling rather than replacing the map: {entries:?}"
    );
    assert_eq!(
        entries.get("WebFetch").map(String::as_str),
        Some("mf_fetch"),
        "the overlay re-points an existing spelling"
    );
    assert_eq!(
        entries.get("web_search").map(String::as_str),
        Some("websearch"),
        "an entry the overlay does not mention survives"
    );
    assert_eq!(entries.get("file_search").map(String::as_str), Some("Grep"));
}

#[test]
fn test_merge_keeps_the_builtin_request_when_the_overlay_adds_an_entry() {
    let base = parse(r#"{ "tools": { "aliases": true } }"#);
    let overlay = parse(r#"{ "tools": { "aliases": { "WebFetch": "webfetch" } } }"#);
    let merged = Config::merge(base, overlay);

    assert!(
        merged.tools.aliases.uses_builtin_table(),
        "the user-global `true` is not discarded by a project entry"
    );
    assert!(merged.tools.aliases.is_enabled());
    assert_eq!(
        merged.tools.aliases.entries().len(),
        1,
        "the overlay's entry is unioned in"
    );
    // Such a merged state serialises as the entries object, because one JSON key
    // cannot express both forms. That rendering is only ever read by `/config
    // show`; a user's file is persisted from its raw bytes.
    let serialised = serde_json::to_value(&merged.tools).expect("serialise tools");
    assert_eq!(
        serialised.get("aliases"),
        Some(&serde_json::json!({ "WebFetch": "webfetch" })),
        "the entries form carries the merged state: {serialised}"
    );
}

#[test]
fn test_merge_explicit_false_turns_resolution_off_and_drops_inherited_entries() {
    // `false` is the only way a config author can say "I do not want a table".
    // A user-global `true` must therefore be switchable off from a higher-
    // precedence layer rather than unioned back on.
    let base = parse(r#"{ "tools": { "aliases": { "WebFetch": "webfetch" } } }"#);
    let overlay = parse(r#"{ "tools": { "aliases": false } }"#);
    let merged = Config::merge(base, overlay);

    assert!(
        !merged.tools.aliases.is_enabled(),
        "an explicit `false` must switch alias resolution off"
    );
    assert!(
        merged.tools.aliases.entries().is_empty(),
        "the inherited entry must be dropped, not kept under a disabled key"
    );
    assert!(!merged.tools.aliases.uses_builtin_table());
    assert!(
        !merged.tools.aliases.is_unspecified(),
        "the merged value is the overlay's explicit `false`, not an absent key"
    );
}

#[test]
fn test_merge_explicit_false_leaves_the_other_switches_alone() {
    // `aliases: false` disables aliases only: the trace and the outbound
    // advertising switch are separate capabilities.
    let base = parse(r#"{ "tools": { "aliases": true, "wire_trace": true } }"#);
    let overlay = parse(r#"{ "tools": { "aliases": false } }"#);
    let merged = Config::merge(base, overlay);

    assert!(!merged.tools.aliases.is_enabled());
    assert!(merged.tools.wire_trace, "the trace switch is unrelated");
}

#[test]
fn test_merge_project_keeps_the_tools_section() {
    // The section resolves names, not capabilities: the permission layer, the
    // loop restriction and the bash validation all still run on the resolved
    // call, so a project may contribute it like any other non-privileged key.
    // The refusal the rename needs is the per-entry retired-spelling rejection
    // when the config loads (FR-031), not a blanket strip here.
    let base = parse(r#"{ "tools": { "wire_trace": false } }"#);
    let overlay = parse(r#"{ "tools": { "aliases": { "WebFetch": "webfetch" } } }"#);
    let merged = Config::merge_project(base, overlay);

    assert!(
        merged.tools.aliases.is_enabled(),
        "a project-level alias entry survives the untrusted-overlay merge"
    );
    assert_eq!(
        merged
            .tools
            .aliases
            .entries()
            .get("WebFetch")
            .map(String::as_str),
        Some("webfetch")
    );
}

// ---------------------------------------------------------------------------
// AliasesConfig in isolation
// ---------------------------------------------------------------------------

#[test]
fn test_aliases_config_default_reports_unspecified_and_disabled() {
    let aliases = AliasesConfig::default();
    assert!(aliases.is_unspecified());
    assert!(!aliases.is_enabled());
    assert!(!aliases.uses_builtin_table());
    assert!(aliases.entries().is_empty());
}

#[test]
fn test_aliases_config_empty_object_is_specified_but_disabled() {
    // `"aliases": {}` is a deliberate, empty table: it round-trips as `{}` and
    // resolves nothing. It is not the same as an absent key.
    let config = parse(r#"{ "tools": { "aliases": {} } }"#);
    let aliases = &config.tools.aliases;
    assert!(!aliases.is_unspecified(), "the key was present");
    assert!(!aliases.is_enabled(), "an empty table resolves nothing");
    assert!(aliases.entries().is_empty());
}

#[test]
fn test_aliases_empty_object_round_trips_as_an_empty_object() {
    // `{}` and `false` are different statements: one declares an empty table,
    // the other switches alias resolution off. Collapsing them would let a
    // round-trip rewrite the author's file with a key they never wrote.
    let config = parse(r#"{ "tools": { "aliases": {} } }"#);
    let serialised = serde_json::to_value(&config.tools).expect("serialise tools");
    assert_eq!(
        serialised.get("aliases"),
        Some(&serde_json::json!({})),
        "an empty object must stay an object, never become `false`: {serialised}"
    );
    assert!(
        !config.tools.is_default(),
        "an explicit `{{}}` is a deliberate section, not the absent default"
    );
}

// ---------------------------------------------------------------------------
// `aliases: {}` is additive, not a switch (FR-015)
// ---------------------------------------------------------------------------

#[test]
fn test_merge_empty_object_does_not_clear_the_inherited_table() {
    // Only `false` turns alias resolution off. An empty object contributes no
    // entry of its own, so a project file that writes `{}` must not silently
    // discard a table the user-global file declared.
    let base = parse(r#"{ "tools": { "aliases": { "WebFetch": "webfetch" } } }"#);
    let overlay = parse(r#"{ "tools": { "aliases": {} } }"#);
    let merged = Config::merge(base, overlay);

    assert!(
        merged.tools.aliases.is_enabled(),
        "`{{}}` must not disable the inherited table"
    );
    assert_eq!(
        merged
            .tools
            .aliases
            .entries()
            .get("WebFetch")
            .map(String::as_str),
        Some("webfetch"),
        "the inherited entry survives an empty overlay table"
    );
}

#[test]
fn test_merge_empty_object_keeps_the_builtin_request() {
    let base = parse(r#"{ "tools": { "aliases": true } }"#);
    let overlay = parse(r#"{ "tools": { "aliases": {} } }"#);
    let merged = Config::merge(base, overlay);

    assert!(
        merged.tools.aliases.uses_builtin_table(),
        "`{{}}` is not the `false` switch, so the built-in request stands"
    );
    assert!(merged.tools.aliases.entries().is_empty());
}

#[test]
fn test_merge_empty_object_does_not_undo_an_explicit_false() {
    // An explicit `false` in a lower layer stays off through an overlay that
    // merely declares an empty table - the overlay never asked for resolution.
    let base = parse(r#"{ "tools": { "aliases": false } }"#);
    let overlay = parse(r#"{ "tools": { "aliases": {} } }"#);
    let merged = Config::merge(base, overlay);

    assert!(
        !merged.tools.aliases.is_enabled(),
        "an empty object cannot re-enable a table the base switched off"
    );
}

#[test]
fn test_merge_absent_aliases_key_changes_nothing() {
    let base = parse(r#"{ "tools": { "aliases": { "WebFetch": "webfetch" } } }"#);
    let overlay = parse(r#"{ "tools": { "wire_trace": true } }"#);
    let merged = Config::merge(base, overlay);

    assert!(
        merged.tools.aliases.is_enabled(),
        "an overlay that says nothing about `aliases` leaves the table alone"
    );
    assert_eq!(merged.tools.aliases.entries().len(), 1);
    assert!(
        merged.tools.wire_trace,
        "and its own unrelated switch is applied"
    );
}
