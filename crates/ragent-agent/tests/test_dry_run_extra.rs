//! Second-pass integration tests for the dry-run readiness report.
//!
//! The first-pass file `tests/test_dry_run.rs` covers the three headline
//! verdicts (READY/BLOCKED/WARNING) through a single explicit config each.
//! This file drives the *remaining* branches of `src/dry_run.rs` through its
//! public surface only (`run_dry_run`, `ReadinessReport`, the enums/structs).
//!
//! Every test that calls `run_dry_run` first isolates the ambient global
//! config (`XDG_CONFIG_HOME`, `RAGENT_CONFIG`, `RAGENT_CONFIG_CONTENT`) with
//! [`EnvGuard`] so a developer's real `~/.config/ragent/ragent.json` cannot
//! change the verdict, then restores the previous environment on drop. Those
//! process-global mutations are serialised with `serial_test::serial`.
//!
//! `std::env::set_var`/`remove_var` are `unsafe` in Rust 2024; the workspace
//! denies `unsafe_code`, so this test target opts back in explicitly (the
//! same pattern as `test_mcp_connect_timeout.rs` and
//! `crates/ragent-config/tests/test_yolo_persistence.rs`).

#![allow(unsafe_code)]

use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::Arc;

use ragent_agent::ProviderRegistry;
use ragent_agent::dry_run::{
    DryRunInputs, ReadinessItem, ReadinessReport, ReadinessSection, ReadinessStatus,
    ReadinessVerdict,
};
use ragent_agent::provider::create_default_registry as create_provider_registry;
use ragent_agent::tool::{
    Tool, ToolContext, ToolOutput, ToolRegistry, create_default_registry as create_tool_registry,
};
use serde_json::json;
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Environment isolation
// ---------------------------------------------------------------------------

/// Saves an environment variable and restores it (or removes it) on drop.
struct EnvGuard {
    key: &'static str,
    prev: Option<OsString>,
}

impl EnvGuard {
    fn set(key: &'static str, value: impl AsRef<std::ffi::OsStr>) -> Self {
        let prev = std::env::var_os(key);
        // SAFETY: process-global mutation; serialised by `serial_test::serial`.
        unsafe { std::env::set_var(key, value) };
        Self { key, prev }
    }

    fn remove(key: &'static str) -> Self {
        let prev = std::env::var_os(key);
        // SAFETY: process-global mutation; serialised by `serial_test::serial`.
        unsafe { std::env::remove_var(key) };
        Self { key, prev }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        // SAFETY: restoring the value saved in `set`/`remove`.
        unsafe {
            match &self.prev {
                Some(v) => std::env::set_var(self.key, v),
                None => std::env::remove_var(self.key),
            }
        }
    }
}

/// Redirects `XDG_CONFIG_HOME` to an empty temp dir and clears the two config
/// override env vars, so `run_dry_run` observes no ambient configuration.
///
/// The returned `TempDir` and guards must stay bound for the test's duration.
fn isolate() -> (TempDir, EnvGuard, EnvGuard, EnvGuard) {
    let xdg = TempDir::new().expect("temp dir");
    let g1 = EnvGuard::set("XDG_CONFIG_HOME", xdg.path());
    let g2 = EnvGuard::remove("RAGENT_CONFIG");
    let g3 = EnvGuard::remove("RAGENT_CONFIG_CONTENT");
    (xdg, g1, g2, g3)
}

/// Restores the runtime YOLO flag on drop.
struct YoloGuard(bool);

impl YoloGuard {
    fn on() -> Self {
        let prev = ragent_config::yolo::is_enabled();
        ragent_config::yolo::set_enabled(true);
        Self(prev)
    }
}

