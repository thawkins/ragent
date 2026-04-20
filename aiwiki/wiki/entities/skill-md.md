---
title: "SKILL.md"
entity_type: "technology"
type: entity
generated: "2026-04-19T20:25:21.326329758+00:00"
---

# SKILL.md

**Type:** technology

### From: mod

SKILL.md is a file format specification that defines AI agent skills through a combination of YAML frontmatter and markdown content. The format serves as the canonical definition for skills in the Ragent ecosystem and compatible systems, enabling both human readability and machine parsing. The YAML frontmatter section contains structured metadata including name, description, invocation permissions, allowed tools, model overrides, execution context, and licensing information, while the markdown body contains the actual instructions provided to the agent.

The format specification includes constraints on the name field: lowercase, hyphens allowed, maximum 64 characters. Boolean flags control invocation modes: user_invocable determines visibility in the / command menu, while disable_model_invocation prevents automatic agent triggering. The format supports optional features like dynamic context injection (allow_dynamic_context), forked execution contexts (context: Fork), and subagent type specification (agent field). These capabilities enable sophisticated execution patterns while maintaining declarative simplicity.

SKILL.md files are discovered through hierarchical directory scanning based on scope rules, with higher-priority locations overriding lower ones when names conflict. The format's design reflects lessons from static site generators like Jekyll that popularized YAML frontmatter, adapted for the specific requirements of agent instruction packaging. The separation of metadata from instructions enables tooling to index and query skills without parsing full instruction content, supporting efficient registry operations and skill discovery interfaces.

## External Resources

- [Jekyll front matter documentation (historical precedent for YAML frontmatter)](https://jekyllrb.com/docs/front-matter/) - Jekyll front matter documentation (historical precedent for YAML frontmatter)

## Sources

- [mod](../sources/mod.md)
