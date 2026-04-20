---
title: "SQLite Schema Migration Strategy"
type: concept
generated: "2026-04-19T16:04:17.125747651+00:00"
---

# SQLite Schema Migration Strategy

### From: mod

The ragent storage module implements database schema management through the migrate method, which applies a comprehensive DDL script ensuring all tables and indexes exist with appropriate constraints. The migration approach is idempotent and cumulative, using CREATE TABLE IF NOT EXISTS and CREATE INDEX IF NOT EXISTS statements that can safely run on both fresh databases and existing deployments. This strategy avoids complex version tracking tables or sequential migration files, instead relying on SQLite's schema introspection capabilities and the IF NOT EXISTS clause for graceful handling of existing objects. The approach suits embedded applications where deployment simplicity outweighs the need for complex migration rollback capabilities.

The schema design demonstrates sophisticated relational modeling for an AI agent context. Core entities include sessions (conversation containers with versioning and archival), messages (with JSON-serialized parts supporting multi-modal content), provider_auth (encrypted credentials), and mcp_servers (MCP tool server configurations). Extended subsystems include todos with status tracking, journal_entries with FTS5 virtual table integration for full-text search, and the comprehensive memories system with category constraints, confidence scores, and access metrics. Foreign key relationships enforce referential integrity with CASCADE deletes for dependent objects like tags.

Index design follows query pattern analysis: messages are indexed by session and timestamp for chronological retrieval, memories have compound indexes for category-confidence and project-recency queries, journal entries support tag and project filtering. The FTS5 virtual tables (journal_fts) enable efficient relevance-ranked text search over entry titles and content. The schema evolves through milestone markers in comments (Milestone 2 for journaling, Milestone 3 for memory systems), suggesting iterative development with backward compatibility considerations. Migration occurs automatically on Storage::open and Storage::open_in_memory, ensuring schema consistency without manual intervention.

## External Resources

- [SQLite CREATE TABLE syntax and options](https://www.sqlite.org/lang_createtable.html) - SQLite CREATE TABLE syntax and options
- [SQLite foreign key support documentation](https://www.sqlite.org/foreignkeys.html) - SQLite foreign key support documentation
- [SQLite index creation and optimization](https://www.sqlite.org/lang_createindex.html) - SQLite index creation and optimization

## Sources

- [mod](../sources/mod.md)
