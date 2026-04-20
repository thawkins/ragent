---
title: "Claude Desktop"
entity_type: "product"
type: entity
generated: "2026-04-19T15:15:57.534165786+00:00"
---

# Claude Desktop

**Type:** product

### From: discovery

Claude Desktop is an AI assistant application developed by Anthropic that serves as a primary consumer of MCP servers. The application maintains a dedicated MCP registry directory at `~/.claude/mcp-servers` where users can install server configurations. This directory is one of the key locations scanned by the discovery module to find locally configured MCP servers. Claude Desktop uses these discovered servers to extend its capabilities, allowing the AI to interact with external tools, databases, file systems, and APIs through the standardized Model Context Protocol.

The integration between Claude Desktop and MCP servers represents a significant shift in how AI assistants interact with external systems. Rather than baking specific integrations into the core application, Claude Desktop can dynamically discover and connect to any MCP-compliant server, creating an ecosystem of extensible tools. The discovery module specifically checks for `server.json` configuration files within subdirectories of the Claude MCP registry, parsing command paths, arguments, and environment variables needed to launch each server.

From a technical perspective, Claude Desktop's use of a well-known directory structure enables interoperability with other MCP-aware tools. The discovery module treats Claude's registry as a first-class discovery source alongside npm packages and PATH executables, ensuring that servers configured for Claude are also available to other MCP clients like `ragent`. This design promotes configuration portability and reduces duplication across the MCP ecosystem.

## External Resources

- [Official Claude AI assistant product page from Anthropic](https://www.anthropic.com/claude) - Official Claude AI assistant product page from Anthropic
- [Official MCP servers repository with configuration examples](https://github.com/modelcontextprotocol/servers) - Official MCP servers repository with configuration examples

## Sources

- [discovery](../sources/discovery.md)
