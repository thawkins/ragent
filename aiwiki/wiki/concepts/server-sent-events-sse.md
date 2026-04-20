---
title: "Server-Sent Events (SSE)"
type: concept
generated: "2026-04-19T15:49:04.731218499+00:00"
---

# Server-Sent Events (SSE)

### From: openai

Server-Sent Events represent a web standard enabling servers to push real-time data to clients over a single HTTP connection, distinct from WebSockets in being unidirectional (server-to-client) and operating over standard HTTP. In the context of LLM APIs like OpenAI's, SSE provides the infrastructure for streaming responses, allowing tokens to be delivered incrementally as they are generated rather than waiting for complete responses. This dramatically improves perceived latency for end users, as they see text appearing character-by-character rather than experiencing a long delay followed by a complete dump.

The implementation in this Rust code demonstrates practical SSE handling at the protocol level. The client processes `bytes_stream` from the HTTP response, accumulating chunks into a buffer that is split on newline characters to extract individual SSE lines. Each line is expected to begin with the `data: ` prefix per the SSE specification, followed by either JSON content or the `[DONE]` sentinel indicating stream completion. The parsing must handle arbitrary chunk boundaries—network packets may split UTF-8 characters or SSE lines across multiple chunks—requiring careful buffer management and the `String::from_utf8_lossy` conversion for resilience.

Beyond latency improvement, SSE streaming enables sophisticated interactive behaviors like token-by-token processing, real-time usage statistics, and mid-stream tool call detection. The `async_stream::stream!` macro generates a `Stream` implementation that yields `StreamEvent` variants, translating the low-level SSE protocol into semantically meaningful events for application consumption. This abstraction allows downstream code to work with structured events (`TextDelta`, `ToolCallStart`, `Usage`) without concerning themselves with HTTP chunking, JSON parsing, or state machine management required by the SSE format.

## External Resources

- [HTML Living Standard - Server-sent events](https://html.spec.whatwg.org/multipage/server-sent-events.html) - HTML Living Standard - Server-sent events
- [Using server-sent events - Web APIs](https://developer.mozilla.org/en-US/docs/Web/API/Server-sent_events/Using_server-sent_events) - Using server-sent events - Web APIs

## Sources

- [openai](../sources/openai.md)
