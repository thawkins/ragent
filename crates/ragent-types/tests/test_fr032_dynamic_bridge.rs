//! Contract for the dynamic-bridge rule (spec `uniformtools` FR-032).
//!
//! FR-032 has two clauses and this file pins both, in the order a reader needs
//! them:
//!
//! 1. **namespacing** - an MCP or connector tool is named under its server
//!    (`mcp_<server>_<tool>`) and a plugin tool under its plugin
//!    (`plugin_<id>_<tool>`), so it cannot occupy a bare ragent name at all;
//! 2. **the backstop** - a surfaced name that reaches the bare namespace anyway is
//!    refused with a sentence naming the source, the spelling and its `after`
//!    replacement, reported once rather than once per turn, and contained to the
//!    one tool.
//!
//! The three properties the requirement states positively are asserted here: the
//! refusal names what a reader needs, it reports *once*, and it is per tool so a
//! source keeps its other tools.

use ragent_types::tool_bridge::{
    BridgeRefusal, BridgeRefusalReason, check_dynamic_bridge, mcp_source_label, mcp_tool_name,
    mcp_tool_prefix, plugin_source_label, plugin_tool_name, plugin_tool_prefix,
    report_bridge_refusals_once, reset_bridge_refusals_for_test,
};
use ragent_types::tool_rename::TOOL_RENAMES;

// -- clause 1: namespacing ---------------------------------------------------

#[test]
fn mcp_tool_names_are_always_namespaced_under_the_server() {
    assert_eq!(mcp_tool_name("srv", "notes_read"), "mcp_srv_notes_read");
    // The mangling folds the three characters that occur in real ids and tool
    // names, in both segments, so a name stays one identifier-shaped word.
    assert_eq!(mcp_tool_name("my-server", "do-it"), "mcp_my_server_do_it");
    assert_eq!(
        mcp_tool_name("mongodb.mongodb", "coll/read"),
        "mcp_mongodb_mongodb_coll_read"
    );
    // The prefix is the name minus the tool segment, which is what the TUI's
    // stale-server scan matches on.
    assert_eq!(mcp_tool_prefix("mongodb.mongodb"), "mcp_mongodb_mongodb_");
    for server in ["srv", "my-server", "mongodb.mongodb", "s"] {
        for tool in ["read", "do-it", "a.b/c"] {
            let name = mcp_tool_name(server, tool);
            assert!(
                name.starts_with(&mcp_tool_prefix(server)),
                "`{name}` must carry `{}`",
                mcp_tool_prefix(server)
            );
            assert!(name.starts_with("mcp_"), "`{name}` must be namespaced");
        }
    }
}

#[test]
fn plugin_tool_names_are_always_namespaced_under_the_plugin() {
    assert_eq!(
        plugin_tool_name("codex-weather", "get_weather"),
        "plugin_codex-weather_get_weather"
    );
    assert_eq!(plugin_tool_prefix("codex-weather"), "plugin_codex-weather_");

    // A plugin id keeps its own spelling: it keys the store entry and the
    // `plugin:<id>` permission category, so the registry name must not fold it.
    assert!(plugin_tool_name("codex-weather", "x").contains("codex-weather"));
    for spelling in ["bash", "Bash", "notes_read"] {
        let name = plugin_tool_name("p", spelling);
        assert!(name.starts_with("plugin_p_"), "`{name}` must be namespaced");
    }
}

#[test]
fn no_dynamic_name_can_equal_a_bare_registry_name() {
    // The property clause 1 buys: for every retired and every adopted spelling,
    // no bridged form of it is that spelling. A source cannot resurrect a name it
    // cannot spell.
    for pair in TOOL_RENAMES {
        for spelling in [pair.before, pair.after] {
            assert_ne!(mcp_tool_name("srv", spelling), spelling);
            assert_ne!(plugin_tool_name("p", spelling), spelling);
            // The bridged form is namespaced, so the check accepts it: a server
            // that offers a tool called `bash` still bridges, under `mcp_srv_bash`.
            assert!(
                check_dynamic_bridge(
                    "MCP server `srv`",
                    "mcp_srv_",
                    &mcp_tool_name("srv", spelling)
                )
                .is_ok(),
                "`{spelling}` bridged as a namespaced name must not be refused"
            );
        }
    }
}

#[test]
fn the_rule_is_prefix_agnostic_so_a_new_bridge_uses_its_own_namespace() {
    // A connector's servers are bridged as `<connector-id>.<server-id>` and reach
    // the MCP bridge, so they inherit `mcp_`. A skill contributes no tool in this
    // build (FR-029: `allowed-tools` restricts the registry, it does not add to
    // it), so no `skill_` prefix is declared - but the rule takes the prefix as a
    // parameter, so the first bridge that does contribute one applies the same two
    // clauses with its own namespace rather than inventing a third policy.
    for (prefix, namespaced, bare) in [
        ("skill_rename-check_", "skill_rename-check_bash", "bash"),
        ("acp_agent_", "acp_agent_edit", "edit"),
        ("backend_remote_", "backend_remote_glob", "glob"),
    ] {
        assert!(
            check_dynamic_bridge("some source", prefix, namespaced).is_ok(),
            "`{namespaced}` is namespaced under `{prefix}` and must bridge"
        );
        let refusal = check_dynamic_bridge("some source", prefix, bare)
            .expect_err("a bare retired spelling is refused whatever the source kind");
        assert!(matches!(
            refusal.reason,
            BridgeRefusalReason::RetiredSpelling { .. }
        ));
    }
}

