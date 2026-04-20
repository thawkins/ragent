---
title: "OllamaClient"
entity_type: "technology"
type: entity
generated: "2026-04-19T15:44:02.749178939+00:00"
---

# OllamaClient

**Type:** technology

### From: ollama

The `OllamaClient` struct implements the `LlmClient` trait and serves as the actual HTTP client for communicating with Ollama servers, handling the complexities of streaming chat completions and protocol translation. This struct maintains the runtime connection state including an optional API key for authenticated remote servers, the normalized base URL, and a reusable `reqwest::Client` instance optimized for streaming responses. The design encapsulates all Ollama-specific protocol knowledge, isolating the rest of the application from endpoint paths, header requirements, and response parsing details.

The client's most sophisticated functionality lies in `build_request_body()`, which translates the crate's internal `ChatRequest` representation into the JSON format expected by Ollama's OpenAI-compatible endpoint. This translation layer handles multiple complexity dimensions: message role mapping (system, user, assistant, tool), content part serialization (text, images, tool calls, tool results), and parameter injection (temperature, top_p, max_tokens). The implementation demonstrates particular care around tool calling, building a mapping from tool use IDs to tool names to satisfy both OpenAI-compatible (`tool_call_id`) and native Ollama (`tool_name`) field requirements. This dual-format support ensures maximum compatibility across Ollama versions and configurations.

Streaming implementation via the `chat()` method showcases production-grade async Rust patterns. The client establishes Server-Sent Event (SSE) connections with configurable timeouts for both initial response and ongoing stream health. The response processing uses `async_stream` to yield events incrementally, parsing SSE data lines and dispatching `StreamEvent` variants including text deltas, tool call fragments, usage statistics, and error conditions. The parser maintains state across chunks using a buffer and tracks tool call accumulation through index-based hash maps, enabling reconstruction of parallel tool invocations. Comprehensive error handling includes timeout detection, HTTP status validation with request body logging for debugging, and graceful stream termination on parse failures.

## Diagram

```mermaid
sequenceDiagram
    participant App as Application
    participant OP as OllamaProvider
    participant OC as OllamaClient
    participant OS as Ollama Server
    
    App->>OP: create_client(api_key, base_url)
    OP->>OC: new with config
    OC-->>App: Box&lt;dyn LlmClient&gt;
    
    App->>OC: chat(ChatRequest)
    OC->>OC: build_request_body()
    Note over OC: Translate messages,<br/>tools, parameters<br/>to OpenAI format
    
    OC->>OS: POST /v1/chat/completions
    Note over OS: SSE streaming response
    
    loop SSE Stream Processing
        OS-->>OC: data: {...}
        OC->>OC: Parse JSON delta
        alt Text content
            OC-->>App: StreamEvent::TextDelta
        else Tool calls
            OC-->>App: StreamEvent::ToolCall
        else Usage stats
            OC-->>App: StreamEvent::Usage
        end
    end
    
    OS-->>OC: data: [DONE]
    OC-->>App: Stream complete
```

## External Resources

- [OpenAPI Specification for understanding API design patterns](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.1.0.md) - OpenAPI Specification for understanding API design patterns

## Sources

- [ollama](../sources/ollama.md)
