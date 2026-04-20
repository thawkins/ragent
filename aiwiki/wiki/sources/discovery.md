---
title: "MCP Server Discovery Module in Rust"
source: "discovery"
type: source
tags: [rust, mcp, model-context-protocol, server-discovery, npm, claude, cline, async, tokio, system-scanning]
generated: "2026-04-19T15:15:57.533453234+00:00"
---

# MCP Server Discovery Module in Rust

This document describes the `discovery.rs` module from the `ragent-core` crate, which implements automatic discovery of Model Context Protocol (MCP) servers installed on a local system. The module provides a comprehensive scanning mechanism that checks multiple sources for MCP server executables, including the system PATH, npm global package directories, and well-known MCP registry directories used by applications like Claude Desktop and Cline.

The discovery process begins with the `discover()` function, which orchestrates the entire scanning workflow. It first checks for known MCP server executables by iterating through a predefined list of well-known servers and attempting to resolve them on the PATH. Each known server includes metadata such as its identifier, human-readable name, candidate executable names, and required arguments. The module then extends the search to npm global installations, specifically looking for packages under the `@modelcontextprotocol` scope or packages with names matching `mcp-server-*` or `mcp_server_*` patterns. Finally, it scans MCP registry directories in common locations like `~/.claude/mcp-servers`, `~/.cline/mcp-servers`, and XDG configuration directories.

A key design principle of this module is its resilience—discovered servers are returned with `disabled: true` in their configuration, requiring explicit user opt-in before activation. This approach prioritizes security and user control over automatic connection. The module also implements deduplication logic to prevent the same server from being reported multiple times when found through different discovery sources. The architecture uses Rust's async/await patterns with Tokio for non-blocking I/O operations, and includes comprehensive error handling that silently skips missing servers rather than failing the entire discovery process.

## Related

### Entities

- [Claude Desktop](../entities/claude-desktop.md) — product
- [Cline](../entities/cline.md) — product
- [npm](../entities/npm.md) — technology
- [Tokio](../entities/tokio.md) — technology

