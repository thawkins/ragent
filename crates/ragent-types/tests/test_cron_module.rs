//! Cron unit tests relocated out of the inline `#[cfg(test)]` block in
//! `src/cron.rs` (ANTIPAT M2.5/F1). All tested items are public API -
//! the inline block is deleted from library source.

use chrono::{DateTime, Utc};

use ragent_types::cron::*;

#[test]
fn test_cron_form_serde_roundtrip() {
    for form in [CronForm::OneShot, CronForm::RepeatFrom, CronForm::RepeatNow] {
        let json = serde_json::to_string(&form).unwrap();
        let back: CronForm = serde_json::from_str(&json).unwrap();
        assert_eq!(form, back);
    }
}

#[test]
fn test_cron_form_serde_snake_case() {
    assert_eq!(
        serde_json::to_string(&CronForm::OneShot).unwrap(),
        "\"one_shot\""
    );
    assert_eq!(
        serde_json::to_string(&CronForm::RepeatFrom).unwrap(),
        "\"repeat_from\""
    );
    assert_eq!(
        serde_json::to_string(&CronForm::RepeatNow).unwrap(),
        "\"repeat_now\""
    );
}

#[test]
fn test_cron_schedule_one_shot() {
    let ts = Utc::now();
    let s = CronSchedule::one_shot(ts);
    assert_eq!(s.form, CronForm::OneShot);
    assert_eq!(s.start_at, Some(ts));
    assert_eq!(s.duration_secs, None);
    assert!(s.is_one_shot());
    assert!(!s.is_repeating());
}

#[test]
fn test_cron_schedule_repeat_from() {
    let ts = Utc::now();
    let s = CronSchedule::repeat_from(ts, 1800);
    assert_eq!(s.form, CronForm::RepeatFrom);
    assert_eq!(s.start_at, Some(ts));
    assert_eq!(s.duration_secs, Some(1800));
    assert!(!s.is_one_shot());
    assert!(s.is_repeating());
}

#[test]
fn test_cron_schedule_repeat_now() {
    let s = CronSchedule::repeat_now(3600);
    assert_eq!(s.form, CronForm::RepeatNow);
    assert_eq!(s.start_at, None);
    assert_eq!(s.duration_secs, Some(3600));
    assert!(!s.is_one_shot());
    assert!(s.is_repeating());
}

#[test]
fn test_cron_schedule_serde_roundtrip() {
    let ts = Utc::now();
    for s in [
        CronSchedule::one_shot(ts),
        CronSchedule::repeat_from(ts, 1800),
        CronSchedule::repeat_now(3600),
    ] {
        let json = serde_json::to_string(&s).unwrap();
        let back: CronSchedule = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }
}

#[test]
fn test_cron_event_new_defaults() {
    let ts = Utc::now();
    let event = CronEvent::new(
        "cron-test".to_string(),
        "general".to_string(),
        "Run tests".to_string(),
        CronSchedule::repeat_now(1800),
        "every 30m".to_string(),
        ts,
    );
    assert_eq!(event.id, "cron-test");
    assert_eq!(event.agent_type, "general");
    assert_eq!(event.prompt, "Run tests");
    assert_eq!(event.schedule_raw, "every 30m");
    assert!(event.enabled);
    assert_eq!(event.next_due, ts);
    assert!(event.last_fired.is_none());
    // created_at should be ~now
    let now = Utc::now();
    assert!(event.created_at <= now);
}

#[test]
fn test_cron_event_serde_roundtrip() {
    let ts = Utc::now();
    let event = CronEvent::new(
        "cron-123".to_string(),
        "build".to_string(),
        "cargo test".to_string(),
        CronSchedule::repeat_from(ts, 86400),
        format!("from {} every 1d", ts.to_rfc3339()),
        ts,
    );
    let json = serde_json::to_string(&event).unwrap();
    let back: CronEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(event, back);
}

// ---- Duration parser tests (FR-014, FR-018, FR-019) ----

