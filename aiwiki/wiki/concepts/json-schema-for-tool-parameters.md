---
title: "JSON Schema for Tool Parameters"
type: concept
generated: "2026-04-19T19:53:40.459131658+00:00"
---

# JSON Schema for Tool Parameters

### From: webfetch

JSON Schema is a vocabulary for annotating and validating JSON documents, widely adopted in AI agent systems for declaring tool parameter structures. In WebFetchTool, the `parameters_schema` method returns a JSON Schema object describing the expected input: a required URL string, optional format enum ("raw" or "text"), optional max_length integer, and optional timeout integer. This machine-readable contract enables multiple automation capabilities that bridge between language model outputs and type-safe Rust execution.

The schema serves several purposes in the agent pipeline. First, it provides context to the language model, describing available parameters in structured format that can be incorporated into system prompts or function definitions. Models generate tool invocations as JSON objects, and the schema guides this generation toward valid structures. Second, it enables validation before execution—frameworks can check that provided parameters match expected types and constraints, failing fast with clear errors rather than propagating type mismatches into Rust code. Third, it supports user interface generation, as schemas can drive form builders or command-line argument parsers for human-directed tool use.

WebFetchTool's schema design reflects practical tradeoffs in agent tool design. The URL is the only required parameter, providing a minimal viable invocation. Optional parameters with sensible defaults (text format, 50K length limit, 30s timeout) reduce cognitive load while allowing override when needed. The enum constraint on format ensures only valid values reach execution. This design pattern—required core parameters, optional tuning parameters with defaults, constrained enums for discrete choices—appears consistently across production agent tools. The inline `json!` macro usage keeps schema and implementation co-located, though larger projects might prefer external schema files for maintainability.

## External Resources

- [JSON Schema specification and documentation](https://json-schema.org/) - JSON Schema specification and documentation
- [Understanding JSON Schema guide](https://json-schema.org/understanding-json-schema/) - Understanding JSON Schema guide

## Related

- [Tool Pattern in Agent Architectures](tool-pattern-in-agent-architectures.md)

## Sources

- [webfetch](../sources/webfetch.md)
