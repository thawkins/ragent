Orchestration Design — Multi-Agent Orchestration (F6)

Overview

This document defines the core orchestration model and interfaces for Multi-Agent Orchestration (Feature F6). It captures the primary types, responsibilities, and sequence diagrams for synchronous and asynchronous orchestration flows. The goal is to provide a stable, testable interface surface for the coordinator, agent handles, messaging/router, and aggregation logic used by the ragent-core crate.

Principles

- Keep the core interfaces small and async-friendly (Tokio-based).
- Make the messaging/router pluggable so in-process channels can be replaced by network transports later.
- Keep agent capability discovery simple for MVP (tag-based matching) but design for extensibility.
- Ensure observability by emitting events/spans at key lifecycle points.

Key types & interfaces (sketches)

1) JobDescriptor
- id: String (UUID)
- required_capabilities: Vec<String>
- payload: String
- priority: Option<u8>
- metadata: Option<HashMap<String,String>>

2) TaskResult
- job_id: String
- status: TaskStatus (Success, Partial, Failed)
- parts: Vec<SubtaskResult>
- diagnostics: Option<String>

3) SubtaskResult
- agent_id: String
- status: TaskStatus
- payload: String
- latency_ms: u64

4) AgentHandle (trait)
- id(&self) -> &str
- capabilities(&self) -> &[String]
- async fn handle(&self, msg: OrchestrationMessage) -> anyhow::Result<String>

Purpose: An AgentHandle abstracts either a local in-process agent (responder closure) or a remote agent reachable via network transport.

5) AgentRegistry (trait / impl)
- async fn register(&self, agent: AgentEntry)
- async fn unregister(&self, agent_id: &str)
- async fn list(&self) -> Vec<AgentEntry>
- async fn match_agents(&self, required: &[String]) -> Vec<AgentEntry>

AgentEntry contains id, capabilities, and an AgentHandle (or a responder reference / transport address).

6) Router (trait)
- async fn send(&self, agent_id: &str, msg: OrchestrationMessage) -> anyhow::Result<String>
- Implementations: InProcessRouter (Tokio mpsc / direct responder call), RemoteRouter (gRPC/WebSocket adapter)

7) Coordinator (trait + concrete Coordinator)
- async fn start_job_sync(&self, desc: JobDescriptor) -> anyhow::Result<TaskResult>
- async fn start_job_async(&self, desc: JobDescriptor) -> anyhow::Result<JobHandle>
- async fn subscribe_job_events(&self, job_id: &str) -> Stream<Event>

Coordinator responsibilities:
- Accept JobDescriptor
- Match agents via AgentRegistry
- Decompose job into subtasks (MVP: broadcast same payload to matched agents)
- Assign subtasks via Router
- Monitor subtask completion, timeouts, failures
- Aggregate SubtaskResults into TaskResult using Aggregator
- Emit lifecycle events (JobStarted, SubtaskAssigned, SubtaskCompleted, JobCompleted)

8) Aggregator (trait)
- fn aggregate(&self, parts: &[SubtaskResult]) -> TaskResult
- Default strategies: concat_all, first_success, reduce_with_function (pluggable)

9) Negotiation & Conflict Resolution (MVP rules)
- Coordinator selects agents deterministically in matching order (registry ordering)
- Assignment is optimistic: send to multiple agents; first successful response can satisfy some job types
- Per-subtask timeout; on timeout, optionally reassign to next candidate
- For stateful conflicts (e.g., multiple agents editing same file) define deterministic merge strategy or surface to caller as conflict diagnostic

Event Model

Event {
  JobStarted { job_id, job_descriptor, timestamp },
  SubtaskAssigned { job_id, agent_id, subtask_id, timestamp },
  SubtaskCompleted { job_id, agent_id, subtask_id, status, latency_ms, timestamp },
  JobCompleted { job_id, status, result_summary, timestamp },
  JobFailed { job_id, error, timestamp },
}

Sequence Diagrams (ASCII)

Synchronous flow (start_job_sync):