#[test]
fn test_parse_duration_canonical_units() {
    assert_eq!(parse_duration("1m").unwrap(), 60);
    assert_eq!(parse_duration("1h").unwrap(), 3_600);
    assert_eq!(parse_duration("1d").unwrap(), 86_400);
    assert_eq!(parse_duration("1w").unwrap(), 604_800);
    assert_eq!(parse_duration("1mo").unwrap(), 2_592_000);
}

#[test]
fn test_parse_duration_aliased_units() {
    // Minutes
    assert_eq!(parse_duration("1min").unwrap(), 60);
    assert_eq!(parse_duration("1mins").unwrap(), 60);
    // Hours
    assert_eq!(parse_duration("1hr").unwrap(), 3_600);
    assert_eq!(parse_duration("1hrs").unwrap(), 3_600);
    // Days
    assert_eq!(parse_duration("1day").unwrap(), 86_400);
    assert_eq!(parse_duration("1days").unwrap(), 86_400);
    // Weeks
    assert_eq!(parse_duration("1wk").unwrap(), 604_800);
    assert_eq!(parse_duration("1wks").unwrap(), 604_800);
    // Months
    assert_eq!(parse_duration("1month").unwrap(), 2_592_000);
    assert_eq!(parse_duration("1months").unwrap(), 2_592_000);
}

#[test]
fn test_parse_duration_multi_digit() {
    assert_eq!(parse_duration("30m").unwrap(), 1_800);
    assert_eq!(parse_duration("2h").unwrap(), 7_200);
    assert_eq!(parse_duration("7d").unwrap(), 604_800);
    assert_eq!(parse_duration("12mo").unwrap(), 31_104_000);
}

#[test]
fn test_parse_duration_with_space() {
    assert_eq!(parse_duration("30 m").unwrap(), 1_800);
    assert_eq!(parse_duration("2  h").unwrap(), 7_200);
    assert_eq!(parse_duration("1  d").unwrap(), 86_400);
}

#[test]
fn test_parse_duration_case_insensitive() {
    assert_eq!(parse_duration("30M").unwrap(), 1_800);
    assert_eq!(parse_duration("2H").unwrap(), 7_200);
    assert_eq!(parse_duration("1D").unwrap(), 86_400);
    assert_eq!(parse_duration("1MO").unwrap(), 2_592_000);
}

#[test]
fn test_parse_duration_whitespace_trimmed() {
    assert_eq!(parse_duration("  30m  ").unwrap(), 1_800);
    assert_eq!(parse_duration("\t2h\t").unwrap(), 7_200);
}

#[test]
fn test_parse_duration_zero_rejected() {
    assert!(matches!(
        parse_duration("0m"),
        Err(DurationParseError::Zero)
    ));
    assert!(matches!(
        parse_duration("0h"),
        Err(DurationParseError::Zero)
    ));
    assert!(matches!(
        parse_duration("0mo"),
        Err(DurationParseError::Zero)
    ));
}

#[test]
fn test_parse_duration_negative_rejected() {
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
}

#[test]
fn test_parse_duration_unknown_unit_rejected() {
    // Seconds are not a supported unit
    assert!(matches!(
        parse_duration("5s"),
        Err(DurationParseError::UnknownUnit(_, _))
    ));
    // Years are not a supported unit
    assert!(matches!(
        parse_duration("3y"),
        Err(DurationParseError::UnknownUnit(_, _))
    ));
    // Nonsense unit
    assert!(matches!(
        parse_duration("5xyz"),
        Err(DurationParseError::UnknownUnit(_, _))
    ));
}

#[test]
fn test_parse_duration_missing_unit() {
    assert!(matches!(
        parse_duration("30"),
        Err(DurationParseError::MissingUnit(_))
    ));
    assert!(matches!(
        parse_duration("42 "),
        Err(DurationParseError::MissingUnit(_))
    ));
}

#[test]
fn test_parse_duration_empty() {
    assert!(matches!(parse_duration(""), Err(DurationParseError::Empty)));
    assert!(matches!(
        parse_duration("   "),
        Err(DurationParseError::Empty)
    ));
}

