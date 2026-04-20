---
title: "Serde Custom Serialization"
type: concept
generated: "2026-04-19T15:13:13.811893777+00:00"
---

# Serde Custom Serialization

### From: mod

This module employs advanced Serde serialization patterns to achieve flexible, provider-compatible data representations while maintaining Rust's type safety. Three distinct enum representation strategies appear: `#[serde(untagged)]` for `ChatContent` enabling automatic string-or-array detection, `#[serde(tag = "type", rename_all = "snake_case")]` for `ContentPart`'s explicit discriminated unions, and standard internally-tagged variants for other types. These choices reflect careful tradeoffs between human-readable output, format compatibility, and deserialization reliability. Field attributes like `#[serde(default)]` and `#[serde(skip)]` control presence in serialized forms—supporting optional evolution and internal state.

The `untagged` representation on `ChatContent` exemplifies pragmatic compatibility: many APIs accept either `"content": "hello"` or `"content": [{"type": "text", "text": "hello"}]`, and untagged deserialization attempts variants sequentially. This risks ambiguity if `String` could validly parse as single-element `Vec`, mitigated here by order (simple case first) and usage patterns. The tagged `ContentPart` approach inverts this—explicit `"type"` fields ensure unambiguous round-trip serialization, critical for message logging, caching, and protocol debugging. The `rename_all = "snake_case"` transformation matches JSON conventions against Rust's PascalCase variant names.

Field-level attributes solve specific architectural requirements. `#[serde(default)]` on `tools` and `options` allows omitting empty collections, producing cleaner JSON while accepting missing fields from deserializing older data. `#[serde(skip)]` on `session_id`, `request_id`, and `stream_timeout_secs` keeps infrastructure concerns out of wire formats—session tracking and timeout configuration are local implementation details, not protocol messages. The combination of `Value` (arbitrary JSON) in `options` and `parameters` with strict struct fields elsewhere balances flexibility with validation. These patterns require careful testing—round-trip serialization tests verify that provider JSON parses to Rust types and re-serializes equivalently, catching representation mismatches.

## External Resources

- [Serde enum representations comprehensive guide](https://serde.rs/enum-representations.html) - Serde enum representations comprehensive guide
- [Serde field attributes reference](https://serde.rs/field-attrs.html) - Serde field attributes reference
- [Serde container attributes for struct-level control](https://serde.rs/container-attrs.html) - Serde container attributes for struct-level control

## Sources

- [mod](../sources/mod.md)
