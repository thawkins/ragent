//! T-008 regression tests: FR-013 output size cap with an explicit truncation
//! marker (spec `prompts`).
//!
//! FR-013: WHILE the assembled render exceeds the 100,000-character cap, the
//! renderer SHALL truncate the displayed text at the cap and SHALL append an
//! explicit marker so the user is never shown a silent cut-off, and the header
//! summary SHALL always precede the body.
//!
//! The pure functions (`apply_size_cap`, `PROMPT_REPORT_MAX_CHARS`) are
//! asserted directly; the marker wording comes from
//! `crates/ragent-tui/src/app/prompt.rs`.

use ragent_tui::app::prompt::{PROMPT_REPORT_MAX_CHARS, apply_size_cap};

#[test]
fn test_size_cap_under_cap_passes_through_unchanged() {
    let report = "From: /prompt\n\nshort report body";
    let out = apply_size_cap(report);
    assert_eq!(out, report, "under-cap report must pass through unchanged");
}

#[test]
fn test_size_cap_at_exactly_max_chars_passes_through() {
    // Exactly at the cap (inclusive): no truncation, no marker.
    let report = "x".repeat(PROMPT_REPORT_MAX_CHARS);
    let out = apply_size_cap(&report);
    assert_eq!(out, report);
    assert!(
        !out.contains("truncated"),
        "exact-size report must not be marked"
    );
}

#[test]
fn test_size_cap_over_max_truncates_with_marker() {
    let total = PROMPT_REPORT_MAX_CHARS + 1234;
    let report = "y".repeat(total);
    let out = apply_size_cap(&report);
    // Body is cut at the cap (exactly `cap` body chars survive)...
    assert_eq!(
        out.chars().filter(|c| *c == 'y').count(),
        PROMPT_REPORT_MAX_CHARS,
        "exactly the cap's worth of body characters must survive"
    );
    // ...plus the explicit marker stating N of M characters (the marker adds
    // its own text, so total length is cap + marker, not the cap alone).
    let flat = out.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        flat.contains("[truncated:"),
        "marker must announce the truncation, got tail: {:?}",
        &out[out.len().saturating_sub(400)..]
    );
    assert!(
        flat.contains(&format!("showing {PROMPT_REPORT_MAX_CHARS} of {total}")),
        "marker must state the shown/total counts"
    );
    // The header (which precedes the body) is still present: the marker is
    // appended after a cut of a real report-shaped string.
    let headered = apply_size_cap(&format!(
        "From: /prompt\n\n**Prompt report** - agent: `a` - source: built-in - mode: primary - tools: 1 - size: {total} chars\n{}",
        report
    ));
    assert!(
        headered.starts_with("From: /prompt"),
        "header summary must precede the body even under truncation"
    );
    assert!(headered.contains("[truncated:"));
}

#[test]
fn test_size_cap_marker_is_multibyte_safe() {
    // The cap counts characters, not bytes: a multi-byte body must cut on a
    // char boundary and never panic.
    let body = "é".repeat(PROMPT_REPORT_MAX_CHARS + 10); // 2 bytes per char
    let out = apply_size_cap(&body);
    // The cut lands on a char boundary: exactly the cap's worth of `é`.
    assert_eq!(
        out.chars().filter(|c| *c == 'é').count(),
        PROMPT_REPORT_MAX_CHARS,
        "multi-byte bodies must cut on a character boundary"
    );
    assert!(out.contains("[truncated:"));
}
