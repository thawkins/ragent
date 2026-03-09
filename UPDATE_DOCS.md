# Documentation Updates Required

This document tracks missing docblocks for public functions that need documentation.

## Overview

Found **1** public function without proper documentation.

---

## Remediation Tasks

### 1. Add docblock to `redact_secrets()` in `crates/ragent-core/src/sanitize.rs`

**Location:** Line 11  
**Current Code:**
```rust
pub fn redact_secrets(msg: &str) -> String {
    SECRET_PATTERN.replace_all(msg, "[REDACTED]").into_owned()
}
```

**Remediation:**  
Add a documentation comment above the function explaining its purpose, parameters, and return value.

**Suggested docblock:**
```rust
/// Redacts sensitive information from a message string.
///
/// This function replaces API keys and authentication tokens with `[REDACTED]`
/// to prevent accidental exposure in logs or output. It detects patterns for:
/// - OpenAI-style keys (sk-[...])
/// - Generic API keys (key-[...])
/// - Bearer tokens (Bearer [...])
///
/// # Arguments
///
/// * `msg` - The message string potentially containing secrets
///
/// # Returns
///
/// A new `String` with all detected secrets replaced by `[REDACTED]`
///
/// # Examples
///
/// ```ignore
/// let secret = "My API key is sk-1234567890abcdefghij";
/// let redacted = redact_secrets(secret);
/// assert_eq!(redacted, "My API key is [REDACTED]");
/// ```
pub fn redact_secrets(msg: &str) -> String {
```

**Status:** Not started  
**Priority:** Low (only 1 function)

---

## Summary

- **Total public functions:** 22 documented
- **Functions without docs:** 1
- **Coverage:** 95.7%

All other public functions in the crate have proper documentation.
