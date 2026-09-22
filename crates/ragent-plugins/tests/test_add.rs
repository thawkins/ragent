//! Tests for `/plugins add` source handling (spec `plugins` T-006; FR-007,
//! FR-010, FR-023).

use std::io::Write as _;
use std::path::{Path, PathBuf};

use ragent_plugins::{AddError, MAX_ARCHIVE_BYTES, PluginDialect, StoreDirs, add, scan_dirs};

static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

struct TempTree(PathBuf);

impl TempTree {
    fn new(name: &str) -> Self {
        let unique = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/temp/plugins-test/add-{name}-{}-{unique}",
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

const CODEX: &str = r#"{ "name": "Weather", "version": "1.0.0", "entry": "index.js" }"#;

fn stage_codex_plugin(root: &Path, manifest: &str) -> PathBuf {
    let plugin = root.join("codex-weather");
    std::fs::create_dir_all(&plugin).unwrap();
    std::fs::write(plugin.join("codex-plugin.json"), manifest).unwrap();
    std::fs::write(plugin.join("index.js"), "// entry").unwrap();
    plugin
}

fn make_zip(path: &Path, files: &[(&str, &str)]) {
    std::fs::create_dir_all(path.parent().expect("has parent")).unwrap();
    let file = std::fs::File::create(path).unwrap();
    let mut writer = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default();
    for (name, body) in files {
        writer.start_file(name, options).unwrap();
        writer.write_all(body.as_bytes()).unwrap();
    }
    writer.finish().unwrap();
}

fn make_tar_gz(path: &Path, files: &[(&str, &str)]) {
    std::fs::create_dir_all(path.parent().expect("has parent")).unwrap();
    let file = std::fs::File::create(path).unwrap();
    let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
    let mut builder = tar::Builder::new(encoder);
    for (name, body) in files {
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(tar::EntryType::Regular);
        header.set_size(body.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder
            .append_data(&mut header, name, body.as_bytes())
            .unwrap();
    }
    builder.into_inner().unwrap().finish().unwrap();
}

// ── local directory source (FR-007, FR-010) ─────────────────────────────────

#[test]
fn add_local_directory_installs_enabled_and_reports_id_and_dialect() {
    let tree = TempTree::new("dir");
    let source = stage_codex_plugin(&tree.0.join("src"), CODEX);

    let outcome = add(&dirs(&tree), &tree.0, source.to_str().unwrap(), false).expect("add");
    assert_eq!(outcome.parsed.descriptor.id, "weather");
    assert_eq!(outcome.parsed.descriptor.dialect, PluginDialect::Codex);
    assert_eq!(
        outcome.installed_dir,
        tree.0.join("proj/.ragent/plugins/weather")
    );
    // Entry code is copied, never executed (FR-023): the file exists but no
    // runtime touched it.
    assert!(outcome.installed_dir.join("index.js").exists());

    // The plugin exists in a fresh scan, enabled (FR-007): add records it in
    // the ledger so it loads at the next session start.
    let found = scan_dirs(dirs(&tree));
    assert_eq!(found.len(), 1);
    assert!(found[0].enabled);
}

#[test]
fn add_relative_source_resolves_against_workdir() {
    let tree = TempTree::new("dir-relative");
    let workdir = tree.0.join("proj");
    stage_codex_plugin(&workdir.join("vendor"), CODEX);

    let outcome = add(&dirs(&tree), &workdir, "vendor/codex-weather", false).expect("add");
    assert_eq!(outcome.parsed.descriptor.id, "weather");
}

#[test]
fn add_nested_codex_manifest_directory_installs() {
    let tree = TempTree::new("dir-nested-codex");
    let plugin = tree.0.join("src/linear");
    std::fs::create_dir_all(plugin.join(".codex-plugin")).unwrap();
    std::fs::write(
        plugin.join(".codex-plugin/plugin.json"),
        r#"{ "name": "Linear", "version": "5.0.1" }"#,
    )
    .unwrap();

    let outcome = add(&dirs(&tree), &tree.0, plugin.to_str().unwrap(), false).expect("add");
    assert_eq!(outcome.parsed.descriptor.id, "linear");
    assert_eq!(outcome.parsed.descriptor.dialect, PluginDialect::Codex);
    assert!(
        outcome
            .installed_dir
            .join(".codex-plugin/plugin.json")
            .exists()
    );
}

#[test]
fn add_existing_id_refuses_without_force_and_overwrites_with_force() {
    let tree = TempTree::new("dir-force");
    let source = stage_codex_plugin(&tree.0.join("src"), CODEX);

    add(&dirs(&tree), &tree.0, source.to_str().unwrap(), false).expect("first add");
    let source2 = stage_codex_plugin(&tree.0.join("src2"), &CODEX.replace("1.0.0", "2.0.0"));

    let err = add(&dirs(&tree), &tree.0, source2.to_str().unwrap(), false).unwrap_err();
    assert!(matches!(err, AddError::Exists(id) if id == "weather"));

    let outcome = add(&dirs(&tree), &tree.0, source2.to_str().unwrap(), true).expect("forced add");
    assert_eq!(outcome.parsed.descriptor.version, "2.0.0");
}

#[test]
fn add_directory_without_manifest_is_refused() {
    let tree = TempTree::new("dir-nomanifest");
    let source = tree.0.join("src/notaplugin");
    std::fs::create_dir_all(&source).unwrap();
    std::fs::write(source.join("README.md"), "no manifest").unwrap();

    let err = add(&dirs(&tree), &tree.0, source.to_str().unwrap(), false).unwrap_err();
    assert!(matches!(err, AddError::NotAPlugin(_)));
    // Store untouched: no plugin directory was created.
    let store = tree.0.join("proj/.ragent/plugins");
    assert!(scan_dirs(dirs(&tree)).is_empty());
    assert!(!store.join("notaplugin").exists() || scan_dirs(dirs(&tree)).is_empty());
}

#[test]
fn add_directory_with_bad_manifest_is_refused_nothing_installed() {
    let tree = TempTree::new("dir-badmanifest");
    let source = stage_codex_plugin(&tree.0.join("src"), "{ \"name\": \"broken\",");

    let err = add(&dirs(&tree), &tree.0, source.to_str().unwrap(), false).unwrap_err();
    assert!(matches!(err, AddError::Manifest(_)));
    // Store untouched.
    assert!(scan_dirs(dirs(&tree)).is_empty());
}

// ── local archive sources (FR-010) ──────────────────────────────────────────

#[test]
fn add_local_zip_extracts_and_installs() {
    let tree = TempTree::new("zip");
    let zip_path = tree.0.join("src/codex-weather.zip");
    make_zip(
        &zip_path,
        &[
            ("codex-weather/codex-plugin.json", CODEX),
            ("codex-weather/index.js", "// entry"),
        ],
    );

    let outcome = add(&dirs(&tree), &tree.0, zip_path.to_str().unwrap(), false).expect("add zip");
    assert_eq!(outcome.parsed.descriptor.id, "weather");
    assert_eq!(outcome.parsed.descriptor.dialect, PluginDialect::Codex);
    assert!(outcome.installed_dir.join("codex-plugin.json").exists());
    assert!(outcome.installed_dir.join("index.js").exists());
}

#[test]
fn add_local_tar_gz_extracts_and_installs() {
    let tree = TempTree::new("tgz");
    let tgz_path = tree.0.join("src/claude-todo.tar.gz");
    make_tar_gz(
        &tgz_path,
        &[
            (
                "claude-todo/.claude-plugin/plugin.json",
                r#"{ "name": "Todo", "main": "index.js" }"#,
            ),
            ("claude-todo/index.js", "// entry"),
        ],
    );

    let outcome = add(&dirs(&tree), &tree.0, tgz_path.to_str().unwrap(), false).expect("add tgz");
    assert_eq!(outcome.parsed.descriptor.id, "todo");
    assert_eq!(outcome.parsed.descriptor.dialect, PluginDialect::Claude);
}

#[test]
fn add_zip_with_traversal_entry_is_refused_and_removed() {
    let tree = TempTree::new("zip-evil");
    let zip_path = tree.0.join("src/evil.zip");
    make_zip(
        &zip_path,
        &[
            ("codex-plugin.json", CODEX),
            ("../../escape.js", "// never written"),
        ],
    );

    let err = add(&dirs(&tree), &tree.0, zip_path.to_str().unwrap(), false).unwrap_err();
    assert!(matches!(err, AddError::UnsafePath(_)));
    // Refusal leaves no trace: no staging, no installed plugin, nothing
    // outside the store tree.
    assert!(scan_dirs(dirs(&tree)).is_empty());
    assert!(!tree.0.join("escape.js").exists());
    assert!(!tree.0.join("proj/.ragent/plugins/.add-staging").exists());
}

#[test]
fn add_zip_with_absolute_entry_is_refused() {
    let tree = TempTree::new("zip-abs");
    let zip_path = tree.0.join("src/abs.zip");
    make_zip(
        &zip_path,
        &[("codex-plugin.json", CODEX), ("/etc/evil.js", "// no")],
    );
    let err = add(&dirs(&tree), &tree.0, zip_path.to_str().unwrap(), false).unwrap_err();
    assert!(matches!(err, AddError::UnsafePath(_)));
}

#[test]
fn add_oversize_archive_is_refused() {
    let tree = TempTree::new("zip-big");
    let zip_path = tree.0.join("src/big.zip");
    std::fs::create_dir_all(zip_path.parent().unwrap()).unwrap();
    // 50 MiB + 1 byte of stored (uncompressed) data.
    let file = std::fs::File::create(&zip_path).unwrap();
    let mut writer = zip::ZipWriter::new(file);
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    writer.start_file("payload.bin", options).unwrap();
    writer
        .write_all(&vec![b'x'; (MAX_ARCHIVE_BYTES + 1) as usize])
        .unwrap();
    writer.finish().unwrap();

    let err = add(&dirs(&tree), &tree.0, zip_path.to_str().unwrap(), false).unwrap_err();
    assert!(
        matches!(err, AddError::Io(_) | AddError::TooLarge { .. }),
        "oversize must be refused, got {err:?}"
    );
}

// ── URL sources (FR-010) ────────────────────────────────────────────────────

#[test]
fn add_refuses_http_url() {
    let tree = TempTree::new("http");
    let err = add(
        &dirs(&tree),
        &tree.0,
        "http://example.com/plugins/w.zip",
        false,
    )
    .unwrap_err();
    assert!(matches!(err, AddError::NotHttps(_)));
}

#[test]
fn add_refuses_other_url_schemes() {
    let tree = TempTree::new("schemes");
    let err = add(&dirs(&tree), &tree.0, "ftp://example.com/w.zip", false).unwrap_err();
    assert!(matches!(err, AddError::NotHttps(_)), "ftp: {err:?}");
    let err = add(&dirs(&tree), &tree.0, "file:///tmp/w.zip", false).unwrap_err();
    assert!(matches!(err, AddError::NotHttps(_)), "file: {err:?}");
    // Scheme-less strings that are not files are unrecognised sources, not
    // URLs — the `://`-bearing https check covers the URL family.
    let err = add(&dirs(&tree), &tree.0, "javascript:alert(1)", false).unwrap_err();
    assert!(
        matches!(err, AddError::UnknownSource(_)),
        "javascript: {err:?}"
    );
}

#[test]
fn add_refuses_https_url_without_package_suffix() {
    let tree = TempTree::new("https-nosuffix");
    let err = add(&dirs(&tree), &tree.0, "https://example.com/plugin", false).unwrap_err();
    assert!(matches!(err, AddError::UnknownSource(_)));
}

#[test]
fn add_unknown_local_source_is_refused() {
    let tree = TempTree::new("unknown");
    let err = add(&dirs(&tree), &tree.0, "does/not/exist", false).unwrap_err();
    assert!(matches!(err, AddError::UnknownSource(_)));
}

// ── staging hygiene ─────────────────────────────────────────────────────────

#[test]
fn successful_add_leaves_no_staging_directories() {
    let tree = TempTree::new("staging-clean");
    let source = stage_codex_plugin(&tree.0.join("src"), CODEX);
    add(&dirs(&tree), &tree.0, source.to_str().unwrap(), false).expect("add");
    assert!(!tree.0.join("proj/.ragent/plugins/.add-staging").exists());
    // The ledger records the install as enabled (FR-007); no staging dir
    // survives the successful commit.
    assert!(tree.0.join("proj/.ragent/plugins/_state.json").exists());
}
