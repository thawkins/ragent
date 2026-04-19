---
title: "Tool System"
type: concept
generated: "2026-04-18T15:03:50.973475634+00:00"
---

# Tool System

### From: LSP_QUICK_REFERENCE

Extensible framework where tools implement the Tool trait with name, parameters_schema, permission_category, and execute methods. Tools are registered in ToolRegistry and invoked by the LLM

## Related

- [Permission System](permission-system.md)

## Sources

- [LSP_QUICK_REFERENCE](../sources/lsp-quick-reference.md)

### From: README_LSP_EXPLORATION

Extensible architecture based on Tool trait with ToolRegistry for management, ToolContext for execution environment, and ToolOutput for results. Supports async execution and permission gating

### From: introduction

Comprehensive framework of built-in, extended, and sub-agent tools for performing file operations, web integration, task management, and more
