//! Unit tests for the human-readable schedule description helper (FR-015).
//!
//! Covers `CronSchedule::human_readable()` for all three schedule forms,
//! plus the internal `duration_to_string` conversion via the public API.

use chrono::{DateTime, Utc};

use ragent_types::{CronForm, CronSchedule};

// ── OneShot form ──────────────────────────────────────────────────

#[test]
fn test_human_readable_one_shot() {
    let ts = DateTime::parse_from_rfc3339("2025-01-15T09:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let sched = CronSchedule::one_shot(ts);
    let desc = sched.human_readable();
    assert!(
        desc.starts_with("at 2025-01-15T09:00:00"),
        "one-shot description should start with 'at <timestamp>': got {desc}"
    );
    assert!(
        !desc.contains("every"),
        "one-shot should not mention 'every'"
    );
}

#[test]
fn test_human_readable_one_shot_includes_timestamp() {
    let ts = DateTime::parse_from_rfc3339("2025-06-01T12:30:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let sched = CronSchedule::one_shot(ts);
    let desc = sched.human_readable();
    assert!(
        desc.contains("2025-06-01T12:30:00"),
        "description should contain the full timestamp: got {desc}"
    );
}

// ── RepeatNow form ───────────────────────────────────────────────

#[test]
fn test_human_readable_repeat_now_minutes() {
    let sched = CronSchedule::repeat_now(1_800); // 30 minutes
    let desc = sched.human_readable();
    assert_eq!(desc, "every 30m");
}

#[test]
fn test_human_readable_repeat_now_hours() {
    let sched = CronSchedule::repeat_now(7_200); // 2 hours
    let desc = sched.human_readable();
    assert_eq!(desc, "every 2h");
}

#[test]
fn test_human_readable_repeat_now_days() {
    let sched = CronSchedule::repeat_now(86_400); // 1 day
    let desc = sched.human_readable();
    assert_eq!(desc, "every 1d");
}

#[test]
fn test_human_readable_repeat_now_weeks() {
    let sched = CronSchedule::repeat_now(604_800); // 1 week
    let desc = sched.human_readable();
    assert_eq!(desc, "every 1w");
}

#[test]
fn test_human_readable_repeat_now_months() {
    let sched = CronSchedule::repeat_now(2_592_000); // 1 month
    let desc = sched.human_readable();
    assert_eq!(desc, "every 1mo");
}

#[test]
fn test_human_readable_repeat_now_no_start_at() {
    let sched = CronSchedule::repeat_now(3_600);
    let desc = sched.human_readable();
    assert!(
        !desc.contains("from"),
        "repeat_now should not include 'from <timestamp>': got {desc}"
    );
}

// ── RepeatFrom form ──────────────────────────────────────────────

#[test]
fn test_human_readable_repeat_from() {
    let ts = DateTime::parse_from_rfc3339("2025-01-15T09:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let sched = CronSchedule::repeat_from(ts, 1_800); // 30m
    let desc = sched.human_readable();
    assert_eq!(desc, "every 30m from 2025-01-15T09:00:00+00:00");
}

#[test]
fn test_human_readable_repeat_from_hours() {
    let ts = DateTime::parse_from_rfc3339("2025-03-01T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let sched = CronSchedule::repeat_from(ts, 3_600); // 1h
    let desc = sched.human_readable();
    assert!(
        desc.starts_with("every 1h from 2025-03-01T00:00:00"),
        "got {desc}"
    );
}

#[test]
fn test_human_readable_repeat_from_weeks() {
    let ts = DateTime::parse_from_rfc3339("2025-01-01T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let sched = CronSchedule::repeat_from(ts, 604_800); // 1w
    let desc = sched.human_readable();
    assert!(
        desc.starts_with("every 1w from 2025-01-01T00:00:00"),
        "got {desc}"
    );
}

#[test]
fn test_human_readable_repeat_from_includes_both_parts() {
    let ts = DateTime::parse_from_rfc3339("2025-07-15T08:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let sched = CronSchedule::repeat_from(ts, 86_400); // 1d
    let desc = sched.human_readable();
    assert!(
        desc.contains("every 1d"),
        "should contain 'every 1d': got {desc}"
    );
    assert!(
        desc.contains("from 2025-07-15T08:00:00"),
        "should contain 'from <timestamp>': got {desc}"
    );
}

// ── Duration-to-string edge cases ─────────────────────────────────

#[test]
fn test_human_readable_multi_hour_duration() {
    // 6 hours = 21600 seconds
    let sched = CronSchedule::repeat_now(21_600);
    assert_eq!(sched.human_readable(), "every 6h");
}

#[test]
fn test_human_readable_multi_day_duration() {
    // 3 days = 259200 seconds
    let sched = CronSchedule::repeat_now(259_200);
    assert_eq!(sched.human_readable(), "every 3d");
}

#[test]
fn test_human_readable_multi_week_duration() {
    // 2 weeks = 1209600 seconds
    let sched = CronSchedule::repeat_now(1_209_600);
    assert_eq!(sched.human_readable(), "every 2w");
}

#[test]
fn test_human_readable_multi_month_duration() {
    // 3 months = 7776000 seconds
    let sched = CronSchedule::repeat_now(7_776_000);
    assert_eq!(sched.human_readable(), "every 3mo");
}

#[test]
fn test_human_readable_one_minute() {
    let sched = CronSchedule::repeat_now(60);
    assert_eq!(sched.human_readable(), "every 1m");
}

// ── Form field consistency ────────────────────────────────────────

#[test]
fn test_human_readable_form_in_description() {
    // The description should implicitly convey the form.
    let one_shot = CronSchedule::one_shot(
        DateTime::parse_from_rfc3339("2025-01-15T09:00:00Z")
            .unwrap()
            .with_timezone(&Utc),
    );
    assert!(one_shot.human_readable().starts_with("at "));

    let repeat_now = CronSchedule::repeat_now(3_600);
    assert!(repeat_now.human_readable().starts_with("every "));
    assert!(!repeat_now.human_readable().contains(" from "));

    let repeat_from = CronSchedule::repeat_from(
        DateTime::parse_from_rfc3339("2025-01-15T09:00:00Z")
            .unwrap()
            .with_timezone(&Utc),
        3_600,
    );
    assert!(repeat_from.human_readable().starts_with("every "));
    assert!(repeat_from.human_readable().contains(" from "));
}

// ── CronForm matches ─────────────────────────────────────────────

#[test]
fn test_cron_form_one_shot_in_readable() {
    let ts = Utc::now();
    let sched = CronSchedule::one_shot(ts);
    assert_eq!(sched.form, CronForm::OneShot);
    assert!(sched.human_readable().starts_with("at "));
}

#[test]
fn test_cron_form_repeat_now_in_readable() {
    let sched = CronSchedule::repeat_now(3_600);
    assert_eq!(sched.form, CronForm::RepeatNow);
    assert!(sched.human_readable().starts_with("every "));
}

#[test]
fn test_cron_form_repeat_from_in_readable() {
    let ts = Utc::now();
    let sched = CronSchedule::repeat_from(ts, 3_600);
    assert_eq!(sched.form, CronForm::RepeatFrom);
    let desc = sched.human_readable();
    assert!(desc.starts_with("every "));
    assert!(desc.contains(" from "));
}
