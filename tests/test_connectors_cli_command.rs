//! Integration tests for the `ragent connectors` CLI parity surface
//! (spec `connectors` T-014; FR-004, FR-006).
//!
//! Each test spawns the built `ragent` binary inside a fresh temp working
//! directory with an isolated `XDG_CONFIG_HOME`, so the project store
//! (`.ragent/connectors/`) and the user-global store are both contained. The
//! tests assert the non-TUI output spelling (`ragent connectors ...` attribution
//! rather than `From: /connectors ...`) and that every subcommand routes to the
//! shared `ragent-connectors` logic.

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
        std::fs::create_dir_all(temp.path().join(".ragent")).expect(".ragent dir");
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

    /// Write a minimal connector source directory (one expressible stdio server).
    fn write_connector_source(&self, id: &str) -> PathBuf {
        self.write_categorised_connector_source(id, "developer")
    }

    /// Write a minimal connector source directory with a declared category.
    fn write_categorised_connector_source(&self, id: &str, category: &str) -> PathBuf {
        let dir = self.temp.path().join(format!("src-{id}"));
        std::fs::create_dir_all(&dir).expect("source dir");
        std::fs::write(
            dir.join("connector.json"),
            format!(
                r#"{{ "id": "{id}", "name": "{id}", "category": "{category}",
                     "servers": [ {{ "id": "main", "transport": "stdio", "command": "/bin/true" }} ] }}"#
            ),
        )
        .expect("manifest");
        dir
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

#[test]
fn bare_and_help_print_the_usage_block() {
    let fx = Fixture::new();

    for args in [vec!["connectors"], vec!["connectors", "help"]] {
        let out = fx.ragent(&args);
        assert!(out.status.success(), "usage must succeed: {args:?}");
        let text = stdout(&out);
        assert!(
            text.contains("## ragent connectors command reference"),
            "usage must carry the CLI heading: {text}"
        );
        // The non-TUI surface spelling, not the TUI `From:` attribution.
        assert!(
            text.contains("`ragent connectors list [--verbose] [--category <name>]`"),
            "usage must document the CLI row for list: {text}"
        );
        assert!(
            !text.contains("From: /connectors"),
            "usage must not print the TUI attribution: {text}"
        );
        for needle in ["https://", "--force", "--category", "--check"] {
            assert!(
                text.contains(needle),
                "usage must mention `{needle}`: {text}"
            );
        }
    }
}

#[test]
fn unknown_subcommand_renders_usage() {
    let fx = Fixture::new();
    let text = stdout(&fx.ragent(&["connectors", "frobnicate"]));
    assert!(
        text.contains("## ragent connectors command reference"),
        "an unknown subcommand renders usage: {text}"
    );
}

#[test]
fn list_on_an_empty_store_reports_none_installed() {
    let fx = Fixture::new();
    let out = fx.ragent(&["connectors", "list"]);
    assert!(out.status.success());
    let text = stdout(&out);
    assert!(
        text.starts_with("ragent connectors list"),
        "list uses the CLI attribution: {text}"
    );
    assert!(
        text.contains("No connectors installed."),
        "an empty store reports none: {text}"
    );
    assert!(text.contains("category ALL (0 of 0)"), "{text}");
}

#[test]
fn list_creates_no_connector_store() {
    // FR-017 parity: a read-only listing changes no state and must not create
    // the connector store directory.
    let fx = Fixture::new();
    let _ = fx.ragent(&["connectors", "list"]);
    assert!(
        !fx.path().join(".ragent/connectors").exists(),
        "listing must not create the connector store"
    );
}

#[test]
fn store_round_trip_uses_the_cli_surface_spelling() {
    let fx = Fixture::new();
    let src = fx.write_connector_source("echo");

    // add (FR-011): installed disabled, recorded in the project store.
    let out = fx.ragent(&["connectors", "add", src.to_str().expect("utf8 src")]);
    assert!(out.status.success(), "add must succeed");
    let added = stdout(&out);
    assert!(added.starts_with("ragent connectors add"), "{added}");
    assert!(
        added.contains("Installed connector `echo`"),
        "add reports the install: {added}"
    );
    assert!(
        fx.path()
            .join(".ragent/connectors/echo/connector.json")
            .exists(),
        "the connector must be copied into the project store"
    );

    // list shows it disabled (FR-011, FR-018).
    let listed = stdout(&fx.ragent(&["connectors", "list"]));
    assert!(listed.starts_with("ragent connectors list"), "{listed}");
    assert!(listed.contains("echo"), "{listed}");
    assert!(listed.contains("state disabled"), "{listed}");

    // `--category` filters (FR-041).
    let filtered = stdout(&fx.ragent(&["connectors", "list", "--category", "developer"]));
    assert!(
        filtered.contains("category developer (1 of 1)"),
        "{filtered}"
    );

    // A category that is known but matches nothing is an empty-result message,
    // not an error (FR-041). The fixture store holds a second connector in the
    // `science` category so the value is accepted.
    let science_src = fx.write_categorised_connector_source("atom", "science");
    let added = fx.ragent(&["connectors", "add", science_src.to_str().expect("utf8 src")]);
    assert!(added.status.success(), "add must succeed");
    let filtered_out = stdout(&fx.ragent(&["connectors", "list", "--category", "science"]));
    assert!(
        filtered_out.contains("category science (1 of 2)"),
        "{filtered_out}"
    );
    assert!(filtered_out.contains("atom"), "{filtered_out}");

    let empty = stdout(&fx.ragent(&["connectors", "list", "--category", "developer"]));
    assert!(empty.contains("category developer (1 of 2)"), "{empty}");
    assert!(!empty.contains("atom"), "{empty}");
    assert!(!empty.contains("[err]"), "{empty}");

    // A category the store does not know is refused with an `[err]` row and
    // changes no state (FR-041).
    let unknown = stdout(&fx.ragent(&["connectors", "list", "--category", "nosuch"]));
    assert!(
        unknown.contains("[err] Unknown category `nosuch`."),
        "{unknown}"
    );

    // disable a disabled connector is a no-op transition that reports ok.
    let disabled = stdout(&fx.ragent(&["connectors", "disable", "echo"]));
    assert!(
        disabled.starts_with("ragent connectors disable"),
        "{disabled}"
    );
    assert!(disabled.contains("[ok]"), "{disabled}");

    // remove uninstalls it (FR-030).
    let removed = stdout(&fx.ragent(&["connectors", "remove", "echo"]));
    assert!(removed.starts_with("ragent connectors remove"), "{removed}");
    assert!(
        removed.contains("[ok] Removed connector `echo`"),
        "{removed}"
    );
    assert!(
        !fx.path().join(".ragent/connectors/echo").exists(),
        "remove deletes the connector directory"
    );
}

