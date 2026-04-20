---
title: "cosine_similarity"
entity_type: "technology"
type: entity
generated: "2026-04-19T21:55:53.400756460+00:00"
---

# cosine_similarity

**Type:** technology

### From: compact

Cosine similarity is a fundamental metric in vector space models that measures the cosine of the angle between two non-zero vectors, providing a normalized similarity score ranging from -1 (perfectly dissimilar) to 1 (identical direction). In the ragent memory system, this mathematical construct enables semantic deduplication by comparing high-dimensional embedding vectors that capture the meaning of memory content rather than surface lexical features. The implementation computes similarity between a query embedding generated from proposed memory content and stored embeddings retrieved from the database, with thresholds calibrated to 0.95 for automatic duplicate classification and 0.8 for near-duplicate detection. This approach excels at identifying paraphrased content, synonymous expressions, and conceptually equivalent memories that would evade keyword-based detection. The cosine similarity calculation is imported from the crate::memory::embedding module, suggesting integration with modern embedding models such as sentence-transformers or OpenAI's embedding API. The metric's normalization by vector magnitude ensures that document length differences don't disproportionately influence similarity scores, enabling fair comparison between memories of varying verbosity.

## External Resources

- [Mathematical foundation and applications of cosine similarity](https://en.wikipedia.org/wiki/Cosine_similarity) - Mathematical foundation and applications of cosine similarity
- [OpenAI embedding API documentation for semantic similarity applications](https://platform.openai.com/docs/guides/embeddings) - OpenAI embedding API documentation for semantic similarity applications

## Sources

- [compact](../sources/compact.md)
