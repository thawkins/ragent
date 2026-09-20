//! Tests for the `/plugins add` and `/plugins remove` command glue
//! (spec `plugins` T-011; FR-007, FR-010).

use std::path::{Path, PathBuf};

use ragent_plugins::{
    StoreArgError, StoreCommand, StoreDirs, parse_store_command, run_store_command, scan_dirs,
};

static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Enabled default plugin configuration (the tests here exercise the store
/// operations, not the master switch).
fn config() -> ragent_config::PluginsConfig {
    ragent_config::PluginsConfig::default()
}

/// RAII sandboxed temp tree rooted at `target/temp/` (AGENTS.md: no `/tmp`).
struct TempTree(PathBuf);

impl TempTree {
    fn new(name: &str) -> Self {
        let unique = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/temp/plugins-test/commands-{name}-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).expect("temp tree creatable");
        Self(path)
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn dirs(tree: &TempTree) -> StoreDirs {
    StoreDirs {
        project: Some(tree.0.join("proj/.ragent/plugins")),
        global: Some(tree.0.join("global/plugins")),
    }
}

const CODEX: &str =
    r#"{ "id": "codex-weather", "name": "Weather", "version": "1.0.0", "entry": "index.js" }"#;

fn stage_codex_plugin(root: &Path, dir_name: &str) -> PathBuf {
    let plugin = root.join(dir_name);
    std::fs::create_dir_all(&plugin).unwrap();
    std::fs::write(plugin.join("codex-plugin.json"), CODEX).unwrap();
    std::fs::write(plugin.join("index.js"), "// entry").unwrap();
    plugin
}

// ── parse_store_command ──────────────────────────────────────────────────────

#[test]
fn parse_add_extracts_source_and_force() {
    assert_eq!(
        parse_store_command("add", "/tmp/plugin --force"),
        Some(Ok(StoreCommand::Add {
            source: "/tmp/plugin".to_string(),
            force: true,
        }))
    );
    assert_eq!(
        parse_store_command("add", "https://example.com/p.zip"),
        Some(Ok(StoreCommand::Add {
            source: "https://example.com/p.zip".to_string(),
            force: false,
        }))
    );
}

#[test]
fn parse_add_joins_quoted_source_tokens() {
    // A path with spaces survives because non-flag tokens are re-joined.
    assert_eq!(
        parse_store_command("add", "/home/u/my plugins/weather"),
        Some(Ok(StoreCommand::Add {
            source: "/home/u/my plugins/weather".to_string(),
            force: false,
        }))
    );
}

#[test]
fn parse_add_without_source_is_missing_source() {
    assert_eq!(
        parse_store_command("add", ""),
        Some(Err(StoreArgError::MissingAddSource))
    );
    // A lone `--force` still has no source.
    assert_eq!(
        parse_store_command("add", "--force"),
        Some(Err(StoreArgError::MissingAddSource))
    );
}

#[test]
fn parse_remove_takes_first_token_as_id() {
    assert_eq!(
        parse_store_command("remove", "codex-weather"),
        Some(Ok(StoreCommand::Remove {
            plugin_id: "codex-weather".to_string(),
        }))
    );
    // Extra tokens are ignored (only the id matters).
    assert_eq!(
        parse_store_command("remove", "codex-weather --force"),
        Some(Ok(StoreCommand::Remove {
            plugin_id: "codex-weather".to_string(),
        }))
    );
}

#[test]
fn parse_remove_without_id_is_missing_id() {
    assert_eq!(
        parse_store_command("remove", ""),
        Some(Err(StoreArgError::MissingRemoveId))
    );
}

#[test]
fn parse_unknown_subcommand_returns_none() {
    assert_eq!(parse_store_command("list", "anything"), None);
    assert_eq!(parse_store_command("enable", "id"), None);
}

#[test]
fn arg_error_reports_are_actionable() {
    let add = StoreArgError::MissingAddSource.report("add");
    assert!(add.starts_with("From: /plugins add"));
    assert!(add.contains("[err]"));
    assert!(add.contains("--force"));

    let remove = StoreArgError::MissingRemoveId.report("remove");
    assert!(remove.starts_with("From: /plugins remove"));
    assert!(remove.contains("Usage"));
}

// ── run_store_command ────────────────────────────────────────────────────────

#[test]
fn run_add_installs_and_reports() {
    let tree = TempTree::new("run-add");
    let source = stage_codex_plugin(&tree.0.join("src"), "codex-weather");

    let report = run_store_command(
        &config(),
        &dirs(&tree),
        &tree.0,
        "add",
        source.to_str().unwrap(),
    )
    .expect("Some");
    assert!(report.contains("[ok]"));
    assert!(report.contains("`codex-weather`"));
    assert!(report.contains("/plugins enable codex-weather"));
    // Installed disabled (FR-007).
    let found = scan_dirs(dirs(&tree));
    assert_eq!(found.len(), 1);
    assert!(!found[0].enabled);
}

#[test]
fn run_add_refusal_is_reported_not_returned_as_error() {
    let tree = TempTree::new("run-add-refuse");
    let source = stage_codex_plugin(&tree.0.join("src"), "codex-weather");
    // First add succeeds, second refuses without --force.
    run_store_command(
        &config(),
        &dirs(&tree),
        &tree.0,
        "add",
        source.to_str().unwrap(),
    )
    .expect("ok");

    let report = run_store_command(
        &config(),
        &dirs(&tree),
        &tree.0,
        "add",
        source.to_str().unwrap(),
    )
    .expect("Some");
    assert!(report.contains("[err]"));
    assert!(report.contains("--force"));
}

#[test]
fn run_add_force_overwrites() {
    let tree = TempTree::new("run-add-force");
    let source = stage_codex_plugin(&tree.0.join("src"), "codex-weather");
    run_store_command(
        &config(),
        &dirs(&tree),
        &tree.0,
        "add",
        source.to_str().unwrap(),
    )
    .expect("ok");
    let report = run_store_command(
        &config(),
        &dirs(&tree),
        &tree.0,
        "add",
        &format!("{} --force", source.display()),
    )
    .expect("Some");
    assert!(report.contains("[ok]"));
}

#[test]
fn run_remove_deletes_disabled_plugin() {
    let tree = TempTree::new("run-remove");
    let source = stage_codex_plugin(&tree.0.join("src"), "codex-weather");
    run_store_command(
        &config(),
        &dirs(&tree),
        &tree.0,
        "add",
        source.to_str().unwrap(),
    )
    .expect("ok");

    let report = run_store_command(&config(), &dirs(&tree), &tree.0, "remove", "codex-weather")
        .expect("Some");
    assert!(report.contains("[ok]"));
    assert!(scan_dirs(dirs(&tree)).is_empty());
}

#[test]
fn run_remove_unknown_is_reported() {
    let tree = TempTree::new("run-remove-unknown");
    let report =
        run_store_command(&config(), &dirs(&tree), &tree.0, "remove", "ghost").expect("Some");
    assert!(report.contains("[err]"));
    assert!(report.contains("ghost"));
}

#[test]
fn run_store_command_returns_none_for_other_subcommands() {
    let tree = TempTree::new("run-none");
    assert!(run_store_command(&config(), &dirs(&tree), &tree.0, "list", "").is_none());
}

#[test]
fn run_store_command_refuses_while_subsystem_disabled() {
    // Acceptance criterion 8 (`plugins.enabled: false`): install/uninstall is
    // refused and nothing is written to the store.
    let tree = TempTree::new("run-disabled");
    let source = stage_codex_plugin(&tree.0.join("src"), "codex-weather");
    let disabled = ragent_config::PluginsConfig {
        enabled: false,
        ..ragent_config::PluginsConfig::default()
    };

    let report = run_store_command(
        &disabled,
        &dirs(&tree),
        &tree.0,
        "add",
        source.to_str().unwrap(),
    )
    .expect("Some");
    assert!(report.contains("[err]"), "{report}");
    assert!(report.contains("plugins.enabled = false"), "{report}");
    assert!(scan_dirs(dirs(&tree)).is_empty(), "nothing installed");
}

#[test]
fn run_missing_source_reports_usage_without_panicking() {
    let tree = TempTree::new("run-missing");
    let report = run_store_command(&config(), &dirs(&tree), &tree.0, "add", "").expect("Some");
    assert!(report.contains("[err]"));
    assert!(report.contains("Usage"));
}
