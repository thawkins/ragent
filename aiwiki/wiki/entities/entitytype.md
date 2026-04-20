---
title: "EntityType"
entity_type: "technology"
type: entity
generated: "2026-04-19T21:52:48.054362479+00:00"
---

# EntityType

**Type:** technology

### From: knowledge_graph

EntityType is a strongly-typed enumeration that categorizes extracted entities into six semantically distinct categories, providing the foundational taxonomy for the knowledge graph. The enum uses serde's rename_all attribute to ensure consistent snake_case serialization while maintaining idiomatic PascalCase Rust naming conventions. Each variant carries specific semantic weight: Project represents codebases and repositories, Tool captures libraries and frameworks, Language covers programming languages, Pattern identifies design patterns and conventions, Person tracks individuals and teams, and Concept serves as a flexible catch-all for abstract topics.

The implementation provides bidirectional string conversion through from_str and as_str methods, enabling seamless interoperability with database storage and external APIs. The Display trait implementation ensures consistent string representation throughout the application. This type system design prevents invalid entity classifications at compile time while supporting extensibility—new entity types can be added without breaking existing serialization contracts. The roundtrip test coverage verifies that all variants serialize and deserialize correctly, ensuring data integrity across system boundaries.

## External Resources

- [Rust FromStr trait documentation](https://doc.rust-lang.org/std/str/trait.FromStr.html) - Rust FromStr trait documentation
- [Serde enum serialization strategies](https://serde.rs/enum-representations.html) - Serde enum serialization strategies

## Sources

- [knowledge_graph](../sources/knowledge-graph.md)
