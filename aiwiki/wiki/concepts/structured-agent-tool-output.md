---
title: "Structured Agent Tool Output"
type: concept
generated: "2026-04-19T19:39:01.958655566+00:00"
---

# Structured Agent Tool Output

### From: team_task_claim

Structured agent tool output is a design pattern for tool implementations that simultaneously serve human-readable content and machine-parseable metadata, accommodating the dual nature of LLM-based agent consumption. The ToolOutput type in this implementation embodies this pattern with content (natural language for LLM context windows) and metadata (JSON for deterministic processing) fields. This bifurcation enables sophisticated agent frameworks where LLMs interpret results conversationally while surrounding infrastructure triggers automated actions based on structured flags.

The pattern's implementation in TeamTaskClaimTool demonstrates thoughtful content layering. The content field provides narrative descriptions of claim outcomes, incorporating task details and conditional guidance (dependency tips, completion reminders) that LLMs can synthesize into agent responses. Simultaneously, the metadata encodes identical information in normalized form—boolean flags for claim success, enumerated fields for error categories, string identifiers for tasks and agents—enabling deterministic branching in agent control flow without requiring LLM parsing of natural language.

This design's power emerges in integration scenarios. The claimed boolean enables automated workflow transitions, triggering downstream tool sequences on success or retry loops on failure. The blocked_by_dependencies flag supports dependency notification systems, allowing agents to register callbacks rather than poll. The ready_for_idle flag coordinates team-wide completion detection, automatically transitioning agents to available pools when work exhausts. These capabilities would be fragile if inferred from content parsing but become reliable with structured metadata. The pattern represents best practice for agent tool design, balancing LLM interpretability with system integrability.

## External Resources

- [OpenAI function calling patterns for structured tool outputs](https://platform.openai.com/docs/guides/function-calling) - OpenAI function calling patterns for structured tool outputs
- [Anthropic tool use documentation for Claude](https://docs.anthropic.com/en/docs/build-with-claude/tool-use) - Anthropic tool use documentation for Claude

## Sources

- [team_task_claim](../sources/team-task-claim.md)
