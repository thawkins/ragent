//! Tests for the `/connectors` parse-and-run glue and report wording (spec
//! `connectors` T-010; FR-004, FR-006, FR-023).
//!
//! The registration itself (the `SLASH_COMMANDS` entry, the dispatch arm, and
//! the autocomplete menu) is asserted in the TUI crate
//! (`crates/ragent-tui/tests/test_connectors_command.rs`); here the pure parser,
//! the usage/attribution renderers, and the shared dispatcher are exercised
//! crate-side with no live session.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use ragent_config::ConnectorsConfig;
use ragent_connectors::{
    ALL_CATEGORY, CONNECTOR_FLAGS, CONNECTOR_SUBCOMMANDS, CategoryFilter, ConnectorArgError,
    ConnectorCommand, ConnectorDescriptor, MANIFEST_FILE, attribution, autocomplete_tokens,
    disabled_subsystem_report, is_known_subcommand, parse_connector_command, render_help,
    run_connector_subcommand, store_and_config, subcommand_of,
};

static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Serialise cwd-mutating tests in this binary.
fn cwd_test_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

/// RAII cwd guard so a temp working directory is restored on drop.
struct CwdGuard(PathBuf);

impl Drop for CwdGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.0);
    }
}

/// RAII sandboxed temp tree rooted at `target/temp/` (AGENTS.md: no `/tmp`).
struct TempTree(PathBuf);

