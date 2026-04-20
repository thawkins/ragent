---
title: "Streaming LLM Responses"
type: concept
generated: "2026-04-19T15:13:13.811529584+00:00"
---

# Streaming LLM Responses

### From: mod

Streaming responses represent a paradigm shift in LLM application architecture, transforming blocking request-response cycles into incremental, event-driven data flows. Traditional API calls wait for complete generation, potentially 10-30 seconds for long responses, creating poor user experiences and timeout risks. Streaming instead yields tokens as they're generated, typically 10-50 milliseconds between events, enabling immediate display, progressive rendering, and responsive cancellation. This module's `StreamEvent` enum and `LlmClient::chat` return type embody this pattern through Rust's `futures::Stream` abstraction.

The implementation leverages HTTP/2 server-sent events or chunked transfer encoding, depending on provider implementation. Each provider normalizes their native format—OpenAI's SSE `data: {...}` lines, Anthropic's similar event streams—into the unified `StreamEvent` types. This normalization handles provider differences: some send content as raw tokens, others as word fragments; reasoning blocks appear in specific formats; tool calls may arrive as complete JSON or streaming partial arguments requiring accumulation. The streaming architecture enables sophisticated application behaviors: typing indicators, word-by-word display, immediate tool execution upon call completion, and real-time usage tracking.

Error handling in streaming contexts requires special consideration. Network failures may interrupt streams mid-generation; the `Error` variant propagates these. Application logic must handle partial content—rendered text before an error, incomplete tool arguments—through the `Finish` event's reason codes. Backpressure management ensures slow consumers don't overwhelm memory, with `stream_timeout_secs` preventing indefinite blocking. The pattern fundamentally changes application structure from imperative `let response = await completion` to reactive `while let Some(event) = stream.next().await { handle(event) }`.

## External Resources

- [MDN documentation on Server-Sent Events](https://developer.mozilla.org/en-US/docs/Web/API/Server-sent_events) - MDN documentation on Server-Sent Events
- [Rust futures::Stream trait documentation](https://docs.rs/futures/latest/futures/stream/trait.Stream.html) - Rust futures::Stream trait documentation
- [Tokio streams tutorial](https://tokio.rs/tokio/tutorial/streams) - Tokio streams tutorial

## Sources

- [mod](../sources/mod.md)
