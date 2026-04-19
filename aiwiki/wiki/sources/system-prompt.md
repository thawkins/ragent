---
title: "Practical Guidance for Writing Effective System Prompts"
source: "SYSTEM_PROMPT"
type: source
tags: [system prompts, LLM engineering, prompt engineering, AI safety, instruction tuning, agent systems, tool use, best practices, OpenAI, Anthropic, Google Vertex AI, Azure OpenAI, few-shot learning, output formatting, adversarial testing]
generated: "2026-04-18T15:16:37.744514779+00:00"
---

# Practical Guidance for Writing Effective System Prompts

This document provides comprehensive best practices for designing system prompts in large language model (LLM) applications, synthesizing guidance from major providers including OpenAI, Anthropic, Google/Vertex AI, and Microsoft Azure. System prompts serve as the foundational directive layer that shapes model behavior across entire conversations, establishing role, safety constraints, global style, and output contracts. The document emphasizes actionable recommendations including stating role and authority first, scoping behavior with explicit prohibitions, providing structured output contracts, using few-shot examples, and implementing verification steps for high-impact actions. It also covers practical patterns, token constraints, safety limitations, and the academic foundation in instruction-tuning research.

## Related

### Entities

- [OpenAI](../entities/openai.md) — organization
- [Anthropic](../entities/anthropic.md) — organization
- [Google](../entities/google.md) — organization
- [Microsoft Azure](../entities/microsoft-azure.md) — organization
- [FLAN](../entities/flan.md) — technology
- [T0](../entities/t0.md) — technology
- [Super-NaturalInstructions](../entities/super-naturalinstructions.md) — technology
- [Constitutional AI](../entities/constitutional-ai.md) — technology

### Concepts

- [system prompts](../concepts/system-prompts.md)
- [instruction tuning](../concepts/instruction-tuning.md)
- [few-shot examples](../concepts/few-shot-examples.md)
- [output contracts](../concepts/output-contracts.md)
- [steerability](../concepts/steerability.md)
- [adversarial testing](../concepts/adversarial-testing.md)
- [tool use](../concepts/tool-use.md)
- [constitutional AI](../concepts/constitutional-ai.md)

