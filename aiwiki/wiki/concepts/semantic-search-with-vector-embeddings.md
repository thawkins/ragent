---
title: "Semantic Search with Vector Embeddings"
type: concept
generated: "2026-04-19T17:07:37.129855914+00:00"
---

# Semantic Search with Vector Embeddings

### From: memory_search

Semantic search represents a fundamental shift from lexical matching to meaning-based retrieval in information systems. Unlike traditional keyword search that matches exact or stemmed word forms, semantic search encodes queries and documents as high-dimensional dense vectors (embeddings) where spatial proximity corresponds to semantic similarity. In MemorySearchTool, this manifests as cosine similarity computation between query embeddings and stored memory embeddings, enabling retrieval of conceptually related content even when vocabulary differs completely. For example, a query about "increasing system throughput" could match memories discussing "performance optimization" or "load balancing" without sharing any keywords.

The implementation details reveal production considerations often glossed over in research presentations. Lazy embedding generation ensures that enabling semantic search doesn't require expensive batch reprocessing of existing memories—new embeddings are computed on-demand during the first search that encounters unembedded content. Similarity thresholding (min_similarity parameter, default 0.3) provides tunable precision-recall control: higher thresholds reduce false positives at the cost of potentially missing relevant but distantly related content. The fallback to FTS5 when embeddings fail or are unavailable demonstrates operational pragmatism; semantic search is a progressive enhancement rather than hard dependency.

The mathematical foundation rests on the observation that neural network embeddings capture distributional semantics—words and concepts appearing in similar contexts obtain similar vector representations. This property, emergent from training on large text corpora with objectives like masked language modeling or contrastive learning, enables cross-lingual retrieval and conceptual abstraction. However, the implementation also hints at limitations: embedding quality depends on training data distribution, out-of-domain queries may map to unexpected regions of vector space, and the black-box nature of similarity scores can complicate debugging. MemorySearchTool's explicit mode tracking in events ("semantic" vs "fts") supports operational monitoring of these tradeoffs in production.

## Diagram

```mermaid
flowchart LR
    subgraph QueryPath["Query Processing"]
        raw_query["Raw text query"]
        embed_model["Embedding model<br/>(e.g., text-embedding-3-small)"]
        query_vec["Query vector<br/>dimension: 1536"]
    end
    
    subgraph IndexPath["Memory Index"]
        mem_content["Memory content"]
        lazy_embed["Lazy embedding generation"]
        stored_vecs["Stored embeddings<br/>+ row_id mapping"]
    end
    
    subgraph SimilaritySearch["Similarity Computation"]
        dot_product["Dot product /<br/>cosine similarity"]
        score["Similarity score<br/>[-1, 1] → [0, 1]"]
        threshold{"score ≥<br/>min_similarity?"}
    end
    
    subgraph Results["Result Ranking"]
        filter_pass["Pass: include in results"]
        rank["Rank by score"]
        top_k["Return top-k<br/>with metadata"]
    end
    
    raw_query --> embed_model
    embed_model --> query_vec
    mem_content --> lazy_embed
    lazy_embed --> stored_vecs
    query_vec --> dot_product
    stored_vecs --> dot_product
    dot_product --> score
    score --> threshold
    threshold -->|yes| filter_pass
    threshold -->|no| discard["Discard"]
    filter_pass --> rank
    rank --> top_k
```

## External Resources

- [Dense Passage Retrieval for Open-Domain QA (Karpukhin et al., 2021)](https://arxiv.org/abs/2104.05740) - Dense Passage Retrieval for Open-Domain QA (Karpukhin et al., 2021)
- [Sentence-BERT models for semantic similarity tasks](https://huggingface.co/sentence-transformers) - Sentence-BERT models for semantic similarity tasks
- [OpenAI embeddings API and cosine similarity explanation](https://platform.openai.com/docs/guides/embeddings/what-are-embeddings) - OpenAI embeddings API and cosine similarity explanation
- [Practical guide to text embeddings for search applications](https://www.deepset.ai/blog/the-beginners-guide-to-text-embeddings) - Practical guide to text embeddings for search applications

## Sources

- [memory_search](../sources/memory-search.md)
