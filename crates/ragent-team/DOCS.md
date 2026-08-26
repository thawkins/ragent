# ragent-team

Team coordination runtime and team tools for ragent. Note: the team
coordination runtime has been consolidated into `ragent-agent` (via `#[path]`
includes) and this crate re-exports the public types for ergonomic access.

## Workspace Dependencies

- ragent-types
- ragent-storage

## External Dependencies

- tokio, serde, serde_json, anyhow, thiserror, tracing
- chrono, uuid, dashmap, parking_lot

## Public API (crate root)

This crate re-exports the team coordination types from `ragent-agent`'s team
module. The canonical implementation lives in `ragent-agent/src/team/`.

- **TeamManager** (struct) — Team coordination manager; owns teammates, tasks, mailbox, and state.
- **TeamStore** (struct) — Persistent team state store.
- **TeamConfig** (struct) — Team configuration.
- **TaskList** (struct) — Shared task list for a team.
- **Task** (struct) — A single team task.
- **TaskStatus** (enum) — Team task lifecycle status.
- **Mailbox** (struct) — Per-teammate message mailbox.
- **TeamMessage** (struct) — A message in a teammate's mailbox.
- **Blueprint** (enum) — Team blueprint (e.g. `code-review`) defining teammate composition.
- **TeamHandle** (struct) — Handle to an active team.

### Modules

- **classify** — Agent type inference for teammate assignment.
- **config** — Team configuration types.
- **mailbox** — Mailbox messaging types.
- **manager** — `TeamManager` — the coordination runtime.
- **store** — `TeamStore` — persistent team state.
- **swarm** — LLM-based swarm decomposition for parallel work.
- **task** — `TaskList` — shared task list.

> **Note:** The 20 team tools (`team_create`, `team_spawn`, `team_message`,
> `team_task_claim`, `team_task_complete`, etc.) are registered in
> `ragent-agent/src/tool/team_*.rs`, not in this crate.