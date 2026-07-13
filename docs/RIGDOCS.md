# Rig Framework Integration — Configuration Guide

**Spec:** `rig` (`specs/rig/SPEC.md`)
**Crate:** `crates/ragent-rig/`
**Pinned dependency:** `rig-core = "0.9"` (FR-033)
**Status:** Implemented (phase 1 — strictly additive and opt-in)

This document is the single consolidated reference for **every configuration
change required to enable each capability** the [Rig](https://rig.rs) framework
integration provides inside ragent. It covers:

1. The top-level `rig` config section in `ragent.json`.
2. Compile-time Cargo feature flags (crate-level and binary-level).
3. Runtime environment variables.
4. The exact config + feature combination needed to turn on each individual
   function (providers, memory policies, embeddings, vector stores, semantic
   code index, semantic memory, semantic research, tool wrapping, mock/VCR
   testing).
5. Related non-Rig config sections that interact with the integration.
6. Validation, troubleshooting, and known gaps.

---

## 1. Design Principles

The integration follows four non-negotiable rules (FR-003, FR-028, FR-031,
FR-034, NFR-002):

| Rule | Effect |
|------|--------|
| **Strictly additive** | When no `rig` section is present in `ragent.json`, ragent behaves exactly as before. All native providers, memory, code index, and `/research` run unchanged. |
| **Opt-in** | Every Rig backend is off by default. Users must explicitly add config + compile the right features. |
| **Native remains default** | Where a native ragent provider already covers a model family, the native implementation stays the default. Rig backends are exposed only when you add a `rig.providers[]` entry. |
| **Zero binary-size cost when unused** | LTO + dead-stripping eliminates all Rig code from the release binary when no Rig provider is configured at runtime (measured: 0 bytes, 0.00% — NFR-002 PASS). |

---

## 2. The `rig` Config Section

Add a top-level `rig` key to `ragent.json` (or `ragent.jsonc`). All sub-sections
are optional and default to disabled.

```jsonc
{
  "rig": {
    "providers": [ /* Rig-backed LLM completion providers */ ],
    "memory":    { /* optional conversation-memory policy */ },
    "embeddings": { /* optional embedding generation */ },
    "vector_store": { /* optional vector store backend */ }
  }
}
```

### 2.1 `rig.providers[]` — `RigProviderConfig`

One entry per Rig-backed LLM provider you want to expose.

| Field | Type | Default | Notes |
|-------|------|---------|-------|
| `alias` | string | *(required)* | Becomes the ragent provider id. Must be unique; duplicates are skipped with a warning. |
| `provider` | string | *(required)* | Rig provider name: `openai`, `anthropic`, `gemini`, `ollama`, `cohere`, `deepseek`, `groq`, `huggingface`, `mistral`, `perplexity`, `together`, `xai`. |
| `model` | string | *(required)* | Model id exposed to ragent (e.g. `gpt-4o`, `claude-3-5-sonnet`). |
| `api_key` | string \| null | null | Optional API-key override. When `null`, the provider reads its standard environment variable. |
| `base_url` | string \| null | null | Optional base URL override for self-hosted or proxy endpoints. |
| `streaming` | bool | true | Whether streaming responses are allowed for this provider. |

**Config source:** `crates/ragent-config/src/config.rs` → `RigProviderConfig`

**Runtime wiring:** `crates/ragent-rig/src/registry.rs` →
`register_rig_providers()`, called unconditionally from `src/main.rs` at
startup. The function is a no-op when `config.rig` is `None` or has no
providers.

### 2.2 `rig.memory` — `RigMemoryConfig`

Optional Rig conversation-memory policy for history trimming/compaction.

| Field | Type | Default | Notes |
|-------|------|---------|-------|
| `enabled` | bool | false | Enable Rig conversation-memory policies as the trimming/compaction backend. |
| `policy` | string | `"sliding_window"` | One of `sliding_window`, `token_budget`, `compaction`. |
| `limit` | usize | 20 | Message count (sliding window) or token budget (token budget) or recent-message keep count (compaction). Must be > 0. |

**Config source:** `crates/ragent-config/src/config.rs` → `RigMemoryConfig`

**Runtime wiring:** `crates/ragent-rig/src/memory.rs` →
`build_memory_policy()`, called from `src/main.rs` behind the
`ragent-rig-memory` binary feature. When `enabled = true`, the resulting
policy is registered on the `SessionProcessor` via `set_memory_policy()`.
The processor then delegates history trimming to the Rig policy instead of
the native Headroom compression pipeline (FR-014 / FR-020).

### 2.3 `rig.embeddings` — `RigEmbeddingsConfig`

Optional Rig embedding model for semantic search.

| Field | Type | Default | Notes |
|-------|------|---------|-------|
| `enabled` | bool | false | Enable Rig-backed embedding generation. |
| `provider_alias` | string | `"rig-openai"` | Must match a `providers[].alias`; that provider's API key is used for embedding calls. |
| `model` | string | `"text-embedding-3-small"` | Embedding model id. |

**Config source:** `crates/ragent-config/src/config.rs` → `RigEmbeddingsConfig`

**Runtime wiring:** `crates/ragent-rig/src/embeddings.rs` →
`build_embedding_backend_by_provider()`. The embedding backend is the
foundation for semantic code index, semantic memory, and semantic research.
All three require `rig.embeddings.enabled = true` **plus** a vector store.

### 2.4 `rig.vector_store` — `RigVectorStoreConfig`

Optional vector store for semantic retrieval.

| Field | Type | Default | Notes |
|-------|------|---------|-------|
| `enabled` | bool | false | Enable Rig-backed vector stores. |
| `backend` | string | `"memory"` | One of `memory`, `sqlite`, `http`. |
| `connection` | string \| null | null | SQLite file path (`sqlite`), HTTP base URL (`http`), ignored (`memory`). |

**Config source:** `crates/ragent-config/src/config.rs` → `RigVectorStoreConfig`

**Runtime wiring:** `crates/ragent-rig/src/vector_store.rs` →
`VectorStoreAdapter::from_config()`. The adapter dispatches to one of three
backends: `MemoryVectorStoreBackend`, `SqliteVectorStoreBackend`, or
`HttpVectorStoreBackend`.

---

## 3. Compile-Time Feature Flags

Every Rig dependency is optional at compile time. There are two layers:

### 3.1 Crate-level features (`crates/ragent-rig/Cargo.toml`)

#### Provider features

Each pulls in `rig-core` and (for streaming providers) `async-stream`.

| Feature | Rig provider | Streaming? | Default? |
|---------|-------------|------------|----------|
| `provider-openai` | OpenAI | yes | **yes** |
| `provider-anthropic` | Anthropic | yes | no |
| `provider-gemini` | Google Gemini | yes | no |
| `provider-ollama` | Ollama (local or cloud) | yes | no |
| `provider-cohere` | Cohere | no | no |
| `provider-deepseek` | DeepSeek | no | no |
| `provider-groq` | Groq | no | no |
| `provider-huggingface` | Hugging Face | no | no |
| `provider-mistral` | Mistral | no | no |
| `provider-perplexity` | Perplexity | no | no |
| `provider-together` | Together AI | no | no |
| `provider-xai` | xAI (Grok) | no | no |

> **Implementation note:** As of this release, concrete completion-backend
> builders exist for `openai`, `anthropic`, `gemini`, and `ollama` only
> (`build_backend_by_provider` in `crates/ragent-rig/src/provider.rs`). The
> other eight provider features compile and pull in `rig-core`, but
> `build_backend_by_provider` returns `ProviderNotEnabled` for them because
> no match arm is implemented. They are reserved for future completion.

#### Capability features

| Feature | Pulls in | Used by |
|---------|----------|---------|
| `memory` | `rig-core` | Conversation-memory policy (T-011) |
| `embeddings` | `rig-core` | Embedding generation (T-007) |
| `vector-store-memory` | `embeddings` | In-memory vector store |
| `vector-store-sqlite` | `embeddings`, `rusqlite` | Local SQLite vector store |
| `vector-store-http` | `embeddings`, `reqwest` | Remote vector-store service |
| `mock` | `rig-core` | Rig mock models for tests (T-014) |
| `vcr` | — | VCR cassette record/replay (T-015) |
| `codeindex` | `ragent-codeindex` | Semantic code-index dependency (T-009) |
| `rig-semantic` | `codeindex` + `embeddings` + all vector stores | Semantic code-index (T-009) |
| `memory-semantic` | `embeddings` + all vector stores | Semantic memory search (T-010) |
| `research` | `ragent-research` + `embeddings` + all vector stores | Semantic `/research` (T-012) |

### 3.2 Binary-level features (`Cargo.toml` `[features]`)

The `ragent` binary exposes two features that control CLI-side wiring:

| Feature | Effect | Default? |
|---------|--------|----------|
| `ragent-rig-research` | Enables `ragent-rig/research` so the `ragent research` CLI path constructs a Rig-backed `ResearchAugmentor` when `rig.embeddings` + `rig.vector_store` are both enabled at runtime. | **on** |
| `ragent-rig-memory` | Enables `ragent-rig/memory` so the binary registers a Rig conversation-memory policy when `rig.memory.enabled` is true in `ragent.json`. | **on** |

Both features are **on by default**. A binary built without them (`--no-default-features`) still accepts the `rig` config section (it is parsed and providers are registered); only the semantic research augmentation and memory-policy wiring are omitted.

---

## 4. Environment Variables

When `api_key` is `null` (the default), each Rig provider reads its standard
environment variable. The resolution order in `RigProvider::resolve_api_key()`
is:

1. `api_key` field in `RigProviderConfig`.
2. `RIG_<PROVIDER>_API_KEY` environment variable (e.g.
   `RIG_OPENAI_API_KEY`).
3. The provider's conventional env var as a fallback.

| `provider` | Conventional env var |
|------------|---------------------|
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

## 5. Capability-by-Capability Enablement Guide

Each subsection lists the **exact** config keys and feature flags needed to
turn on one Rig capability. Capabilities are independent unless stated
otherwise.

### 5.1 Rig-Backed LLM Completion Providers

**What it does:** Exposes Rig-backed LLM providers (OpenAI, Anthropic, Gemini,
Ollama) as first-class ragent providers. Select them with
`--model <alias>/<model>`.

**Config required:**

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
    ]
  }
}
```

**Feature flags required:**

| Layer | Feature |
|-------|---------|
| `ragent-rig` crate | `provider-openai` *(default)* — or `provider-anthropic`, `provider-gemini`, `provider-ollama` for other providers |
| `ragent` binary | none (providers are registered unconditionally) |

**Build command:**

```bash
# Default (provider-openai is on by default)
cargo build --release

