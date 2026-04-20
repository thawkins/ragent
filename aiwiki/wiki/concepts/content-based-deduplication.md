---
title: "Content-Based Deduplication"
type: concept
generated: "2026-04-19T21:58:03.995190065+00:00"
---

# Content-Based Deduplication

### From: extract

Content-based deduplication in the ExtractionEngine implements a multi-strategy approach to preventing redundant memory storage, combining cryptographic hashing for exact match detection with natural language similarity metrics for near-duplicate identification. This concept addresses the fundamental challenge that automatic extraction systems, operating continuously across sessions, will inevitably encounter similar patterns, repeated errors, and recurring conventions that should not generate duplicate memory entries. The implementation provides two primary deduplication interfaces: `is_duplicate` which checks against both proposed candidate hashes and existing storage, and `is_duplicate_content` which specifically queries the structured memory store. This layered architecture enables both immediate intra-session deduplication (preventing the same extraction from being proposed multiple times) and cross-session deduplication (recognizing when a new extraction substantially overlaps with existing memories).

The hashing strategy employs `std::hash::Hasher` with `hash_slice` on content bytes to generate `u64` identifiers, providing fast exact-match detection with manageable collision risk for the intended application. The `proposed_hashes` field in ExtractionEngine maintains a `HashSet<u64>` of already-proposed content hashes, enabling O(1) lookup for intra-session duplicates. This stateful tracking complements the external storage query by capturing candidates that have been proposed but not yet confirmed or stored, addressing the temporal gap between extraction and persistence decisions. The word overlap algorithm, implemented in `word_overlap`, provides fuzzy similarity detection for cases where content variations (different phrasing, additional context, minor edits) prevent exact hash matching while still representing semantically equivalent information.

The word overlap implementation tokenizes content into words, filters English stop words (common terms like "the", "and", "of" that provide little discriminative value), and computes the Jaccard-like ratio of intersection to maximum set size. This approach balances computational efficiency with semantic sensitivity, recognizing that code-related content often contains significant overlapping vocabulary (variable names, function calls, framework terms) that should trigger deduplication even when surrounding narrative differs. The stop word filtering, implemented in `is_stop_word`, improves precision by focusing similarity assessment on content-bearing terms. The confidence decay function (`decay_confidence`) complements deduplication by enabling temporal relevance management—rather than hard deletion, memories gradually reduce in confidence based on age and access patterns, allowing effective "soft deduplication" where dated entries diminish in retrieval priority without requiring explicit removal decisions. Together, these mechanisms demonstrate that effective deduplication in knowledge systems requires both exact and approximate matching strategies, stateful tracking of extraction lifecycle stages, and temporal relevance modeling to maintain memory store quality over extended operation.

## External Resources

- [Similarity measures for text and document comparison](https://en.wikipedia.org/wiki/Similarity_measure) - Similarity measures for text and document comparison
- [Jaccard index and set similarity measures](https://en.wikipedia.org/wiki/Jaccard_index) - Jaccard index and set similarity measures
- [Locality-sensitive hashing for approximate nearest neighbor search](https://en.wikipedia.org/wiki/Locality-sensitive_hashing) - Locality-sensitive hashing for approximate nearest neighbor search
- [Stop words in information retrieval and natural language processing](https://en.wikipedia.org/wiki/Stop_words) - Stop words in information retrieval and natural language processing

## Sources

- [extract](../sources/extract.md)
