---
title: "Anthropic Claude Provider Implementation for ragent-core"
source: "anthropic"
type: source
tags: [rust, llm, anthropic, claude, api, streaming, sse, ai, provider, async, tool-use, multimodal]
generated: "2026-04-19T15:30:55.694277620+00:00"
---

# Anthropic Claude Provider Implementation for ragent-core

This Rust source file implements a complete provider integration for the Anthropic Claude API within the ragent-core framework. The implementation consists of two primary structures: `AnthropicProvider`, which implements the `Provider` trait for configuration and client instantiation, and `AnthropicClient`, which implements the `LlmClient` trait for actual API communication. The code demonstrates sophisticated handling of modern LLM features including streaming Server-Sent Events (SSE), multi-modal content processing with base64-encoded images, tool use with JSON argument streaming, and extended thinking/reasoning capabilities. The provider supports Claude Sonnet 4 and Claude 3.5 Haiku models with detailed cost and capability metadata, including context windows up to 200,000 tokens. The implementation also includes robust rate limit parsing from Anthropic-specific response headers and comprehensive error handling throughout the request lifecycle.

The streaming chat implementation processes Anthropic's SSE event stream in real-time, yielding appropriate `StreamEvent` variants for different content types including text deltas, reasoning steps, tool call invocations, and usage statistics. The code handles complex message construction for the Anthropic Messages API, converting the framework's internal `ChatRequest` format into Anthropic's expected JSON structure with proper handling for system prompts, temperature, top-p sampling, and tool definitions. Notable features include automatic MIME type extraction from data URIs for image processing, support for both enabled and disabled thinking modes with configurable token budgets, and careful tracking of partial JSON tool arguments across multiple stream events. The implementation represents a production-ready integration that abstracts the complexities of Anthropic's API behind a clean, async streaming interface compatible with the broader ragent ecosystem.

## Related

### Entities

- [Anthropic](../entities/anthropic.md) — organization
- [Claude Sonnet 4](../entities/claude-sonnet-4.md) — product
- [Anthropic Messages API](../entities/anthropic-messages-api.md) — technology

### Concepts

- [Server-Sent Events (SSE) Streaming](../concepts/server-sent-events-sse-streaming.md)
- [Data URI Handling for Multimodal Inputs](../concepts/data-uri-handling-for-multimodal-inputs.md)
- [Streaming Tool Use with JSON Accumulation](../concepts/streaming-tool-use-with-json-accumulation.md)
- [Extended Thinking and Reasoning Control](../concepts/extended-thinking-and-reasoning-control.md)
- [Async Stream Abstraction in Rust](../concepts/async-stream-abstraction-in-rust.md)