# Add Anthropic support
cargo build --release --features "ragent-rig/provider-anthropic"

# Add all four implemented providers
cargo build --release --features \
  "ragent-rig/provider-openai,ragent-rig/provider-anthropic,ragent-rig/provider-gemini,ragent-rig/provider-ollama"
```

**Runtime selection:**

```bash
ragent run "hello" --model rig-openai/gpt-4o
```

**Requirements covered:** FR-002, FR-004, FR-005, FR-006, FR-012, FR-013,
FR-019, FR-024, FR-027, FR-028.

---

### 5.2 Conversation-Memory Policy (History Trimming)

**What it does:** Replaces the native Headroom compression pipeline with a
Rig-backed memory policy (sliding window, token budget, or compaction) for
session history trimming. The policy runs entirely in-process (no LLM call),
so it is dramatically cheaper than Headroom.

**Config required:**

```jsonc
{
  "rig": {
    "memory": {
      "enabled": true,
      "policy": "token_budget",
      "limit": 4096
    }
  }
}
```

**Feature flags required:**

| Layer | Feature |
|-------|---------|
| `ragent-rig` crate | `memory` |
| `ragent` binary | `ragent-rig-memory` *(on by default)* |

**Build command:**

```bash
# Default build (ragent-rig-memory is on by default)
cargo build --release
```

**How it works:** When `rig.memory.enabled = true`, `src/main.rs` calls
`ragent_rig::memory::build_memory_policy()` and registers the result on the
`SessionProcessor` via `set_memory_policy()`. The processor then delegates
history trimming to the Rig policy whenever
`should_compress_with_reported()` reports the context window is exceeded
(FR-014), instead of running the Headroom compression pipeline (FR-020).

**Policy types:**

| Policy | `limit` meaning | Behaviour |
|--------|----------------|-----------|
| `sliding_window` | Max non-system messages | Retains the most recent `limit` messages; drops everything older. Always preserves the leading system message and the very last message. |
| `token_budget` | Max estimated tokens | Drops oldest messages until the total estimated token count is within `limit`. Preserves system message and last message. |
| `compaction` | Recent messages to keep | Keeps the most recent `limit` messages and replaces everything older with a `[...earlier conversation compacted...]` marker. |

> **Note:** The policy semantics are implemented in `ragent-rig` using
> ragent's existing `estimate_chat_message_tokens` helper, not by depending
> on the upstream `rig-memory` crate (which tracks a different `rig-core`
> version range — FR-033). When the pinned Rig version catches up, the
> policy bodies can delegate to upstream types without changing the public
> API.

**Requirements covered:** FR-014, FR-020.

---

### 5.3 Embedding Generation

**What it does:** Provides Rig-backed text embedding generation via Rig's
`EmbeddingModel` trait. This is the foundation for all semantic search
capabilities (code index, memory, research). Enabled on its own, it does not
produce user-visible behaviour — it must be combined with a vector store and
a semantic consumer.

**Config required:**

```jsonc
{
  "rig": {
    "providers": [
      {
        "alias": "rig-openai",
        "provider": "openai",
        "model": "gpt-4o"
      }
    ],
    "embeddings": {
      "enabled": true,
      "provider_alias": "rig-openai",
      "model": "text-embedding-3-small"
    }
  }
}
```

`provider_alias` must match a `providers[].alias`. The embedding backend uses
that provider's API key for embedding calls.

**Feature flags required:**

| Layer | Feature |
|-------|---------|
| `ragent-rig` crate | `embeddings` + a provider feature (e.g. `provider-openai`) |
| `ragent` binary | none |

**Supported embedding providers:** OpenAI (`text-embedding-3-small` etc.),
Gemini, Ollama (local, no API key needed). The dispatch table is in
`build_embedding_backend_by_provider()`.

**Requirements covered:** FR-007.

---

### 5.4 Vector Store

**What it does:** Provides a Rig-backed vector store for semantic retrieval.
Three backends are available: in-memory, SQLite (local persistent), and HTTP
(remote service). Like embeddings, a vector store on its own does not produce
user-visible behaviour — it must be combined with a semantic consumer.

**Config required:**

```jsonc
{
  "rig": {
    "vector_store": {
      "enabled": true,
      "backend": "sqlite",
      "connection": ".ragent/vectors.db"
    }
  }
}
```

**Feature flags required:**

| Layer | Feature |
|-------|---------|
| `ragent-rig` crate | At least one: `vector-store-memory`, `vector-store-sqlite`, or `vector-store-http` |
| `ragent` binary | none |

**Backend comparison:**

| Backend | Feature | `connection` | Persistence | Use case |
|---------|---------|-------------|-------------|----------|
| `memory` | `vector-store-memory` | ignored | Ephemeral (process lifetime) | Testing, small corpora |
| `sqlite` | `vector-store-sqlite` | File path | Local persistent file | Single-binary deployments |
| `http` | `vector-store-http` | Base URL | Remote service | Shared/large-scale vector stores |

**Requirements covered:** FR-008, FR-009.

---

### 5.5 Semantic Code Index (Hybrid Search)

**What it does:** Maintains a parallel semantic index over indexed source
files using Rig embeddings + the configured vector store. `/codeindex` search
fuses lexical (Tantivy FTS) and vector-similarity results into a combined
ranking.

**Config required:**

```jsonc
{
  "rig": {
    "providers": [
      { "alias": "rig-openai", "provider": "openai", "model": "gpt-4o" }
    ],
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
  "code_index": { "enabled": true }
}
```

**Feature flags required:**

| Layer | Feature |
|-------|---------|
| `ragent-rig` crate | `rig-semantic` (auto-pulls `codeindex`, `embeddings`, and all vector-store backends) |
| `ragent` binary | none |

**Build command:**

```bash
cargo build --release --features "ragent-rig/rig-semantic"
```

**How it works:** The `SemanticCodeIndex` type
(`crates/ragent-rig/src/codeindex.rs`) wraps a `CodeIndex` plus a
`VectorStoreAdapter` and an embedding backend. It can:

- Embed source files when they are indexed (FR-015).
- Run pure semantic search over the vector store.
- Fuse lexical code-index results with vector-similarity results (FR-021 /
  AC-3) via `hybrid_search()`.

The lexical/symbol code index is never removed (FR-035). When the vector
store is unavailable, the system falls back to lexical search and logs a
warning (FR-018).

> **Note:** The `SemanticCodeIndex` is constructed programmatically from
> code that owns a `CodeIndex` instance. There is no separate
> `codeindex.semantic.enabled` config field — the semantic layer activates
> when `rig.embeddings.enabled` + `rig.vector_store.enabled` are both `true`
> and the `rig-semantic` feature is compiled in.

**Requirements covered:** FR-010, FR-015, FR-018, FR-021, FR-035.

---

### 5.6 Semantic Memory Search (Hybrid Search)

**What it does:** Adds vector-similarity search over ragent's structured
memory store (SQLite). The `memory_search` tool and `SemanticMemory` API
fuse lexical (FTS5) keyword results with vector-similarity results.

**Config required:**

```jsonc
{
  "rig": {
    "providers": [
      { "alias": "rig-openai", "provider": "openai", "model": "gpt-4o" }
    ],
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
  "memory": {
    "enabled": true,
    "structured": { "enabled": true }
  }
}
```

**Feature flags required:**

| Layer | Feature |
|-------|---------|
| `ragent-rig` crate | `memory-semantic` (auto-pulls `embeddings` and all vector-store backends) |
| `ragent` binary | none |

**Build command:**

```bash
cargo build --release --features "ragent-rig/memory-semantic"
```

**How it works:** The `SemanticMemory` type
(`crates/ragent-rig/src/memory_semantic.rs`) wraps a `Storage` handle plus a
`VectorStoreAdapter` and an embedding backend. It can:

- Embed structured memories when they are created or updated (FR-010).
- Run pure semantic search over the vector store (`semantic_search()`).
- Fuse lexical (FTS5) memory results with vector-similarity results
  (`hybrid_search()`).

The `MemoryExt` extension trait adds a `semantic_search()` method to a plain
`Storage` handle for one-off queries without wrapping.

> **Important distinction:** The `memory.semantic` config section
> (`SemanticConfig` in `ragent-config`) controls ragent's **native** ONNX
>-based local embedding system, **not** the Rig integration. The Rig
> semantic memory path is controlled entirely by `rig.embeddings` +
> `rig.vector_store` + the `memory-semantic` feature flag. The two systems
> are independent and can coexist.

**Requirements covered:** FR-010.

---

### 5.7 Semantic Research (`/research` Augmentation)

**What it does:** Augments the `/research` pipeline with vector-similarity
retrieval over embedded sources and prior findings. Specifically:

1. Retrieves semantically similar prior findings/sources **before** the web
   phase (FR-016), injecting them as additional sources.
2. Embeds captured web/local sources after the gather phase (FR-017).
3. Embeds the completed research document so later runs can find it (FR-016).

**Config required:**

```jsonc
{
  "rig": {
    "providers": [
      { "alias": "rig-openai", "provider": "openai", "model": "gpt-4o" }
    ],
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
  }
}
```

**Feature flags required:**

| Layer | Feature |
|-------|---------|
| `ragent-rig` crate | `research` (auto-pulls `ragent-research`, `embeddings`, and all vector-store backends) |
| `ragent` binary | `ragent-rig-research` *(on by default)* |

**Build command:**

```bash
# Default build already enables ragent-rig-research; just add the crate feature
cargo build --release --features "ragent-rig/research"
```

**How it works:** The `ResearchAugmentor` type
(`crates/ragent-rig/src/research.rs`) implements the
`SemanticResearchAugmentor` trait from `ragent-research`. The CLI path
(`src/cli.rs` → `build_cli_semantic_augmentor()`) constructs the augmentor
when `rig.embeddings.enabled` + `rig.vector_store.enabled` are both `true`
and passes it to `build_research_session()`.

> **Known gap — TUI and HTTP server:** Only the `ragent research` CLI path
> wires the semantic augmentor. The TUI (`crates/ragent-tui/src/app/research.rs`)
> and HTTP server (`crates/ragent-server/src/routes/research.rs`) call
> `build_research_session()` with `None` for the `semantic_augmentor`
> parameter. Wiring them to construct a Rig augmentor from config is
> deferred to a later task.

**Requirements covered:** FR-016, FR-017, FR-022, FR-029, NFR-006.

---

### 5.8 Tool Wrapping (Rig `Tool` Trait)

**What it does:** Exposes ragent's security-audited tool registry (~111 tools)
through Rig's `ToolDyn` / `ToolSet` so a Rig-backed agent can invoke ragent
tools **without** bypassing ragent's permission system, shell security model,
or tool approval gating (FR-031).

**Config required:** None (tool wrapping is a programmatic API, not a config
option).

**Feature flags required:**

| Layer | Feature |
|-------|---------|
| `ragent-rig` crate | Any provider feature or `mock` (anything that pulls in `rig-core`) |
| `ragent` binary | none |

**How it works:** The `RigToolWrapper` type
(`crates/ragent-rig/src/tool.rs`) implements Rig's `ToolDyn` by holding an
`Arc<dyn ragent Tool>` plus a cloned `ToolContext` and delegating `call`
straight to `ragent_tool::execute(input, ctx)`. Because the execution path
is identical to the native ragent agent loop, every permission check, path
guard, and shell-security layer fires exactly as it does for a native
session.

The `ragent_toolset()` helper builds a Rig `ToolSet` from a ragent
`ToolRegistry`, wrapping only the specified tool names (or all exported
tools if the list is empty). Hidden tools are excluded.

**Security invariants (FR-031):**

1. No direct execution — always goes through `ragent_tool::execute`.
2. Permission bus preserved — `ToolContext::event_bus` is forwarded verbatim.
3. Hidden tools excluded — Rig agents cannot discover or call hidden tools.
4. No privilege escalation — the wrapper stores a plain `ToolContext`.

**Requirements covered:** FR-031.

---

### 5.9 Mock Models (Testing)

**What it does:** Provides deterministic mock implementations of Rig's
`CompletionModel` and `EmbeddingModel` traits for unit and integration tests.
No network calls required.

**Config required:** None (test-only feature).

**Feature flags required:**

| Layer | Feature |
|-------|---------|
| `ragent-rig` crate | `mock` |
| `ragent` binary | none (test-only) |

**Build command:**

```bash
cargo test -p ragent-rig --features mock
```

**Key types:**

| Type | Purpose |
|------|---------|
| `MockCompletionModel` | Implements `rig::completion::CompletionModel`. Holds canned `MockResponse`s, returns them one per `completion()` call (round-robin). |
| `MockEmbeddingModel` | Implements `rig::embeddings::EmbeddingModel`. Produces deterministic `Vec<f64>` vectors of a fixed dimension. |
| `MockResponse` | Builder for canned responses: `.text("hello")` or `.tool_call("read", "c1", json!({...}))`. |
| `build_mock_llm_client()` | Wires a `MockCompletionModel` into a `RigLlmClient` so tests can call `client.chat(req).await` and collect `StreamEvent`s. |
| `MockRigProvider` | Test-only `Provider` implementation for exercising the full provider-registry loop. |

**Requirements covered:** FR-023, FR-025, NFR-005.

---

### 5.10 VCR Cassettes (Testing)

**What it does:** Provides a deterministic record/playback layer for LLM
interactions. Wraps any `LlmClient` and stores the resulting `StreamEvent`s
in a JSON cassette file. In playback mode the same file is read and matching
requests replay the recorded event stream without any network calls.

**Config required:** None (test-only feature).

**Feature flags required:**

| Layer | Feature |
|-------|---------|
| `ragent-rig` crate | `vcr` (and typically `mock` for test responses) |
| `ragent` binary | none (test-only) |

**Build command:**

```bash
cargo test -p ragent-rig --features "vcr,mock"
```

**Key types:**

| Type | Purpose |
|------|---------|
| `VcrClient` | Wraps a `Box<dyn LlmClient>` and intercepts `chat()` calls. |
| `VcrMode` | `Record(path)`, `Playback(path)`, or `PlaybackRecordNew(path)`. |
| `VcrCassette` | JSON container for recorded interactions. |

**Requirements covered:** FR-026.

---

## 6. Full Example Configuration

A `ragent.json` that enables a Rig-backed OpenAI provider, token-budget
conversation memory, OpenAI embeddings, a SQLite vector store, and the
native memory + code index subsystems:

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
  "memory": {
    "enabled": true,
    "structured": { "enabled": true }
  }
}
```

**Build command (everything enabled):**

```bash
# ragent-rig-research and ragent-rig-memory are on by default;
# only the crate-level features need to be specified
cargo build --release \
  --features "ragent-rig/provider-openai,ragent-rig/memory,ragent-rig/embeddings,ragent-rig/vector-store-sqlite,ragent-rig/research,ragent-rig/rig-semantic,ragent-rig/memory-semantic"
```

> **NFR-002 note:** When no Rig providers are enabled at runtime (no `rig`
> section in `ragent.json`), LTO + strip dead-strips all Rig code, so the
> release binary size impact is 0% regardless of which features were
> compiled in. See
> [`docs/reports/rig-binary-size-compile-time-impact.md`](reports/rig-binary-size-compile-time-impact.md)
> for measured figures.

---

## 7. Feature-to-Function Quick Reference Matrix

| Capability | `ragent.json` keys | Crate feature(s) | Binary feature(s) | Requirements |
|---|---|---|---|---|
| Completion providers | `rig.providers[]` | `provider-<name>` | none | FR-002, FR-004, FR-005, FR-012, FR-013, FR-019, FR-024, FR-027, FR-028 |
| Memory policy | `rig.memory.enabled` | `memory` | `ragent-rig-memory` *(default on)* | FR-014, FR-020 |
| Embeddings | `rig.embeddings.enabled` | `embeddings` + `provider-<name>` | none | FR-007 |
| Vector store | `rig.vector_store.enabled` | `vector-store-*` | none | FR-008, FR-009 |
| Semantic code index | `rig.embeddings` + `rig.vector_store` | `rig-semantic` | none | FR-010, FR-015, FR-018, FR-021, FR-035 |
| Semantic memory | `rig.embeddings` + `rig.vector_store` | `memory-semantic` | none | FR-010 |
| Semantic research | `rig.embeddings` + `rig.vector_store` | `research` | `ragent-rig-research` *(default on)* | FR-016, FR-017, FR-022, FR-029, NFR-006 |
| Tool wrapping | *(none — programmatic)* | any `provider-*` or `mock` | none | FR-031 |
| Mock models | *(none — test only)* | `mock` | none | FR-023, FR-025, NFR-005 |
| VCR cassettes | *(none — test only)* | `vcr` (+ `mock`) | none | FR-026 |

---

## 8. Related Non-Rig Config Sections

These existing config sections interact with the Rig integration:

### 8.1 `code_index` (`CodeIndexConfig`)

Controls ragent's native lexical/symbol code index (tree-sitter + Tantivy
FTS + SQLite). The Rig semantic code index layers **on top** of this — the
lexical index must be enabled for the hybrid search to work.

```jsonc
{
  "code_index": { "enabled": true }
}
```

Default: `enabled: true`.

### 8.2 `memory` (`MemoryConfig`)

Controls ragent's native three-tier memory system (file blocks, structured
SQLite store, semantic search). The `structured.enabled` sub-field must be
`true` for the Rig semantic memory search to have a corpus to search over.

