---
title: "ToolCallState Struct"
entity_type: "technology"
type: entity
generated: "2026-04-19T15:23:30.249415972+00:00"
---

# ToolCallState Struct

**Type:** technology

### From: mod

The `ToolCallState` struct provides comprehensive tracking for tool invocation lifecycles, bridging the gap between fire-and-forget function calls and observable, debuggable agent operations. It captures the complete execution context: the current status from the `ToolCallStatus` enum, serialized JSON input arguments, optional JSON output results, optional error messages, and optional timing information in milliseconds. This design enables sophisticated observability features like execution tracing, performance profiling, and failure analysis. The struct uses `serde_json::Value` for input and output with an explicit TODO comment acknowledging this as temporary until tool schemas stabilize—a pragmatic trade-off between type safety and development velocity. The optional fields reflect the reality of asynchronous execution where output, errors, and timing may not be immediately available. By embedding this state within `MessagePart::ToolCall`, the system maintains immutable message history that records not just that a tool was called, but exactly how it behaved, supporting reproducibility and audit requirements.

## Diagram

```mermaid
stateDiagram-v2
    [*] --> Pending: Tool called
    Pending --> Running: Execution starts
    Running --> Completed: Success
    Running --> Error: Exception
    Completed --> [*]
    Error --> [*]
    
    note right of Running
        ToolCallState tracks:
        - input: JSON args
        - duration_ms: timing
    end note
    
    note right of Completed
        Final state includes:
        - output: JSON result
        - error: None
    end note
    
    note right of Error
        Final state includes:
        - output: None or partial
        - error: description
```

## External Resources

- [Serde_json Value type documentation](https://docs.rs/serde_json/latest/serde_json/value/enum.Value.html) - Serde_json Value type documentation

## Sources

- [mod](../sources/mod.md)
