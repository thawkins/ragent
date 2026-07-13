# Rig Replacement / Delegation Map and Future Migration Candidates

**Spec:** `rig`
**Task:** T-017
**Requirement:** FR-011 — identify and document which ragent components could be
replaced or delegated to Rig in future phases **without changing them in the
first phase**.
**Status:** Reference document (read-only analysis — no code changes in phase 1)
**Inputs:** `specs/rig/SPEC.md`, `docs/reports/rig-interface-audit.md` (T-002),
`research/rig/RESEARCH.md`

---

## 1. Purpose

This document is the authoritative inventory of how each ragent subsystem
relates to the [Rig](https://rig.rs) LLM-agent framework. For every ragent
crate/component it records:

- The **current ragent responsibility**.
- The **Rig equivalent** (if any).
- A **disposition** — one of *Replace*, *Delegate*, *Augment*, *Wrap*,
  *Keep native*, or *No replacement*.
- The **phase** in which any change would be considered.
- A short **rationale** grounded in the interface audit and research findings.

**Phase 1** (this spec) is strictly *additive*: it adds the `ragent-rig` adapter
crate, optional Rig-backed providers, embeddings, vector stores, memory
policies, and research semantic retrieval. **No existing ragent component is
removed, replaced, or behaviourally changed in phase 1.** The classifications
below describe *future* migration candidates, not phase-1 work.

---

## 2. Disposition legend

| Disposition | Meaning |
|---|---|
| **Replace** | Rig provides an equivalent or superior implementation; the ragent component is a candidate to be *removed* in a future phase after the Rig path is proven. |
| **Delegate** | ragent keeps the public interface/orchestration but delegates the implementation to Rig behind an adapter. The native path may remain as a fallback. |
| **Augment** | Rig adds a *parallel* capability (e.g. semantic search) alongside the existing ragent capability; both coexist and results are merged. |
| **Wrap** | ragent's existing implementation is exposed *to* Rig (e.g. via Rig's `Tool` trait) without surrendering control; ragent remains the source of truth. |
| **Keep native** | No Rig equivalent exists, or the ragent implementation has bespoke value (auth, security) that Rig cannot replicate. Rig is not used here. |
| **No replacement** | The subsystem is outside Rig's domain (UI, HTTP server, specs). No migration is contemplated. |

---

## 3. Component inventory

### 3.1 `ragent-llm` — provider clients and model registry

| Subsystem | Current responsibility | Rig equivalent | Disposition | Phase | Rationale |
|---|---|---|---|---|---|
| Generic providers (OpenAI, Anthropic, Gemini, Ollama, HuggingFace, xAI, Groq, DeepSeek, Mistral, Together, Perplexity, Cohere) | Native HTTP clients per provider | `rig-core` `CompletionModel`/`EmbeddingModel` clients | **Delegate** (future) | 2+ | Rig already covers these providers behind a single trait. Phase 1 adds the adapter (`ragent-rig`, T-004/T-005); a future phase may make Rig the *default* for generic providers once the adapter is proven stable and parity is verified (FR-028, FR-032). Native providers remain operational in phase 1 (FR-003). |
| Bespoke providers (GitHub Copilot, Azure Resource/File, Microsoft Foundry Local, Amazon Bedrock, Azure AI Foundry) | Custom auth flows (device code, SigV4, Azure AD) | No direct equivalent; would need custom Rig client impls | **Keep native** | — | These providers carry authentication and request-shaping logic (Copilot device-code flow, Bedrock SigV4, Azure Resource SAS) that Rig does not provide. They may gain Rig `CompletionModel` impls later, but the auth layer stays ragent-owned. (Research Finding 14; audit §1.3.) |
| Provider registry / `Provider` trait | Static + dynamic provider registration | Rig has no registry concept | **Keep native** | — | The registry is ragent's routing surface; Rig providers are registered *into* it (T-006), not the other way around. |
| Router classifier (`router_classifier.rs`) | Heuristic + LLM routing across providers | None | **Keep native** | — | ragent-specific routing logic; no Rig equivalent. |
| Mock LLM client (`mock_llm_client.rs`) | Deterministic test client | Rig mock models | **Delegate** (test only) | 1 | Phase 1 adds Rig mock models (T-014) as an *alternative* test path; the native mock remains for tests that do not exercise the Rig adapter. |

### 3.2 `ragent-agent` — sessions, orchestration, memory, tools

