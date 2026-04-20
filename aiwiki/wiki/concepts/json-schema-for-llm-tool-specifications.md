---
title: "JSON Schema for LLM Tool Specifications"
type: concept
generated: "2026-04-19T17:09:53.940464383+00:00"
---

# JSON Schema for LLM Tool Specifications

### From: aliases

JSON Schema serves as the interchange format for describing tool parameters in ragent, enabling LLMs to understand what inputs a tool expects and generate appropriate function call structures. Each alias tool implements `parameters_schema()` returning a `serde_json::Value` containing a JSON Schema object that specifies types, descriptions, required fields, and constraints. This standardization is crucial for LLM function calling, where models must generate valid JSON matching the expected structure to successfully invoke tools.

The schema design in ragent demonstrates practical patterns for coding agent tools: string types for paths and content, integer types for line numbers, optional fields for advanced features, and detailed descriptions that help models understand semantic constraints (like 1-based line numbering). The `replace_in_file` alias shows sophisticated schema design where both alias parameters (`old`, `new`) and canonical parameters (`old_str`, `new_str`) are documented, giving models flexibility while guiding them toward working patterns. Descriptions like "Exact text to find — include enough context to make it unique" encode operational knowledge that improves model success rates.

The use of JSON Schema rather than Rust's type system for external interface definition reflects the cross-language nature of LLM tool use—models don't understand Rust types directly, but can process JSON Schema through their training on API documentation and OpenAPI specifications. This creates an interesting layering where Rust's type safety protects internal implementation, while JSON Schema enables flexible external interfaces. The `serde_json::json!` macro provides ergonomic schema authoring within Rust source, though production systems might prefer external schema files for easier maintenance and versioning.

## External Resources

- [JSON Schema specification](https://json-schema.org/) - JSON Schema specification
- [OpenAI function calling with JSON Schema](https://platform.openai.com/docs/guides/function-calling) - OpenAI function calling with JSON Schema
- [serde_json::json! macro documentation](https://docs.rs/serde_json/latest/serde_json/macro.json.html) - serde_json::json! macro documentation

## Sources

- [aliases](../sources/aliases.md)
