---
title: "Server-Sent Events for LLM Streaming"
type: concept
generated: "2026-04-19T15:44:02.750972462+00:00"
---

# Server-Sent Events for LLM Streaming

### From: ollama

Server-Sent Events (SSE) is a web standard enabling servers to push real-time updates to clients over HTTP, forming the transport layer for streaming large language model responses token-by-token rather than waiting for complete generation. Unlike WebSockets which provide bidirectional communication, SSE maintains a unidirectional server-to-client stream over a single persistent HTTP connection, making it ideally suited for LLM use cases where the client sends a single request and receives incremental text generation. The protocol uses `text/event-stream` content type with simple framing: each event begins with `data:` followed by JSON payload, and streams terminate with a special marker or connection close.

The implementation complexity in LLM streaming stems from the intersection of network realities and application requirements. Network buffers and proxy configurations may coalesce or split SSE messages arbitrarily, meaning parser implementations must handle partial JSON across chunks and multiple events in single reads. The ragent-core implementation demonstrates robust handling through a buffering approach—raw bytes accumulate in a `String` buffer, with explicit newline scanning to extract complete `data:` lines before attempting JSON parsing. This defensive programming prevents parse failures from transient network conditions. Timeout handling adds another dimension, requiring separate timers for initial response latency and ongoing stream health to detect stalled generations.

Application-layer semantics on top of SSE vary between providers, though a common pattern has emerged. The `data:` payloads typically contain JSON objects with `delta` fields for incremental content, `finish_reason` fields for generation termination signals, and optional `usage` blocks with token counts. Tool calling introduces streaming complexity as function arguments may arrive in fragments across multiple events, requiring client-side accumulation before invocation. The ragent-core code shows sophisticated state management with `HashMap<u64, String>` structures tracking partial tool calls by index, enabling parallel tool invocations to be reconstructed correctly. Error handling must distinguish between transport failures (HTTP status, connection drops) and application errors embedded in SSE payloads, with appropriate `StreamEvent` variants for each case.

## Diagram

```mermaid
flowchart LR
    subgraph Transport["HTTP Connection"]
        direction TB
        A[POST /v1/chat/completions] --> B[Response Headers<br/>Content-Type: text/event-stream]
        B --> C[SSE Stream]
    end
    
    subgraph Framing["Event Framing"]
        direction TB
        D[data: {"choices":[...]}] --> E[\n newline]
        E --> F[data: {"choices":[...]}]
        F --> G[...]
        G --> H[data: [DONE]]
    end
    
    subgraph Processing["Client Processing"]
        direction TB
        I[Raw bytes] --> J[Buffer accumulation]
        J --> K[Split on newlines]
        K --> L[Strip 'data: ' prefix]
        L --> M[Parse JSON]
        M --> N{Event type?}
        N -->|Text| O[Yield TextDelta]
        N -->|Tool| P[Accumulate in HashMap]
        N -->|Usage| Q[Yield Usage stats]
        N -->|Done| R[End stream]
    end
    
    C --> I
    H -.->|terminator| R
```

## External Resources

- [MDN documentation on Server-Sent Events](https://developer.mozilla.org/en-US/docs/Web/API/Server-sent_events) - MDN documentation on Server-Sent Events
- [WHATWG HTML Living Standard - Server-sent events](https://html.spec.whatwg.org/multipage/server-sent-events.html) - WHATWG HTML Living Standard - Server-sent events

## Related

- [OpenAI-compatible API](openai-compatible-api.md)
- [Async Streaming in Rust](async-streaming-in-rust.md)

## Sources

- [ollama](../sources/ollama.md)
