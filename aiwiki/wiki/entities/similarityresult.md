---
title: "SimilarityResult"
entity_type: "technology"
type: entity
generated: "2026-04-19T21:45:52.708337621+00:00"
---

# SimilarityResult

**Type:** technology

### From: embedding

SimilarityResult is a data structure designed to represent scored outcomes from semantic search operations within the ragent memory system. This struct pairs a database row identifier with a cosine similarity score, enabling ranked retrieval of memory or journal entries based on their semantic proximity to a query. The design follows the pattern of search result tuples common in information retrieval systems, where each candidate item is annotated with a relevance metric for ranking purposes. The row_id field uses i64 to accommodate SQLite's 64-bit rowid type, ensuring compatibility with the underlying storage layer without truncation risks.

The score field holds a 32-bit floating-point value in the range [-1.0, 1.0], where higher values indicate stronger semantic similarity between the query and the retrieved document. This range directly corresponds to the output of the cosine_similarity function, creating a coherent type system where mathematical operations and result structures are aligned. The struct derives Debug and Clone traits, supporting standard Rust patterns for logging, inspection, and result duplication across async boundaries. The choice of f32 over f64 for scores reflects practical considerations about storage efficiency and the inherent noise floor in neural embedding similarity measurements, where precision beyond single-precision floats provides diminishing returns.

SimilarityResult serves as the interchange format between the embedding computation layer and higher-level search interfaces. Collections of SimilarityResult instances can be sorted by score to produce ranked result lists, filtered by threshold to exclude low-relevance matches, or joined with full-text search scores for hybrid ranking algorithms. The structure's simplicity—just two fields—belies its importance in the semantic search pipeline, as it bridges the gap between raw vector mathematics and human-meaningful search outcomes. The clear semantic meaning of each field (identity and relevance) makes the struct self-documenting and reduces cognitive load when interpreting search results in downstream code.

## Sources

- [embedding](../sources/embedding.md)
