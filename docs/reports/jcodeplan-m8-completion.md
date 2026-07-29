# JCODEPLAN M8 Completion Report — Durable Initiatives & Skill Management

**Status:** ✅ complete
**Date:** 2025 (see git history)
**Tasks:** T-070 (storage + tool), T-071 (skill_manage), T-072 (tests),
T-073 (registration) — all done.

---

## Scope

Implement JCODEPLAN Milestone 8:

> Port `initiative` (durable goals with milestones) and `skill_manage`
> (skill load/list/reload/read at runtime). Where:
> `ragent-agent/src/tool/initiative.rs` and
> `ragent-agent/src/tool/skill_manage.rs`; storage in `ragent-storage`.

**Acceptance (from plan):**

- `initiative action="checkpoint" id="api-v2"` updates progress ✅
- `skill_manage action="load" name="rust-error-handling"` injects the skill ✅

Both acceptance criteria are covered by dedicated integration tests
(`test_initiative_checkpoint_updates_progress`,
`test_skill_manage_load_injects_skill_prompt`).

---

## Deliverables

### 1. `initiative` tool — `crates/ragent-agent/src/tool/initiative.rs`

- Actions: `create`, `read`, `update`, `checkpoint`, `list`, `close`.
- Project-scoped (key = canonical working-directory string) so initiatives are
  shared across sessions in the same project and invisible to other projects.
- `checkpoint` marks a milestone complete with a timestamp, bumps overall
  progress (clamped 0–100), and appends a timestamped `Checkpoint:` line to
  the description as an audit trail.
- `close` requires `status` `completed` (progress auto-jumps to 100 when the
  caller did not specify it) or `abandoned` (progress preserved).
- Slug validation on user-supplied ids (ASCII alphanumerics/`-`/`_`, ≤ 64
  chars); auto-generates `initiative-<8 hex chars>` when omitted.
- All storage calls offloaded via `tokio::task::spawn_blocking` so rusqlite
  never blocks the async executor.
- Permission category: `storage:write`.

### 2. Storage layer — `crates/ragent-storage/src/storage.rs`

- New `initiatives` table created in `migrate()`:
  `id TEXT PRIMARY KEY`, `title`, `description`, `status`
  (`active|paused|completed|abandoned`), `milestones_json`, `progress`,
  `project`, `session_id`, `created_at`, `updated_at`, `closed_at`, plus
  index on `(project, status)`.
- CRUD methods: `create_initiative`, `get_initiative`, `list_initiatives`
  (status filter + `all`), `update_initiative` (sets `closed_at` on
  `completed`/`abandoned`, clears it on re-open), `delete_initiative`.
- `closed_at` lifecycle covered by dedicated storage tests.
- Types `InitiativeMilestone` (serde: `id`, `title`, `done`, `completed_at`)
  and `InitiativeRow` (with `milestones()` JSON decoder that falls back to an
  empty vec on malformed rows) live next to `TodoRow` and are re-exported
  from `ragent-storage/src/lib.rs` **and** `ragent-agent/src/storage/mod.rs`
  so `crate::storage::{InitiativeMilestone, InitiativeRow}` resolves
  everywhere.

### 3. System-prompt surfacing — `crates/ragent-agent/src/session/loop_steps.rs`

- `build_turn_system_prompt()` now builds an
  `## Active Initiatives` section via
  `initiative::build_initiatives_prompt_section()` on a `spawn_blocking`
  worker (mirroring the existing memory-section pattern, with its own
  profiler scope `prompt.build_initiatives_section`).
- Section lists each active initiative's id, progress %, title, and the next
  pending milestones (up to 3, with a "+N more" suffix).
- Injected immediately before the tool-reference block on every turn.
- Returns empty string (skipped) when no active initiatives exist — zero
  extra tokens in the common case.

### 4. `skill_manage` tool — `crates/ragent-agent/src/tool/skill_manage.rs`

- Actions: `list`, `read`, `load`, `reload`.
- `list` renders a scope/summary table from the metadata-only
  `SkillRegistry::catalog()`; optional `scope` filter and `include_bodies`
  flag that appends each skill's prompt body.
