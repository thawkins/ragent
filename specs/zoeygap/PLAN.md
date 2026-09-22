# Implementation Plan — Zoey Gap

## Overview

This plan implements the Zoey Gap specification in **7 incremental milestones**.
Each milestone delivers a standalone, testable feature set that builds on the
previous one, culminating in a full voice-first companion orchestration product.

**Architecture principle:** leverage existing ragent subsystems wherever
possible. New code is additive — new config fields, new tools, new agent
types, new HTTP routes — not replacements for existing infrastructure.

---

## Milestone Summary

| Milestone | Title                              | Key Deliverable                                          |
| --------- | ---------------------------------- | -------------------------------------------------------- |
| M1        | Voice I/O Foundation               | STT capture + TTS playback + voice/text mode toggling     |
| M2        | Coordinator Agent & Routing        | Built-in coordinator agent type + intent-to-companion routing |
| M3        | Companion Lifecycle & Skills       | Companion auto-loading, skill injection, parallel execution |
| M4        | Behaviour Interviews               | Guided Q&A prompt refinement                              |
| M5        | Audit Trail & Activity Dashboard   | Structured audit logging + queryable activity feed        |
| M6        | Telegram Bridge & Mobile Voice     | Bidirectional Telegram bot + mobile voice relay           |
| M7        | Polish, Integration & Documentation | End-to-end integration, CLI subcommand, docs, benchmarks |

---

## Tasks

| ID     | Title                                                   | Requirement  | Effort | Priority   | Dependencies |
| ------ | ------------------------------------------------------- | ------------ | ------ | ---------- | ------------ |
| T-001  | Add `voice` config block to `Config` struct              | FR-005       | S      | Critical   | —            |
| T-002  | Implement `VoiceConfig` with STT/TTS backend selection   | FR-005       | S      | Critical   | T-001        |
| T-003  | Create `ragent-voice` crate skeleton                     | FR-005       | S      | Critical   | T-002        |
| T-004  | Implement `SttBackend` trait + local subprocess backend  | FR-001       | M      | Critical   | T-003        |
| T-005  | Implement `TtsBackend` trait + local subprocess backend  | FR-002       | M      | Critical   | T-003        |
| T-006  | Implement OpenAI-compatible STT backend                  | FR-005       | M      | High       | T-004        |
| T-007  | Implement OpenAI-compatible TTS backend                  | FR-005       | M      | High       | T-005        |
| T-008  | Add voice input capture loop with mic recording          | FR-001       | L      | Critical   | T-004        |
| T-009  | Add audio playback with streaming TTS output             | FR-002, NFR-002 | L   | Critical   | T-005        |
| T-010  | Implement voice/text mode toggle (keyboard + CLI flag)   | FR-004       | S      | Critical   | T-008, T-009 |
| T-011  | Implement audio failure handling + text fallback          | FR-006       | S      | Critical   | T-008        |
| T-012  | Implement voice interruption (stop TTS on input)          | FR-020       | S      | High       | T-009        |
| T-013  | Implement voice session state persistence                | FR-021       | S      | Medium     | T-010        |
| T-014  | Implement VAD-based auto-segmentation                     | FR-023       | L      | Low        | T-008        |
| T-015  | Implement graceful degradation without audio backend      | FR-024, NFR-003 | S   | High       | T-003        |
| T-016  | Unit tests for STT/TTS backend trait mocking              | FR-001, FR-002 | M  | Critical   | T-004, T-005 |
| T-017  | Integration test: voice input → session message           | FR-001, FR-003 | M  | High       | T-008, T-010 |
| T-018  | Add `coordinator` to `KNOWN_AGENT_TYPES` + system prompt   | FR-007       | S      | Critical   | —            |
| T-019  | Extend `infer_agent_type` with companion awareness        | FR-008       | M      | Critical   | T-018        |
| T-020  | Implement companion registry (loaded custom agents index)  | FR-009       | M      | Critical   | T-018        |
| T-021  | Implement coordinator routing logic (intent → companion)  | FR-008       | L      | Critical   | T-019, T-020 |
| T-022  | Add `coordinator` agent system prompt with delegation rules | FR-007       | M      | Critical   | T-018        |
| T-023  | Unit tests for intent classification routing               | FR-008, NFR-004 | M  | Critical   | T-021        |
| T-024  | Auto-load companion skills on spawn                       | FR-012       | M      | High       | T-020        |
| T-025  | Wire coordinator to `team_create` + `team_spawn`            | FR-013       | M      | Critical   | T-021        |
| T-026  | Implement parallel companion execution with result streaming | FR-013, FR-014 | L | Critical   | T-025        |
| T-027  | Companion memory injection on spawn (`memory_recall`)      | FR-011       | S      | High       | T-020        |
| T-028  | Enforce companion permission rules during execution        | FR-018       | M      | High       | T-025        |
| T-029  | Integration test: coordinator → team → companion → result | FR-007, FR-013 | L  | Critical   | T-026        |
| T-030  | Implement `/interview <companion>` slash command           | FR-010       | L      | High       | T-020        |
| T-031  | Implement guided Q&A prompt refinement engine             | FR-010       | L      | High       | T-030        |
| T-032  | Persist updated companion profile to disk                  | FR-010       | S      | High       | T-031        |
| T-033  | Unit tests for interview prompt generation + persistence  | FR-010       | M      | High       | T-032        |
| T-034  | Create `audit_log` SQLite table + migration                | FR-016       | S      | High       | —            |
| T-035  | Implement `AuditEntry` struct + batched background writer   | FR-016, NFR-005 | M | High       | T-034        |
| T-036  | Wire audit logging into tool execution + team operations   | FR-016       | M      | High       | T-035        |
| T-037  | Implement `/audit` slash command + `GET /audit` endpoint    | FR-017       | M      | High       | T-036        |
| T-038  | Implement `POST /companions` + `GET /companions` endpoints  | FR-022       | M      | High       | T-020        |
| T-039  | Publish companion activity events to event bus + SSE       | FR-014       | S      | High       | T-026        |
| T-040  | Unit tests for audit log write + query                     | FR-016, FR-017 | M   | High       | T-037        |
| T-041  | Implement Telegram bot polling listener                     | FR-015       | L      | Medium     | T-021        |
| T-042  | Route incoming Telegram messages to coordinator            | FR-015       | M      | Medium     | T-041        |
| T-043  | Send coordinator/companion responses back via Telegram     | FR-015       | S      | Medium     | T-042        |
| T-044  | Add voice relay: Telegram voice message → STT → coordinator | FR-015       | L      | Low        | T-042, T-008 |
| T-045  | Integration test: Telegram → coordinator → response cycle  | FR-015       | M      | Medium     | T-043        |
| T-046  | Add `ragent companion` CLI subcommand / `--coordinator` flag | FR-021       | S   | High       | T-021        |
| T-047  | End-to-end integration test: voice → coordinator → companion → voice output | FR-001, FR-007, FR-013, FR-002 | L | Critical | T-017, T-029 |
| T-048  | Performance benchmark: voice round-trip latency             | NFR-001      | M      | Medium     | T-047        |
| T-049  | Performance benchmark: coordinator routing overhead         | NFR-004      | S      | Medium     | T-023        |
| T-050  | Update SPEC.md, README.md, QUICKSTART.md, CHANGELOG.md     | —            | S      | Low        | T-047        |
| T-051  | Update TUI-QUICKSTART.md with voice + companion commands    | —            | S      | Low        | T-047        |

