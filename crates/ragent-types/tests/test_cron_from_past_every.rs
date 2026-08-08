//! Integration test for FR-009: `from <past> every <d>` → next_due advanced to future.
//!
//! FR-009 (event-driven): When a user adds an event with the
//! `from <timestamp> every <duration>` form, the system shall set the start
//! time to the given timestamp and compute `next_due` as that timestamp
//! (or, if the timestamp is in the past, advance by whole duration intervals
//! until `next_due` is in the future).
//!
//! This test exercises the full flow from schedule expression → parsed
//! schedule → `CronEvent` construction, verifying that:
//!
//! - `start_at` preserves the original past timestamp (not advanced).
//! - `next_due` is advanced to a strictly future time.
//! - The advancement is by whole duration intervals.
//! - The `CronForm` is `RepeatFrom`.
//! - Multiple duration units and various past-start offsets are covered.

use chrono::{Duration, Utc};

use ragent_types::{CronEvent, CronForm, parse_schedule};

// ────────────────────────────────────────────────��────────────────
// Parser-level: next_due is strictly in the future
// ─────────────────────────────────────────────────────────────────

/// A start time 5 hours in the past with a 1h interval should produce a
/// `next_due` that is strictly after `now`.
#[test]
fn test_from_past_next_due_strictly_in_future() {
    let now = Utc::now();
    let past_start = now - Duration::hours(5);
    let expr = format!("from {} every 1h", past_start.to_rfc3339());
    let parsed = parse_schedule(&expr, now).unwrap();
    assert!(
        parsed.next_due > now,
        "next_due {} must be strictly after now {}",
        parsed.next_due,
        now
    );
}

/// `start_at` should preserve the original past timestamp, not the advanced
/// `next_due`.
#[test]
fn test_from_past_start_at_preserves_original_timestamp() {
    let now = Utc::now();
    let past_start = now - Duration::hours(3);
    let expr = format!("from {} every 1h", past_start.to_rfc3339());
    let parsed = parse_schedule(&expr, now).unwrap();
    assert_eq!(parsed.schedule.start_at, Some(past_start));
    assert_ne!(parsed.next_due, past_start, "next_due must be advanced");
}

/// The `CronForm` must be `RepeatFrom` for this schedule form.
#[test]
fn test_from_past_form_is_repeat_from() {
    let now = Utc::now();
    let past_start = now - Duration::hours(2);
    let expr = format!("from {} every 30m", past_start.to_rfc3339());
    let parsed = parse_schedule(&expr, now).unwrap();
    assert_eq!(parsed.schedule.form, CronForm::RepeatFrom);
    assert!(parsed.schedule.is_repeating());
    assert!(!parsed.schedule.is_one_shot());
}

// ─────────────────────────────────────────────────────────────────
// Exact advancement arithmetic
// ─────────────────────────────────────────────────────────────────

/// Start exactly 3h ago, interval 1h: advance 4 intervals (3+1) → next_due
/// = start + 4h = now + 1h.
#[test]
fn test_from_past_exact_3h_ago_1h_interval() {
    let now = Utc::now();
    let past_start = now - Duration::hours(3);
    let expr = format!("from {} every 1h", past_start.to_rfc3339());
    let parsed = parse_schedule(&expr, now).unwrap();
    let expected = past_start + Duration::hours(4);
    assert_eq!(parsed.next_due, expected);
    assert!(parsed.next_due > now);
}

/// Start 90m ago, interval 30m: advance 4 intervals (3+1) → next_due
/// = start + 120m = now + 30m.
#[test]
fn test_from_past_90m_ago_30m_interval() {
    let now = Utc::now();
    let past_start = now - Duration::minutes(90);
    let expr = format!("from {} every 30m", past_start.to_rfc3339());
    let parsed = parse_schedule(&expr, now).unwrap();
    let expected = past_start + Duration::minutes(120);
    assert_eq!(parsed.next_due, expected);
    assert!(parsed.next_due > now);
}

/// Start 2 days ago, interval 1d: advance 3 intervals (2+1) → next_due
/// = start + 3d = now + 1d.
#[test]
fn test_from_past_2d_ago_1d_interval() {
    let now = Utc::now();
    let past_start = now - Duration::days(2);
    let expr = format!("from {} every 1d", past_start.to_rfc3339());
    let parsed = parse_schedule(&expr, now).unwrap();
    let expected = past_start + Duration::days(3);
    assert_eq!(parsed.next_due, expected);
    assert!(parsed.next_due > now);
}

/// Start 2 weeks ago, interval 1w: advance 3 intervals (2+1) → next_due
/// = start + 3w = now + 1w.
#[test]
fn test_from_past_2w_ago_1w_interval() {
    let now = Utc::now();
    let past_start = now - Duration::weeks(2);
    let expr = format!("from {} every 1w", past_start.to_rfc3339());
    let parsed = parse_schedule(&expr, now).unwrap();
    let expected = past_start + Duration::weeks(3);
    assert_eq!(parsed.next_due, expected);
    assert!(parsed.next_due > now);
}

