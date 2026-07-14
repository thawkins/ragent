# RIGKILL — Plan for Removing `ragent-rig` and All Rig-Provided Functionality

**Version:** 0.1.0-alpha.145 planning document  
**Status:** Draft plan — not yet implemented  
**Author:** RAgent (planning agent)  
**Date:** Generated during current session  

## 1. Goal

Completely remove the `ragent-rig` workspace crate and all functionality it provides, together with the Rig-specific documentation, specifications, research artifacts, and the orphaned abstraction points that were created *only* for the Rig integration. After this plan is executed:

- `cargo check` continues to pass for the whole workspace.
- `cargo test -p <crate>` passes for every crate whose tests are not already broken for unrelated reasons.
- The `Cargo.lock` no longer contains `ragent-rig`, `rig-core`, or any Rig-only transitive dependencies.
- No source file, documentation file, or spec file references Rig providers, `rig-core`, `ragent-rig`, or the Rig-backed semantic adapters.
- The research and memory search tools continue to use their existing native (non-Rig) implementations.

## 2. Rationale

The Rig integration is currently **orphaned and non-functional**:

- `cargo check -p ragent-rig` fails because `ragent-rig/src/config.rs` re-exports `RigConfig`, `RigProviderConfig`, `RigMemoryConfig`, `RigEmbeddingsConfig`, and `RigVectorStoreConfig` from `ragent_config::config`, but those types no longer exist in `ragent-config`.
- `ragent-rig/src/registry.rs` references `config.rig`, a field that no longer exists on the current `Config` struct.
- No crate in the workspace (including the root `ragent` binary) depends on `ragent-rig`.
- The Rig-backed provider registration, embeddings, vector stores, memory policies, code-index semantic layer, memory semantic layer, and research augmentor are therefore dead code.
- The only semantic-research test (`crates/ragent-research/tests/test_research_semantic.rs`) is also broken: it calls `ResearchSession::with_semantic_augmentor()` and references `SessionEvent::SemanticSourceRetrieved`, neither of which exist in the current codebase.

Rather than repairing an unused, feature-flag-heavy adapter against a fast-moving external framework, the project is choosing to delete it and rely on the existing native providers and native semantic-search path.

## 3. Scope

### 3.1 In scope

- Delete the entire `crates/ragent-rig/` directory.
- Remove Rig-specific documentation files.
- Remove the `specs/rig/` specification and plan.
- Remove the generated research artifacts under `research/rig/` (these are gitignored generated outputs).
- Remove the Rig-only abstraction points that are now dead code:
  - `crates/ragent-agent/src/session/semantic_handles.rs`
  - `crates/ragent-research/src/semantic.rs`
  - `crates/ragent-research/tests/test_research_semantic.rs`
- Update `crates/ragent-research/src/lib.rs` to stop re-exporting the `semantic` module.
- Remove stale Rig references from source-code comments (`semantic_handles.rs`, `semantic.rs`, etc.).
- Update `Cargo.lock` by running `cargo check` / `cargo build` after the crate removal.
- Update `CHANGELOG.md` with a removal entry.
- Verify `cargo check`, `cargo test -p ragent-research --lib`, `cargo test -p ragent-agent --lib`, and root `cargo test` pass.

### 3.2 Out of scope

- Fixing unrelated pre-existing test failures (e.g. `crates/ragent-agent/tests/test_thinking_pipeline.rs` is already broken due to a missing `as_any_static` method on a mock `Provider` impl). These should be tracked separately.
- Removing the `async-stream` workspace dependency: it is still used by `ragent-agent`, `ragent-llm`, and `ragent-server`, so it stays.
- Changing any native provider, memory, code-index, or research behavior. Only Rig-specific wiring is removed.
- Removing any other `docs/research*` or `research/*` directories outside `research/rig/`.

## 4. Current State Inventory

### 4.1 `ragent-rig` crate contents (to be deleted)

```
crates/ragent-rig/
├── Cargo.toml
├── src/
│   ├── codeindex.rs        (615 lines)
│   ├── completion.rs         (354 lines)
│   ├── config.rs             (11 lines)
│   ├── embeddings.rs         (634 lines)
│   ├── embeddings_trait.rs   (259 lines)
│   ├── error.rs              (57 lines)
│   ├── lib.rs                (240 lines)
│   ├── memory.rs             (645 lines)
│   ├── memory_semantic.rs    (718 lines)
│   ├── provider.rs           (1,080 lines)
│   ├── registry.rs           (249 lines)
│   ├── research.rs           (312 lines)
│   ├── testing_marker.rs     (22 lines)
│   ├── testing.rs            (690 lines)
│   ├── tool.rs               (543 lines)
│   ├── vcr.rs                (533 lines)
│   └── vector_store.rs       (1,090 lines)
└── tests/
    ├── integration_test.rs   (33 lines)
    ├── mock_model_test.rs    (111 lines)
    ├── provider_loop_test.rs (150 lines)
    └── vcr_test.rs           (127 lines)
```