#[test]
fn test_parse_duration_no_number() {
    assert!(matches!(
        parse_duration("abc"),
        Err(DurationParseError::InvalidNumber(_))
    ));
    assert!(matches!(
        parse_duration("m"),
        Err(DurationParseError::InvalidNumber(_))
    ));
}

#[test]
fn test_parse_duration_unknown_unit_error_lists_supported() {
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

// ---- Schedule parser tests (FR-008, FR-009) ----

#[test]
fn test_parse_schedule_at_one_shot() {
    let now = Utc::now();
    let parsed = parse_schedule("at 2025-01-15T09:00:00Z", now).unwrap();
    assert_eq!(parsed.schedule.form, CronForm::OneShot);
    assert_eq!(
        parsed.schedule.start_at,
        Some(
            DateTime::parse_from_rfc3339("2025-01-15T09:00:00Z")
                .unwrap()
                .with_timezone(&Utc)
        )
    );
    assert_eq!(parsed.schedule.duration_secs, None);
    assert_eq!(parsed.next_due, parsed.schedule.start_at.unwrap());
}

#[test]
fn test_parse_schedule_every_repeat_now() {
    let now = Utc::now();
    let parsed = parse_schedule("every 30m", now).unwrap();
    assert_eq!(parsed.schedule.form, CronForm::RepeatNow);
    assert_eq!(parsed.schedule.start_at, None);
    assert_eq!(parsed.schedule.duration_secs, Some(1800));
    // next_due = now + 30m
    let expected = now + chrono::Duration::seconds(1800);
    assert_eq!(parsed.next_due, expected);
}

#[test]
fn test_parse_schedule_from_every_repeat_from() {
    let now = Utc::now();
    let parsed = parse_schedule("from 2025-01-15T09:00:00Z every 1h", now).unwrap();
    assert_eq!(parsed.schedule.form, CronForm::RepeatFrom);
    let start = DateTime::parse_from_rfc3339("2025-01-15T09:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    assert_eq!(parsed.schedule.start_at, Some(start));
    assert_eq!(parsed.schedule.duration_secs, Some(3600));
    // Start is in the past relative to now, so next_due should be advanced
    assert!(parsed.next_due > now);
}

#[test]
fn test_parse_schedule_from_future_start() {
    let now = Utc::now();
    let future_ts = now + chrono::Duration::days(7);
    let expr = format!("from {} every 1h", future_ts.to_rfc3339());
    let parsed = parse_schedule(&expr, now).unwrap();
    assert_eq!(parsed.schedule.form, CronForm::RepeatFrom);
    assert_eq!(parsed.schedule.start_at, Some(future_ts));
    // Start is in the future, so next_due = start unchanged
    assert_eq!(parsed.next_due, future_ts);
}

#[test]
fn test_parse_schedule_from_past_start_advances() {
    let now = Utc::now();
    let past_start = now - chrono::Duration::hours(5);
    let expr = format!("from {} every 1h", past_start.to_rfc3339());
    let parsed = parse_schedule(&expr, now).unwrap();
    // next_due should be in the future, advanced by whole 1h intervals
    assert!(parsed.next_due > now);
    // It should be at most 1h from now
    let diff = parsed.next_due - now;
    assert!(diff.num_seconds() <= 3600);
}

#[test]
fn test_parse_schedule_whitespace_trimmed() {
    let now = Utc::now();
    let parsed = parse_schedule("  every  30m  ", now).unwrap();
    assert_eq!(parsed.schedule.form, CronForm::RepeatNow);
    assert_eq!(parsed.schedule.duration_secs, Some(1800));
}

#[test]
fn test_parse_schedule_case_insensitive_keyword() {
    let now = Utc::now();
    let parsed = parse_schedule("EVERY 30m", now).unwrap();
    assert_eq!(parsed.schedule.form, CronForm::RepeatNow);
}

#[test]
fn test_parse_schedule_empty() {
    let now = Utc::now();
    assert!(matches!(
        parse_schedule("", now),
        Err(ScheduleParseError::Empty)
    ));
    assert!(matches!(
        parse_schedule("   ", now),
        Err(ScheduleParseError::Empty)
    ));
}

#[test]
fn test_parse_schedule_unknown_keyword() {
    let now = Utc::now();
    assert!(matches!(
        parse_schedule("in 30m", now),
        Err(ScheduleParseError::UnknownKeyword(k)) if k == "in"
    ));
    assert!(matches!(
        parse_schedule("schedule 30m", now),
        Err(ScheduleParseError::UnknownKeyword(_))
    ));
}

#[test]
fn test_parse_schedule_invalid_timestamp() {
    let now = Utc::now();
    assert!(matches!(
        parse_schedule("at not-a-date", now),
        Err(ScheduleParseError::InvalidTimestamp(_, _))
    ));
}

#[test]
fn test_parse_schedule_at_missing_timestamp() {
    let now = Utc::now();
    assert!(matches!(
        parse_schedule("at", now),
        Err(ScheduleParseError::InvalidTimestamp(_, _))
    ));
}

#[test]
fn test_parse_schedule_from_missing_every() {
    let now = Utc::now();
    assert!(matches!(
        parse_schedule("from 2025-01-15T09:00:00Z 30m", now),
        Err(ScheduleParseError::MissingEvery(_))
    ));
}

#[test]
fn test_parse_schedule_from_missing_duration() {
    let now = Utc::now();
    assert!(matches!(
        parse_schedule("from 2025-01-15T09:00:00Z every", now),
        Err(ScheduleParseError::MissingDuration(_))
    ));
}

#[test]
fn test_parse_schedule_every_invalid_duration() {
    let now = Utc::now();
    assert!(matches!(
        parse_schedule("every 0m", now),
        Err(ScheduleParseError::Duration(DurationParseError::Zero))
    ));
    assert!(matches!(
        parse_schedule("every 5s", now),
        Err(ScheduleParseError::Duration(
            DurationParseError::UnknownUnit(_, _)
        ))
    ));
}

#[test]
fn test_parse_schedule_naive_timestamp_assumed_utc() {
    let now = Utc::now();
    // Without timezone suffix - should be assumed UTC
    let parsed = parse_schedule("at 2025-01-15T09:00:00", now).unwrap();
    assert_eq!(parsed.schedule.form, CronForm::OneShot);
    let expected =
        chrono::NaiveDateTime::parse_from_str("2025-01-15T09:00:00", "%Y-%m-%dT%H:%M:%S")
            .unwrap()
            .and_utc();
    assert_eq!(parsed.schedule.start_at, Some(expected));
}

#[test]
fn test_parse_schedule_timestamp_with_offset() {
    let now = Utc::now();
    let parsed = parse_schedule("at 2025-01-15T09:00:00+02:00", now).unwrap();
    assert_eq!(parsed.schedule.form, CronForm::OneShot);
    // The stored timestamp should be converted to UTC
    let expected = DateTime::parse_from_rfc3339("2025-01-15T09:00:00+02:00")
        .unwrap()
        .with_timezone(&Utc);
    assert_eq!(parsed.schedule.start_at, Some(expected));
    assert_eq!(parsed.next_due, expected);
}

// ---- natural-language timestamp tests ----

#[test]
fn test_parse_natural_time_5pm() {
    let now = Utc::now();
    let parsed = parse_schedule("at 5pm", now).unwrap();
    assert_eq!(parsed.schedule.form, CronForm::OneShot);
    assert!(parsed.next_due > now, "5pm should be in the future");
}

#[test]
fn test_parse_natural_time_5pm_case_insensitive() {
    let now = Utc::now();
    let parsed = parse_schedule("at 5PM", now).unwrap();
    assert_eq!(parsed.schedule.form, CronForm::OneShot);
    assert!(parsed.next_due > now);
}

#[test]
fn test_parse_natural_time_5_30pm() {
    let now = Utc::now();
    let parsed = parse_schedule("at 5:30pm", now).unwrap();
    assert_eq!(parsed.schedule.form, CronForm::OneShot);
    assert!(parsed.next_due > now);
    // Verify minute is 30
    assert_eq!(parsed.next_due.format("%M").to_string(), "30");
}

#[test]
fn test_parse_natural_time_24_hour() {
    use chrono::Offset;
    let now = Utc::now();
    let parsed = parse_schedule("at 17:00", now).unwrap();
    assert_eq!(parsed.schedule.form, CronForm::OneShot);
    assert!(parsed.next_due > now);
    // 17:00 local should produce a UTC time whose hour accounts for offset.
    let local_offset_secs: i64 = i64::from(chrono::Local::now().offset().fix().local_minus_utc());
    let expected_utc_hour = (17i64 - (local_offset_secs / 3600)).rem_euclid(24);
    assert_eq!(
        parsed
            .next_due
            .format("%H")
            .to_string()
            .parse::<i64>()
            .unwrap(),
        expected_utc_hour
    );
}

#[test]
fn test_parse_natural_time_5pm_tomorrow() {
    let now = Utc::now();
    let parsed = parse_schedule("at 5pm tomorrow", now).unwrap();
    assert_eq!(parsed.schedule.form, CronForm::OneShot);
    // "tomorrow" always resolves to the next calendar day, so the
    // result must be in the future and on a later date than today.
    assert!(parsed.next_due > now);
    assert!(parsed.next_due.date_naive() > now.date_naive());
}

#[test]
fn test_parse_natural_time_5am() {
    let now = Utc::now();
    let parsed = parse_schedule("at 5am", now).unwrap();
    assert_eq!(parsed.schedule.form, CronForm::OneShot);
    assert!(parsed.next_due > now);
}

#[test]
fn test_parse_natural_time_12pm_noon() {
    let now = Utc::now();
    let parsed = parse_schedule("at 12pm", now).unwrap();
    assert_eq!(parsed.schedule.form, CronForm::OneShot);
    // 12pm = noon, should be resolved to the next noon
    assert!(parsed.next_due > now);
}

#[test]
fn test_parse_natural_time_12am_midnight() {
    let now = Utc::now();
    let parsed = parse_schedule("at 12am", now).unwrap();
    assert_eq!(parsed.schedule.form, CronForm::OneShot);
    // 12am = midnight, should be resolved to the next midnight
    assert!(parsed.next_due > now);
}

#[test]
fn test_parse_natural_time_from_5pm_every_1h() {
    let now = Utc::now();
    let parsed = parse_schedule("from 5pm every 1h", now).unwrap();
    assert_eq!(parsed.schedule.form, CronForm::RepeatFrom);
    assert!(parsed.next_due > now);
}

#[test]
fn test_parse_natural_time_invalid() {
    let now = Utc::now();
    assert!(matches!(
        parse_schedule("at 13pm", now),
        Err(ScheduleParseError::InvalidTimestamp(_, _))
    ));
    assert!(matches!(
        parse_schedule("at 25:00", now),
        Err(ScheduleParseError::InvalidTimestamp(_, _))
    ));
    assert!(matches!(
        parse_schedule("at abc", now),
        Err(ScheduleParseError::InvalidTimestamp(_, _))
    ));
}

// ---- next_due computation tests (FR-004, FR-005, FR-008, FR-009) ----

#[test]
fn test_initial_next_due_one_shot() {
    let now = Utc::now();
    let ts = now + chrono::Duration::days(1);
    let sched = CronSchedule::one_shot(ts);
    assert_eq!(sched.initial_next_due(now), ts);
}

#[test]
fn test_initial_next_due_repeat_now() {
    let now = Utc::now();
    let sched = CronSchedule::repeat_now(1800);
    assert_eq!(
        sched.initial_next_due(now),
        now + chrono::Duration::seconds(1800)
    );
}

#[test]
fn test_initial_next_due_repeat_from_future() {
    let now = Utc::now();
    let start = now + chrono::Duration::days(1);
    let sched = CronSchedule::repeat_from(start, 3600);
    // Start is in the future -> next_due = start
    assert_eq!(sched.initial_next_due(now), start);
}

#[test]
fn test_initial_next_due_repeat_from_past() {
    let now = Utc::now();
    let start = now - chrono::Duration::hours(5);
    let sched = CronSchedule::repeat_from(start, 3600);
    let next = sched.initial_next_due(now);
    // Should be advanced into the future
    assert!(next > now);
    // At most one interval from now
    let diff = next - now;
    assert!(diff.num_seconds() <= 3600);
}

#[test]
fn test_advance_next_due_one_shot_returns_none() {
    let now = Utc::now();
    let ts = now + chrono::Duration::days(1);
    let sched = CronSchedule::one_shot(ts);
    assert_eq!(sched.advance_next_due(ts, now), None);
}

#[test]
fn test_advance_next_due_repeat_from_simple() {
    let now = Utc::now();
    let start = now - chrono::Duration::hours(1);
    let duration = 3600;
    let sched = CronSchedule::repeat_from(start, duration);
    let current = sched.initial_next_due(now);
    let next = sched.advance_next_due(current, now).unwrap();
    // Should be exactly one duration after current
    assert_eq!(next, current + chrono::Duration::seconds(duration));
    assert!(next > now);
}

#[test]
fn test_advance_next_due_repeat_now_simple() {
    let now = Utc::now();
    let duration = 1800;
    let sched = CronSchedule::repeat_now(duration);
    let current = sched.initial_next_due(now);
    let next = sched.advance_next_due(current, now).unwrap();
    assert_eq!(next, current + chrono::Duration::seconds(duration));
    assert!(next > now);
}

#[test]
fn test_advance_next_due_catches_up_after_gap() {
    let now = Utc::now();
    let duration = 60; // 1 minute
    let sched = CronSchedule::repeat_now(duration);
    // Simulate the scheduler being down for 10 minutes:
    // current next_due was 10 minutes ago
    let current = now - chrono::Duration::minutes(10);
    let next = sched.advance_next_due(current, now).unwrap();
    // Should skip ahead to the next future interval, not fire 10 catch-up runs
    assert!(next > now);
    let diff = next - now;
    assert!(
        diff.num_seconds() <= duration,
        "should be within one interval"
    );
}

#[test]
fn test_advance_next_due_repeating_zero_duration_returns_none() {
    // SEC-ragent-types-003 (SECTASKS T-035): a repeating schedule with a
    // zero duration used to trip `assert!` inside `advance_next_due`, and
    // the schedule is deserialisable from persisted/archived input. It now
    // returns `None` so the caller can disable the event instead of the
    // process aborting.
    let now = Utc::now();
    let sched = CronSchedule {
        form: CronForm::RepeatNow,
        start_at: None,
        duration_secs: Some(0),
    };
    assert_eq!(sched.advance_next_due(now, now), None);
    assert_eq!(sched.positive_duration_secs(), None);
    assert!(sched.human_readable().contains("invalid schedule"));
    assert_eq!(sched.initial_next_due(now), now);
}

#[test]
fn test_advance_next_due_missing_duration_returns_none() {
    // SEC-ragent-types-003: `duration_secs: None` on a repeating form was
    // an `expect` panic reachable from a deserialised row.
    let now = Utc::now();
    let sched = CronSchedule {
        form: CronForm::RepeatFrom,
        start_at: Some(now - chrono::Duration::hours(1)),
        duration_secs: None,
    };
    assert_eq!(sched.advance_next_due(now, now), None);
    assert_eq!(sched.initial_next_due(now), now);
    assert!(sched.human_readable().contains("unspecified interval"));
}

#[test]
fn test_parse_duration_rejects_overflow() {
    // SEC-ragent-types-003: `num * secs` wrapped in release and panicked in
    // debug; it is now a checked multiplication.
    assert!(parse_duration("999999999999999mo").is_err());
    assert!(parse_duration("30m").is_ok());
}
