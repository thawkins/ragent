---
title: "Team Collaboration Workflow"
type: concept
generated: "2026-04-19T14:54:44.383530336+00:00"
---

# Team Collaboration Workflow

### From: ref:AGENTS

The team collaboration workflow specifies a structured approach to parallel development tasks using a team-based abstraction, designed for scenarios requiring multiple simultaneous reviewers or workers. The workflow follows a four-phase pattern: team creation with blueprint specification and context provisioning, blocking wait for completion, result aggregation through status checks or output file reading, and strict avoidance of duplicate work. This pattern addresses coordination challenges in distributed or AI-assisted development where multiple agents might otherwise conflict or redundantly process the same inputs.

The team create command requires explicit blueprint selection (e.g., code-review) and comprehensive context provision including target directories, specific tasks, and output locations. This context prepending mechanism ensures all team members share consistent understanding of objectives without repeated clarification. The mandatory team wait call—distinguished from task-specific wait mechanisms—provides synchronization without busy-waiting, blocking until all teammates reach idle state. The result reading phase emphasizes output file inspection over real-time monitoring, enabling asynchronous result consumption.

The workflow's prohibition against independent file reading while teammates process the same data prevents race conditions and redundant I/O. The specific example demonstrates security review, test coverage analysis, and performance auditing of a server directory with findings written to COMPLIANCE.md. This concrete illustration suggests organizational experience with compliance verification and multi-faceted code quality assessment. The workflow abstraction appears designed for AI agent coordination where instantiation of multiple specialized agents—each with the full context but specific focus—can parallelize work that would be sequential for single agents.

## External Resources

- [Fork-join parallel computing model](https://en.wikipedia.org/wiki/Fork%E2%80%93join_model) - Fork-join parallel computing model

## Sources

- [ref:AGENTS](../sources/ref-agents.md)
