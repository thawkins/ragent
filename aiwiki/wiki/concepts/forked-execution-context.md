---
title: "Forked Execution Context"
type: concept
generated: "2026-04-19T20:23:29.814387788+00:00"
---

# Forked Execution Context

### From: loader

Forked execution context is an advanced skill configuration that creates isolated subagent processes for executing skill logic, preventing pollution of the main agent's state and enabling concurrent or independent operation chains. When `context: fork` is specified in a skill's frontmatter, the Ragent system spawns a dedicated subagent instance—configured via the `agent` field (e.g., `agent: general-purpose`)—to handle the skill's execution. This pattern is particularly valuable for long-running operations, potentially dangerous commands that might affect global state, or workflows requiring specialized agent configurations different from the main session. The `is_forked()` method on `SkillInfo` provides runtime introspection of this property. Forked contexts enable sophisticated workflows like deployment pipelines that run independently while the main agent continues interactive work, or specialized analysis tasks using different model configurations. The isolation ensures that tool executions, context modifications, and errors within the forked skill don't propagate to the primary agent session, maintaining stability and predictability.

## Diagram

```mermaid
sequenceDiagram
    participant User
    participant MainAgent as Main Agent
    participant SkillSystem as Skill System
    participant ForkedAgent as Forked Subagent
    participant Tools as Allowed Tools

    User->>MainAgent: /deploy production
    MainAgent->>SkillSystem: Invoke 'deploy' skill
    SkillSystem->>SkillSystem: Check context=fork?
    Note over SkillSystem: context=fork, agent=general-purpose
    SkillSystem->>ForkedAgent: Spawn subagent with config
    ForkedAgent->>ForkedAgent: Parse $ARGUMENTS
    ForkedAgent->>Tools: Execute bash, read tools
    Tools-->>ForkedAgent: Results
    ForkedAgent->>ForkedAgent: Complete workflow
    ForkedAgent-->>SkillSystem: Return result
    SkillSystem-->>MainAgent: Skill complete
    MainAgent-->>User: Deployment finished
```

## External Resources

- [Fork system call concept in operating systems](https://en.wikipedia.org/wiki/Fork_(system_call)) - Fork system call concept in operating systems
- [Anthropic's computer use and agent isolation patterns](https://docs.anthropic.com/en/docs/build-with-claude/computer-use) - Anthropic's computer use and agent isolation patterns

## Sources

- [loader](../sources/loader.md)