| Subsystem | Current responsibility | Rig equivalent | Disposition | Phase | Rationale |
|---|---|---|---|---|---|
| Session manager / `SessionManager` | SQLite-backed session lifecycle | None (Rig has no session store) | **Keep native** | — | Rig has no session-persistence concept. `ragent-storage` remains the store of record. |
| Orchestration / agent loop | Tool-call loop, permission gating, streaming fan-out | Rig `Agent` type + agentic loop | **Keep native** (evaluate later) | 3+ | Rig's `Agent` loop is a candidate for headless/non-TUI execution paths, but ragent's loop is tightly coupled to the permission system, event bus, and TUI. Per spec Out-of-Scope and FR-030, the orchestration layer is NOT replaced in phase 1. |
| History trimming (`session::history`) | Ad-hoc byte/token estimation + emergency compress | `rig-memory` policies (sliding window, token budget, compaction) | **Delegate** (optional backend) | 1 | Phase 1 wires Rig memory policies as an *optional* trimming backend (T-011, FR-014/FR-020). The trigger logic (`should_compress_with_reported`) and estimation helpers stay ragent-owned; only the policy execution is delegated when `memory.rig.enabled`. |
| Compression pipeline (`compression::pipeline`) | Headroom-backed summarisation/compaction | Rig compaction policy | **Augment** (pre-filter) | 1 | Rig memory policies layer *underneath* the Headroom compressor as an optional pre-filter (audit §2.4). The compressor remains the authoritative compaction step. |
| Structured memory store (`memory::store`) | SQLite structured memories + tags/confidence/decay | None (Rig has no structured-memory store) | **Keep native** | — | ragent's three-tier memory system is bespoke; Rig has no equivalent. Embeddings (T-007/T-010) *augment* retrieval, not replace storage. |
| Memory embeddings (`memory::embedding`) | `EmbeddingProvider` trait, brute-force cosine | Rig `EmbeddingModel` | **Delegate** (embedding gen) / **Augment** (search) | 1 | A `RigEmbeddingProvider` implements `EmbeddingProvider` by delegating to Rig's `EmbeddingModel::embed` (T-007). Vector search is augmented by Rig `VectorStoreIndex` (T-008) for large corpora; the storage API remains the boundary (audit §2.5, §5.1). |
| Tool registry (`ToolRegistry`, ~111 tools) | Security-audited tools with permission gating | Rig `Tool`/`ToolSet` | **Wrap** | 1 (T-013) | ragent tools are wrapped *into* Rig's `Tool` trait so Rig agents can call them, but the implementation, permission checks, and shell security stay ragent-owned (FR-031). Rig is never the executor of sensitive tools. |
| Research adapter (`research_adapter.rs`) | Builds `ResearchSession` from agent context | None | **Keep native** | — | ragent-specific wiring; augmented, not replaced (see §3.4). |

### 3.3 `ragent-codeindex` — lexical/symbol index

| Subsystem | Current responsibility | Rig equivalent | Disposition | Phase | Rationale |
|---|---|---|---|---|---|
| Tree-sitter parsing + SQLite symbol store | Lexical/symbol index | None (Rig has no parser) | **Keep native** | — | Rig does not parse source code. The lexical/symbol index is ragent's and is explicitly NOT replaced (FR-035). |
| Tantivy FTS | Full-text search | None | **Keep native** | — | Lexical FTS stays; Rig adds a *parallel* semantic index. |
| Embeddings / semantic search | None today | Rig `EmbeddingModel` + `VectorStoreIndex` | **Augment** | 1 (T-009) | When `codeindex.semantic.enabled`, the worker generates Rig embeddings alongside `FtsIndex::add_symbols` and `CodeIndex::search` issues a parallel vector query, merging results by combined score (FR-015/FR-021). The lexical index is never removed (FR-035, FR-018 fallback). |
| Index worker (`IndexWorker`) | File-watch reindex events | None | **Keep native** | — | The worker is the embedding hook point (audit §3.5) but remains ragent-owned. |

### 3.4 `ragent-research` — research loop and sources

| Subsystem | Current responsibility | Rig equivalent | Disposition | Phase | Rationale |
|---|---|---|---|---|---|
| Research engine / iteration | Web + local gather → LLM synthesis loop | Rig RAG agent + loaders | **Augment** | 1 (T-012) | Phase 1 augments the pipeline with vector-similarity retrieval over embedded sources and prior findings (FR-016/FR-017/FR-022). The gatherer traits (`WebSearchTool`, `WebFetchTool`, `LocalTool`, `Planner`, `Critic`, `AnalysisEngine`) are untouched; a `SemanticRetriever` is injected as an optional handle (audit §4.9). Whether the synthesis step uses Rig's RAG agent is an open question (spec Open Questions). |
| Source capture / `Source` enum | Web/local/spec source bodies | Rig loaders (chunking) | **Delegate** (chunking, optional) | 1 | Where Rig's loaders can ingest a document type `/research` uses, the system *may* use Rig loaders to chunk and embed (FR-029). Source capture itself stays ragent-owned. |
| Analysis engine | LLM-based synthesis | None (Rig has no analysis engine) | **Keep native** | — | The `AnalysisEngine` prompt is augmented with top-k semantically similar prior items, but the engine itself is ragent's. |

