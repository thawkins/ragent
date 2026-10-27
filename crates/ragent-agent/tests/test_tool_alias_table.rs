//! Tests for the declarative alias table and its inbound resolver (spec
//! `uniformtools` FR-015, decisions D5 and D7).
//!
//! Two groups:
//!
//! - **table shape** - the built-in rows exist, are ascending, map only onto
//!   registered tools, and hold no spelling the FR-023 rename retires or adopts,
//!   so the table cannot shadow a real registration or duplicate a rename
//!   (FR-026, FR-031);
//! - **resolver behaviour** - a registered name always wins over a table entry,
//!   an unmapped name is never guessed at, and a target this build does not
//!   register falls through to the unknown-tool error rather than resolving onto
//!   a name that cannot be dispatched.
//!
//! Nothing here opens a provider, a socket or the filesystem: the table is data
//! and the resolver is a pure function of the registry and the table.

use ragent_agent::tool::ToolRegistry;
use ragent_agent::tool::alias_table::{
    AliasTable, BUILTIN_PROVIDER_ALIASES, ProviderAlias, resolve_inbound_tool_name,
};
use ragent_agent::tool::create_default_registry;
use ragent_agent::tool::rename::TOOL_RENAMES;
use ragent_config::ToolsConfig;

/// Parses a `tools` section from JSON.
fn tools(json: &str) -> ToolsConfig {
    serde_json::from_str(json).unwrap_or_else(|e| panic!("parse {json}: {e}"))
}

// ---------------------------------------------------------------------------
// Default: alias resolution is off
// ---------------------------------------------------------------------------

#[test]
fn test_alias_table_is_empty_when_the_section_is_absent() {
    let table = AliasTable::from_tools_config(&ToolsConfig::default());
    assert!(
        table.is_empty(),
        "an absent `tools` section resolves nothing"
    );
    assert_eq!(table.len(), 0);
    assert_eq!(table.resolve("WebFetch"), None);
}

