---
title: "Tool System Architecture"
type: concept
generated: "2026-04-18T14:47:32.732570846+00:00"
---

# Tool System Architecture

### From: README_LSP_EXPLORATION

Pattern-based system where tools implement the Tool trait with name(), permission_category(), and async execute() methods. Tools are registered in ToolRegistry and receive ToolContext with session id, working directory, event bus, storage, and task manager. ToolOutput contains content string and optional metadata JSON.

## Related

- [permission gating](permission-gating.md)

## Sources

- [README_LSP_EXPLORATION](../sources/readme-lsp-exploration.md)

### From: LSP_INTEGRATION_GUIDE

Pattern based on Tool trait with name, description, JSON schema parameters, permission category, and async execute method. Tools are registered in a ToolRegistry and executed with ToolContext containing session ID, working directory, event bus, storage, and task manager.
