---
title: "tokio::sync::Notify"
entity_type: "technology"
type: entity
generated: "2026-04-19T21:12:06.196558716+00:00"
---

# tokio::sync::Notify

**Type:** technology

### From: mailbox

`tokio::sync::Notify` is a synchronization primitive from the Tokio async runtime that enables single-producer, multi-consumer notification patterns without carrying data. It serves as the backbone for the mailbox system's real-time delivery optimization, bridging the gap between synchronous file I/O and asynchronous agent poll loops. Unlike channels which buffer messages, `Notify` is purely a signaling mechanism—one `notify_one()` call wakes exactly one waiter, making it ideal for triggering processing without duplicating message delivery responsibility.

In the mailbox architecture, `Notify` handles are registered per-agent through `register_notifier`, stored in a global `OnceLock<RwLock<HashMap<...>>>`, and signaled via `signal_notifier` after successful `push` operations. This pattern solves a classic async coordination problem: how to make file polling responsive without busy-waiting. The alternative—pure periodic polling—introduces latency (messages wait up to the poll interval) and wastes CPU (checking unchanged files). The `Notify` integration enables immediate wake-up while retaining the file system as the source of truth for durability.

The choice of `Arc<Notify>` in the registry enables shared ownership across the async runtime and mailbox operations, with `tokio::sync`'s implementation providing memory-efficient waiter queues using intrusive linked lists. The `notify_one()` method specifically (versus `notify_waiters()`) prevents thundering herd problems when multiple tasks might be interested in the same agent's mailbox, ensuring orderly processing. This integration demonstrates sophisticated async Rust patterns: combining blocking file operations (inside the methods) with async signaling (the notification), bridged through the global registry pattern.

## Sources

- [mailbox](../sources/mailbox.md)
