//! Tests for the [`StartupTimings`] report format, verifying that the
//! Sum and Untracked rows correctly surface uninstrumented gaps.

use ragent_types::StartupTimings;
use std::thread;
use std::time::Duration;

#[test]
fn test_sum_and_untracked_when_no_gap() {
    let mut timings = StartupTimings::new();
    thread::sleep(Duration::from_millis(10));
    timings.record("Stage A", Duration::from_millis(10));
    thread::sleep(Duration::from_millis(10));
    timings.record("Stage B", Duration::from_millis(10));

    // No uninstrumented sleep between stages — untracked should be small.
    let report = timings.format_report();
    assert!(report.contains("Sum"));
    assert!(report.contains("Untracked"));
    assert!(report.contains("Total"));

    // Sum of stages should be ~20ms.
    assert!(timings.sum_stages_ms() >= 20);
}

#[test]
fn test_untracked_reveals_gap() {
    let mut timings = StartupTimings::new();
    timings.record("Stage A", Duration::from_millis(5));

    // Simulate uninstrumented work.
    thread::sleep(Duration::from_millis(50));

    timings.record("Stage B", Duration::from_millis(5));

    // Sum should be ~10ms; total should be >50ms; untracked should be >40ms.
    let sum = timings.sum_stages_ms();
    let total = timings.total_elapsed_ms();
    let untracked = timings.untracked_ms();

    assert!(sum <= 12, "sum should be ~10ms, got {sum}");
    assert!(total >= 50, "total should be >=50ms, got {total}");
    assert!(
        untracked >= 38,
        "untracked should reveal the ~40ms gap, got {untracked}"
    );
}

#[test]
fn test_format_report_contains_all_rows() {
    let mut timings = StartupTimings::new();
    timings.record("CLI parse", Duration::from_millis(1));
    timings.record("Config load", Duration::from_millis(5));

    let report = timings.format_report();

    // The report should contain all expected row labels.
    assert!(report.contains("CLI parse"));
    assert!(report.contains("Config load"));
    assert!(report.contains("Sum"));
    assert!(report.contains("Untracked"));
    assert!(report.contains("Total"));
}

#[test]
fn test_empty_timings_report() {
    let timings = StartupTimings::new();
    let report = timings.format_report();

    // Even with no stages, Sum/Untracked/Total should appear.
    assert!(report.contains("Sum"));
    assert!(report.contains("Untracked"));
    assert!(report.contains("Total"));
    assert_eq!(timings.sum_stages_ms(), 0);
}

#[test]
fn test_finish_freezes_total_and_untracked() {
    let mut timings = StartupTimings::new();
    timings.record("Stage A", Duration::from_millis(10));

    // Finish after a short delay so the live total has advanced.
    thread::sleep(Duration::from_millis(20));
    timings.finish();
    let frozen_total = timings.total_elapsed_ms();
    let frozen_untracked = timings.untracked_ms();

    // A further delay must not change the reported total or untracked.
    thread::sleep(Duration::from_millis(50));
    assert_eq!(
        timings.total_elapsed_ms(),
        frozen_total,
        "total_elapsed_ms must stay frozen after finish()"
    );
    assert_eq!(
        timings.untracked_ms(),
        frozen_untracked,
        "untracked_ms must stay frozen after finish()"
    );

    // finish() is idempotent.
    let total_before_second_finish = timings.total_elapsed_ms();
    timings.finish();
    assert_eq!(timings.total_elapsed_ms(), total_before_second_finish);
}

#[test]
fn test_total_still_live_before_finish() {
    let timings = StartupTimings::new();
    let before = timings.total_elapsed_ms();
    thread::sleep(Duration::from_millis(30));
    let after = timings.total_elapsed_ms();
    assert!(
        after > before,
        "total should advance before finish(), {before} -> {after}"
    );
}
