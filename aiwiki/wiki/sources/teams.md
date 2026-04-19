---
title: "Teams — Agent Team Coordination Specification for ragent"
source: "TEAMS"
type: source
tags: [agent-teams, multi-agent, coordination, rust, tui, architecture, specification, ragent, ai-agents, concurrency, file-locking, milestones]
generated: "2026-04-18T15:17:02.936303087+00:00"
---

# Teams — Agent Team Coordination Specification for ragent

This document specifies the "Teams" capability for ragent, a Rust-based TUI application for AI agents. Inspired by Anthropic's Agent Teams pattern, this feature enables coordinated multi-agent workflows where a Team Lead (the user session) can spawn and manage multiple teammate agents that work on shared tasks concurrently. The architecture consists of a TUI interface with dedicated panels for team status, a TeamManager runtime component that spawns and tracks teammate sessions, a file-backed TaskStore with exclusive locking for safe concurrent access, and a Mailbox system for inter-agent communication.

The specification details storage schemas for team configuration, task lists with dependency tracking, and message passing between agents. It covers TUI integration including a Teams Panel showing real-time teammate status, slash commands for team management (/team create, /team status, /team tasks, etc.), and integration with the existing Agents window. Quality gates through configurable hooks allow external validation of teammate idle states and task completions. The implementation is organized into six milestones: core data structures and storage (M1), tool integration (M2), session and execution layer with TeamManager (M3), TUI integration (M4), documentation and examples (M5), and comprehensive testing (M6). Key limitations for the initial release include no session resumption for teammates, only one active team per lead session, no nested teams, and no split-pane display support.

## Related

### Entities

- [ragent](../entities/ragent.md) — product
- [Anthropic](../entities/anthropic.md) — organization
- [TeamManager](../entities/teammanager.md) — technology
- [TaskStore](../entities/taskstore.md) — technology
- [Mailbox](../entities/mailbox.md) — technology
- [fs2](../entities/fs2.md) — technology
- [SessionProcessor](../entities/sessionprocessor.md) — technology
- [EventBus](../entities/eventbus.md) — technology
- [tokio](../entities/tokio.md) — technology
- [serde](../entities/serde.md) — technology

### Concepts

- [Agent Teams Pattern](../concepts/agent-teams-pattern.md)
- [File-Locked Task Claiming](../concepts/file-locked-task-claiming.md)
- [Plan Approval Workflow](../concepts/plan-approval-workflow.md)
- [Quality Gates (Hooks)](../concepts/quality-gates-hooks.md)
- [Project-Local vs Global Teams](../concepts/project-local-vs-global-teams.md)
- [Dependency-Based Task Scheduling](../concepts/dependency-based-task-scheduling.md)
- [System Prompt Injection](../concepts/system-prompt-injection.md)
- [Read-Only Plan Mode](../concepts/read-only-plan-mode.md)
- [Mailbox Polling Loop](../concepts/mailbox-polling-loop.md)
- [Milestone-Based Development](../concepts/milestone-based-development.md)

