---
title: "Environment Variable Injection for Inter-Process Communication"
type: concept
generated: "2026-04-19T21:28:55.054650139+00:00"
---

# Environment Variable Injection for Inter-Process Communication

### From: mod

The hooks module implements a structured environment variable protocol that serves as the primary communication channel between the ragent runtime and hook subprocesses. This design choice reflects practical constraints: shell commands are ubiquitous and language-agnostic, but they lack sophisticated IPC mechanisms. By convention, variables prefixed with RAGENT_ provide contextual information—RAGENT_TRIGGER identifies the lifecycle phase, RAGENT_WORKING_DIR establishes filesystem context, and tool-specific variables (RAGENT_TOOL_NAME, RAGENT_TOOL_INPUT, RAGENT_TOOL_OUTPUT, RAGENT_TOOL_SUCCESS) serialize structured data as JSON strings. The pattern elegantly handles the impedance mismatch between Rust's rich type system and shell's string-oriented environment: JSON encoding preserves complex tool argument structures, while the shell's string handling remains straightforward. Hook responses leverage stdout with JSON output, creating a simple request-response pattern for PreToolUse and PostToolUse hooks. This protocol design prioritizes simplicity and debuggability—users can test hooks independently by setting expected environment variables, and runtime inspection reveals complete communication state. The approach mirrors CGI protocol conventions and 12-factor app configuration principles.

## External Resources

- [12-Factor App methodology on configuration via environment](https://12factor.net/config) - 12-Factor App methodology on configuration via environment
- [W3C Common Gateway Interface specification](https://www.w3.org/CGI/) - W3C Common Gateway Interface specification

## Sources

- [mod](../sources/mod.md)
