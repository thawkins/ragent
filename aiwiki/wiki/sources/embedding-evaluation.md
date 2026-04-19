---
title: "Embedding Provider Evaluation: ONNX Runtime with all-MiniLM-L6-v2"
source: "embedding-evaluation"
type: source
tags: [machine-learning, embeddings, rust, onnx-runtime, sentence-transformers, performance-evaluation, architecture-decision, nlp, vector-search]
generated: "2026-04-18T15:13:35.265146875+00:00"
---

# Embedding Provider Evaluation: ONNX Runtime with all-MiniLM-L6-v2

This document presents a technical evaluation of embedding providers for a Rust-based application, dated July 2025. The evaluation compares four options: ONNX Runtime (ort crate), Candle, rust-bert, and Remote API across multiple criteria including cold start time, per-entry latency, external dependencies, and binary size impact. The decision was made to use ONNX Runtime (ort v2.0.0-rc) with the all-MiniLM-L6-v2 sentence-transformer model. The rationale focuses on ONNX Runtime's maturity, model compatibility, performance optimizations through graph optimizations, flexible feature flags, and thread safety characteristics suitable for async architectures. The document also details trade-offs including increased binary size (~30–50 MB), C++ dependencies, and compile time impacts, along with rejection reasons for the alternatives. The selected model produces 384-dimensional embeddings with L2 normalization, targeting a Spearman correlation of ~0.82 on the STS benchmark. The architecture includes an EmbeddingProvider trait with NoOp and LocalEmbeddingProvider implementations, with embeddings stored as BLOB columns in SQLite using brute-force cosine similarity for search.

## Related

### Entities

- [ONNX Runtime](../entities/onnx-runtime.md) — technology
- [ort](../entities/ort.md) — product
- [Candle](../entities/candle.md) — technology
- [rust-bert](../entities/rust-bert.md) — product
- [all-MiniLM-L6-v2](../entities/all-minilm-l6-v2.md) — product
- [Microsoft](../entities/microsoft.md) — organization
- [HuggingFace](../entities/huggingface.md) — organization
- [tokenizers](../entities/tokenizers.md) — product
- [reqwest](../entities/reqwest.md) — product

### Concepts

- [Embedding](../concepts/embedding.md)
- [ONNX format](../concepts/onnx-format.md)
- [Cold start](../concepts/cold-start.md)
- [Graph optimization](../concepts/graph-optimization.md)
- [Mean pooling](../concepts/mean-pooling.md)
- [L2 normalization](../concepts/l2-normalization.md)
- [Cosine similarity](../concepts/cosine-similarity.md)
- [Feature gating](../concepts/feature-gating.md)
- [Thread safety](../concepts/thread-safety.md)
- [Zero external service dependencies](../concepts/zero-external-service-dependencies.md)

