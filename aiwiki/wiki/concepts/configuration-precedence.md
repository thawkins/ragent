---
title: "Configuration Precedence"
type: concept
generated: "2026-04-18T14:47:32.740877643+00:00"
---

# Configuration Precedence

### From: README_LSP_EXPLORATION

Multi-level config loading: global (~/.config/ragent/ragent.json) → project (./ragent.json) → environment variables → inline JSON. MCP servers already configurable; LSP would follow same pattern with per-server settings and environments.

## Sources

- [README_LSP_EXPLORATION](../sources/readme-lsp-exploration.md)

### From: LSP_QUICK_REFERENCE

Hierarchical config loading: compiled defaults → ~/.config/ragent/ragent.json → ./ragent.json → $RAGENT_CONFIG env var → $RAGENT_CONFIG_CONTENT env var
