//! Integration test for FR-008: `every <d>` with no explicit start timestamp.
//!
//! FR-008 (event-driven): When a user adds an event with the `every <duration>`
//! form (no explicit start timestamp), the system shall set the start time to
//! the current time and compute `next_due` as now + duration.
//!
//! This test exercises the full flow from schedule expression → parsed
//! schedule → `CronEvent` construction, verifying that:
//!
//! - `start_at` is `None` (no explicit start given).
//! - `schedule.form` is `RepeatNow`.
//! - `next_due` equals `now + duration` for each supported unit.
//! - The `CronEvent` constructed from the parsed result carries the correct
//!   `next_due` and schedule metadata.

use chrono::{Duration, Utc};

use ragent_types::{CronEvent, CronForm, parse_schedule};

/// Verify the `every <d>` form sets `next_due = now + d` for each unit.
#[test]
fn test_every_no_start_next_due_is_now_plus_duration() {
    // Use a fixed "now" so the assertion is exact.
    let now = Utc::now();

    // 30 minutes
    {
        let parsed = parse_schedule("every 30m", now).unwrap();
        assert_eq!(parsed.next_due, now + Duration::seconds(1800));
    }

    // 1 hour
    {
        let parsed = parse_schedule("every 1h", now).unwrap();
        assert_eq!(parsed.next_due, now + Duration::seconds(3600));
    }

    // 1 day
    {
        let parsed = parse_schedule("every 1d", now).unwrap();
        assert_eq!(parsed.next_due, now + Duration::seconds(86_400));
    }

    // 1 week
    {
        let parsed = parse_schedule("every 1w", now).unwrap();
        assert_eq!(parsed.next_due, now + Duration::seconds(604_800));
    }

    // 1 month (30 days = 2_592_000 seconds)
    {
        let parsed = parse_schedule("every 1mo", now).unwrap();
        assert_eq!(parsed.next_due, now + Duration::seconds(2_592_000));
    }
}

/// Verify `start_at` is `None` and `form` is `RepeatNow` for `every <d>`.
#[test]
fn test_every_no_start_has_no_start_at_and_repeat_now_form() {
    let now = Utc::now();
    let parsed = parse_schedule("every 30m", now).unwrap();

    assert_eq!(parsed.schedule.start_at, None, "start_at must be None");
    assert_eq!(parsed.schedule.form, CronForm::RepeatNow);
    assert!(parsed.schedule.is_repeating());
    assert!(!parsed.schedule.is_one_shot());
}

/// Verify the full flow: parse `every <d>` → construct `CronEvent` → check
/// that the event's `next_due` matches `now + d`.
#[test]
fn test_every_no_start_cron_event_next_due() {
    let now = Utc::now();
    let parsed = parse_schedule("every 2h", now).unwrap();

    let event = CronEvent::new(
        "evt-every-2h".to_string(),
        "general".to_string(),
        "Run tests".to_string(),
        parsed.schedule,
        "every 2h".to_string(),
        parsed.next_due,
    );

    assert_eq!(event.id, "evt-every-2h");
    assert_eq!(event.agent_type, "general");
    assert_eq!(event.prompt, "Run tests");
    assert_eq!(event.schedule_raw, "every 2h");
    assert!(event.enabled);
    assert_eq!(event.next_due, now + Duration::seconds(7200));
    assert_eq!(event.schedule.start_at, None);
    assert_eq!(event.schedule.duration_secs, Some(7200));
    assert_eq!(event.schedule.form, CronForm::RepeatNow);
}

/// Verify that a large duration (e.g. `every 2w`) produces the correct
/// `next_due` far in the future.
#[test]
fn test_every_no_start_large_duration() {
    let now = Utc::now();
    let parsed = parse_schedule("every 2w", now).unwrap();

    // 2 weeks = 14 days = 1_209_600 seconds
    assert_eq!(parsed.schedule.duration_secs, Some(1_209_600));
    assert_eq!(parsed.next_due, now + Duration::seconds(1_209_600));
}

/// Verify that the `every <d>` form with unit aliases also computes
/// `next_due = now + d` correctly.
#[test]
fn test_every_no_start_unit_aliases() {
    let now = Utc::now();

    // Alias: `mins` → minutes
    {
        let parsed = parse_schedule("every 15mins", now).unwrap();
        assert_eq!(parsed.next_due, now + Duration::seconds(900));
    }

    // Alias: `hrs` → hours
    {
        let parsed = parse_schedule("every 3hrs", now).unwrap();
        assert_eq!(parsed.next_due, now + Duration::seconds(10_800));
    }

    // Alias: `days` → days
    {
        let parsed = parse_schedule("every 5days", now).unwrap();
        assert_eq!(parsed.next_due, now + Duration::seconds(432_000));
    }

    // Alias: `wks` → weeks
    {
        let parsed = parse_schedule("every 2wks", now).unwrap();
        assert_eq!(parsed.next_due, now + Duration::seconds(1_209_600));
    }

    // Alias: `months` → months (30 days)
    {
        let parsed = parse_schedule("every 6months", now).unwrap();
        assert_eq!(parsed.next_due, now + Duration::seconds(15_552_000));
    }
}

/// Verify that `next_due` is strictly in the future relative to `now`.
#[test]
fn test_every_no_start_next_due_is_in_future() {
    let now = Utc::now();
    let parsed = parse_schedule("every 1m", now).unwrap();
    assert!(
        parsed.next_due > now,
        "next_due ({}) must be after now ({})",
        parsed.next_due,
        now
    );
}
