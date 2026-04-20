---
title: "Graph-Based Retrieval"
type: concept
generated: "2026-04-19T21:52:48.056738191+00:00"
---

# Graph-Based Retrieval

### From: knowledge_graph

Graph-based retrieval extends the memory system's search capabilities beyond vector similarity and full-text search, enabling queries that traverse semantic relationships between entities. The knowledge graph structure supports retrieval patterns impossible with isolated document search: finding related technologies through shared usage, identifying preference clusters among tools, discovering dependency chains, and surfacing anti-patterns through avoidance relationships. This multi-modal retrieval architecture recognizes that different query intents benefit from different underlying representations—factual lookup from text search, conceptual similarity from embeddings, and relational navigation from graph traversal.

The storage schema design enables efficient graph operations through proper indexing of foreign key relationships, with entity IDs serving as graph nodes and relationship records as typed edges. The get_knowledge_graph function provides complete graph materialization for visualization and analysis tools, while the underlying storage interface supports targeted queries for specific entity neighborhoods or relationship paths. The mention_count field in Entity records enables frequency-based ranking, helping distinguish central concepts from incidental mentions.

The integration with memory provenance—via source_memory_id in relationships—creates an audit trail connecting derived graph structure back to original observations. This provenance supports explanation generation ("why is Rust connected to Tokio?") and confidence recalculation when source memories are updated or deleted. The system's architecture anticipates incremental graph updates as new memories arrive, with store_extraction providing transactional entity and relationship persistence that maintains graph consistency.

## Diagram

```mermaid
flowchart LR
    subgraph RetrievalModes["Multi-Modal Retrieval"]
        direction TB
        
        subgraph Vector["Vector Search"]
            v1[Memory Embedding] --> v2[Similarity Score]
            v2 --> v3[Semantic Matches]
        end
        
        subgraph FTS["Full-Text Search"]
            f1[Text Index] --> f2[Keyword Matching]
            f2 --> f3[Lexical Matches]
        end
        
        subgraph Graph["Graph Retrieval"]
            g1[Entity Query] --> g2[Relationship Traversal]
            g2a[Find neighbors] --> g2
            g2b[Find paths] --> g2
            g2c[Cluster detection] --> g2
            g2 --> g3[Connected Entities]
        end
        
        Vector --> Combine[Result Fusion]
        FTS --> Combine
        Graph --> Combine
        Combine --> Output[Ranked Results]
    end
    
    subgraph GraphOps["Graph Operations"]
        direction TB
        op1[Technology Recommendations<br/>"Projects like yours use..."]
        op2[Anti-pattern Detection<br/>"Others avoid this combination"]
        op3[Dependency Analysis<br/>"What does this stack require?"]
        op4[Concept Clustering<br/>"Related methodologies"]
    end
    
    Graph --> GraphOps
```

## External Resources

- [Graph database concepts](https://en.wikipedia.org/wiki/Graph_database) - Graph database concepts
- [Graph traversal algorithms](https://en.wikipedia.org/wiki/Graph_traversal) - Graph traversal algorithms
- [Neo4j graph database platform](https://neo4j.com/) - Neo4j graph database platform

## Sources

- [knowledge_graph](../sources/knowledge-graph.md)
