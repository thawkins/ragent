---
title: "TodoReadTool"
entity_type: "technology"
type: entity
generated: "2026-04-19T17:01:41.898618490+00:00"
---

# TodoReadTool

**Type:** technology

### From: todo

TodoReadTool is a Rust struct implementing the Tool trait for read-only operations on session-scoped TODO items. This structure serves as the query interface for task retrieval, designed specifically for AI agent integration where natural language understanding must translate into structured data access. The tool validates incoming JSON parameters against a defined schema, specifically checking that status filters match allowed values before delegating to the storage backend.

The implementation leverages Rust's type system and the async-trait crate to provide asynchronous execution capabilities while maintaining trait object safety. When executed, TodoReadTool retrieves the storage reference from ToolContext, validates the optional status filter against VALID_STATUSES, and calls get_todos with appropriate session scoping. The results are transformed through format_todo_list into markdown with visual status indicators—⏳ for pending, 🔄 for in_progress, ✅ for done, and 🚫 for blocked—creating human-readable output suitable for LLM consumption.

The tool's design emphasizes defensive programming: it validates all inputs before storage interaction, provides detailed error context when storage is unavailable, and gracefully handles the "all" filter case by passing None to the underlying query. Metadata enrichment tracks result counts and applied filters, enabling downstream consumers to understand the scope of returned data. This architecture demonstrates how traditional CRUD read operations can be wrapped in AI-friendly interfaces that prioritize discoverability through JSON Schema and operational safety through comprehensive validation.

## Diagram

```mermaid
flowchart TD
    start([execute called]) --> validate_storage{Storage available?}
    validate_storage -->|No| error_storage[Return storage error]
    validate_storage -->|Yes| extract_filter[Extract status filter]
    extract_filter --> validate_status{Valid status?}
    validate_status -->|No| error_status[Return validation error]
    validate_status -->|Yes| query_storage[Call get_todos]
    query_storage --> format_output[format_todo_list]
    format_output --> build_result[Create ToolOutput]
    build_result --> end([Return result])
    error_storage --> end
    error_status --> end
```

## External Resources

- [async-trait crate documentation for async method traits in Rust](https://docs.rs/async-trait/latest/async_trait/) - async-trait crate documentation for async method traits in Rust
- [JSON Schema specification for parameter validation](https://json-schema.org/) - JSON Schema specification for parameter validation

## Sources

- [todo](../sources/todo.md)
