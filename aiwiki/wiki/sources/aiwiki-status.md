---
title: "AiwikiStatusTool: AIWiki Status and Statistics Tool for Agents"
source: "aiwiki_status"
type: source
tags: [rust, ai-agent, knowledge-base, tooling, filesystem, statistics, async, tokio]
generated: "2026-04-19T20:00:04.149480086+00:00"
---

# AiwikiStatusTool: AIWiki Status and Statistics Tool for Agents

This document describes the `AiwikiStatusTool` Rust implementation, which provides comprehensive status reporting and statistics gathering for the AIWiki knowledge base system. The tool is designed as part of an agent-based architecture where AI agents can query the state of their associated knowledge base to understand its configuration, content volume, synchronization status, and storage utilization. The implementation follows a structured tool pattern with clear separation between initialization checks, statistics gathering, and formatted output generation.

The source code reveals a sophisticated statistics collection system that recursively traverses the wiki directory structure to categorize content into entities, concepts, sources, and analyses. It also monitors the raw file storage area, tracking both file counts and byte-level storage consumption. The tool implements human-readable formatting for technical metrics like file sizes, converting raw byte counts into appropriate units (KB, MB, GB, etc.). The asynchronous design using `tokio::fs` operations allows efficient I/O handling for potentially large directory trees.

From an architectural perspective, this tool exemplifies the integration pattern between agent tooling and knowledge management systems. It demonstrates how agents can introspect their own knowledge bases through well-defined interfaces, enabling self-awareness about available information resources. The permission category "aiwiki:read" indicates a security model where knowledge base access is gated, and the structured JSON metadata output supports both human-readable display and programmatic consumption by other system components.

## Related

### Entities

- [AiwikiStatusTool](../entities/aiwikistatustool.md) — technology
- [WikiStats](../entities/wikistats.md) — technology
- [Tokio](../entities/tokio.md) — technology

