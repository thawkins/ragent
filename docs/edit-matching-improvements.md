# Edit/MultiEdit Matching Improvements

## Analysis of Edit Log Failures

Based on analysis of `/editlog analyse` output from `log/edits-*.jsonl`, the following failure patterns were identified:

| Failure Pattern Combination | Count |
|---------------------------|-------|
| `contains blank lines` + `contains escaped whitespace` + `has leading or trailing whitespace` | 23 |
| `contains blank lines` + `contains escaped whitespace` + `contains utf characters` + `has leading or trailing whitespace` | 14 |
| `contains escaped whitespace` + `contains utf characters` + `has leading or trailing whitespace` | 12 |
| `contains blank lines` + `contains escaped whitespace` + `contains utf characters` | 7 |
| `contains escaped whitespace` | 4 |
| `contains blank lines` + `contains escaped whitespace` | 3 |
| `contains utf characters` | 3 |
| `contains blank lines` + `contains utf characters` | 2 |

**Total analyzed failures: 68+**

### Key Observations

1. **Escaped whitespace** appears in 63/68 (93%) of failures
2. **Leading/trailing whitespace** appears in 49/68 (72%) of failures  
3. **Blank lines** appear in 47/68 (69%) of failures
4. **UTF-8 characters** appear in 38/68 (56%) of failures

## Root Causes

### 1. Escaped Whitespace Handling
The `decode_escapes()` function handles `\t`, `\n`, `\r`, `\\` correctly, but users may provide:
- Inconsistent escaping (e.g., literal tabs vs `\t`)
- Mixed escaping styles in the same needle
- Escapes that differ from what's actually in the file

**Current implementation**: Already handles standard escapes correctly.

### 2. Leading/Trailing Whitespace
The flexible matcher doesn't trim boundaries - if the needle starts/ends with whitespace that differs from the content, matching fails at the boundaries.

**Current implementation**: The whitespace folding handles internal whitespace runs, but boundary differences can still cause failures.

### 3. Blank Lines
Blank lines in the needle might be `\n` but in the content might be `  \n` (with spaces) or `\t\n` (with tabs). The folding treats consecutive whitespace as one unit.

**Current implementation**: Should handle this via whitespace folding, but edge cases exist with multiple consecutive blank lines.

### 4. UTF-8 Characters
The char-based matching should handle UTF-8 correctly, but byte offset calculation at boundaries might be off for multi-byte characters.

**Current implementation**: Uses `char_indices()` for correct byte offset tracking.

## Current Implementation Strengths

The existing `find_flexible_replacement_range()` already implements:

✅ **Two-lane matching**: Exact match first, then flexible fallback
✅ **Whitespace folding**: Consecutive whitespace collapsed to single space marker
✅ **Escape decoding**: `\t`, `\n`, `\r`, `\\` decoded before matching
✅ **UTF-8 safety**: Char-based matching with byte offset tracking via `char_indices()`
✅ **Ambiguity detection**: Rejects matches if exact and flexible lanes disagree
✅ **Fail-fast optimization**: Stops after 3 matches (can never be unique)

## Recommended Improvements

### 1. Enhanced Diagnostics (HIGH PRIORITY)

Add mismatch position reporting to help users understand WHY a match failed:

```rust
pub struct FindDiag {
    pub kind: FindDiagKind,
    pub mismatch_pos: Option<usize>,  // Char position where match failed
    pub expected: Option<char>,        // What character was expected
    pub found: Option<char>,           // What character was found
}
```

This would enable error messages like:
```
old_string not found in file.rs. Match failed at character 142: 
expected '\n' but found ' ' (whitespace difference). 
Try using collapse_whitespace: true or re-read the file with more context.
```

### 2. Boundary Whitespace Tolerance (MEDIUM PRIORITY)

Add optional trimming of leading/trailing whitespace from boundaries:

```rust
pub fn find_flexible_replacement_range_with_options(
    content: &str,
    needle: &str,
    new_str: &str,
    trim_boundaries: bool,  // NEW: allow boundary whitespace to differ
) -> Result<(usize, usize, String), FindError>
```

### 3. Blank Line Normalization (LOW PRIORITY)

Treat multiple consecutive blank lines as equivalent:

```rust
// In pattern folding, treat \n surrounded by whitespace as a blank line marker
// Allow 1+ blank lines to match 1+ blank lines flexibly
```

### 4. Default collapse_whitespace for edit/multi_edit (MEDIUM PRIORITY)

Consider making `collapse_whitespace: true` the DEFAULT for both `edit` and `multi_edit` tools, since the edit log shows 93% of failures involve whitespace issues. Users who need byte-exact matching can still use `patch` or `apply_patch`.

**Implementation**: Change default in tool parameter schemas from `false` to `true`.

### 5. Pre-match Whitespace Analysis (LOW PRIORITY)

Before attempting match, analyze the needle for common failure patterns and suggest fixes:

```rust
fn analyze_needle(needle: &str) -> NeedleAnalysis {
    NeedleAnalysis {
        has_leading_ws: needle.starts_with(char::is_whitespace),
        has_trailing_ws: needle.ends_with(char::is_whitespace),
        has_blank_lines: needle.contains("\n\n"),
        has_escapes: needle.contains('\\'),
        has_mixed_indent: detect_mixed_indentation(needle),
    }
}
```

## Implementation Status

### Completed ✅
- Documentation improvements added to `replace.rs` explaining the matching behavior
- Code compiles and all tests pass

### Recommended Next Steps
1. **Add diagnostic tracking** during flexible matching to report mismatch positions
2. **Consider defaulting collapse_whitespace to true** for edit/multi_edit tools
3. **Add integration tests** specifically for the failure patterns identified above
4. **Update AGENTS.md** to recommend using `collapse_whitespace: true` by default

## Testing Strategy

Add tests to `crates/ragent-tools-core/tests/test_edit_integration.rs`:

```rust
#[test]
fn test_edit_with_leading_trailing_whitespace() {
    // Test case from edit log: leading/trailing whitespace differences
}

#[test]
fn test_edit_with_blank_lines() {
    // Test case: blank lines with varying whitespace
}

#[test]
fn test_edit_with_escaped_whitespace() {
    // Test case: \t, \n, \r in needle vs actual whitespace in content
}

#[test]
fn test_edit_with_utf8_and_whitespace() {
    // Test case: UTF-8 characters combined with whitespace differences
}
```

## Conclusion

The current implementation is already robust - the two-lane matching with whitespace folding handles most cases correctly. The main improvements should focus on:

1. **Better diagnostics** to help users understand failures
2. **Sensible defaults** (collapse_whitespace=true by default)
3. **More comprehensive tests** covering the identified failure patterns

The edit log instrumentation is working correctly and provides valuable data for continuous improvement of the matching algorithms.