/// Start 2 months ago, interval 1mo (30 days): advance 3 intervals (2+1).
#[test]
fn test_from_past_2mo_ago_1mo_interval() {
    let now = Utc::now();
    let past_start = now - Duration::days(60);
    let expr = format!("from {} every 1mo", past_start.to_rfc3339());
    let parsed = parse_schedule(&expr, now).unwrap();
    // 2 months (60 days) / 30 days = 2 intervals, +1 = 3 intervals = 90 days from start.
    let expected = past_start + Duration::days(90);
    assert_eq!(parsed.next_due, expected);
    assert!(parsed.next_due > now);
}

// ─────────────────────────────────────────────────────────────────
// Advancement stays within one interval of now
// ─────────────────────────────────────────────────────────────────

/// The advanced `next_due` should be within one duration interval of `now`.
#[test]
fn test_from_past_next_due_within_one_interval() {
    let now = Utc::now();
    let past_start = now - Duration::hours(5);
    let expr = format!("from {} every 1h", past_start.to_rfc3339());
    let parsed = parse_schedule(&expr, now).unwrap();
    let diff = parsed.next_due - now;
    assert!(
        diff.num_seconds() > 0 && diff.num_seconds() <= 3600,
        "next_due should be within one interval of now, got {diff}"
    );
}

/// Large past gap (30 days ago) with 1d interval: next_due within 1 day of now.
#[test]
fn test_from_past_large_gap_within_one_interval() {
    let now = Utc::now();
    let past_start = now - Duration::days(30);
    let expr = format!("from {} every 1d", past_start.to_rfc3339());
    let parsed = parse_schedule(&expr, now).unwrap();
    let diff = parsed.next_due - now;
    assert!(
        diff.num_seconds() > 0 && diff.num_seconds() <= 86_400,
        "next_due should be within one day of now, got {diff}"
    );
}

// ─────────────────────────────────────────────────────────────────
// Full CronEvent construction from past-start schedule
// ──────────────────────────────────────────────��──────────────────

/// Construct a `CronEvent` from a `from <past> every <d>` schedule and verify
/// all fields including the advanced `next_due`.
#[test]
fn test_from_past_cron_event_next_due_advanced() {
    let now = Utc::now();
    let past_start = now - Duration::hours(3);
    let expr = format!("from {} every 1h", past_start.to_rfc3339());
    let parsed = parse_schedule(&expr, now).unwrap();

    let event = CronEvent::new(
        "evt-from-past".to_string(),
        "coder".to_string(),
        "Build and test".to_string(),
        parsed.schedule,
        expr.clone(),
        parsed.next_due,
    );

    assert_eq!(event.id, "evt-from-past");
    assert_eq!(event.agent_type, "coder");
    assert_eq!(event.prompt, "Build and test");
    assert_eq!(event.schedule_raw, expr);
    assert!(event.enabled);
    assert!(event.next_due > now, "event.next_due must be in the future");
    assert_eq!(
        event.schedule.start_at,
        Some(past_start),
        "start_at must preserve the original past timestamp"
    );
    assert_eq!(event.schedule.duration_secs, Some(3600));
    assert_eq!(event.schedule.form, CronForm::RepeatFrom);
}

// ──────────────────────────────��──────────────────────────────────
// Edge: start exactly at now (boundary between past and future)
// ─────────────────────────────────────────────────────────────────

/// A start time exactly at `now` should not be advanced (start >= now).
#[test]
fn test_from_now_start_not_advanced() {
    let now = Utc::now();
    let expr = format!("from {} every 30m", now.to_rfc3339());
    let parsed = parse_schedule(&expr, now).unwrap();
    // start == now, so advance_to_future returns start unchanged (start >= now).
    assert_eq!(parsed.next_due, now);
    assert_eq!(parsed.schedule.start_at, Some(now));
}

// ─────────────────────────────────────────────────────────────────
// Edge: start 1 second in the past (minimal past gap)
// ─────────────────────────────────────────────────────────────────

/// A start time 1 second in the past with a 1-minute interval should advance
/// by 1 interval (0 full intervals + 1).
#[test]
fn test_from_past_one_second_ago() {
    let now = Utc::now();
    let past_start = now - Duration::seconds(1);
    let expr = format!("from {} every 1m", past_start.to_rfc3339());
    let parsed = parse_schedule(&expr, now).unwrap();
    // 1 second / 60 = 0 intervals, +1 = 1 interval = 60 seconds from start.
    let expected = past_start + Duration::seconds(60);
    assert_eq!(parsed.next_due, expected);
    assert!(parsed.next_due > now);
}

// ─────────────────────────────────────────────────────────────────
// Edge: start far in the past with large interval
// ─────────────────────────────────────────────────────────────────

/// Start 1 year ago, interval 1mo: next_due should be within one month of now.
#[test]
fn test_from_past_one_year_ago_monthly_interval() {
    let now = Utc::now();
    let past_start = now - Duration::days(365);
    let expr = format!("from {} every 1mo", past_start.to_rfc3339());
    let parsed = parse_schedule(&expr, now).unwrap();
    let diff = parsed.next_due - now;
    // 1 month = 2_592_000 seconds; next_due should be within one month of now.
    assert!(
        diff.num_seconds() > 0 && diff.num_seconds() <= 2_592_000,
        "next_due should be within one month of now, got {diff}"
    );
    assert!(parsed.next_due > now);
}
