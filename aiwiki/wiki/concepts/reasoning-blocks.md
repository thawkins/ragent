---
title: "Reasoning Blocks"
type: concept
generated: "2026-04-19T15:13:13.811986850+00:00"
---

# Reasoning Blocks

### From: mod

Reasoning blocks represent an emerging paradigm in LLM interfaces where models explicitly separate chain-of-thought reasoning from final responses, enabled by `StreamEvent` variants `ReasoningStart`, `ReasoningDelta`, and `ReasoningEnd`. This feature, pioneered by Anthropic's Claude 3.5 Sonnet and similar models, addresses the tension between transparent reasoning and clean outputs—models think step-by-step in structured blocks that can be displayed, hidden, or processed separately from answers. The streaming events treat reasoning as first-class content with its own lifecycle, distinct from `TextDelta` final output, enabling UIs to collapse reasoning sections or style them differently.

The architectural significance extends beyond presentation. Separating reasoning enables different processing paths—reasoning might be logged for debugging while responses are shown to users, or reasoning tokens might be counted separately for billing. The block structure supports recursive reasoning, where models internally iterate before producing final answers. Tool use integration becomes cleaner when reasoning about which tool to call is distinct from the actual `ToolCallStart` event. The `ReasoningDelta` with `text: String` follows the same incremental pattern as content, supporting real-time reasoning display for long chains of thought.

Implementation considerations include handling models without explicit reasoning support (events simply don't fire), distinguishing reasoning from regular text when providers mix them, and UI patterns for expandable reasoning sections. The three-event pattern (start/delta/end) mirrors `TextDelta` and tool call sequences, creating API consistency. Applications may choose to concatenate reasoning deltas for storage, stream them to separate UI components, or ignore them entirely. As reasoning capabilities expand—longer chains, branching exploration, verification steps—this event structure accommodates growth without protocol changes. The explicit lifecycle enables accurate token accounting and timeout handling specifically for reasoning phases.

## External Resources

- [Anthropic extended thinking documentation](https://docs.anthropic.com/en/docs/build-with-claude/extended-thinking) - Anthropic extended thinking documentation
- [Anthropic research on visible chain-of-thought](https://www.anthropic.com/research/showing-not-just-telling-alignment) - Anthropic research on visible chain-of-thought

## Sources

- [mod](../sources/mod.md)
