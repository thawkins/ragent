# Hardwired Permissions for Codeindex Tools

## Summary

Codeindex tools are now hardwired to always be allowed without requiring permission checks. This change recognizes that codeindex tools are read-only analysis utilities that pose no security risk.

## Rationale

The codeindex tools (`codeindex_search`, `codeindex_symbols`, `codeindex_references`, `codeindex_dependencies`, `codeindex_status`, `codeindex_reindex`) are:

1. **Read-only**: They only analyze existing code, never modify files
2. **Local-only**: They only access the local codebase being worked on
3. **Essential for AI workflow**: Required for effective code understanding and navigation
4. **Low risk**: No network access, no file writes, no command execution

Requiring permission approval for these tools creates unnecessary friction without providing meaningful security benefits.

## Changes Made

### 1. Modified `crates/ragent-agent/src/session/processor.rs`

Added hardwired bypass for codeindex tools in the permission check logic:

```rust
let needs_approval = match tool_name {
    "bash" => {
        // Bash command permission is checked per-command below
        false
    }
    // Codeindex tools are always allowed (read-only analysis)
    "codeindex_search" | "codeindex_symbols" | "codeindex_references" 
    | "codeindex_dependencies" | "codeindex_status" | "codeindex_reindex" => {
        false
    }
    _ => {
        // Check if tool requires permission
        let tool_permission = Permission::Tool(tool_name.to_string());
        self.config.check_permission(&tool_permission) == PermissionAction::Ask
    }
};
```

### 2. Added Comprehensive Tests

Created `crates/ragent-agent/tests/test_codeindex_tool_permissions.rs` with 4 tests:
- `test_codeindex_tools_hardwired_allow` - Verifies hardwiring bypasses deny rules
- `test_codeindex_tools_with_ask_permission` - Verifies hardwiring bypasses ask rules
- `test_other_tools_still_require_permission` - Confirms other tools still need permission
- `test_bash_tool_special_handling` - Documents bash tool's special handling

All tests passing ✅

## Behavior

### Before
- Codeindex tools required permission rules in config
- Users had to explicitly allow them via `permission` array in ragent.json
- Permission dialogs would block codeindex tool usage if not configured

### After
- Codeindex tools work immediately without configuration
- No permission checks performed (hardwired bypass)
- Permission rules for codeindex tools in config are ignored
- Other tools continue to require permission checks as configured

## Hardwired Tools List

The following tools are now hardwired to always allow:

1. **codeindex_search** - Search codebase for symbols/functions/types
2. **codeindex_symbols** - List symbols in a file
3. **codeindex_references** - Find all references to a symbol
4. **codeindex_dependencies** - Query file-level dependencies
5. **codeindex_status** - Check codebase index status
6. **codeindex_reindex** - Trigger full re-index

## Config Migration

Users can safely remove codeindex permission entries from their ragent.json config:

**Can be removed:**
```json
{
  "permission": [
    {
      "action": "allow",
      "permission": "tool:codeindex_search"
    },
    {
      "action": "allow",
      "permission": "tool:codeindex_symbols"
    },
    // ... etc
  ]
}
```

These entries are now redundant and have no effect.

## Other Tools with Special Permission Handling

- **bash**: Bypasses tool-level permission in favor of per-command security layers (7-layer bash security system)
- **codeindex_***: Hardwired to always allow (this change)
- **All other tools**: Continue to use standard permission checking

## Testing

Run tests with:
```bash
cargo test -p ragent-agent test_codeindex_tool_permissions
```

Expected output: 4 tests passing

## Files Modified

1. `crates/ragent-agent/src/session/processor.rs` - Added hardwired bypass logic
2. `crates/ragent-agent/tests/test_codeindex_tool_permissions.rs` - New test file (4 tests)
3. `docs/reports/codeindex_hardwired_permissions.md` - This document
