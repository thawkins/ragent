---
title: "OpenAI Provider Implementation for Rust LLM Client"
source: "openai"
type: source
tags: [rust, openai, llm, streaming-api, sse, tool-calling, async, api-client, chat-completions, gpt-4o]
generated: "2026-04-19T15:49:04.729585666+00:00"
---

# OpenAI Provider Implementation for Rust LLM Client

This source code implements a complete OpenAI provider for an asynchronous Rust LLM (Large Language Model) client framework. The implementation provides seamless integration with OpenAI's Chat Completions API, supporting streaming Server-Sent Events (SSE), tool calling capabilities, vision inputs, and comprehensive rate limiting. The module defines two primary structures: `OpenAiProvider`, which implements the trait-based provider interface for dependency injection and configuration, and `OpenAiClient`, which handles the actual HTTP communication and response streaming.

The implementation demonstrates sophisticated handling of modern LLM features including multimodal content (text and images), function calling with streaming tool use, and reasoning control. The `build_request_body` method performs complex transformations between the framework's internal `ChatRequest` format and OpenAI's expected JSON structure, handling system messages, user content with mixed text and image parts, and tool-related messages. Tool calls are particularly carefully managed, with the code tracking tool call IDs and names across multiple streaming chunks to reconstruct complete function invocations from partial data.

Rate limiting is implemented through header parsing that extracts request and token usage percentages from OpenAI's standard `x-ratelimit-*` headers, providing applications with visibility into their API consumption. The streaming response handler uses `async_stream` to yield events incrementally, parsing SSE data lines, extracting text deltas, tool call fragments, and usage statistics. The code supports GPT-4o and GPT-4o Mini models by default, with configurable context windows up to 128,000 tokens and comprehensive capability flags for streaming, vision, and tool use.

## Related

### Entities

- [OpenAiProvider](../entities/openaiprovider.md) — technology
- [OpenAiClient](../entities/openaiclient.md) — technology
- [OpenAI](../entities/openai.md) — organization

### Concepts

- [Server-Sent Events (SSE)](../concepts/server-sent-events-sse.md)
- [Function Calling / Tool Use](../concepts/function-calling--tool-use.md)
- [API Rate Limiting](../concepts/api-rate-limiting.md)
- [Multimodal LLM Content](../concepts/multimodal-llm-content.md)
- [Async Trait Patterns in Rust](../concepts/async-trait-patterns-in-rust.md)

