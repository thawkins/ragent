//! Integration tests for the `ragent plugins` CLI parity surface
//! (spec `plugins` T-017; FR-021).
//!
//! Each test spawns the built `ragent` binary inside a fresh temp working
//! directory with an isolated `XDG_CONFIG_HOME`, so the project store
//! (`.ragent/plugins/`) and the user-global store are both contained. The tests
//! assert the non-TUI output spelling (`ragent plugins …` attribution rather
//! than `From: /plugins …`), that every subcommand routes to the shared
//! `ragent-plugins` logic, and that the store round-trip works end to end.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::{Mutex, MutexGuard};

/// Serialise tests that change the process working directory.
static CWD_LOCK: Mutex<()> = Mutex::new(());

/// A temp working directory plus an isolated config/home root. Holds the cwd
/// lock for its lifetime and restores the previous cwd on drop, so a panicking
/// test cannot leave the process cwd inside a deleted tempdir.
struct Fixture {
    temp: tempfile::TempDir,
    prev: PathBuf,
    _lock: MutexGuard<'static, ()>,
}

impl Fixture {
    fn new() -> Self {
        let lock = CWD_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let temp = tempfile::tempdir().expect("tempdir");
        let prev = std::env::current_dir().expect("cwd");
        std::env::set_current_dir(temp.path()).expect("set cwd");
        Self {
            temp,
            prev,
            _lock: lock,
        }
    }

    fn path(&self) -> &Path {
        self.temp.path()
    }

