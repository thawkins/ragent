---
title: "Skill Loader Module - YAML Frontmatter Parsing and Discovery"
source: "loader"
type: source
tags: [rust, ai-agents, skill-system, yaml-parsing, configuration, serde, ragent, openskills, anthropic]
generated: "2026-04-19T20:23:29.810023858+00:00"
---

# Skill Loader Module - YAML Frontmatter Parsing and Discovery

This document describes the `loader.rs` module from the `ragent-core` crate, which implements skill discovery and YAML frontmatter parsing for an AI agent system. The module defines the core functionality for parsing `SKILL.md` files that contain YAML frontmatter delimited by `---` markers, extracting metadata fields such as skill name, description, execution context, allowed tools, and agent configuration. The module supports multiple skill scopes with a defined priority hierarchy: OpenSkills global, personal user skills, extra configured directories, OpenSkills project-local, project-specific, and monorepo nested directories. Higher-scope skills override lower-scope ones when name conflicts occur. The implementation includes comprehensive validation for skill names (lowercase ASCII letters, digits, and hyphens, maximum 64 characters), flexible parsing for tool specifications (single string or list), and robust error handling that logs warnings for individual parse failures while continuing to process valid skills. The module also provides extensive test coverage with 24 test cases validating minimal frontmatter, full configuration parsing, edge cases, and discovery from various directory structures.

## Related

### Entities

- [Ragent](../entities/ragent.md) — product
- [OpenSkills](../entities/openskills.md) — technology
- [Serde](../entities/serde.md) — technology

### Concepts

- [YAML Frontmatter Parsing](../concepts/yaml-frontmatter-parsing.md)
- [Skill Scope Hierarchy](../concepts/skill-scope-hierarchy.md)
- [Tool Allowlisting and Security](../concepts/tool-allowlisting-and-security.md)
- [Forked Execution Context](../concepts/forked-execution-context.md)

