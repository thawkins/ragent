---
title: "StrReplaceEditorTool: Claude-Compatible Multi-Command File Editor"
source: "str_replace_editor"
type: source
tags: [rust, file-editing, ai-tools, claude-compatibility, anthropic, async-rust, tool-system, code-generation, filesystem-operations, trait-implementation]
generated: "2026-04-19T20:11:43.959448568+00:00"
---

# StrReplaceEditorTool: Claude-Compatible Multi-Command File Editor

This Rust source file implements `StrReplaceEditorTool`, a file editing utility designed for compatibility with Anthropic's Claude AI models. The tool provides a unified interface for five distinct file operations: viewing file contents with optional line ranges, creating new files, performing exact string replacements, inserting text at specific line positions, and deleting line ranges. The implementation demonstrates sophisticated software engineering practices including command delegation patterns, parameter validation, and security-conscious path resolution. Each command handler either delegates to specialized tools from the same module or implements custom logic with careful attention to edge cases like trailing newline preservation and line number validation.

The architecture follows a clear separation of concerns where the main `StrReplaceEditorTool` struct implements the `Tool` trait, defining the contract that AI models expect. The tool's `execute` method acts as a router, dispatching to appropriate handlers based on the `command` parameter. Notable implementation details include the `handle_create` function's intelligent fallback between CreateTool and WriteTool depending on file existence, the `handle_str_replace` function's proactive validation to catch a common model error (omitting `old_str`), and the manual line manipulation in `handle_insert` and `handle_delete` that carefully preserves file formatting. Security is addressed through `check_path_within_root` calls that prevent directory traversal attacks.

The code reveals important insights about AI tool design: the explicit warnings in the description about `old_str` requirements, the parameter aliasing (`file_text` to `content`, `new_str` as fallback for `new_str_insert`), and the JSON schema that guides model behavior. This tool exemplifies how software can bridge the gap between AI model outputs and precise file system operations, handling the messy reality that models may produce slightly incorrect calls while maintaining strict safety guarantees.

## Related

### Entities

- [StrReplaceEditorTool](../entities/strreplaceeditortool.md) — technology
- [Anthropic Claude](../entities/anthropic-claude.md) — product
- [Tokio](../entities/tokio.md) — technology

