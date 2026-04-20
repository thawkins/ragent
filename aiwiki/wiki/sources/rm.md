---
title: "Ragent-Core RmTool: Secure Single File Deletion Tool"
source: "rm"
type: source
tags: [rust, file-system, security, async, tool-framework, agent-system, delete-operation]
generated: "2026-04-19T16:23:42.241696983+00:00"
---

# Ragent-Core RmTool: Secure Single File Deletion Tool

This source file implements `RmTool`, a Rust-based file deletion tool designed for secure, controlled file removal within an agent-based system. The tool provides a carefully constrained interface that explicitly rejects wildcard patterns and glob characters to prevent accidental or malicious mass deletions. It implements the `Tool` trait from the ragent-core framework, exposing itself as a JSON-callable tool with schema-defined parameters and permission categorization.

The implementation demonstrates several important security patterns for file system operations in agent contexts. Path resolution combines absolute and relative path handling with a mandatory root containment check via `check_path_within_root`, preventing directory traversal attacks. The tool validates that the target exists and is a regular file (not a directory) before attempting deletion, providing clear error messages for each failure case. Execution uses Tokio's asynchronous file system operations for non-blocking I/O.

The tool follows modern Rust practices with comprehensive error handling through `anyhow`, structured JSON parameter validation via `serde_json`, and async/await patterns. Its design reflects lessons from Unix philosophy—doing one thing well—while adding safety constraints appropriate for automated systems where unintended deletions could have severe consequences. The permission category "file:write" enables coarse-grained access control in larger agent orchestration systems.

## Related

### Entities

- [RmTool](../entities/rmtool.md) — technology
- [Tokio](../entities/tokio.md) — technology
- [serde_json](../entities/serde-json.md) — technology
- [anyhow](../entities/anyhow.md) — technology

