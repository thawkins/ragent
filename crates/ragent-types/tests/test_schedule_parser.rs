//! Unit tests for the schedule parser covering all three forms (FR-008, FR-009).
//!
//! Tests the `parse_schedule` function for:
//! - `at <timestamp>` — one-shot form
//! - `every <duration>` — repeat-from-now form (FR-008)
//! - `from <timestamp> every <duration>` — repeat-from-start form (FR-009)
//!
//! Also covers error cases and edge conditions.

use chrono::{DateTime, Duration, NaiveDateTime, Utc};

use ragent_types::{
    CronForm, CronSchedule, DurationParseError, ScheduleParseError, parse_schedule,
};

// ─────────────────────────────────────────────────────────────────
// Form 1: `at <timestamp>` — OneShot
// ─────────────────────────────────────────────────────────────────

#[test]
fn test_at_one_shot_basic() {
    let now = Utc::now();
    let parsed = parse_schedule("at 2025-01-15T09:00:00Z", now).unwrap();
    assert_eq!(parsed.schedule.form, CronForm::OneShot);
    assert!(parsed.schedule.is_one_shot());
    assert!(!parsed.schedule.is_repeating());
}

#[test]
fn test_at_next_due_equals_start_at() {
    let now = Utc::now();
    let parsed = parse_schedule("at 2025-01-15T09:00:00Z", now).unwrap();
    assert_eq!(parsed.next_due, parsed.schedule.start_at.unwrap());
}

#[test]
fn test_at_start_at_is_parsed_timestamp() {
    let now = Utc::now();
    let parsed = parse_schedule("at 2025-06-30T18:45:00Z", now).unwrap();
    let expected = DateTime::parse_from_rfc3339("2025-06-30T18:45:00Z")
        .unwrap()
        .with_timezone(&Utc);
    assert_eq!(parsed.schedule.start_at, Some(expected));
}

#[test]
fn test_at_duration_secs_is_none() {
    let now = Utc::now();
    let parsed = parse_schedule("at 2025-01-15T09:00:00Z", now).unwrap();
    assert_eq!(parsed.schedule.duration_secs, None);
}

#[test]
fn test_at_timestamp_with_offset_converted_to_utc() {
    let now = Utc::now();
    let parsed = parse_schedule("at 2025-01-15T09:00:00+02:00", now).unwrap();
    let expected = DateTime::parse_from_rfc3339("2025-01-15T09:00:00+02:00")
        .unwrap()
        .with_timezone(&Utc);
    assert_eq!(parsed.schedule.start_at, Some(expected));
    assert_eq!(parsed.next_due, expected);
}

#[test]
fn test_at_naive_timestamp_assumed_utc() {
    let now = Utc::now();
    let parsed = parse_schedule("at 2025-01-15T09:00:00", now).unwrap();
    let expected = NaiveDateTime::parse_from_str("2025-01-15T09:00:00", "%Y-%m-%dT%H:%M:%S")
        .unwrap()
        .and_utc();
    assert_eq!(parsed.schedule.start_at, Some(expected));
}

#[test]
fn test_at_case_insensitive_keyword() {
    let now = Utc::now();
    let parsed = parse_schedule("AT 2025-01-15T09:00:00Z", now).unwrap();
    assert_eq!(parsed.schedule.form, CronForm::OneShot);
}

#[test]
fn test_at_missing_timestamp_error() {
    let now = Utc::now();
    let result = parse_schedule("at", now);
    assert!(matches!(
        result,
        Err(ScheduleParseError::InvalidTimestamp(_, _))
    ));
}

#[test]
fn test_at_invalid_timestamp_error() {
    let now = Utc::now();
    let result = parse_schedule("at not-a-date", now);
    assert!(matches!(
        result,
        Err(ScheduleParseError::InvalidTimestamp(_, _))
    ));
}

#[test]
fn test_at_whitespace_only_timestamp_error() {
    let now = Utc::now();
    let result = parse_schedule("at   ", now);
    assert!(matches!(
        result,
        Err(ScheduleParseError::InvalidTimestamp(_, _))
    ));
}

// ─────────────────────────────────────────────────────────────────
// Form 2: `every <duration>` — RepeatNow (FR-008)
// ─────────────────────────────────────────────────────────────────

#[test]
fn test_every_repeat_now_basic() {
    let now = Utc::now();
    let parsed = parse_schedule("every 30m", now).unwrap();
    assert_eq!(parsed.schedule.form, CronForm::RepeatNow);
    assert!(parsed.schedule.is_repeating());
    assert!(!parsed.schedule.is_one_shot());
}

#[test]
fn test_every_start_at_is_none() {
    let now = Utc::now();
    let parsed = parse_schedule("every 30m", now).unwrap();
    assert_eq!(parsed.schedule.start_at, None);
}

