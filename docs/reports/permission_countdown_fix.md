# Permission Dialog Countdown Timer Fix

**Date:** 2025-01-17  
**Status:** ✅ COMPLETE

## Issue

The permission approval dialog was not displaying the 120-second countdown timer as documented in the project memory and SPEC.md. The dialog showed a static title " Permission Required " without any visual feedback about the remaining time before automatic denial.

## Root Cause

The `render_permission_dialog()` function in `crates/ragent-tui/src/layout.rs` (line 2730+) was using a hardcoded static title string `" Permission Required "` instead of calculating and displaying the countdown timer based on the `PermissionRequest` fields (`created_at` and `timeout_secs`).

## Solution

Updated the `DialogType::PermissionApproval` rendering logic to:

1. Calculate elapsed time since request creation
2. Compute remaining seconds before timeout
3. Format as `M:SS` (minutes:seconds with zero-padding)
4. Display "EXPIRED" when timeout reaches zero
5. Update title on every render cycle

### Code Changes

**File:** `crates/ragent-tui/src/layout.rs` (around line 2730-2758)

```rust
fn render_permission_dialog(frame: &mut Frame, app: &App) {
    let Some(ref request) = app.permission_queue.front() else {
        return;
    };

    let is_question = request.permission == "question";
    let dialog_area = centered_rect(80, 80, frame.area());

    let title = if is_question {
        " Question ".to_string()
    } else {
        // Calculate remaining time for countdown
        use std::time::{SystemTime, UNIX_EPOCH};
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let elapsed = now.saturating_sub(request.created_at);
        let remaining = request.timeout_secs.saturating_sub(elapsed);
        
        if remaining == 0 {
            " Permission Required (EXPIRED) ".to_string()
        } else {
            let remaining_mins = remaining / 60;
            let remaining_secs = remaining % 60;
            format!(" Permission Required ({}:{:02} remaining) ", remaining_mins, remaining_secs)
        }
    };

    let permission_text = if is_question {
        // ... question handling ...
    } else {
        // ... permission display ...
    };

    // ... rest of rendering ...
}
```

## Behavior After Fix

**Before:**
- Dialog title: " Permission Required " (static)
- No visual feedback about timeout
- Users unaware of remaining time

**After:**
- Dialog title: " Permission Required (2:00 remaining) " (dynamic)
- Live countdown updates every render cycle
- Shows "EXPIRED" when timeout reached
- Format examples:
  - `2:00` — 2 minutes
  - `1:30` — 1 minute 30 seconds
  - `0:59` — 59 seconds
  - `0:05` — 5 seconds
  - `EXPIRED` — timeout reached

## Test Coverage

Created comprehensive test suite in `crates/ragent-tui/tests/test_permission_countdown.rs`:

1. **test_permission_request_has_timeout_fields** — validates `PermissionRequest` structure
2. **test_countdown_calculation_logic** — verifies elapsed/remaining time math
3. **test_countdown_expired** — confirms "EXPIRED" display when timeout reached
4. **test_countdown_formats_correctly** — validates M:SS formatting for various times

**Result:** 4 tests, all passing ✅

## Files Modified

1. `crates/ragent-tui/src/layout.rs` — Added countdown timer logic to `render_permission_dialog()`
2. `crates/ragent-tui/tests/test_permission_countdown.rs` — New test file with 4 tests
3. `docs/reports/permission_countdown_fix.md` — Completion report

## Build Status

✅ Compiles successfully  
✅ 4 new tests passing  
✅ No regressions

## Integration

This fix completes the permission timeout feature documented in:
- Project memory (permission system implementation notes)
- SPEC.md section 24.3.1 (Permission Request)
- docs/reports/permission_timeout_countdown.md

The countdown timer now matches the documented behavior and provides clear visual feedback to users about the remaining time to approve or deny permission requests.
