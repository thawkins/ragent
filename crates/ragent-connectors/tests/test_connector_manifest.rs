//! Tests for connector manifest read/write and install staging (spec
//! `connectors` T-004; FR-002, FR-011, FR-027).

mod support;

use support::TempTree;

use std::io::Write as _;
use std::path::{Path, PathBuf};

use ragent_connectors::{
    ConnectorDescriptor, MANIFEST_FILE, MAX_SOURCE_BYTES, StageError, StoreDirs,
    install_descriptor, read_manifest, stage, write_manifest,
};

fn dirs(tree: &TempTree) -> StoreDirs {
    StoreDirs {
        project: Some(tree.0.join("proj/.ragent/connectors")),
        global: Some(tree.0.join("global/connectors")),
    }
}

/// A minimal, valid connector manifest with one stdio server.
fn manifest_for(id: &str, name: &str) -> String {
    format!(
        r#"{{
            "id": "{id}",
            "name": "{name}",
            "servers": [ {{ "id": "main", "transport": "stdio", "command": "/bin/true" }} ]
        }}"#
    )
}

/// A descriptor used for direct-install tests.
fn descriptor_for(id: &str) -> ConnectorDescriptor {
    serde_json::from_str(&manifest_for(id, "Echo")).expect("descriptor parses")
}

fn stage_source_dir(root: &Path, id: &str) -> PathBuf {
    let dir = root.join(id);
    std::fs::create_dir_all(&dir).expect("source dir creatable");
    std::fs::write(dir.join(MANIFEST_FILE), manifest_for(id, "Echo")).expect("manifest writable");
    dir
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

// -- manifest read/write (FR-002) --------------------------------------------

#[test]
fn manifest_round_trips_through_write_and_read() {
    let tree = TempTree::new("round-trip");
    let dir = tree.0.join("store/echo");
    let descriptor = descriptor_for("echo");

    write_manifest(&dir, &descriptor).expect("manifest writes");

    let loaded = read_manifest(&dir).expect("manifest reads");
    assert_eq!(loaded, descriptor);
    // The manifest is a JSON file that a human can read and edit.
    let text = std::fs::read_to_string(dir.join(MANIFEST_FILE)).expect("readable");
    assert!(text.ends_with('\n'));
    assert!(text.contains("\"id\": \"echo\""));
}

#[test]
fn manifest_never_records_a_secret_value() {
    let tree = TempTree::new("no-secret");
    let dir = tree.0.join("store/echo");
    let mut descriptor = descriptor_for("echo");
    descriptor.credential = Some("ECHO_TOKEN".to_string());

    write_manifest(&dir, &descriptor).expect("manifest writes");

    let text = std::fs::read_to_string(dir.join(MANIFEST_FILE)).expect("readable");
    // The credential reference is a name; the descriptor carries no value field.
    assert!(text.contains("ECHO_TOKEN"));
    assert!(!text.contains("secret-value"));
}

#[test]
fn read_manifest_reports_parse_failure_for_malformed_json() {
    let tree = TempTree::new("parse-fail");
    let dir = tree.0.join("store/echo");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(MANIFEST_FILE), b"{ not json").unwrap();
    assert!(read_manifest(&dir).is_err());
}

// -- direct descriptor install (catalogue path, FR-011, FR-027) ---------------

#[test]
fn install_descriptor_writes_an_enabled_connector() {
    let tree = TempTree::new("install-descriptor");
    let store = tree.0.join("store");

    let staged = install_descriptor(&store, descriptor_for("echo"), false).expect("install");

    assert_eq!(staged.installed_dir, store.join("echo"));
    assert!(store.join("echo").join(MANIFEST_FILE).is_file());
    // FR-011: recorded enabled so a fresh connector is ready to use.
    let ledger = ragent_connectors::StoreLedger::load(&store);
    assert!(ledger.state("echo").is_some_and(|s| s.enabled));
}

#[test]
fn install_descriptor_refuses_existing_id_without_force_and_allows_with_force() {
    let tree = TempTree::new("install-collision");
    let store = tree.0.join("store");
    install_descriptor(&store, descriptor_for("echo"), false).expect("first install");

    let err = install_descriptor(&store, descriptor_for("echo"), false).expect_err("refused");
    assert!(
        matches!(err, StageError::Exists(ref id) if id == "echo"),
        "expected Exists(echo), got {err:?}"
    );

    install_descriptor(&store, descriptor_for("echo"), true).expect("force overwrites");
}