#[test]
fn test_every_duration_secs_stored() {
    let now = Utc::now();
    let parsed = parse_schedule("every 30m", now).unwrap();
    assert_eq!(parsed.schedule.duration_secs, Some(1800));
}

#[test]
fn test_every_next_due_is_now_plus_duration() {
    let now = Utc::now();
    let parsed = parse_schedule("every 30m", now).unwrap();
    let expected = now + Duration::seconds(1800);
    assert_eq!(parsed.next_due, expected);
}

#[test]
fn test_every_next_due_one_hour() {
    let now = Utc::now();
    let parsed = parse_schedule("every 1h", now).unwrap();
    assert_eq!(parsed.schedule.duration_secs, Some(3600));
    assert_eq!(parsed.next_due, now + Duration::seconds(3600));
}

#[test]
fn test_every_next_due_one_day() {
    let now = Utc::now();
    let parsed = parse_schedule("every 1d", now).unwrap();
    assert_eq!(parsed.schedule.duration_secs, Some(86_400));
    assert_eq!(parsed.next_due, now + Duration::seconds(86_400));
}

#[test]
fn test_every_next_due_one_week() {
    let now = Utc::now();
    let parsed = parse_schedule("every 1w", now).unwrap();
    assert_eq!(parsed.schedule.duration_secs, Some(604_800));
    assert_eq!(parsed.next_due, now + Duration::seconds(604_800));
}

#[test]
fn test_every_next_due_one_month() {
    let now = Utc::now();
    let parsed = parse_schedule("every 1mo", now).unwrap();
    assert_eq!(parsed.schedule.duration_secs, Some(2_592_000));
    assert_eq!(parsed.next_due, now + Duration::seconds(2_592_000));
}

#[test]
fn test_every_duration_alias_mins() {
    let now = Utc::now();
    let parsed = parse_schedule("every 15mins", now).unwrap();
    assert_eq!(parsed.schedule.duration_secs, Some(900));
    assert_eq!(parsed.next_due, now + Duration::seconds(900));
}

#[test]
fn test_every_duration_alias_hrs() {
    let now = Utc::now();
    let parsed = parse_schedule("every 3hrs", now).unwrap();
    assert_eq!(parsed.schedule.duration_secs, Some(10_800));
    assert_eq!(parsed.next_due, now + Duration::seconds(10_800));
}

#[test]
fn test_every_duration_alias_days() {
    let now = Utc::now();
    let parsed = parse_schedule("every 2days", now).unwrap();
    assert_eq!(parsed.schedule.duration_secs, Some(172_800));
}

#[test]
fn test_every_duration_alias_wks() {
    let now = Utc::now();
    let parsed = parse_schedule("every 2wks", now).unwrap();
    assert_eq!(parsed.schedule.duration_secs, Some(1_209_600));
}

#[test]
fn test_every_duration_alias_months() {
    let now = Utc::now();
    let parsed = parse_schedule("every 6months", now).unwrap();
    assert_eq!(parsed.schedule.duration_secs, Some(15_552_000));
}

#[test]
fn test_every_case_insensitive_keyword() {
    let now = Utc::now();
    let parsed = parse_schedule("EVERY 30m", now).unwrap();
    assert_eq!(parsed.schedule.form, CronForm::RepeatNow);
}

#[test]
fn test_every_whitespace_trimmed() {
    let now = Utc::now();
    let parsed = parse_schedule("  every  30m  ", now).unwrap();
    assert_eq!(parsed.schedule.form, CronForm::RepeatNow);
    assert_eq!(parsed.schedule.duration_secs, Some(1800));
}

#[test]
fn test_every_zero_duration_rejected() {
    let now = Utc::now();
    let result = parse_schedule("every 0m", now);
    assert!(matches!(
        result,
        Err(ScheduleParseError::Duration(DurationParseError::Zero))
    ));
}

#[test]
fn test_every_bad_unit_rejected() {
    let now = Utc::now();
    let result = parse_schedule("every 5s", now);
    assert!(matches!(
        result,
        Err(ScheduleParseError::Duration(
            DurationParseError::UnknownUnit(_, _)
        ))
    ));
}

#[test]
fn test_every_empty_duration_error() {
    let now = Utc::now();
    let result = parse_schedule("every", now);
    assert!(matches!(
        result,
        Err(ScheduleParseError::Duration(DurationParseError::Empty))
    ));
}

// ─────────────────────────────────────────────────────────────────
// Form 3: `from <timestamp> every <duration>` — RepeatFrom (FR-009)
// ─────────────────────────────────────────────────────────────────

#[test]
fn test_from_repeat_from_basic() {
    let now = Utc::now();
    let parsed = parse_schedule("from 2025-01-15T09:00:00Z every 1h", now).unwrap();
    assert_eq!(parsed.schedule.form, CronForm::RepeatFrom);
    assert!(parsed.schedule.is_repeating());
    assert!(!parsed.schedule.is_one_shot());
}

