---
title: "Event-Driven Architecture"
type: concept
generated: "2026-04-18T14:47:32.733428280+00:00"
---

# Event-Driven Architecture

### From: README_LSP_EXPLORATION

System using tokio broadcast channels where components publish and subscribe to events. 30+ event types include SessionCreated, MessageStart, TextDelta, ToolCallStart/End, and PermissionRequested. Tools publish events directly, TUI subscribes for updates, and complete audit trail is maintained.

## Sources

- [README_LSP_EXPLORATION](../sources/readme-lsp-exploration.md)

### From: LSP_INTEGRATION_GUIDE

Central EventBus using broadcast channels for publish-subscribe pattern. Events cover session lifecycle, message streaming, tool calls, permissions, agent switching, and token usage. TUI subscribes to events for UI updates.

### From: LSP_QUICK_REFERENCE

System uses EventBus for pub/sub pattern with 30+ event types including ToolCallStart, ToolCallEnd, TextDelta, PermissionRequested. TUI subscribes to update interface
