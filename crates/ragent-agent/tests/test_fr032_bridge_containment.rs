//! Guard: a refused dynamic bridge costs one tool, is reported once, and leaves
//! the session usable (spec `uniformtools` FR-032).
//!
//! The rule itself is proved in `crates/ragent-types/tests/test_fr032_dynamic_bridge.rs`.
//! This file proves the two properties the *bridge* is responsible for, which the
//! rule cannot promise on its own:
//!
//! 1. **containment** - a tool that may not be registered is absent from the
//!    wrappers while every other tool from the same server is present, so a stale
//!    spelling costs one tool and not the server;
//! 2. **once, and never fatal** - the refusal reports the first time that source
//!    offers it, is silent on a reconnect, and does not panic.
//!
//! ## Why the backstop needs a test-supplied mangler
//!
//! With the real one, [`McpToolWrapper::ragent_name_for`] always produces
//! `mcp_<server>_<tool>`, so [`check_dynamic_bridge`] can never refuse and the
//! containment property would be vacuous. [`bridge_server_tools_named`] takes the
//! name function as a parameter precisely so this file can drive the shape of the
//! regression the backstop exists for - a call site that stopped namespacing - and
//! show that it costs exactly one tool.
//!
//! ## Why every case is `#[serial]`
//!
//! The refusal ledger is process-wide (that is what makes "once" mean once across
//! every session), and this crate's consolidated `--test all` binary shares one
//! process across every child file. Two cases here resetting and then asserting on
//! the same ledger would race without it.

use std::sync::Arc;

use ragent_agent::mcp::{McpClient, McpToolDef};
use ragent_agent::tool::McpToolWrapper;
use ragent_agent::tool::mcp_tool::{bridge_server_tools, bridge_server_tools_named};
use ragent_types::tool_bridge::{
    BridgeRefusal, BridgeRefusalReason, check_dynamic_bridge, mcp_tool_name,
    report_bridge_refusals_once, reset_bridge_refusals_for_test,
};
use serde_json::json;
use serial_test::serial;
use tokio::sync::RwLock;

/// An empty, never-connected client: the wrappers only store the handle, so no
/// MCP process is started by any case here.
fn idle_client() -> Arc<RwLock<McpClient>> {
    Arc::new(RwLock::new(McpClient::new()))
}

/// One advertised tool with a permissive object schema.
fn tool(name: &str) -> McpToolDef {
    McpToolDef {
        name: name.to_string(),
        description: format!("the {name} tool"),
        parameters: json!({"type": "object", "properties": {}}),
    }
}

/// The registry names the bridge produced, in advertisement order.
fn names(bridged: &ragent_agent::tool::mcp_tool::BridgedServerTools) -> Vec<&str> {
    bridged
        .wrappers
        .iter()
        .map(|wrapper| wrapper.ragent_name.as_str())
        .collect()
}

#[test]
#[serial]
fn the_real_mangler_namespaces_every_tool_of_a_server() {
    reset_bridge_refusals_for_test();
    let tools = vec![
        tool("notes_read"),
        tool("bash"),
        tool("Bash"),
        tool("do-it"),
    ];
    let bridged = bridge_server_tools("srv", &tools, idle_client());

    assert!(
        bridged.refusals.is_empty(),
        "a namespaced tool is never refused: {:?}",
        bridged.refusals
    );
    assert_eq!(
        names(&bridged),
        vec![
            "mcp_srv_notes_read",
            "mcp_srv_bash",
            "mcp_srv_Bash",
            "mcp_srv_do_it",
        ],
        "every tool bridges, in advertisement order, under the server's namespace"
    );
    // The server-local `bash`/`Bash` pair survives as two distinct namespaced
    // tools: namespacing neither collapses them nor resurrects the bare `bash`.
    assert!(
        !names(&bridged).contains(&"bash"),
        "a namespaced tool may not equal the retired spelling"
    );
    assert!(check_dynamic_bridge("MCP server `srv`", "mcp_srv_", "mcp_srv_bash").is_ok());
}

