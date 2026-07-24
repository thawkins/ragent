//! ID scanner: find the highest-numbered FR-NNN, NFR-NNN, and T-NNN IDs
//! in raw markdown strings.
//!
//! Used by the `/spec add` command to determine the next available
//! requirement and task IDs when incrementally updating a spec.

use regex::Regex;
use std::sync::LazyLock;

// ── Regex patterns ────────────────────────────────────────────────────────

static RE_FR_ID: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\bFR-(\d+)\b").expect("FR-ID regex should compile"));

static RE_NFR_ID: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\bNFR-(\d+)\b").expect("NFR-ID regex should compile"));

static RE_TASK_ID: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\bT-(\d+)\b").expect("T-ID regex should compile"));

// ── Public API ───────────────────────────────────────────────────────────

/// Find the highest numeric ID for a given prefix pattern in a markdown string.
///
/// Returns `0` when no matching IDs are found.
///
/// # Examples
///
/// ```
/// use ragent_specs::id_scanner::highest_id;
///
/// assert_eq!(highest_id("FR-001, FR-003, FR-007", "FR"), 7);
/// assert_eq!(highest_id("no IDs here", "FR"), 0);
/// ```
#[must_use]
pub fn highest_id(markdown: &str, prefix: &str) -> u32 {
    let re: &Regex = match prefix.to_uppercase().as_str() {
        "FR" => &RE_FR_ID,
        "NFR" => &RE_NFR_ID,
        "T" => &RE_TASK_ID,
        _ => return 0,
    };
    re.captures_iter(markdown)
        .filter_map(|cap| cap[1].parse::<u32>().ok())
        .max()
        .unwrap_or(0)
}

/// Find the highest `FR-NNN` ID in a spec markdown string.
///
/// Returns `0` when no FR IDs are found.
///
/// # Examples
///
/// ```
/// use ragent_specs::id_scanner::highest_fr;
///
/// assert_eq!(highest_fr("FR-001 through FR-012"), 12);
/// assert_eq!(highest_fr("FR-1, FR-01, FR-001"), 1);
/// ```
#[must_use]
pub fn highest_fr(spec_md: &str) -> u32 {
    highest_id(spec_md, "FR")
}

/// Find the highest `NFR-NNN` ID in a spec markdown string.
///
/// Returns `0` when no NFR IDs are found.
///
/// # Examples
///
/// ```
/// use ragent_specs::id_scanner::highest_nfr;
///
/// assert_eq!(highest_nfr("NFR-001, NFR-002"), 2);
/// assert_eq!(highest_nfr("no non-functional requirements"), 0);
/// ```
#[must_use]
pub fn highest_nfr(spec_md: &str) -> u32 {
    highest_id(spec_md, "NFR")
}

/// Find the highest `T-NNN` ID in a plan markdown string.
///
/// Returns `0` when no T IDs are found.
///
/// # Examples
///
/// ```
/// use ragent_specs::id_scanner::highest_task;
///
/// assert_eq!(highest_task("T-001, T-010"), 10);
/// assert_eq!(highest_task("no tasks"), 0);
/// ```
#[must_use]
pub fn highest_task(plan_md: &str) -> u32 {
    highest_id(plan_md, "T")
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

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
}
