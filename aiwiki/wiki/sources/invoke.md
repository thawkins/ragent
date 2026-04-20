---
title: "ragent-core Skill Invocation System"
source: "invoke"
type: source
tags: [rust, ai-agent, skill-system, subagent-orchestration, dynamic-context, forked-execution, ragent-core, async-rust, tokio]
generated: "2026-04-19T20:22:02.782641487+00:00"
---

# ragent-core Skill Invocation System

This document presents the skill invocation architecture for the ragent-core Rust framework, specifically the `invoke.rs` module which handles the execution and formatting of skills within an AI agent system. The module implements a two-phase invocation pipeline: first, `invoke_skill` processes skill bodies through argument substitution and optional dynamic context injection to produce a `SkillInvocation` result; second, `invoke_forked_skill` enables isolated execution of skills in separate subagent sessions when forked execution is required. The design emphasizes security through explicit opt-in for dynamic context execution, flexibility through model and agent overrides, and comprehensive testing coverage with 14 distinct test cases validating behavior from simple argument substitution to complex forked session orchestration.

The architecture distinguishes between inline and forked execution modes. Inline invocation embeds processed skill content directly into the current conversation flow, while forked execution creates isolated sub-sessions with fresh message history, allowing potentially risky or resource-intensive operations to run without polluting the parent conversation state. The system supports sophisticated configuration including per-skill model overrides (with both `provider/model` and `provider:model` syntax), tool allowlisting, and agent specialization through the `fork_agent` field. The comprehensive test suite validates edge cases including dynamic context disablement, session ID substitution, multiline content formatting, and proper metadata propagation through the fork boundary.

## Related

### Entities

- [SkillInvocation](../entities/skillinvocation.md) — technology
- [ForkedSkillResult](../entities/forkedskillresult.md) — technology
- [SessionProcessor](../entities/sessionprocessor.md) — technology

### Concepts

- [Dynamic Context Injection](../concepts/dynamic-context-injection.md)
- [Argument Substitution](../concepts/argument-substitution.md)
- [Forked Skill Execution](../concepts/forked-skill-execution.md)
- [Skill Message Formatting](../concepts/skill-message-formatting.md)

