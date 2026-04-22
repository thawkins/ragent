# Config Parse Error Reporting Enhancement

## Summary

Enhanced the config file parser to provide clear, actionable error messages when JSON parsing fails. The new error format shows:
- The exact file path that failed
- Line and column number of the error
- The problematic line from the file
- A visual caret (^) indicator pointing to the error location
- The underlying serde_json parse error message

## Changes Made

### 1. Enhanced `Config::load_file()` in `crates/ragent-config/src/config.rs`

**Before:**
```rust
fn load_file(path: &Path) -> anyhow::Result<Self> {
    let content = std::fs::read_to_string(path)?;
    let config: Self = serde_json::from_str(&content)?;
    Ok(config)
}
```

**After:**
```rust
pub(crate) fn load_file(path: &Path) -> anyhow::Result<Self> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("Failed to read config file '{}': {}", path.display(), e))?;
    
    let config: Self = serde_json::from_str(&content).map_err(|e| {
        // Extract line and column from serde_json error
        let line = e.line();
        let column = e.column();
        
        // Get the problematic line from the content
        let problematic_line = content.lines().nth(line.saturating_sub(1))
            .unwrap_or("<line not found>");
        
        anyhow::anyhow!(
            "Failed to parse config file '{}':\n\
             Error at line {}, column {}:\n\
             {}\n\
             Problematic line:\n\
             {}\n\
             {}^\n\
             Parse error: {}",
            path.display(),
            line,
            column,
            "─".repeat(80),
            problematic_line,
            " ".repeat(column.saturating_sub(1)),
            e
        )
    })?;
    
    Ok(config)
}
```

### 2. Enhanced inline config parsing in `Config::load()` 

Similar error formatting applied to `RAGENT_CONFIG_CONTENT` environment variable parsing.

### 3. Enhanced CLI config file parsing in `src/main.rs`

Similar error formatting applied to `--config` flag handling.

## Error Message Examples

### Example 1: Unclosed Object in Array

**Input file:**
```json
{
    "username": "testuser",
    "permission": [
        {"permission": "file:write"
    ]
}
```

**Error output:**
```
Error: Failed to parse config file '/tmp/ragent.json':
Error at line 5, column 5:
────────────────────────────────────────────────────────────────────────────────
Problematic line:
    ]
    ^
Parse error: expected `,` or `}` at line 5 column 5
```

### Example 2: Type Mismatch

**Input file:**
```json
{
    "username": "bob",
    "permission": "not an array"
}
```

**Error output:**
```
Error: Failed to parse config file '/tmp/ragent.json':
Error at line 3, column 32:
────────────────────────────────────────────────────────────────────────────────
Problematic line:
    "permission": "not an array"
                               ^
Parse error: invalid type: string "not an array", expected a sequence at line 3 column 32
```

### Example 3: Invalid Escape Sequence

**Input file:**
```json
{
    "username": "test\xuser"
}
```

**Error output:**
```
Error: Failed to parse config file '/tmp/ragent.json':
Error at line 2, column 23:
────────────────────────────────────────────────────────────────────────────────
Problematic line:
    "username": "test\xuser"
                      ^
Parse error: invalid escape at line 2 column 23
```

## Testing

Created comprehensive test suite and manual test script:
- `crates/ragent-config/tests/test_config_parse_errors.rs` - Unit tests
- `target/temp/test_config_errors.sh` - Manual integration tests

All test cases confirm that:
✅ Parse errors are caught and reported with clear formatting
✅ Line numbers and column positions are accurate
✅ Visual caret indicator points to the exact error location
✅ Valid JSON continues to parse successfully

## Benefits

1. **Faster debugging**: Users can immediately see which line has the error
2. **Clear guidance**: Visual caret points exactly to the problem
3. **Better UX**: No need to open the file in a separate editor to find line numbers
4. **Consistent format**: Same error format across all config loading paths (file, env var, CLI)

## Files Modified

- `crates/ragent-config/src/config.rs` - Enhanced load_file() and environment variable parsing
- `src/main.rs` - Enhanced CLI config file parsing
- `crates/ragent-config/tests/test_config_parse_errors.rs` - New test suite
- `target/temp/test_config_errors.sh` - Manual test script
