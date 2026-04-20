---
title: "ChatContent"
entity_type: "technology"
type: entity
generated: "2026-04-19T15:13:13.811197381+00:00"
---

# ChatContent

**Type:** technology

### From: mod

The `ChatContent` enum represents the polymorphic payload of a chat message, solving the fundamental challenge that LLM messages may contain either simple text or complex structured content. This enum uses `#[serde(untagged)]` serialization, meaning Serde attempts variants in order without explicit type discriminators—first `Text(String)`, then `Parts(Vec<ContentPart>)`. The untagged approach maximizes compatibility with APIs that expect either simple string content or complex arrays, though it requires careful variant ordering since deserialization ambiguity could cause misinterpretation.

The `Text(String)` variant handles the common case of plain text messages, present in virtually all LLM conversations. The `Parts(Vec<ContentPart>)` variant enables sophisticated multimodal and tool-use scenarios where a single message contains interleaved content types—text explaining reasoning, a tool invocation request, an image for analysis, and a tool result. This structure directly mirrors emerging standards in LLM APIs: OpenAI's chat completions with vision capabilities, Anthropic's content blocks, and similar patterns across the industry. The vector of parts preserves order, critical for maintaining logical flow in complex interactions.

The design anticipates future content type expansion without breaking changes. New `ContentPart` variants automatically extend `ChatContent` capabilities, while the `untagged` representation maintains backward compatibility with simple text consumers. The `Clone` derive enables message duplication for conversation branching or retry logic, while `Debug` supports development inspection. This enum exemplifies Rust's enum modeling power—capturing mutually exclusive alternatives with different data payloads in a type-safe, zero-cost abstraction.

## External Resources

- [Serde untagged enum representation documentation](https://serde.rs/enum-representations.html#untagged) - Serde untagged enum representation documentation
- [OpenAI vision capabilities with content arrays](https://platform.openai.com/docs/guides/vision) - OpenAI vision capabilities with content arrays

## Sources

- [mod](../sources/mod.md)
