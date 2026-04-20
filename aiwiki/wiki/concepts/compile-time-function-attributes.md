---
title: "Compile-Time Function Attributes"
type: concept
generated: "2026-04-19T22:05:36.474377437+00:00"
---

# Compile-Time Function Attributes

### From: intern

The module makes strategic use of Rust's compile-time function attributes to encode important semantic information and improve developer experience. The `#[must_use]` attribute appears on all public functions that return values without side effects, indicating that ignoring the return value almost certainly represents a bug. This attribute causes the compiler to emit warnings when callers don't explicitly handle return values, catching mistakes like calling `intern("value")` without storing the resulting symbol.

This pattern reflects Rust's philosophy of using the type system and compiler attributes to prevent common errors. The `#[must_use]` attribute is particularly valuable for pure functions like `intern()`, `resolve()`, `len()`, and `is_empty()` where the entire purpose is to compute and return a value. Without this attribute, a developer might mistakenly write `intern("tool_name");` thinking it performs some registration, when in fact the symbol handle is discarded and the operation has no observable effect.

The module also uses standard documentation conventions including `# Examples` sections in doc comments, enabling `rustdoc` to compile and test example code. The test module is conditionally compiled with `#[cfg(test)]`, ensuring test code doesn't bloat production binaries. These patterns collectively demonstrate mature Rust development practices where compiler-enforced correctness, comprehensive documentation, and efficient compilation are all given careful attention.

## External Resources

- [Rust reference on must_use attribute](https://doc.rust-lang.org/reference/attributes/diagnostics.html#the-must_use-attribute) - Rust reference on must_use attribute
- [Rust documentation testing guide](https://doc.rust-lang.org/rustdoc/documentation-tests.html) - Rust documentation testing guide

## Related

- [Defensive Programming](defensive-programming.md)

## Sources

- [intern](../sources/intern.md)
