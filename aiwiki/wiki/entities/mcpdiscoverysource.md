---
title: "McpDiscoverySource"
entity_type: "technology"
type: entity
generated: "2026-04-19T22:16:23.341365076+00:00"
---

# McpDiscoverySource

**Type:** technology

### From: test_mcp_discovery

`McpDiscoverySource` is an enumeration that captures the provenance of discovered MCP servers, serving as a critical component for security policy enforcement and source-specific behavior in the discovery system. This enum distinguishes between three distinct installation paradigms: system-wide installations accessible via PATH, Node.js/NPM global package installations, and dedicated MCP registry directories managed by the ragent system itself.

The `SystemPath` variant represents traditional executable discovery, where MCP servers are installed as standalone binaries in directories listed in the system's PATH environment variable. This is the most common distribution method for compiled MCP servers written in languages like Rust, Go, or C++. The `NpmGlobal` variant acknowledges the significant ecosystem of JavaScript and TypeScript-based MCP servers distributed through NPM, capturing the prefix directory where global NPM packages are installed. This is essential because Node.js-based servers typically require execution through the `node` runtime with the package's entry point as an argument.

The `McpRegistry` variant supports a dedicated management approach where MCP servers are organized in a specific directory structure, enabling more sophisticated versioning, isolation, and management capabilities. Each variant can carry additional metadata—such as `prefix_dir` for NPM or `registry_dir` for registry sources—that enables precise path resolution and source-specific validation. This provenance tracking allows the system to apply appropriate execution contexts, such as using `node` to run NPM-installed servers or applying sandboxing policies based on installation trust level.

## External Resources

- [NPM documentation on global package installation directories](https://docs.npmjs.com/cli/v10/configuring-npm/folders) - NPM documentation on global package installation directories
- [Rust enum definition and variant data documentation](https://doc.rust-lang.org/book/ch06-01-defining-an-enum.html) - Rust enum definition and variant data documentation

## Sources

- [test_mcp_discovery](../sources/test-mcp-discovery.md)
