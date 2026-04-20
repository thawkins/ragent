---
title: "resolve_agent_id"
entity_type: "technology"
type: entity
generated: "2026-04-19T19:04:55.571389754+00:00"
---

# resolve_agent_id

**Type:** technology

### From: team_assign_task

The `resolve_agent_id` function is a utility component imported from the `team_message` module that performs identifier resolution in the ragent team system. This function bridges the gap between human-friendly agent references and the canonical identifiers used for persistent storage and access control. Based on its usage pattern in `TeamAssignTaskTool`, it accepts a team directory path and a string identifier that may represent either a human-readable name or an already-canonical agent ID, returning a standardized `Result` containing the resolved identifier.

The resolution logic likely implements a flexible lookup strategy common in multi-agent systems: first checking if the input matches a canonical ID directly, then falling back to name-based lookups against the team's membership configuration. This enables users and invoking agents to reference teammates naturally (e.g., "alice" or "Alice Smith") while the system maintains strict internal identifiers for security and consistency. The function's placement in the `team_message` module suggests it originated from messaging use cases where addressing agents by convenient names was essential, but its utility proved broad enough to support task assignment and other management operations.

The special-case handling of the `"lead"` identifier in the assignment tool provides insight into the resolution behavior. The code explicitly bypasses membership verification when `agent_id == "lead"`, indicating that `"lead"` is a reserved, context-sensitive identifier that doesn't require registry validation. This suggests `resolve_agent_id` may normalize various lead references to this canonical string, or that the team directory structure implicitly recognizes lead roles without explicit member entries. The function's error propagation through `anyhow::Result` maintains consistency with the broader error handling strategy in ragent-core tools.

## Sources

- [team_assign_task](../sources/team-assign-task.md)
