//! Generic tests for the MCP `adopt_running` candidate derivation.
//!
//! `McpClient::connect` adopts an already-running server before spawning its
//! own. The candidates come from the config's own declared transport, so these
//! tests pin that derivation itself: a declared HTTP/SSE URL is the candidate,
//! and a stdio entry is only a candidate when its arguments name an HTTP port.
//! The transport-neutral helper that decides is reached directly: it takes only
//! a `McpServerConfig` and does no I/O, so no client or runtime state is needed.

#[cfg(unix)]
use ragent_agent::mcp::orphan_cmdline_matches;
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

// ── Orphaned stdio child matching ───────────────────────────────────────────

/// The real MongoDB MCP start line (`npx -y mongodb-mcp-server@<3`): both the
/// `npx` launcher argv and the node child argv (which shows the package name
/// in the script path) match the same config.
#[cfg(unix)]
#[test]
fn npx_commandline_matches_launcher_and_node_child() {
    let args = vec!["-y".to_string(), "mongodb-mcp-server@<3".to_string()];
    // The npx launcher itself.
    assert!(orphan_cmdline_matches(
        b"npx\0-y\0mongodb-mcp-server@<3\0",
        "npx",
        &args
    ));
    // /usr/bin/npx - the install path must not matter.
    assert!(orphan_cmdline_matches(
        b"/usr/bin/npx\0-y\0mongodb-mcp-server@<3\0",
        "npx",
        &args
    ));
    // The node child npx spawns; only the script path mentions the package.
    assert!(orphan_cmdline_matches(
        b"node\0/home/u/.npm/_npx/abc/node_modules/mongodb-mcp-server/dist/index.js\0",
        "npx",
        &args
    ));
}

/// A *different* package served through the same launcher is not an orphan of
/// this config - `npx -y other-mcp-server` must survive the sweep.
#[cfg(unix)]
#[test]
fn npx_commandline_rejects_another_package() {
    let args = vec!["-y".to_string(), "mongodb-mcp-server@<3".to_string()];
    assert!(!orphan_cmdline_matches(
        b"npx\0-y\0other-mcp-server\0",
        "npx",
        &args
    ));
    assert!(!orphan_cmdline_matches(
        b"node\0/usr/lib/node_modules/other-mcp-server/index.js\0",
        "npx",
        &args
    ));
}

/// A launcher command with no positional package argument cannot be
/// identified safely: `npx` alone matches every npx process on the machine,
/// so the matcher must refuse rather than sweep them all.
#[cfg(unix)]
#[test]
fn npx_without_a_package_matches_nothing() {
    assert!(!orphan_cmdline_matches(
        b"npx\0-y\0",
        "npx",
        &["-y".to_string()]
    ));
    assert!(!orphan_cmdline_matches(b"npx\0", "npx", &[]));
}

/// A plain (non-launcher) command requires every argv basename to be the
/// command name, so `node server.js` matches a configured `node` while a
/// command line mentioning a *different* interpreter does not.
#[cfg(unix)]
#[test]
fn plain_command_requires_all_basenames_to_match() {
    let args = vec!["server.js".to_string()];
    assert!(orphan_cmdline_matches(b"node\0server.js\0", "node", &args));
    assert!(orphan_cmdline_matches(
        b"/usr/bin/node\0/home/u/server.js\0",
        "node",
        &args
    ));
    assert!(!orphan_cmdline_matches(
        b"python\0server.py\0",
        "node",
        &args
    ));
    // An empty cmdline (a kernel thread, or a process exiting mid-scan) never matches.
    assert!(!orphan_cmdline_matches(b"", "node", &args));
    assert!(!orphan_cmdline_matches(b"\0\0", "node", &args));
}

/// A Windows-style `npx.exe` / path-laden command line still matches: the
/// `.exe` suffix and directory components are stripped before comparison.
#[cfg(unix)]
#[test]
fn exe_suffix_and_path_components_are_stripped() {
    let args = vec!["-y".to_string(), "mongodb-mcp-server".to_string()];
    assert!(orphan_cmdline_matches(
        b"C:\\tools\\npx.exe\0-y\0mongodb-mcp-server\0",
        "npx",
        &args
    ));
    assert!(orphan_cmdline_matches(
        b"/usr/bin/node\0x.js\0",
        "/usr/bin/node",
        &["x.js".to_string()]
    ));
}

// ── Orphan detection: ppid gate ─────────────────────────────────────────────

/// The ppid is read from field 4 of `/proc/<pid>/stat`, with the `comm` field
/// skipped by splitting after the *last* `)` so hostile comms (spaces, `)`)
/// cannot shift the column index.
#[cfg(unix)]
#[test]
fn ppid_is_parsed_after_the_last_closing_paren() {
    use ragent_agent::mcp::ppid_from_stat_contents;

    // Ordinary comm.
    assert_eq!(
        ppid_from_stat_contents("1234 (node) S 1 99 100 0 -1 4194560"),
        Some(1)
    );
    // comm containing spaces and a `)` of its own.
    assert_eq!(
        ppid_from_stat_contents("1234 (node worker) (bin) S 4138 99 100 0 -1 4194560"),
        Some(4138)
    );
    // A live ragent child (real parent pid) parses as-is.
    assert_eq!(
        ppid_from_stat_contents("99 (sh) R 555 1 1 0 -1 0"),
        Some(555)
    );
    // Malformed contents never produce a bogus ppid.
    assert_eq!(ppid_from_stat_contents(""), None);
    assert_eq!(ppid_from_stat_contents("no parens at all"), None);
    assert_eq!(ppid_from_stat_contents("1 (x) S"), None);
    assert_eq!(ppid_from_stat_contents("1 (x) S notanumber"), None);
}

/// `read_ppid_of` against the live process table: the current process has a
/// real parent; a pid that cannot exist returns `None` (the "exited mid-scan"
/// path), which callers treat as "not an orphan".
#[cfg(unix)]
#[test]
fn read_ppid_of_reads_this_process_and_rejects_missing_pids() {
    use ragent_agent::mcp::read_ppid_of;

    let ppid = read_ppid_of(std::process::id()).expect("current process has a stat");
    assert!(ppid >= 1);
    // PID 4_194_304 is the default kernel pid_max upper bound; it cannot exist.
    assert_eq!(read_ppid_of(4_194_304), None);
}
