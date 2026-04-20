---
title: "Ragent-Core TODO Tool Implementation"
source: "todo"
type: source
tags: [rust, todo-management, agent-tools, async-trait, serde-json, crud-operations, session-state, llm-integration, task-tracking, anyhow]
generated: "2026-04-19T17:01:41.897818007+00:00"
---

# Ragent-Core TODO Tool Implementation

This document presents a comprehensive Rust implementation of session-based TODO management tools within the ragent-core crate. The implementation defines two primary tool structures—TodoReadTool and TodoWriteTool—that enable AI agents to maintain and manipulate task lists during conversational sessions. These tools integrate with a storage backend through a ToolContext, allowing persistent TODO operations across session boundaries while maintaining clean separation between read and write concerns.

The TodoReadTool provides read-only access to TODO items with optional status filtering, supporting five filter states: pending, in_progress, done, blocked, or all. It returns formatted markdown output with visual status indicators and metadata about the result count. The TodoWriteTool implements a full CRUD interface with six actions: add, update, remove, clear, and three aliases for completion (complete, completed, done). Both tools follow a consistent pattern of JSON Schema parameter validation, permission-based categorization, and structured error handling using the anyhow crate for ergonomic error propagation.

The implementation demonstrates sophisticated Rust patterns including async trait objects via async-trait, JSON manipulation with serde_json, UUID generation for unique identifiers, and comprehensive input validation. Error messages are user-friendly and contextual, guiding users toward correct usage. The formatting layer produces human-readable markdown with emoji status indicators, demonstrating attention to UX even in programmatic interfaces. This architecture enables LLM-based agents to maintain structured task state throughout complex multi-turn interactions, bridging the gap between conversational AI and traditional task management systems.

## Related

### Entities

- [TodoReadTool](../entities/todoreadtool.md) — technology
- [TodoWriteTool](../entities/todowritetool.md) — technology
- [uuid::Uuid](../entities/uuid-uuid.md) — technology
- [serde_json](../entities/serde-json.md) — technology