impl TempTree {
    fn new(name: &str) -> Self {
        let unique = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/temp/connectors-test/commands-{name}-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir_all(path.join(".ragent")).expect("temp tree creatable");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Enter a fresh temp working directory and return the guard plus its tree.
fn enter_temp_dir(name: &str) -> (CwdGuard, TempTree) {
    let original = std::env::current_dir().expect("cwd");
    let tree = TempTree::new(name);
    std::env::set_current_dir(tree.path()).expect("set cwd");
    (CwdGuard(original), tree)
}

/// Write a `connectors` block into the temp tree's `ragent.json`.
fn write_connectors_config(tree: &TempTree, body: &str) {
    std::fs::write(
        tree.path().join(".ragent").join("ragent.json"),
        format!(r#"{{ "connectors": {body} }}"#),
    )
    .expect("config writable");
}

/// A valid descriptor with a `token` auth shape and a declared credential name.
fn token_descriptor() -> ConnectorDescriptor {
    serde_json::from_str(
        r#"{
            "id": "gdrive",
            "name": "Google Drive",
            "category": "productivity",
            "auth": "token",
            "credential": "GDRIVE_TOKEN",
            "servers": [ { "id": "main", "transport": "stdio", "command": "/bin/true" } ]
        }"#,
    )
    .expect("descriptor parses")
}

// -- parse_connector_command (FR-004, FR-041) --------------------------------

#[test]
fn parses_list_default_and_verbose() {
    assert_eq!(
        parse_connector_command("list", ""),
        Some(Ok(ConnectorCommand::List {
            verbose: false,
            category: CategoryFilter::all(),
        }))
    );
    assert_eq!(
        parse_connector_command("list", "--verbose"),
        Some(Ok(ConnectorCommand::List {
            verbose: true,
            category: CategoryFilter::all(),
        }))
    );
}

#[test]
fn parses_list_and_claude_category_flag() {
    let Some(Ok(ConnectorCommand::List { category, .. })) =
        parse_connector_command("list", "--category data")
    else {
        panic!("list --category data must parse");
    };
    assert_eq!(category.label(), "data");

    let Some(Ok(ConnectorCommand::Claude {
        query,
        category,
        refresh,
    })) = parse_connector_command("claude", "drive --category productivity")
    else {
        panic!("claude with a category must parse");
    };
    assert_eq!(query, "drive");
    assert_eq!(category.label(), "productivity");
    assert!(!refresh);

    // `--category ALL` clears the filter (FR-040).
    let Some(Ok(ConnectorCommand::List { category, .. })) =
        parse_connector_command("list", "--category ALL")
    else {
        panic!("list --category ALL must parse");
    };
    assert!(category.is_all());
    assert_eq!(category.label(), ALL_CATEGORY);

    // A missing `--category` value degrades to the unfiltered `ALL` selection
    // rather than a filter that can never match (FR-040).
    let Some(Ok(ConnectorCommand::List { category, .. })) =
        parse_connector_command("list", "--category")
    else {
        panic!("a bare --category must still parse");
    };
    assert!(category.is_all());
}

#[test]
fn parses_claude_query_category_and_refresh() {
    assert_eq!(
        parse_connector_command("claude", "google drive"),
        Some(Ok(ConnectorCommand::Claude {
            query: "google drive".to_string(),
            category: CategoryFilter::all(),
            refresh: false,
        }))
    );
    // A launch with no arguments is valid: the browser opens unfiltered.
    assert_eq!(
        parse_connector_command("claude", ""),
        Some(Ok(ConnectorCommand::Claude {
            query: String::new(),
            category: CategoryFilter::all(),
            refresh: false,
        }))
    );
    // The flags are accepted in any order and never leak into the query.
    assert_eq!(
        parse_connector_command("claude", "--refresh drive --category data"),
        Some(Ok(ConnectorCommand::Claude {
            query: "drive".to_string(),
            category: CategoryFilter::parse("data"),
            refresh: true,
        }))
    );
}

#[test]
fn parses_add_with_and_without_force() {
    assert_eq!(
        parse_connector_command("add", "google-drive"),
        Some(Ok(ConnectorCommand::Add {
            source: "google-drive".to_string(),
            force: false,
        }))
    );
    assert_eq!(
        parse_connector_command("add", "https://example.org/x.zip --force"),
        Some(Ok(ConnectorCommand::Add {
            source: "https://example.org/x.zip".to_string(),
            force: true,
        }))
    );
    assert_eq!(
        parse_connector_command("add", ""),
        Some(Err(ConnectorArgError::MissingAddSource))
    );
}

#[test]
fn parses_single_id_subcommands_and_rejects_missing_id() {
    assert_eq!(
        parse_connector_command("remove", "echo"),
        Some(Ok(ConnectorCommand::Remove {
            id: "echo".to_string(),
        }))
    );
    assert_eq!(
        parse_connector_command("enable", "echo"),
        Some(Ok(ConnectorCommand::Enable {
            id: "echo".to_string(),
        }))
    );
    assert_eq!(
        parse_connector_command("disable", "echo"),
        Some(Ok(ConnectorCommand::Disable {
            id: "echo".to_string(),
        }))
    );
    assert_eq!(
        parse_connector_command("connect", "echo"),
        Some(Ok(ConnectorCommand::Connect {
            id: "echo".to_string(),
        }))
    );
    assert_eq!(
        parse_connector_command("disconnect", "echo"),
        Some(Ok(ConnectorCommand::Disconnect {
            id: "echo".to_string(),
        }))
    );
    assert_eq!(
        parse_connector_command("auth", "echo"),
        Some(Ok(ConnectorCommand::Auth {
            id: "echo".to_string(),
        }))
    );
    assert_eq!(
        parse_connector_command("test", "echo"),
        Some(Ok(ConnectorCommand::Test {
            id: "echo".to_string(),
        }))
    );

    for sub in [
        "remove",
        "enable",
        "disable",
        "connect",
        "disconnect",
        "auth",
        "test",
    ] {
        assert_eq!(
            parse_connector_command(sub, ""),
            Some(Err(ConnectorArgError::MissingId)),
            "`{sub}` with no id must be a missing-argument error"
        );
    }
}

#[test]
fn parses_stores_with_and_without_check() {
    assert_eq!(
        parse_connector_command("stores", ""),
        Some(Ok(ConnectorCommand::Stores { check: false }))
    );
    assert_eq!(
        parse_connector_command("stores", "--check"),
        Some(Ok(ConnectorCommand::Stores { check: true }))
    );
}

#[test]
fn unknown_subcommand_is_none() {
    assert_eq!(parse_connector_command("bogus", "x"), None);
    assert_eq!(parse_connector_command("help", ""), None);
}

#[test]
fn known_subcommand_covers_the_whole_family() {
    for sub in [
        "list",
        "claude",
        "add",
        "remove",
        "enable",
        "disable",
        "connect",
        "disconnect",
        "auth",
        "test",
        "stores",
        "help",
    ] {
        assert!(
            is_known_subcommand(sub),
            "`{sub}` must be a known subcommand"
        );
    }
    assert!(!is_known_subcommand("bogus"));
    assert_eq!(CONNECTOR_SUBCOMMANDS.len(), 12);
}

// -- help / attribution (FR-006, FR-017) -------------------------------------

#[test]
fn attribution_prefixes_the_subcommand() {
    assert_eq!(attribution(""), "From: /connectors");
    assert_eq!(attribution("stores"), "From: /connectors stores");
    assert_eq!(subcommand_of("stores --check"), "stores");
    assert_eq!(subcommand_of("   "), "");
}

#[test]
fn render_help_documents_every_subcommand_and_is_ascii() {
    let help = render_help("help");
    assert!(help.starts_with("From: /connectors help"));
    assert!(help.is_ascii(), "usage text must be ASCII only");
    for sub in [
        "list",
        "claude",
        "add",
        "remove",
        "enable",
        "disable",
        "connect",
        "disconnect",
        "auth",
        "test",
        "stores",
        "help",
    ] {
        assert!(help.contains(sub), "usage must document `{sub}`: {help}");
    }
    for needle in [
        "https://",
        "--force",
        "--category",
        "--check",
        "--verbose",
        "--refresh",
    ] {
        assert!(help.contains(needle), "usage must mention `{needle}`");
    }
}

#[test]
fn autocomplete_tokens_are_seeded_from_subcommands_and_flags() {
    let tokens = autocomplete_tokens();
    assert_eq!(tokens, ragent_connectors::autocomplete_tokens());
    for sub in CONNECTOR_SUBCOMMANDS {
        assert!(
            tokens.iter().any(|t| t == sub),
            "the menu must offer `{sub}`: {tokens:?}"
        );
    }
    for flag in CONNECTOR_FLAGS {
        assert!(
            tokens.iter().any(|t| t == flag),
            "the menu must offer `{flag}`: {tokens:?}"
        );
    }
    assert_eq!(
        tokens.len(),
        CONNECTOR_SUBCOMMANDS.len() + CONNECTOR_FLAGS.len()
    );
    let mut sorted = tokens.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), tokens.len(), "tokens must be unique");
}

