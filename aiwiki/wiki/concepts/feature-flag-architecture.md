---
title: "Feature Flag Architecture"
type: concept
generated: "2026-04-19T21:45:52.712280923+00:00"
---

# Feature Flag Architecture

### From: embedding

Feature flag architecture in this crate demonstrates conditional compilation as a software product line technique, enabling multiple valid configurations from a single codebase. The `embeddings` feature gate controls inclusion of ONNX Runtime dependencies, ML model files, and the associated LocalEmbeddingProvider implementation. When disabled (the default), the crate compiles to a minimal footprint suitable for resource-constrained environments or users who don't require semantic search. When enabled, the additional functionality becomes available through the `local` submodule and its public re-export.

This design implements the principle of pay-for-what-you-use in systems programming. The default configuration avoids heavy dependencies: ONNX Runtime binaries can exceed 100MB, and transformer model files add tens of megabytes. For embedded deployments, CI pipelines, or applications using only keyword search, these costs are prohibitive. The feature flag ensures such users aren't burdened with unused capabilities. Conversely, users needing semantic search simply enable the feature and gain full functionality without modifying source code or maintaining separate forks.

The implementation uses Rust's cfg attribute system with feature gates on module declarations and re-exports. The pattern `#[cfg(feature = "embeddings")] mod local;` conditionally includes the local submodule, while the corresponding `pub use` makes types available only when compiled with the feature. This creates a coherent API where LocalEmbeddingProvider either exists or doesn't, with no partial states or runtime unavailability. Documentation comments explicitly describe feature behavior, guiding users to understand compile-time vs runtime availability. This architecture pattern extends to testing, where conditional test inclusion ensures CI can verify both configurations without requiring ONNX in minimal test environments.

## External Resources

- [Cargo Features reference documentation](https://doc.rust-lang.org/cargo/reference/features.html) - Cargo Features reference documentation
- [Rust API Guidelines on feature documentation](https://rust-lang.github.io/api-guidelines/documentation.html) - Rust API Guidelines on feature documentation

## Sources

- [embedding](../sources/embedding.md)
