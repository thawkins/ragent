---
title: "Memory Decay and Compaction"
type: concept
generated: "2026-04-19T15:06:38.736551606+00:00"
---

# Memory Decay and Compaction

### From: mod

Memory decay and compaction implement biological-inspired mechanisms for managing unbounded context growth in persistent agent systems. The `DecayConfig` applies Ebbinghaus forgetting curve principles to artificial memory, where confidence values degrade over time unless reinforced through retrieval or update operations. This prevents context accumulation from overwhelming the retrieval system with obsolete information, while preserving important memories through usage-based reinforcement. The `decay_min_confidence` threshold establishes a floor below which memories become candidates for eviction, creating natural lifecycle management without explicit deletion operations.

The `CompactionConfig` addresses fragmentation in semantic memory, where related information may scatter across multiple stored fragments created during different interaction sessions. Compaction aggregates semantically similar memories, consolidating redundant information and extracting higher-level abstractions—analogous to how human sleep is hypothesized to consolidate declarative memories. The `block_size_limit` and `memory_count_threshold` parameters control compaction triggering, balancing processing overhead against retrieval efficiency. The `min_interval_hours` prevents excessive compaction that might destabilize the memory landscape or consume disproportionate computational resources.

`EvictionConfig` provides the ultimate backstop for memory management, removing memories that have fallen below confidence thresholds or exceeded staleness duration. The distinction between automatic and manual eviction modes respects user autonomy—some deployments may prefer human review of memory deletion, particularly in regulated industries with audit requirements. The `eviction_stale_days` and `eviction_min_confidence` parameters create multi-dimensional eviction criteria, ensuring that neither temporal staleness nor confidence degradation alone determines memory survival. Together, these mechanisms create a self-tuning memory system that approximates optimal relevance through distributed algorithms rather than centralized optimization, appropriate for the stochastic nature of human-agent interaction patterns.

## External Resources

- [Ebbinghaus forgetting curve and memory retention theory](https://en.wikipedia.org/wiki/Forgetting_curve) - Ebbinghaus forgetting curve and memory retention theory
- [Sleep and memory consolidation research](https://en.wikipedia.org/wiki/Sleep_and_memory) - Sleep and memory consolidation research
- [Cache eviction algorithms and policies](https://en.wikipedia.org/wiki/Cache_replacement_policies) - Cache eviction algorithms and policies
- [Neural network approaches to memory management in LLMs](https://arxiv.org/abs/2208.03299) - Neural network approaches to memory management in LLMs

## Sources

- [mod](../sources/mod.md)
