---
title: "RagentAgentPayload"
entity_type: "technology"
type: entity
generated: "2026-04-19T15:00:25.376897806+00:00"
---

# RagentAgentPayload

**Type:** technology

### From: custom

RagentAgentPayload contains the runtime-specific configuration for ragent agents, deserialized from the OASF module payload. This structure bridges the gap between the standardized OASF envelope format and ragent's specific execution requirements. The system_prompt field serves as the core behavioral instruction, with validation ensuring it is non-empty and within length limits. The mode field determines agent participation in conversation routing: Primary agents receive all user messages directly, Subagent agents only respond when explicitly invoked by name, and All agents participate in both contexts. Model specification uses a provider-prefixed format enabling multi-provider abstraction, while sampling parameters temperature and top-p control response creativity and diversity. The max_steps parameter prevents infinite loops in agentic execution by capping iteration counts. Security policies express through permissions with pattern matching, supporting allow, deny, and ask actions for fine-grained tool and resource access control. The memory field enables persistent context across sessions at user or project granularity. The options field preserves forward compatibility, accepting arbitrary provider-specific configuration without schema changes.

## External Resources

- [OpenAI API documentation on temperature sampling parameter](https://platform.openai.com/docs/api-reference/chat/create#chat-create-temperature) - OpenAI API documentation on temperature sampling parameter
- [Hugging Face explanation of nucleus sampling (top-p) and temperature](https://huggingface.co/blog/how-to-generate) - Hugging Face explanation of nucleus sampling (top-p) and temperature

## Sources

- [custom](../sources/custom.md)
