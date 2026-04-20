---
title: "MCP Server Discovery Test Suite"
source: "test_mcp_discovery"
type: source
tags: [rust, testing, mcp, model-context-protocol, discovery, configuration, security, npm, async-testing]
generated: "2026-04-19T22:16:23.340141172+00:00"
---

# MCP Server Discovery Test Suite

This document contains a comprehensive test suite for the Model Context Protocol (MCP) server discovery functionality within the ragent-core Rust crate. The test file validates the core discovery mechanisms that enable automatic detection and configuration of MCP servers from various installation sources. The tests cover four primary areas: conversion of discovered servers to configuration objects, handling of NPM globally installed servers, processing of registry-based server sources, and ensuring proper deduplication of discovered servers by their unique identifiers.

The test suite employs both synchronous and asynchronous testing patterns, with three synchronous tests validating the `DiscoveredMcpServer` struct's behavior and conversion methods, and two asynchronous tests verifying the actual discovery operations. A critical design decision reflected in these tests is the security-conscious approach of disabling discovered servers by default, requiring explicit user enablement before activation. This prevents potentially malicious or unwanted servers from automatically executing without user consent.

The discovery system supports multiple installation sources including system PATH directories, NPM global installations (common for JavaScript/TypeScript based MCP servers), and a dedicated MCP registry directory structure. The tests demonstrate how the system handles environment variables, command-line arguments, and executable path resolution across these different source types. The deduplication test ensures that when multiple discovery sources identify the same server (by ID), only one instance is retained, preventing configuration conflicts and redundant entries.

## Related

### Entities

- [DiscoveredMcpServer](../entities/discoveredmcpserver.md) — technology
- [McpDiscoverySource](../entities/mcpdiscoverysource.md) — technology
- [ragent-core](../entities/ragent-core.md) — product