```jsonc
{
  "memory": {
    "enabled": true,
    "structured": { "enabled": true }
  }
}
```

> **The `memory.semantic` sub-section** (`SemanticConfig`) controls ragent's
> **native** ONNX-based local embedding system, not the Rig integration.
> The two embedding systems are independent and can coexist. The Rig path
> is controlled by `rig.embeddings` + `rig.vector_store`.

### 8.3 `compression` (`CompressionConfig`)

Controls the native Headroom compression pipeline. When a Rig memory policy
is active (`rig.memory.enabled = true` + `ragent-rig-memory` feature), the
Rig policy replaces Headroom as the history-trimming backend. The
compression config's `auto_threshold` still controls **when** trimming is
triggered (via `should_compress_with_reported`), but the **how** is delegated
to the Rig policy.

---

## 9. Dependency Policy (`deny.toml`)

The `deny.toml` file codifies five policy invariants for the Rig integration
(T-018 / NFR-001 / AC-5):

1. **Registry-only:** `rig-core` and companion crates must come from
   crates.io, never from a Git source (`[sources] unknown-git = "deny"`,
   no `allow-git` entry for Rig).
2. **Version pinning (FR-033):** `rig-core` is pinned to `"0.9"` in
   `crates/ragent-rig/Cargo.toml`. Routine `cargo update` must not advance
   the Rig major version. Bumps require a deliberate spec-tracked change.
