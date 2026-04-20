---
title: "MemorySearchTool"
entity_type: "technology"
type: entity
generated: "2026-04-19T17:07:37.128380052+00:00"
---

# MemorySearchTool

**Type:** technology

### From: memory_search

MemorySearchTool is a Rust struct implementing the Tool trait that provides unified semantic and keyword-based search across an AI agent's memory systems. The tool encapsulates complex retrieval logic behind a clean interface accepting natural language queries, with automatic backend selection between embedding-based similarity search and traditional full-text search. Its implementation demonstrates sophisticated software engineering practices including strategy pattern for search modes, lazy initialization for embeddings, and graceful degradation when dependencies are unavailable. The tool operates on two distinct storage backends: structured memories in SQLite with rich metadata (categories, confidence scores, tags) and file-based memory blocks with cross-project scoping capabilities.

The tool's architecture enables incremental adoption of semantic search capabilities. Organizations can deploy systems using only FTS5 search, then enable embeddings later without changing calling code. The lazy embedding feature ensures backward compatibility—when semantic search is first enabled, existing memories are automatically embedded on first query rather than requiring batch migration. This design pattern appears in production systems like OpenAI's retrieval implementations and enterprise knowledge bases where data volumes make eager migration prohibitive.

MemorySearchTool integrates deeply with the surrounding agent framework. It participates in the permission system through its "file:read" category, emits structured events for observability, and conforms to the async Tool trait for consistent execution. The cross-project block search with shadowing semantics enables sophisticated multi-tenant scenarios where base knowledge can be specialized per-project without duplication. These features collectively position the tool as a production-grade component for AI systems requiring reliable, observable, and scalable memory retrieval.

## Diagram

```mermaid
flowchart TD
    subgraph Input["Input Processing"]
        query["Natural Language Query"]
        params["Parameters: scope, limit, min_similarity"]
    end
    
    subgraph StrategySelection["Search Strategy Selection"]
        check_embed["EmbeddingProvider<br/>available?"]
        scope_check{"scope parameter"}
    end
    
    subgraph SemanticSearch["Semantic Search Path"]
        embed_query["Embed query text"]
        lazy_embed["Lazy-embed unindexed memories"]
        cosine["Cosine similarity search"]
        filter["Filter by min_similarity"]
    end
    
    subgraph FallbackSearch["FTS5 Fallback Path"]
        tokenize["Tokenize query"]
        fts_search["FTS5 keyword search"]
    end
    
    subgraph BlockSearch["Memory Block Search"]
        cross_project["Cross-project config resolution"]
        file_scan["Scan FileBlockStorage"]
        shadow_resolve["Resolve shadowing rules"]
    end
    
    subgraph Output["Result Assembly"]
        rank["Rank by similarity/score"]
        format["Format with metadata"]
        emit["Emit MemorySearched event"]
    end
    
    query --> check_embed
    params --> scope_check
    check_embed -->|yes| embed_query
    check_embed -->|no| tokenize
    embed_query --> lazy_embed
    lazy_embed --> cosine
    cosine --> filter
    tokenize --> fts_search
    filter --> rank
    fts_search --> rank
    scope_check -->|memories/all| check_embed
    scope_check -->|blocks/all| cross_project
    cross_project --> file_scan
    file_scan --> shadow_resolve
    shadow_resolve --> rank
    rank --> format
    format --> emit
```

## External Resources

- [SQLite FTS5 documentation for full-text search capabilities](https://www.sqlite.org/fts5.html) - SQLite FTS5 documentation for full-text search capabilities
- [OpenAI embeddings guide covering semantic search with vector similarity](https://platform.openai.com/docs/guides/embeddings) - OpenAI embeddings guide covering semantic search with vector similarity
- [Cosine similarity mathematical foundation for semantic search scoring](https://en.wikipedia.org/wiki/Cosine_similarity) - Cosine similarity mathematical foundation for semantic search scoring
- [Vector similarity search concepts and applications in AI systems](https://www.pinecone.io/learn/what-is-similarity-search/) - Vector similarity search concepts and applications in AI systems

## Sources

- [memory_search](../sources/memory-search.md)
