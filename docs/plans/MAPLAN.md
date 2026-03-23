# MAPLAN: Multi-Agent Orchestration (F6)

Overview

This plan defines tasks and milestones to design, implement, test, and document Feature F6: "Multi-agent orchestration" — enabling multiple agents to collaborate on a single task. The feature is currently not implemented (❌) and this plan breaks the work into well-defined milestones and tasks with acceptance criteria.

Goals

- Provide a reliable orchestration layer so multiple agents can collaboratively work on the same user request or job.
- Make orchestration pluggable and observable (tracing, logs, metrics).
- Ensure safe concurrency, conflict resolution, and secure inter-agent communication.
- Provide tests, examples, and documentation for maintainers and integrators.

Scope

Includes:
- Orchestration coordinator component (synchronous and asynchronous flows)
- Agent registry and capability discovery
- Message bus / task routing (in-process for MVP; extensible for distributed)
- Protocols for negotiation, subtask assignment, result aggregation, and conflict resolution
- APIs for starting orchestrated jobs and monitoring status
- Integration tests, unit tests, and documentation

Out of scope for MVP:
- Multi-host distributed transport (introduced later as an extension)

Acceptance Criteria

1. A coordinator can accept a user task and route subtasks to two or more agents.
2. Agents can register capabilities and subscribe to tasks matching capabilities.
3. Coordinator aggregates partial results and returns a composed final result.
4. Orchestration provides traceable logs and metrics for each job.
5. Comprehensive unit and integration tests exercise orchestration flows.

Milestones & Tasks

Milestone 1 — Research & Design (1 week)
- Task 1.1: Define orchestration model and interfaces (Coordinator, AgentHandle, Task, Result, Event). (2d)
  - Deliverable: MD doc of interfaces and sequence diagrams.
  - Acceptance: Team review signoff.

- Task 1.2: Choose an in-process messaging pattern (actor-style inboxes, async channels, or work-stealing). (1d)
  - Deliverable: Recommendation and rationale.

- Task 1.3: Define capability discovery scheme and task matching rules (simple tags for MVP). (1d)

Milestone 2 — Core Implementation (2–3 weeks)
- Task 2.1: Implement Agent Registry
  - Responsibilities: agent registration/unregistration, capability metadata, liveness. (3d)
  - API: register(agent_id, capabilities), unregister(agent_id), list(), get().

- Task 2.2: Implement Coordinator core
  - Responsibilities: accept tasks, decompose (if required), match to agents, send subtask messages, collect responses, aggregate results. (5d)
  - Provide both synchronous (await final result) and asynchronous (job id + poll/subscribe) modes.

- Task 2.3: Implement Messaging Layer (in-process)
  - Use Tokio mpsc channels and a router abstraction to allow later swapping for network transport. (3d)

- Task 2.4: Implement basic negotiation and conflict resolution
  - Simple rules: first-accepted assignment, timeouts and fallback agents, deterministic merge strategy. (3d)

Milestone 3 — API, CLI & Integration (1–2 weeks)
- Task 3.1: Public API for starting orchestration jobs
  - Implement functions in ragent-core crate for starting a job and receiving results. (2d)

- Task 3.2: CLI/TUI integration example
  - Provide an example command in ragent-server or ragent-tui showing multi-agent orchestration request. (2d)

- Task 3.3: Observability hooks
  - Integrate tracing spans (tracing crate), structured logging, and basic Prometheus metrics endpoints or counters. (2d)

Milestone 4 — Tests, Docs & Examples (1–2 weeks)
- Task 4.1: Unit tests for registry, coordinator routing, messaging and aggregation. (3d)
- Task 4.2: Integration tests under tests/ that spin up multiple agents and assert correct composition and timeouts. (3d)
- Task 4.3: Add docs and usage examples
  - Update SPEC.md, QUICKSTART.md if feature changes usage flow.
  - Add MAPLAN.md (this file) to repo (done). (2d)

Milestone 5 — Hardening & Extensions (2+ weeks, optional)
- Task 5.1: Pluggable transport adapters (gRPC, WebSocket) for distributed agents. (5d)
- Task 5.2: Advanced leader election and distributed coordination. (5d)
- Task 5.3: Policy-based conflict resolution and human-in-the-loop fallbacks. (4d)

Implementation Details & Design Notes

- Data structures
  - TaskId (UUID), AgentId (String/UUID), TaskDescriptor (capabilities, priority, metadata), TaskResult (status, payload, diagnostics).

- Coordinator API sketch
  - start_job(descriptor) -> JobHandle
  - JobHandle.await_result() -> TaskResult
  - subscribe_job_events(job_id) -> Stream<Event>

- Messaging abstraction
  - Router trait with impls: InProcessRouter (Tokio channels) and placeholder RemoteRouter trait for future network transports.

- Capability matching
  - MVP: substring/tag match; later: semantic capability scoring.

- Timeouts and retries
  - Per-subtask timeout; coordinator reassigns to next candidate agent on timeout or error.

- Aggregation
  - For MVP, define simple merge strategies: concat, first-success, reduce-with-fn.

- Observability
  - Use tracing::span for job lifecycle. Emit structured events to logs. Provide counters for active jobs, errors, timeouts.

Testing Plan

- Unit tests for each module verifying happy-path and failure modes.
- Integration tests in tests/integration/ that use mock agents implementing a test Agent trait.
- End-to-end scenario tests that exercise negotiation, reassignment on timeouts, and result aggregation.
- Coverage targets: aim for high coverage of coordinator logic.

Documentation & Examples

- Add examples/ or examples/orchestration.rs demonstrating:
  - Registering two or more agents
  - Starting a job that requires both agents
  - Receiving aggregated result

- Update SPEC.md and QUICKSTART.md with a short section showing the new orchestration API.

Milestone Acceptance Criteria

- Milestone 1 accepted when interfaces and diagrams are approved.
- Milestone 2 accepted when core coordinator and messaging are implemented and compile with cargo build.
- Milestone 3 accepted when API example works end-to-end locally.
- Milestone 4 accepted when all tests pass (cargo test) and docs updated.

Risks & Mitigations

- Risk: Concurrency bugs and deadlocks. Mitigation: Keep design simple, use Tokio channels and timeouts, add stress tests.
- Risk: Scope creep to full distributed system. Mitigation: Keep distributed transports out of MVP; design for pluggability.
- Risk: Inconsistent agent semantics. Mitigation: Define clear capability schema and simple deterministic resolution rules.

Estimated Timeline

Total MVP effort: ~5–8 weeks for a small team or 3–4 weeks for a dedicated developer (estimates in days above per task).

Next Steps (Immediate)

1. Review & approve the proposed model and interfaces (Task 1.1).
2. Create design doc and lightweight sequence diagrams.
3. Kick off implementation of Agent Registry and InProcessRouter.

Contact / Ownership

- Recommended owner: core maintainer / someone familiar with ragent-core.

---

Generated by the RAgent planning process. Update this document as design decisions or scope change.
