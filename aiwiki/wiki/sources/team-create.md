---
title: "TeamCreateTool: Dynamic Agent Team Provisioning in Ragent"
source: "team_create"
type: source
tags: [rust, multi-agent-system, team-management, blueprint-pattern, dynamic-provisioning, ragent-core, tool-implementation, async-rust]
generated: "2026-04-19T19:11:35.138228065+00:00"
---

# TeamCreateTool: Dynamic Agent Team Provisioning in Ragent

The `team_create.rs` file implements `TeamCreateTool`, a sophisticated tool for provisioning and initializing agent teams within the Ragent multi-agent framework. This tool serves as the primary entry point for creating named teams with configurable blueprints, enabling dynamic team composition based on declarative templates. The implementation handles complex initialization workflows including directory creation, blueprint resolution, seed task execution, and automated teammate spawning through structured JSON configuration files.

The tool's architecture demonstrates several advanced Rust patterns, including error handling with `anyhow`, asynchronous trait implementations via `async-trait`, and flexible JSON schema generation for tool parameter validation. The `TeamCreateTool` implements the `Tool` trait, providing metadata about its name, description, parameter schema, and permission category. The execute method orchestrates a multi-stage initialization process that begins with validation of the mandatory blueprint parameter and proceeds through team name generation, directory structure creation, and blueprint-based configuration application.

A particularly notable aspect of this implementation is its support for idempotent team creation. When a team with the requested name already exists, the tool attempts to load the existing team and continue with blueprint seeding rather than failing outright. This design supports scenarios where teams need incremental enhancement or recovery from partial initialization states. The tool also implements sophisticated directory traversal logic for blueprint discovery, searching project-local `.ragent/blueprints/teams/` directories before falling back to global user-level blueprints in `~/.ragent/blueprints/teams/`, enabling both project-specific and reusable team templates.

## Related

### Entities

- [TeamCreateTool](../entities/teamcreatetool.md) — technology
- [TeamStore](../entities/teamstore.md) — technology
- [HookEvent](../entities/hookevent.md) — technology

