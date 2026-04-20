---
title: "Forked Subagent Context"
type: concept
generated: "2026-04-19T20:25:21.328507238+00:00"
---

# Forked Subagent Context

### From: mod

The forked subagent context execution mode enables skills to run in isolated conversation environments, preventing context pollution between specialized operations. When a skill specifies context: Fork in its YAML frontmatter, the system creates a new subagent with independent conversation history rather than continuing the current dialogue thread. This isolation proves valuable for operations requiring clean state, such as exploration tasks that might otherwise pollute the main context with temporary findings, or operations requiring different model configurations.

The implementation represents execution context through the SkillContext enumeration, currently containing only the Fork variant but designed for extensibility through the enum structure. The SkillInfo::is_forked method checks for SkillContext::Fork presence, returning false for skills without explicit context configuration. This design follows Rust's Option-based null handling, where None represents standard inline execution and Some(SkillContext::Fork) triggers subagent creation.

The forked context pattern addresses specific challenges in AI agent architecture: conversation context windows are finite, and specialized operations may consume significant tokens or introduce irrelevant information. By forking, skills can perform extensive exploration, multiple tool calls, or iterative refinement without affecting the parent conversation's coherence. The agent field complements this by specifying subagent type (e.g., "explore", "general-purpose"), enabling different system prompts and capabilities for forked contexts. This architecture mirrors process forking in operating systems, where child processes inherit environment but maintain independent execution state.

## External Resources

- [Operating system fork concept (conceptual parallel)](https://en.wikipedia.org/wiki/Fork_(system_call)) - Operating system fork concept (conceptual parallel)

## Sources

- [mod](../sources/mod.md)