#[test]
fn every_documented_subcommand_is_seeded_into_the_menu() {
    // FR-004: the autocomplete menu documents exactly the subcommands the usage
    // block documents. A subcommand named in the usage body must be a token.
    let help = render_help("help");
    for sub in CONNECTOR_SUBCOMMANDS {
        assert!(
            help.contains(sub),
            "usage must document the seeded subcommand `{sub}`"
        );
    }
}

#[test]
fn usage_block_documents_argument_placeholders_and_source_forms() {
    let help = render_help("help");
    // FR-017: every subcommand's arguments and the accepted `<source>` forms.
    for needle in [
        "/connectors list [--verbose] [--category <name>]",
        "/connectors claude [query] [--category <name>] [--refresh]",
        "/connectors add <id|source> [--force]",
        "/connectors remove <id>",
        "/connectors enable <id>",
        "/connectors disable <id>",
        "/connectors connect <id>",
        "/connectors disconnect <id>",
        "/connectors auth <id>",
        "/connectors test <id>",
        "/connectors stores [--check]",
        "/connectors help",
        "### Sources accepted by `/connectors add`",
        "a catalogue connector id",
        "a local `.zip` or `.tar.gz` package",
        "an `https://` URL",
    ] {
        assert!(help.contains(needle), "usage must document `{needle}`");
    }
}

// -- reports (FR-006, FR-021, FR-023) ----------------------------------------

#[test]
fn disabled_subsystem_report_names_the_master_switch() {
    let report = disabled_subsystem_report("list");
    assert!(report.starts_with("From: /connectors list"));
    assert!(report.contains("[err]"));
    assert!(report.contains("disabled"));
    assert!(report.is_ascii());
}

#[test]
fn add_report_surfaces_the_credential_requirement_without_a_secret() {
    let outcome = ragent_connectors::StagedConnector {
        descriptor: token_descriptor(),
        installed_dir: PathBuf::from("/store/gdrive"),
    };
    let report = ragent_connectors::add_report(&outcome);
    assert!(report.starts_with("From: /connectors add"));
    assert!(report.contains("[ok]"));
    assert!(report.contains("gdrive"));
    assert!(
        report.contains("enabled"),
        "the install must report enabled"
    );
    assert!(
        report.contains("Credential requirement:") && report.contains("GDRIVE_TOKEN"),
        "the credential requirement must be surfaced (FR-023): {report}"
    );
    assert!(report.is_ascii());
}

// -- store_and_config --------------------------------------------------------

