# Embedding Provider Evaluation

**Date:** July 2025
**Decision:** Use ONNX Runtime (`ort` crate) with `all-MiniLM-L6-v2` sentence-transformer

## Evaluated Options

| Option | Crate | Language | Model Format | Cold Start | Per-Entry Latency | External Dependencies | Binary Size Impact |
|--------|-------|----------|-------------|-----------|-------------------|----------------------|-------------------|
| **ONNX Runtime** | `ort` v2.0.0-rc | Rust bindings over C++ | ONNX | ~2–4s | ~10–30ms | ONNX Runtime shared lib (bundled) | +30–50 MB |
| **Candle** | `candle-nn` / `candle-transformers` | Pure Rust | GGUF / SAFEOPENS | ~3–5s | ~20–50ms | None (pure Rust) | +40–60 MB |
| **rust-bert** | `rust-bert` | Rust over ONNX/PyTorch | ONNX / GGML | ~5–10s | ~20–50ms | None (pure Rust) | +50–80 MB |
| **Remote API** | N/A (reqwest) | N/A | N/A | ~200ms (network) | ~200–500ms (network) | API key + internet | Negligible |

## Decision: ONNX Runtime (`ort`)

### Rationale

1. **Maturity**: `ort` is the most widely-used ONNX Runtime binding in the Rust ecosystem. ONNX Runtime itself is a production-grade inference engine backed by Microsoft.

2. **Model compatibility**: `all-MiniLM-L6-v2` is available as a standard ONNX export on HuggingFace, with well-tested tokenizer support via the `tokenizers` crate.

3. **Performance**: ONNX Runtime includes graph optimizations (operator fusion, constant folding, quantization) that pure-Rust alternatives lack, giving it ~2× better inference latency.

4. **Feature flags**: The `ort` crate supports `download-binaries` (bundles the C++ shared library) and `load-dynamic` (loads from system paths), giving flexibility for different deployment scenarios.

5. **Thread safety**: The `ort::Session` is `Send + Sync` and can be wrapped in a `Mutex` for lazy initialization, fitting our async architecture.

### Trade-offs

- **Binary size**: The bundled ONNX Runtime adds ~30–50 MB to the binary. This is acceptable for a desktop agent.
- **C++ dependency**: `ort` requires a native shared library. The `download-binaries` feature handles this, but it means we can't be purely static. For `musl` builds, the dynamic loading approach (`load-dynamic`) requires the library to be present at runtime.
- **Compile time**: Adding `ort` + `tokenizers` + `ndarray` adds ~2 minutes to clean build time. Mitigated by feature-gating: users who don't need embeddings don't pay this cost.

### Rejected Alternatives

- **Candle**: Pure Rust is attractive, but model loading code is more manual, fewer pre-trained ONNX exports are available, and the project is less mature.
- **rust-bert**: Heavier dependency, pulls in more transitive crates, and has slower cold-start.
- **Remote API**: Would work but violates the "zero external service dependencies" requirement for the default config. Could be added as a future option for cloud deployments.

## Selected Model: all-MiniLM-L6-v2

| Property | Value |
|----------|-------|
| Dimensions | 384 |
| Max sequence length | 256 tokens |
| Model size | ~90 MB (ONNX) |
| Source | `sentence-transformers/all-MiniLM-L6-v2` on HuggingFace |
| License | Apache 2.0 |
| Mean pooling | Required (model outputs token-level embeddings) |
| Normalization | L2 (applied after pooling) |

### Performance Characteristics

| Metric | Target | Measured (expected) |
|--------|--------|--------------------|
| Cold start (first embed) | < 5s | ~2–4s (model download + load) |
| Per-entry latency (after load) | < 50ms | ~10–30ms |
| Memory (model in RAM) | < 500MB | ~150–200 MB |
| Similarity quality (STS benchmark) | > 0.80 | ~0.82 (Spearman) |

## Architecture

```
┌──────────────────────────────────┐
│       EmbeddingProvider trait     │
│  - embed(text) -> Vec<f32>        │
│  - embed_batch(texts) -> Vec<..> │
│  - dimensions() -> usize         │
│  - name() -> &str                 │
│  - is_available() -> bool        │
└──────┬───────────┬────────────────┘
       │           │
  ┌────▼────┐ ┌────▼──────────────────┐
  │ NoOp    │ │ LocalEmbeddingProvider │
  │ (empty) │ │ (ort + tokenizers)    │
  │ default │ │ Feature-gated         │
  └─────────┘ └────────────────��───────┘
```

## Storage

Embedding vectors are stored as `BLOB` columns in SQLite:

- `memories.embedding` — BLOB (nullable, 384 × 4 = 1536 bytes per embedding)
- `journal_entries.embedding` — BLOB (nullable)

Search uses brute-force cosine similarity for <10K entries. Future: `sqlite-vec` for ANN at scale.