---
title: "Server-Sent Events (SSE) Streaming"
type: concept
generated: "2026-04-19T15:30:55.696050176+00:00"
---

# Server-Sent Events (SSE) Streaming

### From: anthropic

Server-Sent Events (SSE) is a web standard that enables servers to push real-time data to clients over a single HTTP connection, implemented in this code to stream Claude's responses incrementally. Unlike WebSockets which provide bidirectional communication, SSE is unidirectional (server to client) and operates over standard HTTP, making it easier to integrate with existing infrastructure including load balancers, CDNs, and authentication systems. The protocol uses the `text/event-stream` MIME type and a simple text-based format where each event consists of an optional event type line (`event: <name>`), a data line (`data: <json>`), and a blank line separator. This implementation demonstrates production-quality SSE handling with careful buffer management, UTF-8 decoding, and event type dispatching.

The streaming approach provides significant user experience and efficiency benefits for LLM applications. Rather than waiting for the complete response, users can see tokens appear in real-time, creating the perception of a conversational interaction. The implementation in this Rust code shows how to transform the low-level byte stream into higher-level `StreamEvent` abstractions, handling the complexity of partial JSON parsing, event boundary detection, and error recovery. The code maintains internal state including a line buffer, current event type tracking, and tool call argument accumulation across multiple events. This stateful processing is necessary because SSE provides no guarantees about event boundaries aligning with logical content units—an event may span multiple chunks, or multiple events may arrive in a single chunk.

Advanced SSE usage in this implementation includes handling multiple distinct event types (`message_start`, `content_block_start`, `content_block_delta`, etc.) that map to different stages of the model's generation process. The code demonstrates proper cleanup on stream termination, including handling the `[DONE]` sentinel that signals completion, and error propagation through the async stream. Rate limit events are injected at the stream's beginning when header information is available, showing how SSE can be combined with HTTP metadata for rich application feedback. The use of `async_stream::stream!` macro demonstrates idiomatic Rust patterns for creating asynchronous streams with internal mutable state, while `futures::pin_mut!` ensures the stream is properly pinned for safe async iteration. This pattern of SSE-based streaming has become the de facto standard for LLM APIs, offering an optimal balance of latency, complexity, and compatibility.

## External Resources

- [MDN documentation on Server-Sent Events](https://developer.mozilla.org/en-US/docs/Web/API/Server-sent_events) - MDN documentation on Server-Sent Events
- [WHATWG HTML Living Standard - SSE specification](https://html.spec.whatwg.org/multipage/server-sent-events.html) - WHATWG HTML Living Standard - SSE specification

## Sources

- [anthropic](../sources/anthropic.md)
