---
title: "TodoWriteTool"
entity_type: "technology"
type: entity
generated: "2026-04-19T17:01:41.899128685+00:00"
---

# TodoWriteTool

**Type:** technology

### From: todo

TodoWriteTool implements comprehensive TODO mutation capabilities for session-scoped task management, supporting six distinct action variants with nuanced validation requirements. Unlike its read-only counterpart, this tool handles complex state transitions including creation, modification, deletion, and bulk clearing operations. The implementation demonstrates sophisticated pattern matching in Rust, with each action variant requiring different parameter combinations and enforcing distinct validation rules.

The add action generates UUID-based identifiers through generate_todo_id when not provided, validates non-empty titles, and defaults status to "pending" while rejecting the "all" pseudo-status. Update operations require at least one mutable field (title, status, or description) and verify item existence before modification. The completion actions (complete/completed/done) provide convenient shortcuts for status transition to "done" without requiring full update parameter sets. Remove operations enhance UX by preserving and displaying the deleted item's title when available, while clear operations atomically remove all session TODOs with count reporting.

Post-operation, TodoWriteTool performs a read-back to display the updated state, enriching success messages with affected item titles when applicable. This read-after-write pattern ensures UI consistency and provides immediate feedback on operation effects. Metadata construction tracks action types and result counts, with conditional title insertion for add and update operations. The tool's error handling distinguishes between storage failures, validation errors, and missing-item scenarios, guiding users toward corrective actions through descriptive messages.

## Diagram

```mermaid
flowchart TD
    subgraph ActionDispatch["Action Dispatch"]
        start([execute]) --> extract_action[Extract action parameter]
        extract_action --> match{Match action}
        match -->|add| handle_add[Validate title/status<br/>Generate ID if needed<br/>Create in storage]
        match -->|update| handle_update[Validate ID<br/>Check mutable fields<br/>Update in storage]
        match -->|complete/done| handle_complete[Validate ID<br/>Set status to done]
        match -->|remove| handle_remove[Validate ID<br/>Lookup title<br/>Delete from storage]
        match -->|clear| handle_clear[Delete all session TODOs]
    end
    
    handle_add --> read_back
    handle_update --> read_back
    handle_complete --> read_back
    handle_remove --> read_back
    handle_clear --> read_back
    
    read_back[Read current TODO list] --> enrich[Enrich summary with title]
    enrich --> format[format_todo_list]
    format --> build_output[Build ToolOutput with metadata]
    build_output --> end([Return result])
```

## External Resources

- [UUID crate for unique identifier generation in Rust](https://docs.rs/uuid/latest/uuid/) - UUID crate for unique identifier generation in Rust
- [Rust pattern matching documentation for exhaustive action handling](https://doc.rust-lang.org/book/ch06-02-match.html) - Rust pattern matching documentation for exhaustive action handling

## Sources

- [todo](../sources/todo.md)
