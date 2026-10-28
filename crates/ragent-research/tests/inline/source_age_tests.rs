//! Inline tests for the `--age` span parser ([`crate::source_age`]).

use chrono::{TimeZone, Utc};

use crate::source_age::{AgeSpan, AgeSpanError, AgeUnit, DAYS_PER_MONTH, DAYS_PER_YEAR};

/// Fixed "now" so spans and cut-offs are deterministic.
fn now() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2024, 6, 15, 12, 0, 0)
        .single()
        .expect("valid fixed timestamp")
}

#[test]
fn parses_days_months_and_years() {
    let days = AgeSpan::parse("30d").expect("valid");
    assert_eq!((days.amount(), days.unit()), (30, AgeUnit::Days));
    assert_eq!(days.days(), 30);

    let months = AgeSpan::parse("6m").expect("valid");
    assert_eq!((months.amount(), months.unit()), (6, AgeUnit::Months));
    assert_eq!(months.days(), 6 * DAYS_PER_MONTH);

    let years = AgeSpan::parse("2y").expect("valid");
    assert_eq!((years.amount(), years.unit()), (2, AgeUnit::Years));
    assert_eq!(years.days(), 2 * DAYS_PER_YEAR);
}

#[test]
fn unit_suffix_is_case_insensitive_and_whitespace_is_trimmed() {
    assert_eq!(AgeSpan::parse(" 7D ").expect("valid").days(), 7);
    assert_eq!(
        AgeSpan::parse("3M").expect("valid").days(),
        3 * DAYS_PER_MONTH
    );
    assert_eq!(AgeSpan::parse("1Y").expect("valid").days(), DAYS_PER_YEAR);
}

#[test]
fn display_round_trips_the_written_span() {
    assert_eq!(AgeSpan::parse("12d").expect("valid").to_string(), "12d");
    assert_eq!(AgeSpan::parse("4m").expect("valid").to_string(), "4m");
    assert_eq!(AgeSpan::parse("5y").expect("valid").to_string(), "5y");
}

#[test]
fn rejects_missing_or_unknown_units() {
    assert_eq!(AgeSpan::parse(""), Err(AgeSpanError::Empty));
    assert_eq!(AgeSpan::parse("   "), Err(AgeSpanError::Empty));
    assert!(matches!(
        AgeSpan::parse("30w"),
        Err(AgeSpanError::UnknownUnit { unit: 'w', .. })
    ));
    assert!(matches!(
        AgeSpan::parse("5"),
        Err(AgeSpanError::UnknownUnit { unit: '5', .. })
    ));
}

#[test]
fn rejects_missing_zero_and_oversized_magnitudes() {
    assert!(matches!(
        AgeSpan::parse("d"),
        Err(AgeSpanError::MissingMagnitude { .. })
    ));
    assert!(matches!(
        AgeSpan::parse("-3d"),
        Err(AgeSpanError::MissingMagnitude { .. })
    ));
    assert!(matches!(
        AgeSpan::parse("0d"),
        Err(AgeSpanError::ZeroSpan { .. })
    ));
    assert!(matches!(
        AgeSpan::parse("99999999999d"),
        Err(AgeSpanError::MagnitudeTooLarge { .. })
    ));
}

#[test]
fn oldest_allowed_is_now_minus_the_span() {
    let span = AgeSpan::parse("30d").expect("valid");
    assert_eq!(
        span.oldest_allowed(now()),
        Utc.with_ymd_and_hms(2024, 5, 16, 12, 0, 0)
            .single()
            .expect("valid")
    );
}

#[test]
fn admits_a_source_inside_the_span() {
    let span = AgeSpan::parse("30d").expect("valid");
    let inside = Utc
        .with_ymd_and_hms(2024, 6, 1, 0, 0, 0)
        .single()
        .expect("valid");
    assert!(span.admits(Some(inside), now()));
    assert_eq!(span.rejection_reason(Some(inside), now()), None);
}

#[test]
fn rejects_a_source_older_than_the_span() {
    let span = AgeSpan::parse("30d").expect("valid");
    let outside = Utc
        .with_ymd_and_hms(2024, 1, 1, 0, 0, 0)
        .single()
        .expect("valid");
    assert!(!span.admits(Some(outside), now()));
    let reason = span
        .rejection_reason(Some(outside), now())
        .expect("rejected");
    assert!(reason.contains("2024-01-01"), "got: {reason}");
    assert!(reason.contains("--age 30d"), "got: {reason}");
}

#[test]
fn rejects_an_undated_source() {
    let span = AgeSpan::parse("1y").expect("valid");
    assert!(!span.admits(None, now()));
    let reason = span.rejection_reason(None, now()).expect("rejected");
    assert!(reason.contains("no publication date"), "got: {reason}");
    assert!(reason.contains("--age 1y"), "got: {reason}");
}