    /// Run `ragent` with `args` against this fixture's isolated environment.
    fn ragent(&self, args: &[&str]) -> Output {
        let config = self.temp.path().join("config");
        let home = self.temp.path().join("home");
        std::fs::create_dir_all(&config).expect("config dir");
        std::fs::create_dir_all(&home).expect("home dir");
        Command::new(env!("CARGO_BIN_EXE_ragent"))
            .args(args)
            .env("XDG_CONFIG_HOME", &config)
            .env("HOME", &home)
            .env_remove("GITHUB_TOKEN")
            .env_remove("GITLAB_TOKEN")
            .output()
            .expect("spawn ragent")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.prev);
    }
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// Write a minimal Codex-dialect plugin (one tool, no commands) into `dir`.
fn write_codex_plugin(dir: &Path, id: &str) {
    std::fs::create_dir_all(dir).expect("plugin dir");
    std::fs::write(
        dir.join("codex-plugin.json"),
        format!(r#"{{ "id": "{id}", "name": "{id}", "version": "1.0.0", "entry": "index.js" }}"#),
    )
    .expect("manifest");
    std::fs::write(
        dir.join("index.js"),
        "ragent.register_tool({ name: \"ping\", description: \"Ping\", \
         parameters: { type: \"object\" }, handler: function(){ return \"pong\"; } });",
    )
    .expect("entry");
}

#[test]
fn bare_and_help_print_the_usage_block() {
    let fx = Fixture::new();

    for args in [vec!["plugins"], vec!["plugins", "help"]] {
        let out = fx.ragent(&args);
        assert!(out.status.success(), "usage must succeed: {args:?}");
        let text = stdout(&out);
        assert!(
            text.contains("## ragent plugins command reference"),
            "usage must carry the CLI heading: {text}"
        );
        // The non-TUI surface spelling, not the TUI `From:` attribution.
        assert!(
            text.contains("`ragent plugins list [--verbose]`"),
            "usage must document the CLI row for list: {text}"
        );
        assert!(
            !text.contains("From: /plugins"),
            "usage must not print the TUI attribution: {text}"
        );
    }
}

#[test]
fn list_on_an_empty_store_reports_none_discovered() {
    let fx = Fixture::new();
    let out = fx.ragent(&["plugins", "list"]);
    assert!(out.status.success());
    let text = stdout(&out);
    assert!(
        text.starts_with("ragent plugins list"),
        "list uses the CLI attribution: {text}"
    );
    assert!(
        text.contains("No plugins discovered."),
        "an empty store reports none: {text}"
    );
}

#[test]
fn store_round_trip_uses_the_cli_surface_spelling() {
    let fx = Fixture::new();

    // Stage a source plugin outside the store.
    let src = fx.path().join("src-plugin");
    write_codex_plugin(&src, "codex-weather");

    // add
    let out = fx.ragent(&["plugins", "add", src.to_str().expect("utf8 src")]);
    assert!(out.status.success(), "add must succeed");
    let added = stdout(&out);
    assert!(added.starts_with("ragent plugins add"), "{added}");
    assert!(
        added.contains("[ok] Installed plugin `codex-weather`"),
        "{added}"
    );
    assert!(
        fx.path()
            .join(".ragent/plugins/codex-weather/codex-plugin.json")
            .exists(),
        "the plugin must be copied into the project store"
    );

    // list shows it disabled (FR-007, FR-016)
    let listed = stdout(&fx.ragent(&["plugins", "list"]));
    assert!(listed.contains("codex-weather"), "{listed}");
    assert!(listed.contains("disabled"), "{listed}");

    // enable loads it and reports the contributions (FR-011)
    let enabled = stdout(&fx.ragent(&["plugins", "enable", "codex-weather"]));
    assert!(enabled.starts_with("ragent plugins enable"), "{enabled}");
    assert!(enabled.contains("[ok]"), "{enabled}");
    assert!(
        enabled.contains("registered 1 tool(s)"),
        "enable registers the plugin tool: {enabled}"
    );

    // verbose list names the registered tool (FR-009, FR-022)
    let verbose = stdout(&fx.ragent(&["plugins", "list", "--verbose"]));
    assert!(
        verbose.contains("plugin_codex-weather_ping"),
        "verbose list names the registry tool: {verbose}"
    );
    assert!(verbose.contains("Telemetry:"), "{verbose}");

    // test drives the isolated harness (FR-013)
    let tested = stdout(&fx.ragent(&["plugins", "test", "codex-weather"]));
    assert!(tested.starts_with("ragent plugins test"), "{tested}");
    assert!(tested.contains("[ ok ]"), "{tested}");
    assert!(
        tested.contains("live session untouched"),
        "the harness leaves the session untouched: {tested}"
    );

    // disable unloads it (FR-012)
    let disabled = stdout(&fx.ragent(&["plugins", "disable", "codex-weather"]));
    assert!(disabled.starts_with("ragent plugins disable"), "{disabled}");
    assert!(disabled.contains("deregistered"), "{disabled}");

    // remove uninstalls it (FR-010)
    let removed = stdout(&fx.ragent(&["plugins", "remove", "codex-weather"]));
    assert!(removed.starts_with("ragent plugins remove"), "{removed}");
    assert!(
        removed.contains("[ok] Removed plugin `codex-weather`"),
        "{removed}"
    );
    assert!(
        !fx.path().join(".ragent/plugins/codex-weather").exists(),
        "remove deletes the plugin directory"
    );
}

#[test]
fn missing_plugin_id_is_reported_as_usage_error() {
    let fx = Fixture::new();
    let out = fx.ragent(&["plugins", "enable"]);
    assert!(
        out.status.success(),
        "a usage error is not a process failure"
    );
    let text = stdout(&out);
    assert!(
        text.contains("[err] Missing <pluginid>."),
        "enable without an id reports the usage error: {text}"
    );
}

#[test]
fn unknown_subcommand_renders_usage() {
    let fx = Fixture::new();
    let text = stdout(&fx.ragent(&["plugins", "frobnicate"]));
    assert!(
        text.contains("## ragent plugins command reference"),
        "an unknown subcommand renders usage: {text}"
    );
}

#[test]
fn master_switch_disables_the_subsystem() {
    // Acceptance criterion 8 (FR-021 parity): with `plugins.enabled: false` the
    // CLI reports the disabled subsystem and changes no state; `help` still
    // prints usage.
    let fx = Fixture::new();
    std::fs::create_dir_all(fx.path().join(".ragent")).expect(".ragent dir");
    std::fs::write(
        fx.path().join(".ragent/ragent.json"),
        r#"{ "plugins": { "enabled": false } }"#,
    )
    .expect("config writable");

    let listed = stdout(&fx.ragent(&["plugins", "list"]));
    assert!(listed.contains("[err]"), "{listed}");
    assert!(listed.contains("plugins.enabled = false"), "{listed}");

    let enabled = stdout(&fx.ragent(&["plugins", "enable", "ghost"]));
    assert!(enabled.contains("[err]"), "{enabled}");

    let help = stdout(&fx.ragent(&["plugins", "help"]));
    assert!(
        help.contains("## ragent plugins command reference"),
        "help still renders while disabled: {help}"
    );
    assert!(
        !fx.path().join(".ragent/plugins").exists(),
        "the disabled subsystem creates no store directory"
    );
}
