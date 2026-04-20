---
title: "substitute_args"
entity_type: "technology"
type: entity
generated: "2026-04-19T20:17:24.138512617+00:00"
---

# substitute_args

**Type:** technology

### From: args

The `substitute_args` function serves as the primary public API for argument substitution within the ragent skill system. This function takes four parameters: the raw skill body text, the argument string provided during skill invocation, a session identifier, and the absolute path to the skill's directory. It returns a fully processed String with all placeholder variables replaced by their corresponding values. The function's design reflects careful consideration of substitution ordering to prevent incorrect partial replacements—specifically, it processes braced environment variables first, then indexed argument patterns, then the complete arguments string, and finally positional shorthand references. This ordering ensures that `$ARGUMENTS[0]` is fully resolved before `$ARGUMENTS` could incorrectly match just the `$ARGUMENTS` prefix.

The function's implementation demonstrates idiomatic Rust patterns including the `#[must_use]` attribute to warn callers who might accidentally discard the result, comprehensive documentation with examples, and efficient string handling. The use of `body.to_string()` creates an owned String that can be progressively modified, while helper functions handle the more complex parsing logic for indexed and positional patterns. The function gracefully handles edge cases including empty arguments, out-of-bounds indices (returning empty strings), and multiline content preservation. Its integration with the broader system is evident through its use of `Path::display()` for cross-platform path stringification and its role as the final transformation step before skill body execution.

## External Resources

- [Rust must_use attribute documentation](https://doc.rust-lang.org/reference/attributes/diagnostics.html#the-must_use-attribute) - Rust must_use attribute documentation
- [Rust Path handling examples](https://doc.rust-lang.org/rust-by-example/std_misc/path.html) - Rust Path handling examples

## Sources

- [args](../sources/args.md)