#[test]
fn test_from_start_at_is_given_timestamp() {
    let now = Utc::now();
    let parsed = parse_schedule("from 2025-01-15T09:00:00Z every 1h", now).unwrap();
    let expected = DateTime::parse_from_rfc3339("2025-01-15T09:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    assert_eq!(parsed.schedule.start_at, Some(expected));
}

#[test]
fn test_from_duration_secs_stored() {
    let now = Utc::now();
    let parsed = parse_schedule("from 2025-01-15T09:00:00Z every 30m", now).unwrap();
    assert_eq!(parsed.schedule.duration_secs, Some(1800));
}

#[test]
fn test_from_future_start_next_due_equals_start() {
    let now = Utc::now();
    let future = now + Duration::days(7);
    let expr = format!("from {} every 1h", future.to_rfc3339());
    let parsed = parse_schedule(&expr, now).unwrap();
    assert_eq!(parsed.next_due, future);
    assert_eq!(parsed.schedule.start_at, Some(future));
}

#[test]
fn test_from_future_start_next_due_not_advanced() {
    let now = Utc::now();
    let future = now + Duration::hours(2);
    let expr = format!("from {} every 30m", future.to_rfc3339());
    let parsed = parse_schedule(&expr, now).unwrap();
    // Start is in the future, so next_due should equal start, not be advanced.
    assert_eq!(parsed.next_due, future);
}

#[test]
fn test_from_past_start_next_due_in_future() {
    let now = Utc::now();
    let past_start = now - Duration::hours(5);
    let expr = format!("from {} every 1h", past_start.to_rfc3339());
    let parsed = parse_schedule(&expr, now).unwrap();
    assert!(parsed.next_due > now, "next_due should be in the future");
}

#[test]
fn test_from_past_start_advanced_by_whole_intervals() {
    // If start is 5 hours in the past with 1h interval, next_due should be
    // approximately 1h from now (5 intervals forward + 1 to be strictly future).
    let now = Utc::now();
    let past_start = now - Duration::hours(5);
    let expr = format!("from {} every 1h", past_start.to_rfc3339());
    let parsed = parse_schedule(&expr, now).unwrap();
    let diff = parsed.next_due - now;
    // Should be at most 1h from now (the remainder of the interval).
    assert!(
        diff.num_seconds() > 0 && diff.num_seconds() <= 3600,
        "next_due should be within one interval of now, got {diff}"
    );
}

#[test]
fn test_from_past_start_exact_advancement() {
    // Start exactly 3 hours ago, interval 1h: should advance 4 intervals (3+1).
    let now = Utc::now();
    let past_start = now - Duration::hours(3);
    let expr = format!("from {} every 1h", past_start.to_rfc3339());
    let parsed = parse_schedule(&expr, now).unwrap();
    // 4 intervals forward = 4 hours from start = 1 hour from now.
    let expected = past_start + Duration::hours(4);
    assert_eq!(parsed.next_due, expected);
    assert!(parsed.next_due > now);
}

#[test]
fn test_from_past_start_30m_interval() {
    let now = Utc::now();
    let past_start = now - Duration::minutes(90);
    let expr = format!("from {} every 30m", past_start.to_rfc3339());
    let parsed = parse_schedule(&expr, now).unwrap();
    // 90 minutes / 30 = 3 intervals, +1 = 4 intervals = 120 min from start = 30 min from now.
    let expected = past_start + Duration::minutes(120);
    assert_eq!(parsed.next_due, expected);
    assert!(parsed.next_due > now);
}

#[test]
fn test_from_case_insensitive_keyword() {
    let now = Utc::now();
    let parsed = parse_schedule("FROM 2025-01-15T09:00:00Z EVERY 1h", now).unwrap();
    assert_eq!(parsed.schedule.form, CronForm::RepeatFrom);
}

#[test]
fn test_from_case_insensitive_every() {
    let now = Utc::now();
    // "every" keyword inside the expression should also be case-insensitive.
    let parsed = parse_schedule("from 2025-01-15T09:00:00Z Every 1h", now).unwrap();
    assert_eq!(parsed.schedule.form, CronForm::RepeatFrom);
}

#[test]
fn test_from_timestamp_with_offset() {
    let now = Utc::now();
    let parsed = parse_schedule("from 2025-01-15T09:00:00+02:00 every 1h", now).unwrap();
    let expected = DateTime::parse_from_rfc3339("2025-01-15T09:00:00+02:00")
        .unwrap()
        .with_timezone(&Utc);
    assert_eq!(parsed.schedule.start_at, Some(expected));
}

#[test]
fn test_from_naive_timestamp_assumed_utc() {
    let now = Utc::now();
    let parsed = parse_schedule("from 2025-01-15T09:00:00 every 1h", now).unwrap();
    let expected = NaiveDateTime::parse_from_str("2025-01-15T09:00:00", "%Y-%m-%dT%H:%M:%S")
        .unwrap()
        .and_utc();
    assert_eq!(parsed.schedule.start_at, Some(expected));
}

#[test]
fn test_from_missing_every_keyword_error() {
    let now = Utc::now();
    let result = parse_schedule("from 2025-01-15T09:00:00Z 30m", now);
    assert!(matches!(result, Err(ScheduleParseError::MissingEvery(_))));
}

#[test]
fn test_from_missing_duration_error() {
    let now = Utc::now();
    let result = parse_schedule("from 2025-01-15T09:00:00Z every", now);
    assert!(matches!(
        result,
        Err(ScheduleParseError::MissingDuration(_))
    ));
}

#[test]
fn test_from_invalid_timestamp_error() {
    let now = Utc::now();
    let result = parse_schedule("from not-a-date every 1h", now);
    assert!(matches!(
        result,
        Err(ScheduleParseError::InvalidTimestamp(_, _))
    ));
}

#[test]
fn test_from_zero_duration_rejected() {
    let now = Utc::now();
    let result = parse_schedule("from 2025-01-15T09:00:00Z every 0m", now);
    assert!(matches!(
        result,
        Err(ScheduleParseError::Duration(DurationParseError::Zero))
    ));
}

#[test]
fn test_from_bad_unit_rejected() {
    let now = Utc::now();
    let result = parse_schedule("from 2025-01-15T09:00:00Z every 5s", now);
    assert!(matches!(
        result,
        Err(ScheduleParseError::Duration(
            DurationParseError::UnknownUnit(_, _)
        ))
    ));
}

// ─────────────────────────────────────────────────────────────────
// General parser tests
// ─────────────────────────────────────────────────────────────────

#[test]
fn test_empty_expression_error() {
    let now = Utc::now();
    assert!(matches!(
        parse_schedule("", now),
        Err(ScheduleParseError::Empty)
    ));
}

#[test]
fn test_whitespace_only_expression_error() {
    let now = Utc::now();
    assert!(matches!(
        parse_schedule("   ", now),
        Err(ScheduleParseError::Empty)
    ));
}

#[test]
fn test_unknown_keyword_error() {
    let now = Utc::now();
    let result = parse_schedule("in 30m", now);
    assert!(matches!(
        result,
        Err(ScheduleParseError::UnknownKeyword(k)) if k == "in"
    ));
}

#[test]
fn test_unknown_keyword_schedule() {
    let now = Utc::now();
    let result = parse_schedule("schedule 30m", now);
    assert!(matches!(result, Err(ScheduleParseError::UnknownKeyword(_))));
}

#[test]
fn test_parsed_schedule_fields_consistent() {
    // Verify that ParsedSchedule fields are internally consistent.
    let now = Utc::now();

    // OneShot: next_due == start_at
    let parsed = parse_schedule("at 2025-06-15T12:00:00Z", now).unwrap();
    assert_eq!(parsed.next_due, parsed.schedule.start_at.unwrap());

    // RepeatNow: next_due == now + duration
    let parsed = parse_schedule("every 2h", now).unwrap();
    let dur = parsed.schedule.duration_secs.unwrap();
    assert_eq!(parsed.next_due, now + Duration::seconds(dur));

    // RepeatFrom (future start): next_due == start_at
    let future = now + Duration::days(1);
    let expr = format!("from {} every 1h", future.to_rfc3339());
    let parsed = parse_schedule(&expr, now).unwrap();
    assert_eq!(parsed.next_due, parsed.schedule.start_at.unwrap());
}

#[test]
fn test_all_three_forms_produce_correct_cron_form() {
    let now = Utc::now();

    let one_shot = parse_schedule("at 2025-01-15T09:00:00Z", now).unwrap();
    assert_eq!(one_shot.schedule.form, CronForm::OneShot);

    let repeat_now = parse_schedule("every 30m", now).unwrap();
    assert_eq!(repeat_now.schedule.form, CronForm::RepeatNow);

    let future = now + Duration::days(1);
    let expr = format!("from {} every 1h", future.to_rfc3339());
    let repeat_from = parse_schedule(&expr, now).unwrap();
    assert_eq!(repeat_from.schedule.form, CronForm::RepeatFrom);
}

#[test]
fn test_schedule_can_be_constructed_from_parsed_fields() {
    // Verify the parsed schedule fields can reconstruct a CronSchedule.
    let now = Utc::now();
    let parsed = parse_schedule("every 45m", now).unwrap();
    let reconstructed = CronSchedule::repeat_now(parsed.schedule.duration_secs.unwrap());
    assert_eq!(parsed.schedule, reconstructed);
}
