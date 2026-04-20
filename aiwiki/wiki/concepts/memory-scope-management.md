---
title: "Memory Scope Management"
type: concept
generated: "2026-04-19T19:31:06.819290099+00:00"
---

# Memory Scope Management

### From: team_spawn

Memory scope management in `TeamSpawnTool` addresses the challenge of persistent context across agent sessions through a tiered storage abstraction supporting user-global, project-local, and ephemeral memory configurations. The `MemoryScope` enum—with variants `User`, `Project`, and `None`—enables agents to maintain accumulated knowledge, preferences, and working state across conversation boundaries while respecting appropriate isolation boundaries. This capability transforms stateless request-response interactions into stateful collaborations where teammates build upon prior context, a critical enabler for complex multi-session workflows in software development, research, and other longitudinal tasks.

The implementation reveals careful attention to scope semantics and persistence guarantees. Memory scope configuration occurs during teammate spawning and persists through `TeamStore` mutation, indicating that scope selection affects long-term storage allocation rather than merely runtime environment variables. The default `None` variant provides safe baseline behavior for ephemeral collaboration, while explicit opt-in to `User` or `Project` scopes indicates intentional context sharing decisions. The persistence mechanism—modifying member records in team storage—suggests that memory scope travels with the agent identity, enabling consistent scope behavior even if the same teammate is spawned multiple times or across different orchestration sessions.

The relationship between memory scope and other persistence systems merits examination. The implementation distinguishes memory scope (cross-session context) from immediate prompt content (single-session context) and from task store assignments (workflow state). This separation of concerns enables independent optimization: ephemeral prompts minimize token consumption, durable task state enables recovery from failures, and persistent memory enables personalization and knowledge accumulation. The filesystem-based storage implied by `find_team_dir` and `TeamStore::load` operations suggests portable, inspectable persistence that integrates with version control and backup systems, contrasting with opaque database or remote service dependencies. Together these patterns constitute a comprehensive state management strategy balancing immediate efficiency, operational resilience, and longitudinal capability growth.

## External Resources

- [LangChain memory module patterns for conversational AI](https://langchain.com/v1/docs/modules/memory/) - LangChain memory module patterns for conversational AI
- [Anthropic guidance on long context and memory techniques](https://docs.anthropic.com/en/docs/build-with-claude/prompt-engineering/long-context) - Anthropic guidance on long context and memory techniques

## Related

- [Context Window Management](context-window-management.md)
- [Multi-Agent Orchestration](multi-agent-orchestration.md)

## Sources

- [team_spawn](../sources/team-spawn.md)
