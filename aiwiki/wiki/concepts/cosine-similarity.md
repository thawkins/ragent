---
title: "Cosine Similarity"
type: concept
generated: "2026-04-19T21:45:52.710905193+00:00"
---

# Cosine Similarity

### From: embedding

Cosine similarity is a fundamental metric in vector space models that measures the cosine of the angle between two non-zero vectors, providing a normalized measure of their directional alignment. In the context of text embeddings, this metric quantifies semantic similarity by comparing the orientations of high-dimensional vectors produced by neural networks. The implementation in this module computes the dot product of two vectors divided by the product of their L2 norms, yielding a value in the range [-1.0, 1.0] where 1.0 indicates identical direction (perfect similarity), 0.0 indicates orthogonality (no similarity), and -1.0 indicates opposite directions. This normalization makes cosine similarity invariant to vector magnitude, ensuring that document length doesn't artificially inflate similarity scores.

The mathematical properties of cosine similarity make it particularly well-suited for semantic search applications. Unlike Euclidean distance, which conflates magnitude and direction, cosine similarity focuses purely on directional alignment, capturing the intuitive notion that two texts are similar if they point in the same semantic direction in embedding space. The module's implementation includes important edge case handling: when either vector has zero magnitude (all zeros), the function returns 0.0 rather than triggering division by zero. This graceful handling supports the no-op embedding scenario where empty vectors propagate through the system without causing panics.

The computational approach leverages Rust's iterator adapters for clarity and potential auto-vectorization. The implementation uses `zip` to pair corresponding elements, `map` for element-wise multiplication, and `sum` for reduction, expressing the dot product in functional style. Norm calculations similarly use iterator chains with `map` for squaring and `sum` followed by `sqrt`. The assertion of equal lengths documents a critical precondition while providing clear error messages on violation. This implementation balances mathematical correctness, performance, and code clarity, serving as the scoring function for the entire semantic search pipeline.

## External Resources

- [Wikipedia article on cosine similarity with mathematical foundations](https://en.wikipedia.org/wiki/Cosine_similarity) - Wikipedia article on cosine similarity with mathematical foundations
- [Tutorial on cosine similarity in NLP applications](https://www.machinelearningplus.com/nlp/cosine-similarity/) - Tutorial on cosine similarity in NLP applications

## Sources

- [embedding](../sources/embedding.md)
