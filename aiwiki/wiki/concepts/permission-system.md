---
title: "Permission System"
type: concept
generated: "2026-04-18T15:03:19.532987941+00:00"
---

# Permission System

### From: LSP_INTEGRATION_GUIDE

Category-based permission checking with user prompts via PermissionRequested/PermissionReplied events. Tools declare permission categories like 'file:read' that are validated before execution.

## Sources

- [LSP_INTEGRATION_GUIDE](../sources/lsp-integration-guide.md)

### From: LSP_QUICK_REFERENCE

Fine-grained access control where tools declare permission categories (e.g., 'code:query', 'file:read', 'bash:execute'). Supports Ask/Allow/Deny modes with user approval via events

### From: O365_TOOL

Access control system for ragent tools; Office tools use file:read and file:write permissions

### From: roo_code_research

Granular access controls requiring approval for actions by default, with auto-approve options for trusted operations and safety guardrails
