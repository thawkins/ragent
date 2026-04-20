---
title: "DiscoveredMcpServer"
entity_type: "technology"
type: entity
generated: "2026-04-19T22:16:23.340859166+00:00"
---

# DiscoveredMcpServer

**Type:** technology

### From: test_mcp_discovery

The `DiscoveredMcpServer` struct represents a fundamental abstraction in the MCP discovery system, encapsulating all metadata required to identify, configure, and execute a discovered MCP server. This data structure serves as the bridge between the discovery mechanism and the runtime configuration system, capturing critical attributes including a unique server identifier, human-readable name, executable path, command-line arguments, environment variables, and the source from which the server was discovered.

The struct's design reflects careful consideration of real-world deployment scenarios. The `id` field provides a stable identifier for deduplication and reference purposes, while the `name` field offers user-facing display text. The `executable` field uses `PathBuf` for cross-platform path handling, acknowledging that MCP servers may be written in any language and installed anywhere on the filesystem. The `args` and `env` fields capture server-specific configuration, allowing discovered servers to maintain their customization parameters.

The `source` field, typed as `McpDiscoverySource`, is particularly significant as it enables provenance tracking and source-specific handling. This allows the system to apply different security policies, update mechanisms, or validation rules based on where a server originated—whether from a trusted system path, an NPM global installation, or a user-specific registry directory. This provenance awareness is essential for maintaining security boundaries in a system where arbitrary code execution is a core capability.

## Diagram

```mermaid
classDiagram
    class DiscoveredMcpServer {
        +String id
        +String name
        +PathBuf executable
        +Vec~String~ args
        +HashMap~String,String~ env
        +McpDiscoverySource source
        +to_config() ServerConfig
    }
    class McpDiscoverySource {
        <<enumeration>>
        SystemPath
        NpmGlobal { prefix_dir: PathBuf }
        McpRegistry { registry_dir: PathBuf }
    }
    class ServerConfig {
        +Option~String~ command
        +Vec~String~ args
        +HashMap~String,String~ env
        +bool disabled
    }
    DiscoveredMcpServer --> McpDiscoverySource : source
    DiscoveredMcpServer --> ServerConfig : converts to
```

## External Resources

- [Model Context Protocol official documentation and specification](https://modelcontextprotocol.io/) - Model Context Protocol official documentation and specification
- [Rust PathBuf documentation for cross-platform path handling](https://doc.rust-lang.org/std/path/struct.PathBuf.html) - Rust PathBuf documentation for cross-platform path handling

## Sources

- [test_mcp_discovery](../sources/test-mcp-discovery.md)
