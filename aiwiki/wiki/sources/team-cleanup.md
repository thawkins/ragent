---
title: "Team Cleanup Tool Implementation in Ragent Core"
source: "team_cleanup"
type: source
tags: [rust, multi-agent-systems, team-management, cleanup, async-trait, serde-json, anyhow, filesystem-operations, configuration-management]
generated: "2026-04-19T19:09:50.891760930+00:00"
---

# Team Cleanup Tool Implementation in Ragent Core

This Rust source file implements the `TeamCleanupTool`, a critical administrative utility within the ragent-core system designed for safe teardown and removal of team directories from persistent storage. The tool operates with strict safety constraints, requiring that all team members be in a stopped state before allowing deletion unless an explicit force flag is provided. This design reflects a careful balance between operational convenience and data integrity, preventing accidental destruction of active computational resources.

The implementation follows the async Tool trait pattern established in the codebase, with clear separation between metadata definition and execution logic. The tool integrates deeply with the team's persistence layer, utilizing `TeamStore` for configuration management and status tracking. Notably, it implements a two-phase safety mechanism: first validating member states through configuration inspection, then optionally marking the team as disbanded before filesystem deletion. This creates an audit trail even when resources are ultimately removed.

The permission model assigns this tool to the "team:manage" category, indicating it requires elevated privileges. The description explicitly notes "Lead-only" access, suggesting hierarchical governance within the multi-agent system. The error handling strategy leverages anyhow for ergonomic error propagation while providing actionable user guidance—when active members block cleanup, the error message includes specific remediation steps and alternative approaches.

## Related

### Entities

- [TeamCleanupTool](../entities/teamcleanuptool.md) — technology
- [TeamStore](../entities/teamstore.md) — technology
- [ragent-core](../entities/ragent-core.md) — product

### Concepts

- [Async Tool Pattern](../concepts/async-tool-pattern.md)
- [Multi-Agent Team Lifecycle Management](../concepts/multi-agent-team-lifecycle-management.md)
- [Defensive Resource Management](../concepts/defensive-resource-management.md)
- [JSON Schema-Driven Interfaces](../concepts/json-schema-driven-interfaces.md)

