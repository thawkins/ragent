---
title: "MemoryExport"
entity_type: "technology"
type: entity
generated: "2026-04-19T21:59:33.768442328+00:00"
---

# MemoryExport

**Type:** technology

### From: import_export

MemoryExport is the primary data structure defining ragent's portable memory export format, serving as the top-level container for all exportable memory data. This struct implements Rust's Serialize and Deserialize traits from serde, enabling seamless JSON serialization for data portability. The format is explicitly versioned using semantic versioning (starting at 1.0), includes ISO 8601 timestamp tracking via RFC 3339 format, and identifies its source application—design decisions that support future format evolution and provenance tracking.

The structure organizes memory data into three distinct categories reflecting ragent's hybrid storage architecture. The memories field contains a vector of StructuredMemory objects representing the SQLite-persisted semantic memory entries with full metadata including content, category, confidence scores, source attribution, project context, session identifiers, and tag associations. The journal field preserves chronological JournalEntry records capturing development session logs and progress tracking. The blocks field uses a nested MemoryBlocksExport structure to organize file-based storage hierarchically by scope (project versus global), with each scope containing a HashMap mapping string labels to string content.

Design decisions in MemoryExport reflect production concerns for data migration systems. The serde(default) attributes on collection fields ensure backward compatibility when deserializing exports that may lack certain data categories. The explicit source field enables multi-tool ecosystem recognition, supporting future scenarios where exports might originate from compatible third-party tools. The flat memory vector structure (rather than hierarchical organization) simplifies export logic while preserving all relational information through embedded metadata fields, enabling faithful reconstruction of complex memory relationships during import operations.

## External Resources

- [Serde Rust serialization framework documentation](https://serde.rs/) - Serde Rust serialization framework documentation
- [RFC 3339: Date and Time on the Internet - Timestamps](https://www.rfc-editor.org/rfc/rfc3339) - RFC 3339: Date and Time on the Internet - Timestamps

## Sources

- [import_export](../sources/import-export.md)