### 3.5 `ragent-storage` — SQLite persistence

| Subsystem | Current responsibility | Rig equivalent | Disposition | Phase | Rationale |
|---|---|---|---|---|---|
| Session/message persistence | SQLite chat store | None | **Keep native** | — | Rig has no persistence layer. Storage remains the read/write boundary. |
| Structured memory store | SQLite memories + embeddings | None | **Keep native** | — | See §3.2. |
| Embedding store + brute-force cosine | `store_memory_embedding` / `search_memories_by_embedding` | Rig `VectorStoreIndex` | **Delegate** (search backend, optional) | 1 (T-008) | A Rig `VectorStoreIndex` may *replace* the brute-force cosine path for large corpora (FR-008/FR-009); the storage API remains the boundary so callers are unaffected (audit §5.1). |
| Knowledge graph | Entities + relationships | None | **Keep native** | — | Untouched by Rig in phase 1 (audit §5.1). |
| Snapshots / auth / TODOs | File snapshots, encrypted creds, TODO items | None | **Keep native** | — | Pure ragent domain; no Rig equivalent. |

### 3.6 `ragent-tui` / `ragent-server` — UI and HTTP

| Subsystem | Current responsibility | Rig equivalent | Disposition | Phase | Rationale |
|---|---|---|---|---|---|
| Terminal UI (`ragent-tui`) | Ratatui full-screen interface | None | **No replacement** | — | Rig is a library, not a UI framework (Research Finding 15). The TUI consumes the same `Event` stream regardless of provider backend (FR-013), so it is untouched. |
| HTTP server (`ragent-server`) | Axum REST + SSE API | None | **No replacement** | — | Same as TUI; Rig has no server concept. |
| Slash-command system | `/research`, `/codeindex`, `/team`, etc. | None | **No replacement** | — | ragent-specific command surface. |

### 3.7 `ragent-team` / swarms

| Subsystem | Current responsibility | Rig equivalent | Disposition | Phase | Rationale |
|---|---|---|---|---|---|
| Team coordination / mailbox / shared tasks | Multi-agent coordination | Rig multi-agent workflows | **Evaluate later** | 3+ | Rig's multi-agent patterns are a potential future delegation target, but ragent's team model (named teammates, mailbox, swarm decomposition) is bespoke and coupled to the event bus. Not touched in phase 1 (spec Candidate Map: "Evaluate later"). |

### 3.8 `ragent-specs` / planning

| Subsystem | Current responsibility | Rig equivalent | Disposition | Phase | Rationale |
|---|---|---|---|---|---|
| Spec lifecycle management | Discovery, validation, status transitions | None | **No replacement** | — | Entirely ragent-specific; no Rig equivalent. |

### 3.9 `ragent-types` — shared primitives

| Subsystem | Current responsibility | Rig equivalent | Disposition | Phase | Rationale |
|---|---|---|---|---|---|
| `ChatRequest` / `ChatMessage` / `StreamEvent` / `Event` | LLM wire types + event bus | Rig `CompletionRequest` / `Message` / streaming chunks | **Keep native** (map at adapter) | — | The ragent types are the canonical internal representation; the `ragent-rig` adapter translates at the boundary (FR-004/FR-005/FR-013). ragent types are NOT replaced by Rig types. |
| Sanitisation primitives | Input/path sanitisation | None | **Keep native** | — | Security primitive; no Rig equivalent. |

### 3.10 `ragent-tools-*` — tool surface

| Subsystem | Current responsibility | Rig equivalent | Disposition | Phase | Rationale |
|---|---|---|---|---|---|
| Core tools (`ragent-tools-core`) | bash, file, search, codeindex tools | Rig `Tool` trait | **Wrap** (T-013) | 1 | Tools are wrapped into Rig's `Tool` trait so Rig agents can invoke them, but execution, permission checks, and the 7-layer bash security model stay ragent-owned (FR-031). Rig never bypasses ragent security. |
| Extended tools (`ragent-tools-extended`) | web, memory, office, PDF tools | Rig loaders (document ingestion) | **Wrap** / **Delegate** (loaders, optional) | 1+ | Document tools may use Rig loaders for chunking (FR-029); the tool surface stays ragent-owned. |
| VCS tools (`ragent-tools-vcs`) | GitHub/GitLab native tools | None | **Keep native** | — | No Rig equivalent. |

