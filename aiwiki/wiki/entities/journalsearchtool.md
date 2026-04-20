---
title: "JournalSearchTool"
entity_type: "technology"
type: entity
generated: "2026-04-19T18:06:26.487418073+00:00"
---

# JournalSearchTool

**Type:** technology

### From: journal

The JournalSearchTool implements hybrid search capabilities across the agent's journal, combining SQLite's FTS5 full-text search with optional semantic similarity search when embeddings are available. This tool represents a sophisticated information retrieval interface that understands both exact keyword matching and conceptual similarity, enabling agents to surface relevant past experiences through multiple query strategies. The search accepts a required query string, optional tag filters requiring all specified tags to be present, a configurable result limit, and a semantic search toggle.

The implementation demonstrates careful attention to search quality and result presentation. The FTS5 integration leverages SQLite's built-in ranking algorithms to surface the most textually relevant entries, while tag filtering applies an intersection constraint ensuring only entries matching all categorical requirements are returned. When semantic search is enabled and an embedding provider is available, the system can theoretically extend beyond lexical matching to find entries with conceptually similar meaning, though the current implementation notes that full semantic enhancement with lazy-embedding is deferred to future iterations.

Result processing includes snippet generation for preview purposes, with content truncated to 200 characters with ellipsis notation for longer entries. Complete tag retrieval from storage enriches each result with full categorical information. The tool emits a JournalSearched event capturing query and result count for analytics, and returns formatted output distinguishing between FTS-only and hybrid search modes. The output formatting uses Markdown for readability, with enumerated results showing titles, timestamps, content snippets, tags, and entry IDs in a structured presentation optimized for agent consumption.

## Diagram

```mermaid
flowchart TD
    subgraph Input["Search Parameters"]
        Q["query string"]
        T["optional tags filter"]
        L["limit default 10"]
        S["semantic flag"]
    end
    
    subgraph Search["Search Execution"]
        C1["Check semantic availability"]
        C2["Execute FTS5 search"]
        C3["Filter by tags if specified"]
    end
    
    subgraph Results["Result Processing"]
        R1["Retrieve tags for each entry"]
        R2["Generate 200-char snippets"]
        R3["Build JournalEntrySummary objects"]
    end
    
    subgraph Output["Output Generation"]
        O1["Emit JournalSearched event"]
        O2["Format Markdown output"]
        O3["Return with metadata"]
    end
    
    Q --> C1
    T --> C3
    L --> C2
    S --> C1
    C1 --> C2 --> C3 --> R1 --> R2 --> R3 --> O1 --> O2 --> O3
```

## External Resources

- [SQLite FTS5 full-text search engine documentation](https://www.sqlite.org/fts5.html) - SQLite FTS5 full-text search engine documentation
- [Conceptual overview of semantic search technology](https://en.wikipedia.org/wiki/Semantic_search) - Conceptual overview of semantic search technology
- [OpenAI embeddings guide for semantic similarity applications](https://platform.openai.com/docs/guides/embeddings) - OpenAI embeddings guide for semantic similarity applications

## Sources

- [journal](../sources/journal.md)
