---
title: "Cost-Aware Model Selection"
type: concept
generated: "2026-04-19T15:41:24.541238811+00:00"
---

# Cost-Aware Model Selection

### From: mod

The embedding of `Cost` information within `ModelInfo` reflects a maturation in LLM application architecture from capability-first to economics-first design. Early AI applications often defaulted to the most capable available model, but production systems at scale require sophisticated cost optimization where task-appropriate model selection can reduce expenses by orders of magnitude. The per-million-token pricing structure, normalized to USD, enables apples-to-apples comparison across providers with different billing granularities and currency denominations.

This cost awareness enables intelligent routing policies that balance quality and economics. A code review application might route simple linting suggestions to a $0.15/Mtks model while reserving $15/Mtks frontier models for complex architectural analysis. The `Cost` type's likely structure—separating input and output token pricing—acknowledges the asymmetric economics of modern LLMs, where generation is often significantly more expensive than processing. Some providers charge premium rates for cached context, others for specific capabilities like function calling, requiring nuanced cost modeling beyond simple per-token rates.

The integration of cost data into the provider abstraction, rather than external configuration, ensures accuracy through provider-sourced canonical values. This prevents the configuration drift where hardcoded prices become stale as providers adjust pricing, which occurred frequently during 2023-2024's rapid price competition. The serialization support via serde enables cost data to propagate through distributed systems, supporting centralized cost tracking and budget enforcement across service meshes. This architectural decision embeds financial awareness deeply into the AI infrastructure stack, recognizing that economic sustainability is as critical as technical capability for production deployments.

## External Resources

- [OpenAI API pricing](https://openai.com/pricing) - OpenAI API pricing
- [Anthropic API pricing](https://www.anthropic.com/pricing) - Anthropic API pricing
- [EU AI Act: Regulatory context for AI costs](https://artificialintelligenceact.eu/) - EU AI Act: Regulatory context for AI costs

## Sources

- [mod](../sources/mod.md)