---

## 4. Phase-1 invariants (what does NOT change)

The following are explicitly preserved in phase 1, per the spec's Out-of-Scope
and Unwanted requirements:

1. **`ragent-tui`, `ragent-server`, slash commands, orchestration layer** —
   not replaced by Rig's `Agent` type (Out-of-Scope; FR-030).
2. **Shell security model and permission system** — not bypassed when tools are
   exposed via Rig's `Tool` trait (FR-031).
3. **Native providers** — not removed or deprecated solely because a Rig
   equivalent exists (FR-032, FR-034). All native providers remain operational
   when Rig-backed providers are not configured (FR-003).
4. **Lexical/symbol code index** — not replaced by a pure vector-store index
   (FR-035). Semantic search augments; lexical search remains and is the
   fallback (FR-018).
5. **Session/message persistence** — `ragent-storage` remains the store of
   record; Rig has no persistence layer.
6. **ragent internal types** — `ChatRequest`, `StreamEvent`, `Event` are the
   canonical internal representation; the adapter translates at the boundary,
   it does not swap the types.

---

## 5. Future migration candidates (phases 2+)

These are candidates for *later* phases, contingent on the phase-1 adapter
proving stable and parity being measured. They are **not** committed and
require separate specs.

| Candidate | From → To | Trigger / gate | Risk |
|---|---|---|---|
| Generic-provider default | Native `ragent-llm` generic providers → Rig adapter as default | Adapter parity verified across ≥3 providers; no auth regressions | Loss of provider-specific knobs not exposed by Rig's trait |
| Headless agent loop | ragent orchestration → Rig `Agent` loop (non-TUI paths only) | Measured equivalence on tool-call routing + streaming cadence (NFR-003/NFR-005) | Decoupling from permission system; rework of event-bus integration |
| Team/swarm delegation | `ragent-team` → Rig multi-agent workflows | Rig multi-agent API stabilises; ragent team model mapped cleanly | Bespoke mailbox/task semantics may not map |
| Vector-store default | Brute-force SQLite cosine → Rig `VectorStoreIndex` as default for large corpora | Corpus-size threshold where brute-force is too slow | New runtime dependency; storage API must stay the boundary |
| Mock/VCR test default | Native mock client → Rig mock models + VCR cassettes | Test harness parity confirmed (T-014/T-015/T-016) | Existing tests must migrate; coverage gap risk |

---

## 6. Decision gates for any future migration

Before any *Replace* or *Delegate* disposition is acted on in a future phase,
the following gates must be met (derived from the spec Assumptions and NFRs):

1. **Parity** — the Rig path matches the native path on authentication,
   streaming cadence (NFR-003), tool-call routing, and error handling.
2. **No regression** — `cargo test -p ragent-rig` and the affected downstream
   crate tests pass (NFR-005).
3. **Size budget** — binary-size impact is measured and within NFR-002 (≤15%
   when no Rig providers are enabled at runtime).
4. **Dependency policy** — `deny.toml` policy remains satisfied (AC-5, T-018);
   no new license families or git sources are introduced without review.
5. **Opt-out preserved** — Rig remains optional (FR-034); a user can run ragent
   with zero Rig dependencies compiled in.
6. **Security preserved** — no ragent tool, permission check, or shell-security
   layer is bypassed (FR-031).
7. **Spec-tracked** — every migration is a separate spec with its own tasks,
   not a stealth change.

---

## 7. Cross-references

- Spec: [`specs/rig/SPEC.md`](../specs/rig/SPEC.md) — requirements FR-001…
  FR-035, NFR-001…NFR-006, AC-5, AC-7.
- Interface audit: [`docs/reports/rig-interface-audit.md`](reports/rig-interface-audit.md)
  (T-002) — exact public signatures the adapter maps onto.
- Research: [`research/rig/RESEARCH.md`](../research/rig/RESEARCH.md) —
  Findings 13 (hybrid strategy), 14 (existing provider layer), 15 (TUI/server
  out of scope), 16 (shell security), 18 (Rig breaking-change policy).
- Dependency policy: [`deny.toml`](../deny.toml) header block (T-018).