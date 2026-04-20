---
title: "Session-Based Architecture"
type: concept
generated: "2026-04-19T14:56:28.626711345+00:00"
---

# Session-Based Architecture

### From: main

The session-based architecture is ragent's core interaction model, treating conversations as persistent, resumable, and exportable entities rather than ephemeral command executions. Each session captures complete context including the working directory, conversation history as structured messages, and metadata like creation and modification times. This design philosophy acknowledges that AI-assisted coding is inherently stateful—understanding a codebase requires accumulated context that should survive process restarts and support collaborative handoff.

The SessionManager centralizes lifecycle operations: creating sessions with canonicalized working directories, listing available sessions with metadata for UI display, and retrieving sessions for resumption. Sessions are identified by UUIDs ensuring uniqueness across reinstalls and multi-user scenarios. The SessionProcessor handles the actual message processing loop, maintaining the agentic execution context (tool availability, permission state, streaming configuration) that persists across turns within a session.

Message structure within sessions preserves the full exchange history including role attribution (user/assistant/tool), multipart content (text, tool calls, tool results), and temporal metadata. This enables sophisticated features like conversation branching (not shown in excerpt but suggested by architecture), partial replay for debugging, and format conversion for import/export. The import/export functionality supports migration from competing tools (Cline, Claude Code) through adapter patterns that normalize external formats to ragent's internal representation.

## External Resources

- [State management in software engineering](https://en.wikipedia.org/wiki/State_management) - State management in software engineering

## Sources

- [main](../sources/main.md)
