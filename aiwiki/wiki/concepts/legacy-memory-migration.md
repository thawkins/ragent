---
title: "Legacy Memory Migration"
type: concept
generated: "2026-04-19T21:36:42.616912751+00:00"
---

# Legacy Memory Migration

### From: storage

Legacy memory migration is a backward compatibility mechanism that enables smooth transition from older ragent versions that used a simpler MEMORY.md file format without structured block metadata. The `load_legacy_memory` function provides this bridge by detecting and loading legacy MEMORY.md files as proper MemoryBlock instances with synthesized metadata. This migration path is crucial for user experience, preventing data loss when users upgrade their tooling while maintaining the structural benefits of the new block-based system.

The legacy format consisted of plain Markdown content in a file named MEMORY.md without YAML frontmatter, lacking the label, description, and scope metadata that the new system expects. When `load_legacy_memory` is called, it resolves the appropriate memory directory for the given scope, checks for the existence of MEMORY.md, and if found, reads the entire file content. Empty files are filtered out, and non-existent files return None. Valid legacy files are converted to MemoryBlock instances with a fixed label of "MEMORY", the provided scope, and the file content as the block content. This allows the rest of the system to treat legacy data uniformly with new block data.

This migration strategy demonstrates pragmatic software engineering: rather than requiring users to manually migrate data or breaking existing workflows, the system transparently upgrades legacy data on access. The approach has limitations—legacy files don't support descriptions, content limits, or the rich metadata of full blocks—but it preserves valuable user content. The function is called in appropriate initialization contexts (implied by its standalone nature), and tests validate both successful migration and graceful handling when no legacy file exists. Over time, as users resave migrated content through the normal block interface, the data naturally transitions to the new format without explicit migration tooling.

## External Resources

- [Strangler Fig pattern for gradual system migration](https://martinfowler.com/bliki/StranglerFigApplication.html) - Strangler Fig pattern for gradual system migration
- [Joel Spolsky on the importance of backward compatibility](https://www.joelonsoftware.com/2000/04/06/things-you-should-never-do-part-i/) - Joel Spolsky on the importance of backward compatibility

## Sources

- [storage](../sources/storage.md)
