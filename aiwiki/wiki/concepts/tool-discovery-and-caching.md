---
title: "Tool Discovery and Caching"
type: concept
generated: "2026-04-19T15:18:45.474756699+00:00"
---

# Tool Discovery and Caching

### From: mod

The architectural pattern for discovering available capabilities from MCP servers and maintaining local copies to avoid repeated network or process communication. When a connection is established, the McpClient automatically sends a tools/list JSON-RPC request to the server, receiving a manifest of available tools with their names, descriptions, and input schemas. These are transformed into McpToolDef structs and cached in the McpServer's tools field, enabling fast local queries without server round-trips. The caching strategy balances freshness with efficiency: initial discovery populates the cache, with explicit refresh methods available for when tool availability changes. This pattern is essential for responsive user interfaces and efficient batch operations, as tool listing is a frequent operation while tool manifests change infrequently.

The implementation provides multiple access patterns: list_tools() aggregates across all connected servers, list_tools_for_server() scopes to one server, and refresh variations update the cache on demand. The refresh operations demonstrate graceful degradation—individual server failures are logged but don't fail the entire operation, recognizing that distributed systems should tolerate partial availability. Tool definitions include JSON Schema for input validation, enabling client-side argument checking before server invocation. The caching is shallow (in-memory, process-lifetime only) rather than persistent, ensuring fresh starts don't propagate stale data and recognizing that tool availability often depends on server-side state.

This pattern appears throughout AI tool ecosystems, from OpenAI's function calling to LangChain's tool registries, but MCP standardizes the discovery protocol. The cache invalidation strategy is explicit (manual refresh) rather than time-based, respecting that different deployments have different consistency requirements. The McpToolDef struct's serialization support enables potential future extensions like persisting discovered tools to configuration or sharing across client instances. The design anticipates dynamic tool ecosystems where servers may add capabilities at runtime, though currently requires explicit refresh to detect changes. For developers building on McpClient, this caching abstracts the latency and availability characteristics of individual servers, presenting a unified, responsive interface to tool consumers.

## External Resources

- [MCP Tools specification - discovery and invocation](https://spec.modelcontextprotocol.io/specification/server/tools/) - MCP Tools specification - discovery and invocation
- [JSON Schema standard for tool input validation](https://json-schema.org/) - JSON Schema standard for tool input validation
- [Martin Fowler on cache invalidation as a hard problem in computer science](https://martinfowler.com/bliki/TwoHardThings.html) - Martin Fowler on cache invalidation as a hard problem in computer science

## Sources

- [mod](../sources/mod.md)
