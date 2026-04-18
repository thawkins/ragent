---
title: "Practical Guidance for Writing Effective System Prompts"
source: "SYSTEM_PROMPT"
type: source
tags: [system prompts, prompt engineering, LLM, AI safety, agent systems, tool use, instruction tuning, best practices, OpenAI, Anthropic, Google Vertex AI, Azure OpenAI, few-shot learning, output formatting, adversarial testing]
generated: "2026-04-18T14:50:40.938449765+00:00"
---

# Practical Guidance for Writing Effective System Prompts

This document provides comprehensive best practices for designing system prompts in large language model (LLM) applications, synthesizing guidance from major providers including OpenAI, Anthropic, Google/Vertex AI, and Microsoft Azure. It establishes system prompts as critical "ground truth" directives that shape model behavior across conversations, while acknowledging their limitations in guaranteeing compliance against adversarial inputs or hallucinations. The core recommendations emphasize practical, testable approaches: establishing clear role definitions, scoping behavior with explicit prohibitions, defining output contracts with exact formats, providing fallback rules for ambiguity, and using few-shot examples for style guidance.

The document details actionable patterns including the "Role + Rules + Output Contract + Examples" template, self-check loops for high-impact actions, and explicit tool boundaries. It addresses important constraints such as token limits, the non-security nature of system prompts, and the tradeoff between steerability and robustness. Drawing on instruction-tuning research (FLAN, T0, Super-NaturalInstructions) and Constitutional AI methods, the guidance supports building agent systems, tool-using assistants, and production chat flows. The document concludes with evaluation metrics and authoritative source links for further implementation.

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
- [Chat Completions API](../entities/chat-completions-api.md) — product
- [Vertex AI](../entities/vertex-ai.md) — product

### Concepts

- [system prompt](../concepts/system-prompt.md)
- [instruction tuning](../concepts/instruction-tuning.md)
- [few-shot prompting](../concepts/few-shot-prompting.md)
- [output contract](../concepts/output-contract.md)
- [steerability](../concepts/steerability.md)
- [self-check loop](../concepts/self-check-loop.md)
- [tool boundaries](../concepts/tool-boundaries.md)
- [adversarial testing](../concepts/adversarial-testing.md)
- [token constraints](../concepts/token-constraints.md)

