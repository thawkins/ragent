---
title: "Extended Thinking and Reasoning Control"
type: concept
generated: "2026-04-19T15:30:55.697103436+00:00"
---

# Extended Thinking and Reasoning Control

### From: anthropic

Extended thinking is an advanced capability in modern LLMs that exposes the model's internal reasoning process, implemented in this code through optional `thinking` configuration and specialized event types for reasoning content. Unlike standard mode where reasoning is implicit and hidden, extended thinking produces explicit reasoning steps that can be observed, measured, and controlled. The implementation supports three states: disabled (no reasoning output), enabled with budget (controlled reasoning depth), and default (model-determined reasoning). This feature addresses important use cases in trustworthy AI applications—users and developers can verify the reasoning process behind answers, identify where models may be confused, and provide feedback on reasoning quality in addition to final outputs.

The technical implementation demonstrates careful API design for controlling cognitive effort. The `thinking` option accepts string values `"disabled"` and `"enabled"`, with enabled mode requiring a `thinking_budget_tokens` parameter that sets an upper bound on reasoning tokens. This budget mechanism prevents runaway reasoning that could consume excessive time and cost, while still allowing deep analysis when needed. The code constructs the appropriate JSON structure for Anthropic's API, with `{"type": "enabled", "budget_tokens": N}` for enabled mode and `{"type": "disabled"}` for disabled mode. The streaming response separates reasoning content from final answers through distinct event types (`ReasoningStart`, `ReasoningDelta`) and content block types (`thinking`, `thinking_delta`), enabling UIs to display reasoning in collapsible sections or with different styling.

The architectural significance of extended thinking extends beyond UI presentation to fundamental questions of AI transparency and control. By making reasoning explicit, this feature enables new debugging workflows, educational applications where showing work is pedagogically valuable, and high-stakes decision support where reasoning audit trails may be required. The implementation's handling of thinking as a separate content type that streams alongside (and potentially interleaved with) final output suggests a future where LLM responses are structured compositions of multiple cognitive processes rather than monolithic text generation. The careful separation of reasoning deltas from text deltas in the event stream, and the provider-agnostic abstraction through `StreamEvent` variants, demonstrates how this capability can be integrated into broader agent frameworks while maintaining clean separation between transport-specific details and application-level semantics.

## External Resources

- [Anthropic extended thinking documentation](https://docs.anthropic.com/en/docs/build-with-claude/extended-thinking) - Anthropic extended thinking documentation
- [Anthropic research on showing model thoughts](https://www.anthropic.com/research/showing-language-model-thoughts) - Anthropic research on showing model thoughts

## Sources

- [anthropic](../sources/anthropic.md)
