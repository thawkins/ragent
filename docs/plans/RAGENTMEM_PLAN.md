# Persistent Memory Implementation Plan for ragent

**Document Version:** 1.0  
**Date:** July 2025  
**Status:** Implementation Plan  
**Source:** RAGENTMEM.md, RAGENTMEMOG.md

---

## Executive Summary

This plan translates the research in RAGENTMEM.md and RAGENTMEMOG.md into a concrete, milestone-driven implementation roadmap for ragent's persistent memory system. The design preserves backward compatibility with the existing `memory_read`/`memory_write` tools while progressively adding structured memory blocks, a journal system, semantic search, and advanced features.

**Total Estimated Duration:** 8–12 weeks (iterative, one milestone at a time)

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                         ragent-tui                              │
│  (Memory browser, journal viewer, search interface)            │
└─────────────────────┬───────────���───────────────────────────────┘
                      │
┌─────────────────────▼───────────────────────────────────────────┐
│                       ragent-server                             │
│  (Memory API endpoints, SSE events)                            │
└─────────────────────┬───────────────────────────────────────────┘
                      │
┌─────────────────────▼───────────────────────────────────────────┐
│                       ragent-core                               │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐              │
│  │  Memory     │  │   Journal   │  │   Block     │              │
│  │  Manager    │  │   Manager   │  │   Manager   │              │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘              │
│         │                │                │                      │
│  ┌──────▼────────────────▼────────────────▼──────┐              │
│  │              Memory Storage                  │              │
│  │  ┌─────────┐ ┌─────────┐ ┌─────────┐       │              │
│  │  │  Files  │ │ SQLite  │ │  Vector │       │              │
│  │  │ (.md)   │ │ (meta)  │ │ (embed) │       │              │
│  │  └─────────┘ └─────────┘ └─────────┘       │              │
│  └──────────────────────────────────────────────┘              │
└─────────────────────────────────────────────────────────────────┘
```

---

## Current State (Baseline)

### Existing Implementation

| Component | Location | Status |
|-----------|----------|--------|
| `memory_write` tool | `crates/ragent-core/src/tool/memory.rs` | ✅ Working — appends to MEMORY.md |
| `memory_read` tool | `crates/ragent-core/src/tool/memory.rs` | ✅ Working — reads MEMORY.md |
| `MemoryScope` enum | `crates/ragent-core/src/tool/memory.rs` | ✅ User / Project / None |
| Auto-loading | `crates/ragent-core/src/agent/mod.rs` (~line 120) | ✅ Loads project + user MEMORY.md into prompt |
| Team memory tools | `crates/ragent-core/src/tool/team_memory.rs` | ✅ Team-scoped read/write |
| SQLite storage | `crates/ragent-core/src/storage/` | ✅ Sessions table |
| Event bus | `crates/ragent-core/src/event.rs` | ✅ Pub/sub with `Event` enum |
| Tool registration | `crates/ragent-core/src/tool/mod.rs` | ✅ Registry pattern |
| Config parsing | `crates/ragent-core/src/config.rs` | ✅ `AppConfig` from ragent.json |

### Current Limitations (to solve)

- Single flat MEMORY.md file per scope — no named blocks or categorisation
- No journal/append-only log for insights and decisions
- No semantic search — full-text only
- No structured storage with metadata (category, tags, confidence)
- No automatic memory extraction from conversations
- No memory compaction or deduplication
- No TUI panel for memory browsing

---

## Milestone 1: Memory Block System (File-Based MVP)

**Goal:** Extend the existing memory tools to support named, structured memory blocks with YAML frontmatter, while maintaining 100% backward compatibility with the current MEMORY.md approach.

**Duration:** 1–2 weeks  
**Priority:** P1 (foundational — everything else depends on it)

### Tasks

#### M1-T1: Define Memory Block Types
- **File:** `crates/ragent-core/src/memory/mod.rs` (new)
- **File:** `crates/ragent-core/src/memory/block.rs` (new)
- **Description:** Create the `MemoryBlock` struct and `MemoryScope` enum (extended with `Global` variant replacing `User`, adding `Session`).
- **Acceptance Criteria:**
  - `MemoryBlock` struct with fields: `label`, `description`, `scope`, `content`, `limit`, `read_only`, `created_at`, `updated_at`
  - `MemoryScope` enum: `Global`, `Project(PathBuf)`, `Session(String)`, `Team(String)`
  - YAML frontmatter parse/serialize (using `serde_yaml`)
  - Unit tests for block creation, serialisation, and deserialisation
  - Doc-blocks on all public items

#### M1-T2: Implement Block Storage Backend
- **File:** `crates/ragent-core/src/memory/storage.rs` (new)
- **Description:** File-based storage for memory blocks in `.ragent/memory/` (project) and `~/.ragent/memory/` (global). Each block is a `.md` file with YAML frontmatter.
- **Acceptance Criteria:**
  - `BlockStorage` trait with `load`, `save`, `list`, `delete` methods
  - `FileBlockStorage` implementation reading/writing `.md` files
  - Scope-aware path resolution (global → `~/.ragent/memory/`, project → `.ragent/memory/`)
  - Atomic writes (write to temp file, then rename)
  - Size limit enforcement on `save` — reject content exceeding `limit`
  - Backward compatibility: existing `MEMORY.md` files load as a block with label `MEMORY`
  - Unit tests for all operations

#### M1-T3: Extend `memory_read` Tool for Block Support
- **File:** `crates/ragent-core/src/tool/memory.rs`
- **Description:** Add optional `label` and `path` parameters to `memory_read`. When `label` is provided, read the named block; when omitted, fall back to existing MEMORY.md behaviour (backward compatible).
- **Acceptance Criteria:**
  - New parameters: `label: Option<String>`, `path: Option<String>` (for sub-files within memory dir)
  - Backward compatible — existing `memory_read` calls with no `label` still work
  - When `label` is given, reads from `<memory_dir>/<label>.md`
  - Returns block content including frontmatter metadata
  - Integration test with actual file I/O

#### M1-T4: Extend `memory_write` Tool for Block Support
- **File:** `crates/ragent-core/src/tool/memory.rs`
- **Description:** Add optional `label`, `description`, `limit`, and `mode` parameters. When `label` is provided, write to the named block file; when omitted, fall back to existing MEMORY.md append behaviour.
- **Acceptance Criteria:**
  - New parameters: `label: Option<String>`, `description: Option<String>`, `limit: Option<usize>`, `mode: Option<String>` (`"append"` | `"overwrite"`)
  - Backward compatible — existing `memory_write` calls still work
  - Creates new block files with YAML frontmatter when `label` is given
  - `mode: "append"` adds content to existing block (current default behaviour)
  - `mode: "overwrite"` replaces entire block content
  - Enforces `limit` on block content length
  - Integration test

#### M1-T5: Implement `memory_replace` Tool
- **File:** `crates/ragent-core/src/tool/memory.rs`
- **Description:** New tool for surgical edits within a memory block, analogous to the existing `edit` tool for code files.
- **Acceptance Criteria:**
  - Parameters: `label: String`, `old_str: String`, `new_str: String`, `scope: Option<String>`
  - Finds exact `old_str` match within block content and replaces it
  - Returns error if `old_str` not found or found multiple times
  - Preserves YAML frontmatter during replacement
  - Unit tests and integration test

#### M1-T6: Create Default Memory Blocks
- **File:** `crates/ragent-core/src/memory/defaults.rs` (new)
- **Description:** Seed default memory blocks on first use if they don't exist.
- **Acceptance Criteria:**
  - Default blocks: `persona.md`, `human.md`, `project.md`
  - Only created if the file does not already exist (no overwrite)
  - Template content from RAGENTMEM.md §6.7
  - Triggered on first memory tool call in a session
  - Unit tests verifying idempotent creation

#### M1-T7: Update System Prompt Auto-Loading
- **File:** `crates/ragent-core/src/agent/mod.rs`
- **Description:** Extend the existing system prompt assembly to load all memory blocks (not just MEMORY.md) and inject tool descriptions.
- **Acceptance Criteria:**
  - Scan `.ragent/memory/*.md` and `~/.ragent/memory/*.md` at session start
  - Inject all block contents under a `## Memory Blocks` heading
  - Include tool descriptions for `memory_read`, `memory_write`, `memory_replace`
  - Respect `read_only` blocks (inform agent they cannot be modified)
  - Existing MEMORY.md loading still works (backward compatible)
  - Performance: load time adds <50ms for up to 20 blocks

#### M1-T8: Register New Tools and Wire Module
- **File:** `crates/ragent-core/src/memory/mod.rs`
- **File:** `crates/ragent-core/src/tool/mod.rs`
- **File:** `crates/ragent-core/src/lib.rs`
- **Description:** Register `memory_replace` as a new tool. Wire the `memory` module into the crate's public API.
- **Acceptance Criteria:**
  - `memory_replace` appears in tool list and is callable by the agent
  - `memory` module is a public submodule of `ragent_core`
  - No clippy warnings on the new code
  - `cargo test -p ragent-core` passes

#### M1-T9: Write Migration Path for Existing MEMORY.md
- **File:** `crates/ragent-core/src/memory/migrate.rs` (new)
- **Description:** Optional one-time migration that splits an existing flat MEMORY.md into structured blocks based on markdown headings.
- **Acceptance Criteria:**
  - Detects existing MEMORY.md with structured headings
  - Proposes split into separate block files (e.g., `## Patterns` → `patterns.md`)
  - Requires user confirmation before modifying files
  - Leaves original MEMORY.md intact as backup
  - Unit tests with sample MEMORY.md content

---

## Milestone 2: Journal System

**Goal:** Add an append-only journal for recording insights, decisions, and discoveries with tag-based filtering and SQLite-backed storage.

**Duration:** 1–2 weeks  
**Priority:** P1 (complements memory blocks for experiential knowledge)

### Tasks

#### M2-T1: Design and Create Journal SQLite Schema
- **File:** `crates/ragent-core/src/memory/journal.rs` (new)
- **File:** `crates/ragent-core/src/storage/` (extend existing SQLite connection)
- **Description:** Add journal tables to the existing SQLite database used for sessions.
- **Acceptance Criteria:**
  - `journal_entries` table: `id TEXT PK`, `title TEXT`, `content TEXT`, `project TEXT`, `session_id TEXT`, `timestamp DATETIME`, `created_at DATETIME`
  - `journal_tags` table: `entry_id TEXT`, `tag TEXT`, composite PK
  - `journal_fts` virtual table using FTS5 on `(title, content)`
  - Migration runs on first journal operation (auto-create tables)
  - Use existing `rusqlite` connection pool from storage module
  - Unit tests for schema creation and idempotency

#### M2-T2: Implement JournalEntry Type
- **File:** `crates/ragent-core/src/memory/journal.rs`
- **Description:** Define the `JournalEntry` struct with serialisation support.
- **Acceptance Criteria:**
  - `JournalEntry` struct: `id`, `title`, `content`, `tags`, `project`, `session_id`, `timestamp`
  - UUID v4 generation for `id`
  - `chrono::Utc::now()` for timestamps
  - `Serialize`/`Deserialize` derives for JSON output
  - Unit tests

#### M2-T3: Implement `journal_write` Tool
- **File:** `crates/ragent-core/src/tool/journal.rs` (new)
- **Description:** Tool for appending journal entries.
- **Acceptance Criteria:**
  - Parameters: `title: String`, `content: String`, `tags: Option<Vec<String>>`
  - Generates UUID, captures project name and session ID from `ToolContext`
  - Stores entry in SQLite `journal_entries` table
  - Stores tags in `journal_tags` table
  - Updates FTS5 index
  - Returns confirmation with entry ID
  - Integration test with in-memory SQLite

#### M2-T4: Implement `journal_search` Tool
- **File:** `crates/ragent-core/src/tool/journal.rs`
- **Description:** Tool for searching journal entries using FTS5 and tag filtering.
- **Acceptance Criteria:**
  - Parameters: `query: String`, `tags: Option<Vec<String>>`, `limit: Option<usize>` (default 10)
  - Uses SQLite FTS5 MATCH for full-text search
  - Filters by tags when provided (JOIN with `journal_tags`)
  - Returns ranked results with title, content snippet, tags, and timestamp
  - Returns empty vec (not error) when no results found
  - Integration test

#### M2-T5: Implement `journal_read` Tool
- **File:** `crates/ragent-core/src/tool/journal.rs`
- **Description:** Tool for reading a specific journal entry by ID.
- **Acceptance Criteria:**
  - Parameters: `id: String`
  - Returns full entry content including tags
  - Returns error if entry not found
  - Unit test

#### M2-T6: Register Journal Tools
- **File:** `crates/ragent-core/src/tool/mod.rs`
- **File:** `crates/ragent-core/src/agent/mod.rs`
- **Description:** Register `journal_write`, `journal_search`, `journal_read` in the tool registry and add them to the system prompt tool descriptions.
- **Acceptance Criteria:**
  - All three tools appear in tool list
  - System prompt includes journal tool descriptions
  - `cargo test -p ragent-core` passes
  - No clippy warnings

#### M2-T7: Add Journal-Related Event Bus Events
- **File:** `crates/ragent-core/src/event.rs`
- **Description:** Add `JournalEntryCreated` and `JournalSearched` event variants for real-time UI updates.
- **Acceptance Criteria:**
  - `Event` enum extended with `JournalEntryCreated { id: String, title: String }` and `JournalSearched { query: String, result_count: usize }`
  - Events are emitted from journal tool handlers
  - TUI can subscribe to these events (future TUI work)
  - Unit test for event emission

---

## Milestone 3: Structured Memory Store (SQLite)

**Goal:** Add a SQLite-backed structured memory store with categories, tags, confidence scoring, and FTS5 retrieval — enabling automatic memory extraction and intelligent retrieval beyond simple file blocks.

**Duration:** 2–3 weeks  
**Priority:** P2 (enables intelligent memory management)

### Tasks

#### M3-T1: Design Structured Memory SQLite Schema
- **File:** `crates/ragent-core/src/memory/store.rs` (new)
- **File:** `crates/ragent-core/src/storage/` (extend)
- **Description:** Add structured memory tables to the existing SQLite database.
- **Acceptance Criteria:**
  - `memories` table: `id INTEGER PK`, `content TEXT NOT NULL`, `category TEXT NOT NULL` (fact/pattern/preference/insight/error/workflow), `source TEXT`, `confidence REAL DEFAULT 0.5`, `project TEXT`, `session_id TEXT`, `created_at DATETIME`, `updated_at DATETIME`, `access_count INTEGER DEFAULT 0`, `last_accessed DATETIME`
  - `memory_tags` table: `memory_id INTEGER`, `tag TEXT`, composite PK
  - `memories_fts` virtual table using FTS5 on `(content)`
  - Migration runs on first structured memory operation
  - Reuse existing SQLite connection pool
  - Unit tests for schema creation

#### M3-T2: Implement `MemoryStore` Trait and SQLite Backend
- **File:** `crates/ragent-core/src/memory/store.rs`
- **Description:** Abstract storage trait with SQLite implementation.
- **Acceptance Criteria:**
  - `MemoryStore` trait: `store`, `recall`, `forget`, `update_confidence`, `increment_access`
  - `SqliteMemoryStore` implementation using `rusqlite`
  - `store()` inserts memory with category, tags, confidence
  - `recall()` performs FTS5 search + category filter + tag filter + confidence threshold
  - `forget()` deletes by ID or by filter (older_than_days, max_confidence)
  - `update_confidence()` adjusts confidence score
  - `increment_access()` increments access_count and updates last_accessed
  - Unit tests with in-memory SQLite

#### M3-T3: Implement `memory_store` Tool
- **File:** `crates/ragent-core/src/tool/memory.rs` (extend)
- **Description:** Tool for storing structured memories with metadata.
- **Acceptance Criteria:**
  - Parameters: `content: String`, `category: String`, `tags: Option<Vec<String>>`, `confidence: Option<f64>` (default 0.7), `source: Option<String>`
  - Validates category is one of: fact, pattern, preference, insight, error, workflow
  - Stores in SQLite `memories` table
  - Returns confirmation with memory ID
  - Integration test

#### M3-T4: Implement `memory_recall` Tool
- **File:** `crates/ragent-core/src/tool/memory.rs` (extend)
- **Description:** Tool for querying structured memories with FTS5 and filters.
- **Acceptance Criteria:**
  - Parameters: `query: String`, `categories: Option<Vec<String>>`, `tags: Option<Vec<String>>`, `limit: Option<usize>` (default 5), `min_confidence: Option<f64>` (default 0.5)
  - Performs FTS5 MATCH on `content`
  - Filters by category, tags, and confidence threshold
  - Increments `access_count` for returned results
  - Returns formatted results with content, category, tags, confidence, timestamp
  - Integration test

#### M3-T5: Implement `memory_forget` Tool
- **File:** `crates/ragent-core/src/tool/memory.rs` (extend)
- **Description:** Tool for removing outdated or incorrect memories.
- **Acceptance Criteria:**
  - Parameters: `id: Option<i64>`, `filter: Option<ForgetFilter>` where `ForgetFilter` has `older_than_days: Option<u32>`, `max_confidence: Option<f64>`, `category: Option<String>`, `tags: Option<Vec<String>>`
  - Deletes by specific ID or by filter criteria
  - Returns count of deleted memories
  - Does not delete without at least one criterion (safety)
  - Integration test

#### M3-T6: Add Memory Events to Event Bus
- **File:** `crates/ragent-core/src/event.rs`
- **Description:** Extend `Event` enum with structured memory events.
- **Acceptance Criteria:**
  - `MemoryStored { id: i64, category: String }`
  - `MemoryRecalled { query: String, result_count: usize }`
  - `MemoryForgotten { count: usize }`
  - Events emitted from tool handlers
  - Unit test

#### M3-T7: Context-Aware Memory Retrieval in Prompt Assembly
- **File:** `crates/ragent-core/src/agent/mod.rs`
- **Description:** On session start and during conversation, retrieve relevant structured memories and inject them into the system prompt.
- **Acceptance Criteria:**
  - On session start: retrieve top-N memories by recency + confidence for the current project
  - Inject under a `## Relevant Memories` heading in the system prompt
  - Respect configurable `max_memories_per_prompt` (default 5)
  - No significant latency impact (<100ms added to prompt assembly)
  - Integration test

#### M3-T8: Configuration Schema for Memory System
- **File:** `crates/ragent-core/src/config.rs`
- **Description:** Add memory configuration to `AppConfig` from RAGENTMEMOG.md §Configuration.
- **Acceptance Criteria:**
  - `MemoryConfig` struct: `enabled`, `tier` ("core" | "structured" | "semantic"), `structured: StructuredConfig`, `semantic: SemanticConfig`, `auto_extract: AutoExtractConfig`, `retrieval: RetrievalConfig`
  - Parsed from `ragent.json` under `"memory"` key
  - Sensible defaults when key is absent (enabled: true, tier: "core")
  - Unit test for config parsing

---

## Milestone 4: Semantic Search (Embeddings)

**Goal:** Add embedding-based semantic search for both memory blocks and journal entries, enabling similarity-based retrieval that goes beyond keyword matching.

**Duration:** 2–3 weeks  
**Priority:** P2 (significant quality-of-life improvement)

### Tasks

#### M4-T1: Research and Select Embedding Approach
- **Description:** Evaluate embedding options for local, zero-dependency deployment.
- **Options to evaluate:**
  - `candle` crate (HuggingFace models in pure Rust)
  - `ort` crate (ONNX Runtime for running sentence-transformers)
  - `rust-bert` crate (Rust-native BERT models)
  - Remote embedding APIs (OpenAI, Anthropic) as optional fallback
- **Acceptance Criteria:**
  - Written decision document in `docs/performance/embedding-evaluation.md`
  - Selected approach supports `all-MiniLM-L6-v2` or equivalent (~384-dim vectors)
  - Cold start < 5 seconds
  - Embedding generation < 50ms per entry (after model load)
  - Zero external service dependencies for default config

#### M4-T2: Define `EmbeddingProvider` Trait
- **File:** `crates/ragent-core/src/memory/embedding.rs` (new)
- **Description:** Abstract embedding interface.
- **Acceptance Criteria:**
  - `EmbeddingProvider` trait: `async fn embed(&self, text: &str) -> Result<Vec<f32>>`
  - `async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>>`
  - `fn dimensions(&self) -> usize`
  - `fn name(&self) -> &str`
  - `NoOpEmbedding` implementation (returns empty vec — for when embeddings disabled)
  - Unit tests

#### M4-T3: Implement Local Embedding Provider
- **File:** `crates/ragent-core/src/memory/embedding.rs`
- **File:** `crates/ragent-core/Cargo.toml` (add optional deps)
- **Description:** Implement local embedding using the selected approach from M4-T1.
- **Acceptance Criteria:**
  - Feature-gated: `#[cfg(feature = "embeddings")]`
  - Lazy model loading on first `embed()` call
  - Thread-safe (`Send + Sync`)
  - Caches model in memory after first load
  - Handles errors gracefully (model not found, OOM, etc.)
  - Integration test with sample text

#### M4-T4: Add Vector Storage to SQLite
- **File:** `crates/ragent-core/src/memory/store.rs` (extend)
- **Description:** Add embedding column and similarity search using `sqlite-vec` or brute-force cosine similarity for small datasets.
- **Acceptance Criteria:**
  - Add `embedding BLOB` column to `memories` table (nullable — not all memories need embeddings)
  - Add `embedding BLOB` column to `journal_entries` table
  - `store_embedding()` method: serialise `Vec<f32>` to bytes, store in blob column
  - `search_similar()` method: compute cosine similarity between query embedding and stored embeddings
  - For MVP: brute-force search (acceptable for <10K memories)
  - Optional: `sqlite-vec` feature for ANN search at scale
  - Unit tests for serialisation and similarity computation

#### M4-T5: Implement `memory_search` Tool (Semantic)
- **File:** `crates/ragent-core/src/tool/memory.rs` (extend)
- **Description:** Semantic search across memory blocks and structured memories.
- **Acceptance Criteria:**
  - Parameters: `query: String`, `scope: Option<String>`, `limit: Option<usize>` (default 5)
  - Generates embedding for query text
  - Searches structured memories by cosine similarity
  - Also searches memory blocks by embedding their content (lazy embedding on first search)
  - Returns ranked results with similarity scores
  - Falls back to FTS5 search if embeddings are disabled
  - Integration test

#### M4-T6: Upgrade `journal_search` with Semantic Search
- **File:** `crates/ragent-core/src/tool/journal.rs` (extend)
- **Description:** Enhance journal search with semantic similarity alongside FTS5.
- **Acceptance Criteria:**
  - When embeddings are enabled, `journal_search` performs hybrid search: FTS5 + cosine similarity
  - Results are merged and ranked by combined score
  - Falls back to FTS5-only when embeddings disabled
  - Integration test

#### M4-T7: Embedding Configuration and Feature Flags
- **File:** `crates/ragent-core/Cargo.toml`
- **File:** `crates/ragent-core/src/config.rs`
- **Description:** Add Cargo feature flag and config options for embeddings.
- **Acceptance Criteria:**
  - `[features] embeddings = ["candle"]` (or whichever crate selected)
  - `SemanticConfig` in `MemoryConfig`: `enabled: bool`, `embedding_model: String`, `dimensions: usize`
  - Default: embeddings disabled (opt-in)
  - `cargo build -p ragent-core` succeeds without embeddings feature
  - `cargo build -p ragent-core --features embeddings` builds with embedding support

---

## Milestone 5: Automatic Memory Extraction

**Goal:** Automatically extract key learnings, patterns, and preferences from conversations and tool usage, storing them as structured memories without explicit user action.

**Duration:** 1–2 weeks  **Priority:** P2 (reduces manual memory management burden)

### Tasks

#### M5-T1: Design Extraction Hook Points
- **File:** `crates/ragent-core/src/memory/extract.rs` (new)
- **Description:** Identify and implement hook points in the session lifecycle where memory extraction should occur.
- **Acceptance Criteria:**
  - Hook after tool execution: extract patterns from file edits, bash results
  - Hook after error resolution: capture debugging insights
  - Hook at session end: summarise key learnings from conversation
  - Each hook is an async function that receives `ToolContext` + result and optionally produces `MemoryCandidate`
  - Hooks are gated by `auto_extract.enabled` config flag
  - Unit tests for hook invocation

#### M5-T2: Implement Memory Candidate Type and Confirmation Flow
- **File:** `crates/ragent-core/src/memory/extract.rs`
- **Description:** Define `MemoryCandidate` (proposed memory before user approval) and the confirmation workflow.
- **Acceptance Criteria:**
  - `MemoryCandidate` struct: `content`, `category`, `tags`, `confidence`, `source`, `reason` (why this was extracted)
  - Candidates are presented to the user via a question/prompt tool
  - User can approve, reject, or edit before storing
  - Approved candidates are stored via `MemoryStore::store()`
  - Configurable: `auto_extract.require_confirmation: bool` (default: true)
  - Unit tests

#### M5-T3: Pattern Extraction from File Edits
- **File:** `crates/ragent-core/src/memory/extract.rs`
- **Description:** After file write/edit tool calls, extract patterns and conventions.
- **Acceptance Criteria:**
  - Detect recurring code patterns (e.g., error handling style, import grouping)
  - Generate `MemoryCandidate` with `category: "pattern"` and appropriate tags
  - Low confidence (0.5) — requires confirmation
  - Deduplication: don't propose the same pattern twice
  - Integration test with mock tool results

#### M5-T4: Error Resolution Extraction
- **File:** `crates/ragent-core/src/memory/extract.rs`
- **Description:** After a bash command fails and is subsequently fixed, capture the error and solution.
- **Acceptance Criteria:**
  - Detect: bash exit code != 0 followed by a successful correction
  - Generate `MemoryCandidate` with `category: "error"` and tags from the file/command context
  - Medium confidence (0.7) — captures concrete debugging insights
  - Integration test with mock bash results

#### M5-T5: Session Summary Extraction
- **File:** `crates/ragent-core/src/memory/extract.rs`
- **Description:** At session end, use the LLM to summarise key learnings and store them.
- **Acceptance Criteria:**
  - On session close, compile conversation summary
  - Call LLM with extraction prompt to identify key learnings
  - Generate multiple `MemoryCandidate` entries from the summary
  - Present to user for confirmation (or auto-store if configured)
  - Handles LLM call failure gracefully (skip extraction rather than crash)
  - Integration test

#### M5-T6: Memory Confidence Decay
- **File:** `crates/ragent-core/src/memory/store.rs` (extend)
- **Description:** Implement time-based confidence decay for memories that are not accessed.
- **Acceptance Criteria:**
  - `decay_confidence()` method: reduce confidence by a decay factor based on time since last access
  - Configurable: `memory.decay.factor: f64` (default: 0.95 per day)
  - Configurable: `memory.decay.min_confidence: f64` (default: 0.1 — don't decay below this)
  - Run as periodic background task (every 24h or on session start)
  - Memories below `min_confidence` for >30 days are candidates for `memory_forget`
  - Unit tests for decay calculation

---

## Milestone 6: Memory Compaction and Deduplication

**Goal:** Keep the memory store clean and efficient by merging duplicate memories, compacting large blocks, and evicting stale entries.

**Duration:** 1–2 weeks  
**Priority:** P3 (quality-of-life, enables long-term use)

### Tasks

#### M6-T1: Memory Deduplication
- **File:** `crates/ragent-core/src/memory/compact.rs` (new)
- **Description:** Detect and merge semantically duplicate memories.
- **Acceptance Criteria:**
  - When a new memory is stored, check for existing memories with high similarity (>0.95 cosine)
  - If duplicate found: merge content, take highest confidence, merge tags
  - If near-duplicate (0.8–0.95 similarity): propose merge to user
  - Works with or without embeddings (FTS5 fallback when embeddings disabled)
  - Unit tests

#### M6-T2: Memory Block Compaction
- **File:** `crates/ragent-core/src/memory/compact.rs`
- **Description:** When a memory block exceeds its size limit, automatically summarise it.
- **Acceptance Criteria:**
  - Detect blocks where `content.len() > limit * 0.9` (90% full)
  - Call LLM to produce a compressed summary preserving key information
  - Replace block content with summary (keeping frontmatter)
  - Log original content to journal before compaction
  - Configurable: `memory.compaction.enabled: bool` (default: true)
  - Integration test

#### M6-T3: Stale Memory Eviction
- **File:** `crates/ragent-core/src/memory/compact.rs`
- **Description:** Periodically evict memories that have decayed below the minimum confidence threshold.
- **Acceptance Criteria:**
  - Find memories with `confidence < min_confidence` AND `last_accessed > 30 days ago`
  - Present list to user for confirmation before deletion
  - Or auto-evict if `memory.eviction.auto: true` (default: false)
  - Log evicted memories to journal before deletion
  - Unit tests

#### M6-T4: Compaction Triggers
- **File:** `crates/ragent-core/src/memory/compact.rs`
- **Description:** Define when compaction runs.
- **Acceptance Criteria:**
  - Trigger on session start (if not run in >24 hours)
  - Trigger after storing >10 new memories in a session
  - Trigger when total memory count exceeds configurable threshold
  - Runs asynchronously — does not block the agent loop
  - Configurable thresholds in `ragent.json`
  - Integration test

---

## Milestone 7: TUI Memory Integration

**Goal:** Add TUI panels and slash commands for browsing, searching, and managing memories and journal entries.

**Duration:** 1–2 weeks  
**Priority:** P2 (visibility drives adoption)

### Tasks

#### M7-T1: Memory Browser Panel
- **File:** `crates/ragent-tui/src/panels/memory_browser.rs` (new)
- **Description:** TUI panel listing all memory blocks with content preview.
- **Acceptance Criteria:**
  - List all memory blocks (global + project) in a scrollable list
  - Show label, scope badge, size, last-updated timestamp
  - Enter/expand to view full block content
  - Highlight blocks that are near size limit (>90%)
  - Keyboard navigation (vim-style: j/k, Enter to expand, Esc to close)
  - Subscribes to `MemoryUpdated` events for live updates

#### M7-T2: Journal Viewer Panel
- **File:** `crates/ragent-tui/src/panels/journal_viewer.rs` (new)
- **Description:** TUI panel for browsing and searching journal entries.
- **Acceptance Criteria:**
  - Chronological list of journal entries
  - Show title, tags, timestamp in list view
  - Expand to view full content
  - Filter by tag (dropdown or shortcut)
  - Search bar for FTS5 queries
  - Keyboard navigation

#### M7-T3: Memory Status in Status Bar
- **File:** `crates/ragent-tui/src/` (extend existing status bar)
- **Description:** Show memory count and last update in the TUI status bar.
- **Acceptance Criteria:**
  - Display: `MEM: 12 blocks, 45 entries` in status bar
  - Show relative time of last memory update (e.g., "2m ago")
  - Click/hover for quick memory summary
  - Subscribes to memory events for live updates

#### M7-T4: Slash Commands for Memory
- **File:** `crates/ragent-tui/src/` (extend slash command system)
- **Description:** Add memory-related slash commands.
- **Acceptance Criteria:**
  - `/memory` — open memory browser panel
  - `/memory read <label>` — quick read a block
  - `/memory write <label> <content>` — quick write to a block
  - `/memory search <query>` — search structured memories
  - `/journal` — open journal viewer panel
  - `/journal search <query>` — search journal entries
  - `/journal add <title>` — add journal entry
  - Autocomplete for labels and commands

---

## Milestone 8: HTTP Server Memory API

**Goal:** Expose memory operations via the ragent HTTP server for external integrations and programmatic access.

**Duration:** 1 week  
**Priority:** P3 (enables external tooling)

### Tasks

#### M8-T1: Memory REST Endpoints
- **File:** `crates/ragent-server/src/routes/memory.rs` (new)
- **Description:** REST API for memory block and structured memory operations.
- **Acceptance Criteria:**
  - `GET /memory/blocks` — list all blocks
  - `GET /memory/blocks/:label` — read a block
  - `PUT /memory/blocks/:label` — create/update a block
  - `DELETE /memory/blocks/:label` — delete a block
  - `GET /memory/search?q=<query>` — search structured memories
  - `POST /memory/store` — store a structured memory
  - `DELETE /memory/:id` — forget a memory
  - Auth required (Bearer token from `RAGENT_TOKEN`)
  - JSON request/response
  - Integration tests with test server

#### M8-T2: Journal REST Endpoints
- **File:** `crates/ragent-server/src/routes/journal.rs` (new)
- **Description:** REST API for journal operations.
- **Acceptance Criteria:**
  - `GET /journal/entries` — list entries (with pagination, tag filter)
  - `GET /journal/entries/:id` — read specific entry
  - `POST /journal/entries` — create entry
  - `GET /journal/search?q=<query>` — FTS5 search
  - Auth required
  - Integration tests

#### M8-T3: Memory SSE Events
- **File:** `crates/ragent-server/src/routes/sse.rs` (extend)
- **Description:** Stream memory and journal events via SSE.
- **Acceptance Criteria:**
  - `MemoryStored`, `MemoryUpdated`, `JournalEntryCreated` events sent over existing SSE channel
  - Clients can subscribe to memory events specifically
  - Backward compatible with existing SSE stream

---

## Milestone 9: Advanced Features

**Goal:** Cross-project memory sharing, import/export, and memory visualisation.

**Duration:** 2–3 weeks  
**Priority:** P4 (backlog — future enhancement)

### Tasks

#### M9-T1: Cross-Project Memory References
- **Description:** Allow one project to reference global memories and share patterns across projects.
- **Acceptance Criteria:**
  - Global memory blocks are accessible from any project
  - Cross-project memory search (search global + current project)
  - Project-specific overrides for global patterns
  - Config: `memory.cross_project: bool`

#### M9-T2: Import/Export
- **Description:** Export memories and journal to portable JSON; import from other ragent instances or Cline/Claude Code formats.
- **Acceptance Criteria:**
  - `ragent memory export > memories.json` — dump all memories
  - `ragent memory import < memories.json` — load from file
  - Import from Cline Memory Bank format (best-effort mapping)
  - Import from Claude Code auto-memory format
  - Dry-run mode for import (preview without writing)

#### M9-T3: Memory Visualisation
- **Description:** Optional web UI or TUI visualisation of memory relationships.
- **Acceptance Criteria:**
  - Graph view of memory categories and connections
  - Timeline view of journal entries
  - Tag cloud visualisation
  - Access pattern heatmap

#### M9-T4: Knowledge Graph Memory
- **Description:** Extract entities and relationships from memories to build a knowledge graph.
- **Acceptance Criteria:**
  - Entity extraction from memory content (projects, tools, patterns, people)
  - Relationship tracking (uses, prefers, depends_on, avoids)
  - Graph-based retrieval alongside vector search
  - Visual graph browser in TUI or web UI

---

## Dependency Graph

```
Milestone 1 (Memory Blocks)
    │
    ├──► Milestone 2 (Journal)
    │       │
    │       └──► Milestone 4 (Semantic Search)
    │               │
    │               └──► Milestone 6 (Compaction)
    │
    ├──► Milestone 3 (Structured Store)
    │       │
    │       ├──► Milestone 5 (Auto-Extract)
    │       │       │
    │       │       └──► Milestone 6 (Compaction)
    │       │
    │       └──► Milestone 4 (Semantic Search)
    │
    ├──► Milestone 7 (TUI)
    │
    └──► Milestone 8 (HTTP API)

Milestone 9 (Advanced) — independent, after M6
```

**Critical Path:** M1 → M2 → M4 → M6 (or M1 → M3 → M5 → M6)

---

## Risk Register

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Embedding model size increases binary size significantly | Medium | High | Feature-gate embeddings; default off; lazy-load model |
| SQLite schema changes break existing databases | Low | High | Use migration system; version the schema; test upgrades |
| Memory blocks consume too much context window | Medium | Medium | Configurable max memories per prompt; compaction at 90% |
| Auto-extraction produces low-quality memories | Medium | Medium | Confidence scoring; user confirmation gate; decay over time |
| Backward compatibility breaks existing MEMORY.md workflows | Low | High | M1 explicitly preserves existing behaviour; extensive integration tests |
| TUI panels increase rendering complexity | Low | Low | Incremental rendering; async data loading |

---

## Testing Strategy

### Unit Tests
- Every new module (`memory/`, `journal/`, `extract/`, `compact/`) has its own test file
- Tests in `crates/ragent-core/tests/memory/` (following AGENTS.md convention)
- In-memory SQLite for storage tests
- Mock `EmbeddingProvider` for search tests

### Integration Tests
- End-to-end: tool call → storage → retrieval → prompt injection
- Cross-crate: `ragent-core` → `ragent-tui` → `ragent-server`
- Migration: upgrade from pre-memory schema to post-memory schema

### Performance Benchmarks
- `cargo bench -p ragent-core` — add memory benchmarks
- Measure: block load time, FTS5 search latency, embedding generation time
- Target: block load < 10ms, FTS5 search < 50ms, embedding < 100ms (after model load)

---

## Configuration Summary

Final `ragent.json` schema for the complete memory system:

```json
{
  "memory": {
    "enabled": true,
    "tier": "structured",
    "storage": {
      "type": "hybrid",
      "file_path": ".ragent/memory",
      "database_path": ".ragent/memory.db"
    },
    "embeddings": {
      "enabled": false,
      "model": "local",
      "dimensions": 384
    },
    "auto_extract": {
      "enabled": true,
      "require_confirmation": true,
      "categories": ["fact", "pattern", "error"],
      "min_confidence": 0.7
    },
    "compaction": {
      "enabled": true,
      "threshold_percent": 90
    },
    "decay": {
      "factor": 0.95,
      "min_confidence": 0.1
    },
    "eviction": {
      "auto": false,
      "stale_days": 30
    },
    "retrieval": {
      "max_memories_per_prompt": 5,
      "recency_weight": 0.3,
      "relevance_weight": 0.7
    },
    "default_blocks": {
      "persona": true,
      "human": true,
      "project": true
    },
    "cross_project": false
  }
}
```

---

## Storage Locations Summary

| Scope | Path | Contents |
|-------|------|----------|
| Global blocks | `~/.ragent/memory/*.md` | persona.md, human.md, etc. |
| Project blocks | `.ragent/memory/*.md` | project.md, patterns.md, etc. |
| Structured store | `.ragent/memory.db` | SQLite: memories, journal, tags, FTS |
| Embedding cache | `~/.ragent/cache/memory/` | Model files, vector indices |
| Team memory | `<team_dir>/memory/*.md` | Existing team memory (unchanged) |

---

*This plan is a living document. As implementation proceeds, update task status and adjust timelines based on actual velocity. Each milestone should be validated with integration tests before proceeding to the next.*