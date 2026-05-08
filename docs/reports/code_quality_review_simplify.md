# Code Quality Review - /simplify Analysis

**Date:** 2026-04-22
**Scope:** Recently changed files (HEAD~3)
**Focus:** Code duplication, complexity, performance, and error handling

## Summary

Reviewed recently modified files in the benchmark suite and configuration modules. Found and fixed several instances of code duplication and performance inefficiencies.

## Issues Found and Fixed

### 1. Extracted Duplicate Pattern Recompilation Logic

**Files:** `crates/ragent-config/src/dir_lists.rs`

**Issue:** The pattern recompilation logic was duplicated across four functions (`add_allowlist`, `remove_allowlist`, `add_denylist`, `remove_denylist`). Each contained ~10 lines of identical code for recompiling glob patterns after modification.

**Fix:** Extracted two helper functions:
- `recompile_allowlist()` - Recompiles and updates the allowlist cache
- `recompile_denylist()` - Recompiles and updates the denylist cache

**Lines Reduced:** ~40 lines of duplicated code → 2 helper functions (~16 lines)

**Code Change:**
```rust
// Before: Each add/remove function had this duplicated block:
{
    let g = global().read().map_err(|_| anyhow::anyhow!("lock poisoned"))?;
    let compiled = compile_patterns(&g.allowlist);
    if let Ok(mut guard) = compiled_allowlist().write() {
        *guard = compiled;
    }
}

// After: Single function call
recompile_allowlist()?;
```

---

### 2. Simplified `normalized_code()` Function

**File:** `crates/ragent-bench/src/suites/metrics.rs`

**Issue:** The function used manual String manipulation with loops, boolean flags, and intermediate allocations. This is more verbose and less efficient than necessary.

**Before:**
```rust
pub(crate) fn normalized_code(value: &str) -> String {
    let mut result = String::new();
    let mut first = true;
    for word in value.split_whitespace() {
        if !first {
            result.push(' ');
        }
        result.push_str(word);
        first = false;
    }
    result.trim().to_string()
}
```

**After:**
```rust
pub(crate) fn normalized_code(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}
```

**Improvements:**
- More idiomatic Rust
- Fewer allocations (no intermediate String growth)
- Clearer intent
- Eliminates manual loop and flag tracking

---

### 3. Extracted Duplicate `pass_at_1` Calculation

**Files:** 
- `crates/ragent-bench/src/suites/metrics.rs` (added helper)
- `crates/ragent-bench/src/suites/bigcodebench.rs`
- `crates/ragent-bench/src/suites/livecodebench.rs`
- `crates/ragent-bench/src/suites/multipl_e.rs`

**Issue:** The 8-line pattern for computing `pass_at_1` (first-sample exact match rate) was duplicated across three benchmark adapters:

```rust
let pass_at_1 = if evaluations.is_empty() {
    0.0
} else {
    evaluations
        .iter()
        .filter(|evaluation| evaluation.first_sample_exact_match)
        .count() as f64
        / evaluations.len() as f64
};
```

**Fix:** Added `pass_at_1()` helper function in `metrics.rs`:

```rust
pub(crate) fn pass_at_1(evaluations: &[BenchCaseEvaluation]) -> f64 {
    if evaluations.is_empty() {
        return 0.0;
    }
    evaluations
        .iter()
        .filter(|e| e.first_sample_exact_match)
        .count() as f64
        / evaluations.len() as f64
}
```

Updated all three adapters to use `crate::suites::pass_at_1(evaluations)` instead of the inline calculation.

**Lines Reduced:** ~24 lines of duplicated code → 1 helper function (~9 lines)

---

## Files Modified

1. `crates/ragent-config/src/dir_lists.rs` - Extracted recompilation helpers
2. `crates/ragent-bench/src/suites/metrics.rs` - Added `pass_at_1()` helper and simplified `normalized_code()`
3. `crates/ragent-bench/src/suites/mod.rs` - Exported `pass_at_1`
4. `crates/ragent-bench/src/suites/bigcodebench.rs` - Use new helper
5. `crates/ragent-bench/src/suites/livecodebench.rs` - Use new helper
6. `crates/ragent-bench/src/suites/multipl_e.rs` - Use new helper

---

## Additional Observations (Not Fixed)

### Large Module Warning
**File:** `crates/ragent-tui/src/app.rs` (12,644 lines)

This is an extremely large module that should be broken into submodules:
- `commands/` - Slash command handlers
- `handlers/` - Event/key handlers
- `render/` - UI rendering functions
- `state/` - State management

### Potential Future Improvements

1. **Benchmark Suite Adapters:** The `summarize()` methods in adapters share common patterns that could be further unified with a macro or trait default implementation.

2. **skipped_metrics_for_suite:** This pattern is repeated across adapters with only metric names differing. Could be extracted into a generic helper.

3. **Edit Similarity Performance:** In `metrics.rs`, `edit_similarity()` calls `.chars().count()` on both strings. For ASCII-only content, using `.len()` would be faster. Consider adding an ASCII-fast-path.

---

## Verification

All changes compile successfully:
```bash
$ cargo check -p ragent-bench -p ragent-config
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 26.51s
```

No functional changes were made - only code deduplication and simplification.