#[test]
#[serial]
fn a_refused_tool_costs_one_tool_and_the_rest_of_the_server_keeps_working() {
    reset_bridge_refusals_for_test();
    let tools = vec![tool("notes_read"), tool("bash"), tool("keep_me")];

    // The regression shape: one arm of a per-tool branch forgot to namespace, so
    // `bash` arrives bare while its siblings are namespaced. Only `bash` is
    // refused; the server keeps the other two.
    let bridged = bridge_server_tools_named("srv", &tools, idle_client(), |server, tool_name| {
        if tool_name == "bash" {
            tool_name.to_string()
        } else {
            mcp_tool_name(server, tool_name)
        }
    });

    assert_eq!(
        bridged.refusals.len(),
        1,
        "exactly the one bare retired spelling is refused: {:?}",
        bridged.refusals
    );
    let refusal = &bridged.refusals[0];
    assert_eq!(refusal.surfaced, "bash");
    assert_eq!(refusal.source, "MCP server `srv`");
    let sentence = refusal.sentence();
    assert!(
        sentence.contains("`Bash`"),
        "the replacement is named: {sentence}"
    );

    assert_eq!(
        names(&bridged),
        vec!["mcp_srv_notes_read", "mcp_srv_keep_me"],
        "the server keeps its other tools - refusing the whole server would punish \
         the wrong party (FR-032)"
    );
}

#[test]
#[serial]
fn a_bridge_that_stopped_namespacing_refuses_every_tool_but_still_returns() {
    reset_bridge_refusals_for_test();
    let tools = vec![tool("bash"), tool("edit"), tool("notes_read")];
    let bridged = bridge_server_tools_named("srv", &tools, idle_client(), |_server, tool_name| {
        tool_name.to_string()
    });

    assert_eq!(
        bridged.refusals.len(),
        3,
        "every bare name is refused: {:?}",
        bridged.refusals
    );
    assert!(
        bridged.wrappers.is_empty(),
        "an empty contribution is a loss, not a failure - the caller registers nothing \
         and the session stays usable"
    );
    // The two retired names get the sentence that names their replacement; the
    // third gets the namespacing one.
    let sentences: Vec<String> = bridged
        .refusals
        .iter()
        .map(BridgeRefusal::sentence)
        .collect();
    assert!(sentences[0].contains("`Bash`"), "{}", sentences[0]);
    assert!(sentences[1].contains("`Edit`"), "{}", sentences[1]);
    assert!(sentences[2].contains("mcp_srv_"), "{}", sentences[2]);
    for sentence in &sentences {
        assert!(
            sentence.contains("keeps working"),
            "every refusal must say the rest of the source keeps working: {sentence}"
        );
    }
}

#[test]
#[serial]
fn a_reconnect_reports_the_same_refusal_once() {
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
        1
    );
    assert_eq!(
        report_bridge_refusals_once(std::slice::from_ref(&refusal)),
        0,
        "the TUI re-runs its bridge on every housekeeping pass; a refusal that \
         reported each time would be noise the reader learns to ignore"
    );
}

#[test]
#[serial]
fn the_wrapper_reports_the_same_prefix_the_bridge_checks() {
    // The two derivations that must never disagree: the prefix the stale-server
    // scan looks for, and the prefix the FR-032 check demands.
    for server in ["srv", "mongodb.mongodb", "my-server"] {
        let prefix = McpToolWrapper::ragent_prefix_for(server);
        let name = McpToolWrapper::ragent_name_for(server, "x");
        assert_eq!(
            check_dynamic_bridge("source", &prefix, &name),
            Ok(()),
            "`{name}` must satisfy the prefix `{prefix}` the scan uses"
        );
        assert_eq!(
            McpToolWrapper::ragent_name_for(server, "x"),
            mcp_tool_name(server, "x"),
            "the wrapper and `ragent-types` must mangle identically"
        );
    }
}
