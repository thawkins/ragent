//! Tests for the npm `_npx` cache repair pass (spec `mcp`).
//!
//! npm installs an `npx` package into `<cache>/_npx/<hash>/`, and replaces an
//! existing copy by first *retiring* it (`rename` onto the deterministic
//! sibling `node_modules/.<name>-<8 alnum>`) and then placing the new copy. If
//! an earlier run was interrupted, or its retire rename already failed, the
//! retired directory is still there - and every later `reify` of that package
//! fails `ENOTEMPTY` on the same rename before the server emits any MCP
//! framing. The transport can only report that as `connection closed:
//! initialize response`.
//!
//! The sweep is exercised against a synthetic cache tree under `target/temp/`,
//! never a real `~/.npm`: a test must not depend on, or mutate, the operator's
//! package cache.

use std::path::{Path, PathBuf};

use ragent_agent::mcp::npx_cache::{RepairReport, repair_npx_cache, retired_package_stem};

/// A scratch directory unique to `name`, removed and recreated so a previous
/// failed run cannot influence this one.
fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp")
        .join("npx-cache-tests")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

/// Build one `npx` install directory: `package.json`, `node_modules/` and the
/// `node_modules/.package-lock.json` npm writes once an install has completed.
fn install_dir(root: &Path, hash: &str) -> PathBuf {
    let dir = root.join("_npx").join(hash);
    let node_modules = dir.join("node_modules");
    std::fs::create_dir_all(&node_modules).expect("mkdir node_modules");
    std::fs::write(dir.join("package.json"), "{\"dependencies\":{}}").expect("write package.json");
    std::fs::write(node_modules.join(".package-lock.json"), "{}").expect("write package-lock");
    dir
}

/// Create a package directory with one file in it, so "still present" and
/// "removed" are both observable.
fn package(dir: &Path, name: &str) {
    let pkg = dir.join("node_modules").join(name);
    std::fs::create_dir_all(&pkg).expect("mkdir package");
    std::fs::write(pkg.join("index.js"), "module.exports = 1;\n").expect("write file");
}

/// Create a package directory nested under an npm scope (`@scope/<name>`).
fn scoped_package(dir: &Path, scope: &str, name: &str) {
    let pkg = dir.join("node_modules").join(scope).join(name);
    std::fs::create_dir_all(&pkg).expect("mkdir scoped package");
    std::fs::write(pkg.join("index.js"), "module.exports = 1;\n").expect("write file");
}

/// Create a retired directory for `stem`, named as npm names it.
fn retired(dir: &Path, stem: &str) -> PathBuf {
    let path = dir.join("node_modules").join(format!(".{stem}-Ab3Cd9Ef"));
    std::fs::create_dir_all(&path).expect("mkdir retired");
    std::fs::write(path.join("index.js"), "module.exports = 1;\n").expect("write file");
    path
}

// -- The name rule -----------------------------------------------------------

/// npm's retire names carry exactly eight alphanumeric characters after the
/// final dash; the package name itself is what precedes it.
#[test]
fn test_retired_package_stem_recognises_npm_retire_names() {
    assert_eq!(
        retired_package_stem(".chrome-devtools-mcp-FKjxnyhX"),
        Some("chrome-devtools-mcp")
    );
    assert_eq!(retired_package_stem(".pdfcraft-1a2B3c4D"), Some("pdfcraft"));
    // A name that itself contains dashes keeps them: only the last dash splits.
    assert_eq!(
        retired_package_stem(".some-multi-part-name-Zz9Yy8Xx"),
        Some("some-multi-part-name")
    );
}

/// Everything else in `node_modules` must be left alone - in particular the
/// two dotted siblings npm always writes, which a looser rule would delete.
#[test]
fn test_retired_package_stem_rejects_everything_else() {
    for name in [
        "chrome-devtools-mcp",    // the live package, no leading dot
        ".bin",                   // npm's bin shim directory
        ".package-lock.json",     // npm's install receipt
        ".",                      // readdir's own entry
        "..",                     // ditto
        ".camelCase-TooLongSufx", // suffix longer than eight
        ".pkg-shrt",              // suffix shorter than eight
    ] {
        assert_eq!(
            retired_package_stem(name),
            None,
            "unexpected verdict for {name:?}"
        );
    }
}

