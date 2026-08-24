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

/// Extract all FR-NNN IDs (formatted as `FR-001`, `FR-002`, ...) from a text.
///
/// Returns a sorted, deduplicated list.
///
/// # Examples
///
/// ```
/// use ragent_specs::id_scanner::extract_fr_ids;
///
/// assert_eq!(extract_fr_ids("### FR-019\n...\n### FR-001\n"), vec!["FR-001", "FR-019"]);
/// ```
#[must_use]
pub fn extract_fr_ids(text: &str) -> Vec<String> {
    extract_ids(text, &RE_FR_ID, "FR-")
}

/// Extract all NFR-NNN IDs (formatted as `NFR-001`, `NFR-002`, ...) from a text.
///
/// Returns a sorted, deduplicated list.
///
/// # Examples
///
/// ```
/// use ragent_specs::id_scanner::extract_nfr_ids;
///
/// assert_eq!(extract_nfr_ids("### NFR-005\n...\n### NFR-001\n"), vec!["NFR-001", "NFR-005"]);
/// ```
#[must_use]
pub fn extract_nfr_ids(text: &str) -> Vec<String> {
    extract_ids(text, &RE_NFR_ID, "NFR-")
}

/// Extract all T-NNN IDs (formatted as `T-001`, `T-002`, ...) from a text.
///
/// Returns a sorted, deduplicated list.
///
/// # Examples
///
/// ```
/// use ragent_specs::id_scanner::extract_task_ids;
///
/// assert_eq!(extract_task_ids("| T-040 |...| T-001 |"), vec!["T-001", "T-040"]);
/// ```
#[must_use]
pub fn extract_task_ids(text: &str) -> Vec<String> {
    extract_ids(text, &RE_TASK_ID, "T-")
}

/// Shared helper: extract `<prefix>NNN` IDs matched by `re`, sorted and deduplicated.
fn extract_ids(text: &str, re: &regex::Regex, prefix: &str) -> Vec<String> {
    let mut ids: Vec<String> = re
        .captures_iter(text)
        .filter_map(|c| c[1].parse::<u32>().ok())
        .map(|n| format!("{prefix}{n:03}"))
        .collect();
    ids.sort();
    ids.dedup();
    ids
}

// ── Tests ────────────────────────────────────────────────────────────────
