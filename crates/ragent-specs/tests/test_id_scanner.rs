use ragent_specs::id_scanner::{highest_fr, highest_id, highest_nfr, highest_task};

/// External tests for `tests` from `crates/ragent-specs/src/id_scanner.rs`
///
/// Relocated from the inline `#[cfg(test)]` module.

#[test]
fn test_highest_fr_standard_numbering() {
    let md = "FR-001, FR-002, FR-003";
    assert_eq!(highest_fr(md), 3);
}

#[test]
fn test_highest_fr_non_contiguous() {
    let md = "FR-001, FR-007, FR-012";
    assert_eq!(highest_fr(md), 12);
}

#[test]
fn test_highest_fr_zero_padded_vs_non_padded() {
    // All these represent ID 1
    let md = "FR-1, FR-01, FR-001";
    assert_eq!(highest_fr(md), 1);
}

#[test]
fn test_highest_fr_empty() {
    assert_eq!(highest_fr(""), 0);
    assert_eq!(highest_fr("no requirements here"), 0);
}

#[test]
fn test_highest_fr_case_insensitive() {
    let md = "fr-005, FR-010";
    assert_eq!(highest_fr(md), 10);
}

#[test]
fn test_highest_nfr_standard() {
    let md = "NFR-001, NFR-002";
    assert_eq!(highest_nfr(md), 2);
}

#[test]
fn test_highest_nfr_empty() {
    assert_eq!(highest_nfr("no NFRs"), 0);
}

#[test]
fn test_mixed_prefixes() {
    // FR and NFR in the same file — each scanner only finds its own prefix
    let md = "FR-003 and NFR-002";
    assert_eq!(highest_fr(md), 3);
    assert_eq!(highest_nfr(md), 2);
}

#[test]
fn test_highest_task_standard() {
    let md = "| T-001 | Task 1 |\n| T-010 | Task 10 |";
    assert_eq!(highest_task(md), 10);
}

#[test]
fn test_highest_task_empty() {
    assert_eq!(highest_task("no tasks"), 0);
}

#[test]
fn test_highest_id_unknown_prefix() {
    assert_eq!(highest_id("some text", "XYZ"), 0);
}

#[test]
fn test_highest_fr_in_full_spec() {
    let spec = r"---
status: draft
---
# My Spec

## Requirements

### Authentication

**FR-001** (Ubiquitous) The system shall authenticate users.

**FR-002** (Event-driven) When a user logs out, the system shall clear the session.

### Non-Functional Requirements

**NFR-001** (Ubiquitous) The system shall respond within 200ms.
";
    assert_eq!(highest_fr(spec), 2);
    assert_eq!(highest_nfr(spec), 1);
    assert_eq!(highest_task(spec), 0);
}

#[test]
fn test_highest_task_in_full_plan() {
    let plan = r"# Plan

## Tasks

| ID | Title | Requirement | Effort | Priority | Dependencies |
|---|---|---|---|---|---|
| T-001 | Define types | FR-003 | S | Critical | — |
| T-002 | Build parser | FR-004 | M | High | T-001 |
| T-010 | Add tests | FR-005 | M | High | T-002 |
";
    assert_eq!(highest_task(plan), 10);
}

#[test]
fn test_no_false_positives() {
    // "FR-123" inside a word like "REFR-123X" should not match due to \b
    // Actually \b matches at word boundaries, so "FR-123" in normal text
    // will match. Let's verify it doesn't match things like "XFR-123".
    let md = "XFR-123 should not match";
    assert_eq!(highest_fr(md), 0);
}
