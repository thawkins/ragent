---
title: "ToolDefinition"
entity_type: "technology"
type: entity
generated: "2026-04-19T15:13:13.811354196+00:00"
---

# ToolDefinition

**Type:** technology

### From: mod

The `ToolDefinition` struct provides the schema description that enables LLMs to understand and invoke external capabilities through function calling. This struct bridges the gap between LLM reasoning and executable software, containing `name: String` for invocation identification, `description: String` for explaining purpose to the model's reasoning process, and `parameters: Value` holding a JSON Schema object defining valid argument structures. The design follows the JSON Schema standard for parameter validation, allowing models to generate syntactically correct tool invocations that can be parsed and executed by the application.

The struct's simplicity belies its architectural significance. Tool definitions transform LLMs from text generators into agents capable of interacting with external systems—querying databases, calling APIs, performing calculations, or controlling hardware. The `description` field is particularly important as it directly influences model behavior; well-crafted descriptions explaining when and how to use a tool dramatically improve invocation accuracy. The JSON Schema in `parameters` enables type-safe argument validation, with providers using schemas to constrain generation and applications validating before execution.

A TODO comment indicates plans to replace `serde_json::Value` with a typed JSON Schema struct, which would enable compile-time schema validation, IDE autocomplete for schema construction, and stronger type safety. This evolution would align with crates like `schemars` that derive JSON Schema from Rust types. The `Clone` derive supports sharing tool definitions across multiple requests, while `Debug` enables logging of available capabilities.

## External Resources

- [JSON Schema official website and specification](https://json-schema.org/) - JSON Schema official website and specification
- [OpenAI function calling and tool definitions](https://platform.openai.com/docs/guides/function-calling) - OpenAI function calling and tool definitions
- [schemars crate for JSON Schema generation from Rust types](https://docs.rs/schemars/latest/schemars/) - schemars crate for JSON Schema generation from Rust types

## Sources

- [mod](../sources/mod.md)