3. **Feature-flag isolation (FR-006 / FR-034):** Every Rig provider
   dependency is optional behind a feature flag. The default binary build
   enables no Rig providers.
4. **License allow-list:** `rig-core` (MIT/Apache-2.0) and its transitive
   dependencies use licenses already in the `[licenses] allow` list. No new
   license families were added.
5. **No advisory suppression for Rig:** Vulnerabilities in Rig or its
   transitive deps are fixed by upgrading, not by adding `ignore` entries.

**Verification commands:**

```bash
cargo deny check                  # default features (no Rig providers)
cargo deny --all-features check   # all Rig providers + embeddings + vector stores
```

---

## 10. Validation

After configuring the Rig integration, verify it is working:

| Check | Command | Expected |
|-------|---------|----------|
| Config parsed correctly | `ragent config` | Prints resolved config including the `rig` section |
| Provider registered | `ragent models` | Lists Rig-backed aliases (e.g. `rig-openai`) |
| Feature compiles | `cargo check -p ragent-rig --features <feat>` | Compiles without errors |
| Mock model test | `cargo test -p ragent-rig --features mock` | All tests pass (NFR-005 / AC-4) |
| VCR test | `cargo test -p ragent-rig --features "vcr,mock"` | All tests pass |
| Dependency policy | `cargo deny check` | No violations (AC-5) |
| Binary size | `cargo build --release && stat -c%s target/release/ragent` | No increase when no Rig providers configured (NFR-002) |

