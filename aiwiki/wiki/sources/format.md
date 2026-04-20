---
title: "ragent-core Tool Format Module: Output Formatting Utilities"
source: "format"
type: source
tags: [rust, formatting, i18n, pluralization, cli-tools, output-presentation, ragent, text-processing, path-handling, testing]
generated: "2026-04-19T16:38:35.794670979+00:00"
---

# ragent-core Tool Format Module: Output Formatting Utilities

This document describes the `format.rs` module from the `ragent-core` crate, a Rust library that provides comprehensive content formatting utilities for consistent tool output presentation. The module implements standardized formatting patterns designed to ensure uniform output across different tool types in the ragent ecosystem. It defines three primary formatting patterns: Pattern A for summary-plus-content output, Pattern B for simple summary-only results, and Pattern C for structured execution output with exit codes, timing, and stream data.

The module offers ten public functions covering diverse formatting needs. These include `format_summary_content` and `format_simple_summary` for basic content presentation, `format_status_output` for structured execution results, and a suite of pluralization-aware utilities (`format_bytes`, `format_line_count`, `format_file_count`, `format_match_count`, `format_entry_count`) that handle common counting scenarios with proper singular/plural forms. The `format_edit_summary` function provides specialized formatting for edit operations, showing before-and-after line counts with intelligent phrasing. Finally, `format_display_path` handles path presentation by making absolute paths relative to a working directory when possible.

The implementation demonstrates several Rust best practices including generic programming with `AsRef` trait bounds for flexible string and path handling, comprehensive documentation with examples, and thorough unit test coverage. Each formatting function includes edge case handling, such as empty content detection in `format_summary_content` and multiple conditional branches in `format_edit_summary` for different singular/plural combinations. The byte formatting uses binary prefixes (1024-based) with floating-point division for human-readable representations at different scales.

## Related

### Entities

- [ragent-core](../entities/ragent-core.md) — technology
- [std::path::Path](../entities/std-path-path.md) — technology

### Concepts

- [Content Format Patterns](../concepts/content-format-patterns.md)
- [Pluralization Logic](../concepts/pluralization-logic.md)
- [Human-Readable Formatting](../concepts/human-readable-formatting.md)
- [Generic Programming with AsRef](../concepts/generic-programming-with-asref.md)
- [Unit Testing Patterns](../concepts/unit-testing-patterns.md)

