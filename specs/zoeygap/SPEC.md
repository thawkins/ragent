---
status: draft
spec_id: zoeygap
title: "Zoey Gap — Voice-First Companion Orchestration for ragent"
created: 2026-01-15
---
# Zoey Gap — Voice-First Companion Orchestration

## Overview

This specification defines the integration of Zoey OS–style capabilities into
ragent, leveraging as many existing ragent subsystems as possible. The core
goals are:

1. **Voice interaction** (required) — speech-to-text input and text-to-speech
   output so users can talk to ragent and hear responses.
2. **Companion orchestration** — a lead "coordinator" agent that receives user
   intent, routes work to specialised companion agents, and aggregates results
   — analogous to Zoey OS's "Zoey delegates to companions, bots execute" model.
3. **Leverage existing ragent features** — teams/swarm, custom agents, skills,
   memory, MCP, HTTP/SSE server, cron, Gmail/messaging, VCS tools, permissions,
   prompt optimization, and session management — rather than building parallel
   systems.

3D visualisation is explicitly **out of scope**.

## Background — Zoey OS Feature Analysis

Zoey OS (https://zoeyos.com) is a voice-first AI operating system built around
the concept of a central coordinator ("Zoey") that delegates to specialised
"companions" which in turn dispatch "bots" for parallel execution. Key features
identified from the public site:

| Zoey OS Feature         | Description                                                                      |
| ----------------------- | -------------------------------------------------------------------------------- |
| Voice-first interaction | Talk to Zoey; voice/text switching; one continuous conversation                  |
| Persistent memory       | Companions remember preferences, patterns, context; never resets                 |
| Companion delegation    | Zoey coordinates; companions specialise; bots execute; ~200 agents               |
| Companion customisation | Define role, personality, skills, tools per companion (up to 20)                 |
| Skill execution         | Give a companion a skill; it runs the whole workflow (email, CRM…)              |
| Integration-first       | Work inside existing tools (Drive, Notion, GitHub, Telegram, Slack)              |
| Behaviour interviews    | Refine companion behaviour via guided Q&A; no prompt editing                     |
| Activity streaming      | Watch work happen in real time; bidirectional streaming                          |
| Mobile via Telegram     | Telegram bridge                                                                  |
| Portals                 | Share your world with trusted people; audit visits                               |
| Workflow automation     | Unify data, systems, workflows                                                   |
| Security & compliance   | Row-level security, encrypted credentials, audit logs                            |

### Mapping to Existing ragent Capabilities

| Zoey OS Feature          | ragent Equivalent (existing or partial)                                      |
| ------------------------ | ---------------------------------------------------------------------------- |
| Companion delegation     | Teams & Swarm (`team_create`, `team_spawn`, `team_task_*`, `/swarm`) |
| Companion customisation  | Custom agents (`CustomAgentDef`, OASF JSON / Markdown profiles)            |
| Skill execution          | Skills system (`SkillPack`, `skill_manage` tool, `/skill` commands)    |
| Persistent memory        | Memory system (file blocks, structured SQLite, semantic search)              |
| Integration-first        | MCP client (external tool servers), Gmail,`send_channel_message`           |
| GitHub integration       | VCS tools (GitHub issues, PRs, pipelines)                                    |
| Scheduling / automation  | Cron scheduler (`cron_add`, `cron_list`, etc.)                           |
| Activity streaming       | HTTP/SSE server (`/events` SSE stream, `event_to_sse`)                   |
| Behaviour interviews     | Prompt optimization (`/opt`, 12 frameworks)                                |
| Workflow automation      | Sub-agents (`new_task`, `wait_tasks`), orchestrator module               |
| Session persistence      | Session management (SQLite history, resume, export)                          |
| Configuration            | `ragent.json` / `ragent.jsonc` configuration                             |

### Gaps Requiring New Work

| Gap                            | What's Missing                                                         |
| ------------------------------ | ---------------------------------------------------------------------- |
| Voice interaction              | No STT input, no TTS output, no voice/text mode switching              |
| Coordinator routing            | No "Zoey" — a lead agent that detects intent and routes to companions |
| Companion behaviour interviews | No guided Q&A to refine agent profiles without editing prompts         |
| Activity dashboard             | No real-time text-based activity feed beyond the TUI log panel         |
| Telegram bridge                | No bidirectional Telegram bot integration (only outbound messages)     |
| Audit log                      | No structured audit trail of all agent actions                         |

## Requirements

Requirements are written in EARS (Easy Approach to Requirements Syntax)
notation. Each requirement uses one of the five EARS templates:

- **Ubiquitous:** "The system shall `<action>`."
- **Event-driven:** "When `<trigger>`, the system shall `<action>`."
- **State-driven:** "While `<state>`, the system shall `<action>`."
- **Optional:** "Where `<feature>` is enabled, the system shall `<action>`."
- **Unwanted:** "If `<unwanted condition>`, the system shall `<action>`."

### FR-001 — Voice Input Capture (Event-Driven)

> When the user activates voice input mode, the system shall capture audio from
> the microphone, transcribe it to text via a configurable speech-to-text
> backend, and submit the transcribed text as a user message to the active
> session.

### FR-002 — Voice Output (Event-Driven)

> When the active session produces an assistant text response, the system
> shall synthesise speech from the response text via a configurable
> text-to-speech backend and play the audio through the system audio output,
> provided voice output mode is active.

### FR-003 — Voice/Text Mode Switching (State-Driven)

> While voice interaction mode is active, the system shall accept both spoken
> and typed input interchangeably within the same session, maintaining a single
> continuous conversation context.

### FR-004 — Voice Mode Toggle (Optional)

> Where voice interaction is enabled in configuration, the system shall provide
> a toggle (keyboard shortcut and CLI flag) to enable or disable voice input
> and voice output independently.

### FR-005 — Voice Backend Configuration (Ubiquitous)

> The system shall support configurable speech-to-text and text-to-speech
> backends specified in `ragent.json`, including at minimum a local subprocess
> backend (e.g. `whisper` CLI for STT, `espeak`/`piper` for TTS) and an
> OpenAI-compatible API backend.

### FR-006 — Audio Capture Failure Handling (Unwanted)

> If the audio capture device is unavailable or transcription fails, the system
> shall display a user-visible error message, automatically fall back to text
> input mode, and shall not crash or hang.

### FR-007 — Coordinator Agent Role (Ubiquitous)

> The system shall provide a built-in "coordinator" agent type (analogous to
> Zoey OS's "Zoey") that receives user requests, detects intent, and delegates
> work to specialised companion agents using the existing team and sub-agent
> infrastructure.

### FR-008 — Intent Detection and Routing (Event-Driven)

> When the coordinator agent receives a user request, the system shall classify
> the request intent using the existing `infer_agent_type` classifier (extended
> with companion-awareness) and route the request to the most appropriate
> registered companion agent or team.

### FR-009 — Companion Agent Definitions (Ubiquitous)

> The system shall treat custom agent definitions (OASF JSON / Markdown
> profiles loaded from `.ragent/agents/`) as "companions" and shall expose them
> to the coordinator for routing, including their declared skills, tools, and
> permissions.

### FR-010 — Companion Behaviour Interviews (Event-Driven)

> When the user issues a `/interview <companion>` command, the system shall
> conduct a guided question-and-answer session that refines the companion's
> system prompt based on user responses, persisting the updated profile to disk
> without requiring manual prompt editing.

### FR-011 — Persistent Companion Memory (Ubiquitous)

> The system shall provide each companion agent with access to the existing
> structured memory system (`memory_store`, `memory_recall`) so that preferences,
> patterns, and context accumulate across sessions without reset.

### FR-012 — Skill Assignment to Companions (Event-Driven)

> When a companion agent is spawned, the system shall automatically load any
> skills declared in the companion's profile using the existing skill system
> (`SkillPack` / `skill_manage`), making those skill tools and prompts available
> to the companion's session.

### FR-013 — Parallel Companion Execution (State-Driven)

> While a team is active, the system shall allow multiple companion agents to
> execute tasks in parallel using the existing `team_spawn` and sub-agent
> (`new_task`) infrastructure, streaming results back as they complete.

### FR-014 — Real-Time Activity Stream (Event-Driven)

> When any companion or sub-agent performs an action, the system shall publish
> an event to the existing event bus and SSE stream (`/events`) so that external
> clients and the TUI can display real-time activity.

### FR-015 — Telegram Bridge (Optional)

> Where Telegram channel configuration is present in `ragent.json`, the system
> shall accept incoming messages from a Telegram bot as user input to the
> coordinator agent and shall send coordinator/companion responses back through
> the existing `send_channel_message` infrastructure, enabling mobile
> interaction.

### FR-016 — Audit Trail (Ubiquitous)

> The system shall log every coordinator delegation, companion action, and tool
> execution to a structured audit table in SQLite, capturing timestamp, agent
> name, action type, and outcome, so that all agent activity is traceable.

### FR-017 — Audit Query (Event-Driven)

> When the user issues an `/audit` command or queries the
> `GET /audit` HTTP endpoint, the system shall return a filterable, paginated
> list of audit log entries.

### FR-018 — Permission Enforcement for Companions (State-Driven)

> While a companion agent is executing, the system shall enforce the
> companion's declared permission rules (from its OASF profile) in addition to
> global permission rules, ensuring that companions cannot exceed their
> authorised tool or file scope.

### FR-019 — Voice Output Interruption (Unwanted)

> If the user begins speaking or presses a key while TTS audio is playing, the
> system shall immediately stop the audio playback and accept the new input,
> preventing overlapping audio and unresponsive states.

### FR-020 — Voice Session Persistence (State-Driven)

> While a voice-mode session is active, the system shall persist the
> voice/text mode flags to the session record so that resuming the session
> restores the previous voice interaction state.

### FR-021 — Coordinator CLI Subcommand (Ubiquitous)

> The system shall provide a `ragent companion` CLI subcommand (or
> `--coordinator` flag) that launches a session with the coordinator agent,
> automatically enabling team infrastructure and companion routing.

### FR-022 — HTTP API for Companion Management (Event-Driven)

> When a client sends `POST /companions` with a companion definition, the
> system shall register the companion, make it available to the coordinator for
> routing, and return its identifier; `GET /companions` shall list all registered
> companions with their current status.

### FR-023 — Voice Activity Detection (Optional)

> Where voice activity detection (VAD) is enabled in configuration, the system
> shall use a VAD algorithm to automatically segment speech and submit complete
> utterances without requiring manual push-to-talk activation.

### FR-024 — Graceful Degradation Without Audio (Unwanted)

> If no audio backend is available at startup, the system shall log a warning,
> disable voice features, and continue operating in text-only mode without
> error.

## Non-Functional Requirements

### NFR-001 — Voice Latency

> The system shall achieve end-to-end voice input latency (from end of speech
> to transcribed text appearing in the session) of less than 3 seconds when
> using a local STT backend.

### NFR-002 — TTS Streaming

> The system shall stream TTS audio output as it is generated, not waiting for
> the full response to be synthesised before playback begins.

### NFR-003 — No External Dependencies for Default Voice

> The default local STT/TTS backends shall work without internet access and
> without cloud API keys, using subprocess-based tools that can be installed via
> standard package managers.

### NFR-004 — Companion Routing Overhead

> The coordinator's intent classification and routing decision shall complete
> in less than 200ms (excluding LLM call time) so that delegation does not
> introduce perceptible latency.

### NFR-005 — Audit Log Performance

> The audit log write shall not block the agent loop; writes shall be
> batched or performed on a background task.

## Scope Exclusions

- **3D visualisation** — explicitly out of scope per the feature brief.
- **Custom integrations** — no Python or visual builder for new integrations;
  MCP and existing tool systems cover integration needs.
- **Multi-seat tenancy** — no per-team-member worlds, shared knowledge, or
  role-based multi-tenant permissions. ragent's HTTP server already supports
  bearer-token auth; a full multi-tenant identity system is out of scope.
- **Data ownership / four-tier privacy** — no four-tier privacy model or
  "never train on data" guarantee is specified. The existing permission system
  and encrypted credentials remain.
- **Custom themes** — no colours, layouts, or orb style customisation.
- **Marketplace** — no marketplace for buying/selling companion templates,
  themes, or voice packs.
- **Claude Code terminal / codebase visualisation** — no codebase visualisation;
  engineering task dispatch and GitHub sync are covered by existing VCS tools
  and coordinator infrastructure.
- **Visual workflow builder** — workflow automation leverages existing skills
  and cron; a GUI builder is out of scope.