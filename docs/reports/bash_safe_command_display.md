# Bash Safe Command Display Implementation

**Date:** 2025-01-17  
**Status:** ✅ COMPLETE

## Summary

Added the built-in safe command whitelist (Layer 1) to the `/bash show` TUI command display. Users can now see all 50 safe commands that are auto-approved without permission prompts.

## Changes

### 1. Added Public API for Safe Commands (`crates/ragent-core/src/tool/bash.rs`)

**Lines 241-247:**
```rust
/// Returns the built-in safe commands allowlist.
///
/// These commands are auto-approved without user prompting (Layer 1).
#[must_use]
pub fn get_safe_commands() -> Vec<&'static str> {
    SAFE_COMMANDS.to_vec()
}
```

- Exposes the private `SAFE_COMMANDS` constant via a public function
- Returns a `Vec<&'static str>` for easy iteration
- Enables TUI and tests to access the list

### 2. Enhanced `/bash show` Command (`crates/ragent-tui/src/app.rs`)

**Lines 6321-6371 (revised):**

Added new section at the top of the output:

```markdown
### Built-in Safe Commands (Layer 1 - Auto-approved)
*These commands are auto-approved without user prompting*

  - `ls`
  - `cd`
  - `pwd`
  ... (50 total commands)
```

Updated section headers to clarify layer relationships:
- **Layer 1:** Safe commands (auto-approved)
- **Layer 2 exemptions:** User allowlist (bypasses banned commands)
- **Layer 3 custom blocks:** User denylist (adds restrictions)
- **Layer 2:** Built-in banned commands
- **Layer 3:** Built-in denied patterns

### 3. Updated Test Files

Fixed tests to use the new public API instead of accessing private constant:

- `crates/ragent-core/tests/test_auto_approve_flag.rs`
- `crates/ragent-core/tests/test_bash_command_splitting.rs`
- `crates/ragent-core/tests/test_permission_enforcement.rs`

Corrected safe command count from 51 to 50 (actual count).

### 4. Fixed Unrelated Test Issues

Updated tests to use `Option<String>` for `PermissionRule.pattern` field:
- `crates/ragent-core/src/permission/mod.rs:356,372`
- `crates/ragent-core/tests/test_permission.rs:18`
- `crates/ragent-core/tests/test_permission_system.rs` (10 instances)
- `crates/ragent-core/tests/test_custom_agents.rs:375`

## Test Results

All tests passing:
- `test_safe_commands_exported_as_public` ✅
- `test_safe_commands_format` ✅
- `test_safe_commands_categories` ✅
- `test_safe_commands_count` ✅ (corrected to 50)
- `test_safe_commands_list_is_not_empty` ✅
- `test_safe_commands_do_not_include_rm` ✅
- `test_safe_commands_are_unique` ✅
- `test_safe_commands_include_common_utilities` ✅
- Plus 2 additional tests in `test_permission_enforcement.rs` ✅

**Total:** 10 tests passing, 0 failures

## Safe Command List (50 commands)

### File Management (7)
ls, cd, pwd, mkdir, touch, cp, mv

### File Reading & Search (8)
cat, head, tail, grep, egrep, fgrep, find, rg, wc

### Version Control (2)
git, gh

### Build / Package Management (11)
cargo, rustc, rustfmt, clippy-driver, npm, yarn, pnpm, node, npx, python3, python, pip, pip3, make, docker-compose

### Text / Data Utilities (15)
echo, printf, chmod, jq, yq, sed, awk, sort, uniq, cut, tr, xargs, date, which, tree, diff, patch

**Notable exclusions:**
- `rm` — intentionally excluded (prefix matching can't distinguish safe vs destructive)
- Network tools (`curl`, `wget`, `nc`, etc.) — in banned list

## User Experience

Users can now run `/bash show` in the TUI to see:
1. **Built-in safe commands** — auto-approved
2. **User allowlist** — custom exemptions
3. **User denylist** — custom restrictions
4. **Built-in banned commands** — blocked by default
5. **Built-in denied patterns** — unconditionally blocked

Clear layer annotations help users understand the security model.

## Files Modified

- `crates/ragent-core/src/tool/bash.rs` (added `get_safe_commands()`)
- `crates/ragent-tui/src/app.rs` (enhanced `/bash show` output)
- `crates/ragent-core/tests/test_auto_approve_flag.rs` (updated API usage)
- `crates/ragent-core/tests/test_bash_command_splitting.rs` (updated API + count)
- `crates/ragent-core/tests/test_permission_enforcement.rs` (updated API usage)
- `crates/ragent-core/src/permission/mod.rs` (fixed test patterns)
- `crates/ragent-core/tests/test_permission.rs` (fixed test patterns)
- `crates/ragent-core/tests/test_permission_system.rs` (fixed test patterns)
- `crates/ragent-core/tests/test_custom_agents.rs` (fixed test patterns)

## Next Steps

No further action required. Feature is complete and tested.
