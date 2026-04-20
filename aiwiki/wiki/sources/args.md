---
title: "Argument Substitution Module for Ragent Skill System"
source: "args"
type: source
tags: [rust, parsing, string-substitution, cli-arguments, skill-system, template-engine, ragent, tokenization, text-processing]
generated: "2026-04-19T20:17:24.135915519+00:00"
---

# Argument Substitution Module for Ragent Skill System

This Rust source file implements argument parsing and substitution functionality for the ragent-core skill system. The module provides two primary public functions: `substitute_args` for replacing placeholder variables in skill bodies with actual argument values and environment information, and `parse_args` for tokenizing raw argument strings with support for quoted strings. The substitution system supports five variable types: `$ARGUMENTS` for all arguments as a single string, `$ARGUMENTS[N]` for indexed argument access, `$N` shorthand for positional arguments, `${RAGENT_SESSION_ID}` for session identification, and `${RAGENT_SKILL_DIR}` for skill directory paths. The implementation demonstrates careful attention to parsing order and edge cases, with comprehensive test coverage including 22 test functions that validate behavior across empty inputs, whitespace handling, quoted strings, out-of-bounds indices, and multiline preservation.

The code architecture follows a layered substitution approach where longer patterns are replaced before shorter ones to prevent partial match issues. The `substitute_args` function orchestrates the replacement process in four sequential phases: first replacing braced environment variables, then indexed argument patterns, followed by the full arguments string, and finally positional shorthand references. The argument parser implements a character-by-character tokenizer using Rust's `Peekable` iterator pattern to handle whitespace-separated tokens and both single and double-quoted strings without external dependencies. The module includes two private helper functions—`substitute_indexed_args` and `substitute_positional_shorthand`—that handle the more complex pattern matching required for indexed access and positional references respectively.

## Related

### Entities

- [ragent-core](../entities/ragent-core.md) — technology
- [substitute_args](../entities/substitute-args.md) — technology
- [parse_args](../entities/parse-args.md) — technology

### Concepts

- [Template Substitution](../concepts/template-substitution.md)
- [Shell-Style Argument Parsing](../concepts/shell-style-argument-parsing.md)
- [Defensive Programming](../concepts/defensive-programming.md)
- [Zero-Copy and Efficient String Processing](../concepts/zero-copy-and-efficient-string-processing.md)

