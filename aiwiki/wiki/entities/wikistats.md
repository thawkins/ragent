---
title: "WikiStats"
entity_type: "technology"
type: entity
generated: "2026-04-19T20:00:04.150531022+00:00"
---

# WikiStats

**Type:** technology

### From: aiwiki_status

WikiStats is a private struct within the aiwiki_status.rs module that serves as the central data structure for aggregating comprehensive statistics about an AIWiki knowledge base instance. The struct captures quantitative metrics across multiple dimensions: content volume (total pages and category breakdowns), source material tracking (raw files and pending synchronization), storage consumption (wiki and raw directory sizes), and operational state (synchronization needs and last sync timestamp). This design reflects a holistic approach to knowledge base monitoring.

The struct's field design reveals the dual nature of the AIWiki architecture, which maintains both processed content in the wiki directory and original source materials in the raw directory. The category breakdown into entities, concepts, sources, and analyses suggests a semantic organization system where knowledge is classified by type rather than arbitrary folder structure. The inclusion of both `needs_sync` boolean and `last_sync` optional timestamp provides both immediate actionable state and historical context for synchronization operations.

As a data transfer object, WikiStats bridges the gap between raw filesystem operations and structured reporting. The struct is populated through the `gather_stats()` async function, which orchestrates multiple concurrent or sequential filesystem traversals to collect information. The design choice to make this struct private (not exported) indicates it's an implementation detail of the status tool rather than a public API, allowing the internal representation to evolve without breaking changes to external consumers.

## External Resources

- [Rust struct definitions and usage patterns](https://doc.rust-lang.org/rust-by-example/custom_types/structs.html) - Rust struct definitions and usage patterns

## Sources

- [aiwiki_status](../sources/aiwiki-status.md)
