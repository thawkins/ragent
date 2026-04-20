---
title: "Semantic Deduplication"
type: concept
generated: "2026-04-19T21:55:53.403357167+00:00"
---

# Semantic Deduplication

### From: compact

Semantic deduplication represents an advanced approach to identifying duplicate information that transcends exact textual matching to recognize equivalent meaning expressed through different surface forms. Unlike syntactic deduplication that requires byte-for-byte identity or simple normalizations, semantic techniques leverage vector embeddings that capture latent semantic relationships in high-dimensional space. The ragent implementation exemplifies this through its dual-threshold strategy: cosine similarity above 0.95 triggers automatic merging as functionally identical content, while the 0.8-0.95 range flags near-duplicates requiring human judgment. This graduated response acknowledges the inherent uncertainty in semantic similarity and the varying consequences of false positives versus false negatives in different application contexts.

The practical implementation demonstrates important engineering tradeoffs between accuracy and operational complexity. When embedding services are unavailable, the system gracefully degrades to FTS5 keyword overlap, maintaining functionality while accepting reduced semantic sensitivity. This resilience pattern ensures continuous operation across network partitions, service outages, or resource-constrained environments. The merge_content and merge_tags functions implement conservative union semantics that preserve all unique information from both source memories, preventing data loss while eliminating redundancy. This approach contrasts with more aggressive deduplication strategies that might select representative samples or synthesize abstractions, prioritizing completeness over compression ratio.

## External Resources

- [Sentence-BERT: Sentence embeddings using Siamese BERT-networks](https://arxiv.org/abs/1908.10084) - Sentence-BERT: Sentence embeddings using Siamese BERT-networks
- [Microsoft Research on dual-encoder architectures for semantic matching](https://www.microsoft.com/en-us/research/publication/dual-encoder-transformer/) - Microsoft Research on dual-encoder architectures for semantic matching

## Related

- [cosine_similarity](cosine-similarity.md)

## Sources

- [compact](../sources/compact.md)
