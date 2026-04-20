---
title: "MemoryScope"
entity_type: "technology"
type: entity
generated: "2026-04-19T19:15:44.485538187+00:00"
---

# MemoryScope

**Type:** technology

### From: team_memory_read

MemoryScope is an enumeration type that defines the visibility and persistence boundaries for agent memory within the team architecture. The type distinguishes between three distinct levels: None (memory disabled), User (agent-specific persistent storage), and Project (shared team-wide persistent storage). This scoping mechanism enables fine-grained control over data isolation and sharing patterns in multi-agent deployments, addressing critical requirements for both privacy and collaboration.

The None variant serves as an explicit opt-out, allowing agents to operate without persistent memory when statelessness is desired or when security policies prohibit retention. The User scope provides isolated storage per agent identity, ensuring that personal preferences, learned patterns, and confidential information remain accessible only to the originating agent. The Project scope enables collaborative memory spaces where team members can share context, build collective knowledge bases, and maintain synchronized state across multiple agents working toward common objectives.

This three-tier architecture reflects lessons from distributed systems design applied to AI agent contexts. The scoping abstraction allows the same underlying storage mechanisms to serve diverse use cases without code duplication, while the explicit enum typing prevents accidental scope confusion at compile time. Resolution of actual filesystem paths from MemoryScope values occurs through the `resolve_memory_dir` function, which translates these logical scopes into concrete directory structures based on agent names and working directories.

## External Resources

- [Multitenancy architecture patterns for isolated data](https://en.wikipedia.org/wiki/Multitenancy) - Multitenancy architecture patterns for isolated data
- [Rust enums and pattern matching](https://doc.rust-lang.org/book/ch06-00-enums.html) - Rust enums and pattern matching

## Sources

- [team_memory_read](../sources/team-memory-read.md)

### From: team_memory_write

MemoryScope is an enumerated type that defines the persistence boundaries for agent memory within the ragent framework. Based on the implementation's usage patterns, this enum likely includes at least three variants: None (disabling memory persistence), User (scope memory to individual agent/users), and Project (share memory across all agents in a project context). This scoping mechanism represents a fundamental architectural decision in multi-agent system design, balancing isolation against collaboration needs.

The None variant serves important operational purposes beyond simple feature disabling. When an agent's memory scope is set to None, the tool returns an informative error message directing users toward proper configuration, specifically mentioning the JSON configuration keys `"memory": "user"` or `"memory": "project"`. This design choice embeds documentation directly into runtime behavior, improving developer experience when onboarding new agents or debugging configuration issues. The error metadata includes a machine-readable error code ("memory_disabled") enabling programmatic handling by calling systems.

The User and Project variants enable fundamentally different collaboration patterns. User-scoped memory creates isolated workspaces where each agent maintains independent knowledge bases, suitable for personal assistants or competitive agent scenarios. Project-scoped memory enables shared knowledge pools where agents can read each other's memories and build upon collective learning, essential for collaborative problem-solving and emergent multi-agent intelligence. The resolve_memory_dir function maps these abstract scopes to concrete filesystem paths, likely incorporating the agent's teammate_name for user scope or using team-wide directories for project scope.
