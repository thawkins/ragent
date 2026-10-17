//! Additional coverage for `ragent-surface::store`: the ledger reader, the
//! corrupt-ledger quarantine, the entry-directory resolver (real dir, plain
//! file, contained and escaping symlinks, unresolved symlink), and the
//! live-environment `store_dirs` wrapper.

use std::fs;
use std::path::Path;

use ragent_surface::store::{
    MAX_LEDGER_BYTES, quarantine_corrupt_ledger, read_ledger_bytes, store_dirs, store_dirs_at,
    store_entry_dir,
};

fn workdir(tag: &str) -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix(&format!("ragent-surface-store-{tag}-"))
        .tempdir()
        .expect("tempdir")
}

#[cfg(unix)]
fn make_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[test]
fn store_entry_dir_returns_a_real_directory_unchanged() {
    let tmp = workdir("realdir");
    let store = tmp.path().join("store");
    let entry = store.join("plugin-a");
    fs::create_dir_all(&entry).expect("mkdir");
    assert_eq!(store_entry_dir(&store, &entry, "plugin"), Some(entry));
}

#[test]
fn store_entry_dir_rejects_a_plain_file_and_a_missing_path() {
    let tmp = workdir("file");
    let store = tmp.path().join("store");
    fs::create_dir_all(&store).expect("mkdir store");
    let file = store.join("plain.txt");
    fs::write(&file, b"x").expect("write");
    assert_eq!(store_entry_dir(&store, &file, "plugin"), None);

    let missing = store.join("does-not-exist");
    assert_eq!(store_entry_dir(&store, &missing, "plugin"), None);
}

#[cfg(unix)]
#[test]
fn store_entry_dir_accepts_a_symlink_contained_in_the_store() {
    let tmp = workdir("contained");
    let store = tmp.path().join("store");
    let inside = store.join("real-plugin");
    fs::create_dir_all(&inside).expect("mkdir inside");
    let link = store.join("link-plugin");
    if make_symlink(&inside, &link).is_err() {
        return;
    }
    let resolved = store_entry_dir(&store, &link, "plugin").expect("accepted");
    assert!(resolved.is_dir());
    assert!(resolved.starts_with(store.canonicalize().expect("canonical store")));
}

#[cfg(unix)]
#[test]
fn store_entry_dir_rejects_a_symlink_that_escapes_the_store() {
    let tmp = workdir("escape");
    let store = tmp.path().join("store");
    let outside = tmp.path().join("outside");
    fs::create_dir_all(&store).expect("mkdir store");
    fs::create_dir_all(&outside).expect("mkdir outside");
    let link = store.join("escaped");
    if make_symlink(&outside, &link).is_err() {
        return;
    }
    assert_eq!(store_entry_dir(&store, &link, "plugin"), None);
}

#[cfg(unix)]
#[test]
fn store_entry_dir_rejects_a_dangling_symlink() {
    let tmp = workdir("dangling");
    let store = tmp.path().join("store");
    fs::create_dir_all(&store).expect("mkdir store");
    let link = store.join("dangling");
    // Target does not exist, so canonicalize fails and the entry is skipped.
    if make_symlink(&store.join("nowhere"), &link).is_err() {
        return;
    }
    assert_eq!(store_entry_dir(&store, &link, "plugin"), None);
}

#[test]
fn read_ledger_bytes_reads_a_small_file() {
    let tmp = workdir("read");
    let ledger = tmp.path().join("state.json");
    fs::write(&ledger, br#"{"servers":{"a":true}}"#).expect("write");
    assert_eq!(
        read_ledger_bytes(&ledger),
        Some(br#"{"servers":{"a":true}}"#.to_vec())
    );
}

#[test]
fn read_ledger_bytes_returns_none_for_a_missing_file() {
    let tmp = workdir("missing");
    let ledger = tmp.path().join("nope.json");
    assert_eq!(read_ledger_bytes(&ledger), None);
}

#[test]
fn read_ledger_bytes_returns_none_for_an_oversized_file() {
    let tmp = workdir("huge");
    let ledger = tmp.path().join("big.json");
    let file = fs::File::create(&ledger).expect("create");
    file.set_len(MAX_LEDGER_BYTES + 1).expect("set sparse len");
    drop(file);
    assert_eq!(read_ledger_bytes(&ledger), None);
}

#[test]
fn quarantine_corrupt_ledger_renames_the_file_aside() {
    let tmp = workdir("quarantine");
    let ledger = tmp.path().join("state.json");
    fs::write(&ledger, b"{ corrupt").expect("write");
    quarantine_corrupt_ledger(&ledger, "plugin", "parse error");
    assert!(!ledger.exists());
    let aside = ledger.with_extension("json.corrupt");
    assert!(aside.is_file());
    assert_eq!(fs::read_to_string(&aside).expect("read aside"), "{ corrupt");
}

#[test]
fn quarantine_corrupt_ledger_tolerates_a_missing_file() {
    let tmp = workdir("quarantine-missing");
    let ledger = tmp.path().join("absent.json");
    // The rename fails because the file is absent; the call must not panic.
    quarantine_corrupt_ledger(&ledger, "connector", "boom");
    assert!(!ledger.exists());
}

#[test]
fn store_dirs_resolves_both_legs_from_the_live_environment() {
    let tmp = workdir("live");
    let dirs = store_dirs(tmp.path(), None, "plugins");
    assert_eq!(
        dirs.project.as_deref(),
        Some(tmp.path().join(".ragent/plugins").as_path())
    );
    // The global leg mirrors `global_state_dir()`, whatever this host resolves.
    let expected_global = ragent_config::user_dirs::global_state_dir().map(|d| d.join("plugins"));
    assert_eq!(dirs.global, expected_global);

    // An override replaces the project leg and does not affect the global leg.
    let override_dir = tmp.path().join("custom");
    let overridden = store_dirs(tmp.path(), Some(&override_dir), "connectors");
    assert_eq!(overridden.project.as_deref(), Some(override_dir.as_path()));
    assert_eq!(
        overridden.global,
        ragent_config::user_dirs::global_state_dir().map(|d| d.join("connectors"))
    );
}

#[test]
fn store_dirs_at_orders_project_as_destination_over_global() {
    let tmp = workdir("order");
    let global_root = tmp.path().join("config/ragent");
    let dirs = store_dirs_at(tmp.path(), None, Some(&global_root), "plugins");
    // destination prefers the project leg even when a global leg exists.
    assert_eq!(dirs.destination(), dirs.project.as_deref());
    assert!(dirs.destination().is_some());

    let empty = store_dirs_at(tmp.path(), None, None, "plugins");
    assert!(empty.global.is_none());
    assert_eq!(empty.destination(), empty.project.as_deref());
}
