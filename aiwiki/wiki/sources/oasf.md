---
title: "OASF Agent Configuration Schema for Ragent"
source: "oasf"
type: source
tags: [oasf, agent-configuration, rust, serde, json-schema, ai-agents, open-standards, ragent, data-structures, serialization]
generated: "2026-04-19T14:58:16.759745669+00:00"
---

# OASF Agent Configuration Schema for Ragent

This document defines the Rust data structures implementing the Open Agentic Schema Framework (OASF) for agent configuration in the ragent-core system. The module establishes a standardized JSON-based format for defining AI agents, combining the extensible OASF envelope structure with ragent-specific payload configuration. The schema enables declarative agent definitions through UTF-8 JSON files stored in discovery directories, supporting versioned agent records with metadata, taxonomy annotations, source locators, and extension modules.

The implementation centers on two primary structures: the `OasfAgentRecord` envelope that provides interoperability with the broader OASF ecosystem, and the `RagentAgentPayload` that contains ragent-specific runtime configuration. This architectural separation allows the ragent system to participate in standardized agent registries while maintaining flexibility for implementation-specific features. The design includes comprehensive fields for agent behavior control including system prompts with template variable substitution, execution mode selection, step limits, model configuration, skill preloading, permission rulesets, memory scoping, and provider-specific options.

Key technical characteristics include serde-based serialization for JSON compatibility, hierarchical taxonomy annotations for skills and domains, locator references to source code or container registries, and a permission system using glob patterns for fine-grained tool access control. The module identifier constant `RAGENT_MODULE_TYPE` ensures proper module discrimination within the extensible OASF framework. The permission rule structure supports security-sensitive operations through configurable actions of allow, deny, or interactive prompting.

## Related

### Entities

- [Open Agentic Schema Framework (OASF)](../entities/open-agentic-schema-framework-oasf.md) — technology
- [ragent](../entities/ragent.md) — product
- [Serde](../entities/serde.md) — technology

### Concepts

- [Agent Configuration as Code](../concepts/agent-configuration-as-code.md)
- [Capability-Based Security for AI Agents](../concepts/capability-based-security-for-ai-agents.md)
- [Hierarchical Taxonomy for Agent Classification](../concepts/hierarchical-taxonomy-for-agent-classification.md)
- [Extension Module Architecture](../concepts/extension-module-architecture.md)

