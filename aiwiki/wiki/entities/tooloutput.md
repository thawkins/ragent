---
title: "ToolOutput"
entity_type: "technology"
type: entity
generated: "2026-04-19T17:24:32.540980424+00:00"
---

# ToolOutput

**Type:** technology

### From: codeindex_reindex

ToolOutput is a structured result type that standardizes how tools communicate their execution results back to callers within the agent-core tool system. This struct addresses the fundamental challenge of tool integration: providing both immediate human-readable feedback and machine-parseable structured data within a single return value. The type contains two primary fields: content, a String for human consumption, and metadata, an Option<Value> containing JSON-structured data for programmatic use.

The design of ToolOutput reflects deep understanding of the dual audiences for tool results in modern development environments. Human developers need clear, concise descriptions of what happened, formatted for readability in terminals and log viewers. Concurrently, automated systems—including AI agents, CI/CD pipelines, and IDE integrations—require structured data they can parse and act upon without natural language processing. By bundling both representations together, ToolOutput eliminates the need for callers to parse human-readable strings or for tools to maintain parallel output formats.

The metadata field's use of serde_json::Value provides maximal flexibility for tools to return domain-specific result structures without requiring changes to the ToolOutput type itself. This dynamic typing approach allows each tool to define its own result schema while still conforming to a common interface. In the case of CodeIndexReindexTool, the metadata includes quantitative metrics about the re-indexing operation: counts of files added, updated, and removed, symbols extracted, and elapsed milliseconds. These metrics enable downstream consumers to track indexing performance over time, detect anomalies, and make data-driven decisions about when to trigger re-indexing operations. The optional nature of metadata acknowledges that not all tool operations produce structured data, while the JSON Value type ensures that when metadata is present, it can be serialized, stored, and transmitted across network boundaries with full fidelity.

## Sources

- [codeindex_reindex](../sources/codeindex-reindex.md)

### From: team_memory_write

ToolOutput is the standardized return type for tool executions in the ragent framework, designed to provide both human-readable feedback and machine-processable metadata. The struct balances immediate user communication needs with structured data for downstream automation, logging, and debugging systems. Every tool execution concludes with a ToolOutput instance, whether successful or handled as an expected error condition.

The content field carries the primary human-readable message, formatted with specific details about the operation performed. In TeamMemoryWriteTool, this includes byte counts, target paths, and write modes—information immediately useful to developers monitoring agent behavior or users reviewing execution logs. The format! macro constructs these messages dynamically, incorporating runtime values that make each output instance specific to its execution context.

The metadata field, typed as Option<Value> (serde_json's flexible JSON representation), enables rich structured output without rigid schema constraints. Successful writes include memory_dir, path, mode, byte_count, and line_count as JSON properties. Error conditions embed machine-readable error codes like "memory_disabled" or "path_escape". This dual-output design supports multiple consumption patterns: log aggregation systems can index metadata fields, debugging interfaces can display rich tooltips from structured data, and simple consumers can rely solely on the content string. The Option wrapper permits tools to omit metadata when unnecessary, though this implementation consistently provides it.
