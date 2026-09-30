//! Inline tests for `edit_log.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;
use std::path::Path;
use std::sync::Mutex;

/// Serialize tests that mutate the process-global `EDIT_LOG_MODE` flag.
/// Without this, a parallel test can restore its own saved flag between
/// one test's `set_edit_log_enabled(true)` and its `log_edit_operation`
/// calls, causing writes to be skipped and counts to come up short.
static EDIT_LOG_TEST_GUARD: Mutex<()> = Mutex::new(());

#[test]
fn log_dir_resolves_under_working_dir() {
    assert_eq!(
        log_dir(Path::new("/project")),
        PathBuf::from("/project/log/editlog")
    );
}

#[test]
fn summary_counts_success_and_failure() {
    let _guard = EDIT_LOG_TEST_GUARD.lock().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let wd = tmp.path();
    let prev = is_edit_log_enabled();
    set_edit_log_enabled(true);

    log_edit_operation(
        wd,
        "edit",
        Path::new("a.rs"),
        "old",
        "new",
        "success",
        false,
    );
    log_edit_operation(wd, "edit", Path::new("b.rs"), "x", "y", "not found", false);
    log_edit_operation(
        wd,
        "multi_edit",
        Path::new("c.rs"),
        "x",
        "y",
        "success",
        true,
    );

    let (total, success, fail, pct) = edit_log_summary(wd);
    assert_eq!(total, 3);
    assert_eq!(success, 2);
    assert_eq!(fail, 1);
    assert!((pct - 66.67).abs() < 0.01, "expected ~66.67%, got {pct}");

    set_edit_log_enabled(prev);
    clear_edit_logs(wd);
}

#[test]
fn edit_log_stats_aggregates_by_tool_and_reason() {
    let _guard = EDIT_LOG_TEST_GUARD.lock().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let wd = tmp.path();
    let prev = is_edit_log_enabled();
    set_edit_log_enabled(true);

    log_edit_operation(
        wd,
        "edit",
        Path::new("a.rs"),
        "old",
        "new",
        "success",
        false,
    );
    // Same underlying reason, different files: should roll up to one entry.
    log_edit_operation(wd, "edit", Path::new("b.rs"), "x", "y", "not found", false);
    log_edit_operation(wd, "edit", Path::new("c.rs"), "x", "y", "not found", false);
    // Path embedded in a longer message: should be normalised to <file>.
    log_edit_operation(
        wd,
        "edit",
        Path::new("d.rs"),
        "x",
        "y",
        "old exact text not found in /work/src/d.rs",
        false,
    );
    log_edit_operation(
        wd,
        "multi_edit",
        Path::new("e.rs"),
        "x",
        "y",
        "success",
        true,
    );
    log_edit_operation(
        wd,
        "multi_edit",
        Path::new("f.rs"),
        "x",
        "y",
        "stale file",
        false,
    );

    let stats = edit_log_stats(wd).unwrap();
    assert_eq!(stats.tool_counts.get("edit").copied().unwrap_or(0), 4);
    assert_eq!(stats.tool_counts.get("multi_edit").copied().unwrap_or(0), 2);
    assert_eq!(stats.success_for("edit"), 1);
    assert_eq!(stats.failure_for("edit"), 3);
    assert!((stats.success_pct_for("edit") - 25.0).abs() < 0.1);
    // All "not found" variants (plain and with path) should collapse.
    assert_eq!(
        stats.failure_reasons.get("not found").copied().unwrap_or(0),
        2
    );
    assert_eq!(
        stats
            .failure_reasons
            .get("old exact text not found in <file>")
            .copied()
            .unwrap_or(0),
        1
    );
    assert_eq!(
        stats
            .failure_reasons
            .get("stale file")
            .copied()
            .unwrap_or(0),
        1
    );

    set_edit_log_enabled(prev);
    clear_edit_logs(wd);
}

#[test]
fn detect_old_str_risks_finds_common_problems() {
    assert!(detect_old_str_risks("caf\u{e9}").contains(&OldStrRisk::ContainsUtf));
    assert!(detect_old_str_risks("a\tb").contains(&OldStrRisk::ContainsTabs));
    assert!(detect_old_str_risks("  leading").contains(&OldStrRisk::LeadingTrailingWhitespace));
    assert!(detect_old_str_risks("trailing  ").contains(&OldStrRisk::LeadingTrailingWhitespace));
    assert!(detect_old_str_risks("line\n  indented").contains(&OldStrRisk::EscapedWhitespace));
    assert!(detect_old_str_risks("line\n\nline").contains(&OldStrRisk::ContainsBlankLines));
    assert!(detect_old_str_risks("a\r\nb\nc").contains(&OldStrRisk::MixedLineEndings));
    assert!(
        detect_old_str_risks("plain ascii text.").is_empty(),
        "plain ascii should have no risks"
    );
}

#[test]
fn edit_log_analyse_aggregates_risks_for_failures() {
    let _guard = EDIT_LOG_TEST_GUARD.lock().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let wd = tmp.path();
    let prev = is_edit_log_enabled();
    set_edit_log_enabled(true);

    log_edit_operation(
        wd,
        "edit",
        Path::new("a.rs"),
        "old",
        "new",
        "success",
        false,
    );
    log_edit_operation(
        wd,
        "edit",
        Path::new("b.rs"),
        "  leading",
        "new",
        "not found",
        false,
    );
    log_edit_operation(
        wd,
        "edit",
        Path::new("c.rs"),
        "line\n\nline",
        "new",
        "stale file",
        false,
    );

    let analysis = edit_log_analyse(wd).unwrap();
    assert_eq!(analysis.failure_count, 2);
    assert_eq!(analysis.risky_failure_count, 2);
    // Per-tool success/failure counts: one success and two failures for "edit".
    assert_eq!(
        analysis.success_by_tool.get("edit").copied().unwrap_or(0),
        1
    );
    assert_eq!(
        analysis.failure_by_tool.get("edit").copied().unwrap_or(0),
        2
    );
    // Fail/success ratio: 2/1 = 200%.
    assert!((analysis.failure_success_ratio_pct_for("edit") - 200.0).abs() < 0.01);
    assert!(
        analysis
            .risk_counts
            .get(&OldStrRisk::LeadingTrailingWhitespace)
            .copied()
            .unwrap_or(0)
            >= 1
    );
    assert!(
        analysis
            .risk_counts
            .get(&OldStrRisk::ContainsBlankLines)
            .copied()
            .unwrap_or(0)
            >= 1
    );

    set_edit_log_enabled(prev);
    clear_edit_logs(wd);
}
