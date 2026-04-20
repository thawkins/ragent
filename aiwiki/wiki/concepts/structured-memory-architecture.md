---
title: "Structured Memory Architecture"
type: concept
generated: "2026-04-19T21:44:33.455809640+00:00"
---

# Structured Memory Architecture

### From: store

The structured memory architecture represents a paradigm shift from document-centric to entity-centric knowledge management in agent systems. Where traditional approaches store monolithic Markdown files with implicit structure, this architecture elevates individual facts, patterns, and insights to first-class database entities with explicit schemas and queryable metadata. The design reflects database normalization principles applied to agent memory—decomposing knowledge into atomic units with foreign key relationships (memory_tags table) rather than embedding tags as delimited strings. This enables precise operations like "find all patterns with confidence above 0.8 in the current project" that would require fragile regex parsing in document-based systems.

The six-category taxonomy (fact, pattern, preference, insight, error, workflow) encodes epistemic distinctions critical for agent reasoning. Facts represent objective ground truth about codebases or tools; patterns capture recurring solutions; preferences encode stylistic constraints; insights record derived knowledge from problem-solving; errors maintain debugging history; workflows preserve procedural knowledge. This ontology enables context-appropriate retrieval—debugging scenarios prioritize error and pattern memories, while planning tasks leverage workflow and preference knowledge. The category system is extensible through compile-time constants, though changes require migration considerations for existing databases.

Full-text search integration via SQLite FTS5 bridges the gap between structured and unstructured retrieval. While category and tag filters enable precise targeting, the FTS5 virtual table supports semantic similarity matching on content fields. The memories_fts table (referenced in documentation) likely uses contentless or external content tables to maintain synchronization with the primary memories table. This hybrid approach—structured metadata for filtering, inverted indices for content search—provides flexibility without sacrificing performance. Access tracking fields (access_count, last_accessed) enable adaptive relevance ranking, supporting future implementations of forgetting policies based on usage patterns rather than just age or confidence.

## External Resources

- [SQLite FTS5 documentation for full-text search](https://www.sqlite.org/fts5.html) - SQLite FTS5 documentation for full-text search
- [Database normalization principles](https://en.wikipedia.org/wiki/Database_normalization) - Database normalization principles
- [Survey on knowledge graphs and neural-symbolic integration](https://arxiv.org/abs/2009.00031) - Survey on knowledge graphs and neural-symbolic integration

## Sources

- [store](../sources/store.md)