---

## Milestone Breakdown

### Milestone 1 — Voice I/O Foundation

**Goal:** Users can talk to ragent and hear responses. Text mode continues to
work unchanged.

**Tasks:** T-001 through T-017

**Deliverable:** A `ragent-voice` crate with configurable STT/TTS backends,
voice/text mode toggling, streaming TTS playback, and graceful degradation.

**Testable outcome:** Start ragent with `--voice` flag, speak a prompt, hear the
spoken response. Toggle voice off with a keyboard shortcut and continue in text
mode.

**Key design decisions:**

- STT/TTS backends are trait-based (`SttBackend`, `TtsBackend`) with two
  implementations: local subprocess (no internet needed) and OpenAI-compatible
  API.
- Audio I/O uses a lightweight cross-platform library (e.g. `cpal` for capture,
  `rodio` for playback) or shelling out to `arecord`/`aplay`/`ffmpeg`.
- Voice config lives in `ragent.json` under a new `voice` block:

```json
{
  "voice": {
    "stt": { "backend": "local", "command": "whisper" },
    "tts": { "backend": "local", "command": "piper" },
    "vad": { "enabled": false }
  }
}
```

### Milestone 2 — Coordinator Agent & Routing

**Goal:** A built-in coordinator agent that receives requests and routes them
to the most appropriate companion.

**Tasks:** T-018 through T-023

**Deliverable:** A `coordinator` agent type with a system prompt designed for
delegation, an extended `infer_agent_type` classifier that considers registered
companions, and a companion registry built from loaded custom agent definitions.

**Testable outcome:** Create a custom agent (e.g. `researcher.json`), start
ragent with `--agent coordinator`, type a research request, and verify the
coordinator delegates to the `researcher` companion via `team_spawn`.

