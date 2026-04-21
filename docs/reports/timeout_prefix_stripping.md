# Timeout Prefix Stripping for Bash Permission Checks

## Overview

Implemented support for stripping `timeout [nnn]` prefix from bash commands before permission checks. The `timeout` command is a wrapper that executes another command with a time limit, and should be transparent to the permission system.

## Implementation

### Changes Made

1. **Added `strip_timeout_prefix()` function** in `crates/ragent-core/src/session/processor.rs`
   - Detects `timeout [numeric]` prefix at the start of a command
   - Returns the underlying command without the timeout wrapper
   - Case-sensitive (only lowercase "timeout" is recognized)
   - Requires whitespace after "timeout" and a numeric argument

2. **Integrated into `split_bash_command()`**
   - Applies timeout stripping to each sub-command in compound statements
   - Works with `&&`, `||`, and `;` delimiters
   - Handles both single and compound commands

3. **Added comprehensive test documentation**
   - `crates/ragent-core/tests/test_bash_command_splitting.rs` - Updated with timeout test cases
   - `crates/ragent-core/tests/test_timeout_stripping.rs` - New dedicated timeout test file
   - `examples/test_timeout_strip.rs` - Executable example demonstrating the behavior

## Behavior

### Examples

| Input | Output (for permission check) |
|-------|-------------------------------|
| `timeout 600 cargo build` | `cargo build` |
| `timeout 10 ls -la` | `ls -la` |
| `timeout 600 cargo build && cargo test` | `["cargo build", "cargo test"]` |
| `timeout 10 ls && timeout 20 cat file` | `["ls", "cat file"]` |
| `cargo test` | `cargo test` (unchanged) |
| `timeout_tool --flag` | `timeout_tool --flag` (not stripped) |
| `TIMEOUT 600 cargo build` | `TIMEOUT 600 cargo build` (case-sensitive) |

### Edge Cases Handled

- ✅ Commands without timeout prefix remain unchanged
- ✅ "timeout" must be followed by whitespace
- ✅ Numeric argument must be all digits
- ✅ Extra whitespace is normalized
- ✅ Case-sensitive (uppercase TIMEOUT is not stripped)
- ✅ Works with quoted arguments after the timeout
- ✅ Non-timeout commands starting with "timeout" (e.g., `timeout_tool`) are not affected

## Testing

All test cases pass successfully:

```bash
$ ./target/debug/examples/test_timeout_strip
Testing strip_timeout_prefix and split_bash_command:

Input:    "timeout 600 cargo build"
Expected: ["cargo build"]
Got:      ["cargo build"]
Status:   ✓ PASS

[... all 10 test cases pass ...]
```

## Permission Flow

When a bash command is executed:

1. The full command string is passed to `split_bash_command()`
2. Command is split on delimiters (`&&`, `||`, `;`)
3. **NEW:** Each sub-command has its `timeout [nnn]` prefix stripped
4. Permission check runs on the actual command (e.g., `cargo`, `ls`, `cat`)
5. If all sub-commands are approved, the original command (with timeout) executes

## Rationale

The `timeout` utility is a standard Unix tool for limiting command execution time. It's commonly used in build scripts and test runners (e.g., `timeout 600 cargo test`). 

Permission checks should evaluate the **actual command being executed** (e.g., `cargo`), not the timeout wrapper. This change ensures that:

- Permission rules for `cargo`, `git`, etc. work correctly even when wrapped with timeout
- Users don't need separate permission rules for timeout-wrapped vs. bare commands
- The system remains transparent to common development workflows

## Files Modified

- `crates/ragent-core/src/session/processor.rs` - Added `strip_timeout_prefix()`, integrated into `split_bash_command()`
- `crates/ragent-core/tests/test_bash_command_splitting.rs` - Added timeout test cases
- `crates/ragent-core/tests/test_timeout_stripping.rs` - New test file (documentation tests)
- `examples/test_timeout_strip.rs` - New executable example

## Backward Compatibility

✅ Fully backward compatible. Commands without timeout prefixes are unaffected. Existing permission rules continue to work as before.