// -- clause 2: the backstop --------------------------------------------------

#[test]
fn a_namespaced_tool_bridges_even_when_its_server_side_name_is_retired() {
    // `mcp_shell_bash` is not `bash`: a shell server is exactly the source that
    // would offer a tool called `bash`, and refusing it would break a legitimate
    // integration to satisfy a sentence written about bare names.
    for pair in TOOL_RENAMES {
        let name = mcp_tool_name("srv", pair.before);
        check_dynamic_bridge("MCP server `srv`", &mcp_tool_prefix("srv"), &name)
            .unwrap_or_else(|e| panic!("`{name}` must bridge: {}", e.sentence()));
    }
}

#[test]
fn a_bridge_that_stops_namespacing_is_refused_with_the_replacement_named() {
    // The shape of the regression the backstop exists for: a call site that passes
    // the advertised name through unchanged. `bash` is both bare and retired, so
    // the sentence that names `Bash` is the one a reader gets.
    let refusal = check_dynamic_bridge("plugin `p`", &plugin_tool_prefix("p"), "bash")
        .expect_err("a bare `bash` must be refused");
    assert_eq!(
        refusal.reason,
        BridgeRefusalReason::RetiredSpelling {
            spelling: "bash",
            after: "Bash",
        }
    );
    assert_eq!(refusal.source, "plugin `p`");
    assert_eq!(refusal.surfaced, "bash");
    let sentence = refusal.sentence();
    for needle in ["plugin `p`", "`bash`", "`Bash`", "FR-032"] {
        assert!(
            sentence.contains(needle),
            "the sentence must name {needle}: {sentence}"
        );
    }

    // Every retired spelling produces the same shape of refusal, naming its own
    // replacement rather than a generic one.
    for pair in TOOL_RENAMES {
        let refusal = check_dynamic_bridge("MCP server `s`", &mcp_tool_prefix("s"), pair.before)
            .expect_err("a retired spelling is bare and must be refused");
        let sentence = refusal.sentence();
        assert!(sentence.contains(pair.before), "{sentence}");
        assert!(sentence.contains(pair.after), "{sentence}");
    }
}

#[test]
fn any_other_bare_name_is_refused_for_not_being_namespaced() {
    let refusal = check_dynamic_bridge("MCP server `srv`", &mcp_tool_prefix("srv"), "Bash")
        .expect_err("a bare adopted spelling is not a plugin/mcp name");
    assert_eq!(
        refusal.reason,
        BridgeRefusalReason::NotNamespaced {
            prefix: "mcp_srv_".to_string(),
        }
    );
    let sentence = refusal.sentence();
    assert!(sentence.contains("mcp_srv_"), "{sentence}");
    assert!(sentence.contains("FR-032"), "{sentence}");

    // A name under a *different* source's prefix is refused too: crossing sources
    // is how one server would claim another's tools.
    assert!(
        check_dynamic_bridge("MCP server `srv`", &mcp_tool_prefix("srv"), "mcp_other_x").is_err()
    );
    // The prefix alone (an empty tool segment) is still namespaced; the registry
    // collision rules, not this check, decide whether it may be registered.
    assert!(check_dynamic_bridge("MCP server `srv`", &mcp_tool_prefix("srv"), "mcp_srv_").is_ok());
}

#[test]
fn source_labels_name_the_kind_and_the_id() {
    assert_eq!(mcp_source_label("srv"), "MCP server `srv`");
    assert_eq!(
        plugin_source_label("codex-weather"),
        "plugin `codex-weather`"
    );
    // A connector's servers are bridged as `<connector-id>.<server-id>` and reach
    // the same MCP bridge, so their label is the MCP one carrying the bridged id.
    assert_eq!(
        mcp_source_label("postgres.main"),
        "MCP server `postgres.main`"
    );
}

// -- reported once, and never fatal -----------------------------------------

#[test]
fn a_refusal_is_reported_once_per_source_and_name() {
    reset_bridge_refusals_for_test();
    let refusal = BridgeRefusal {
        source: "MCP server `srv`".to_string(),
        surfaced: "bash".to_string(),
        reason: BridgeRefusalReason::RetiredSpelling {
            spelling: "bash",
            after: "Bash",
        },
    };

    assert_eq!(
        report_bridge_refusals_once(std::slice::from_ref(&refusal)),
        1,
        "the first report must emit",
    );
    assert_eq!(
        report_bridge_refusals_once(std::slice::from_ref(&refusal)),
        0,
        "a reconnect of the same server must be silent - `once` is not `once per turn`",
    );

    // A different source offering the same spelling is a different file to fix,
    // so it reports.
    let other = BridgeRefusal {
        source: "plugin `p`".to_string(),
        ..refusal.clone()
    };
    assert_eq!(report_bridge_refusals_once(&[other]), 1);

    // So does a second offender from a source already in the ledger.
    let second = BridgeRefusal {
        source: "MCP server `srv`".to_string(),
        surfaced: "edit".to_string(),
        reason: BridgeRefusalReason::RetiredSpelling {
            spelling: "edit",
            after: "Edit",
        },
    };
    assert_eq!(report_bridge_refusals_once(&[second]), 1);

    assert_eq!(
        report_bridge_refusals_once(&[]),
        0,
        "an empty list reports nothing and does not panic"
    );
}
