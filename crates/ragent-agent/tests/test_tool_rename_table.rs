//! Invariant test for the FR-023 tool-rename table
//! (`crates/ragent-types/src/tool_rename.rs`, re-exported as
//! `ragent_agent::tool::rename`, spec `uniformtools`).
//!
//! The table moved to `ragent-types` in T-042 so the config loader can read it
//! without `ragent-config` depending on `ragent-agent`; the re-export keeps every
//! path in this file unchanged.
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
//! The two registry-facing cases (`..._every_before_is_retired...` and
//! `..._every_after_is_registered...`) describe the **post-rename** registry:
//! T-030 landed the table ahead of the rename, and T-031 flipped these two in the
//! same change that renamed the tools, which is FR-034's atomic-landing rule. A
//! tree that resolves a `before` spelling, or fails to resolve an `after` one,
//! fails here. The remaining cases are registry-independent.

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
            "FR-023(a): `before` {:?} is still a name the default registry registers, \
             so the rename kept a second spelling for it",
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
            "FR-023(b): `after` {:?} is not a name the default registry registers, \
             so the rename did not land",
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