User -> Coordinator: start_job_sync(JobDescriptor)
Coordinator -> Registry: match_agents(required_capabilities)
Registry -> Coordinator: [AgentEntry, ...]
Coordinator -> Router (parallel): send(agent, OrchestrationMessage)
Router -> Agent: deliver message (in-process or remote)
Agent -> Router: response
Router -> Coordinator: response
Coordinator -> Aggregator: aggregate(responses)
Aggregator -> Coordinator: TaskResult
Coordinator -> User: TaskResult

Asynchronous flow (start_job_async):

User -> Coordinator: start_job_async(JobDescriptor)
Coordinator -> Registry: match_agents(...)
Coordinator -> Router (parallel): send(...)
Coordinator -> JobStore: create JobHandle (job_id)
Coordinator -> User: JobHandle { job_id }
(Background) Router/Coordinator handles responses and publishes events
User -> Coordinator: subscribe_job_events(job_id) -> Stream<Event>

Timeouts and retries

- Each subtask send will have a per-agent timeout (configurable). On timeout Coordinator may retry with remaining candidates.
- Global job timeout is optional; Coordinator should allow callers to await with their own timeout.

Observability

- Use tracing spans around start_job_sync/start_job_async and per-subtask assignments.
- Emit structured events on EventBus for job lifecycle.
- Counters: active_jobs, job_failures, subtask_timeouts, subtask_successes

API examples (Rust pseudo-code)

let registry = AgentRegistry::new();
// register agents (in-process)
registry.register("agent-a", vec!["search"], Some(responder_a)).await;
registry.register("agent-b", vec!["compile"], Some(responder_b)).await;

let coord = Coordinator::new(registry.clone());
let desc = JobDescriptor { id: uuid::Uuid::new_v4().to_string(), required_capabilities: vec!["search".into()], payload: "find TODOs".into(), priority: None, metadata: None };
let result = coord.start_job_sync(desc).await?;

Implementation notes

- Place core interfaces in crates/ragent-core/src/orchestrator/mod.rs (or split into orchestrator/{api.rs, impl.rs}).
- Keep in-process Router simple (direct responder call) and implement AgentEntry.responder as Arc<dyn Fn(String)->BoxFuture<String>>.
- Use tokio::spawn for parallel subtask execution in Coordinator; join handles and collect results.
- Return deterministic ordering in aggregated result to simplify testing.

Acceptance criteria mapping

- Coordinator can accept a task and route subtasks to multiple agents: Coordinator.start_job_sync + AgentRegistry.match_agents + InProcessRouter.request_response
- Agents register capabilities and subscribe: AgentRegistry.register + match_agents
- Coordinator aggregates partial results: Aggregator implementation + start_job_sync returns composed TaskResult
- Observability: tracing spans + EventBus events emitted in Coordinator lifecycle
- Tests: unit tests for registry, router, coordinator happy and error paths; integration tests to simulate multiple agents

Next steps (implementation tasks)

- Create lightweight trait files and add tests for AgentRegistry and InProcessRouter (done: orchestrator/mod.rs contains a working MVP implementation)
- Add Aggregator trait and multiple strategies.
- Implement start_job_async + JobHandle and job event stream.
- Add CLI/TUI example demonstrating orchestration (ragent-tui or ragent-server).

Appendix: Minimal Rust trait sketches

trait AgentHandle: Send + Sync {
    fn id(&self) -> &str;
    fn capabilities(&self) -> &[String];
    fn handle(&self, msg: OrchestrationMessage) -> BoxFuture<'static, anyhow::Result<String>>;
}

trait Router: Send + Sync {
    fn send(&self, agent_id: &str, msg: OrchestrationMessage) -> BoxFuture<'static, anyhow::Result<String>>;
}

trait CoordinatorApi: Send + Sync {
    fn start_job_sync(&self, desc: JobDescriptor) -> BoxFuture<'static, anyhow::Result<TaskResult>>;
    fn start_job_async(&self, desc: JobDescriptor) -> BoxFuture<'static, anyhow::Result<JobHandle>>;
}

End of design document.
