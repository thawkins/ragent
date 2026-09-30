# Team runtime + tool unification decision (ANTIPAT M7.8)

Status: applied. The `ragent-team` crate has been folded into `ragent-agent`.

## Background

Historically the team subsystem lived in `crates/ragent-team/` and was
compiled into `ragent-agent` through 27 `#[path = "../../../ragent-team/..."]`
attributes. REMPLAN.md M3 (T3.3) moved the sources *natively* into
`crates/ragent-agent/src/team/` and `crates/ragent-agent/src/tool/team_*.rs`
to remove the `#[path]` cycle workaround, leaving `crates/ragent-team` as a
59-line pure re-export shim over `ragent-agent`.

By M7.8 the shim had zero definitions of its own, `ragent-tui` already
depended on `ragent-agent` directly, and no other crate used it. The
remaining consumers were:

- `crates/ragent-tui` (15 files, all using `ragent_team::team::*`), and
- the 16 integration test files under `crates/ragent-team/tests/`.

## Decision

Folding beats documenting the split here. The crate was a pass-through with
no independent value: keeping it preserved a second name for the same types,
forced an extra path dependency, and let the stale
`scripts/check-team-duplication.sh` guard (which asserted the crate *was* the
canonical source) contradict the actual post-M3 layout.

## What changed

1. `crates/ragent-team/` was deleted.
2. `crates/ragent-tui` dropped the `ragent-team` dependency and now imports
   `ragent_agent::team::*` / `ragent_agent::tool::*` directly.
3. The 16 team integration tests moved to `crates/ragent-agent/tests/`, with
   `ragent_team::team::` rewritten to `ragent_agent::team::`; the shared
   `tests/support/mod.rs` helper moved to `crates/ragent-agent/tests/team_support/`.
   `ragent-agent` gained `ragent-tools-core` as a dev-dependency for the
   tests that exercise `BashTool` / `CommandCatalog`.
4. `scripts/check-team-duplication.sh` was rewritten to assert the *current*
   invariant: the team runtime lives in `ragent-agent/src/team/`, the team
   tools live in `ragent-agent/src/tool/team_*.rs`, and no `ragent-team`
   crate or `#[path]` references to it are re-introduced. It is wired into
   `ci.yml` and `pre-flight.sh` and ships a `--self-test`.

`crates/ragent-agent/src/team/mod.rs` is the single canonical home for the
team runtime.