- `read` / `load` share an implementation that:
  1. Re-discovers the registry from disk on every call (so skills added or
     edited *after* session start are found — proven by
     `test_skill_manage_reload_picks_up_new_skill_added_after_first_scan`).
  2. Invokes the skill through the canonical
     `crate::skill::invoke::invoke_skill` path (argument substitution +
     dynamic-context injection when the skill opts in).
  3. Returns the fully processed prompt. `load` additionally frames the
     result as "The following prompt is now active for this turn" — the
     injection required by the M8 acceptance line.
- `reload` clears every cached `SKILL.md` body via the new
  `SkillInfo::clear_body_cache()`, re-discovers from disk, and reports
  added/removed skill names plus the bundled-baseline count.
- Unknown-skill errors list the currently available skills and include a
  `next_action` hint pointing at the skills directories and the `reload`
  action.
- Permission category: `skill:manage`.

### 5. Registration — `crates/ragent-agent/src/tool/mod.rs`

- New modules wired in alongside the other M-series tools:
  `pub mod initiative;` / `pub mod skill_manage;`.
- Registered in `create_default_registry()` (immediately after `BgTool`):
  `initiative::InitiativeTool` and `skill_manage::SkillManageTool`.
- No changes needed in `ragent-tools-extended` — both tools are
  session/storage-bound and therefore live in the agent crate per the plan.

---

## Tests (T-072)

| File | Tests | Focus |
|---|---|---|
| `crates/ragent-agent/tests/test_initiative.rs` | 26 | identity, schema, create (incl. auto-id, duplicate, invalid slug, missing title), read, update (fields + required-field check), checkpoint (progress, unknown milestone, idempotent double-complete, closed-initiative rejection), close (completed/abandoned/invalid status), list (default filter, `all`, invalid filter, empty hint), cross-session visibility, per-project isolation, missing storage error, unknown action, direct storage round-trip, prompt section (empty + populated) |
| `crates/ragent-agent/tests/test_skill_manage.rs` | 12 | identity, schema, list (bundled+project, scope filter), read (arg substitution, unknown-skill listing), load injects prompt (acceptance), reload (baseline counts, picks up added skills, reflects edited bodies) |
| `crates/ragent-storage/tests/test_initiatives.rs` | 7 | table creation via migrate, full field round-trip, status filter (active/paused/all), `closed_at` set on completed *and* abandoned, cleared on re-open, update-missing-row → false, malformed-milestone-JSON safe fallback |

Full-suite runs after the change:

- `cargo test -p ragent-agent` — **545 passed, 0 failed**
- `cargo test -p ragent-storage` — **all suites green** (lib, integration,
  doctests)
- `cargo clippy -p ragent-agent -p ragent-storage --all-targets` — no new
  warnings from M8 (the single remaining `let_unit_value` warning in
  `tests/test_bg_service.rs` predates M8).

---

## Documentation

- `SPEC.md` — new **§19B "Durable Initiatives & Skill Management"**
  documenting both tools, their actions, examples, permission categories, and
  the system-prompt surfacing behaviour.
- `docs/JCODEPLAN.md` — M8 tasks T-070, T-070, T-071, T-072, T-073 marked ✅.
- `CHANGELOG.md` — Unreleased entry describing the milestone.

---

## Gotchas encountered (for future milestones)

1. **`ragent-storage`'s `migrate()` SQL block had wildly inconsistent
   indentation.** A naïve string edit introduced a comment line that
   swallowed a `CREATE TABLE` statement. Fixed by re-indenting the entire
   SQL string consistently (12-space indent) via a one-off script, then
   hand-repairing two long lines. Any future edit to this block should keep
   the uniform indentation.
2. **`serde_json::Value` has no `.is_some()`.** Use `.as_str().is_some()`
   (or the relevant typed accessor) — first compile of `initiative.rs`
   tripped on this in the checkpoint milestone guard.
3. **Borrow-check on temporaries:** `registry.catalog().iter()` borrows the
   temporary `Vec`; bind the catalog first (`let catalog = registry.catalog();`)
   before collecting references.
4. **`status="all"` vs `Some` filtering**: passing `Some("all")` down to
   storage would have filtered by a non-existent status. `action_list` maps
   `all` → `None` (no filter) explicitly.
