# Implementation Plan: Test Consolidation

> Companion to `specs/testconsolidate/SPEC.md` (status: draft).
>
> This plan migrates every inline `#[cfg(test)]` module across the 15-crate ragent
> workspace into per-crate `tests/` directories, preserving 100% of existing test
> behaviour and satisfying the `AGENTS.md` test-organisation mandate.

## Approach

Migration proceeds **one crate at a time**, smallest first, so that the visibility-
widening and `#[path]` re-import strategies (FR-007 / FR-008) are validated on cheap
crates before being applied to the large ones (`ragent-agent`, `ragent-research`,
`ragent-llm`, `ragent-tui`). After each crate, the workspace-wide gate (FR-010) runs
`cargo test --workspace` and `cargo clippy --workspace` before the next crate begins.

**Crate ordering** (by inline-test file count, ascending):

1. `ragent-prompt_opt` (1) 2. `ragent-storage` (1, **no tests/ dir**) 3. `ragent-config`
   (2) 4. `ragent-team` (3) 5. `ragent-bench` (4) 6. `ragent-tools-core` (6) 7.
   `ragent-tools-extended` (7) 8. `ragent-types` (3) 9. `ragent-specs` (9) 10.
   `ragent-tui` (10) 11. `ragent-server` (1, already partial) 12. `ragent-tools-vcs`
   13. `ragent-codeindex` (19) 14. `ragent-llm` (16) 15. `ragent-research` (20) 16.
   `ragent-agent` (34)

