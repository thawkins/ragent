---
title: "npm"
entity_type: "technology"
type: entity
generated: "2026-04-19T15:15:57.535073537+00:00"
---

# npm

**Type:** technology

### From: discovery

npm (Node Package Manager) is the default package manager for the JavaScript runtime environment Node.js, and it serves as a critical distribution channel for MCP servers in the discovery module. The module specifically targets npm's global installation directories to find MCP server packages, executing `npm prefix -g` to determine the correct path for the current system and then traversing the `node_modules` structure to locate relevant packages.

The discovery module implements sophisticated npm scanning logic that handles platform differences—on Unix-like systems, global packages are located under `prefix/lib/node_modules`, while on Windows they reside directly in `prefix/node_modules`. The scanner specifically looks for packages under the `@modelcontextprotocol` scope, which represents the official MCP package namespace, as well as community packages following the `mcp-server-*` naming convention. For each candidate package, the module parses `package.json` to extract the binary entry point, package name, and description.

npm's role in MCP server distribution highlights the protocol's JavaScript/TypeScript origins and the prevalence of Node.js-based implementations. Many official MCP servers, including those for filesystem access, GitHub integration, and web browsing, are implemented as Node.js applications distributed through npm. The discovery module's npm integration ensures these servers are automatically detected when globally installed, without requiring manual PATH configuration or registry entry creation. This approach leverages npm's existing infrastructure for package management, versioning, and dependency resolution.

## External Resources

- [npm official website and package registry](https://www.npmjs.com/) - npm official website and package registry
- [npm documentation for the prefix command used by discovery](https://docs.npmjs.com/cli/v10/commands/npm-prefix) - npm documentation for the prefix command used by discovery

## Sources

- [discovery](../sources/discovery.md)
