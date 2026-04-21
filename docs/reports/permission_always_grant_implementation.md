# Permission "Always Allow" Implementation Complete

## Summary

Implemented the missing functionality for the permission dialog's `[a]` (always allow) option. Previously, pressing 'a' behaved identically to 'y' (once) — the permission was granted but not persisted, so subsequent identical requests would prompt again.

## Changes Made

### 1. Event Structure Update (`crates/ragent-core/src/event/mod.rs`)
- Added `decision: PermissionDecision` field to `Event::PermissionReplied`
- This carries the full decision context (Once, Always, or Deny) from the UI to the permission handler

### 2. TUI Input Handler (`crates/ragent-tui/src/input.rs`)
- Updated all three key handlers (y/a/n) to include the appropriate `decision` field:
  - `KeyCode::Char('y')` → `PermissionDecision::Once`
  - `KeyCode::Char('a')` → `PermissionDecision::Always`
  - `KeyCode::Char('n')` → `PermissionDecision::Deny`

### 3. Session Processor (`crates/ragent-core/src/session/processor.rs`)
- Updated `check_permission_with_prompt()` to:
  - Extract the `decision` field from `PermissionReplied` events
  - Call `checker.record_always(permission, resource)` when decision is `Always`
  - Added debug logging for always-grant recordings

### 4. HTTP Server (`crates/ragent-server/src/routes/mod.rs`)
- Extended `PermissionReplyDecision` enum to include `Always` variant
- Updated `reply_permission()` to map HTTP request decision to core `PermissionDecision` type
- Now supports `{"decision": "always"}` in permission reply POST requests

### 5. SSE Event Streaming (`crates/ragent-server/src/sse.rs`)
- Updated `PermissionReplied` pattern match to use `..` (ignore extra fields)
- SSE clients continue to receive `allowed: bool` field; decision field not exposed in SSE

### 6. Test Updates
- `test_event_system.rs`: Added `PermissionDecision` import and updated test event
- `test_event_to_sse.rs`: Added decision field to test event
- All existing tests pass

### 7. Other Event Consumers
- `team_spawn.rs`: Updated two match arms to ignore the new `decision` field
- `app.rs` (TUI event handler): Updated to ignore the new field

## Behavior

**Before:**
- Pressing `[a]` granted permission once
- Next identical request prompted again

**After:**
- Pressing `[a]` grants permission AND records an always-grant
- Next identical request auto-approves without prompting
- Grant is stored in `PermissionChecker.always_grants` HashMap
- Grants persist for the lifetime of the session (in-memory only, not saved to disk)

## Testing

All existing tests pass:
- `cargo test -p ragent-core --lib` ✅
- `cargo test -p ragent-core --test test_permission_system` ✅ 
- `cargo test -p ragent-core --test test_event_system` ✅
- `cargo test -p ragent-server --lib` ✅
- `cargo check` ✅ (all crates)

## Implementation Notes

- Always-grants are stored as `globset::GlobMatcher` instances for efficient pattern matching
- Precedence: always-grants are checked before ruleset evaluation
- The grant is scoped to the exact permission type and glob pattern
- No configuration file changes — grants are runtime-only
- HTTP API now supports `{"decision": "always"}` for programmatic permission replies

## Files Modified

1. `crates/ragent-core/src/event/mod.rs`
2. `crates/ragent-core/src/session/processor.rs`
3. `crates/ragent-core/src/tool/team_spawn.rs`
4. `crates/ragent-core/tests/test_event_system.rs`
5. `crates/ragent-server/src/routes/mod.rs`
6. `crates/ragent-server/src/sse.rs`
7. `crates/ragent-server/tests/test_event_to_sse.rs`
8. `crates/ragent-tui/src/app.rs`
9. `crates/ragent-tui/src/input.rs`