Total crate source: ~8,473 lines (excluding blank/comment lines from rough `wc` count).

### 4.2 Documentation and specs (to be deleted)

| File | Purpose | Lines (approx) |
|---|---|---|
| `docs/howtos/rig-integration.md` | User-facing Rig config how-to | 351 |
| `docs/reports/rig-interface-audit.md` | Interface audit for Rig adapters | 769 |
| `docs/reports/rig-binary-size-compile-time-impact.md` | Size/compile-time report | 230 |
| `specs/rig/SPEC.md` | Rig integration spec | 174 |
| `specs/rig/PLAN.md` | Rig implementation plan | 82 |

Note: `docs/RIGDOCS.md` and `docs/rig-delegation-map.md` are already deleted in the working tree (`git status` shows `D`). The plan should verify no dangling references to them remain.

### 4.3 Generated research artifacts (to be deleted)

```
research/rig/
├── RESEARCH.md
└── sources/
    ├── local-01.md … local-10.md
    └── web-01.md … web-79.md
```

These are ignored by `.gitignore` (`/research/`) and were produced by the `ragent research` command. They should be removed from disk because they document and depend on the Rig integration that is being removed.

### 4.4 Dead abstraction points (to be deleted)

| File | Purpose | Why it is dead |
|---|---|---|
| `crates/ragent-agent/src/session/semantic_handles.rs` | Trait-object handles for Rig-backed semantic code index / memory | Not re-exported by `session/mod.rs`, not referenced by `SessionProcessor` or tool context. Created only to let `ragent-rig` plug in without a compile-time cycle. |
| `crates/ragent-research/src/semantic.rs` | `SemanticResearchAugmentor` trait + `SemanticHit` types | The only consumer was the Rig-backed `ResearchAugmentor` in `ragent-rig`. The test that uses it is broken. |
| `crates/ragent-research/tests/test_research_semantic.rs` | Integration test for Rig-backed semantic research | Calls non-existent `ResearchSession::with_semantic_augmentor()` and references non-existent `SessionEvent::SemanticSourceRetrieved`. |

### 4.5 Source comment cleanup (minor edits)

- `crates/ragent-agent/src/session/semantic_handles.rs`: delete file entirely (the comments referencing `ragent_rig::…` go with it).
- `crates/ragent-research/src/semantic.rs`: delete file entirely; or, if the project later wants to keep the `SemanticHit` types, rename/repurpose them. **Recommendation:** delete the whole file now because the types are Rig-only.
- Any other stray Rig mentions found by a final `grep -R "ragent-rig\|rig-core\|RigProvider\|register_rig_providers"` across the repo.

## 5. Detailed Execution Plan

### Phase 1 — Delete the crate and its docs

1. `rm -rf crates/ragent-rig`
2. `rm -f docs/howtos/rig-integration.md`
3. `rm -f docs/reports/rig-interface-audit.md`
4. `rm -f docs/reports/rig-binary-size-compile-time-impact.md`
5. `rm -rf specs/rig`
6. `rm -rf research/rig`
7. Verify `docs/RIGDOCS.md` and `docs/rig-delegation-map.md` remain deleted (they are already `D` in git status).

### Phase 2 — Delete Rig-only abstraction points

1. `rm -f crates/ragent-agent/src/session/semantic_handles.rs`
2. `rm -f crates/ragent-research/src/semantic.rs`
3. `rm -f crates/ragent-research/tests/test_research_semantic.rs`

### Phase 3 — Edit `ragent-research/src/lib.rs`

Remove the `semantic` module declaration and re-export:

```rust
// DELETE these lines:
pub mod semantic;
```

and

```rust
// DELETE these lines:
pub use semantic::{
    SemanticHit, SemanticHitKind, SemanticResearchAugmentor, arc_boxed as semantic_arc_boxed,
    boxed as semantic_boxed,
};
```

### Phase 4 — Refresh the lockfile

Run:

```bash
cargo check
```

This will:

- Automatically drop `ragent-rig` from the workspace because the `members = ["crates/*"]` glob no longer matches it.
- Remove `rig-core` and any Rig-only transitive dependencies from `Cargo.lock` because nothing references them.
- Update `Cargo.lock` checksums and feature sets for remaining crates.

### Phase 5 — Verify the build and tests

Run the following in order:

```bash
# 1. Whole-workspace check (must pass)
cargo check

# 2. Whole-workspace clippy (should pass with existing allow list)
cargo clippy -- -D warnings

# 3. Root binary tests (must pass; currently 0 tests pass trivially)
cargo test

# 4. Research crate library tests (must pass; 346 tests currently pass)
cargo test -p ragent-research --lib

# 5. Research crate integration tests excluding the deleted one
cargo test -p ragent-research --tests

# 6. Agent crate library tests (must pass; 316 tests currently pass)
cargo test -p ragent-agent --lib

# 7. Agent crate integration tests (some may be pre-existing failures; do not gate on these)
cargo test -p ragent-agent --tests
```

Acceptance: phases 1–6 must pass. Phase 7 may surface pre-existing failures (e.g. `test_thinking_pipeline.rs`) that are out of scope for this removal.

