---
title: "Ragent Core: Custom Agent Discovery and Loading System"
source: "custom"
type: source
tags: [rust, agent-framework, configuration-management, oasf, markdown-profiles, agent-discovery, validation, ragent]
generated: "2026-04-19T15:00:25.374856709+00:00"
---

# Ragent Core: Custom Agent Discovery and Loading System

This Rust source file implements the discovery, loading, and validation system for custom OASF (Open Agent Schema Format) agents in the ragent-core framework. The module provides a hierarchical configuration system where custom agents can be defined in both user-global (`~/.ragent/agents/`) and project-local (`.ragent/agents/`) directories, with project-local definitions taking precedence through an override mechanism. The architecture supports two distinct file formats: raw `.json` OASF agent records and `.md` markdown profiles with JSON frontmatter, where the markdown body serves as the agent's system prompt. The implementation includes comprehensive validation logic ensuring agent names conform to kebab-case without spaces, system prompts meet length constraints (maximum 32,768 characters), and model references follow the `provider:model` or `provider:model@vendor` format. The validation pipeline also enforces proper ranges for sampling parameters like temperature (0.0–2.0) and top-p (0.0–1.0), validates permission rules with actions of allow, deny, or ask, and supports memory scoping at user, project, or none levels. The module's design emphasizes graceful error handling through a non-fatal diagnostic system that collects human-readable error messages while continuing to process valid agent definitions, ensuring robust operation even when individual configuration files are malformed.

## Related

### Entities

- [CustomAgentDef](../entities/customagentdef.md) — technology
- [ProfileFrontmatter](../entities/profilefrontmatter.md) — technology
- [OasfAgentRecord](../entities/oasfagentrecord.md) — technology
- [RagentAgentPayload](../entities/ragentagentpayload.md) — technology

### Concepts

- [Hierarchical Agent Discovery](../concepts/hierarchical-agent-discovery.md)
- [JSON Frontmatter Pattern](../concepts/json-frontmatter-pattern.md)
- [Non-Fatal Diagnostic Pattern](../concepts/non-fatal-diagnostic-pattern.md)
- [Agent Mode Routing](../concepts/agent-mode-routing.md)
- [Permission Rule Validation](../concepts/permission-rule-validation.md)
- [Template-Preserving System Prompts](../concepts/template-preserving-system-prompts.md)

