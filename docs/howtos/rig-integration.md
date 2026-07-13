# Rig Integration — `ragent.json` Schema and Examples

**Spec:** `rig`
**Task:** T-019
**Requirements:** FR-002, FR-006, FR-028

This document describes how to configure the optional [Rig](https://rig.rs)
framework integration from `ragent.json`, which feature flags control what is
compiled, and how Rig-backed providers relate to ragent's native providers.

---

## 1. Overview

The Rig integration is **strictly additive and opt-in**. When no `rig` section
is present in `ragent.json` (the default), ragent behaves exactly as before —
all native providers, the memory system, the code index, and `/research` run
unchanged (FR-003). The `rig` section only activates Rig-backed backends for the
subsystems you explicitly enable.

The integration covers four optional capabilities:

| Capability | Config key | Feature flag | Backing requirement |
|---|---|---|---|
| Rig-backed completion providers | `rig.providers[]` | `provider-<name>` | `ragent-rig` compiled with the matching provider feature |
| Conversation-memory policy | `rig.memory` | `memory` | `ragent-rig` compiled with `memory` |
| Embedding generation | `rig.embeddings` | `embeddings` | `ragent-rig` compiled with `embeddings` |
| Vector store + semantic search | `rig.vector_store` | `vector-store-*` | `ragent-rig` compiled with at least one vector-store backend |

Each row is independent: enabling embeddings does not force a vector store,
and a vector store can run without a memory policy.

---

## 2. Compile-time feature flags (FR-006)

Every Rig dependency is optional at compile time. The `ragent-rig` crate
exposes these Cargo features (in `crates/ragent-rig/Cargo.toml`):

### Provider features

Each pulls in `rig-core` and (for streaming providers) `async-stream`.

| Feature | Rig provider | Streaming? |
|---|---|---|
| `provider-openai` *(default)* | OpenAI | yes |
| `provider-anthropic` | Anthropic | yes |
| `provider-gemini` | Google Gemini | yes |
| `provider-ollama` | Ollama (local or cloud) | yes |
| `provider-cohere` | Cohere | no |
| `provider-deepseek` | DeepSeek | no |
| `provider-groq` | Groq | no |
| `provider-huggingface` | Hugging Face | no |
| `provider-mistral` | Mistral | no |
| `provider-perplexity` | Perplexity | no |
| `provider-together` | Together AI | no |
| `provider-xai` | xAI (Grok) | no |

### Capability features

| Feature | Pulls in | Used by |
|---|---|---|
| `memory` | `rig-core` | Conversation-memory policy (T-011) |
| `embeddings` | `rig-core` | Embedding generation (T-007) |
| `vector-store-memory` | `embeddings` | In-memory vector store |
| `vector-store-sqlite` | `embeddings`, `rusqlite` | Local SQLite vector store |
| `vector-store-http` | `embeddings`, `reqwest` | Remote vector-store service |
| `mock` | `rig-core` | Rig mock models for tests (T-014) |
| `vcr` | — | VCR cassette record/replay (T-015) |
| `codeindex` | `ragent-codeindex` | Semantic code-index (T-009) |
| `rig-semantic` | `codeindex` + embeddings + all vector stores | Semantic code-index (T-009) |
| `memory-semantic` | embeddings + all vector stores | Semantic memory search (T-010) |
| `research` | `ragent-research` + embeddings + all vector stores | Semantic `/research` (T-012) |

### Binary-level features

The `ragent` binary exposes two features that control CLI-side wiring. Both
are **on by default**; NFR-002 is preserved because the code they enable is
only constructed when the runtime config explicitly turns on the matching
`rig.*` sub-section, so the linker dead-strips it when no `rig` section is
present in `ragent.json`.

| Feature | Effect | Default? |
|---|---|---|
| `ragent-rig-research` | Enables `ragent-rig/research` so the `ragent research` CLI path constructs a Rig-backed `ResearchAugmentor` when `rig.embeddings` + `rig.vector_store` are both enabled at runtime. | on |
| `ragent-rig-memory` | Enables `ragent-rig/memory` so the binary registers a Rig conversation-memory policy when `rig.memory.enabled` is true in `ragent.json`. | on |

A binary built **without** these features (`--no-default-features`) still
accepts the `rig` config section (it is parsed and providers are registered);
only the semantic research augmentation and memory-policy wiring are omitted.

---

## 3. `rig` config section

Add a top-level `rig` key to `ragent.json`. All sub-sections are optional and
default to disabled.

```jsonc
{
  "rig": {
    // One entry per Rig-backed provider you want to expose.
    "providers": [
      {
        "alias": "rig-openai",        // becomes the ragent provider id
        "provider": "openai",         // Rig provider name (see table above)
        "model": "gpt-4o",            // model id exposed to ragent
        "api_key": null,              // optional override; falls back to env
        "base_url": null,            // optional override for self-hosted/proxy
        "streaming": true             // allow streaming responses (default true)
      }
    ],

    // Optional: Rig conversation-memory policy for history trimming.
    "memory": {
      "enabled": true,
      "policy": "token_budget",      // sliding_window | token_budget | compaction
      "limit": 4096                   // messages (sliding_window) or tokens (token_budget)
    },

    // Optional: Rig embedding model for semantic search.
    "embeddings": {
      "enabled": true,
      "provider_alias": "rig-openai", // must match a providers[].alias
      "model": "text-embedding-3-small"
    },

    // Optional: vector store for semantic retrieval.
    "vector_store": {
      "enabled": true,
      "backend": "sqlite",            // memory | sqlite | http
      "connection": ".ragent/vectors.db" // path (sqlite) or base URL (http); ignored for memory
    }
  }
}
```

### 3.1 Field reference

#### `rig.providers[]` — `RigProviderConfig`

| Field | Type | Default | Notes |
|---|---|---|---|
| `alias` | string | *(required)* | Becomes the ragent provider id. Must be unique in the registry; duplicates are skipped with a warning. |
| `provider` | string | *(required)* | Rig provider identifier: `openai`, `anthropic`, `gemini`, `ollama`, `cohere`, `deepseek`, `groq`, `huggingface`, `mistral`, `perplexity`, `together`, `xai`. |
| `model` | string | *(required)* | Model id exposed to ragent, e.g. `gpt-4o`, `claude-3-5-sonnet`. |
| `api_key` | string \| null | null | Optional API-key override. When `null`, the provider reads its standard environment variable (e.g. `OPENAI_API_KEY`). |
| `base_url` | string \| null | null | Optional base URL override for self-hosted or proxy endpoints. |
| `streaming` | bool | true | Whether streaming responses are allowed for this provider. |

#### `rig.memory` — `RigMemoryConfig`

| Field | Type | Default | Notes |
|---|---|---|---|
| `enabled` | bool | false | Enable Rig conversation-memory policies as the trimming/compaction backend. |
| `policy` | string | `"sliding_window"` | One of `sliding_window`, `token_budget`, `compaction`. |
| `limit` | usize | 20 | Message count (sliding window) or token budget (token budget policy). |

#### `rig.embeddings` — `RigEmbeddingsConfig`

| Field | Type | Default | Notes |
|---|---|---|---|
| `enabled` | bool | false | Enable Rig-backed embedding generation. |
| `provider_alias` | string | `"rig-openai"` | Must match a `providers[].alias`; that provider's API key is used for embedding calls. |
| `model` | string | `"text-embedding-3-small"` | Embedding model id. |

#### `rig.vector_store` — `RigVectorStoreConfig`

| Field | Type | Default | Notes |
|---|---|---|---|
| `enabled` | bool | false | Enable Rig-backed vector stores. |
| `backend` | string | `"memory"` | One of `memory`, `sqlite`, `http`. |
| `connection` | string \| null | null | SQLite file path (`sqlite`), HTTP base URL (`http`), ignored (`memory`). |

---

## 4. Using Rig-backed providers

Once a provider alias is registered, it behaves like any native ragent
provider. Select it with `--model <alias>/<model>`:

```bash
# Run a one-shot prompt through the rig-openai alias
ragent run "hello" --model rig-openai/gpt-4o

# Or set it as the default agent model in ragent.json
{
  "agent": {
    "general": { "model": "rig-openai/gpt-4o" }
  }
}
```

Streaming, tool calls, and thinking configuration all flow through the same
internal `ChatRequest`/`StreamEvent` types the native providers use, so the
TUI, HTTP server, and CLI consume Rig responses uniformly (FR-012, FR-013).

### 4.1 Environment variables

When `api_key` is `null` (the default), each Rig provider reads its standard
environment variable:

| `provider` | Environment variable |
|---|---|
| `openai` | `OPENAI_API_KEY` |
| `anthropic` | `ANTHROPIC_API_KEY` |
| `gemini` | `GEMINI_API_KEY` |
| `ollama` | `OLLAMA_API_KEY` *(cloud)* / none *(local)* |
| `cohere` | `COHERE_API_KEY` |
| `deepseek` | `DEEPSEEK_API_KEY` |
| `groq` | `GROQ_API_KEY` |
| `huggingface` | `HF_TOKEN` |
| `mistral` | `MISTRAL_API_KEY` |
| `perplexity` | `PERPLEXITY_API_KEY` |
| `together` | `TOGETHER_API_KEY` |
| `xai` | `XAI_API_KEY` |

---

## 5. Native vs. Rig providers (FR-028)

Where an existing native ragent provider already covers a model family, the
**native implementation remains the default** and the Rig implementation is an
opt-in backend exposed only when you add a `rig.providers[]` entry for it
(FR-028, FR-032). Concretely:

- Adding `rig.providers[]` with `provider: "openai"` creates a **new** provider
  under the alias (e.g. `rig-openai`). It does **not** replace the built-in
  `openai` provider. Both coexist in the registry.
- To route a request to the Rig-backed provider, use the alias:
  `--model rig-openai/gpt-4o`. Using `--model openai/gpt-4o` continues to use
  the native OpenAI client.
- Removing the `rig` section (or the matching `providers[]` entry) fully
  reverts to native-only operation (FR-003, FR-019).

This keeps the migration path non-destructive: you can A/B test Rig-backed
providers against the native ones without changing any other configuration.

---

## 6. Semantic search wiring

Semantic search is layered across three subsystems. Each is enabled
independently so you can adopt them incrementally.

### 6.1 Code index (T-009, FR-015, FR-021)

Compile `ragent-rig` with the `rig-semantic` feature. When
`codeindex.semantic.enabled` is `true` in `ragent.json`, the code index
maintains a parallel semantic index via Rig embeddings + the configured vector
store, and `/codeindex` search fuses lexical and vector results.

### 6.2 Memory (T-010)

Compile `ragent-rig` with the `memory-semantic` feature. When
`rig.embeddings.enabled` + `rig.vector_store.enabled` are both `true`, the
structured-memory store gains vector-similarity search over stored memories
(see `memory_search` / `SemanticMemory`).

### 6.3 Research (T-012, FR-016, FR-017, FR-022)

Compile `ragent-rig` with the `research` feature **and** the `ragent` binary
with the `ragent-rig-research` feature. When `rig.embeddings.enabled` +
`rig.vector_store.enabled` are both `true`, the `ragent research` CLI path
constructs a Rig-backed `ResearchAugmentor` that:

1. Retrieves semantically similar prior findings/sources **before** the web
   phase (FR-016), injecting them as additional sources.
2. Embeds captured web/local sources after the gather phase (FR-017).
3. Embeds the completed research document so later runs can find it (FR-016).

The TUI and HTTP server paths accept the same augmentor via
`build_research_session(..., semantic_augmentor)`; wiring them to construct a
Rig augmentor from config is deferred to a later task.

---

## 7. Full example

A `ragent.json` that enables a Rig-backed OpenAI provider, token-budget
conversation memory, OpenAI embeddings, and a SQLite vector store:

```jsonc
{
  "rig": {
    "providers": [
      {
        "alias": "rig-openai",
        "provider": "openai",
        "model": "gpt-4o",
        "streaming": true
      }
    ],
    "memory": {
      "enabled": true,
      "policy": "token_budget",
      "limit": 4096
    },
    "embeddings": {
      "enabled": true,
      "provider_alias": "rig-openai",
      "model": "text-embedding-3-small"
    },
    "vector_store": {
      "enabled": true,
      "backend": "sqlite",
      "connection": ".ragent/vectors.db"
    }
  },
  "code_index": { "enabled": true },
  "memory": { "enabled": true, "structured": { "enabled": true } }
}
```

Build the binary with the features you need:

```bash
# Providers + memory policy + embeddings + sqlite vector store + semantic research
# (ragent-rig-research and ragent-rig-memory are on by default)
cargo build --release \
  --features "ragent-rig/provider-openai,ragent-rig/memory,ragent-rig/embeddings,ragent-rig/vector-store-sqlite,ragent-rig/research,ragent-rig/rig-semantic,ragent-rig/memory-semantic"
```

> **Note (NFR-002):** When no Rig providers are enabled at runtime (no `rig`
> section in `ragent.json`), LTO + strip dead-strips all Rig code, so the
> release binary size impact is 0% regardless of which features were compiled
> in. See [`docs/reports/rig-binary-size-compile-time-impact.md`](reports/rig-binary-size-compile-time-impact.md)
> for the measured figures.

---

## 8. Validation

- `ragent config` prints the resolved configuration, including the `rig`
  section, so you can confirm your edits parsed correctly.
- `ragent models` lists every registered provider, including Rig-backed
  aliases, so you can confirm an alias was registered.
- `cargo check -p ragent-rig --features <feat>` verifies a feature combination
  compiles before you depend on it.

---

## 9. Cross-references

- Spec: [`specs/rig/SPEC.md`](../../specs/rig/SPEC.md)
- Interface audit: [`docs/reports/rig-interface-audit.md`](reports/rig-interface-audit.md)
- Delegation map: [`docs/rig-delegation-map.md`](rig-delegation-map.md)
- Binary-size/compile-time report: [`docs/reports/rig-binary-size-compile-time-impact.md`](reports/rig-binary-size-compile-time-impact.md)
- Config types: `crates/ragent-config/src/config.rs` (`RigConfig`,
  `RigProviderConfig`, `RigMemoryConfig`, `RigEmbeddingsConfig`,
  `RigVectorStoreConfig`)
- Provider wiring: `crates/ragent-rig/src/registry.rs` (`register_rig_providers`)