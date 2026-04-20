---
title: "Resource-Limited Result Streaming"
type: concept
generated: "2026-04-19T16:46:51.318319532+00:00"
---

# Resource-Limited Result Streaming

### From: grep

Resource-limited result streaming is an architectural pattern for handling potentially unbounded data production by imposing explicit bounds on resource consumption and providing clear signals when limits are reached. In search contexts, this prevents unbounded result sets from overwhelming downstream consumers, whether those are network buffers, UI rendering pipelines, or AI context windows with finite token limits. `GrepTool` implements this through the `MAX_RESULTS` constant and associated `truncated` flag, creating a clear contract with callers about maximum output size and whether the complete result set was returned or curtailed.

The implementation demonstrates careful attention to limit enforcement throughout the search lifecycle. The limit check appears in two critical locations: within the main traversal loop (preventing unnecessary file walking once the limit is reached) and within the `CollectSink::matched` callback (preventing excess results from individual large files). Both locations use atomic-style checking against the shared `results` vector length, with early termination via `break` in the traversal and `return Ok(false)` from the sink (which signals the searcher to stop processing the current file). The `truncated` boolean is set when limits are exceeded, enabling accurate reporting in the final output summary.

The pattern extends to user-configurable limits through the `max_results` parameter, which is clamped to `MAX_RESULTS` using `.min(MAX_RESULTS)`. This defense-in-depth approach prevents malicious or erroneous callers from requesting excessive resources while still allowing reduced limits for constrained use cases. The metadata JSON included in successful results exposes `truncated` as a machine-readable signal, enabling programmatic consumers to detect incomplete results and potentially refine their search. This design anticipates integration scenarios where an AI agent might need to iteratively search with adjusted patterns if initial results are truncated, rather than receiving an opaque error. The combination of hard upper bounds, user-tunable limits, and explicit truncation signaling exemplifies robust resource management in production systems.

## External Resources

- [Sled database documentation on systematic testing with resource limits](https://sled.rs/simulation.html) - Sled database documentation on systematic testing with resource limits
- [Luca Palmieri's guide to building scalable Rust services with rate limiting](https://www.lpalmieri.com/posts/2021-03-07-scalable-robust-rust-web-services/) - Luca Palmieri's guide to building scalable Rust services with rate limiting

## Sources

- [grep](../sources/grep.md)
