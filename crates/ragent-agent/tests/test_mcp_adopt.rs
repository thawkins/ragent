//! Generic tests for the MCP `adopt_running` candidate derivation.
//!
//! `McpClient::connect` adopts an already-running server before spawning its
//! own. The candidates come from the config's own declared transport, so these
//! tests pin that derivation itself: a declared HTTP/SSE URL is the candidate,
//! and a stdio entry is only a candidate when its arguments name an HTTP port.
//! The transport-neutral helper that decides is reached directly: it takes only
//! a `McpServerConfig` and does no I/O, so no client or runtime state is needed.

use ragent_agent::mcp::{adopt_candidate_urls, declared_port, parse_port};
use ragent_config::{McpServerConfig, McpTransport};

/// Helper: a stdio config with the given arguments and no command.
fn stdio(args: &[&str]) -> McpServerConfig {
    McpServerConfig {
        type_: McpTransport::Stdio,
        args: args.iter().map(|a| (*a).to_string()).collect(),
        ..McpServerConfig::default()
    }
}

// ── Declared-port extraction ────────────────────────────────────────────────

/// `/npx -y mongodb-mcp-server` declares no port, so nothing is probed.
#[test]
fn stdio_without_a_port_declares_no_candidates() {
    let config = stdio(&["-y", "mongodb-mcp-server"]);
    assert!(adopt_candidate_urls(&config).is_empty());
}

/// `--httpPort 3000` is the MongoDB MCP server's own spelling, and must be
/// recognised so the stdio entry can adopt a copy already serving on that port.
#[test]
fn stdio_with_http_port_flag_declares_the_loopback_candidates() {
    let config = stdio(&["-y", "mongodb-mcp-server@<3", "--httpPort", "3000"]);
    assert_eq!(
        adopt_candidate_urls(&config),
        vec![
            "http://127.0.0.1:3000/mcp".to_string(),
            "http://127.0.0.1:3000".to_string(),
        ]
    );
}

/// The `=port` and `--port` spellings other MCP servers use are recognised too.
#[test]
fn stdio_recognises_other_port_flag_spellings() {
    assert_eq!(declared_port(&["--http-port=8081".to_string()]), Some(8081));
    assert_eq!(
        declared_port(&["--port".to_string(), "9090".to_string()]),
        Some(9090)
    );
    assert_eq!(
        declared_port(&["-p".to_string(), "65535".to_string()]),
        Some(65535)
    );
}

/// A non-numeric or out-of-range value is not a port, so it must not become a
/// probe candidate.
#[test]
fn stdio_ignores_non_port_values() {
    assert_eq!(
        declared_port(&["--port".to_string(), "abc".to_string()]),
        None
    );
    assert_eq!(
        declared_port(&["--port".to_string(), "0".to_string()]),
        None
    );
    assert_eq!(parse_port("99999"), None);
}

// ── Declared-transport candidates ───────────────────────────────────────────

/// An `http` entry's configured URL is its only candidate.
#[test]
fn http_transport_uses_the_configured_url() {
    let config = McpServerConfig {
        type_: McpTransport::Http,
        url: Some("http://example.internal:8123/mcp".to_string()),
        ..McpServerConfig::default()
    };
    assert_eq!(
        adopt_candidate_urls(&config),
        vec!["http://example.internal:8123/mcp".to_string()]
    );
}
