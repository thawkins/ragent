---
title: "Teams — Agent Team Coordination for ragent Specification"
source: "TEAMS"
type: source
tags: [agent-coordination, multi-agent-systems, rust, TUI, anthropic-agent-teams, task-management, messaging, file-locking, specification]
generated: "2026-04-18T14:50:14.935852148+00:00"
---

# Teams — Agent Team Coordination for ragent Specification

This document specifies the "Teams" capability for ragent, a Rust-based TUI application for AI agent coordination. The Teams feature enables a "Team Lead" session to spawn and manage multiple autonomous "Teammate" sessions that collaborate on shared tasks through a coordinated task list and mailbox-based messaging system. The architecture consists of a TUI interface with specialized panels, a TeamManager runtime that spawns and tracks teammates, a file-locked TaskStore for shared task claiming, and a Mailbox system for agent-to-agent communication. The specification covers data schemas for team configuration, tasks, and messages; TUI integration including slash commands and visual indicators; quality gate hooks for customizable workflow control; and a milestone-based implementation plan spanning storage, tools, execution, TUI, documentation, and testing phases. Notable limitations in the initial release include no session resumption for teammates, single active team per lead, no nested teams, and no shared memory beyond tasks and mailbox.

## Related

### Entities

- [ragent](../entities/ragent.md) — product
- [Anthropic](../entities/anthropic.md) — organization
- [Team Lead](../entities/team-lead.md) — person
- [TeamManager](../entities/teammanager.md) — technology
- [TaskStore](../entities/taskstore.md) — technology
- [Mailbox](../entities/mailbox.md) — technology
- [fs2](../entities/fs2.md) — technology
- [tokio](../entities/tokio.md) — technology

### Concepts

- [Agent Teams Pattern](../concepts/agent-teams-pattern.md)
- [File-Locked Task Claiming](../concepts/file-locked-task-claiming.md)
- [Plan Approval State Machine](../concepts/plan-approval-state-machine.md)
- [Quality Gates (Hooks)](../concepts/quality-gates-hooks.md)
- [Session Context Isolation](../concepts/session-context-isolation.md)
- [Project-Local vs Global Teams](../concepts/project-local-vs-global-teams.md)

