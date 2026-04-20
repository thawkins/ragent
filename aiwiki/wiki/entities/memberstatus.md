---
title: "MemberStatus"
entity_type: "technology"
type: entity
generated: "2026-04-19T19:33:54.037222022+00:00"
---

# MemberStatus

**Type:** technology

### From: team_status

MemberStatus is a Rust enumeration defining the complete state space for individual agent membership in a team, serving as the fundamental coordination primitive for multi-agent lifecycle management. The eight defined states—Working, Idle, PlanPending, Blocked, ShuttingDown, Stopped, Spawning, and Failed—constitute a comprehensive state machine that enables both human operators and automated systems to reason about agent availability and health. Each variant carries semantic significance for task assignment decisions, failure recovery procedures, and resource management policies.

The state machine encoded in MemberStatus reveals sophisticated operational semantics. Spawning represents initialization in progress, preventing premature work assignment. PlanPending indicates agents awaiting task planning or human approval before execution. Working denotes active task execution, while Idle suggests availability for new assignments. Blocked captures dependency-waiting or resource-contention scenarios requiring external intervention. The terminal states—ShuttingDown, Stopped, and Failed—enable graceful degradation, with Failed specifically signaling error conditions that may trigger alerting or replacement workflows.

The TeamStatusTool's visualization mapping leverages Unicode emoji to encode these states into instantly recognizable visual patterns: 🔄 for Working (suggesting activity), ⏸ for Idle (suggesting pause), 📋 for PlanPending (suggesting documentation), 🔒 for Blocked (suggesting obstruction), 🛑 for ShuttingDown (suggesting cessation), ⬛ for Stopped (suggesting completion), 🚀 for Spawning (suggesting launch), and ❌ for Failed (suggesting error). This visual encoding transforms dense log output into scannable status dashboards, demonstrating how thoughtful UI design can enhance observability in complex distributed systems.

## External Resources

- [Rust formatting traits enabling debug output like {:?} for enum variants](https://doc.rust-lang.org/std/fmt/) - Rust formatting traits enabling debug output like {:?} for enum variants

## Sources

- [team_status](../sources/team-status.md)
