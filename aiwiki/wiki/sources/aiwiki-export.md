---
title: "AiwikiExportTool: AIWiki Knowledge Base Export Tool for Agents"
source: "aiwiki_export"
type: source
tags: [rust, aiwiki, knowledge-management, export, obsidian, markdown, agent-tool, async, tool-framework]
generated: "2026-04-19T20:04:13.290828176+00:00"
---

# AiwikiExportTool: AIWiki Knowledge Base Export Tool for Agents

This document details the implementation of `AiwikiExportTool`, a Rust-based tool designed for exporting AIWiki knowledge bases to various formats within an agent framework. The tool provides functionality to export wiki content as either a single combined markdown file or as an Obsidian-compatible vault, enabling users to backup, share, or migrate their knowledge base content. The implementation follows a structured tool pattern with permission-based access control, input validation through JSON schemas, and comprehensive error handling for various edge cases such as uninitialized or disabled wiki states.

The tool integrates with a broader `aiwiki` crate ecosystem, leveraging its core data structures and export functions. It implements the `Tool` trait using `async_trait`, enabling asynchronous execution within the agent runtime. The design emphasizes user experience through detailed output messages, metadata-rich responses, and sensible defaults for output paths. Security considerations are addressed through a dedicated permission category (`aiwiki:read`) and validation that the wiki exists and is enabled before attempting export operations.

## Related

### Entities

- [AiwikiExportTool](../entities/aiwikiexporttool.md) — product
- [ragent-core](../entities/ragent-core.md) — technology
- [AIWiki](../entities/aiwiki.md) — technology

### Concepts

- [Knowledge Base Export](../concepts/knowledge-base-export.md)
- [Agent Tool Framework](../concepts/agent-tool-framework.md)
- [Obsidian Vault Structure](../concepts/obsidian-vault-structure.md)
- [Structured Tool Output](../concepts/structured-tool-output.md)

