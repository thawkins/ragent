# `/bash show` Enhancement: Built-in Lists Display

**Date:** 2025-01-19  
**Status:** Complete

## Overview

Extended the `/bash show` slash command to display both user-defined **and** built-in bash security lists (banned commands and denied patterns). Previously, only user-defined allowlists and denylists were shown.

## Changes

### 1. Core API Addition (`ragent-core`)

Added a new public function to expose the built-in security lists:

**File:** `crates/ragent-core/src/tool/bash.rs`

```rust
/// Returns the built-in banned commands and denied patterns.
///
/// Used by the TUI to display the complete security policy in `/bash show`.
#[must_use]
pub fn get_builtin_lists() -> (Vec<&'static str>, Vec<&'static str>) {
    (
        BANNED_COMMANDS.to_vec(),
        DENIED_PATTERNS.to_vec(),
    )
}
```

This function returns:
- **BANNED_COMMANDS**: ~30 commands blocked by default (curl, wget, nmap, etc.)
- **DENIED_PATTERNS**: ~45 dangerous patterns (rm -rf /, sudo, dd if=, etc.)

### 2. TUI Integration

**File:** `crates/ragent-tui/src/app.rs`

Updated the `/bash show` command handler to:
1. Call `get_builtin_lists()` to retrieve built-in lists
2. Display them in separate sections after user-defined lists
3. Include explanatory text for each section

**Output format:**
```
## Bash command lists

### Allowlist (user-defined)
  *(empty)*

### Denylist (user-defined)
  *(empty)*

### Built-in Banned Commands
*These commands are blocked unless allowlisted or YOLO mode is enabled*

  - `curl`
  - `wget`
  - `nmap`
  ... (30+ entries)

### Built-in Denied Patterns
*Commands matching these patterns are unconditionally blocked*

  - `rm -rf /`
  - `sudo `
  - `dd if=`
  ... (45+ entries)
```

### 3. Test Coverage

**File:** `crates/ragent-core/tests/test_bash_show_lists.rs`

Added comprehensive tests:
- `test_get_builtin_lists_returns_commands_and_patterns` — Verifies non-empty lists with expected entries
- `test_builtin_lists_are_consistent` — Validates format and structure

**Test results:** ✅ 2 passed, 0 failed

## Benefits

1. **Transparency**: Users can now see the complete security policy in one place
2. **Education**: Helps users understand what commands require YOLO mode or allowlisting
3. **Debugging**: Makes it easier to diagnose why a command was blocked
4. **Documentation**: Self-documenting security posture directly in the TUI

## Usage

```bash
# In the ragent TUI
/bash show

# Output includes:
# - User-defined allowlist (commands exempted from bans)
# - User-defined denylist (custom blocked patterns)
# - Built-in banned commands (30+ risky tools)
# - Built-in denied patterns (45+ dangerous command patterns)
```

## Related Files

- `crates/ragent-core/src/tool/bash.rs` — Core security lists and new API
- `crates/ragent-core/src/bash_lists.rs` — Runtime allowlist/denylist management
- `crates/ragent-tui/src/app.rs` — TUI slash command handler
- `crates/ragent-core/tests/test_bash_show_lists.rs` — Test coverage

## Verification

```bash
# Build and verify
cargo check -p ragent-core --lib
cargo check -p ragent-tui --lib
cargo test --test test_bash_show_lists -p ragent-core
cargo build --bin ragent
```

All checks passed ✅