---

## 11. Troubleshooting

### "Rig provider 'X' is not enabled in this build"

You configured a provider in `ragent.json` but did not compile the matching
feature flag. Either:

- Add the feature to your build command:
  `--features "ragent-rig/provider-X"`, or
- Remove the provider entry from `ragent.json`.

### "unknown rig.memory.policy 'X'"

The `policy` field must be exactly one of: `sliding_window`,
`token_budget`, `compaction`.

### "rig.memory.limit must be greater than 0"

The `limit` field must be a positive integer.

### "vector store is disabled in configuration"

`rig.vector_store.enabled` is `false` or missing, but a semantic consumer
(code index, memory, research) tried to construct a vector store. Set
`enabled: true`.

### "Failed to build Rig memory policy; falling back to native compression"

The `ragent-rig-memory` binary feature is compiled in and
`rig.memory.enabled = true`, but `build_memory_policy()` returned an error.
Check the `policy` and `limit` values. ragent falls back to the native
Headroom compression pipeline automatically.

### Semantic research not working from TUI or HTTP server

As of this release, only the `ragent research` CLI path wires the semantic
research augmentor. The TUI and HTTP server pass `None` for the
`semantic_augmentor` parameter. This is a known gap, not a configuration
issue.

### Provider not in `build_backend_by_provider` dispatch table

