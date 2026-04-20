---
title: "Semantic Memory and Code Intelligence"
type: concept
generated: "2026-04-19T20:07:04.611548309+00:00"
---

# Semantic Memory and Code Intelligence

### From: mod

The semantic memory and code intelligence capabilities in ragent-core demonstrate how modern agent systems integrate multiple information retrieval paradigms to support software engineering tasks. The memory subsystem combines structured key-value storage, semantic search via embeddings with FTS5 full-text search, and journal-based temporal logging, providing agents with both precise recall and fuzzy similarity matching. This multi-modal approach recognizes that different tasks require different retrieval strategies: exact lookup for known facts, semantic search for conceptually related content, and chronological access for recent context.

Code intelligence leverages the Language Server Protocol (LSP) to provide IDE-grade capabilities within agent workflows. The six LSP tools (symbols, hover, definition, references, diagnostics, symbols) allow agents to navigate and understand codebases without parsing source text themselves. This integration is significant because it delegates syntax and semantics understanding to specialized language servers while the agent focuses on higher-level reasoning. The optional `lsp_manager` and `code_index` in ToolContext reflect an on-demand capability model where these resources are available when configured.

The codebase indexing tools (`codeindex_search`, `codeindex_symbols`, `codeindex_references`, `codeindex_dependencies`, `codeindex_status`, `codeindex_reindex`) provide a dedicated search infrastructure optimized for repository-scale operations, complementing LSP's file-focused analysis. Together these systems enable agents to perform sophisticated software engineering: finding symbol definitions across dependencies, identifying all call sites for refactoring, diagnosing type errors, and searching with both text and semantic similarity. The integration of memory and code intelligence reflects an architectural commitment to context accumulation—agents should learn from and reference prior interactions and codebase analysis.

## Diagram

```mermaid
erDiagram
    MEMORY_TOOLS["Memory Tools"] ||--|| STRUCTURED["structured_memory\n(store/recall/forget)"]
    MEMORY_TOOLS ||--|| SEMANTIC["memory_search\n(embeddings + FTS5)"]
    MEMORY_TOOLS ||--|| JOURNAL["journal\n(write/search/read)"]
    MEMORY_TOOLS ||--|| MIGRATE["memory_migrate/replace/write/read"]
    
    CODE_TOOLS["Code Intelligence"] ||--|| LSP["LSP Tools\n(symbols, hover, definition,\nreferences, diagnostics)"]
    CODE_TOOLS ||--|| INDEX["CodeIndex Tools\n(search, symbols, references,\ndependencies, status, reindex)"]
    
    LSP -->|uses| LSP_MANAGER["LspManager"]
    INDEX -->|uses| CODE_INDEX["CodeIndex"]
    
    SEMANTIC -->|stores in| STORAGE["Storage\n(SQLite + vectors)"]
    STRUCTURED -->|stores in| STORAGE
    JOURNAL -->|stores in| STORAGE
    
    AGENT["Agent"] -->|invokes| MEMORY_TOOLS
    AGENT -->|invokes| CODE_TOOLS
```

## External Resources

- [Language Server Protocol specification](https://microsoft.github.io/language-server-protocol/) - Language Server Protocol specification
- [SQLite FTS5 full-text search module](https://www.sqlite.org/fts5.html) - SQLite FTS5 full-text search module
- [Sentence Transformers for text embeddings](https://huggingface.co/sentence-transformers) - Sentence Transformers for text embeddings

## Sources

- [mod](../sources/mod.md)
