---
title: "Custom Agents in ragent"
source: "custom-agents"
type: source
tags: [ragent, custom-agents, configuration, OASF, agent-profiles, permissions, persistent-memory, team-blueprints, AI-agents, system-prompts]
generated: "2026-04-18T15:04:48.718384174+00:00"
---

# Custom Agents in ragent

This document describes how to create and configure custom agents in ragent, a framework for AI-assisted development. Custom agents can be defined in two formats: Agent Profiles (.md files with JSON frontmatter where the markdown body serves as the system prompt) and OASF Records (.json files following the Open Agentic Schema Framework). Agents are discovered from either user-global (~/.ragent/agents/) or project-local ([PROJECT]/.ragent/agents/) directories, with project-local definitions taking priority.

The document provides detailed configuration options including system prompts, model selection, temperature settings, permissions, and memory scopes. Permission rules control file and shell operations through pattern matching with allow/deny/ask actions. Agents can also maintain persistent memory across sessions at user or project scope, enabling institutional knowledge accumulation. Custom agents can be referenced in team blueprints through spawn-prompts.json, allowing flexible team composition with specialized teammates for different tasks.

## Related

### Entities

- [ragent](../entities/ragent.md) — product
- [Open Agentic Schema Framework](../entities/open-agentic-schema-framework.md) — technology
- [spawn-prompts.json](../entities/spawn-prompts-json.md) — product

### Concepts

- [Agent Profiles](../concepts/agent-profiles.md)
- [OASF Records](../concepts/oasf-records.md)
- [Permission Rules](../concepts/permission-rules.md)
- [Persistent Memory](../concepts/persistent-memory.md)
- [Discovery Paths](../concepts/discovery-paths.md)
- [Team Blueprints](../concepts/team-blueprints.md)
- [Memory Scope Inheritance](../concepts/memory-scope-inheritance.md)

