Orchestration Messaging Recommendation

Purpose

This short document records the recommended in-process messaging pattern for the Multi-Agent Orchestration MVP (F6) and provides rationale and tradeoffs.

Recommendation

Use an actor-style inbox per in-process agent implemented with Tokio mpsc channels (one Sender held by the Registry/Router and a background task owning the Receiver for the agent).

How it works (MVP)

- When an agent is registered with an in-process responder, the registry creates a bounded mpsc channel (mailbox) and spawns a background task that reads OrchestrationRequest messages from the mailbox.
- Each request contains a one-shot reply channel for the agent to send its response.
- The Router sends messages to the agent mailbox and awaits the reply with a configured timeout (tokio::time::timeout).

Rationale

- Backpressure & bounded queues: Using bounded mpsc channels prevents unbounded memory growth if many requests are generated faster than agents can handle them.
- Actor semantics: Per-agent inboxes neatly model independent agents, simplify concurrency reasoning, and allow sequential processing per-agent where required.
- Testability: In-process mailboxes are easy to instantiate and test deterministically in-process (no network setup required).
- Pluggability: The Router abstraction only needs to implement send/receive semantics; the inbox approach can be swapped for a remote transport adapter (gRPC, WebSocket) that provides the same request/response contract.

Tradeoffs

- Slight latency overhead: The mailbox and one-shot reply add a small overhead compared to a direct function call. For orchestration workloads this is acceptable given the concurrency and isolation benefits.
- Single-threaded agent loops: If an agent requires parallelism internally it can spawn internal tasks; the mailbox guarantees a single serialized intake stream which is often desirable for stateful agents.

Alternatives considered

- Direct async function call (Responder closure): Simpler but lacks flow-control and mailbox semantics; harder to extend to remote transports.
- Work-stealing / shared work queue: Higher throughput for lots of short tasks, but complicates per-agent semantics and makes attribution and tracing harder.

Conclusion

Actor-style inboxes implemented using Tokio mpsc + oneshot reply channels provide the best mix of simplicity, safety, and extensibility for the MVP. The router abstraction ensures we can replace the in-process mailbox with distributed transports later without changing Coordinator logic.
