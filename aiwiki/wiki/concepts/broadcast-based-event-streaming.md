---
title: "Broadcast-Based Event Streaming"
type: concept
generated: "2026-04-19T21:00:47.688111549+00:00"
---

# Broadcast-Based Event Streaming

### From: coordinator

The Coordinator implements a reactive event streaming system using Tokio's broadcast channels, enabling multiple concurrent consumers to observe job lifecycle events without coupling to the job execution implementation. This pattern supports real-time monitoring, progress tracking, and reactive downstream processing for asynchronous workflows. The architecture uses a bounded channel with capacity 16, providing backpressure while accommodating transient consumer lag.

Event production occurs at key lifecycle points within spawned async tasks: JobStarted when execution begins, SubtaskAssigned/SubtaskCompleted as individual agent interactions proceed, and JobCompleted/JobFailed for terminal states. The use of `let _ =` for send operations acknowledges that broadcast failures (lagged consumers) are acceptable—event streaming is best-effort rather than guaranteed delivery. This matches operational realities where monitoring consumers may restart or fall behind without impacting core job execution.

The subscription API (`subscribe_job_events`) enables late joining: consumers receive a new Receiver that will process events from the current position, not historical events. This is appropriate for live monitoring but requires consumers to call `get_job_result` for final state if joining after completion. The broadcast pattern natively supports multiple concurrent subscribers—useful for fan-out to monitoring dashboards, audit logs, and alerting systems—without requiring the Coordinator to manage multiple per-consumer channels.

The JobEvent enum uses struct variants with named fields, providing self-documenting event payloads and extensibility for future fields without breaking changes. Event ordering is guaranteed per-job (events arrive in generation order), but no ordering exists across jobs. The memory management is clean: when the last Sender (held in JobEntry) drops, receivers receive None, enabling graceful consumer termination.

## External Resources

- [Tokio broadcast channel documentation](https://docs.rs/tokio/latest/tokio/sync/broadcast/) - Tokio broadcast channel documentation
- [Tracing structured logging framework](https://docs.rs/tracing/latest/tracing/) - Tracing structured logging framework
- [CloudEvents specification for event data](https://cloudevents.io/) - CloudEvents specification for event data

## Sources

- [coordinator](../sources/coordinator.md)