/// A digit-only suffix is still eight alphanumeric characters, so it is a
/// retire name like any other.
#[test]
fn test_retired_package_stem_accepts_a_digit_only_suffix() {
    assert_eq!(retired_package_stem(".pkg-12345678"), Some("pkg"));
}

/// A suffix with a non-alphanumeric character is not a retire name, so a
/// plugin's own dotted scratch directory survives.
#[test]
fn test_retired_package_stem_rejects_non_alphanumeric_suffix() {
    assert_eq!(retired_package_stem(".pkg-Ab3Cd9.f"), None);
    assert_eq!(retired_package_stem(".pkg-Ab3Cd9_f"), None);
    assert_eq!(retired_package_stem(".-Ab3Cd9Ef"), None);
}

// -- The sweep ---------------------------------------------------------------

/// The reported failure mode: the real `~/.npm/_npx/15c61037b1978c83` state,
/// where the live package and an abandoned retired sibling coexist. This is
/// the only state in which `rename(live -> retired)` can fail, so it is the one
/// the sweep must clear - and clearing it is all that is needed.
#[test]
fn test_repair_removes_a_retired_dir_while_the_live_package_exists() {
    let root = scratch("retired-and-live");
    let dir = install_dir(&root, "15c61037b1978c83");
    package(&dir, "chrome-devtools-mcp");
    let poison = retired(&dir, "chrome-devtools-mcp");

    let report = repair_npx_cache(&root.join("_npx"));

    assert_eq!(
        report,
        RepairReport {
            retired_dirs_removed: 1,
            skipped_in_flight: 0,
        }
    );
    assert!(!poison.exists());
    // The live package is the intact install: the rename that would have moved
    // it aside is exactly the one that failed. Deleting it would trade a
    // rename collision for a re-download.
    assert!(
        dir.join("node_modules/chrome-devtools-mcp/index.js")
            .exists()
    );
    // The install directory itself survives: npm owns the tree, and the next
    // launch reuses the same hash.
    assert!(dir.join("package.json").exists());
    assert!(dir.join("node_modules/.package-lock.json").exists());
}

/// `reify` renames the live package away *before* renaming the new copy in, so
/// a retired directory with no live package is a replacement in flight (or one
/// that died between its two renames, which npm heals by placing into an empty
/// path). Removing the directory it is about to restore would break its
/// rollback, so it is left alone - no clock involved.
#[test]
fn test_repair_leaves_a_retired_dir_alone_when_the_live_package_is_absent() {
    let root = scratch("in-flight");
    let dir = install_dir(&root, "15c61037b1978c83");
    let in_flight = retired(&dir, "chrome-devtools-mcp");

    let report = repair_npx_cache(&root.join("_npx"));

    assert_eq!(
        report,
        RepairReport {
            retired_dirs_removed: 0,
            skipped_in_flight: 1,
        }
    );
    assert!(in_flight.exists());
}

/// A healthy install - the normal case on every startup - must not be touched.
#[test]
fn test_repair_leaves_a_healthy_cache_untouched() {
    let root = scratch("healthy");
    let dir = install_dir(&root, "15c61037b1978c83");
    package(&dir, "chrome-devtools-mcp");
    package(&dir, "pdfcraft");
    std::fs::create_dir_all(dir.join("node_modules/.bin")).expect("mkdir .bin");

    let report = repair_npx_cache(&root.join("_npx"));

    assert_eq!(report, RepairReport::default());
    assert!(report.is_empty());
    assert!(
        dir.join("node_modules/chrome-devtools-mcp/index.js")
            .exists()
    );
    assert!(dir.join("node_modules/pdfcraft/index.js").exists());
    assert!(dir.join("node_modules/.bin").is_dir());
}

