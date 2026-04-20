---
title: "ProfileFrontmatter"
entity_type: "technology"
type: entity
generated: "2026-04-19T15:00:25.376155232+00:00"
---

# ProfileFrontmatter

**Type:** technology

### From: custom

ProfileFrontmatter is a private struct that defines the JSON frontmatter schema for markdown-based agent profiles, enabling a more user-friendly alternative to raw JSON configuration. This struct leverages Serde's derive macros for automatic deserialization, supporting optional fields with sensible defaults while requiring only name and description as mandatory fields. The design accommodates various agent runtime parameters including mode selection (primary, subagent, or all), model specification in provider:model format, sampling parameters like temperature and top-p, maximum agentic-loop iterations through max_steps, visibility control via the hidden flag, and persistent memory scoping. The skills field accepts a vector of skill names to preload, while the options field permits arbitrary provider-specific configuration through untyped JSON values. The permissions field maintains parity with the full OASF ragent/agent/v1 schema, ensuring consistent security policy expression across both JSON and markdown formats. This struct exemplifies the framework's philosophy of meeting developers where they are—allowing agent definition in familiar markdown with structured frontmatter rather than forcing pure JSON editing—while maintaining full feature parity with the more verbose format.

## External Resources

- [Serde field attributes documentation for default values and deserialization options](https://serde.rs/attributes.html#field-attributes) - Serde field attributes documentation for default values and deserialization options
- [YAML specification (common frontmatter format, though this implementation uses JSON)](https://yaml.org/spec/) - YAML specification (common frontmatter format, though this implementation uses JSON)

## Sources

- [custom](../sources/custom.md)
