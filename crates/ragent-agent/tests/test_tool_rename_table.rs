//! Invariant test for the FR-023 tool-rename table
//! (`crates/ragent-types/src/tool_rename.rs`, re-exported as
//! `ragent_agent::tool::rename`, spec `uniformtools`).
//!
//! The table itself lives in `ragent-types` so the config loader can read it
//! without `ragent-config` depending on `ragent-agent`; the re-export in
//! `crates/ragent-agent/src/tool/rename.rs` keeps every path in this file
//! unchanged.
//!
//! Asserts exactly what FR-023 requires of the table, and nothing about the
//! workbook: (a) every `before` is a name the default registry registers,
//! (b) every `after` is a valid ASCII identifier the default registry does not
//! register, (c) the `after` column is unique, and (d) the `before` column is
//! unique. Two further checks keep the table honest as a table rather than a
//! set: it is ascending by `after`, and it holds the exact thirteen pairs FR-023
//! pins.
//!
//! This test deliberately does not open `tools.xlsx`.
//! `crates/ragent-tools-core/src/xlsx.rs` is a writer only; adding a reader to
//! compare the table against the workbook in a test would add a dependency and
//! breach NFR-002. Cross-checking the table against the workbook is the manual
//! step TC-019 of `specs/uniformtools/TESTPLAN.md` records.
//!
//! The two registry-facing cases describe the registry **after the rename
//! landed** (T-031), which is FR-034's atomic-landing rule: every `before` is
//! retired and every `after` is registered. Before the rename the two
//! assertions read the other way round; they were flipped by the same change
//! that renamed the tools, so a tree that resolves both spellings or neither
//! fails here. A `before` that is still registered, or an `after` that is not,
//! is the half-renamed state FR-034 exists to refuse. The remaining cases are
//! registry-independent.

use std::collections::HashSet;

use ragent_agent::tool::create_default_registry;
use ragent_agent::tool::rename::{Rename, TOOL_RENAMES};

/// FR-023's `after` spellings, as the specification pins them. A rename that
/// was added, dropped or respelt fails here rather than in a downstream task.
const EXPECTED_AFTERS: [&str; 13] = [
    "AskUserQuestion",
    "Bash",
    "Edit",
    "EnterPlanMode",
    "ExitPlanMode",
    "Glob",
    "Grep",
    "Skill",
    "TaskCreate",
    "TaskGet",
    "TaskList",
    "TaskUpdate",
    "Write",
];

/// Returns `true` when `name` matches `^[A-Za-z][A-Za-z0-9_]*$`.
fn is_valid_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    first.is_ascii_alphabetic() && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

#[test]
fn test_tool_rename_table_holds_exactly_the_thirteen_pinned_pairs() {
    let actual: Vec<&str> = TOOL_RENAMES.iter().map(|pair| pair.after).collect();
    assert_eq!(
        actual,
        EXPECTED_AFTERS.to_vec(),
        "the FR-023 table must hold exactly the 13 pinned pairs, ascending by `after`"
    );
}

#[test]
fn test_tool_rename_table_is_ascending_by_after() {
    for window in TOOL_RENAMES.windows(2) {
        assert!(
            window[0].after < window[1].after,
            "the FR-023 table must be ascending by `after`, but {:?} precedes {:?}",
            window[0].after,
            window[1].after
        );
    }
}

#[test]
fn test_tool_rename_every_before_is_retired_by_the_default_registry() {
    let registry = create_default_registry();
    let registered: HashSet<String> = registry.list().into_iter().collect();
    for pair in &TOOL_RENAMES {
        assert!(
            !registered.contains(pair.before),
            "FR-023(a) after the rename: `before` {:?} is still a name the default \
             registry registers, so the retired spelling survives (FR-026)",
            pair.before
        );
    }
}

#[test]
fn test_tool_rename_every_after_is_registered_by_the_default_registry() {
    let registry = create_default_registry();
    let registered: HashSet<String> = registry.list().into_iter().collect();
    for pair in &TOOL_RENAMES {
        assert!(
            registered.contains(pair.after),
            "FR-023(b) after the rename: `after` {:?} is not a name the default \
             registry registers, so the adopted spelling is unreachable \
             (FR-030 - the surface size must not change)",
            pair.after
        );
    }
}

