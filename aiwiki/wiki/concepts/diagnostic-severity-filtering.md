---
title: "Diagnostic Severity Filtering"
type: concept
generated: "2026-04-19T18:22:18.102886825+00:00"
---

# Diagnostic Severity Filtering

### From: lsp_diagnostics

Diagnostic severity filtering is a critical capability implemented in LspDiagnosticsTool that enables selective analysis of code issues based on their perceived importance. The LSP specification defines four severity levels in inverse numeric order: Error (1), Warning (2), Information (3), and Hint (4), where lower numbers represent more critical issues that typically prevent successful compilation or indicate serious problems. The tool's filtering system interprets user-specified minimum severity levels and includes all diagnostics of that severity or higher criticality, creating an intuitive interface where requesting "warning" level includes both warnings and errors, while "error" level shows only blocking issues.

The implementation of severity filtering in the source code demonstrates sophisticated handling of the LSP severity ranking system. Because DiagnosticSeverity wraps a u32 without exposing it directly, the code defines a custom `rank` closure that maps each severity variant to an explicit u8 ranking value. This enables clean comparison logic in the `severity_passes` function, which determines whether an actual diagnostic severity meets or exceeds a user-specified minimum threshold. The design choice to treat unknown severities conservatively (rank 5, lowest priority) ensures graceful degradation when encountering non-standard severity values from non-compliant language servers.

The practical significance of severity filtering in AI agent workflows cannot be overstated. When agents analyze large codebases, unfiltered diagnostic output can overwhelm reasoning capabilities with thousands of minor style suggestions and informational notes. By allowing agents to focus initially on errors that block compilation, then progressively expand to warnings and hints, the filtering system supports staged code improvement strategies that mirror human developer behavior. This graduated approach is particularly valuable in automated refactoring scenarios where an agent must first establish code correctness before addressing stylistic concerns. The "all" filter option preserves complete visibility for comprehensive analysis when appropriate.

The severity filtering system also enables intelligent automation policies where different agent behaviors trigger based on diagnostic severity thresholds. For example, a deployment preparation agent might gate releases on zero errors while logging warnings for review, while a code quality agent might prioritize eliminating all warnings in critical security-sensitive modules. The structured JSON output containing severity information allows downstream systems to implement such sophisticated policies without re-parsing human-readable text. This machine-readable representation of code health supports data-driven decision making about code quality investments and technical debt management.

## External Resources

- [LSP Diagnostic structure and severity definitions](https://microsoft.github.io/language-server-protocol/specifications/specification-current/#diagnostic) - LSP Diagnostic structure and severity definitions
- [Rust DiagnosticSeverity enum documentation](https://docs.rs/lsp-types/latest/lsp_types/enum.DiagnosticSeverity.html) - Rust DiagnosticSeverity enum documentation

## Related

- [Language Server Protocol (LSP)](language-server-protocol-lsp.md)

## Sources

- [lsp_diagnostics](../sources/lsp-diagnostics.md)
