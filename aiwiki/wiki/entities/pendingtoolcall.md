---
title: "PendingToolCall"
entity_type: "technology"
type: entity
generated: "2026-04-19T15:58:32.776670309+00:00"
---

# PendingToolCall

**Type:** technology

### From: processor

The `PendingToolCall` struct represents an internal state container for tracking tool invocations that have been requested by the LLM but not yet completed or fully processed. While the source shows this as a private struct without detailed field visibility, its presence in the module's struct declarations alongside its usage patterns in the references section indicates it manages the intermediate state during the agentic loop's tool execution phase.

This struct likely encapsulates the information needed to correlate streaming tool call deltas with their final execution, including partial argument accumulation from `ToolCallDelta` events, the target tool identifier, and invocation state tracking. The design separates pending state management from the final `ToolCallState` used in message history, allowing for robust handling of streaming responses where tool calls may arrive incrementally across multiple stream events.

The struct's private visibility scope indicates it's an implementation detail of the processor's internal state machine, not exposed to consumers of the API. This encapsulation prevents external code from depending on unstable internal representations while allowing the processor to evolve its tool tracking logic without breaking changes. The references to `iter_mut`, `args_json`, and `clear` operations in the source suggest mutable iteration over pending calls and JSON argument accumulation capabilities.

## Diagram

```mermaid
flowchart LR
    subgraph ToolExecutionFlow["Tool Execution Flow"]
        direction LR
        stream[LLM Stream] --> delta["ToolCallDelta"]
        delta --> pending["PendingToolCall"]
        pending --> accumulate["Accumulate Args"]
        accumulate --> end["ToolCallEnd"]
        end --> execute["Execute Tool"]
        execute --> result["ToolResult"]
        result --> history["Message History"]
    end
```

## Sources

- [processor](../sources/processor.md)
