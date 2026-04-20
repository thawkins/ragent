---
title: "GeminiClient"
entity_type: "technology"
type: entity
generated: "2026-04-19T15:32:39.530438183+00:00"
---

# GeminiClient

**Type:** technology

### From: gemini

The GeminiClient struct represents the concrete HTTP client implementation for communicating with Google's Generative Language API. This pub(crate) visibility struct encapsulates all network-level concerns, including HTTP connection pooling, request serialization, response streaming, and error handling. The client stores configuration state including the API key for authentication, the base URL for API endpoints, and a `reqwest::Client` instance optimized for streaming responses through the `create_streaming_http_client` helper.

The client's architecture centers on the `chat` method, which implements the `LlmClient` trait's asynchronous interface. This method constructs the API request URL using the v1beta streaming endpoint format, serializes the `ChatRequest` into Google's proprietary JSON structure via `build_request_body`, and initiates an HTTP POST request. The streaming implementation leverages Rust's asynchronous stream primitives, returning a pinned boxed stream of `StreamEvent` values that can be consumed incrementally by callers.

Response processing employs the `async_stream::stream!` macro to generate an asynchronous iterator that yields events as they become available. The implementation handles Google's unique streaming format where JSON objects are delimited by newlines (NDJSON) rather than traditional SSE format. The parser maintains a string buffer for incomplete chunks, extracts complete JSON objects, and transforms them into semantic events including `TextDelta` for incremental text generation, `ToolCallStart`/`ToolCallDelta`/`ToolCallEnd` for function invocations, `Usage` for token consumption metrics, and `Finish` for completion status. Special handling exists for pending tool calls, which are buffered until a `finishReason` is encountered to ensure atomic emission of complete function call specifications.

## Diagram

```mermaid
flowchart TD
    subgraph Request["Request Construction"]
        A[ChatRequest] --> B[build_request_body]
        B --> C[Convert roles: assistant→model]
        B --> D[Handle system instruction]
        B --> E[Process content parts]
        B --> F[Add generation config]
        B --> G[Format tools as functionDeclarations]
    end
    
    subgraph HTTP["HTTP Communication"]
        H[POST /v1beta/models/{model}:streamGenerateContent] --> I[reqwest Client]
        I --> J[Streaming Response]
    end
    
    subgraph Streaming["Stream Processing"]
        K[bytes_stream] --> L[NDJSON Parser]
        L --> M{Event Type}
        M -->|candidates| N[Extract text/function calls]
        M -->|usageMetadata| O[Yield Usage event]
        M -->|finishReason| P[Yield Finish event]
        N --> Q[Buffer tool calls]
        P --> R[Emit pending tool calls]
    end
    
    G --> H
    Request --> HTTP
    HTTP --> Streaming
```

## External Resources

- [Reqwest HTTP client library documentation](https://docs.rs/reqwest/latest/reqwest/) - Reqwest HTTP client library documentation
- [Async-stream crate for generating async iterators](https://docs.rs/async-stream/latest/async_stream/) - Async-stream crate for generating async iterators
- [Google Gemini REST API reference (v1beta)](https://ai.google.dev/api/rest/v1beta/models) - Google Gemini REST API reference (v1beta)

## Sources

- [gemini](../sources/gemini.md)
