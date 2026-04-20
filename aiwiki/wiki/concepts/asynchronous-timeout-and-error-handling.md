---
title: "Asynchronous Timeout and Error Handling"
type: concept
generated: "2026-04-19T20:52:42.206787570+00:00"
---

# Asynchronous Timeout and Error Handling

### From: transport

The transport layer implements comprehensive timeout and error handling patterns essential for reliable distributed system operation. The `HttpRouter::send` method demonstrates layered timeout application using `tokio::time::timeout` wrapped around the HTTP request future, preventing indefinite blocking when remote agents become unresponsive due to network partitions, garbage collection pauses, or process failures. This outer timeout layer complements `reqwest`'s internal connection and read timeouts, creating defense in depth against various failure modes. The timeout duration is configurable per router instance through the `request_timeout` field, allowing operators to tune latency bounds based on observed agent behavior and service level objectives.

Error handling follows Rust's `Result`-based approach with context enrichment through the `anyhow` crate, which enables ergonomic error propagation while preserving diagnostic information. The implementation distinguishes multiple failure categories: registration lookup failures (agent not found), transport timeouts (deadline exceeded), HTTP protocol errors (connection refused, DNS failures), non-success status codes (4xx/5xx responses), and deserialization failures (malformed JSON). Each category receives specific error messages incorporating the target agent identifier, aiding operational troubleshooting in distributed deployments where logs from multiple components must be correlated.

The error propagation strategy in `RouterComposite` implements chain-of-responsibility semantics where router-specific errors are captured but not immediately returned, allowing fallback routers to attempt delivery. Only when all routers exhaust does the composite return an error, specifically the last encountered error which typically represents the most diagnostic information (often from the final fallback router). This pattern requires careful consideration of error masking—transient network failures from an HTTP router should not prevent retry, while persistent configuration errors might warrant immediate escalation. The current implementation's preservation of only the last error loses information about earlier failures, suggesting potential enhancements to aggregate error contexts or implement structured error types distinguishing retriable from permanent failures.

## External Resources

- [Tokio timeout documentation](https://docs.rs/tokio/latest/tokio/time/fn.timeout.html) - Tokio timeout documentation
- [Anyhow error handling library](https://docs.rs/anyhow/latest/anyhow/) - Anyhow error handling library
- [Google SRE: Handling overload and graceful degradation](https://sre.google/sre-book/handling-overload/) - Google SRE: Handling overload and graceful degradation

## Sources

- [transport](../sources/transport.md)
