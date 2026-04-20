---
title: "Ollama Provider Implementation for ragent-core"
source: "ollama"
type: source
tags: [rust, ollama, llm, provider, streaming, openai-compatible, api-client, async, sse, tool-calling, ragent]
generated: "2026-04-19T15:44:02.747750298+00:00"
---

# Ollama Provider Implementation for ragent-core

This Rust source file implements the `OllamaProvider` and `OllamaClient` structs for the ragent-core crate, enabling integration with Ollama servers for large language model inference. The implementation provides a complete provider interface that connects to local or remote Ollama instances using their OpenAI-compatible `/v1/chat/completions` API endpoint, with additional support for Ollama-native endpoints like `/api/tags` for model discovery.

The architecture consists of two primary components: `OllamaProvider` which implements the `Provider` trait for configuration and client factory purposes, and `OllamaClient` which implements the `LlmClient` trait for actual chat completion requests. The provider supports dynamic model discovery through the `/api/tags` endpoint, allowing runtime enumeration of available models rather than relying on static configuration. It handles streaming Server-Sent Events (SSE) for real-time token generation, tool call invocation and result handling, and automatic context window estimation based on model parameter sizes. The implementation also includes sophisticated request body construction that bridges between the crate's internal message format and the OpenAI-compatible format expected by Ollama, including special handling for both `tool_call_id` (OpenAI compatibility) and `tool_name` (native Ollama format) fields.

Configuration flexibility is provided through environment variables (`OLLAMA_HOST`) and programmatic URL specification. The code demonstrates robust error handling with detailed logging, timeout management for both initial connections and streaming responses, and graceful degradation for optional features. The provider is designed to work without API keys for local development while supporting Bearer token authentication for remote deployments, making it suitable for both development workflows and production deployments behind authenticated proxies.

## Related

### Entities

- [Ollama](../entities/ollama.md) — technology
- [OllamaProvider](../entities/ollamaprovider.md) — technology
- [OllamaClient](../entities/ollamaclient.md) — technology
- [ragent-core](../entities/ragent-core.md) — product

### Concepts

- [Provider Pattern in LLM Abstractions](../concepts/provider-pattern-in-llm-abstractions.md)
- [OpenAI-compatible API](../concepts/openai-compatible-api.md)
- [Server-Sent Events for LLM Streaming](../concepts/server-sent-events-for-llm-streaming.md)
- [Tool Calling in LLMs](../concepts/tool-calling-in-llms.md)
- [Async Streaming in Rust](../concepts/async-streaming-in-rust.md)

