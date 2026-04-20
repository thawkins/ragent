---
title: "Async Concurrency Control"
type: concept
generated: "2026-04-19T15:18:45.474300370+00:00"
---

# Async Concurrency Control

### From: mod

The patterns and primitives used to manage multiple concurrent operations in the MCP client without resource exhaustion or race conditions. The implementation uses a tokio::sync::Semaphore instantiated via std::sync::LazyLock to create a global limit of 8 concurrent MCP server connections, preventing scenarios where opening too many connections would exhaust file descriptors, memory, or process table entries. This semaphore is acquired before spawning any child process, with async awaiting if the limit is reached, naturally implementing backpressure. The Arc<RwLock<HashMap>> pattern for storing active connections enables shared, mutable state across async tasks: the Arc provides reference counting for multiple owners, while the RwLock allows concurrent reads (tool listings) with exclusive writes (connection changes). These choices reflect Rust's ownership model applied to async code, where Send and Sync traits ensure thread safety at compile time.

The concurrency architecture separates connection metadata (McpServer in a Vec) from active connection handles (McpConnection in the RwLock-protected HashMap), optimizing for different access patterns. Server listing and tool enumeration read from the Vec without synchronization overhead, while connection operations acquire the RwLock only when necessary. The use of async-aware primitives is crucial: std::sync::Mutex would block the executor thread, defeating the purpose of async I/O, while tokio::sync::Mutex and RwLock yield to the runtime when contended. The implementation also shows proper cleanup patterns with disconnect_all and individual disconnect methods that remove entries from both data structures, preventing resource leaks.

This concurrency model scales with the number of MCP servers rather than blocking thread pool size, enabling efficient handling of many connections on few threads. The timeout mechanism (TOOL_CALL_TIMEOUT_SECS) adds another layer of control, preventing indefinite waits on unresponsive servers. Error isolation ensures one server's failure doesn't affect others—each connection attempt, tool refresh, and invocation is independently error-handled. These patterns are transferable to other Rust async applications managing external resources, representing current best practices for the ecosystem. The code demonstrates that safe concurrency in Rust doesn't require garbage collection or runtime reflection, but rather careful type design and appropriate synchronization primitive selection.

## Diagram

```mermaid
flowchart TD
    subgraph ConcurrencyModel["Async Concurrency Control"]
        direction TB
        
        subgraph GlobalLimit["Global Connection Limit"]
            lazylock["LazyLock<Semaphore>"]
            semaphore["Semaphore: max 8 permits"]
            lazylock --> semaphore
        end
        
        subgraph ConnectionState["Connection State Management"]
            arc["Arc<RwLock<HashMap>>"]
            rwlock["RwLock: protects HashMap"]
            hashmap["HashMap<String, McpConnection>"]
            arc --> rwlock
            rwlock -->|read lock| hashmap
            rwlock -->|write lock| hashmap
        end
        
        subgraph OperationsFlow["Operation Flow"]
            connect["connect(server_id)"]
            acquire["acquire().await"]
            spawn["spawn process"]
            insert["write lock: insert connection"]
            list["list_tools()"]
            read["read lock: iterate connections"]
            call["call_tool()"]
            timeout["timeout(120s)"]
        end
        
        connect --> acquire
        acquire -->|permit acquired| spawn
        spawn --> insert
        list --> read
        call --> read
        read --> timeout
    end
    
    subgraph ThreadSafety["Thread Safety Guarantees"]
        send["Send: safe to move between threads"]
        sync["Sync: safe to share between threads"]
        static["'static lifetime for spawned tasks"]
    end
    
    ConcurrencyModel --> ThreadSafety
```

## External Resources

- [Tokio shared state tutorial - patterns used in McpClient](https://tokio.rs/tokio/tutorial/shared-state) - Tokio shared state tutorial - patterns used in McpClient
- [Tokio Semaphore documentation for limiting concurrency](https://docs.rs/tokio/latest/tokio/sync/struct.Semaphore.html) - Tokio Semaphore documentation for limiting concurrency
- [Async Rust patterns and trait design](https://rust-lang.github.io/async-book/07_workarounds/03_async_in_traits.html) - Async Rust patterns and trait design

## Sources

- [mod](../sources/mod.md)