impl Drop for YoloGuard {
    fn drop(&mut self) {
        ragent_config::yolo::set_enabled(self.0);
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Writes `value` as pretty JSON to `dir/ragent.json` and returns the path.
fn write_json(dir: &TempDir, value: &serde_json::Value) -> PathBuf {
    let path = dir.path().join("ragent.json");
    std::fs::write(
        &path,
        serde_json::to_string_pretty(value).expect("serialise"),
    )
    .expect("write config");
    path
}

/// Builds default dry-run inputs against `working_dir`.
fn inputs(working_dir: PathBuf, config_path: Option<PathBuf>) -> DryRunInputs {
    DryRunInputs {
        config_path,
        agent_name: "general".to_string(),
        model_override: None,
        provider_registry: Arc::new(create_provider_registry()),
        tool_registry: Arc::new(create_tool_registry()),
        working_dir,
        hidden_tools: Vec::new(),
    }
}

fn section<'a>(report: &'a ReadinessReport, name: &str) -> &'a ReadinessSection {
    report
        .sections
        .iter()
        .find(|s| s.name == name)
        .unwrap_or_else(|| panic!("section '{name}' present"))
}

fn item<'a>(sec: &'a ReadinessSection, name: &str) -> &'a ReadinessItem {
    sec.items
        .iter()
        .find(|i| i.name == name)
        .unwrap_or_else(|| panic!("item '{name}' present"))
}

/// A minimal tool whose permission category has no `family:` prefix, so
/// `tools_section` classifies it as uncategorised.
struct EmptyCategoryTool;

#[async_trait::async_trait]
impl Tool for EmptyCategoryTool {
    fn name(&self) -> &'static str {
        "test_empty_category"
    }

    fn description(&self) -> &'static str {
        "test-only tool with an empty permission family"
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({"type": "object", "properties": {}, "additionalProperties": false})
    }

    fn permission_category(&self) -> &'static str {
        ""
    }

    async fn execute(
        &self,
        _input: serde_json::Value,
        _ctx: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        Ok(ToolOutput {
            content: String::new(),
            metadata: None,
        })
    }
}

