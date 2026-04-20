---
title: "JSON Schema Validation"
type: concept
generated: "2026-04-19T16:59:30.010624260+00:00"
---

# JSON Schema Validation

### From: file_ops_tool

JSON Schema validation in `FileOpsTool` implements a self-describing interface pattern where tools declare their input requirements through machine-readable schemas. The `parameters_schema()` method returns a JSON Schema object constructed via the `json!` macro, describing an object with required `edits` array and optional `concurrency` integer and `dry_run` boolean fields. This schema serves multiple purposes: enabling automatic validation of incoming requests, generating documentation and UI forms, and assisting language models in constructing valid tool calls.

The schema structure reflects careful API design for agent systems. The `edits` array contains objects with `path` and `content` strings, a structure that maps naturally to file modification operations while being extensible for future enhancements like line-range specifications or encoding hints. Marking `edits` as required while leaving `concurrency` and `dry_run` optional follows the principle of sensible defaults—agents can invoke the tool with minimal specification while power users can tune behavior. The schema's explicit typing (`"type": "string"`, `"type": "integer"`) enables validators to catch type mismatches before execution.

This approach contrasts with informal parameter passing by providing formal contracts that can be enforced automatically. In LLM-based agent systems, the schema is particularly valuable as it can be included in context windows to guide model output, or used with constrained decoding techniques to ensure syntactically valid invocations. The validation likely occurs in a framework layer before `execute()` is called, providing defense in depth against malformed inputs. The use of JSON Schema specifically, rather than custom formats, ensures interoperability with existing tooling ecosystems including OpenAPI, form generators, and documentation systems.

## External Resources

- [JSON Schema specification and documentation](https://json-schema.org/) - JSON Schema specification and documentation
- [Ajv JSON Schema validator - ecosystem reference](https://ajv.js.org/) - Ajv JSON Schema validator - ecosystem reference

## Sources

- [file_ops_tool](../sources/file-ops-tool.md)

### From: team_approve_plan

JSON Schema validation in this implementation provides machine-readable API contracts that enable automatic parameter validation, IDE autocomplete, and LLM-guided tool usage. The `parameters_schema` method returns a JSON Schema document describing required fields, types, and semantic descriptions for each parameter. This approach contrasts with ad-hoc parsing where validation logic is interspersed with business logic, enabling centralized schema evolution and consistent error messaging across tools.

The schema design reveals thoughtful API ergonomics: the `approved` boolean's clear naming avoids the ambiguity of status strings, while the conditional requirement for feedback (implied by description rather than schema) accommodates the common case of silent approvals without redundant parameters. The `team_name` and `teammate` string fields use descriptive naming that distinguishes team identity from member identity, preventing common confusion in multi-tenant systems. Schema descriptions serve dual purposes: human documentation and LLM context for agent systems that construct tool calls.

The integration with serde_json enables seamless serialization of validated parameters into Rust types, though the implementation shows manual extraction via `get` and `as_str` methods rather than derive-based deserialization. This choice provides fine-grained control over error messages and supports partial validation where some parameters might be processed before others fail. The schema-driven approach future-proofs the API against breaking changes—new optional fields can be added without breaking existing callers, and required field changes are explicitly documented in schema version diffs.
