---
title: "Async Job Execution Patterns"
type: concept
generated: "2026-04-19T21:00:47.687238910+00:00"
---

# Async Job Execution Patterns

### From: coordinator

The Coordinator implements three distinct asynchronous job execution patterns, each optimized for different latency, reliability, and consistency requirements. These patterns demonstrate sophisticated understanding of distributed systems trade-offs and Rust's async/await capabilities for composable concurrency.

The synchronous aggregation pattern (`start_job_sync`) implements a scatter-gather approach: the Coordinator fans out requests to all matched agents concurrently using Tokio's `spawn`, awaits all completions, then applies aggregation logic. This pattern maximizes throughput for parallelizable workloads where complete information is required, such as distributed map-reduce or ensemble model predictions. The implementation handles partial failure explicitly—individual agent timeouts don't fail the entire job, but complete absence of successful responses triggers error conditions.

The first-success pattern (`start_job_first_success`) implements failover chaining with early termination. Agents are attempted sequentially in deterministic order, with successful non-error responses returned immediately. This pattern optimizes for latency in redundant systems where agents are functionally equivalent, such as replicated services or hot-standby configurations. The pragmatic success detection (checking for "error:" prefix) acknowledges the reality of string-based agent protocols while suggesting evolution toward structured Result types.

The asynchronous event-driven pattern (`start_job_async`) decouples job submission from completion, enabling fire-and-forget workflows and long-running computations. Jobs execute in spawned Tokio tasks, with progress exposed through broadcast channels. This supports reactive architectures where clients subscribe to event streams rather than blocking, and enables the Coordinator to handle thousands of concurrent jobs without thread exhaustion. The pattern uses DashMap for shared state and Tokio's broadcast for multi-consumer event distribution.

## Diagram

```mermaid
sequenceDiagram
    actor Client
    participant Coordinator
    participant Registry
    participant Agent1
    participant Agent2
    participant Agent3
    
    rect rgb(225, 245, 254)
        Note over Client,Agent3: Synchronous Aggregation Pattern
        Client->>Coordinator: start_job_sync
        Coordinator->>Registry: match_agents
        Registry-->>Coordinator: [Agent1, Agent2, Agent3]
        par Concurrent Execution
            Coordinator->>Agent1: send(msg)
            Agent1-->>Coordinator: response1
        and
            Coordinator->>Agent2: send(msg)
            Agent2-->>Coordinator: response2
        and
            Coordinator->>Agent3: send(msg)
            Agent3-->>Coordinator: response3
        end
        Coordinator-->>Client: concatenated or resolved result
    end
    
    rect rgb(255, 243, 224)
        Note over Client,Agent3: First-Success Pattern
        Client->>Coordinator: start_job_first_success
        Coordinator->>Registry: match_agents
        loop Until Success
            Coordinator->>Agent1: send(msg)
            Agent1--xCoordinator: timeout
            Coordinator->>Agent2: send(msg)
            Agent2-->>Coordinator: success response
        end
        Coordinator-->>Client: first successful result
    end
    
    rect rgb(232, 245, 233)
        Note over Client,Agent3: Async Event Pattern
        Client->>Coordinator: start_job_async
        Coordinator-->>Client: job_id immediately
        Coordinator->>Coordinator: spawn background task
        Client->>Coordinator: subscribe_job_events(job_id)
        Coordinator-->>Client: broadcast receiver
        Note right of Coordinator: Events stream asynchronously
    end
```

## External Resources

- [Tokio asynchronous programming tutorial](https://tokio.rs/tokio/tutorial/async) - Tokio asynchronous programming tutorial
- [Patterns of Distributed Systems by Martin Fowler](https://martinfowler.com/articles/patterns-of-distributed-systems/) - Patterns of Distributed Systems by Martin Fowler
- [Rust Future and async/await fundamentals](https://doc.rust-lang.org/std/future/) - Rust Future and async/await fundamentals

## Sources

- [coordinator](../sources/coordinator.md)