#[test]
fn test_tool_rename_every_after_is_a_valid_identifier() {
    for pair in &TOOL_RENAMES {
        assert!(
            is_valid_identifier(pair.after),
            "FR-023(b): `after` {:?} is not an ASCII identifier",
            pair.after
        );
    }
}

#[test]
fn test_tool_rename_after_column_is_unique() {
    let mut seen = HashSet::new();
    for pair in &TOOL_RENAMES {
        assert!(
            seen.insert(pair.after),
            "FR-023(c): the `after` column repeats {:?}",
            pair.after
        );
    }
}

#[test]
fn test_tool_rename_before_column_is_unique() {
    let mut seen = HashSet::new();
    for pair in &TOOL_RENAMES {
        assert!(
            seen.insert(pair.before),
            "FR-023(d): the `before` column repeats {:?}",
            pair.before
        );
    }
}

#[test]
fn test_tool_rename_lookup_helpers_agree_with_the_table() {
    assert_eq!(Rename::after_of("bash"), Some("Bash"));
    assert_eq!(Rename::after_of("read"), None);
    assert_eq!(Rename::before_of("Bash"), Some("bash"));
    assert_eq!(Rename::before_of("Read"), None);
    assert!(Rename::is_retired("ask_user"));
    assert!(!Rename::is_retired("AskUserQuestion"));

    for pair in &TOOL_RENAMES {
        assert_eq!(Rename::after_of(pair.before), Some(pair.after));
        assert_eq!(Rename::before_of(pair.after), Some(pair.before));
        assert!(Rename::is_retired(pair.before));
        assert!(!Rename::is_retired(pair.after));
    }
}

/// FR-023's final clause: the invariant test **shall not** open the workbook.
///
/// It is pinned rather than left to review because the natural next step for a
/// reader of this file is "check the table against `tools.xlsx`", and the only
/// way to do that is an XLSX *reader*. `crates/ragent-tools-core/src/xlsx.rs`
/// is a writer (`write_xlsx`, backed by `rust_xlsxwriter`), so a reader means a
/// new external crate - `calamine`, which the workspace already carries for
/// `ragent-bench` - and that breaches NFR-002. Cross-checking the table against
/// the workbook is TC-019's manual step instead.
///
/// Two independent assertions, so neither half can rot alone: this crate gains
/// no workbook reader, and nothing in the agent crate or the table module opens
/// a workbook. (This guard file is skipped by the scan: it names the openers in
/// order to forbid them.)
#[test]
fn test_tool_rename_invariant_test_does_not_open_the_workbook() {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace = manifest_dir
        .parent()
        .and_then(|crates| crates.parent())
        .expect("crates/<name> sits two levels below the workspace root");

    // A workbook reader in this crate's dependency graph is the capability the
    // clause forbids, whether or not anything calls it yet.
    let manifest = std::fs::read_to_string(manifest_dir.join("Cargo.toml"))
        .expect("ragent-agent's Cargo.toml must be readable");
    for reader in ["calamine", "umya-spreadsheet", "simple_excel_writer"] {
        assert!(
            !manifest.contains(reader),
            "FR-023: `ragent-agent` must not depend on the XLSX reader `{reader}` - \
             the rename table's invariant test must not open `tools.xlsx` (NFR-002)"
        );
    }

    // No source reaches for a workbook either. `tools.xlsx` is allowed in prose
    // (both trees document *why* they do not open it); a reading or writing call
    // is not.
    let mut sources = Vec::new();
    for sub in ["src", "tests"] {
        collect_rust_sources(&manifest_dir.join(sub), &mut sources);
    }
    sources.push(workspace.join("crates/ragent-types/src/tool_rename.rs"));

    for path in sources {
        if path
            .file_name()
            .is_some_and(|n| n == "test_tool_rename_table.rs")
        {
            continue;
        }
        let Ok(source) = std::fs::read_to_string(&path) else {
            continue;
        };
        for opener in ["write_xlsx(", "read_xlsx(", "Workbook::open", "calamine::"] {
            assert!(
                !source.contains(opener),
                "FR-023: {} must not touch a workbook, but it names `{opener}`",
                path.display()
            );
        }
    }
}

/// Collects every `.rs` file below `dir`, recursively.
fn collect_rust_sources(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rust_sources(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}
