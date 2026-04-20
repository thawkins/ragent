---
title: "Tagged Enum Serialization"
type: concept
generated: "2026-04-19T15:23:30.249777081+00:00"
---

# Tagged Enum Serialization

### From: mod

Tagged enum serialization is a pattern for representing sum types in serialized formats, where a discriminator field identifies which variant is present. The ragent-core message system uses serde's `#[serde(tag = "type", rename_all = "snake_case")]` attribute to produce JSON objects with a `"type"` field indicating the variant—`"text"`, `"tool_call"`, `"reasoning"`, or `"image"`. This approach maintains type information across serialization boundaries while producing human-readable, self-describing data that integrates cleanly with JavaScript consumers and LLM APIs. The snake_case renaming ensures idiomatic JSON conventions, avoiding the default Rust PascalCase that would produce `"Text"` rather than `"text"`. This pattern is particularly valuable for message parts where the client needs to dispatch to appropriate rendering or handling code based on content type. The technique demonstrates how Rust's type system can be mapped to dynamic, schema-flexible formats without sacrificing compile-time safety for the Rust implementation.

## External Resources

- [Serde internally tagged enum documentation](https://serde.rs/enum-representations.html#internally-tagged) - Serde internally tagged enum documentation
- [JSON Schema discriminator patterns](https://json-schema.org/understanding-json-schema/reference/generic.html) - JSON Schema discriminator patterns

## Sources

- [mod](../sources/mod.md)
