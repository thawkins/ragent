---
title: "TaskEntry"
entity_type: "technology"
type: entity
generated: "2026-04-19T18:17:21.445622286+00:00"
---

# TaskEntry

**Type:** technology

### From: list_tasks

TaskEntry is a core data structure in the ragent-core task management system that encapsulates the complete state and metadata of a sub-agent task execution instance. As referenced throughout ListTasksTool's implementation, this struct maintains comprehensive provenance information including unique task identification, associated agent name, execution status, temporal metadata, session relationships, and optional execution outputs. The structure supports both synchronous and asynchronous task patterns through its background field, enabling fire-and-forget execution models where tasks proceed independently of their spawning context. The inclusion of both result and error optional fields reflects a Result-like semantic for task completion, accommodating both successful and failed execution paths with full output capture.

The temporal metadata fields in TaskEntry demonstrate sophisticated execution tracking capabilities, with created_at marking task inception and completed_at optionally recording termination time for finished tasks. The duration calculation logic in ListTasksTool reveals that runtime duration is computed dynamically based on these timestamps, using chrono::Utc::now() for running tasks to provide real-time elapsed time reporting. The session relationship fields—parent_session_id and child_session_id—establish bidirectional links in a delegation hierarchy, enabling reconstruction of agent spawning chains and supporting debugging of complex multi-agent workflows. The task_prompt field preserves the original instruction or context that initiated the task, providing essential provenance for result interpretation and audit trails.

TaskEntry's design reflects careful consideration of observability and debugging requirements in production AI systems. The agent_name field enables attribution of work to specific agent configurations or versions, supporting performance analysis and cost tracking across different agent types. The status field using the TaskStatus enumeration provides a state machine with explicit terminal states (Completed, Failed, Cancelled) and an intermediate state (Running), enabling reliable progress monitoring and lifecycle management. The structure's support for partial information—with many fields being optional or computed—accommodates tasks in various stages of completion while maintaining type safety. This design pattern enables rich historical analysis of task execution patterns, failure modes, and performance characteristics across the agent system.

## External Resources

- [chrono DateTime documentation for temporal tracking](https://docs.rs/chrono/latest/chrono/struct.DateTime.html) - chrono DateTime documentation for temporal tracking
- [Rust Option type patterns for optional fields](https://doc.rust-lang.org/rust-by-example/error/option_unwrap.html) - Rust Option type patterns for optional fields

## Sources

- [list_tasks](../sources/list-tasks.md)