#[test]
fn missing_id_is_reported_as_a_usage_error() {
    let fx = Fixture::new();
    let out = fx.ragent(&["connectors", "enable"]);
    assert!(
        out.status.success(),
        "a usage error is not a process failure"
    );
    let text = stdout(&out);
    assert!(text.contains("[err] Missing argument."), "{text}");
    assert!(text.starts_with("ragent connectors enable"), "{text}");
}

#[test]
fn auth_on_an_unknown_connector_reports_the_unknown_id() {
    let fx = Fixture::new();
    let out = fx.ragent(&["connectors", "auth", "ghost"]);
    assert!(out.status.success());
    let text = stdout(&out);
    assert!(text.starts_with("ragent connectors auth ghost"), "{text}");
    assert!(
        text.contains("[err] connector auth: unknown connector id ghost"),
        "{text}"
    );
}

#[test]
fn stores_reports_the_effective_endpoint_and_its_source() {
    // FR-036 parity: with no `connectors.stores` override, the endpoint is
    // tagged `default` and names the compiled default URL.
    let fx = Fixture::new();
    let text = stdout(&fx.ragent(&["connectors", "stores"]));
    assert!(text.starts_with("ragent connectors stores"), "{text}");
    assert!(
        text.contains("- claude: [default] https://"),
        "the store is tagged default with an https endpoint: {text}"
    );
    assert!(
        !text.contains("From: /connectors"),
        "the CLI must not print the TUI attribution: {text}"
    );
}

#[test]
fn stores_usage_documents_the_optional_check_flag() {
    let fx = Fixture::new();
    let help = stdout(&fx.ragent(&["connectors", "help"]));
    assert!(
        help.contains("ragent connectors stores") && help.contains("--check"),
        "the usage block must document the --check flag: {help}"
    );

    let plain = stdout(&fx.ragent(&["connectors", "stores"]));
    assert!(
        !plain.contains("available") && !plain.contains("unavailable"),
        "the plain report carries no probe suffix: {plain}"
    );
}

#[test]
fn master_switch_disables_the_subsystem_but_help_still_renders() {
    // FR-021 parity: with `connectors.enabled: false` the CLI reports the
    // disabled subsystem for a management subcommand, while `help` still prints
    // usage.
    let fx = Fixture::new();
    std::fs::write(
        fx.path().join(".ragent/ragent.json"),
        r#"{ "connectors": { "enabled": false } }"#,
    )
    .expect("config writable");

    let listed = stdout(&fx.ragent(&["connectors", "list"]));
    assert!(listed.contains("[err]"), "{listed}");
    assert!(listed.contains("connectors.enabled = false"), "{listed}");

    let help = stdout(&fx.ragent(&["connectors", "help"]));
    assert!(
        help.contains("## ragent connectors command reference"),
        "help still renders while disabled: {help}"
    );
}

#[test]
fn claude_points_at_the_tui_browser_and_names_the_non_interactive_equivalents() {
    // The interactive catalogue browser owns the screen, so the one-shot CLI
    // must not try to open it; `claude` describes the browser and names the
    // shell equivalents instead (FR-004 parity).
    let fx = Fixture::new();
    let out = fx.ragent(&["connectors", "claude"]);
    assert!(out.status.success(), "exit code: {:?}", out.status);
    let text = stdout(&out);
    assert!(
        text.starts_with("ragent connectors claude"),
        "CLI spelling, not the TUI attribution: {text}"
    );
    assert!(
        text.contains("interactive TUI panel"),
        "identifies the browser as a TUI-only panel: {text}"
    );
    assert!(
        text.contains("ragent connectors search"),
        "names the non-interactive search: {text}"
    );
    assert!(
        text.contains("ragent connectors add"),
        "names the non-interactive add: {text}"
    );
    // No catalogue listing is printed: the CLI never opens the panel.
    assert!(
        !text.contains("category ALL"),
        "no catalogue listing is printed: {text}"
    );
}

#[test]
fn help_documents_the_claude_browser_subcommand() {
    let fx = Fixture::new();
    let help = stdout(&fx.ragent(&["connectors", "help"]));
    assert!(
        help.contains("ragent connectors claude [query] [--refresh]"),
        "the usage block documents the browser: {help}"
    );
}
