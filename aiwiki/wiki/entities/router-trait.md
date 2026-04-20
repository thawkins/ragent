---
title: "Router Trait"
entity_type: "technology"
type: entity
generated: "2026-04-19T20:58:41.225340550+00:00"
---

# Router Trait

**Type:** technology

### From: router

The `Router` trait defines the fundamental abstraction for request delivery to agents within the `ragent-core` orchestration system. As an async trait marked with `#[async_trait::async_trait]`, it enables asynchronous implementations while maintaining object safety and trait object compatibility. The trait bounds `Send + Sync + 'static` ensure that router implementations are thread-safe and can be shared across task boundaries in multi-threaded async runtimes, essential for high-concurrency agent systems.

The single required method `send` encapsulates the complete request-response lifecycle: accepting a target `agent_id` string and an `OrchestrationMessage`, then returning a `Result<String>` representing either the agent's response payload or an error condition. This design intentionally abstracts away the underlying transport mechanism, allowing the same orchestration logic to work with in-process routers, network-based routers, or hybrid configurations without code changes. The trait's simplicity—single method with clear inputs and outputs—follows the interface segregation principle while providing sufficient flexibility for diverse implementations.

The use of `Result<String>` rather than a custom response type suggests that agent responses are treated as opaque payloads at the routing layer, with interpretation delegated to higher layers. This separation of concerns keeps the router focused on reliable delivery rather than message semantics. The `'static` bound indicates that router implementations should not contain borrowed references, enforcing ownership patterns appropriate for long-lived infrastructure components that may outlive individual request scopes.

## External Resources

- [async-trait crate documentation explaining async trait patterns](https://docs.rs/async-trait/latest/async_trait/) - async-trait crate documentation explaining async trait patterns
- [Async Rust book chapter on async methods in traits](https://rust-lang.github.io/async-book/07_workarounds/04_async_in_traits.html) - Async Rust book chapter on async methods in traits

## Sources

- [router](../sources/router.md)
