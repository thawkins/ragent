//! Tests for `/plugins add` and `/plugins remove` report rendering
//! (spec `plugins` T-011; FR-007, FR-010).
//!
//! The reports are pure renderings, so they are asserted against outcomes and
//! errors produced by the real store operations.

use std::path::{Path, PathBuf};

use ragent_plugins::{
    AddError, RemoveError, StoreDirs, add, add_error_report, add_report, remove,
    remove_error_report, remove_report,
};

static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// RAII sandboxed temp tree rooted at `target/temp/` (AGENTS.md: no `/tmp`).
struct TempTree(PathBuf);

impl TempTree {
    fn new(name: &str) -> Self {
        let unique = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/temp/plugins-test/report-{name}-{}-{unique}",
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

fn stage_codex_plugin(root: &Path, manifest: &str) -> PathBuf {
    let plugin = root.join("codex-weather");
    std::fs::create_dir_all(&plugin).unwrap();
    std::fs::write(plugin.join("codex-plugin.json"), manifest).unwrap();
    std::fs::write(plugin.join("index.js"), "// entry").unwrap();
    plugin
}

#[test]
fn add_report_names_id_dialect_and_disabled_reminder() {
    let tree = TempTree::new("add-ok");
    let source = stage_codex_plugin(
        &tree.0.join("src"),
        r#"{ "id": "codex-weather", "name": "Weather", "version": "2.3.4", "entry": "index.js" }"#,
    );
    let outcome = add(&dirs(&tree), &tree.0, source.to_str().unwrap(), false).expect("add");

    let report = add_report(&outcome);
    assert!(report.starts_with("From: /plugins add"));
    assert!(report.contains("[ok]"));
    assert!(report.contains("`codex-weather`"));
    assert!(report.contains("dialect: codex"));
    assert!(report.contains("version: 2.3.4"));
    assert!(report.contains("disabled"));
    assert!(report.contains("/plugins enable codex-weather"));
    // No non-ASCII bytes (AGENTS.md ASCII-only reports).
    assert!(report.is_ascii());
}

#[test]
fn add_error_report_renders_refusal() {
    let err = AddError::Exists("codex-weather".to_string());
    let report = add_error_report(&err);
    assert!(report.starts_with("From: /plugins add"));
    assert!(report.contains("[err]"));
    assert!(report.contains("codex-weather"));
    assert!(report.contains("--force"));
}

#[test]
fn remove_report_names_id_and_store() {
    let tree = TempTree::new("remove-ok");
    let source = stage_codex_plugin(
        &tree.0.join("src"),
        r#"{ "id": "codex-weather", "name": "Weather", "version": "1.0.0", "entry": "index.js" }"#,
    );
    add(&dirs(&tree), &tree.0, source.to_str().unwrap(), false).expect("add");
    let outcome = remove(&dirs(&tree), "codex-weather").expect("remove");

    let report = remove_report(&outcome);
    assert!(report.starts_with("From: /plugins remove"));
    assert!(report.contains("[ok]"));
    assert!(report.contains("`codex-weather`"));
    assert!(report.is_ascii());
}

#[test]
fn remove_error_report_renders_disable_hint() {
    let err = RemoveError::Enabled("codex-weather".to_string());
    let report = remove_error_report(&err);
    assert!(report.starts_with("From: /plugins remove"));
    assert!(report.contains("[err]"));
    assert!(report.contains("/plugins disable codex-weather"));
}
