---
title: "MCP stdio servers"
entity_type: "technology"
type: entity
generated: "2026-04-19T22:10:19.611577748+00:00"
---

# MCP stdio servers

**Type:** technology

### From: resource

MCP (Model Context Protocol) stdio servers represent a class of subprocess-based services that communicate through standard input/output streams, forming part of an emerging protocol for structured agent-tool interaction. These servers enable language models to invoke external capabilities through a well-defined interface, with stdio transport providing simplicity and broad compatibility. In the context of this codebase, MCP servers are treated as child processes subject to the same concurrency limits as other spawned processes, reflecting their resource characteristics. The protocol's design emphasizes stateless, request-response patterns over stdio, with JSON-RPC or similar framing for message exchange. This approach allows dynamic extension of agent capabilities through external executables while maintaining isolation and clear failure boundaries.

## External Resources

- [Model Context Protocol specification](https://spec.modelcontextprotocol.io/) - Model Context Protocol specification
- [JSON-RPC 2.0 specification for structured communication](https://jsonrpc.org/specification) - JSON-RPC 2.0 specification for structured communication

## Sources

- [resource](../sources/resource.md)