#[test]
fn install_descriptor_refuses_a_descriptor_with_no_server() {
    let tree = TempTree::new("install-no-server");
    let store = tree.0.join("store");
    let descriptor: ConnectorDescriptor =
        serde_json::from_str(r#"{ "id": "empty", "name": "Empty", "servers": [] }"#)
            .expect("parses");

    let err = install_descriptor(&store, descriptor, false).expect_err("refused");
    assert!(matches!(err, StageError::Descriptor(_)), "got {err:?}");
}

// -- stage: local directory (FR-011, FR-027) ---------------------------------

#[test]
fn stage_local_directory_installs_an_enabled_connector() {
    let tree = TempTree::new("stage-dir");
    let source = stage_source_dir(&tree.0.join("src"), "echo");

    let staged = stage(&dirs(&tree), &tree.0, source.to_str().unwrap(), false).expect("stage");
    assert_eq!(staged.descriptor.id.as_str(), "echo");
    assert_eq!(
        staged.descriptor.provenance,
        ragent_connectors::ConnectorProvenance::Local
    );
    let store = tree.0.join("proj/.ragent/connectors");
    assert!(store.join("echo").join(MANIFEST_FILE).is_file());

    // Discovery finds the installed connector, enabled.
    let found = ragent_connectors::scan_dirs(dirs(&tree));
    assert_eq!(found.len(), 1);
    assert!(found[0].enabled);
}

#[test]
fn stage_second_install_of_same_id_is_refused_without_force() {
    let tree = TempTree::new("stage-collision");
    let source = stage_source_dir(&tree.0.join("src"), "echo");
    let dirs = dirs(&tree);
    stage(&dirs, &tree.0, source.to_str().unwrap(), false).expect("first");

    let err = stage(&dirs, &tree.0, source.to_str().unwrap(), false).expect_err("refused");
    assert!(
        matches!(err, StageError::Exists(ref id) if id == "echo"),
        "got {err:?}"
    );

    // --force overwrites and succeeds.
    stage(&dirs, &tree.0, source.to_str().unwrap(), true).expect("force");
}

#[test]
fn stage_source_relative_resolves_against_workdir() {
    let tree = TempTree::new("stage-relative");
    let workdir = tree.0.join("proj");
    stage_source_dir(&workdir.join("vendor"), "echo");

    let staged = stage(&dirs(&tree), &workdir, "vendor/echo", false).expect("stage");
    assert_eq!(staged.descriptor.id.as_str(), "echo");
}

#[test]
fn stage_descends_into_a_single_wrapper_directory() {
    let tree = TempTree::new("stage-wrapper");
    // GitHub-style archive: everything under `<repo>-<ref>/`.
    let wrapper = tree.0.join("src/echo-abc123");
    std::fs::create_dir_all(&wrapper).unwrap();
    std::fs::write(wrapper.join(MANIFEST_FILE), manifest_for("echo", "Echo")).unwrap();

    let staged = stage(
        &dirs(&tree),
        &tree.0,
        tree.0.join("src").to_str().unwrap(),
        false,
    )
    .expect("stage");
    assert_eq!(staged.descriptor.id.as_str(), "echo");
}

#[test]
fn stage_unrecognised_source_is_refused() {
    let tree = TempTree::new("stage-unknown");
    let err = stage(&dirs(&tree), &tree.0, "no/such/path", false).expect_err("refused");
    assert!(matches!(err, StageError::UnknownSource(_)), "got {err:?}");
}

// -- stage: local archives (FR-011) ------------------------------------------

#[test]
fn stage_zip_archive_installs() {
    let tree = TempTree::new("stage-zip");
    let zip = tree.0.join("echo.zip");
    make_zip(&zip, &[(MANIFEST_FILE, &manifest_for("echo", "Echo"))]);

    let staged = stage(&dirs(&tree), &tree.0, zip.to_str().unwrap(), false).expect("stage");
    assert_eq!(staged.descriptor.id.as_str(), "echo");
    assert!(staged.installed_dir.join(MANIFEST_FILE).is_file());
}

#[test]
fn stage_tar_gz_archive_installs() {
    let tree = TempTree::new("stage-targz");
    let archive = tree.0.join("echo.tar.gz");
    make_tar_gz(&archive, &[(MANIFEST_FILE, &manifest_for("echo", "Echo"))]);

    let staged = stage(&dirs(&tree), &tree.0, archive.to_str().unwrap(), false).expect("stage");
    assert_eq!(staged.descriptor.id.as_str(), "echo");
}

#[test]
fn stage_archive_escape_entry_is_refused_and_writes_nothing_outside() {
    let tree = TempTree::new("stage-escape");
    let zip = tree.0.join("escape.zip");
    make_zip(
        &zip,
        &[
            ("../../escape.json", "{}"),
            (MANIFEST_FILE, &manifest_for("echo", "Echo")),
        ],
    );

    let err = stage(&dirs(&tree), &tree.0, zip.to_str().unwrap(), false).expect_err("refused");
    assert!(
        matches!(err, StageError::UnsafePath(ref p) if p.contains("escape.json")),
        "got {err:?}"
    );
    // No escape file was written beside the tree root.
    assert!(!tree.0.parent().unwrap().join("escape.json").exists());
    assert!(!tree.0.join("escape.json").exists());
}

// -- stage: refusals (FR-027, FR-028) ----------------------------------------

#[test]
fn stage_refuses_a_non_https_url_scheme() {
    let tree = TempTree::new("stage-http");
    let err = stage(
        &dirs(&tree),
        &tree.0,
        "http://example.org/connectors.zip",
        false,
    )
    .expect_err("refused");
    assert!(
        matches!(err, StageError::NotHttps(ref s) if s.starts_with("http://")),
        "got {err:?}"
    );
}

#[test]
fn stage_archive_over_the_size_cap_is_refused() {
    let tree = TempTree::new("stage-too-big");
    let archive = tree.0.join("big.zip");
    std::fs::create_dir_all(&tree.0).unwrap();
    std::fs::write(&archive, vec![0u8; (MAX_SOURCE_BYTES + 1) as usize]).unwrap();

    let err = stage(&dirs(&tree), &tree.0, archive.to_str().unwrap(), false).expect_err("refused");
    assert!(matches!(err, StageError::TooLarge { .. }), "got {err:?}");
}

// -- store-leg selection -----------------------------------------------------

#[test]
fn stage_falls_back_to_the_global_leg_when_no_project_leg_resolves() {
    let tree = TempTree::new("stage-global");
    let source = stage_source_dir(&tree.0.join("src"), "echo");
    let dirs = StoreDirs {
        project: None,
        global: Some(tree.0.join("global/connectors")),
    };

    let staged = stage(&dirs, &tree.0, source.to_str().unwrap(), false).expect("stage");
    assert_eq!(staged.installed_dir, tree.0.join("global/connectors/echo"));
}
