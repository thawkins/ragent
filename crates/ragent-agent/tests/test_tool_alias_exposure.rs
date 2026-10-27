//! Tests for `tools.expose_alias_names` in the outbound tool list (spec
//! `uniformtools` FR-015, task T-021; validated by TESTPLAN case TC-013).
//!
//! FR-015's second clause reads: *Where `tools.expose_alias_names` is `true`, the
//! outbound tool list shall advertise each alias alongside the canonical name.*
//! The outbound list is [`ToolRegistry::definitions`] passed through
//! [`AliasTable::outbound_definitions`] - one function, so every surface that
//! advertises tools (the wire `tools` array, the system prompt's tool reference,
//! `/tools`, the FR-030 count) sees the same names.
//!
//! Five groups:
//!
//! - **off is unchanged** - the default, an explicit `false`, and an empty table
//!   all advertise exactly the registry's own names, in order (FR-015's third
//!   clause);
//! - **on advertises the spelling** - each entry appears as a copy of its
//!   target's definition, carrying the spelling as the name, next to the
//!   canonical name;
//! - **never twice** - a spelling the registry already holds is not advertised a
//!   second time, so `Bash` appears once whatever the table says (TC-013's last
//!   expected result, FR-026);
//! - **never what cannot be called** - an entry whose target this build does not
//!   register is not advertised, because FR-031 refuses such a call at use;
//! - **never a retired spelling** - FR-031's outbound half, for every row of
//!   FR-023's rename table.
//!
//! The table is data and the projection is a pure function of the registry and
//! the table, so nothing here opens a provider, a socket or the filesystem.

use std::collections::BTreeSet;

use ragent_agent::tool::ToolRegistry;
use ragent_agent::tool::alias_table::AliasTable;
use ragent_agent::tool::create_default_registry;
use ragent_agent::tool::rename::TOOL_RENAMES;
use ragent_config::ToolsConfig;
use ragent_types::ToolDefinition;

/// Builds a `ToolsConfig` from an in-memory `tools` section.
fn tools(json: &str) -> ToolsConfig {
    serde_json::from_str(json).unwrap_or_else(|e| panic!("parse {json}: {e}"))
}

/// The advertised names, in the order the list carries them.
fn names(defs: &[ToolDefinition]) -> Vec<String> {
    defs.iter().map(|def| def.name.clone()).collect()
}

/// A small registry: `read`, `think`, `calculator` and `Bash`, so the cases do
/// not depend on the built-in table's targets existing in a full registry.
fn small_registry() -> ToolRegistry {
    let registry = ToolRegistry::new();
    for tool in create_default_registry().all_tools() {
        if matches!(tool.name(), "read" | "think" | "calculator" | "Bash") {
            registry.register(tool);
        }
    }
    registry
}

// ---------------------------------------------------------------------------
// Off: the outbound list is exactly the registry's own names
// ---------------------------------------------------------------------------

