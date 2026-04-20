# Persistent Memory in AI Coding Agents: Research & Design Document

This document surveys the landscape of persistent memory systems in AI coding agents and proposes design options for enhancing ragent's memory capabilities.

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Current State: Ragent Memory System](#current-state-ragent-memory-system)
3. [Competitor Analysis](#competitor-analysis)
4. [Memory Architecture Patterns](#memory-architecture-patterns)
5. [Proposed Design for Ragent](#proposed-design-for-ragent)
6. [Implementation Roadmap](#implementation-roadmap)

---

## Executive Summary

Persistent memory is becoming a critical differentiator in AI coding agents. Current implementations range from simple file-based markdown persistence (ragent, Cline) to sophisticated embedding-based retrieval systems (Mem0, Claude Code Auto-Memory). The key insight is that **memory is not just storage—it is context management** that determines agent effectiveness across sessions.

This document analyzes existing patterns and proposes a three-tier memory architecture for ragent that preserves its current simplicity while enabling future expansion toward semantic memory capabilities.

---

## Current State: Ragent Memory System

### Existing Tools

Ragent currently provides four memory-related tools:

| Tool | Scope | Purpose |
|------|-------|---------|
| `memory_write` | User/Project | Append notes to MEMORY.md files |
| `memory_read` | User/Project | Read back from memory files |
| `team_memory_write` | Team | Teammate-specific persistent storage |
| `team_memory_read` | Team | Teammate memory retrieval |

### Memory Scopes

The `MemoryScope` enum defines three levels of persistence:

```rust
pub enum MemoryScope {
    None,    // No persistent memory (default)
    User,    // Global: ~/.ragent/agent-memory/<agent-name>/
    Project, // Local: <project>/.ragent/agent-memory/<agent-name>/
}
```

### Auto-Loading

Memory files are automatically loaded into the system prompt at session start:

```rust
// From agent/mod.rs
let project_mem = working_dir.join(".ragent").join("memory").join("MEMORY.md");
let user_mem = dirs::home_dir().map(|h| h.join(".ragent").join("memory").join("MEMORY.md"));

if let Ok(content) = std::fs::read_to_string(&project_mem) {
    prompt.push_str("## Project Memory\n");
    prompt.push_str(&content);
}
```

### Current Strengths

- **Simplicity**: Markdown files are human-readable and editable
- **Transparency**: Users can see exactly what the agent remembers
- **Version Control Friendly**: Project memory can be committed with code
- **Cross-Session**: Memory persists between agent invocations

### Current Limitations

- **No Semantic Search**: Full-text only, no embedding-based retrieval
- **Manual Management**: Agent must explicitly write/read; no automatic memory formation
- **Flat Structure**: Single file per scope; no categorization or tagging
- **No Summarization**: No automatic condensation of long memory files
- **Context Window Pressure**: Large memory files consume valuable prompt space

---

## Competitor Analysis

### 1. Cline Memory Bank

**Architecture**: Structured markdown documentation system

**Key Files**:
- `productBrief.md` - Project overview and goals
- `techContext.md` - Technical stack and patterns
- `activeContext.md` - Current work status
- `progress.md` - Completed work and remaining tasks
- `systemPatterns.md` - Recurring patterns and conventions

**How It Works**:
1. Agent is instructed to maintain these files via custom instructions
2. Files are read at session start to restore context
3. Agent updates files as work progresses
4. "Memory Bank" is triggered on session start if files exist

**Advantages**:
- Structured, predictable format
- Self-documenting codebase
- Works with any LLM

**Disadvantages**:
- Requires discipline to maintain
- No automatic memory extraction
- Can become stale if not updated

### 2. Claude Code Auto-Memory

**Architecture**: Automatic memory formation with user confirmation

**Key Features** (from research):
- Automatic detection of important information
- SQLite + FTS5 for full-text search
- User approval before storing memories
- Session-centric schema
- Memory decay/retention policies

**How It Works**:
1. Agent analyzes conversations for key learnings
2. Proposes memories to user for confirmation
3. Stores approved memories in SQLite
4. Retrieves relevant memories based on current context

**Advantages**:
- Automatic extraction reduces user burden
- Structured storage enables search
- User maintains control over what is remembered

**Disadvantages**:
- Requires additional infrastructure
- Potential privacy concerns
- Approval workflow adds friction

### 3. Mem0 Universal Memory Layer

**Architecture**: Production-grade memory system with multiple backends

**Key Components**:
- **Embedding-based retrieval**: Converts memories to vectors
- **Multi-level storage**: SQLite, PostgreSQL, or cloud backends
- **Memory deduplication**: Prevents redundant storage
- **Contextual relevance scoring**: Ranks memories by relevance
- **Multi-agent support**: Shared memory across agents

**Memory Types**:
1. **User Memory**: Preferences, facts about the user
2. **Session Memory**: Current conversation context
3. **Agent Memory**: Learned patterns and insights

**API Example**:
```python
from mem0 import Memory

m = Memory()
m.add("User prefers Python for data processing", user_id="alice")
memories = m.search("What language does the user prefer?", user_id="alice")
```

**Advantages**:
- Scalable architecture
- Semantic search capabilities
- Production-tested
- Multiple backend options

**Disadvantages**:
- Complex deployment
- Requires embedding model
- Potential cost for cloud backends

### 4. LangChain Memory Systems

**Architecture**: Modular memory abstractions

**Memory Types**:
1. **ConversationBufferMemory**: Stores raw conversation history
2. **ConversationSummaryMemory**: Summarizes conversations over time
3. **VectorStoreRetrieverMemory**: Embedding-based retrieval
4. **ConversationEntityMemory**: Extracts and remembers entities
5. **ConversationKGMemory**: Knowledge graph of facts

**Key Insight**: Memory should match the use case:
- **Short-term**: Buffer for immediate context
- **Long-term**: Summaries or vector retrieval for facts
- **Episodic**: Specific past interactions
- **Semantic**: General knowledge about user/domain

---

## Memory Architecture Patterns

### Pattern 1: Working Memory + Long-Term Store

```
┌─────────────────────────────────────────────────────────┐
│  Session Context Window (Working Memory)                │
│  ├─ Active conversation history                         │
│  ├─ Current file context                                │
│  └─ Retrieved relevant memories                         │
└─────────────────────────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────┐
│  Long-Term Memory Store                                 │
│  ├─ Episodic: Past conversations (summarized)           │
│  ├─ Semantic: Facts, preferences, patterns              │
│  └─ Procedural: How-to knowledge, workflows             │
└──────────────────────────���──────────────────────────────┘
```

### Pattern 2: Hierarchical Memory

```
Level 3: Archival (Cold Storage)
  └─ Compressed summaries, completed projects

Level 2: Reference (Warm Storage)
  └─ Project memory, active patterns, user preferences

Level 1: Working (Hot Storage)
  └─ Current session, immediate context, scratchpad
```

### Pattern 3: Vector-Based Semantic Memory

```
User Query
    │
    ▼
[Embedding Model] ──► Query Vector
    │
    ▼
[Vector Database] ──► Similarity Search
    │
    ▼
Top-K Relevant Memories ──► Injected into Prompt
```

### Pattern 4: Knowledge Graph Memory

```
┌─────────────────────────────────────────┐
│  User ──prefers──► Python               │
│   │                                     │
│   ├──works_on──► Project-A              │
│   │                │                    │
│   │                ├──uses──► FastAPI   │
│   │                └──uses──► Postgres  │
│   │                                     │
│   └──knows──► Rust (learning)           │
└─────────────────────────────────────────┘
```

---

## Proposed Design for Ragent

### Design Goals

1. **Backward Compatible**: Existing memory files continue to work
2. **Progressive Enhancement**: Start simple, add sophistication incrementally
3. **User Controlled**: Transparency in what is remembered and why
4. **Performance**: Minimal impact on startup time and runtime
5. **Storage Efficient**: Don't duplicate what git/codebase already stores

### Three-Tier Architecture

#### Tier 1: Core Memory (Current) - **IMPLEMENTED**

**Purpose**: Explicit user/agent notes that must persist

**Storage**: Markdown files in `.ragent/memory/`

**Files**:
```
.ragent/memory/
├── MEMORY.md              # General notes
├── PROJECT_ANALYSIS.md    # Project understanding
├── PATTERNS.md           # Code patterns and conventions
├── DECISIONS.md          # Architecture decisions (ADRs)
└── scratchpad.md         # Temporary notes (not loaded)
```

**Enhancement**: Add structured frontmatter for metadata:
```markdown
---
created: 2025-01-15T10:30:00Z
updated: 2025-01-20T14:22:00Z
category: patterns
tags: [rust, error-handling]
---

# Error Handling Patterns
...
```

#### Tier 2: Structured Memory (Proposed Phase 1)

**Purpose**: Automatic extraction of key facts and patterns

**Storage**: SQLite database in `.ragent/memory.db`

**Schema**:
```sql
-- Memories table
CREATE TABLE memories (
    id INTEGER PRIMARY KEY,
    content TEXT NOT NULL,
    category TEXT NOT NULL,  -- 'fact', 'pattern', 'preference', 'insight'
    source TEXT,             -- Which file/tool created this
    confidence REAL,         -- 0.0-1.0, agent's confidence in this memory
    created_at DATETIME,
    updated_at DATETIME,
    access_count INTEGER DEFAULT 0,
    last_accessed DATETIME
);

-- Full-text search
CREATE VIRTUAL TABLE memories_fts USING fts5(content, content='memories');

-- Tags for categorization
CREATE TABLE memory_tags (
    memory_id INTEGER,
    tag TEXT,
    PRIMARY KEY (memory_id, tag)
);
```

**New Tools**:

| Tool | Purpose |
|------|---------|
| `memory_store` | Store a structured memory with category and tags |
| `memory_recall` | Query memories by text search or category |
| `memory_forget` | Remove outdated or incorrect memories |
| `memory_summarize` | Auto-summarize long conversation into key points |

**Automatic Extraction**:
- Hook into conversation end: extract key learnings
- After file edits: note patterns used
- After error resolution: remember solutions

#### Tier 3: Semantic Memory (Proposed Phase 2)

**Purpose**: Embedding-based retrieval for large memory bases

**Storage**: 
- SQLite for metadata
- Vector database (e.g., `sqlite-vec` or `pgvector` via optional feature)

**Architecture**:
```
┌────────────────────────────────────────────┐
│  Ragent Agent                              │
│  ├─ System Prompt (with relevant memories) │
│  └─ Tools                                  │
└────────────────────────────────────────────┘
              │
              ▼
┌────────────────────────────────────────────┐
│  Memory Manager                            │
│  ├─ Query Analyzer (what do we need?)      │
│  ├─ Retrieval Strategy (sql vs vector)     │
│  └─ Relevance Ranker                       │
└────────────────────────────────────────────┘
              │
     ┌────────┴────────┐
     ▼                 ▼
┌─────────┐     ┌─────────────┐
│ SQLite  │     │ Vector DB   │
│ (exact) │     │ (semantic)  │
└─────────┘     └─────────────┘
```

**Embedding Strategy**:
- Use lightweight local model (e.g., `all-MiniLM-L6-v2`)
- Optional: Cloud embeddings for higher quality
- Hybrid: SQL for structured queries, vectors for semantic

### Memory Categories

| Category | Description | Example |
|----------|-------------|---------|
| `fact` | Objective truths about the project | "Uses Axum for HTTP server" |
| `preference` | User's stated preferences | "Prefers explicit error types" |
| `pattern` | Recurring code patterns | "Repository pattern with traits" |
| `insight` | Agent's learned understanding | "Auth flow is the critical path" |
| `error` | Past errors and solutions | "Mutex deadlock in worker.rs:23" |
| `workflow` | Common task sequences | "Adding a tool: register in mod.rs" |

### Memory Lifecycle

```
┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐
│  Form    │───►│  Store   │───►│ Retrieve │───►│  Update  │
│  (create)│    │  (index) │    │  (query) │    │  (refresh)
└──────────┘    └──────────┘    └─────���────┘    └──────────┘
      │                               │               │
      │                               │               │
      ▼                               ▼               ▼
• Explicit (user)              • By context     • Confidence decay
• Implicit (agent)             • By query       • User feedback
• Automatic (extracted)        • By tag         • Merging duplicates
```

### Integration Points

1. **Prompt Assembly** (agent/mod.rs):
   ```rust
   // Load relevant memories based on current context
   let memories = memory_manager.retrieve_relevant(
       &current_file,
       &conversation_summary,
       limit: 5
   );
   prompt.push_str(&format_relevant_memories(memories));
   ```

2. **Tool Execution** (tool handlers):
   ```rust
   // After tool execution, extract learnings
   if let Some(learning) = extract_learning(&tool_result) {
       memory_manager.store(learning).await?;
   }
   ```

3. **Session End** (session management):
   ```rust
   // Summarize session and store key points
   let summary = summarize_conversation(&messages).await?;
   memory_manager.store_episode(summary).await?;
   ```

### Configuration

```json
{
  "memory": {
    "enabled": true,
    "tier": "structured",  // "core" | "structured" | "semantic"
    "semantic": {
      "enabled": false,
      "embedding_model": "local",  // or "openai", etc.
      "vector_store": "sqlite-vec" // or "pgvector"
    },
    "auto_extract": {
      "enabled": true,
      "categories": ["fact", "pattern", "error"],
      "min_confidence": 0.7
    },
    "retrieval": {
      "max_memories_per_prompt": 5,
      "recency_weight": 0.3,
      "relevance_weight": 0.7
    }
  }
}
```

---

## Implementation Roadmap

### Phase 1: Structured Memory Foundation (2-3 weeks)

**Goals**:
- Add SQLite storage for memories
- Implement core memory tools
- Add automatic extraction hooks
- Maintain backward compatibility with existing memory files

**Tasks**:
1. Create `crates/ragent-core/src/memory/` module
2. Define `MemoryStore` trait with SQLite implementation
3. Implement `memory_store`, `memory_recall`, `memory_forget` tools
4. Add memory extraction hooks to session lifecycle
5. Update prompt assembly to include relevant memories
6. Write migration: parse existing MEMORY.md into structured storage

**Files to Create**:
```
crates/ragent-core/src/memory/
├── mod.rs           # Public API
├── store.rs         # Storage trait + SQLite impl
├── types.rs         # Memory, Category, etc.
├���─ extract.rs       # Automatic extraction logic
├── rank.rs          # Relevance ranking
└── tools.rs         # Memory tool implementations
```

### Phase 2: Enhanced Retrieval (1-2 weeks)

**Goals**:
- Add full-text search with SQLite FTS5
- Implement context-aware retrieval
- Add memory confidence scoring

**Tasks**:
1. Add FTS5 virtual table for memories
2. Implement hybrid query (exact + fuzzy matching)
3. Add context-based retrieval (file path, conversation topic)
4. Implement access tracking and decay

### Phase 3: Semantic Memory (3-4 weeks)

**Goals**:
- Add embedding-based retrieval
- Integrate lightweight local embedding model
- Support cloud embedding providers

**Tasks**:
1. Research and select embedding approach (sqlite-vec, ort, etc.)
2. Implement `EmbeddingProvider` trait
3. Add vector storage and similarity search
4. Implement hybrid retrieval (vector + text)
5. Add memory deduplication and merging

### Phase 4: Advanced Features (2-3 weeks)

**Goals**:
- Knowledge graph memory
- Cross-project memory sharing
- Memory visualization in TUI

**Tasks**:
1. Implement knowledge graph structure
2. Add entity extraction and relationship tracking
3. Cross-project memory references
4. TUI panel for browsing/searching memories

---

## Open Questions

1. **Privacy**: Should memories be stored encrypted? Can users audit what is remembered?

2. **Multi-User**: How should memories work in team/collaborative settings?

3. **Versioning**: Should memories be versioned alongside code? Can we "checkout" a project's state of knowledge?

4. **Transfer**: Can users export/import memories between projects or agents?

5. **Eviction**: What is the policy for forgetting? Should old, low-confidence memories be purged?

---

## References

1. [Mem0 Documentation](https://docs.mem0.ai/) - Universal memory layer
2. [Cline Memory Bank](https://docs.cline.bot/features/memory-bank) - Structured documentation approach
3. [LangChain Memory](https://python.langchain.com/docs/modules/memory/) - Modular memory abstractions
4. [Claude Code Auto-Memory](https://www.anthropic.com/news/claude-code-auto-memory) - Automatic memory formation
5. [Building Memory Architectures for AI Agents](https://hackernoon.com/llms-vector-databases-building-memory-architectures-for-ai-agents) - Vector database patterns
6. [AI Agent Memory Systems](https://mem0.ai/blog/multi-agent-memory-systems) - Multi-agent memory design

---

## Appendix: Memory Tool Specifications

### `memory_store`

Store a structured memory.

```json
{
  "content": "User prefers Result<T, E> over panics",
  "category": "preference",
  "tags": ["rust", "error-handling", "style"],
  "confidence": 0.9,
  "source": "conversation"
}
```

### `memory_recall`

Query memories.

```json
{
  "query": "error handling preferences",
  "categories": ["preference", "pattern"],
  "limit": 5,
  "min_confidence": 0.7
}
```

### `memory_forget`

Remove memories.

```json
{
  "filter": {
    "older_than_days": 30,
    "max_confidence": 0.3
  }
}
```

---

*Document Version: 1.0*
*Last Updated: 2025-01*