### Phase 6 — Final grep sweep

Run a broad search for any remaining Rig-specific identifiers:

```bash
grep -R "ragent-rig\|rig-core\|RigProvider\|register_rig_providers\|RigEmbeddingBackend\|RigError\|RigTool\|SemanticCodeIndexHandle\|SemanticMemoryHandle\|SemanticResearchAugmentor\|semantic_arc_boxed\|semantic_boxed" \
  --include="*.rs" --include="*.toml" --include="*.md" \
  crates/ docs/ specs/ research/ src/ examples/ tests/ \
  | grep -v "All Rights Reserved\|right\|Right\|rigid\|trigger\|fright"
```

Expected result: no matches except incidental English words (e.g. "right", "rigid", "trigger"). If any real matches remain, address them by editing or deleting the containing file.

### Phase 7 — Update project documentation

1. `CHANGELOG.md`: add an entry under a new or existing `Removed` section, e.g.:

   ```markdown
   ### Removed

   - Removed the `ragent-rig` crate and the entire Rig (`rig-core`) integration, including Rig-backed providers, embeddings, vector stores, memory policies, semantic code index, semantic memory, and research augmentor. The native providers and native memory/code-index/search implementations remain unchanged.
   ```

2. `README.md` / `SPEC.md` / `QUICKSTART.md`: confirm they contain no Rig references (current grep shows none). If any are found during the sweep, remove them.

3. No new docs should be created per AGENTS.md guidelines.

## 6. Git Summary

After execution, `git status` should show deletions similar to:

```
D  crates/ragent-rig/Cargo.toml
D  crates/ragent-rig/src/codeindex.rs
D  crates/ragent-rig/src/completion.rs
... (all ragent-rig source files)
D  docs/howtos/rig-integration.md
D  docs/reports/rig-binary-size-compile-time-impact.md
D  docs/reports/rig-interface-audit.md
D  docs/RIGDOCS.md
D  docs/rig-delegation-map.md
D  specs/rig/SPEC.md
D  specs/rig/PLAN.md
D  crates/ragent-agent/src/session/semantic_handles.rs
D  crates/ragent-research/src/semantic.rs
D  crates/ragent-research/tests/test_research_semantic.rs
 M Cargo.lock
 M crates/ragent-research/src/lib.rs
 M CHANGELOG.md
```

`research/rig/` will not appear in git status because the whole `/research/` directory is gitignored.

## 7. Rollback Plan

If the removal causes unexpected issues:

1. The deleted files are under version control (except `research/rig/` which is generated). They can be restored from the previous commit or from a stash created immediately before deletion.
2. Recommended pre-deletion step: create a stash or a temporary branch:

   ```bash
   git stash push -m "pre-rigkill" --include-untracked
   # or
   git checkout -b rigkill-backup
   git add -A && git commit -m "Backup before Rig removal"
   git checkout main
   ```

3. If rollback is needed, restore from the backup branch/stash and rerun `cargo check`.

## 8. Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Something outside the workspace still imports `ragent-rig` | Low | High | Phase 6 grep sweep catches it; `cargo check` fails fast. |
| `Cargo.lock` churn confuses downstream builds | Low | Medium | Run `cargo check` immediately; lockfile diff should only remove Rig-related entries. |
| A benchmark or example references Rig types | Low | Medium | Phase 6 sweep includes `examples/` and `benches/`; fix or delete any matches. |
| Native semantic search loses functionality | Very low | High | Not in scope — native `memory_search` and `codeindex_search` are independent and remain. |
| Pre-existing test failures obscure removal verification | High (known) | Low | Gate only on the explicitly passing test commands; document pre-existing failures separately. |

## 9. Definition of Done

- [ ] `crates/ragent-rig/` directory no longer exists.
- [ ] All Rig-specific docs and specs listed in section 4.2 are removed.
- [ ] `research/rig/` generated artifacts are removed from disk.
- [ ] `crates/ragent-agent/src/session/semantic_handles.rs` is removed.
- [ ] `crates/ragent-research/src/semantic.rs` and its `lib.rs` re-exports are removed.
- [ ] `crates/ragent-research/tests/test_research_semantic.rs` is removed.
- [ ] `cargo check` passes for the whole workspace.
- [ ] `cargo test` (root binary) passes.
- [ ] `cargo test -p ragent-research --lib` passes.
- [ ] `cargo test -p ragent-agent --lib` passes.
- [ ] Phase 6 grep sweep shows no Rig-specific identifiers in source/config/docs.
- [ ] `CHANGELOG.md` contains a removal entry.
- [ ] No push to remote has occurred (per AGENTS.md, only explicit user instruction triggers a push).

## 10. Effort Estimate

- Mechanical deletion and edits: ~15 minutes.
- Build/test verification (including cargo build/cache time): ~10–20 minutes on a warm cache, longer on first run.
- Final grep sweep and changelog update: ~10 minutes.
- **Total expected human-supervised time:** ~30–45 minutes plus build time.
