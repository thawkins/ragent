---
title: "JSON Schema Validation for Tool Parameters"
type: concept
generated: "2026-04-19T16:13:14.936354028+00:00"
---

# JSON Schema Validation for Tool Parameters

### From: plan

JSON Schema validation for tool parameters is a contract-based approach to ensuring that tools receive well-structured, type-correct inputs that match their declared expectations. In this implementation, both `PlanEnterTool` and `PlanExitTool` declare their parameter schemas using Serde's `json!` macro, specifying required fields, optional fields, and their respective types. The `PlanEnterTool` schema requires a "task" string and optionally accepts a "context" string, while `PlanExitTool` requires a "summary" string. This schema-first approach provides multiple benefits: it enables automatic validation before tool execution, it supports documentation generation for both human developers and LLM agents, and it creates a clear contract between tool implementers and tool consumers. The schema declarations in this code are likely consumed by higher-level systems that perform validation, generate OpenAPI-like specifications, or construct prompts for LLM-based tool selection. The combination of compile-time Rust type safety with runtime JSON schema validation provides defense in depth against malformed inputs. Additionally, the schema structure influences how LLM agents invoke these tools—clear, well-documented parameter descriptions in the schema help ensure that LLM agents provide appropriate, well-formed arguments.

## External Resources

- [JSON Schema official specification and documentation](https://json-schema.org/) - JSON Schema official specification and documentation
- [Serde serialization framework for Rust](https://serde.rs/) - Serde serialization framework for Rust

## Sources

- [plan](../sources/plan.md)
