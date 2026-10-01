//! Regression tests for `shorten_middle` / `truncate_with_ellipsis`.
//!
//! The status-bar cwd shortening path (`shorten_path` -> `shorten_middle`)
//! computes a per-frame character budget that can drop to 2 or 3 on a narrow
//! terminal. `shorten_middle` used to compute an unguarded budget subtraction,
//! so a budget of 2 or 3 underflowed `usize` and the TUI panicked with
//! "attempt to subtract with overflow" while rendering the status bar (crash
//! report `crates/ragent-tui/src/utils.rs:49`). The guard is retained after the
//! separator was restored to the single-character ellipsis glyph U+2026.

use ragent_tui::utils::{shorten_middle, truncate_with_ellipsis};

/// The separator used by both helpers.
const ELLIPSIS: &str = "\u{2026}";

/// Every small budget must return the bare ellipsis instead of underflowing.
#[test]
fn test_shorten_middle_small_budgets_do_not_panic() {
    let long = "/very/long/path/that/exceeds/the/budget";
    for max in 0..=3usize {
        let out = shorten_middle(long, max);
        assert_eq!(
            out, ELLIPSIS,
            "budget {max} should return the bare ellipsis"
        );
    }
}

/// A budget of 5 keeps two chars from each end around the single-char
/// separator; budget 4 keeps one from the end and one from the start only when
/// the split is even, otherwise the extra char goes to the tail.
#[test]
fn test_shorten_middle_small_budgets_keep_ends() {
    assert_eq!(shorten_middle("abcdefgh", 5), format!("ab{ELLIPSIS}gh"));
    assert_eq!(shorten_middle("abcdefgh", 4), format!("a{ELLIPSIS}gh"));
}

/// Strings that already fit are returned unchanged.
#[test]
fn test_shorten_middle_within_budget_unchanged() {
    assert_eq!(shorten_middle("short", 20), "short");
    assert_eq!(shorten_middle("exact", 5), "exact");
}

/// The kept result never exceeds the requested budget.
#[test]
fn test_shorten_middle_respects_budget() {
    let long = "/home/thawkins/Projects/ragent";
    for max in 4..=40usize {
        let out = shorten_middle(long, max);
        assert!(
            out.chars().count() <= max,
            "budget {max} produced {} chars: {out}",
            out.chars().count()
        );
    }
}

/// `truncate_with_ellipsis` shared the identical underflow and must not panic.
#[test]
fn test_truncate_with_ellipsis_small_budgets_do_not_panic() {
    let long = "a fairly long label";
    for max in 0..=1usize {
        let out = truncate_with_ellipsis(long, max);
        assert_eq!(
            out, ELLIPSIS,
            "budget {max} should return the bare ellipsis"
        );
    }
}
