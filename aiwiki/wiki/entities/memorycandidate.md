---
title: "MemoryCandidate"
entity_type: "technology"
type: entity
generated: "2026-04-19T21:58:03.993293542+00:00"
---

# MemoryCandidate

**Type:** technology

### From: extract

MemoryCandidate serves as the primary data structure for proposed knowledge entries within the ragent memory system, representing the intermediate state between raw extraction and persistent storage. Unlike the finalized `StructuredMemory` entries stored in the database, MemoryCandidate instances encapsulate extracted insights awaiting confirmation decisions. The struct's design reflects careful attention to metadata richness, incorporating fields for categorical classification (`fact`, `pattern`, `preference`, `insight`, `error`, `workflow`), confidence scoring (0.0-1.0 continuous scale), provenance tracking through source attribution, and human-readable reasoning documentation. This comprehensive metadata enables sophisticated downstream processing, filtering, and prioritization workflows while maintaining transparency about the extraction's origin and reliability.

The categorical system implemented in MemoryCandidate represents a deliberate ontology for knowledge organization. The `pattern` category captures recurring solutions and conventions, such as project-specific code organization or framework usage patterns. The `error` category specifically documents problem-solution pairs, particularly valuable for preserving debugging workflows and preventing repeated troubleshooting. The `insight` category accommodates higher-level learnings about system behavior, architectural decisions, or domain understanding. Preferences capture user or project-specific choices, while `fact` stores verifiable statements and `workflow` documents procedural knowledge. This multi-dimensional classification enables precise retrieval and contextual presentation of relevant memories during future agent operations.

The confirmation flow architecture distinguishes MemoryCandidate from simpler logging mechanisms, implementing a deliberate human-agent collaborative validation layer. When `require_confirmation` is enabled (the default configuration), candidates generate events rather than direct database mutations, allowing explicit review before incorporation into the structured memory store. This design acknowledges the inherent uncertainty in automated extraction—confidence scores may not perfectly reflect utility, and context sensitivity may lead to extraction of transient rather than durable learnings. The optional auto-store pathway, activated when confirmation is disabled, supports high-trust deployments or batch processing scenarios where manual review is impractical. The struct's serialization support through `serde` ensures seamless integration with event buses, storage systems, and potential cross-service communication, positioning MemoryCandidate as a foundational interoperability primitive in the broader agent ecosystem.

## External Resources

- [Serde serialization framework used for MemoryCandidate serialization](https://serde.rs/) - Serde serialization framework used for MemoryCandidate serialization
- [Chrono date/time library for timestamp handling in related structures](https://docs.rs/chrono/latest/chrono/) - Chrono date/time library for timestamp handling in related structures

## Sources

- [extract](../sources/extract.md)