#[test]
fn test_advertising_off_advertises_only_registry_names() {
    let registry = small_registry();
    let table = AliasTable::from_tools_config(&tools(r#"{"aliases": true}"#));
    assert!(!table.is_empty(), "the built-in table must be non-empty");

    let advertised = table.outbound_definitions(&registry.definitions(), false);
    assert_eq!(
        names(&advertised),
        names(&registry.definitions()),
        "with `expose_alias_names` off the list must be the registry's own names, \
         in the registry's own order (FR-015)"
    );
}

#[test]
fn test_an_empty_table_advertises_nothing_even_when_the_switch_is_on() {
    let registry = small_registry();
    let table = AliasTable::from_tools_config(&ToolsConfig::default());
    assert!(table.is_empty());

    let advertised = table.outbound_definitions(&registry.definitions(), true);
    assert_eq!(
        names(&advertised),
        names(&registry.definitions()),
        "a switch with no table behind it changes nothing: there is no spelling to \
         advertise (FR-015)"
    );
}

// ---------------------------------------------------------------------------
// On: each alias spelling is advertised alongside its canonical name
// ---------------------------------------------------------------------------

#[test]
fn test_advertising_on_adds_one_entry_per_resolvable_alias_spelling() {
    let registry = small_registry();
    let table = AliasTable::from_tools_config(&tools(r#"{"aliases": {"Frobnicate": "think"}}"#));

    let advertised = table.outbound_definitions(&registry.definitions(), true);
    let advertised_names = names(&advertised);

    assert!(
        advertised_names.iter().any(|name| name == "think"),
        "the canonical name is still advertised"
    );
    assert!(
        advertised_names.iter().any(|name| name == "Frobnicate"),
        "the alias spelling is advertised alongside the canonical name, got \
         {advertised_names:?}"
    );
    assert_eq!(
        advertised.len(),
        registry.definitions().len() + 1,
        "exactly one entry is added for one alias entry"
    );
}

#[test]
fn test_an_advertised_alias_copies_its_targets_definition() {
    let registry = small_registry();
    let table = AliasTable::from_tools_config(&tools(r#"{"aliases": {"Frobnicate": "think"}}"#));
    let canonical = registry
        .definitions()
        .iter()
        .find(|def| def.name == "think")
        .cloned()
        .expect("the registry holds `think`");

    let advertised = table.outbound_definitions(&registry.definitions(), true);
    let alias = advertised
        .iter()
        .find(|def| def.name == "Frobnicate")
        .expect("the alias is advertised");

    assert_eq!(
        alias.description, canonical.description,
        "an advertised alias carries the target's description, so the model is told \
         what the spelling actually does"
    );
    assert_eq!(
        alias.parameters, canonical.parameters,
        "an advertised alias carries the target's parameter schema, so a call through \
         the spelling is shaped like a call through the canonical name"
    );
    assert_eq!(alias.name, "Frobnicate", "only the name differs");
}

#[test]
fn test_every_builtin_entry_is_advertisable() {
    let registry = create_default_registry();
    let table = AliasTable::from_tools_config(&tools(r#"{"aliases": true}"#));

    // Built-in targets do network I/O, so they are never *dispatched* here; the
    // projection only reads their definitions, which is offline.
    let advertised = table.outbound_definitions(&registry.definitions(), true);
    let advertised_names: BTreeSet<&String> = advertised.iter().map(|d| &d.name).collect();

    for (spelling, target) in table.entries() {
        assert!(
            advertised_names.contains(&spelling.to_string()),
            "the built-in entry `{spelling} -> {target}` must be advertised when the \
             switch is on"
        );
        assert!(
            advertised_names.contains(&target.to_string()),
            "the canonical name `{target}` must stay advertised next to its spelling"
        );
    }
    assert_eq!(
        advertised.len(),
        registry.definitions().len() + table.len(),
        "each built-in entry adds exactly one advertisement"
    );
}

#[test]
fn test_the_advertised_list_is_sorted_by_name() {
    let registry = create_default_registry();
    let table = AliasTable::from_tools_config(&tools(
        r#"{"aliases": {"zzz_provider_spelling": "think", "Frobnicate": "calculator"}}"#,
    ));

    let advertised = table.outbound_definitions(&registry.definitions(), true);
    assert!(
        advertised
            .windows(2)
            .all(|pair| pair[0].name <= pair[1].name),
        "the outbound list is ascending by name like every other registry \
         enumeration (FR-030)"
    );
    let sorted_names = names(&advertised);
    assert_eq!(
        sorted_names.first().map(String::as_str),
        Some("AskUserQuestion"),
        "the `A`-initial canonical names still sort first"
    );
    assert_eq!(
        sorted_names.last().map(String::as_str),
        Some("zzz_provider_spelling"),
        "an advertised spelling sorts among the canonical names, not after all of them"
    );
}

// ---------------------------------------------------------------------------
// Never twice (TC-013's last expected result, FR-026)
// ---------------------------------------------------------------------------

#[test]
fn test_a_registered_spelling_is_never_advertised_twice() {
    let registry = small_registry();
    // `Bash` is a registered name after the FR-023 rename, so an entry that maps
    // it elsewhere must not add a second advertisement of it: a real
    // registration always wins (FR-026).
    let table = AliasTable::from_tools_config(&tools(r#"{"aliases": {"Bash": "think"}}"#));

    let advertised = table.outbound_definitions(&registry.definitions(), true);
    let bash_count = advertised.iter().filter(|def| def.name == "Bash").count();
    assert_eq!(
        bash_count, 1,
        "`Bash` is a registered name and appears exactly once, never as two entries"
    );

    let bash = advertised
        .iter()
        .find(|def| def.name == "Bash")
        .expect("`Bash` is advertised");
    let registered = registry
        .definitions()
        .iter()
        .find(|def| def.name == "Bash")
        .cloned()
        .expect("`Bash` is registered");
    assert_eq!(
        bash.description, registered.description,
        "the surviving `Bash` entry is the registry's own definition, not a copy of \
         the entry's target"
    );
}

#[test]
fn test_no_name_is_advertised_twice_for_any_builtin_entry() {
    let registry = create_default_registry();
    let table = AliasTable::from_tools_config(&tools(r#"{"aliases": true}"#));

    let advertised = table.outbound_definitions(&registry.definitions(), true);
    let unique: BTreeSet<String> = advertised.iter().map(|d| d.name.clone()).collect();
    assert_eq!(
        unique.len(),
        advertised.len(),
        "the outbound list repeats no name: a duplicate would advertise one tool twice \
         and make the model choose between two spellings of it (FR-026)"
    );
}

// ---------------------------------------------------------------------------
// Never what cannot be called (FR-031)
// ---------------------------------------------------------------------------

#[test]
fn test_an_entry_whose_target_is_unregistered_is_not_advertised() {
    let registry = small_registry();
    let table = AliasTable::from_tools_config(&tools(
        r#"{"aliases": {"Frobnicate": "bridged_after_config_load"}}"#,
    ));

    let advertised = table.outbound_definitions(&registry.definitions(), true);
    assert!(
        !advertised.iter().any(|def| def.name == "Frobnicate"),
        "an entry whose target this build does not register is refused when used \
         (FR-031), so advertising it would offer the model a call that cannot run"
    );
    assert_eq!(
        advertised.len(),
        registry.definitions().len(),
        "nothing is added when no entry resolves"
    );
}

#[test]
fn test_only_the_resolvable_entries_are_advertised() {
    let registry = small_registry();
    let table = AliasTable::from_tools_config(&tools(
        r#"{"aliases": {"Frobnicate": "think", "Bogus": "not_a_tool"}}"#,
    ));

    let advertised = table.outbound_definitions(&registry.definitions(), true);
    let advertised_names = names(&advertised);
    assert!(advertised_names.iter().any(|name| name == "Frobnicate"));
    assert!(
        !advertised_names.iter().any(|name| name == "Bogus"),
        "the unresolvable entry is skipped, not the whole table"
    );
}

// ---------------------------------------------------------------------------
// Never a retired spelling (FR-031, the outbound half)
// ---------------------------------------------------------------------------

#[test]
fn test_no_retired_spelling_is_ever_advertised() {
    let registry = create_default_registry();

    for row in TOOL_RENAMES {
        // The table refuses to hold such a spelling at all (the inbound half,
        // FR-031), so this half is assembled through the config the loader would
        // reject: `from_aliases` drops the entry, and the projection is checked
        // for the name regardless.
        let table = AliasTable::from_tools_config(&tools(&format!(
            r#"{{"aliases": {{"{}": "think"}}}}"#,
            row.before
        )));
        let advertised = table.outbound_definitions(&registry.definitions(), true);
        assert!(
            !advertised.iter().any(|def| def.name == row.before),
            "FR-031: `tools.expose_alias_names` must never advertise the retired \
             spelling `{}`",
            row.before
        );
    }
}

#[test]
fn test_advertising_never_renames_a_canonical_entry() {
    let registry = create_default_registry();
    let table = AliasTable::from_tools_config(&tools(r#"{"aliases": {"Frobnicate": "think"}}"#));

    let advertised = table.outbound_definitions(&registry.definitions(), true);
    for canonical in registry.definitions().iter() {
        let advertised_entry = advertised
            .iter()
            .find(|def| def.name == canonical.name)
            .unwrap_or_else(|| panic!("`{}` must stay advertised", canonical.name));
        assert_eq!(
            advertised_entry.description, canonical.description,
            "advertising adds entries and changes no existing one: `{}` is untouched",
            canonical.name
        );
        assert_eq!(
            advertised_entry.parameters, canonical.parameters,
            "the schema of `{}` is the registry's own",
            canonical.name
        );
    }
}
