# Local source (in-project)

- Path: COMMSPLAN.md
- Relevance: 238 match(es) on: agent, an, at, …(+23) — "# COMMSPLAN.md — Agent Communication Remediation Plan"
- Captured (UTC): 2026-06-25T05:53:23.384820450+00:00

```text
Excerpt — 238 keyword match(es)

▶    1:    1  # COMMSPLAN.md — Agent Communication Remediation Plan
     2:    2  
▶    3:    3  **Team:** swarm-20260621-133647  
▶    4:    4  **Author:** swarm-s6 (task s6)  
▶    5:    5  **Date:** 2026-06-21  
▶    6:    6  **Status:** Planning — no code changes yet  
     7:    7  
▶    8:    8  This document synthesizes the findings from the swarm communication review
▶    9:    9  (tasks s1–s5) into an actionable, prioritized remediation plan. It describes
▶   10:   10  what to change, why each change matters, and which reviewer finding(s) it
▶   11:   11  addresses. Implementation of the remediation code itself is **out of scope
▶   12:   12  for this task** and is left to subsequent engineering work.
    13:   13  
…
    17:   17  
▶   18:   18  The ragent multi-agent communication stack is built on three overlapping
    19:   19  subsystems:
    20:   20  
▶   21:   21  1. **Teams / Swarm** (`ragent-team` crate, mirrored partially in
▶   22:   22     `crates/ragent-agent/src/team/`) — file-backed mailboxes, shared
▶   23:   23     `tasks.json`, and the `EventBus`.
▶   24:   24  2. **Sub-agent tasks** (`ragent-agent/src/task/`) — in-memory `TaskManager`
    25:   25     plus `EventBus` events.
▶   26:   26  3. **Orchestrator** (`ragent-agent/src/orchestrator/`) — capability-based
    27:   27     in-process / HTTP routing.
    28:   28  
▶   29:   29  The primary production path is the **Teams / Swarm** subsystem. The audit
▶   30:   30  found it functionally works for simple, low-concurrency cases, but it has
▶   31:   31  serious correctness and observability gaps that become dangerous under
▶   32:   32  concurrent use, leader restarts, or long-running swarms.
    33:   33  

… (208 more match(es) elided)

```
