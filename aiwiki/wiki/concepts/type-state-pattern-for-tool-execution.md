---
title: "Type-State Pattern for Tool Execution"
type: concept
generated: "2026-04-19T15:23:30.251076150+00:00"
---

# Type-State Pattern for Tool Execution

### From: mod

The `ToolCallStatus` enum and `ToolCallState` struct implement a variation of the type-state pattern, encoding tool execution lifecycle invariants in the type system. While not using Rust's type system to enforce state transitions at compile time (which would require more complex generic state machines), the design clearly delineates the phases: `Pending` before execution, `Running` during, and terminal states `Completed` or `Error`. The optional fields in `ToolCallState` (`output`, `error`, `duration_ms`) encode semantic constraints—output only makes sense for completed calls, errors for failed ones, timing for finished executions. This approach prevents invalid state representations like having both output and error simultaneously, or duration without completion. The pattern supports robust agent implementations that can resume interrupted conversations, render appropriate UI for in-flight operations, and provide detailed failure diagnostics. The lowercase serialization ensures compatibility with conventional API conventions while the Rust enum provides type safety.

## External Resources

- [Rust Typestate Pattern explained](http://cliffle.com/blog/rust-typestate/) - Rust Typestate Pattern explained
- [Finite state machine theory](https://en.wikipedia.org/wiki/Finite-state_machine) - Finite state machine theory

## Sources

- [mod](../sources/mod.md)
