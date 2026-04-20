---
title: "Typed Newtype Wrappers for Identifiers in ragent-core"
source: "id"
type: source
tags: [rust, type-safety, newtype-pattern, identifiers, uuid, serde, macro, code-generation]
generated: "2026-04-19T20:15:48.082763582+00:00"
---

# Typed Newtype Wrappers for Identifiers in ragent-core

This source code file defines a Rust macro and implementation for creating strongly-typed identifier wrappers in the ragent-core library. The `define_id!` macro generates newtype structs that wrap `String` values, providing compile-time type safety that prevents accidental misuse of different identifier types. The implementation includes comprehensive trait implementations for common operations including serialization via serde, display formatting, default value creation, and conversions from strings. Four specific identifier types are instantiated: `SessionId` for session tracking, `MessageId` for message identification, `ProviderId` for AI provider references, and `ToolCallId` for tool invocation tracking. Each identifier type automatically receives UUID v4 generation for new instances, making them suitable for distributed systems where collision-resistant unique identifiers are essential. The design pattern leverages Rust's zero-cost abstractions to provide safety without runtime overhead.

## Related

### Entities

- [ragent-core](../entities/ragent-core.md) — product
- [define_id macro](../entities/define-id-macro.md) — technology

### Concepts

- [Newtype Pattern](../concepts/newtype-pattern.md)
- [Compile-Time Type Safety](../concepts/compile-time-type-safety.md)
- [Declarative Macros in Rust](../concepts/declarative-macros-in-rust.md)
- [UUID v4 Generation](../concepts/uuid-v4-generation.md)
- [Serde Serialization Framework](../concepts/serde-serialization-framework.md)

