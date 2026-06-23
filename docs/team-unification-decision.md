# Team Unification Decision Record

**Date:** 2026-06-21  
**Context:** COMMSPLAN.md Milestone 2 — unify the duplicated team implementation.

## Problem

The team subsystem had two copies of the same source:

- `crates/ragent-team/src/team/`
- `crates/ragent-agent/src/team/`

The copies had diverged: the `ragent-team` versions received the Milestone 1
race-free locking fixes, while the `ragent-agent` versions did not. Any fix had
to be applied twice, and the two copies were drifting further apart.

## Constraints

A straightforward Cargo dependency (`ragent-agent` → `ragent-team`) is
impossible because `ragent-team` already depends on `ragent-agent`. The cycle
exists because `TeamManager` (which lives in `ragent-team`) needs to spawn child
sessions through `ragent_agent::session::processor::SessionProcessor` and uses
agent resolution helpers from `ragent_agent::agent`.

Breaking the cycle would require either:

1. Moving the session-runtime parts of `TeamManager` into `ragent-agent` and
   leaving only the data/file-IO layer in `ragent-team` (large architectural
   refactor); or
2. Extracting shared traits/types into a new lower-level crate such as
   `ragent-team-types` and making both crates depend on it.

Both options touch dozens of files and are out of scope for the current
remediation pass, whose primary goal is to stop the duplication from
re-introducing M1 bugs.

## Decision

Keep a **single source of truth** in `crates/ragent-team/src/team/` and compile
it into both crates using Rust `#[path]` attributes in
`crates/ragent-agent/src/team/mod.rs`.

This means:

- `ragent-agent/src/team/` now contains only `mod.rs`.
- `mod.rs` includes each team submodule from the corresponding
  `ragent-team/src/team/*.rs` file.
- Edits made to the `ragent-team` source are reflected in `ragent-agent`
  automatically.
- There is no Cargo dependency cycle because the inclusion is source-level.

The previous `#[path]` include was limited to `store.rs`. Milestone 2 extends
the same mechanism to `classify.rs`, `config.rs`, `mailbox.rs`, `manager.rs`,
`swarm.rs`, and `task.rs`.

### Tools follow the same pattern

The 20 team coordination tools (`team_approve_plan`, `team_assign_task`,
`team_broadcast`, …, `team_wait`) previously existed as **byte-for-byte
identical copies** in two locations:

- `crates/ragent-agent/src/tool/team_*.rs`
- `crates/ragent-team/src/tools/team_*.rs`

Every fix (Milestones 3–5) had to be applied to both copies and then
re-synced with `cp`. To eliminate that maintenance burden, the tools now
follow exactly the same `#[path]` unification as the runtime modules:

- The canonical tool source lives in `crates/ragent-team/src/tools/`.
- `crates/ragent-agent/src/tool/mod.rs` includes each tool via
  `#[path = "../../../ragent-team/src/tools/team_*.rs"] pub mod team_*;`.
- The physical copies under `crates/ragent-agent/src/tool/team_*.rs` have
  been deleted.
- `register_team_tools` in `crates/ragent-team/src/tools/mod.rs` is unchanged
  and still works for callers that want the team-tools-only registry.
- `ragent_agent::tool::create_default_registry()` still registers all team
  tools; the `team_*::Tool*Tool` paths now resolve to the `#[path]`-included
  modules.

The tool source compiles unchanged in both crates because it only references
`crate::team::*`, `crate::tool::*`, `crate::event::*`, and `super::{Tool,
ToolContext, ToolOutput}` — all of which resolve correctly under both
`ragent-agent` (real modules + `#[path]` team runtime) and `ragent-team`
(re-exports from `ragent_agent` + canonical team runtime).

## Trade-offs

| Pros | Cons |
|------|------|
| Single source file per team module | Not a real crate dependency; type identity is duplicated at runtime if values cross the crate boundary |
| No Cargo cycle | `#[path]` is unusual and can surprise maintainers |
| Low-risk, mechanical change | Future work must still break the cycle to get a clean architecture |
| M1 fixes apply to both crates immediately | |

## Mitigations

- Added a `MemoryScope::as_str()` helper so that `TeamManager` can compare memory
  scopes across the crate boundary without tripping over distinct type
  identities.
- Added a CI guard script (`scripts/check-team-duplication.sh`) that fails if:
  - any runtime source file is re-created under `crates/ragent-agent/src/team/`
    (other than `mod.rs`), or
  - any `team_*.rs` tool file is re-created under
    `crates/ragent-agent/src/tool/`, or
  - `mod.rs` (in either `team/` or `tool/`) is missing a `#[path]` include for
    a file that exists in the canonical `ragent-team` location.

## Future work

The long-term target is still the architecture described in COMMSPLAN.md
M2-T1/T2/T3:

1. Move the runtime/session-spawning parts of `TeamManager` into
   `ragent-agent` (or a new runtime crate).
2. Make `ragent-team` a pure data/file-IO crate with no dependency on
   `ragent-agent`.
3. Add `ragent-team` as a normal Cargo dependency of `ragent-agent` and
   remove all `#[path]` includes.

Until that refactor lands, the `#[path]` unification prevents the duplicate
implementation from drifting.
