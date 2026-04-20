---
title: "Message Struct"
entity_type: "technology"
type: entity
generated: "2026-04-19T15:23:30.248802355+00:00"
---

# Message Struct

**Type:** technology

### From: mod

The `Message` struct serves as the central data structure for conversation history in the ragent-core system, representing a single communication unit within a session. Each message carries a UUID v4 identifier for distributed uniqueness, a session identifier for aggregation, a role indicating the sender type, and a vector of message parts that constitute its content. The struct tracks both creation and modification timestamps using UTC datetime, enabling temporal queries and synchronization. The design intentionally separates message metadata from content through the `MessagePart` enum, allowing heterogeneous content types to coexist within a single message while maintaining a uniform interface. The struct provides factory methods including `new()` for general construction and `user_text()` for the common case of simple user messages, demonstrating ergonomic API design. The implementation of `Display` provides a concise preview format useful for logging and debugging, with smart truncation at UTF-8 character boundaries to avoid corrupting multi-byte characters.

## Diagram

```mermaid
classDiagram
    class Message {
        +String id
        +String session_id
        +Role role
        +Vec~MessagePart~ parts
        +DateTime~Utc~ created_at
        +DateTime~Utc~ updated_at
        +new(session_id, role, parts) Self
        +user_text(session_id, text) Self
        +text_content() String
    }
    class Role {
        <<enum>>
        User
        Assistant
    }
    class MessagePart {
        <<enum>>
        Text
        ToolCall
        Reasoning
        Image
    }
    Message --> Role
    Message --> MessagePart
```

## External Resources

- [UUID specification (RFC 4122)](https://www.rfc-editor.org/rfc/rfc4122) - UUID specification (RFC 4122)
- [Rust std::fmt module documentation](https://doc.rust-lang.org/std/fmt/) - Rust std::fmt module documentation

## Sources

- [mod](../sources/mod.md)
