---
title: "Ragent Memory Compaction System: Deduplication, Block Management, and Stale Memory Eviction"
source: "compact"
type: source
tags: [rust, memory-management, deduplication, vector-similarity, cosine-similarity, fts5, sqlite, compaction, eviction, semantic-search, embedding, journal-logging, data-retention]
generated: "2026-04-19T21:55:53.398793355+00:00"
---

# Ragent Memory Compaction System: Deduplication, Block Management, and Stale Memory Eviction

This Rust source file implements Milestone 6 of the ragent memory system, providing comprehensive memory compaction functionality through three primary mechanisms: semantic deduplication, block size management, and stale memory eviction. The deduplication system employs a dual-strategy approach that first attempts cosine similarity matching on vector embeddings when an embedding provider is available, falling back to FTS5 keyword-based search when embeddings are unavailable or fail. This hybrid approach ensures robust duplicate detection across different deployment configurations while maintaining efficiency. The block compaction subsystem monitors memory block sizes against configurable limits, preserving YAML frontmatter while truncating overflow content and maintaining full audit trails through journal logging. Finally, the eviction system implements confidence-based memory lifecycle management, automatically identifying and removing memories that have decayed below threshold confidence levels after extended periods without access.

The architecture demonstrates sophisticated software engineering practices including configurable trigger-based execution, comprehensive audit logging through journaling, and graceful degradation between primary and fallback strategies. The system prioritizes data preservation by logging all original content before any destructive operations, enabling recovery and maintaining provenance. Configuration-driven behavior allows operators to tune compaction aggressiveness, confidence thresholds, and trigger conditions to match their specific use cases and storage constraints. The module integrates deeply with the broader ragent event system, publishing compaction events for monitoring and observability purposes.

## Related

### Entities

- [CompactionTrigger](../entities/compactiontrigger.md) — technology
- [DedupResult](../entities/dedupresult.md) — technology
- [FTS5](../entities/fts5.md) — technology
- [cosine_similarity](../entities/cosine-similarity.md) — technology
- [BlockScope](../entities/blockscope.md) — technology
- [FileBlockStorage](../entities/fileblockstorage.md) — technology
- [uuid::Uuid](../entities/uuid-uuid.md) — technology
- [chrono](../entities/chrono.md) — technology

### Concepts

- [Semantic Deduplication](../concepts/semantic-deduplication.md)
- [Memory Block Compaction](../concepts/memory-block-compaction.md)
- [Confidence-Based Memory Decay](../concepts/confidence-based-memory-decay.md)
- [Graceful Degradation in AI Systems](../concepts/graceful-degradation-in-ai-systems.md)
- [Event-Driven Observability](../concepts/event-driven-observability.md)