Only `openai`, `anthropic`, `gemini`, and `ollama` have concrete completion
backend builders. Configuring `cohere`, `deepseek`, `groq`, `huggingface`,
`mistral`, `perplexity`, `together`, or `xai` will compile the feature but
return `ProviderNotEnabled` at runtime. These providers are reserved for
future completion.

---

## 12. Known Gaps and Limitations

| Gap | Status | Workaround |
|-----|--------|------------|
| Only 4 of 12 provider features have concrete completion builders | The other 8 features compile but return `ProviderNotEnabled` | Use the native ragent providers for those model families |
| Semantic research augmentor wired in CLI only | TUI and HTTP server pass `None` | Use `ragent research` CLI for semantic-augmented research |
| `research` feature has a compile error in some configurations | Noted in `docs/reports/rig-binary-size-compile-time-impact.md` as E0277 | Exclude `research` from all-features builds; enable it explicitly with `ragent-rig-research` |
| `rig-memory` crate not used | The published `rig-memory` tracks a different `rig-core` version range (FR-033) | Policy semantics are implemented in `ragent-rig` using ragent's existing token estimator |
| No config field for per-subsystem semantic toggle | `codeindex.semantic.enabled` referenced in the howto does not exist as a config field | The semantic layer activates when `rig.embeddings.enabled` + `rig.vector_store.enabled` are both `true` and the matching crate feature is compiled |

