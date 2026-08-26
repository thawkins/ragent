# ragent-storage

SQLite-backed storage for ragent: sessions, messages, structured memories,
activity logs, snapshots, encrypted credentials, and provider auth tokens.
Provides the persistent storage layer consumed by the agent, research, and
tool crates.

## Workspace Dependencies

- ragent-types

## External Dependencies

- rusqlite
- serde
- serde_json
- anyhow
- thiserror
- tracing
- chrono
- uuid
- sha2 (credential encryption)
- tokio (async wrappers)
- parking_lot

## Public API (crate root)

### Modules

- **activity_log** — Activity-log event schema and append-only storage (event sourcing for sessions/runs).
- **storage** — Core SQLite storage facade (`Storage` struct).

### Structs

- **Storage** (struct) — Main storage handle wrapping a SQLite connection; provides session, message, memory, snapshot, and credential persistence.
  - Session/message methods: `create_session`, `get_session`, `list_sessions`, `delete_session`, `add_message`, `get_messages`, `update_message`, `delete_message`.
  - Structured memory methods: `store_memory`, `get_memory`, `search_memories`, `delete_memory`, `update_memory_access`.
  - Snapshot methods: `create_snapshot`, `get_snapshot`, `restore_snapshot`, `list_snapshots`.
  - Credential methods: `store_credential`, `get_credential`, `delete_credential` (encrypted).
  - Provider auth methods: `store_provider_auth`, `get_provider_auth`, `delete_provider_auth`.
  - Settings methods: `get_setting`, `set_setting`, `delete_setting`.
  - Schema methods: `schema_version`, `migrate`.

### Module: activity_log

Activity-log event sourcing for sessions/runs — immutable, self-describing
execution facts appended before being projected into user-facing state.

- **ActivityLog** (struct) — Append-only event log store.
  - `append(event)` — Append an activity event.
  - `read_events(run_id)` — Read all events for a run.
  - `read_events_from(run_id, seq)` — Read events from a sequence number.
  - `latest_seq(run_id)` — Get the latest sequence number for a run.
  - `truncate(run_id, seq)` — Truncate events after a sequence number (rollback).
- **ActivityEvent** (struct) — Re-exported from `ragent_types::activity`.
- **RunStatus** (enum) — Re-exported from `ragent_types::activity`.
- **Projection** / **ResumeResult** / **RollbackResult** (structs) — Re-exported projection/replay types.