/// A scoped package is retired under its basename (`@scope/pkg` ->
/// `.pkg-<hash>`), so the live-package lookup has to find it under the scope
/// directory rather than at `node_modules/pkg` - otherwise a scoped install
/// would be misread as in-flight and never repaired.
#[test]
fn test_repair_removes_the_retired_dir_of_a_scoped_package() {
    let root = scratch("scoped");
    let dir = install_dir(&root, "55158e48eb5c59f7");
    scoped_package(&dir, "@modelcontextprotocol", "server-filesystem");
    let poison = retired(&dir, "server-filesystem");

    let report = repair_npx_cache(&root.join("_npx"));

    assert_eq!(report.retired_dirs_removed, 1);
    assert!(!poison.exists());
    assert!(
        dir.join("node_modules/@modelcontextprotocol/server-filesystem")
            .exists()
    );
}

/// Several install directories, several poisoned packages: every one is swept,
/// and each counts once. An untouched third install proves the sweep is
/// per-directory.
#[test]
fn test_repair_sweeps_every_install_directory() {
    let root = scratch("many-installs");
    let first = install_dir(&root, "15c61037b1978c83");
    package(&first, "chrome-devtools-mcp");
    retired(&first, "chrome-devtools-mcp");
    let second = install_dir(&root, "55158e48eb5c59f7");
    package(&second, "pdfcraft");
    retired(&second, "pdfcraft");
    let third = install_dir(&root, "999675247c823d79");
    package(&third, "wordcraft");

    let report = repair_npx_cache(&root.join("_npx"));

    assert_eq!(report.retired_dirs_removed, 2);
    assert_eq!(report.skipped_in_flight, 0);
    assert!(third.join("node_modules/wordcraft/index.js").exists());
}

/// An empty install directory - `npx` creates it before reifying and never
/// cleans it up - carries no retired sibling, so it is not evidence of an
/// interrupted run and stays put.
#[test]
fn test_repair_ignores_an_empty_install_directory() {
    let root = scratch("empty-install");
    let dir = root.join("_npx").join("3c2a9ea6c4b6e0a2");
    std::fs::create_dir_all(&dir).expect("mkdir install");

    let report = repair_npx_cache(&root.join("_npx"));

    assert_eq!(report, RepairReport::default());
    assert!(dir.is_dir());
}

/// A missing cache root is the first-run case, not an error: there is nothing
/// to sweep and no directory to create.
#[test]
fn test_repair_tolerates_a_missing_cache_root() {
    let root = scratch("no-cache").join("_npx");
    assert!(!root.exists());

    let report = repair_npx_cache(&root);

    assert_eq!(report, RepairReport::default());
    assert!(!root.exists(), "the sweep must not create the cache root");
}

/// A regular *file* whose name happens to match the retire pattern is not a
/// directory npm created, so it is left alone rather than treated as poison.
#[test]
fn test_repair_ignores_a_file_that_matches_the_retire_pattern() {
    let root = scratch("retire-shaped-file");
    let dir = install_dir(&root, "15c61037b1978c83");
    package(&dir, "chrome-devtools-mcp");
    let decoy = dir.join("node_modules/.pdfcraft-Ab3Cd9Ef");
    std::fs::write(&decoy, "not a directory\n").expect("write decoy");

    let report = repair_npx_cache(&root.join("_npx"));

    assert!(report.is_empty());
    assert!(decoy.is_file());
}

/// Two install directories that both hold the same package name are swept
/// independently: poisoning one says nothing about the other.
#[test]
fn test_repair_scopes_the_verdict_to_one_install_directory() {
    let root = scratch("independent-installs");
    let poisoned = install_dir(&root, "15c61037b1978c83");
    package(&poisoned, "pdfcraft");
    retired(&poisoned, "pdfcraft");
    let clean = install_dir(&root, "55158e48eb5c59f7");
    package(&clean, "pdfcraft");

    let report = repair_npx_cache(&root.join("_npx"));

    assert_eq!(report.retired_dirs_removed, 1);
    assert!(clean.join("node_modules/pdfcraft/index.js").exists());
}
