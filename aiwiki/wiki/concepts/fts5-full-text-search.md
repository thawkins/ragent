---
title: "FTS5 Full-Text Search"
type: concept
generated: "2026-04-19T17:07:37.130294468+00:00"
---

# FTS5 Full-Text Search

### From: memory_search

FTS5 (Full-Text Search version 5) is SQLite's built-in full-text search extension, providing MemorySearchTool with robust keyword-based retrieval when semantic search is unavailable or inappropriate. FTS5 indexes document content using inverted indices mapping terms to their containing documents, enabling efficient Boolean and proximity queries. The implementation in MemorySearchTool uses FTS5 as both a primary search mechanism (when embeddings disabled) and a fallback (when embedding generation fails), demonstrating the enduring relevance of classical information retrieval in hybrid AI systems.

The technical advantages of FTS5 for this use case include minimal operational overhead (no external services), predictable resource usage, and transactional consistency with other SQLite operations. Unlike vector databases requiring separate infrastructure, FTS5 tables live within the same database file as structured memories, simplifying backup and replication. The extension supports ranking algorithms (BM25), prefix matching, and column-specific queries—features leveraged implicitly through the storage.search_memories abstraction. However, FTS5's limitations motivate the semantic search addition: it cannot match paraphrases, struggles with morphological variation across languages, and provides no notion of conceptual similarity.

The hybrid architecture—semantic with FTS5 fallback—reflects a broader pattern in production AI systems. Elasticsearch and OpenSearch similarly combine inverted indices with approximate nearest neighbor search for vector fields. The choice between search modes in MemorySearchTool is transparent to callers through the mode field in response metadata, enabling A/B testing and quality monitoring. Event emission records which mode served each query, supporting data-driven decisions about when semantic search justifies its computational and infrastructure costs. This measured approach to adopting AI-native retrieval contrasts with over-eager semantic-only architectures that struggle with out-of-vocabulary terms and adversarial queries.

## External Resources

- [Official SQLite FTS5 documentation](https://www.sqlite.org/fts5.html) - Official SQLite FTS5 documentation
- [SQLite FTS3/FTS4 documentation for historical context](https://www.sqlite.org/fts3.html) - SQLite FTS3/FTS4 documentation for historical context
- [BM25 ranking function used in FTS5](https://en.wikipedia.org/wiki/Okapi_BM25) - BM25 ranking function used in FTS5

## Sources

- [memory_search](../sources/memory-search.md)