Large crates are split into sub-batches by module family (e.g. `skill/`,
`compression/`, `memory/`) to keep commits reviewable.

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Record pre-migration baseline (`cargo test --workspace` pass count, clippy state, per-crate inline-test manifest) | FR-001, FR-009 | S | Critical | completed | — |
| T-002 | Build discovery & classification tooling: enumerate every `#[cfg(test)]` module, classify public-only vs private-access, emit a manifest JSON | FR-001, FR-002 | M | Critical | completed | T-001 |
| T-003 | Migrate `ragent-prompt_opt`: 1 inline module → `tests/test_prompt_opt_*.rs`; rewrite imports to `use ragent_prompt_opt::...` | FR-003, FR-005, FR-006, FR-011 | S | High | completed | T-002 |
| T-004 | Migrate `ragent-storage`: create missing `tests/` dir, add `[[test]]` target if needed, move 1 inline module; resolve private access via FR-007 or FR-008 | FR-004, FR-005, FR-006, FR-011 | S | Critical | completed | T-002 |
| T-005 | Migrate `ragent-config`: 2 inline modules → `tests/test_*.rs`; widen `pub(crate)` on accessed internal items per FR-007 | FR-005, FR-006, FR-007, FR-011 | S | High | completed | T-002 |
| T-006 | Migrate `ragent-team`: 3 inline modules (23 private-access files) → `tests/test_team_*.rs`; apply `#[path]` re-import where items must stay private | FR-005, FR-006, FR-008, FR-011 | M | High | completed | T-002 |
| T-007 | Migrate `ragent-bench`: 4 inline modules → `tests/test_bench_*.rs` | FR-005, FR-006, FR-011 | S | Medium | completed | T-002 |
| T-008 | Migrate `ragent-tools-core`: 6 inline modules (24 private-access) → existing `tests/` files following the established `test_<component>_<scenario>.rs` pattern | FR-005, FR-006, FR-007, FR-011 | M | High | completed | T-002 |
| T-009 | Migrate `ragent-tools-extended`: 7 inline modules (28 private-access) → `tests/test_*.rs`; apply FR-007/FR-008 as needed | FR-005, FR-006, FR-007, FR-008, FR-011 | M | High | completed | T-002 |
| T-010 | Migrate `ragent-types`: 3 inline modules → `tests/test_types_*.rs` | FR-005, FR-006, FR-011 | S | Medium | completed | T-002 |
| T-011 | Migrate `ragent-specs`: 9 inline modules → existing `tests/` dir (currently 5 files) | FR-005, FR-006, FR-011 | M | Medium | completed | T-002 |
| T-012 | Migrate `ragent-tui`: 10 inline modules → existing `tests/` (29 files); split into widget/layout/bench sub-batches | FR-005, FR-006, FR-011 | L | Medium | completed | T-002 |
| T-013 | Migrate `ragent-server`: 1 inline module → existing `tests/` dir | FR-005, FR-006, FR-011 | S | Medium | completed | T-002 |
| T-014 | Migrate `ragent-tools-vcs`: inline modules → `tests/test_vcs_*.rs` | FR-005, FR-006, FR-011 | S | Medium | completed | T-002 |
| T-015 | Migrate `ragent-codeindex`: 19 inline modules (19 private-access) → existing `tests/` (5 files); split into parser/store/search sub-batches | FR-005, FR-006, FR-007, FR-008, FR-011 | L | High | completed | T-002 |
| T-016 | Migrate `ragent-llm`: 16 inline modules (19 private-access, spans 12 providers) → `tests/test_llm_*.rs`; split by provider family | FR-005, FR-006, FR-007, FR-008, FR-011 | L | High | completed | T-002 |
| T-017 | Migrate `ragent-research`: 20 inline modules (20 private-access) → `tests/test_research_*.rs`; split into gatherer/analysis/session sub-batches | FR-005, FR-006, FR-007, FR-008, FR-011 | L | High | completed | T-002 |
| T-018 | Migrate `ragent-agent` sub-batch A — `skill/` family (5 files, 204 tests): move to `tests/test_skill_*.rs` | FR-005, FR-006, FR-007, FR-008, FR-011 | L | High | completed | T-002 |
| T-019 | Migrate `ragent-agent` sub-batch B — `compression/` family (3 files, 124 tests): move to `tests/test_compression_*.rs` | FR-005, FR-006, FR-007, FR-008, FR-011 | M | High | completed | T-002 |
| T-020 | Migrate `ragent-agent` sub-batch C — `memory/` family (5 files) + `reference/` family (3 files): move to `tests/test_memory_*.rs` / `tests/test_reference_*.rs` | FR-005, FR-006, FR-007, FR-008, FR-011 | M | High | completed | T-002 |
| T-021 | Migrate `ragent-agent` sub-batch D — `session/`, `orchestrator/`, `permission/`, `message/`, `mcp/`, `task/`, `updater/`, `predictive.rs`, `resource.rs`, `agent/`, `tool/`, `perf/`, `research_adapter.rs` (remaining files): move to appropriately-named `tests/test_*.rs` files | FR-005, FR-006, FR-007, FR-008, FR-011 | L | High | completed | T-018, T-019, T-020 |
| T-022 | Relocate any root-level inline tests (e.g. `src/main.rs`) to root `tests/` directory | FR-012 | S | Medium | completed | T-002 |
| T-023 | Final verification pass: confirm `grep -rl "^#[cfg(test)]" --include="*.rs" crates/ src/` returns zero; run full `cargo test --workspace` and `cargo clippy --workspace -- -D warnings` | FR-009, FR-010 | S | Critical | completed | T-003–T-022 |
| T-024 | Generate completion report `docs/reports/testconsolidate-completion.md` with per-crate counts, visibility changes, and pre/post test pass counts | FR-015 | S | High | completed | T-023 |
| T-025 | Update `AGENTS.md` test-organisation section to codify the post-migration rules and reference the completion report | FR-014 | S | Low | completed | T-024 |
| T-026 | Add CI/pre-commit guard rejecting new inline `#[cfg(test)]` modules in library source files (optional hardening) | FR-014 | M | Low | completed | T-024 |
| T-027 | Commit per crate (or per logical batch) with messages of the form `test(<crate>): relocate N inline tests to tests/` | FR-013 | S | Critical | completed | T-003–T-022 |
## Verification Gates

Each crate task (T-003 … T-022) MUST, before being marked complete:

1. Run `cargo test -p <crate>` — all tests pass, count ≥ pre-migration count for that
   crate (FR-009).
2. Run `cargo clippy -p <crate> -- -D warnings` — no new warnings (FR-010).
3. Confirm the source file no longer contains `#[cfg(test)]` or `mod tests` (FR-011).
4. Confirm the target `tests/test_*.rs` file compiles and runs the relocated tests.

The final gate (T-023) runs the commands workspace-wide and confirms acceptance
criterion #1 (zero inline test modules remain).

## Effort Legend

- **S** — Small: ≤ 1 hour, ≤ 10 test functions or 1–2 files.
- **M** — Medium: 1–4 hours, 10–50 test functions or 3–8 files.
- **L** — Large: > 4 hours, > 50 test functions or > 8 files (split into sub-batches).

## Priority Legend

- **Critical** — blocks the migration's core guarantee or gates all subsequent work.
- **High** — large crates whose migration is essential to hit acceptance criteria.
- **Medium** — standard crate migrations; ordering by size keeps review manageable.
- **Low** — hardening and documentation polish after the core migration lands.