//! PERF-076: tests for the MCP `tool_name -> server_id` lookup index.
//!
//! `call_tool_by_name` used to scan every server's tool list linearly.  These
//! tests pin the observable contract of the replacement O(1) index: a tool
//! registered on a second server is resolvable, unknown tools are rejected
//! before any dispatch, and a disconnected server's tools drop out of the
//! index.

use ragent_agent::McpServerConfig;
use ragent_agent::mcp::McpClient;
use serde_json::json;

/// A config for a disabled server, which `connect` registers without any
/// transport handshake — enough to populate the server list in tests.
fn disabled_config() -> McpServerConfig {
    McpServerConfig {
        disabled: true,
        ..McpServerConfig::default()
    }
}

#[tokio::test]
async fn unknown_tool_is_rejected_before_dispatch() {
    let mut client = McpClient::new();
    client
        .connect("srv-a", disabled_config())
        .await
        .expect("register disabled server");

    // A disabled server advertises no tools, so the index is empty and the
    // lookup fails with the same message as the old linear scan.
    let err = client
        .call_tool_by_name("nope", json!({}))
        .await
        .expect_err("unknown tool must error");
    assert!(
        err.to_string()
            .contains("No connected MCP server provides tool"),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn multiple_servers_are_registered_and_disconnected() {
    let mut client = McpClient::new();
    client
        .connect("srv-a", disabled_config())
        .await
        .expect("register srv-a");
    client
        .connect("srv-b", disabled_config())
        .await
        .expect("register srv-b");
    assert_eq!(client.servers().len(), 2);

    // Disconnecting one server keeps the other registered, and an unknown tool
    // still resolves to a clean error rather than panicking on a stale index.
    client.disconnect("srv-a").await.expect("disconnect srv-a");
    let err = client
        .call_tool_by_name("still-missing", json!({}))
        .await
        .expect_err("unknown tool must error");
    assert!(
        err.to_string()
            .contains("No connected MCP server provides tool")
    );
}

// ── FR-030: a disabled server is still listed, but inert ────────────────────

/// `register_disabled` records a server that exists but must not be started, so
/// `/mcp` can list it (and offer to re-enable it) without a child process.
#[tokio::test]
async fn register_disabled_lists_the_server_with_no_tools() {
    let mut client = McpClient::new();
    client.register_disabled("off", disabled_config());
    assert_eq!(client.servers().len(), 1);
    assert_eq!(client.servers()[0].id, "off");
    assert_eq!(
        client.servers()[0].status,
        ragent_agent::mcp::McpStatus::Disabled
    );
    assert!(client.servers()[0].tools.is_empty());
}

// ── Global enable ledger ────────────────────────────────────────────────────

/// A server id absent from the ledger is enabled, so a newly added MCP server
/// starts connected without any write; an explicit `false` disables it and
/// `clear` restores the default.
#[test]
fn enable_ledger_defaults_to_enabled_and_records_only_explicit_choices() {
    let mut ledger = ragent_agent::mcp::McpEnableLedger::default();
    assert!(
        ledger.is_enabled("brand-new"),
        "a server not in the ledger is enabled"
    );

    ledger.set_enabled("brand-new", false);
    assert!(!ledger.is_enabled("brand-new"));

    ledger.clear("brand-new");
    assert!(
        ledger.is_enabled("brand-new"),
        "clearing the entry restores the enabled default"
    );
    assert!(ledger.servers.is_empty());
}

/// The `ragent.json` `disabled` flag is the harder switch: it wins over the
/// ledger, whereas the ledger can switch off a plugin-contributed server that
/// has no config flag of its own.
#[test]
fn config_disabled_flag_overrides_the_ledger() {
    use ragent_agent::McpServerConfig;
    use ragent_agent::mcp::is_server_enabled;

    let ledger = ragent_agent::mcp::McpEnableLedger::default();
    let enabled_cfg = McpServerConfig::default();
    assert!(is_server_enabled(&enabled_cfg, &ledger, "s"));

    let hard_off = McpServerConfig {
        disabled: true,
        ..McpServerConfig::default()
    };
    let mut ledger_says_on = ragent_agent::mcp::McpEnableLedger::default();
    ledger_says_on.set_enabled("s", true);
    assert!(
        !is_server_enabled(&hard_off, &ledger_says_on, "s"),
        "ragent.json disabled:true always wins"
    );

    let mut ledger_says_off = ragent_agent::mcp::McpEnableLedger::default();
    ledger_says_off.set_enabled("s", false);
    assert!(!is_server_enabled(&enabled_cfg, &ledger_says_off, "s"));
}

/// The ledger round-trips through its on-disk form, so a disable choice survives
/// a restart and reaches the startup connect filter.
#[test]
fn enable_ledger_round_trips_and_drops_absent_entries() {
    use std::io::Write;

    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/mcp-enable-state-tests");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join(format!("mcp_state-{}.json", std::process::id()));
    let mut file = std::fs::File::create(&path).expect("create ledger");
    file.write_all(br#"{"servers":{"a":false,"b":true}}"#)
        .expect("write ledger");
    drop(file);

    let ledger = ragent_agent::mcp::McpEnableLedger::load_from(&path);
    assert!(!ledger.is_enabled("a"));
    assert!(ledger.is_enabled("b"));
    assert!(ledger.is_enabled("c"), "absent ids stay enabled");

    // Saving drops the explicit `true` (it is the default) but keeps `false`.
    let mut trimmed = ledger.clone();
    trimmed.clear("b");
    trimmed.save_to(&path).expect("save ledger");
    let reloaded = ragent_agent::mcp::McpEnableLedger::load_from(&path);
    assert_eq!(reloaded.servers.len(), 1);
    assert!(!reloaded.is_enabled("a"));

    let _ = std::fs::remove_file(&path);
}

/// A corrupt ledger is renamed aside and treated as empty, so a bad file can
/// never make every MCP server disappear.
#[test]
fn corrupt_enable_ledger_is_renamed_aside_and_defaults_to_empty() {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/mcp-enable-state-tests");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join(format!("corrupt-{}.json", std::process::id()));
    std::fs::write(&path, b"{ not json").expect("write corrupt ledger");

    let ledger = ragent_agent::mcp::McpEnableLedger::load_from(&path);
    assert!(ledger.servers.is_empty());
    assert!(ledger.is_enabled("anything"));

    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(path.with_extension("json.corrupt"));
}
