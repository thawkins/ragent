---
title: "Non-Fatal Diagnostic Pattern"
type: concept
generated: "2026-04-19T15:00:25.378085646+00:00"
---

# Non-Fatal Diagnostic Pattern

### From: custom

The non-fatal diagnostic pattern is a resilience strategy that accumulates error information without terminating the overall operation, returning both successful results and failure details for caller disposition. This approach contrasts with fail-fast error handling, prioritizing partial success and user visibility over strict atomicity. In ragent's agent loading, each malformed file generates a descriptive string collected in a diagnostics vector while valid agents continue loading normally. This supports iterative development workflows where users can save partially-complete agent definitions and see specific validation errors without losing access to their entire agent library. The pattern requires careful API design: the return type becomes a tuple of (successes, failures) rather than Result, forcing callers to acknowledge both outcomes. Error messages emphasize human readability over machine parsing, including file paths and specific constraint violations. This design recognizes that configuration files are user-editable and errors are expected during development,不同于 runtime errors that might warrant immediate termination.

## External Resources

- [Rust Result type and error handling patterns](https://doc.rust-lang.org/std/result/enum.Result.html) - Rust Result type and error handling patterns
- [Rust API guidelines on error handling and reporting](https://rust-lang.github.io/api-guidelines/interoperability.html#c-ffi) - Rust API guidelines on error handling and reporting

## Sources

- [custom](../sources/custom.md)
