---
title: "OpenSkills"
entity_type: "technology"
type: entity
generated: "2026-04-19T20:23:29.811918658+00:00"
---

# OpenSkills

**Type:** technology

### From: loader

OpenSkills is a specification for defining agent capabilities in a portable, interoperable format, originally developed by Anthropic for use with Claude and other AI agent systems. The specification defines a standard structure for skill definitions using YAML frontmatter in markdown files, enabling skills to be shared across different agent frameworks and tools. OpenSkills-compatible skills typically reside in `.agent/skills/` or `.claude/skills/` directories and include standardized fields for licensing, compatibility notes, and arbitrary metadata. The Ragent framework implements OpenSkills compatibility as a core feature, allowing users to leverage both Ragent-native skills and OpenSkills-compatible skills within the same project. The specification emphasizes declarative configuration, explicit security boundaries through tool allowlisting, and clear documentation of skill capabilities and requirements. By supporting OpenSkills, Ragent enables interoperability with the broader ecosystem of Claude-based tools and agent configurations.

## Diagram

```mermaid
erDiagram
    SKILL_MD["SKILL.md"] {
        string name
        string description
        string argument-hint
        boolean disable-model-invocation
        boolean user-invocable
        array allowed-tools
        string model
        string context
        string agent
        object hooks
        string license "OpenSkills"
        string compatibility "OpenSkills"
        object metadata "OpenSkills"
        boolean allow-dynamic-context
        string body
    }
    
    OPENSKILLS["OpenSkills Spec"] ||--o{ SKILL_MD : "defines fields"
    RAGENT["Ragent Framework"] ||--o{ SKILL_MD : "consumes"
    RAGENT ||--|| OPENSKILLS : "implements compatibility"
```

## External Resources

- [Anthropic OpenSkills specification documentation](https://docs.anthropic.com/en/docs/build-with-claude/openskills) - Anthropic OpenSkills specification documentation
- [Anthropic news and updates on agent capabilities](https://www.anthropic.com/news) - Anthropic news and updates on agent capabilities

## Sources

- [loader](../sources/loader.md)
