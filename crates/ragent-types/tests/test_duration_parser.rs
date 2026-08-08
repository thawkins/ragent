//! Unit tests for the cron duration parser (`parse_duration`).
//!
//! Covers FR-014 (aliased unit acceptance), FR-018 (zero rejected),
//! and FR-019 (unknown unit lists supported units), plus valid parsing,
//! negative rejection, and edge cases.

use ragent_types::{DurationParseError, parse_duration};

// ── FR-014: Valid canonical units ─────────────────────────────────

#[test]
fn test_duration_canonical_units() {
    assert_eq!(parse_duration("1m").unwrap(), 60);
    assert_eq!(parse_duration("1h").unwrap(), 3_600);
    assert_eq!(parse_duration("1d").unwrap(), 86_400);
    assert_eq!(parse_duration("1w").unwrap(), 604_800);
    assert_eq!(parse_duration("1mo").unwrap(), 2_592_000);
}

// ── FR-014: Plural and long-form unit aliases ────────────────────

#[test]
fn test_duration_aliased_units_min() {
    assert_eq!(parse_duration("1min").unwrap(), 60);
    assert_eq!(parse_duration("1mins").unwrap(), 60);
}

#[test]
fn test_duration_aliased_units_hour() {
    assert_eq!(parse_duration("1hr").unwrap(), 3_600);
    assert_eq!(parse_duration("1hrs").unwrap(), 3_600);
}

#[test]
fn test_duration_aliased_units_day() {
    assert_eq!(parse_duration("1day").unwrap(), 86_400);
    assert_eq!(parse_duration("1days").unwrap(), 86_400);
}

#[test]
fn test_duration_aliased_units_week() {
    assert_eq!(parse_duration("1wk").unwrap(), 604_800);
    assert_eq!(parse_duration("1wks").unwrap(), 604_800);
}

#[test]
fn test_duration_aliased_units_month() {
    assert_eq!(parse_duration("1month").unwrap(), 2_592_000);
    assert_eq!(parse_duration("1months").unwrap(), 2_592_000);
}

// ── Valid: multi-digit values ──────────────────────────────���─────

#[test]
fn test_duration_multi_digit_values() {
    assert_eq!(parse_duration("30m").unwrap(), 1_800);
    assert_eq!(parse_duration("2h").unwrap(), 7_200);
    assert_eq!(parse_duration("7d").unwrap(), 604_800);
    assert_eq!(parse_duration("12mo").unwrap(), 31_104_000);
    assert_eq!(parse_duration("100m").unwrap(), 6_000);
}

// ── Valid: space between number and unit ──────────────────────────

#[test]
fn test_duration_with_space() {
    assert_eq!(parse_duration("30 m").unwrap(), 1_800);
    assert_eq!(parse_duration("2  h").unwrap(), 7_200);
    assert_eq!(parse_duration("1  d").unwrap(), 86_400);
}

// ── Valid: case-insensitive unit ──────────────────────────────────

#[test]
fn test_duration_case_insensitive() {
    assert_eq!(parse_duration("30M").unwrap(), 1_800);
    assert_eq!(parse_duration("2H").unwrap(), 7_200);
    assert_eq!(parse_duration("1D").unwrap(), 86_400);
    assert_eq!(parse_duration("1MO").unwrap(), 2_592_000);
    assert_eq!(parse_duration("1W").unwrap(), 604_800);
}

// ── Valid: leading/trailing whitespace trimmed ───────────────────

#[test]
fn test_duration_whitespace_trimmed() {
    assert_eq!(parse_duration("  30m  ").unwrap(), 1_800);
    assert_eq!(parse_duration("\t2h\t").unwrap(), 7_200);
    assert_eq!(parse_duration(" 1d ").unwrap(), 86_400);
}

// ── FR-018: Zero duration rejected ────────────────────────────────

#[test]
fn test_duration_zero_rejected() {
    assert!(matches!(
        parse_duration("0m"),
        Err(DurationParseError::Zero)
    ));
    assert!(matches!(
        parse_duration("0h"),
        Err(DurationParseError::Zero)
    ));
    assert!(matches!(
        parse_duration("0d"),
        Err(DurationParseError::Zero)
    ));
    assert!(matches!(
        parse_duration("0w"),
        Err(DurationParseError::Zero)
    ));
    assert!(matches!(
        parse_duration("0mo"),
        Err(DurationParseError::Zero)
    ));
}

