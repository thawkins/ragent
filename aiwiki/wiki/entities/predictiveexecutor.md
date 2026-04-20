---
title: "PredictiveExecutor"
entity_type: "technology"
type: entity
generated: "2026-04-19T22:07:14.368173430+00:00"
---

# PredictiveExecutor

**Type:** technology

### From: predictive

PredictiveExecutor serves as the central orchestrator for the entire predictive execution system, integrating pattern detection, argument extraction, file pre-fetching, and validation into a cohesive async workflow. The struct maintains multiple concurrent data structures including a PrefetchCache for file contents, an RwLock-protected HashMap for current predictions keyed by tool-argument combinations, and a HashSet tracking in-flight pre-fetch operations to prevent duplicate I/O. This architecture reflects careful separation of concerns where the executor manages state and coordination while delegating specific operations to specialized components and spawned async tasks.

The `analyze_text` method implements the core prediction algorithm, scanning streaming LLM output for patterns indicating likely tool calls and extracting relevant arguments. The implementation uses case-insensitive matching to handle natural language variation, then applies sophisticated heuristics to extract file paths and glob patterns from the surrounding context. The method assigns confidence scores based on pattern type—0.7 for file read patterns and 0.6 for glob patterns—providing a simple but extensible foundation for more sophisticated probabilistic modeling. Upon detecting a likely file read, the executor immediately spawns an async pre-fetch task, achieving true parallelism between prediction and I/O operations.

The `prefetch_file` method demonstrates advanced async Rust patterns including structured concurrency and resource cleanup. The method uses a two-phase locking approach to check and update the pending set, then spawns a detached tokio task that performs the actual file I/O, handles success and error cases with structured logging via the `tracing` crate, and ensures cleanup of the pending set even on failure. The `validate_tool_args` method showcases graceful handling of streaming JSON, recognizing partial structures and deferring validation until sufficient data arrives, while providing strict validation of required fields for known tool types. The `clear_turn_state` method enables clean conversation boundaries by atomically clearing prediction and pending state, essential for multi-turn interactions where predictions should not leak between unrelated requests.

## Diagram

```mermaid
sequenceDiagram
    participant LLM as LLM Stream
    participant PE as PredictiveExecutor
    PC as PrefetchCache
    PP as pending_prefetch
    Tokio as Tokio Runtime
    
    LLM->>PE: analyze_text(token)
    PE->>PE: pattern matching
    PE->>PE: extract_file_path_after_pattern()
    PE->>PP: check if pending
    PP-->>PE: not pending
    PE->>PP: mark as pending
    PE->>Tokio: spawn prefetch_file()
    Tokio->>Tokio: async file read
    Tokio->>PC: insert content
    Tokio->>PP: remove from pending
    PE-->>LLM: return predictions
    
    Note over PE: Later: actual tool call
    Tool->>PE: get_prefetched_content(path)
    PE->>PC: get(path)
    PC-->>PE: cached Arc~String~
    PE-->>Tool: return content (cache hit!)
```

## External Resources

- [Tokio spawn documentation - spawning concurrent async tasks](https://docs.rs/tokio/latest/tokio/task/fn.spawn.html) - Tokio spawn documentation - spawning concurrent async tasks
- [Tracing crate documentation - structured logging for Rust applications](https://docs.rs/tracing/latest/tracing/) - Tracing crate documentation - structured logging for Rust applications

## Sources

- [predictive](../sources/predictive.md)