#[test]
fn store_and_config_resolves_project_store_and_config() {
    let _lock = cwd_test_lock().lock().unwrap_or_else(|e| e.into_inner());
    let (_guard, tree) = enter_temp_dir("store-config");
    write_connectors_config(&tree, r#"{ "enabled": true }"#);

    let (dirs, config) = store_and_config(tree.path());
    assert!(config.is_enabled());
    assert_eq!(
        dirs.project,
        Some(tree.path().join(".ragent").join("connectors"))
    );
}

// -- run_connector_subcommand (FR-004, FR-021) -------------------------------

#[test]
fn help_renders_usage_without_config_or_store() {
    // `help` performs no config/store access and creates no files (FR-017).
    let _lock = cwd_test_lock().lock().unwrap_or_else(|e| e.into_inner());
    let (_guard, tree) = enter_temp_dir("help-no-files");
    let report = run_connector_subcommand(tree.path(), "help", "").expect("help renders");
    assert!(report.contains("command reference"));
    assert!(
        !tree.path().join(".ragent").join("connectors").exists(),
        "help must create no store"
    );
}

#[test]
fn unknown_subcommand_returns_none() {
    let _lock = cwd_test_lock().lock().unwrap_or_else(|e| e.into_inner());
    let (_guard, tree) = enter_temp_dir("unknown-none");
    assert!(run_connector_subcommand(tree.path(), "bogus", "x").is_none());
}

#[test]
fn management_subcommands_fall_through_until_wired() {
    // The parser recognises `list` and friends, but their report bodies arrive
    // with T-011/T-013; until then the dispatcher returns `None` so the caller
    // renders the usage block.
    let _lock = cwd_test_lock().lock().unwrap_or_else(|e| e.into_inner());
    let (_guard, tree) = enter_temp_dir("mgmt-fallthrough");
    write_connectors_config(&tree, r#"{ "enabled": true }"#);
    assert!(run_connector_subcommand(tree.path(), "list", "").is_none());
    assert!(run_connector_subcommand(tree.path(), "enable", "echo").is_none());
}

#[test]
fn malformed_arguments_render_an_err_row_changing_no_state() {
    let _lock = cwd_test_lock().lock().unwrap_or_else(|e| e.into_inner());
    let (_guard, tree) = enter_temp_dir("malformed-args");
    write_connectors_config(&tree, r#"{ "enabled": true }"#);

    let report = run_connector_subcommand(tree.path(), "add", "").expect("err report");
    assert!(report.contains("[err]"));
    assert!(report.starts_with("From: /connectors add"));
    assert!(
        !tree.path().join(".ragent").join("connectors").exists(),
        "a malformed argument must install nothing"
    );
}

#[test]
fn stores_report_is_attributed_ascii_and_offline_without_check() {
    let _lock = cwd_test_lock().lock().unwrap_or_else(|e| e.into_inner());
    let (_guard, tree) = enter_temp_dir("stores-report");
    write_connectors_config(&tree, r#"{ "enabled": true }"#);

    let report = run_connector_subcommand(tree.path(), "stores", "").expect("stores report");
    assert!(report.contains("From: /connectors stores"));
    assert!(report.contains("claude: [default] https://"), "{report}");
    assert!(report.is_ascii());
    assert!(
        !tree.path().join(".ragent").join("connectors").exists(),
        "reporting the stores must create no connector store"
    );
}

#[test]
fn disabled_subsystem_reports_for_every_non_help_subcommand() {
    let _lock = cwd_test_lock().lock().unwrap_or_else(|e| e.into_inner());
    let (_guard, tree) = enter_temp_dir("disabled-switch");
    write_connectors_config(&tree, r#"{ "enabled": false }"#);

    for (sub, rest) in [
        ("list", ""),
        ("claude", ""),
        ("stores", ""),
        ("enable", "echo"),
    ] {
        let report = run_connector_subcommand(tree.path(), sub, rest)
            .unwrap_or_else(|| panic!("`{sub}` must render a report"));
        assert!(
            report.contains("[err]") && report.contains("disabled"),
            "`{sub}` must report the disabled subsystem: {report}"
        );
        assert!(
            !tree.path().join(".ragent").join("connectors").exists(),
            "`{sub}` must discover nothing while disabled"
        );
    }

    // `help` remains available while the subsystem is disabled (FR-021).
    let help = run_connector_subcommand(tree.path(), "help", "").expect("help renders");
    assert!(help.contains("command reference"));
}

#[test]
fn add_local_directory_installs_enabled_and_reports_ok() {
    let _lock = cwd_test_lock().lock().unwrap_or_else(|e| e.into_inner());
    let (_guard, tree) = enter_temp_dir("add-local");
    write_connectors_config(&tree, r#"{ "enabled": true }"#);

    // A local source directory holding a connector manifest.
    let source = tree.path().join("src").join("echo");
    std::fs::create_dir_all(&source).expect("source dir");
    std::fs::write(
        source.join(MANIFEST_FILE),
        r#"{ "id": "echo", "name": "Echo", "category": "developer",
             "servers": [ { "id": "main", "transport": "stdio", "command": "/bin/true" } ] }"#,
    )
    .expect("manifest writable");

    let source_arg = source.to_string_lossy().to_string();
    let report = run_connector_subcommand(tree.path(), "add", &source_arg).expect("add report");
    assert!(report.starts_with("From: /connectors add"));
    assert!(report.contains("[ok]"), "{report}");
    assert!(
        report.contains("enabled"),
        "a fresh install is enabled (FR-011)"
    );
    assert!(
        tree.path()
            .join(".ragent")
            .join("connectors")
            .join("echo")
            .join(MANIFEST_FILE)
            .exists(),
        "the connector must be installed into the project store"
    );
}

#[test]
fn store_and_config_defaults_when_no_block_is_present() {
    let _lock = cwd_test_lock().lock().unwrap_or_else(|e| e.into_inner());
    let (_guard, tree) = enter_temp_dir("defaults");
    // No ragent.json at all: the compiled defaults apply (enabled).
    let (_, config): (_, ConnectorsConfig) = store_and_config(tree.path());
    assert!(config.is_enabled());
}