### Milestone 3 — Companion Lifecycle & Skills

**Goal:** Companions are spawned as team members with their declared skills
auto-loaded, permissions enforced, and results streamed back.

**Tasks:** T-024 through T-029

**Deliverable:** Full companion lifecycle: coordinator creates a team, spawns
companions with skills injected, enforces companion-scoped permissions, and
streams results via the event bus.

**Testable outcome:** Define a companion with a specific skill pack and a
restricted permission set. Issue a request via the coordinator. Verify the
companion has the skill tools available, respects the permission boundary, and
streams activity events.

### Milestone 4 — Behaviour Interviews

**Goal:** Users can refine companion behaviour through guided interviews without
editing prompts.

**Tasks:** T-030 through T-033

**Deliverable:** A `/interview` slash command that conducts guided Q&A and
updates the companion's system prompt, persisting it to disk.

**Testable outcome:** Run `/interview my-researcher`, answer questions, verify
the companion's `.json` profile is updated on disk.

### Milestone 5 — Audit Trail & Activity Dashboard

**Goal:** All agent activity is logged to a structured audit trail, queryable via
CLI and HTTP.

**Tasks:** T-034 through T-040

**Deliverable:** An `audit_log` SQLite table with batched background writes,
`/audit` slash command, `GET /audit` HTTP endpoint, `POST /companions` and
`GET /companions` HTTP endpoints, and companion activity events published to the
SSE stream.

**Testable outcome:** Run several companion tasks, query `/audit` and verify all
actions are logged with timestamps, agent names, and outcomes. Connect an SSE
client and see live activity events.

### Milestone 6 — Telegram Bridge & Mobile Voice

**Goal:** Users can interact with the coordinator from their phone via Telegram,
including voice messages.

**Tasks:** T-041 through T-045

**Deliverable:** A Telegram bot listener that polls for incoming messages,
routes them to the coordinator, and sends responses back. Voice messages are
transcribed via STT before routing.

**Testable outcome:** Configure a Telegram bot in `ragent.json`, send a text
message from Telegram, receive a response. Send a voice message and receive a
transcribed + processed response.

### Milestone 7 — Polish, Integration & Documentation

**Goal:** Everything works end-to-end, benchmarks pass, documentation is
complete.

**Tasks:** T-046 through T-051

**Deliverable:** `ragent companion` CLI subcommand, end-to-end integration test
(voice → coordinator → companion → voice output), performance benchmarks, and
updated documentation.

**Testable outcome:** Run `ragent companion`, speak a request, hear the
coordinator delegate to a companion, hear the companion's result spoken back.
Benchmarks confirm <3s voice latency and <200ms routing overhead.

---

## Dependency Graph

```
M1 (Voice I/O)
 ├── T-001 → T-002 → T-003 → T-004/T-005 → T-008/T-009 → T-010
 │                                         → T-011, T-012, T-013, T-014, T-015
 │                                         → T-016, T-017 (tests)
 │
M2 (Coordinator)
 ├── T-018 → T-019 → T-020 → T-021 → T-022 → T-023 (tests)
 │
M3 (Companion Lifecycle)   [depends on M2]
 ├── T-024 → T-025 → T-026 → T-027, T-028 → T-029 (tests)
 │
M4 (Interviews)             [depends on M2]
 ├── T-030 → T-031 → T-032 → T-033 (tests)
 │
M5 (Audit & Activity)       [depends on M3]
 ├── T-034 → T-035 → T-036 → T-037, T-038, T-039 → T-040 (tests)
 │
M6 (Telegram Bridge)        [depends on M2 + M1]
 ├── T-041 → T-042 → T-043 → T-044 → T-045 (tests)
 │
M7 (Polish)                 [depends on all]
 ├── T-046, T-047, T-048, T-049, T-050, T-051
```

## Effort Summary

| Effort | Count |
| ------ | ----- |
| S      | 18    |
| M      | 22    |
| L      | 11    |
| **Total** | **51** |

## Priority Summary

| Priority   | Count |
| ---------- | ----- |
| Critical   | 20    |
| High       | 20    |
| Medium     | 7     |
| Low        | 4     |

## New Crate

This plan introduces one new workspace crate:

- **`ragent-voice`** — STT/TTS backend abstractions, audio capture/playback,
  voice mode management, and voice session state persistence.

The crate follows the existing workspace pattern: `crates/ragent-voice/` with
`Cargo.toml`, `src/lib.rs`, and `tests/`. It depends on `ragent-config` (for
`VoiceConfig`), `ragent-types` (for events), and optionally `ragent-agent`
(for session integration).