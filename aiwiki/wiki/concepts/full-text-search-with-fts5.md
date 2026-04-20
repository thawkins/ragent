---
title: "Full-Text Search with FTS5"
type: concept
generated: "2026-04-19T18:57:31.764120730+00:00"
---

# Full-Text Search with FTS5

### From: structured_memory

Full-text search (FTS) represents a specialized database indexing and querying capability that enables efficient natural language retrieval over large text corpora, with FTS5 being SQLite's fifth-generation implementation of this technology. Unlike standard SQL pattern matching with LIKE operators that scan entire tables, FTS systems build inverted indices mapping terms to their containing documents, enabling sub-linear query performance regardless of corpus size. This performance characteristic is essential for responsive agent systems that may accumulate thousands or millions of memories over extended operation, where naive scanning would introduce unacceptable latency. The FTS5 extension specifically provides advanced features including ranking algorithms, snippet extraction, and customizable tokenizers that support domain-specific text processing.

The integration pattern observed in this codebase—where MemoryRecallTool accepts a query string and passes it to storage.search_memories—demonstrates typical application architecture for FTS systems. The query processing involves tokenization of input terms, index lookup for each term, and set intersection or union operations depending on search semantics. The implementation specifies that space-separated terms require all to match (AND semantics), which reduces false positives for agent knowledge retrieval where precision often outweighs recall. This differs from web search patterns that might employ OR semantics with relevance ranking, reflecting different optimization targets between conversational agent context and information discovery applications.

FTS5's relationship with structured filtering creates a powerful hybrid retrieval model. The underlying virtual table can be joined with standard relational tables, enabling the combined full-text and metadata constraints exercised by this system—searching content while requiring specific categories, mandating tag presence, and filtering by confidence thresholds. This hybrid approach outperforms either pure vector similarity search (which lacks interpretable filtering) or pure relational querying (which lacks semantic matching). The access count tracking integrated with search results supports reinforcement learning applications where frequently retrieved memories might be promoted or cached, while rarely accessed memories might be candidates for archival or deletion through MemoryForgetTool operations. Understanding FTS capabilities and limitations is essential for effective schema design in knowledge-intensive applications.

## External Resources

- [Official SQLite FTS5 documentation](https://www.sqlite.org/fts5.html) - Official SQLite FTS5 documentation
- [PostgreSQL full-text search for comparison](https://www.postgresql.org/docs/current/textsearch.html) - PostgreSQL full-text search for comparison
- [Inverted index data structure on Wikipedia](https://en.wikipedia.org/wiki/Inverted_index) - Inverted index data structure on Wikipedia

## Related

- [Structured Memory Systems](structured-memory-systems.md)

## Sources

- [structured_memory](../sources/structured-memory.md)