/// Spawns a one-shot-per-connection HTTP responder on an ephemeral loopback
/// port and returns its address. Used to exercise `check_http_mcp` without a
/// real network dependency.
fn spawn_http_responder(status_line: &'static str) -> std::net::SocketAddr {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind loopback");
    let addr = listener.local_addr().expect("local addr");
    std::thread::spawn(move || {
        let mut served = 0;
        for stream in listener.incoming() {
            let Ok(mut s) = stream else { break };
            let mut buf = [0u8; 1024];
            let _ = s.read(&mut buf);
            let resp =
                format!("HTTP/1.1 {status_line}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
            let _ = s.write_all(resp.as_bytes());
            served += 1;
            if served >= 4 {
                break;
            }
        }
    });
    addr
}

// ---------------------------------------------------------------------------
// config section
// ---------------------------------------------------------------------------

#[tokio::test]
#[serial_test::serial]
async fn test_config_section_defaults_when_no_config_found() {
    let (_xdg, _g1, _g2, _g3) = isolate();
    let work = TempDir::new().expect("work dir");

    let (report, exit) =
        ragent_agent::dry_run::run_dry_run(inputs(work.path().to_path_buf(), None)).await;

    let cfg = section(&report, "config");
    assert_eq!(cfg.status, ReadinessStatus::Ready);
    assert_eq!(cfg.items.len(), 1);
    assert_eq!(cfg.items[0].name, "config");
    assert_eq!(
        cfg.items[0].message,
        "No config files found; using defaults"
    );
    assert_eq!(exit, 0);
}

#[tokio::test]
#[serial_test::serial]
async fn test_config_explicit_invalid_json_blocked() {
    let (_xdg, _g1, _g2, _g3) = isolate();
    let dir = TempDir::new().expect("dir");
    let path = dir.path().join("ragent.json");
    std::fs::write(&path, "{ not valid json").expect("write bad config");

    let (report, exit) =
        ragent_agent::dry_run::run_dry_run(inputs(dir.path().to_path_buf(), Some(path))).await;

    let cfg = section(&report, "config");
    assert_eq!(cfg.status, ReadinessStatus::Blocked);
    assert!(
        cfg.items
            .iter()
            .any(|i| i.message.contains("Failed to parse"))
    );
    assert_eq!(exit, 1);
}

#[tokio::test]
#[serial_test::serial]
async fn test_config_explicit_missing_file_blocked() {
    let (_xdg, _g1, _g2, _g3) = isolate();
    let dir = TempDir::new().expect("dir");
    let path = dir.path().join("does-not-exist.json");

    let (report, exit) =
        ragent_agent::dry_run::run_dry_run(inputs(dir.path().to_path_buf(), Some(path))).await;

    let cfg = section(&report, "config");
    assert_eq!(cfg.status, ReadinessStatus::Blocked);
    assert!(
        cfg.items
            .iter()
            .any(|i| i.message.contains("Failed to read config file"))
    );
    assert_eq!(exit, 1);
}

#[tokio::test]
#[serial_test::serial]
async fn test_global_config_loaded_from_xdg() {
    let xdg = TempDir::new().expect("xdg");
    std::fs::create_dir_all(xdg.path().join("ragent")).expect("mkdir");
    std::fs::write(xdg.path().join("ragent").join("ragent.json"), "{}").expect("write global");
    let _x = EnvGuard::set("XDG_CONFIG_HOME", xdg.path());
    let _c = EnvGuard::remove("RAGENT_CONFIG");
    let _cc = EnvGuard::remove("RAGENT_CONFIG_CONTENT");
    let work = TempDir::new().expect("work");

    let (report, _exit) =
        ragent_agent::dry_run::run_dry_run(inputs(work.path().to_path_buf(), None)).await;

    let cfg = section(&report, "config");
    assert_eq!(cfg.status, ReadinessStatus::Ready);
    assert!(
        cfg.items
            .iter()
            .any(|i| i.message == "Config loaded" && i.name.ends_with("ragent.json"))
    );
}

#[tokio::test]
#[serial_test::serial]
async fn test_global_config_invalid_blocked() {
    let xdg = TempDir::new().expect("xdg");
    std::fs::create_dir_all(xdg.path().join("ragent")).expect("mkdir");
    std::fs::write(
        xdg.path().join("ragent").join("ragent.json"),
        "{ broken global",
    )
    .expect("write global");
    let _x = EnvGuard::set("XDG_CONFIG_HOME", xdg.path());
    let _c = EnvGuard::remove("RAGENT_CONFIG");
    let _cc = EnvGuard::remove("RAGENT_CONFIG_CONTENT");
    let work = TempDir::new().expect("work");

    let (report, exit) =
        ragent_agent::dry_run::run_dry_run(inputs(work.path().to_path_buf(), None)).await;

    let cfg = section(&report, "config");
    assert_eq!(cfg.status, ReadinessStatus::Blocked);
    assert!(
        cfg.items
            .iter()
            .any(|i| i.message.contains("Failed to parse"))
    );
    assert_eq!(exit, 1);
}

#[tokio::test]
#[serial_test::serial]
async fn test_project_config_loaded() {
    let (_xdg, _g1, _g2, _g3) = isolate();
    let work = TempDir::new().expect("work");
    let project_dir = work.path().join(".ragent");
    std::fs::create_dir_all(&project_dir).expect("mkdir project");
    let project_path = project_dir.join("ragent.json");
    std::fs::write(&project_path, "{}").expect("write project config");

    let (report, _exit) =
        ragent_agent::dry_run::run_dry_run(inputs(work.path().to_path_buf(), None)).await;

    let cfg = section(&report, "config");
    assert!(
        cfg.items
            .iter()
            .any(|i| i.name == project_path.display().to_string() && i.message == "Config loaded")
    );
}

#[tokio::test]
#[serial_test::serial]
async fn test_project_config_invalid_blocked() {
    let (_xdg, _g1, _g2, _g3) = isolate();
    let work = TempDir::new().expect("work");
    let project_dir = work.path().join(".ragent");
    std::fs::create_dir_all(&project_dir).expect("mkdir project");
    std::fs::write(project_dir.join("ragent.json"), "{ bad project").expect("write project");

    let (report, exit) =
        ragent_agent::dry_run::run_dry_run(inputs(work.path().to_path_buf(), None)).await;

    let cfg = section(&report, "config");
    assert_eq!(cfg.status, ReadinessStatus::Blocked);
    assert!(
        cfg.items
            .iter()
            .any(|i| i.message.contains("Failed to parse"))
    );
    assert_eq!(exit, 1);
}

#[tokio::test]
#[serial_test::serial]
async fn test_env_config_path_loaded() {
    let (_xdg, _g1, _g2, _g3) = isolate();
    let dir = TempDir::new().expect("dir");
    let env_path = dir.path().join("env-config.json");
    std::fs::write(&env_path, "{}").expect("write env config");
    let _c = EnvGuard::set("RAGENT_CONFIG", &env_path);
    let work = TempDir::new().expect("work");

    let (report, _exit) =
        ragent_agent::dry_run::run_dry_run(inputs(work.path().to_path_buf(), None)).await;

    let cfg = section(&report, "config");
    assert!(
        cfg.items
            .iter()
            .any(|i| i.name == env_path.display().to_string() && i.message == "Config loaded")
    );
}

#[tokio::test]
#[serial_test::serial]
async fn test_env_config_path_invalid_blocked() {
    let (_xdg, _g1, _g2, _g3) = isolate();
    let dir = TempDir::new().expect("dir");
    let env_path = dir.path().join("env-config.json");
    std::fs::write(&env_path, "{ bad env").expect("write env config");
    let _c = EnvGuard::set("RAGENT_CONFIG", &env_path);
    let work = TempDir::new().expect("work");

    let (report, exit) =
        ragent_agent::dry_run::run_dry_run(inputs(work.path().to_path_buf(), None)).await;

    let cfg = section(&report, "config");
    assert_eq!(cfg.status, ReadinessStatus::Blocked);
    assert!(
        cfg.items
            .iter()
            .any(|i| i.message.contains("Failed to parse"))
    );
    assert_eq!(exit, 1);
}

#[tokio::test]
#[serial_test::serial]
async fn test_env_config_content_inline_loaded() {
    let (_xdg, _g1, _g2, _g3) = isolate();
    let _cc = EnvGuard::set("RAGENT_CONFIG_CONTENT", "{}");
    let work = TempDir::new().expect("work");

    let (report, _exit) =
        ragent_agent::dry_run::run_dry_run(inputs(work.path().to_path_buf(), None)).await;

    let cfg = section(&report, "config");
    let inline = item(cfg, "RAGENT_CONFIG_CONTENT");
    assert_eq!(inline.status, ReadinessStatus::Ready);
    assert_eq!(inline.message, "Inline config loaded");
}

#[tokio::test]
#[serial_test::serial]
async fn test_env_config_content_invalid_blocked() {
    let (_xdg, _g1, _g2, _g3) = isolate();
    let _cc = EnvGuard::set("RAGENT_CONFIG_CONTENT", "{ not json");
    let work = TempDir::new().expect("work");

    let (report, exit) =
        ragent_agent::dry_run::run_dry_run(inputs(work.path().to_path_buf(), None)).await;

    let cfg = section(&report, "config");
    assert_eq!(cfg.status, ReadinessStatus::Blocked);
    let inline = item(cfg, "RAGENT_CONFIG_CONTENT");
    assert_eq!(inline.status, ReadinessStatus::Blocked);
    assert!(inline.message.contains("Failed to parse"));
    assert_eq!(exit, 1);
}

// ---------------------------------------------------------------------------
// provider section
// ---------------------------------------------------------------------------

#[tokio::test]
#[serial_test::serial]
async fn test_provider_model_override_applied() {
    let (_xdg, _g1, _g2, _g3) = isolate();
    let dir = TempDir::new().expect("dir");
    let path = write_json(&dir, &json!({"provider": {"anthropic": {"env": []}}}));
    let mut ins = inputs(dir.path().to_path_buf(), Some(path));
    ins.model_override = Some("openai/gpt-4o".to_string());

    let (report, exit) = ragent_agent::dry_run::run_dry_run(ins).await;

    let prov = section(&report, "provider");
    let resolved = item(prov, "resolved");
    assert!(
        resolved.message.contains("openai/gpt-4o"),
        "resolved message was: {}",
        resolved.message
    );
    assert_eq!(exit, 0);
}

#[tokio::test]
#[serial_test::serial]
async fn test_provider_model_override_invalid_format_blocked() {
    let (_xdg, _g1, _g2, _g3) = isolate();
    let dir = TempDir::new().expect("dir");
    let path = write_json(&dir, &json!({"provider": {"anthropic": {"env": []}}}));
    let mut ins = inputs(dir.path().to_path_buf(), Some(path));
    ins.model_override = Some("no-slash-here".to_string());

    let (report, exit) = ragent_agent::dry_run::run_dry_run(ins).await;

    let prov = section(&report, "provider");
    assert_eq!(prov.status, ReadinessStatus::Blocked);
    let bad = item(prov, "model_override");
    assert_eq!(bad.status, ReadinessStatus::Blocked);
    assert!(bad.message.contains("Invalid --model format"));
    assert_eq!(exit, 1);
}

#[tokio::test]
#[serial_test::serial]
async fn test_provider_no_model_resolvable_blocked() {
    let (_xdg, _g1, _g2, _g3) = isolate();
    let work = TempDir::new().expect("work");
    let mut ins = inputs(work.path().to_path_buf(), None);
    // An empty provider registry means `resolve_default_model` finds nothing.
    ins.provider_registry = Arc::new(ProviderRegistry::new());

    let (report, exit) = ragent_agent::dry_run::run_dry_run(ins).await;

    let prov = section(&report, "provider");
    assert_eq!(prov.status, ReadinessStatus::Blocked);
    assert!(
        prov.items
            .iter()
            .any(|i| i.message == "No provider/model could be resolved")
    );
    assert_eq!(exit, 1);
}

#[tokio::test]
#[serial_test::serial]
async fn test_provider_all_env_present_ready() {
    let (_xdg, _g1, _g2, _g3) = isolate();
    let dir = TempDir::new().expect("dir");
    let path = write_json(
        &dir,
        &json!({"provider": {"openai": {"env": ["RAGENT_DR_TEST_PRESENT"]}}}),
    );
    let _present = EnvGuard::set("RAGENT_DR_TEST_PRESENT", "1");
    let mut ins = inputs(dir.path().to_path_buf(), Some(path));
    ins.model_override = Some("openai/gpt-4o".to_string());

    let (report, exit) = ragent_agent::dry_run::run_dry_run(ins).await;

    let prov = section(&report, "provider");
    let openai = item(prov, "openai");
    assert_eq!(openai.status, ReadinessStatus::Ready);
    assert!(
        openai
            .message
            .contains("All required environment variables present")
    );
    assert_eq!(exit, 0);
}

#[tokio::test]
#[serial_test::serial]
async fn test_provider_missing_env_warning() {
    let (_xdg, _g1, _g2, _g3) = isolate();
    let dir = TempDir::new().expect("dir");
    let path = write_json(
        &dir,
        &json!({"provider": {"openai": {"env": ["RAGENT_DR_TEST_ABSENT_ZZZ"]}}}),
    );
    let _absent = EnvGuard::remove("RAGENT_DR_TEST_ABSENT_ZZZ");
    let mut ins = inputs(dir.path().to_path_buf(), Some(path));
    ins.model_override = Some("openai/gpt-4o".to_string());

    let (report, exit) = ragent_agent::dry_run::run_dry_run(ins).await;

    let prov = section(&report, "provider");
    let openai = item(prov, "openai");
    assert_eq!(openai.status, ReadinessStatus::Warning);
    assert!(openai.message.contains("Missing environment variables"));
    // A warning is non-fatal: the deployment still exits 0.
    assert_eq!(exit, 0);
    assert_eq!(report.verdict, ReadinessVerdict::Warning);
}

// ---------------------------------------------------------------------------
// skills section
// ---------------------------------------------------------------------------

#[tokio::test]
#[serial_test::serial]
async fn test_skills_dir_is_file_warning() {
    let (_xdg, _g1, _g2, _g3) = isolate();
    let dir = TempDir::new().expect("dir");
    let not_a_dir = dir.path().join("skills-file");
    std::fs::write(&not_a_dir, "not a directory").expect("write file");
    let path = write_json(
        &dir,
        &json!({
            "provider": {"anthropic": {"env": []}},
            "skill_dirs": [not_a_dir.display().to_string()]
        }),
    );

    let (report, exit) =
        ragent_agent::dry_run::run_dry_run(inputs(dir.path().to_path_buf(), Some(path))).await;

    let skills = section(&report, "skills");
    assert_eq!(skills.status, ReadinessStatus::Warning);
    let entry = item(skills, &not_a_dir.display().to_string());
    assert_eq!(entry.status, ReadinessStatus::Warning);
    assert_eq!(
        entry.message,
        "Configured skill_dirs entry is not a directory"
    );
    assert_eq!(exit, 0);
}

// ---------------------------------------------------------------------------
// tools section
// ---------------------------------------------------------------------------

#[tokio::test]
#[serial_test::serial]
async fn test_tools_unknown_category_warning() {
    let (_xdg, _g1, _g2, _g3) = isolate();
    let dir = TempDir::new().expect("dir");
    let path = write_json(&dir, &json!({"provider": {"anthropic": {"env": []}}}));

    let registry = ToolRegistry::new();
    registry.register(Arc::new(EmptyCategoryTool));
    let mut ins = inputs(dir.path().to_path_buf(), Some(path));
    ins.tool_registry = Arc::new(registry);

    let (report, _exit) = ragent_agent::dry_run::run_dry_run(ins).await;

    let tools = section(&report, "tools");
    let uncategorized = item(tools, "uncategorized");
    assert_eq!(uncategorized.status, ReadinessStatus::Warning);
    assert!(uncategorized.message.contains("no family prefix"));
    let total = item(tools, "total");
    assert_eq!(total.message, "1 visible tools");
}

// ---------------------------------------------------------------------------
// mcp section
// ---------------------------------------------------------------------------

#[tokio::test]
#[serial_test::serial]
async fn test_mcp_disabled_server_ready() {
    let (_xdg, _g1, _g2, _g3) = isolate();
    let dir = TempDir::new().expect("dir");
    let path = write_json(
        &dir,
        &json!({
            "provider": {"anthropic": {"env": []}},
            "mcp": {"s": {"type": "http", "url": "http://127.0.0.1:1", "disabled": true}}
        }),
    );

    let (report, exit) =
        ragent_agent::dry_run::run_dry_run(inputs(dir.path().to_path_buf(), Some(path))).await;

    let mcp = section(&report, "mcp");
    assert_eq!(mcp.status, ReadinessStatus::Ready);
    let s = item(mcp, "s");
    assert_eq!(s.status, ReadinessStatus::Ready);
    assert_eq!(s.message, "Server disabled");
    assert_eq!(exit, 0);
}

#[tokio::test]
#[serial_test::serial]
async fn test_mcp_stdio_spawn_success_ready() {
    let (_xdg, _g1, _g2, _g3) = isolate();
    let dir = TempDir::new().expect("dir");
    let path = write_json(
        &dir,
        &json!({
            "provider": {"anthropic": {"env": []}},
            "mcp": {"ok": {"type": "stdio", "command": "/bin/true"}}
        }),
    );

    let (report, exit) =
        ragent_agent::dry_run::run_dry_run(inputs(dir.path().to_path_buf(), Some(path))).await;

    let mcp = section(&report, "mcp");
    assert_eq!(mcp.status, ReadinessStatus::Ready);
    let ok = item(mcp, "ok");
    assert_eq!(ok.status, ReadinessStatus::Ready);
    assert_eq!(ok.message, "stdio server spawned successfully");
    assert_eq!(exit, 0);
}

#[tokio::test]
#[serial_test::serial]
async fn test_mcp_stdio_spawn_failure_blocked() {
    let (_xdg, _g1, _g2, _g3) = isolate();
    let dir = TempDir::new().expect("dir");
    let path = write_json(
        &dir,
        &json!({
            "provider": {"anthropic": {"env": []}},
            "mcp": {"bad": {"type": "stdio", "command": "definitely-not-a-real-binary-xyz-123"}}
        }),
    );

    let (report, exit) =
        ragent_agent::dry_run::run_dry_run(inputs(dir.path().to_path_buf(), Some(path))).await;

    let mcp = section(&report, "mcp");
    assert_eq!(mcp.status, ReadinessStatus::Blocked);
    let bad = item(mcp, "bad");
    assert_eq!(bad.status, ReadinessStatus::Blocked);
    assert!(bad.message.contains("Failed to spawn stdio server"));
    assert_eq!(exit, 1);
}

#[tokio::test]
#[serial_test::serial]
async fn test_mcp_invalid_config_blocked() {
    let (_xdg, _g1, _g2, _g3) = isolate();
    let dir = TempDir::new().expect("dir");
    // A stdio server with no command fails `validate_mcp_config`.
    let path = write_json(
        &dir,
        &json!({
            "provider": {"anthropic": {"env": []}},
            "mcp": {"nocommand": {"type": "stdio"}}
        }),
    );

    let (report, exit) =
        ragent_agent::dry_run::run_dry_run(inputs(dir.path().to_path_buf(), Some(path))).await;

    let mcp = section(&report, "mcp");
    assert_eq!(mcp.status, ReadinessStatus::Blocked);
    let entry = item(mcp, "nocommand");
    assert_eq!(entry.status, ReadinessStatus::Blocked);
    assert!(entry.message.contains("Config invalid"));
    assert_eq!(exit, 1);
}

#[tokio::test]
#[serial_test::serial]
async fn test_mcp_http_reachable_ready() {
    let (_xdg, _g1, _g2, _g3) = isolate();
    let addr = spawn_http_responder("200 OK");
    let dir = TempDir::new().expect("dir");
    let path = write_json(
        &dir,
        &json!({
            "provider": {"anthropic": {"env": []}},
            "mcp": {"h": {
                "type": "http",
                "url": format!("http://{addr}/mcp"),
                "headers": {"X-Test": "1"}
            }}
        }),
    );

    let (report, exit) =
        ragent_agent::dry_run::run_dry_run(inputs(dir.path().to_path_buf(), Some(path))).await;

    let mcp = section(&report, "mcp");
    assert_eq!(mcp.status, ReadinessStatus::Ready);
    let h = item(mcp, "h");
    assert_eq!(h.status, ReadinessStatus::Ready);
    assert_eq!(h.message, "HTTP/SSE reachable (status 200)");
    assert_eq!(exit, 0);
}

#[tokio::test]
#[serial_test::serial]
async fn test_mcp_http_error_status_blocked() {
    let (_xdg, _g1, _g2, _g3) = isolate();
    let addr = spawn_http_responder("500 Internal Server Error");
    let dir = TempDir::new().expect("dir");
    let path = write_json(
        &dir,
        &json!({
            "provider": {"anthropic": {"env": []}},
            "mcp": {"h": {"type": "sse", "url": format!("http://{addr}/sse")}}
        }),
    );

    let (report, exit) =
        ragent_agent::dry_run::run_dry_run(inputs(dir.path().to_path_buf(), Some(path))).await;

    let mcp = section(&report, "mcp");
    assert_eq!(mcp.status, ReadinessStatus::Blocked);
    let h = item(mcp, "h");
    assert_eq!(h.status, ReadinessStatus::Blocked);
    assert!(
        h.message.contains("HTTP/SSE connectivity failed") && h.message.contains("HTTP 500"),
        "message was: {}",
        h.message
    );
    assert_eq!(exit, 1);
}

#[tokio::test]
#[serial_test::serial]
async fn test_mcp_http_missing_url_blocked_with_yolo() {
    let (_xdg, _g1, _g2, _g3) = isolate();
    // With YOLO enabled `validate_mcp_config` short-circuits, so an HTTP server
    // with no url reaches the dry-run url check instead of being rejected as an
    // invalid config. Without YOLO this branch is unreachable.
    let _yolo = YoloGuard::on();
    let dir = TempDir::new().expect("dir");
    let path = write_json(
        &dir,
        &json!({
            "provider": {"anthropic": {"env": []}},
            "mcp": {"h": {"type": "http"}}
        }),
    );

    let (report, exit) =
        ragent_agent::dry_run::run_dry_run(inputs(dir.path().to_path_buf(), Some(path))).await;

    let mcp = section(&report, "mcp");
    let h = item(mcp, "h");
    assert_eq!(h.status, ReadinessStatus::Blocked);
    assert_eq!(h.message, "HTTP/SSE transport requires a url");
    assert_eq!(exit, 1);
}

// ---------------------------------------------------------------------------
// report rendering, serde, and enum display / conversion
// ---------------------------------------------------------------------------

#[test]
fn test_report_to_json_and_human_string() {
    let report = ReadinessReport {
        verdict: ReadinessVerdict::Warning,
        sections: vec![
            ReadinessSection {
                name: "config".to_string(),
                status: ReadinessStatus::Ready,
                items: vec![ReadinessItem {
                    name: "config".to_string(),
                    status: ReadinessStatus::Ready,
                    message: "No config files found; using defaults".to_string(),
                }],
            },
            ReadinessSection {
                name: "mcp".to_string(),
                status: ReadinessStatus::Warning,
                items: Vec::new(),
            },
        ],
    };

    let json = report.to_json().expect("to_json");
    assert!(json.contains("\"verdict\": \"WARNING\""));
    assert!(json.contains("\"status\": \"ready\""));

    let human = report.to_human_string();
    assert!(human.contains("config: ready"));
    assert!(human.contains("mcp: warning"));
    assert!(human.contains("Verdict: WARNING"));

    let back: ReadinessReport = serde_json::from_str(&json).expect("round trip");
    assert_eq!(back.verdict, ReadinessVerdict::Warning);
    assert_eq!(back.sections.len(), 2);
    assert_eq!(back.sections[0].items.len(), 1);
}

#[test]
fn test_verdict_status_display_and_conversion() {
    assert_eq!(ReadinessVerdict::Ready.to_string(), "READY");
    assert_eq!(ReadinessVerdict::Warning.to_string(), "WARNING");
    assert_eq!(ReadinessVerdict::Blocked.to_string(), "BLOCKED");

    assert_eq!(ReadinessStatus::Ready.to_string(), "ready");
    assert_eq!(ReadinessStatus::Warning.to_string(), "warning");
    assert_eq!(ReadinessStatus::Blocked.to_string(), "blocked");

    assert_eq!(
        ReadinessStatus::from(ReadinessVerdict::Ready),
        ReadinessStatus::Ready
    );
    assert_eq!(
        ReadinessStatus::from(ReadinessVerdict::Warning),
        ReadinessStatus::Warning
    );
    assert_eq!(
        ReadinessStatus::from(ReadinessVerdict::Blocked),
        ReadinessStatus::Blocked
    );
}
