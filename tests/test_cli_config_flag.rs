//! Integration tests for `ragent --config <file>` reaching CLI handlers (#57).
//!
//! `main` parses the `--config` file and never loads the default `ragent.json`
//! chain. These tests spawn the built binary with a `--config` file whose
//! values differ from the defaults, inside an isolated `HOME` /
//! `XDG_CONFIG_HOME`, and assert the handler output carries the file's values
//! rather than a reloaded default config.
//!
//! Every write goes under `target/temp/`, and the child process gets its own
//! working directory, so no process-global state is mutated here.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// A per-test scratch root under `target/temp/`, removed on drop.
fn scratch() -> tempfile::TempDir {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/temp/cli-config-flag");
    std::fs::create_dir_all(&base).expect("scratch base");
    tempfile::Builder::new()
        .prefix("t-")
        .tempdir_in(&base)
        .expect("scratch dir")
}

/// Write `json` as the `--config` file and run `ragent --config <file> <args>`
/// from `root` with an isolated home and config directory.
fn ragent_with_config(root: &Path, json: &str, args: &[&str]) -> Output {
    let config_file = root.join("custom.json");
    std::fs::write(&config_file, json).expect("config file");
    let xdg = root.join("xdg");
    let home = root.join("home");
    std::fs::create_dir_all(&xdg).expect("xdg dir");
    std::fs::create_dir_all(&home).expect("home dir");
    Command::new(env!("CARGO_BIN_EXE_ragent"))
        .arg("--config")
        .arg(&config_file)
        .args(args)
        .current_dir(root)
        .env("XDG_CONFIG_HOME", &xdg)
        .env("HOME", &home)
        .output()
        .expect("spawn ragent")
}

#[test]
fn test_cli_config_flag_automation_run_reads_config_file() {
    // A reload of the default config yields an enabled service with no
    // definitions, which refuses with "no enabled automation". Disabling the
    // service in the `--config` file must instead hit the service-disabled
    // refusal, which only the startup config can produce.
    let dir = scratch();
    let out = ragent_with_config(
        dir.path(),
        r#"{ "automation": { "enabled": false, "automations": [
            { "id": "in-file", "prompt": "p", "trigger": { "kind": "webhook" } }
        ] } }"#,
        &["automation", "run", "in-file"],
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success(), "a disabled service must not run");
    assert!(
        stderr.contains("the automation service is disabled"),
        "expected the --config service state to be consulted; stderr: {stderr}"
    );
}

#[test]
fn test_cli_config_flag_automation_list_reads_config_file() {
    let dir = scratch();
    let out = ragent_with_config(
        dir.path(),
        r#"{ "automation": { "enabled": true, "automations": [
            { "id": "from-config-file", "prompt": "p", "trigger": { "kind": "webhook" } }
        ] } }"#,
        &["automation", "list"],
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "exit {:?}, stderr: {}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        stdout.contains("from-config-file"),
        "--config automation not applied; stdout: {stdout}"
    );
}
