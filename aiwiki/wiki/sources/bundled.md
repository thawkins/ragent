---
title: "Ragent Core Bundled Skills Module"
source: "bundled"
type: source
tags: [rust, ai-agent, skill-system, code-review, automation, ragent, bundled-features, software-architecture]
generated: "2026-04-19T20:20:16.271812735+00:00"
---

# Ragent Core Bundled Skills Module

This Rust source file defines the bundled skills system for ragent-core, a Rust-based AI agent framework. The module provides four built-in skills—simplify, batch, debug, and loop—that ship with the ragent system and are always available to users. These skills are implemented through a factory function `make_bundled_skill` that constructs `SkillInfo` structs with standardized configuration, including scope, tool permissions, and invocation settings. The bundled skills demonstrate a sophisticated approach to AI-assisted development workflows, covering code review, bulk refactoring, troubleshooting, and scheduled task automation.

The module employs a priority-based skill scoping system where bundled skills have the lowest priority (`SkillScope::Bundled`), allowing them to be overridden by personal or project-specific skills with identical names. This design enables extensibility while maintaining sensible defaults. Each skill is carefully configured with specific tool permissions—ranging from file reading and bash execution to grep searching and glob pattern matching—reflecting their intended use cases. The skills also distinguish between user-invokable and model-invokable operations, with some like `batch` and `loop` restricted to user initiation for safety and control.

Comprehensive unit tests validate the bundled skills implementation, covering name verification, scope assignment, description presence, user invocability, body content, and tool permissions. The skill bodies are implemented as string constants containing detailed instruction templates that guide the AI agent's behavior, including variable substitution through `$ARGUMENTS` and structured step-by-step workflows. This architecture separates the skill metadata from execution logic, enabling dynamic skill loading while maintaining type safety through Rust's compile-time checks.

## Related

### Entities

- [ragent](../entities/ragent.md) — product
- [SkillInfo](../entities/skillinfo.md) — technology
- [SkillScope](../entities/skillscope.md) — technology

### Concepts

- [Skill-Based AI Agent Architecture](../concepts/skill-based-ai-agent-architecture.md)
- [Priority-Based Configuration Override](../concepts/priority-based-configuration-override.md)
- [Least-Privilege Tool Permissions](../concepts/least-privilege-tool-permissions.md)
- [Instruction Template Pattern](../concepts/instruction-template-pattern.md)
- [User vs Model Invocation Control](../concepts/user-vs-model-invocation-control.md)

