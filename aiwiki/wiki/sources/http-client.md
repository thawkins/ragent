---
title: "Ragent-Core HTTP Client Module for LLM Provider Communication"
source: "http_client"
type: source
tags: [rust, http-client, llm, retry-logic, exponential-backoff, reqwest, tokio, async, connection-pooling, http2, streaming, error-handling, ragent-core]
generated: "2026-04-19T15:35:14.870591543+00:00"
---

# Ragent-Core HTTP Client Module for LLM Provider Communication

This document presents the `http_client.rs` module from the `ragent-core` crate, a Rust library designed to provide robust HTTP client configuration for Large Language Model (LLM) provider communication. The module addresses specific challenges inherent to LLM API interactions, including long-running streaming responses, HTTP/2 race conditions, and transient network failures. It implements three primary public functions: `create_http_client()` for standard requests with comprehensive timeout configuration, `create_streaming_http_client()` for streaming responses that require indefinite execution without global timeouts, and `execute_with_retry()` with exponential backoff for resilient request handling.

The module demonstrates sophisticated error handling patterns, distinguishing between retryable errors (connection failures, timeouts, HTTP/2 protocol errors, and 5xx server responses) and non-retryable errors (4xx client errors). The retry mechanism implements exponential backoff with configurable maximum attempts, starting at 500ms and doubling with each attempt up to a maximum delay. Configuration constants define connection pool limits (8 maximum idle connections per host), connection timeouts (30 seconds), and request timeouts (120 seconds), with TCP keep-alive enabled at 60-second intervals to maintain long-lived connections. This architecture is specifically tailored for sub-agent execution scenarios where multiple concurrent LLM requests must be managed efficiently without resource exhaustion or connection pool contention.

## Related

### Entities

- [reqwest](../entities/reqwest.md) — technology
- [tokio](../entities/tokio.md) — technology
- [anyhow](../entities/anyhow.md) — technology
- [ragent-core](../entities/ragent-core.md) — product

