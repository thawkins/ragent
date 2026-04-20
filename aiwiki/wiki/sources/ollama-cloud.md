---
title: "Ollama Cloud Provider Implementation for ragent-core"
source: "ollama_cloud"
type: source
tags: [rust, ollama, llm, provider, streaming, tool-calling, vision, async, api-client, ragent-core]
generated: "2026-04-19T15:46:48.945276402+00:00"
---

# Ollama Cloud Provider Implementation for ragent-core

This source file implements a complete Rust provider for integrating with Ollama Cloud, a managed service for running large language models. The implementation provides full support for chat completions, streaming responses, tool calling, vision capabilities, and model discovery through Ollama's native REST API endpoints. The code demonstrates sophisticated handling of the Ollama-specific protocol, including custom message formatting for tool use where Ollama uses `name` instead of `tool_call_id`, and special handling for base64-encoded images without data-URL prefixes. The provider implements the `Provider` and `LlmClient` traits from the ragent-core crate, enabling seamless integration with the broader agent framework while abstracting away Ollama-specific implementation details.

The architecture separates concerns between the `OllamaCloudProvider` struct, which handles configuration and model discovery, and the `OllamaCloudClient` struct, which manages actual HTTP communication with the Ollama Cloud API. Model discovery uses the `/api/tags` endpoint to list available models, while detailed model information including context window sizes and capabilities is fetched via the `/api/show` endpoint. The implementation includes intelligent context window estimation based on parameter sizes when explicit information isn't available from the API, with fallback logic that maps common model sizes (7B, 30B, 70B+) to appropriate context lengths (8K, 32K, 65K, 128K tokens respectively).

The streaming chat implementation is particularly robust, handling chunked HTTP responses with proper timeout management, JSON parsing of Server-Sent Events (SSE), and support for tool calling workflows. The code includes comprehensive logging at multiple levels for debugging, with special handling to log full request bodies when tools are present or when debug logging is enabled. The implementation also supports custom base URLs for self-hosted Ollama instances that expose a Cloud-compatible API, making it versatile for both managed cloud and on-premises deployments.

## Related

### Entities

- [Ollama Cloud](../entities/ollama-cloud.md) — product
- [OllamaCloudProvider](../entities/ollamacloudprovider.md) — technology
- [OllamaCloudClient](../entities/ollamacloudclient.md) — technology
- [ragent-core](../entities/ragent-core.md) — technology