---

## 13. Cross-References

| Resource | Location |
|----------|----------|
| Spec | [`specs/rig/SPEC.md`](../specs/rig/SPEC.md) |
| How-to guide | [`docs/howtos/rig-integration.md`](howtos/rig-integration.md) |
| Interface audit (T-002) | [`docs/reports/rig-interface-audit.md`](reports/rig-interface-audit.md) |
| Delegation map (T-017) | [`docs/rig-delegation-map.md`](rig-delegation-map.md) |
| Binary-size report (T-021) | [`docs/reports/rig-binary-size-compile-time-impact.md`](reports/rig-binary-size-compile-time-impact.md) |
| Config types | `crates/ragent-config/src/config.rs` (`RigConfig`, `RigProviderConfig`, `RigMemoryConfig`, `RigEmbeddingsConfig`, `RigVectorStoreConfig`) |
| Provider wiring | `crates/ragent-rig/src/registry.rs` (`register_rig_providers`) |
| Memory policy | `crates/ragent-rig/src/memory.rs` (`build_memory_policy`) |
| Embeddings | `crates/ragent-rig/src/embeddings.rs` (`build_embedding_backend_by_provider`) |
| Vector stores | `crates/ragent-rig/src/vector_store.rs` (`VectorStoreAdapter`) |
| Semantic code index | `crates/ragent-rig/src/codeindex.rs` (`SemanticCodeIndex`) |
| Semantic memory | `crates/ragent-rig/src/memory_semantic.rs` (`SemanticMemory`) |
| Research augmentor | `crates/ragent-rig/src/research.rs` (`ResearchAugmentor`) |
| Tool wrapping | `crates/ragent-rig/src/tool.rs` (`RigToolWrapper`, `ragent_toolset`) |
| Mock models | `crates/ragent-rig/src/testing.rs` (`MockCompletionModel`, `build_mock_llm_client`) |
| VCR cassettes | `crates/ragent-rig/src/vcr.rs` (`VcrClient`, `VcrCassette`) |
| Binary wiring | `src/main.rs` (provider registration + memory policy), `src/cli.rs` (semantic research augmentor) |
| Dependency policy | `deny.toml` (header block, T-018) |