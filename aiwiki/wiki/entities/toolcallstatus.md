---
title: "ToolCallStatus"
entity_type: "technology"
type: entity
generated: "2026-04-19T22:18:58.787054992+00:00"
---

# ToolCallStatus

**Type:** technology

### From: test_message_types

ToolCallStatus is a foundational enumeration in the ragent-core messaging system that defines the discrete states through which a tool invocation progresses during its lifecycle. This enum implements a classic state machine pattern with four distinct variants: Pending, Running, Completed, and Error. The design provides clear semantics for tracking execution progress while remaining simple enough to serialize efficiently to JSON for persistence and network transmission.

The Pending state represents tool invocations that have been requested but not yet begun execution, which is particularly relevant in asynchronous or queued execution environments. This state enables the system to acknowledge tool requests immediately while deferring actual execution, supporting patterns like speculative execution, batched processing, or execution across process or network boundaries. The Running state indicates active execution, enabling progress indicators and timeout monitoring in user interfaces.

The Completed and Error terminal states provide definitive resolution of the execution attempt. Completed indicates successful execution with valid output available, while Error captures any failure condition whether from tool execution, invalid inputs, or system-level problems. The explicit modeling of error as a distinct state rather than a value within output enables robust error handling patterns and ensures that error conditions are never ambiguous. The enum's Display and serde implementations provide human-readable string representations and seamless JSON interoperability, supporting both debugging workflows and API integrations.

## External Resources

- [Rust Display trait for string formatting](https://doc.rust-lang.org/std/fmt/trait.Display.html) - Rust Display trait for string formatting
- [Serde enum serialization patterns](https://serde.rs/remote-derive.html) - Serde enum serialization patterns
- [Finite state machine theory and applications](https://en.wikipedia.org/wiki/Finite-state_machine) - Finite state machine theory and applications

## Sources

- [test_message_types](../sources/test-message-types.md)
