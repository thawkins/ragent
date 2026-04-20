---
title: "Lazy Embedding Generation"
type: concept
generated: "2026-04-19T17:07:37.131182412+00:00"
---

# Lazy Embedding Generation

### From: memory_search

Lazy embedding generation is an optimization strategy that defers computational work until absolutely necessary, specifically applied in MemorySearchTool to avoid expensive batch processing when enabling semantic search on existing memory stores. Rather than requiring administrators to run migration jobs that embed all historical memories before search is available, the system computes embeddings on-demand during the first search that encounters each unembedded memory. This approach trades slightly higher latency for initial queries against old data for dramatically reduced upfront costs and eliminated downtime.

The implementation in search_memories_semantic demonstrates careful attention to atomicity and failure modes. The function first queries existing embeddings to build a HashSet of already-processed memory IDs, then iterates through memories missing from this set. Each memory is embedded individually with error isolation—failure to embed one memory doesn't block others. Successfully generated embeddings are immediately persisted via store_memory_embedding, ensuring progress survives process restarts. The batch size limit of 10,000 memories in list_memories provides backpressure against unbounded memory consumption during initial lazy embedding sweeps.

This pattern appears throughout data-intensive systems where backfill operations are prohibitive. Search engines like Elasticsearch similarly build indices incrementally; machine learning feature stores compute embeddings on first feature request; content delivery networks warm caches on first miss. The economic rationale is compelling: if only 10% of memories are ever searched, lazy embedding avoids 90% of computation versus eager batch processing. MemorySearchTool's event emission enables monitoring of this tradeoff—operators can observe embedding generation rates and query latency distributions to decide if background pre-embedding would improve user experience. The design also accommodates embedding model upgrades: new models can be adopted incrementally as old embeddings are naturally replaced through query-driven recomputation.

## Diagram

```mermaid
flowchart TD
    subgraph InitialState["Initial State"]
        mem_a["Memory A: 'Deploy to staging'<br/>embedding: null"]
        mem_b["Memory B: 'API rate limits'<br/>embedding: null"]
        mem_c["Memory C: 'Database indexing'<br/>embedding: null"]
    end
    
    subgraph FirstSearch["First Search: 'deployment workflow'"]
        query_embed["Embed query"]
        check_existing["List existing embeddings<br/>→ empty set"]
        lazy_loop["For each memory:<br/>embed if missing"]
        embed_a["Embed Memory A<br/>→ [0.23, -0.15, ...]"]
        embed_b["Skip/Embed Memory B"]
        embed_c["Skip/Embed Memory C"]
        store"Store new embeddings"
        search_similarity["Search by similarity"]
    end
    
    subgraph SecondSearch["Second Search: 'rate limiting'"]
        query_embed2["Embed query"]
        check_existing2["List existing embeddings<br/>→ {A}"]
        skip_a["Skip Memory A<br/>(already embedded)"]
        embed_b2["Embed Memory B<br/>→ [0.89, 0.12, ...]"]
        search_similarity2["Search by similarity"]
    end
    
    subgraph FinalState["Final State"]
        final_a["Memory A: embedded ✓"]
        final_b["Memory B: embedded ✓"]
        final_c["Memory C: still null<br/>(never searched)"]
    end
    
    mem_a & mem_b & mem_c --> query_embed
    query_embed --> check_existing
    check_existing --> lazy_loop
    lazy_loop --> embed_a
    lazy_loop --> embed_b
    lazy_loop --> embed_c
    embed_a & embed_b & embed_c --> store
    store --> search_similarity
    
    search_similarity -.->|subsequent query| query_embed2
    query_embed2 --> check_existing2
    check_existing2 --> skip_a
    check_existing2 --> embed_b2
    skip_a & embed_b2 --> search_similarity2
    
    search_similarity2 -.->|time passes| final_a & final_b & final_c
```

## External Resources

- [Lazy evaluation in computer science](https://en.wikipedia.org/wiki/Lazy_evaluation) - Lazy evaluation in computer science
- [Elasticsearch indexing buffer and refresh behavior](https://www.elastic.co/guide/en/elasticsearch/reference/current/indexing-buffer.html) - Elasticsearch indexing buffer and refresh behavior
- [Feature store patterns including on-demand feature computation](https://mlops.community/learn/feature-stores/) - Feature store patterns including on-demand feature computation

## Sources

- [memory_search](../sources/memory-search.md)