#[test]
fn test_alias_table_is_empty_when_aliases_are_explicitly_disabled() {
    let table = AliasTable::from_tools_config(&tools(r#"{"aliases": false}"#));
    assert!(
        table.is_empty(),
        "an explicit `false` must behave exactly like an absent key"
    );
}

#[test]
fn test_alias_table_is_empty_for_an_empty_object() {
    let table = AliasTable::from_tools_config(&tools(r#"{"aliases": {}}"#));
    assert!(table.is_empty(), "an empty entries object enables nothing");
}

// ---------------------------------------------------------------------------
// The built-in table (FR-015 `aliases: true`, narrowed by D7)
// ---------------------------------------------------------------------------

#[test]
fn test_builtin_table_is_requested_by_the_boolean_form() {
    let table = AliasTable::from_tools_config(&tools(r#"{"aliases": true}"#));
    assert!(!table.is_empty(), "`true` must request the built-in table");
    assert_eq!(table.len(), BUILTIN_PROVIDER_ALIASES.len());
    assert_eq!(table.resolve("WebFetch"), Some("webfetch"));
    assert_eq!(table.resolve("WebSearch"), Some("websearch"));
    assert_eq!(table.resolve("web_search"), Some("websearch"));
}

#[test]
fn test_builtin_rows_are_ascending_by_spelling() {
    let spellings: Vec<&str> = BUILTIN_PROVIDER_ALIASES
        .iter()
        .map(|row| row.spelling)
        .collect();
    assert!(
        spellings.windows(2).all(|pair| pair[0] < pair[1]),
        "the built-in table must read ascending: {spellings:?}"
    );
}

#[test]
fn test_builtin_rows_are_ascii_and_never_self_mapping() {
    for row in BUILTIN_PROVIDER_ALIASES {
        assert!(
            row.spelling.is_ascii(),
            "spelling {:?} must be ASCII (NFR-005)",
            row.spelling
        );
        assert!(
            row.ragent_name.is_ascii(),
            "target {:?} must be ASCII (NFR-005)",
            row.ragent_name
        );
        assert!(
            row.spelling != row.ragent_name,
            "{:?} maps onto itself, which D7 deletes rather than carries",
            row.spelling
        );
    }
}

#[test]
fn test_every_builtin_target_is_registered() {
    let registry = create_default_registry();
    for row in BUILTIN_PROVIDER_ALIASES {
        assert!(
            registry.get(row.ragent_name).is_some(),
            "built-in alias {:?} targets {:?}, which the default registry does not register",
            row.spelling,
            row.ragent_name
        );
    }
}

#[test]
fn test_no_builtin_spelling_is_a_before_or_after_name() {
    // FR-026 forbids a second spelling for a renamed tool; FR-031 refuses an
    // alias whose key or target is a retired spelling. The built-in table is
    // therefore the POST-rename table: it holds neither column of FR-023.
    for row in BUILTIN_PROVIDER_ALIASES {
        assert!(
            TOOL_RENAMES.iter().all(|pair| pair.before != row.spelling),
            "{:?} is a retired ragent spelling and may not be an alias key (FR-031)",
            row.spelling
        );
        assert!(
            TOOL_RENAMES.iter().all(|pair| pair.after != row.spelling),
            "{:?} is an adopted ragent spelling and resolves by lookup, not by alias (FR-026)",
            row.spelling
        );
    }
}

#[test]
fn test_builtin_table_omits_the_hosted_tools_with_no_local_execution() {
    // Out of Scope: `code_interpreter` has no local execution and is mapped
    // `No equivalent`, so an entry would claim ragent can run it. `Read` is a
    // `Close` match that the case-insensitive hint already handles.
    let table = AliasTable::from_tools_config(&tools(r#"{"aliases": true}"#));
    assert_eq!(table.resolve("code_interpreter"), None);
    assert_eq!(table.resolve("Read"), None);
    assert_eq!(table.resolve("Bash"), None);
}

// ---------------------------------------------------------------------------
// Explicit entries
// ---------------------------------------------------------------------------

#[test]
fn test_explicit_entries_are_resolved_case_sensitively() {
    let table = AliasTable::from_tools_config(&tools(r#"{"aliases": {"Think": "think"}}"#));
    assert_eq!(table.resolve("Think"), Some("think"));
    assert_eq!(
        table.resolve("think"),
        None,
        "the table is exact-match and case-sensitive (FR-015)"
    );
    assert_eq!(table.resolve("THINK"), None);
    assert_eq!(table.resolve("Thin"), None);
}

#[test]
fn test_explicit_self_mapping_is_dropped() {
    let table = AliasTable::from_tools_config(&tools(r#"{"aliases": {"think": "think"}}"#));
    assert!(
        table.is_empty(),
        "a spelling mapped onto itself changes nothing and must not be kept (D7)"
    );
}

#[test]
fn test_explicit_entry_wins_a_builtin_collision() {
    let table = AliasTable::from_tools_config(&tools(
        r#"{"aliases": {"WebFetch": "mf_fetch", "web_search": "websearch"}}"#,
    ));
    assert_eq!(
        table.resolve("WebFetch"),
        Some("mf_fetch"),
        "the user's own entry is the more specific statement"
    );
    assert_eq!(table.resolve("web_search"), Some("websearch"));
}

#[test]
fn test_true_and_entries_compose_after_a_merge() {
    // `Config::merge` ORs the boolean and unions the entries, so the table has
    // to serve both forms at once rather than picking one.
    let mut merged = tools(r#"{"aliases": true}"#);
    merged.merge(&tools(r#"{"aliases": {"Think": "think"}}"#));
    let table = AliasTable::from_tools_config(&merged);
    assert_eq!(table.resolve("WebFetch"), Some("webfetch"));
    assert_eq!(table.resolve("Think"), Some("think"));
    assert_eq!(table.len(), BUILTIN_PROVIDER_ALIASES.len() + 1);
}

#[test]
fn test_entries_are_returned_ascending_by_spelling() {
    let table = AliasTable::from_tools_config(&tools(
        r#"{"aliases": {"zeta": "think", "alpha": "think", "mu": "think"}}"#,
    ));
    let spellings: Vec<&str> = table.entries().map(|(spelling, _)| spelling).collect();
    assert_eq!(spellings, vec!["alpha", "mu", "zeta"]);
}

// ---------------------------------------------------------------------------
// The resolver, applied before the unknown-tool check
// ---------------------------------------------------------------------------

#[test]
fn test_resolver_maps_an_alias_spelling_onto_its_registered_target() {
    let registry = create_default_registry();
    let table = AliasTable::from_tools_config(&tools(r#"{"aliases": {"Think": "think"}}"#));
    assert_eq!(
        resolve_inbound_tool_name(&registry, &table, "Think").as_deref(),
        Some("think")
    );
}

#[test]
fn test_resolver_leaves_a_registered_name_untouched() {
    // A registration always wins: even a table entry naming this exact spelling
    // cannot shadow the tool, because the resolver checks the registry first.
    let registry = create_default_registry();
    let table = AliasTable::from_tools_config(&tools(r#"{"aliases": {"think": "calculator"}}"#));
    assert_eq!(table.resolve("think"), Some("calculator"));
    assert_eq!(
        resolve_inbound_tool_name(&registry, &table, "think"),
        None,
        "a registered spelling must resolve by lookup, not by a table read"
    );
}

#[test]
fn test_resolver_never_guesses_an_unmapped_name() {
    let registry = create_default_registry();
    let table = AliasTable::from_tools_config(&tools(r#"{"aliases": true}"#));
    // `webfetchh` is one character from a built-in target's spelling and from
    // the registered name; the resolver must not close the gap (D5).
    assert_eq!(
        resolve_inbound_tool_name(&registry, &table, "webfetchh"),
        None
    );
    assert_eq!(
        resolve_inbound_tool_name(&registry, &table, "definitely_not_a_tool"),
        None
    );
}

#[test]
fn test_resolver_ignores_a_target_this_build_does_not_register() {
    let registry = create_default_registry();
    let table =
        AliasTable::from_tools_config(&tools(r#"{"aliases": {"Frobnicate": "frobnicate"}}"#));
    assert_eq!(table.resolve("Frobnicate"), Some("frobnicate"));
    assert_eq!(
        resolve_inbound_tool_name(&registry, &table, "Frobnicate"),
        None,
        "an unregistered target must reach the unknown-tool error naming the \
         spelling the client sent, not fail later pointing at a name nobody sent"
    );
}

#[test]
fn test_resolver_is_inert_when_the_table_is_empty() {
    let registry = create_default_registry();
    let empty = AliasTable::default();
    for name in ["think", "Think", "WebFetch", "nope"] {
        assert_eq!(resolve_inbound_tool_name(&registry, &empty, name), None);
    }
}

#[test]
fn test_resolver_returns_the_target_not_an_owned_clone_of_the_spelling() {
    // The signature returns `Option<String>` on purpose: the caller builds a
    // `PendingToolCall` from it, and `None` is the hot path (every unaliased
    // call), so nothing is allocated when resolution does not apply.
    let registry = create_default_registry();
    let table = AliasTable::from_tools_config(&tools(r#"{"aliases": true}"#));
    assert!(resolve_inbound_tool_name(&registry, &table, "think").is_none());
    assert!(resolve_inbound_tool_name(&registry, &table, "web_search").is_some());
}

// ---------------------------------------------------------------------------
// The table is data: it must agree with the registry it serves
// ---------------------------------------------------------------------------

#[test]
fn test_builtin_table_and_registry_agree_on_every_row() {
    let registry: ToolRegistry = create_default_registry();
    let table = AliasTable::from_tools_config(&tools(r#"{"aliases": true}"#));
    for (spelling, target) in table.entries() {
        assert!(
            registry.get(spelling).is_none(),
            "{spelling:?} is advertised as an alias spelling but is already a \
             registered name, so the entry is a no-op"
        );
        assert!(
            registry.get(target).is_some(),
            "{spelling:?} resolves onto {target:?}, which is not registered"
        );
        assert_eq!(
            resolve_inbound_tool_name(&registry, &table, spelling).as_deref(),
            Some(target)
        );
    }
}

#[test]
fn test_provider_alias_row_is_copy_and_comparable() {
    // The row type is `Copy` so a `const` table can be iterated by value; a
    // reference-returning table would force an index dance at every site.
    let row: ProviderAlias = BUILTIN_PROVIDER_ALIASES[0];
    let copy = row;
    assert_eq!(row, copy);
    assert_eq!(row.spelling, "WebFetch");
}

// ---------------------------------------------------------------------------
// FR-031: the resolver never consults a retired spelling, and an unresolved
// target is reported once and refused when used
// ---------------------------------------------------------------------------

/// A `ToolsConfig` built directly in memory, bypassing the config loader.
///
/// The loader refuses a retired spelling and stops the load (T-042), which is the
/// first half of FR-031. This is the second half: a section assembled without the
/// loader - a test, a program that mutates the struct, a caller that will exist
/// later - must still not resolve a retired spelling through the table.
fn tools_in_memory(json: &str) -> ToolsConfig {
    tools(json)
}

#[test]
fn test_resolver_never_consults_a_retired_spelling_from_the_table() {
    // Every `before` spelling is fed in as a source key *and* as a target for
    // every pair, so the case grows with the table rather than pinning `Bash`.
    let registry = create_default_registry();
    for row in &TOOL_RENAMES {
        let as_key = AliasTable::from_tools_config(&tools_in_memory(&format!(
            r#"{{"aliases": {{"{}": "think"}}}}"#,
            row.before
        )));
        assert_eq!(
            as_key.resolve(row.before),
            None,
            "the table must not hold the retired spelling `{}` as a key (FR-031)",
            row.before
        );
        assert_eq!(
            resolve_inbound_tool_name(&registry, &as_key, row.before),
            None,
            "a retired spelling must never resolve through the table (FR-031)"
        );

        let as_target = AliasTable::from_tools_config(&tools_in_memory(&format!(
            r#"{{"aliases": {{"SomeProviderSpelling": "{}"}}}}"#,
            row.before
        )));
        assert_eq!(
            as_target.resolve("SomeProviderSpelling"),
            None,
            "the table must not hold the retired spelling `{}` as a target (FR-031)",
            row.before
        );
    }
}

#[test]
#[serial_test::serial]
fn test_an_unresolved_alias_target_is_reported_once_and_refused_when_used() {
    ragent_agent::tool::alias_table::reset_unresolved_targets_for_test();
    let registry = create_default_registry();
    let table = AliasTable::from_tools_config(&tools_in_memory(
        r#"{"aliases": {"Frobnicate": "bridged_after_config_load"}}"#,
    ));

    // Reported once: the second turn's identical table says nothing, because a
    // per-turn warning is one a reader learns to ignore.
    assert_eq!(
        table.report_unresolved_targets_once(&registry),
        1,
        "the first sight of an unresolved target is reported"
    );
    assert_eq!(
        table.report_unresolved_targets_once(&registry),
        0,
        "the same entry is not reported a second time"
    );

    // ... and refused at the moment it is used, rather than silently ignored.
    assert_eq!(
        resolve_inbound_tool_name(&registry, &table, "Frobnicate"),
        None,
        "an unresolved target falls through to the unknown-tool refusal (FR-031)"
    );
    assert_eq!(
        table.unresolved_targets(&registry),
        vec![("Frobnicate", "bridged_after_config_load")],
        "the entry names the target that was not found"
    );
}

#[test]
#[serial_test::serial]
fn test_a_resolved_target_is_never_reported_as_unresolved() {
    ragent_agent::tool::alias_table::reset_unresolved_targets_for_test();
    let registry = create_default_registry();
    let table = AliasTable::from_tools_config(&tools_in_memory(
        r#"{"aliases": {"Think": "think", "Frobnicate": "bridged_after_config_load"}}"#,
    ));
    assert_eq!(
        table.unresolved_targets(&registry),
        vec![("Frobnicate", "bridged_after_config_load")],
        "only the entry whose target is missing is unresolved"
    );
    assert_eq!(table.report_unresolved_targets_once(&registry), 1);
}
