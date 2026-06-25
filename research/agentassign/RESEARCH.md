---
---
name: agentassign
title: "\"research"
topic: "\"research how we can change the function of /swarm so that relevant agent types are assigned for each sunagent that matches the nature of the task assigned to the agent, look at stratergies for selecting an agent type based on the description of the task being assigned\""
status: complete
created: 2026-06-25T05:54:55.585527955+00:00
modified: 2026-06-25T05:54:55.595089060+00:00
sources: 0 # see sources/ subdirectory
---

---

# Title: "research

## Topic

"research how we can change the function of /swarm so that relevant agent types are assigned for each sunagent that matches the nature of the task assigned to the agent, look at stratergies for selecting an agent type based on the description of the task being assigned"

## Summary

The sources collectively describe a landscape of multi-agent orchestration strategies for assigning specialized agent types to tasks based on task descriptions. External references highlight three primary approaches: dynamic role reassignment based on task requirements and performance (Swarms API AgentRearrange) [#1], local agent-to-agent handoff decisions driven by declared capabilities and shared context (AutoGen Swarm) [#4], and skill-based automatic delegation to the most qualified agent (Swarms v10 SkillOrchestra) [#5]. Local project documentation describes a multi-agent communication stack spanning Teams/Swarm (file-backed mailboxes), sub-agent tasks, and a capability-based orchestrator [#14], with related work on performance remediation [#6], whitespace-tolerant editing [#13], Kimi K2.6 swarm integration [#11], and comparative analysis against systems that use 100+ specialized agents [#12]. A directly relevant in-project spec stub (`specs/agentassign`) [#19] exists for capability-based agent selection.

## Findings

### Finding 1

Swarms API's `AgentRearrange` swarm type implements dynamic role reassignment where agents are reassigned to different roles based on task requirements, performance metrics, or changing circumstances, with use cases including flexible task allocation based on agent strengths [#1].

### Finding 2

AutoGen's `Swarm` team pattern uses a handoff mechanism where each agent locally decides which other agent to hand off the task to based on capabilities, via `HandoffMessage` signals in a shared group-chat context — this is explicitly a "local decision about task planning, rather than relying on a central orchestrator" [#4].

### Finding 3

Swarms v10 introduces `SkillOrchestra`, "a new routing primitive that automatically delegates tasks to the most qualified agent based on defined skills" — this is a direct prior-art match for selecting agent type by task description [#5].

### Finding 4

Swarms v10's HierarchicalSwarm adds an `agent_as_judge=True` mode that scores each worker's output across five dimensions (task adherence, accuracy, depth, clarity, swarm contribution) and produces a structured report with per-agent scores, enabling performance-driven reassignment [#5].

### Finding 5

Academic work on swarm robotics (Al-Buraiki & Payeur, 2019) frames agent-task assignation as the problem of matching target/task characteristics to specialized agent capabilities, providing a formal basis for capability-based selection [#2].

### Finding 6

The ragent project already maintains a `sub-agent` system whose overhaul introduced "autonomous task spawning" where agents can create and assign tasks to other agents autonomously without manual wiring by the orchestrator [#5, #15].

### Finding 7

The ragent communication stack (described in `COMMSPLAN.md`) identifies three subsystems — Teams/Swarm (file-backed mailboxes + `tasks.json` + `EventBus`), Sub-agent tasks (`TaskManager`), and an Orchestrator that performs "capability-based in-process / HTTP routing" — providing an existing substrate to plug task-description-driven selection into [#14].

### Finding 8

The ragent project has a dedicated spec stub `specs/agentassign` [#19], suggesting that capability/task-based agent assignment is already a recognized design surface in the project.

### Finding 9

The `SubAgentControls` spec [#18] is referenced alongside `agentassign` [#19] and `researchsystem` [#20], implying a control plane for assigning sub-agents based on task semantics already exists or is being scoped.

### Finding 10

Comparative analysis with Eve V2 Unleashed shows that alternative systems use "112 specialized agents" keyed to distinct capabilities, reinforcing the demand-side pattern of mapping tasks → specialist agent types [#12].

### Finding 11

The Kimi K2.6 Agent Swarm integration research notes Moonshot's thesis that "single-agent sequential execution model hits a structural ceiling" and that K2.6 introduces "Claw Groups (heterogeneous swarms)" and "Document-to-Skill Conversion" — both directly relevant to the question of mapping a task description to a specialist agent type [#11].

### Finding 12

The AutoGen docs warn that Swarm handoff relies on the underlying model supporting tool calling, and that `parallel_tool_calls=False` should be set to avoid unexpected behaviour when multiple candidate agents match — a relevant constraint for any task-description-driven selector [#4].

### Finding 13

The `APERFPLAN.md` performance review notes that `Config::load()` is called 3–4× per `process_user_message` and `TeamStore::load()` 5× per `team_task_claim`; any new selector must avoid amplifying these redundant-load patterns [#6].

### Finding 14

Local specs `researchsystem` [#20] and `selectConfigedProvider` [#21] suggest the codebase already has related routing-style modules that an agent-type selector could share infrastructure with (e.g., similar config-loading and capability-lookup patterns).

### Finding 15

The AgentRearrange documentation positions dynamic role reassignment as best suited to "adaptive content creation workflows," "dynamic project management," and "performance optimization in multi-agent systems" — concrete target use cases for a `/swarm` redesign [#1].

## In-Project Cross-References

| Path | Relevance |
|------|-----------|
| `COMMSPLAN.md` | describes the three communication subsystems (Teams/Swarm, Sub-agent tasks, Orchestrator) and explicitly notes the orchestrator performs capability-based in-process/HTTP routing, which is the most directly relevant substrate for task-description-driven agent selection. |
| `APERFPLAN.md` | performance remediation plan enumerating redundant I/O patterns (Config/TeamStore reloads, mailbox re-serialization) that any new agent-type selector must not amplify. |
| `KIMIRESEARCH.md` | integration research for Kimi K2.6, documenting "Claw Groups (heterogeneous swarms)" and "Document-to-Skill Conversion" as prior art for mapping task descriptions to specialized agents. |
| `EAVECOMP.md` | comparative analysis noting Eve V2 Unleashed uses "112 specialized agents," evidencing the demand for capability-keyed specialist routing. |
| `OCCOMP.md` | comparative analysis table positioning ragent as single-user / task-coordination-only versus multi-agent platforms; informs scope of `/swarm` selection. |
| `README.md` | confirms ragent's multi-provider LLM support and ~111-tool taxonomy that a capability model would need to index. |
| `SPEC.md` | main ragent technical specification; the canonical home for any new selector's design constraints. |
| `QUICKSTART.md` | quickstart that lists current agent orchestration features; useful baseline for documenting `/swarm` behavior changes. |
| `CHANGELOG.md` | tracks ongoing rework of the research/synthesis pipeline (sub-agent spawning, event registry) that intersects with agent-type selection. |
| `WSPLAN.md` | remediation plan for edit-tool whitespace matching; relevant if the selector's tool-selection metadata includes edit-tool capabilities. |
| `specs/agentassign` | directly named spec stub for agent assignment; the natural target location for the proposed `/swarm` redesign. |
| `specs/SubAgentControls` | spec stub for sub-agent control surfaces, adjacent to `agentassign`. |
| `specs/researchsystem` | spec stub for the research orchestration subsystem, which is a primary consumer of task-description-driven agent selection. |
| `specs/selectConfigedProvider` | spec stub for provider selection, indicating an existing pattern for "choose X by descriptor" routing that an agent-type selector could mirror. |
| `specs/AgentPerf` | performance spec, relevant for bounding selector latency/cost budgets. |
| `specs/AzureResource` | provider spec, relevant only insofar as a selector may need to consider provider constraints when picking agent types. |

## Open Questions

- What is the canonical task-description input format for `/swarm` today (free-form string, structured `Task` object, both)? The sources show multiple precedent formats (Swarms API JSON, AutoGen handoff tool calls, SkillOrchestra skill declarations) but the current ragent interface is not directly documented.
- Where does the agent-type registry live — is there a single source of truth (e.g., a `tools.yaml` / `skills.yaml` / capability table), or is capability inferred from each agent's `system_prompt` and tool set?
- Should selection be done by a centralized router (like the current "Orchestrator" capability-based routing in `COMMSPLAN.md`), or by per-agent handoffs (AutoGen Swarm style), or by a judge-driven reassignment loop (HierarchicalSwarm `agent_as_judge`)?
- How should ambiguous tasks (no clear best agent) be handled — fall back to a default agent, ask the user, or escalate to a meta-agent?
- How does the selector interact with the existing `TaskManager` task registry and the `EventBus` event flow described in `COMMSPLAN.md`?
- What is the performance budget for selection (the `APERFPLAN.md` findings suggest I/O amplification is a real risk if the selector re-loads config per task)?
- How does Kimi K2.6's "Document-to-Skill Conversion" actually work, and is any of that technique transferable to mapping free-form task descriptions to ragent's specialist profiles?
- Does the `specs/agentassign` stub already contain partial design (acceptance criteria, API surface) that this research should extend rather than reinvent?
- Should agent-type selection be model-driven (an LLM classifies the task) or deterministic (keyword/embedding/rule-based)? The AutoGen pattern relies on the model's tool-calling capability; SkillOrchestra uses declared skills — both are viable and the choice has cost/latency implications.
- How will performance-based reassignment (à la AgentRearrange / HierarchicalSwarm judge) be incorporated, or is the v1 scope strictly capability-based static selection?
- What testing/evaluation harness exists to validate that the right agent type was chosen (the AutoGen and Swarms v10 docs do not deeply cover eval methodology)?

## References Index

| # | Type | Path/URL | Title | Relevance | Captured |
|---|------|----------|-------|-----------|----------|
| 1 | web | https://docs.swarms.ai/docs/documentation/multi-agent/agent_rearrange | > ## Documentation Index | — | 2026-06-25T05:52:46.009340815+00:00 |
| 2 | web | https://www.site.uottawa.ca/~ppayeur/SMART/PAPERS/SYSCON2019.pdf | %PDF-1.5 | — | 2026-06-25T05:52:53.164379152+00:00 |
| 3 | web | https://pmc.ncbi.nlm.nih.gov/articles/PMC7748156 | [ Skip to main content ][1] | — | 2026-06-25T05:52:54.280271753+00:00 |
| 4 | web | https://microsoft.github.io/autogen/stable//user-guide/agentchat-user-guide/swarm.html | [Skip to main content][1] | — | 2026-06-25T05:52:54.701796455+00:00 |
| 5 | web | https://medium.com/@kyeg/introducing-swarms-v10-async-sub-agents-skillorchestra-and-more-6f0754734677 | [Sitemap][1] | — | 2026-06-25T05:52:55.030537776+00:00 |
| 6 | local | APERFPLAN.md | APERFPLAN.md | 500 match(es) on: agent, an, for, …(+19) — "# APERFPLAN — Agent & Team Performance Improvement Plan" | 2026-06-25T05:53:23.378296753+00:00 |
| 7 | local | CHANGELOG.md | CHANGELOG.md | 500 match(es) on: an, change, on, …(+26) — "# Changelog" | 2026-06-25T05:53:23.379350771+00:00 |
| 8 | local | QUICKSTART.md | QUICKSTART.md | 500 match(es) on: agent, an, at, …(+21) — "# Ragent Quick Start Guide" | 2026-06-25T05:53:23.380273900+00:00 |
| 9 | local | SPEC.md | SPEC.md | 500 match(es) on: to, agent, on, …(+21) — "<div style="page-break-after: always; text-align: center; padding-top: 15em;">" | 2026-06-25T05:53:23.381515981+00:00 |
| 10 | local | OCCOMP.md | OCCOMP.md | 358 match(es) on: agent, an, at, …(+19) — "# OpenClaw vs ragent — Comparative Analysis" | 2026-06-25T05:53:23.382256823+00:00 |
| 11 | local | KIMIRESEARCH.md | KIMIRESEARCH.md | 334 match(es) on: agent, at, for, …(+19) — "# Kimi K2.6 Agent Swarm — Integration Research for ragent" | 2026-06-25T05:53:23.383001721+00:00 |
| 12 | local | EAVECOMP.md | EAVECOMP.md | 327 match(es) on: agent, an, at, …(+19) — "# Comparative Analysis: ragent vs Eve Agent V2 Unleashed" | 2026-06-25T05:53:23.383641163+00:00 |
| 13 | local | WSPLAN.md | WSPLAN.md | 301 match(es) on: an, at, on, …(+20) — "# WSPLAN — 'old_str not found' Remediation Plan" | 2026-06-25T05:53:23.384295204+00:00 |
| 14 | local | COMMSPLAN.md | COMMSPLAN.md | 238 match(es) on: agent, an, at, …(+23) — "# COMMSPLAN.md — Agent Communication Remediation Plan" | 2026-06-25T05:53:23.384820450+00:00 |
| 15 | local | README.md | README.md | 215 match(es) on: agent, an, for, …(+20) — "# ragent" | 2026-06-25T05:53:23.385297357+00:00 |
| 16 | spec | AgentPerf | AgentPerf | Spec AgentPerf under specs/ | 2026-06-25T05:53:23.390024243+00:00 |
| 17 | spec | AzureResource | AzureResource | Spec AzureResource under specs/ | 2026-06-25T05:53:23.390025411+00:00 |
| 18 | spec | SubAgentControls | SubAgentControls | Spec SubAgentControls under specs/ | 2026-06-25T05:53:23.390025734+00:00 |
| 19 | spec | agentassign | agentassign | Spec agentassign under specs/ | 2026-06-25T05:53:23.390025938+00:00 |
| 20 | spec | researchsystem | researchsystem | Spec researchsystem under specs/ | 2026-06-25T05:53:23.390026098+00:00 |
| 21 | spec | selectConfigedProvider | selectConfigedProvider | Spec selectConfigedProvider under specs/ | 2026-06-25T05:53:23.390026558+00:00 |
