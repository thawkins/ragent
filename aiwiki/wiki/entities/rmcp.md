---
title: "rmcp"
entity_type: "technology"
type: entity
generated: "2026-04-19T15:18:45.472453878+00:00"
---

# rmcp

**Type:** technology

### From: mod

The official Rust SDK for the Model Context Protocol, developed as the reference implementation for building MCP clients and servers in the Rust ecosystem. The rmcp crate provides the foundational abstractions and transport implementations that this McpClient builds upon, including the ServiceExt trait for service initialization, the RunningService type for managing active connections, and RoleClient for client-side protocol handling. The SDK abstracts the complexity of the MCP wire protocol, JSON-RPC message framing, capability negotiation, and lifecycle management, allowing application developers to focus on business logic rather than protocol details. The crate follows Rust's zero-cost abstraction philosophy, using generics and trait objects to enable efficient, type-safe protocol handling without runtime overhead.

The rmcp SDK implements the full MCP specification including the initialize handshake for capability exchange, the tools/list method for tool discovery, and the tools/call method for invocation. It provides transport adapters for common communication patterns: TokioChildProcess for stdio-based servers where the MCP server runs as a child process with JSON-RPC messages over stdin/stdout, and StreamableHttpClientTransport for HTTP-based servers using the streamable HTTP transport defined in the MCP specification. These transport implementations handle the low-level details of process management, stream multiplexing, connection pooling, and error recovery. The SDK also manages protocol versioning, ensuring compatibility between clients and servers even as the specification evolves.

In this codebase, rmcp serves as the critical dependency that enables MCP functionality. The McpConnection struct wraps rmcp's RunningService<RoleClient, ()> to hold an active connection, while McpClient uses rmcp's transport types to establish connections. The SDK's design patterns influence the client architecture, particularly the peer() method for accessing server capabilities and the serve() method for initializing connections. The rmcp crate is likely maintained by the MCP specification authors or a dedicated team, ensuring alignment between the SDK and protocol standards. Its use here represents a best practice of building upon official, well-maintained libraries rather than implementing protocol details from scratch, reducing security surface area and improving interoperability with the broader MCP ecosystem.

## External Resources

- [rmcp crate on crates.io - Rust package registry](https://crates.io/crates/rmcp) - rmcp crate on crates.io - Rust package registry
- [rmcp SDK source repository on GitHub](https://github.com/modelcontextprotocol/rust-sdk) - rmcp SDK source repository on GitHub
- [MCP specification defining the protocol implemented by rmcp](https://modelcontextprotocol.io/specification) - MCP specification defining the protocol implemented by rmcp

## Sources

- [mod](../sources/mod.md)
