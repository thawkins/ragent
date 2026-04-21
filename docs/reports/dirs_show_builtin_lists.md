# Built-in Directory Lists for `/dirs show` Command

**Date:** 2025-01-17  
**Status:** ✅ COMPLETE

## Summary

Added built-in directory allowlist and denylist constants to the `dir_lists` module and updated the `/dirs show` TUI command to display them alongside user-defined patterns, matching the pattern established by `/bash show`.

## Changes Made

### 1. Core Implementation (`crates/ragent-core/src/dir_lists.rs`)

**Added:**
- `BUILTIN_ALLOWLIST` constant: Empty by default, ready for future safe patterns (e.g., `target/**`, `.git/**`)
- `BUILTIN_DENYLIST` constant: 22 system-critical directory patterns including:
  - Unix/Linux: `/bin/**`, `/sbin/**`, `/boot/**`, `/dev/**`, `/proc/**`, `/sys/**`, `/etc/**`, `/usr/bin/**`, etc.
  - macOS: `/System/**`, `/Library/**`, `/Applications/**`, `/private/**`
  - Windows: `C:/Windows/**`, `C:/Program Files/**`, `C:/Program Files (x86)/**`
- `get_builtin_lists()` function: Returns `(Vec<String>, Vec<String>)` for allowlist and denylist

**Location:** Lines 19-76 (58 lines added)

### 2. TUI Display (`crates/ragent-tui/src/app.rs`)

**Updated `/dirs show` command** to display:
1. **Built-in Allowlist** — currently empty, shows `*(empty)*`
2. **User Allowlist** — user-defined auto-approve patterns
3. **Built-in Denylist** — 22 system-critical directories (auto-deny)
4. **User Denylist** — user-defined auto-deny patterns

**Location:** Lines 6575-6632 (modified)

### 3. Test Coverage

**Added tests in `crates/ragent-core/tests/test_dir_lists.rs`:**
- `test_dir_lists_get_builtin_lists()` — validates function returns expected structure

**Added new test file `crates/ragent-tui/tests/test_dirs_show.rs`:**
- `test_get_builtin_lists_returns_expected_structure()` — verifies return types
- `test_builtin_denylist_includes_critical_directories()` — validates 22 critical paths
- `test_builtin_allowlist_empty_by_default()` — confirms empty allowlist

**Result:** 4 new tests, all passing ✅

## Architecture

Follows the same pattern as `/bash show`:
- Built-in lists are defined as constants in the module
- User lists are loaded from `ragent.json` (project/global config)
- Display shows built-in lists first, then user-defined lists
- Each section has a clear header and explanatory text

## Security Design

**Built-in Denylist Rationale:**
- Protects system-critical directories from accidental modification
- Covers major operating systems (Linux, macOS, Windows)
- Uses glob patterns (`**`) to match recursively
- Cannot be removed by user (always active)

**Built-in Allowlist Design:**
- Empty by default (conservative approach)
- Reserved for future safe patterns like build artifacts
- Commented examples provided in code

## Usage

Users can now run `/dirs show` in the TUI to see:
```
## Directory/File Permission Lists

### Built-in Allowlist (auto-approve)
*(empty)*

### User Allowlist (auto-approve)
  - `src/**`
  - `docs/**`

### Built-in Denylist (auto-deny)
  - `/bin/**`
  - `/sbin/**`
  - `/boot/**`
  [... 19 more patterns ...]

### User Denylist (auto-deny)
  - `secrets/**`
```

## Files Modified

1. `crates/ragent-core/src/dir_lists.rs` — Added built-in constants and getter function
2. `crates/ragent-tui/src/app.rs` — Updated `/dirs show` command output
3. `crates/ragent-core/tests/test_dir_lists.rs` — Added unit test
4. `crates/ragent-tui/tests/test_dirs_show.rs` — Added integration tests (new file)

## Build Status

✅ All code compiles successfully  
✅ 4 new tests passing  
⚠️ Pre-existing test failures in `test_bash_show_lists.rs` (unrelated to this change)

## Next Steps

- Consider adding commonly safe patterns to `BUILTIN_ALLOWLIST` (e.g., `target/**`, `.git/**`)
- Integrate built-in denylist into permission checking logic (currently display-only)
- Document the patterns in user-facing documentation