#[test]
fn test_duration_zero_with_alias_rejected() {
    assert!(matches!(
        parse_duration("0min"),
        Err(DurationParseError::Zero)
    ));
    assert!(matches!(
        parse_duration("0hrs"),
        Err(DurationParseError::Zero)
    ));
    assert!(matches!(
        parse_duration("0days"),
        Err(DurationParseError::Zero)
    ));
}

// ── Negative duration rejected ────────────────────────────────────

#[test]
fn test_duration_negative_rejected() {
    assert!(matches!(
        parse_duration("-5m"),
        Err(DurationParseError::Negative(-5))
    ));
    assert!(matches!(
        parse_duration("-1h"),
        Err(DurationParseError::Negative(-1))
    ));
    assert!(matches!(
        parse_duration("-30d"),
        Err(DurationParseError::Negative(-30))
    ));
    assert!(matches!(
        parse_duration("-2w"),
        Err(DurationParseError::Negative(-2))
    ));
    assert!(matches!(
        parse_duration("-1mo"),
        Err(DurationParseError::Negative(-1))
    ));
}

// ── FR-019: Unknown / bad unit rejected ───────────────────────────

#[test]
fn test_duration_unknown_unit_seconds() {
    // Seconds are explicitly not a supported unit
    assert!(matches!(
        parse_duration("5s"),
        Err(DurationParseError::UnknownUnit(_, _))
    ));
}

#[test]
fn test_duration_unknown_unit_years() {
    // Years are explicitly not a supported unit
    assert!(matches!(
        parse_duration("3y"),
        Err(DurationParseError::UnknownUnit(_, _))
    ));
}

#[test]
fn test_duration_unknown_unit_nonsense() {
    assert!(matches!(
        parse_duration("5xyz"),
        Err(DurationParseError::UnknownUnit(_, _))
    ));
}

// ── FR-019: Error message lists all supported units ──────────────

#[test]
fn test_duration_unknown_unit_error_lists_supported() {
    let err = parse_duration("5s").unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("minutes"),
        "error should mention minutes: {msg}"
    );
    assert!(msg.contains("hours"), "error should mention hours: {msg}");
    assert!(msg.contains("days"), "error should mention days: {msg}");
    assert!(msg.contains("weeks"), "error should mention weeks: {msg}");
    assert!(msg.contains("months"), "error should mention months: {msg}");
}

#[test]
fn test_duration_unknown_unit_error_lists_aliases() {
    let err = parse_duration("5s").unwrap_err();
    let msg = err.to_string();
    // The canonical short forms should appear in the alias list
    assert!(msg.contains("m"), "error should list 'm': {msg}");
    assert!(msg.contains("h"), "error should list 'h': {msg}");
    assert!(msg.contains("d"), "error should list 'd': {msg}");
    assert!(msg.contains("w"), "error should list 'w': {msg}");
    assert!(msg.contains("mo"), "error should list 'mo': {msg}");
}

// ── Edge cases: missing unit ─────────────────────────────────────

#[test]
fn test_duration_missing_unit() {
    assert!(matches!(
        parse_duration("30"),
        Err(DurationParseError::MissingUnit(_))
    ));
    assert!(matches!(
        parse_duration("42 "),
        Err(DurationParseError::MissingUnit(_))
    ));
}

// ── Edge cases: empty input ───────────────────────────────────────

#[test]
fn test_duration_empty() {
    assert!(matches!(parse_duration(""), Err(DurationParseError::Empty)));
    assert!(matches!(
        parse_duration("   "),
        Err(DurationParseError::Empty)
    ));
    assert!(matches!(
        parse_duration("\t"),
        Err(DurationParseError::Empty)
    ));
}

// ── Edge cases: no numeric prefix ─────────────────────────────────

#[test]
fn test_duration_no_number() {
    assert!(matches!(
        parse_duration("abc"),
        Err(DurationParseError::InvalidNumber(_))
    ));
    assert!(matches!(
        parse_duration("m"),
        Err(DurationParseError::InvalidNumber(_))
    ));
    assert!(matches!(
        parse_duration("mins"),
        Err(DurationParseError::InvalidNumber(_))
    ));
}
