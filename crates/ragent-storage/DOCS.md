# ragent-storage

SQLite-backed storage for ragent: sessions, messages, structured memories,
activity logs, snapshots, encrypted credentials, and provider auth tokens.
Provides the persistent storage layer consumed by the agent, research, and
tool crates.

## Workspace Dependencies

- ragent-types

## External Dependencies

- tokio (async wrappers)
- serde
- serde_json
- anyhow
- thiserror
- tracing
- rusqlite
- chrono
- similar (snapshot diffs)
- blake3 (credential key derivation)
- base64
- chacha20poly1305 (credential encryption)
- uuid
- rand
- dirs

## Public API (crate root)

### Modules

- **activity_log** — Activity-log event schema and append-only storage (event sourcing for sessions/runs).
- **snapshot** — File snapshots: `take_snapshot`, `restore_snapshot`, `incremental_save` (`Snapshot`, `IncrementalSnapshot`).
- **storage** — Core SQLite storage facade (`Storage` struct).

### Structs

- **Storage** (struct) — Main storage handle wrapping a SQLite connection; provides session, message, memory, task, and provider-auth persistence.
  - Session/message methods: `create_session`, `get_session`, `list_sessions`, `update_session`, `archive_session`, `create_message`, `get_messages`, `update_message`, `delete_messages`.
  - Structured memory methods: `create_memory`, `get_memory`, `search_memories`, `list_memories`, `delete_memory`, `update_memory_content`, `increment_memory_access`.
  - Provider auth methods: `set_provider_auth`, `get_provider_auth`, `delete_provider_auth` (keys encrypted with `encrypt_key` / `decrypt_key`).
  - Settings methods: `get_setting`, `set_setting`, `delete_setting`.
  - Schema: the `SCHEMA_VERSION` constant; migrations run inside `Storage::open`.
  - Search-index maintenance: `warm_message_search_index` (startup reconcile of
    `messages_fts` against `messages`; diffs the rowid domains via the FTS5
    `messages_fts_content` shadow table rather than joining on the `UNINDEXED`
    `message_id`, backfills missing or stale rows and drops orphaned index rows).

### Module: activity_log

Activity-log event sourcing for sessions/runs — immutable, self-describing
execution facts appended before being projected into user-facing state.

- **ActivityLog** (struct) — Append-only event log store.
  - `append(event)` — Append an activity event.
  - `read_run(run_id)` — Read all events for a run; `read_run_upto` stops at a sequence number.
  - `last_seq(run_id)` — Get the latest sequence number for a run.
  - `rollback_to_seq` / `rollback_to_checkpoint` — Roll a run back.
  - `resume_run` / `replay_run` — Resume or replay a run from its events.
- **AppendError** (enum) — Error returned when an append is rejected.
- `ActivityEvent`, `RunStatus`, `Projection`, `ResumeResult`, `RollbackResult` are defined in
  `ragent_types::activity` (not re-exported by this crate).