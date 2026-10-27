# Test-consolidation review and plan

Status: step 1 of the rollout is DONE (8 safe crates consolidated, 221 top-level
test files -> 8 binaries); steps 2-3 (medium/high-risk crates) await go-ahead.
Scope: reduce the number of discrete integration-test binaries from 904 to
<= 90 (10%) by consolidation, without losing test coverage or changing what any
test asserts.

## 1. Current state (measured)

| Metric | Value |
| --- | --- |
| Top-level `crates/*/tests/*.rs` (one binary each) | 904 |
| Files under any `tests/` dir (incl. support subdirs) | 1041 |
| `rustc` integration-test link steps in a full `cargo test` | 904 |
| Root `tests/*.rs` | 6 |

Per-crate top-level counts:

```
ragent-agent       182   ragent-research     56   ragent-config       47
ragent-tui         187   ragent-storage      45   ragent-codeindex    49
ragent-tools-ext      82  ragent-types        39   ragent-plugins      36
ragent-tools-core    32  ragent-tools-vcs    30   ragent-llm          28
ragent-telemetry     23  ragent-connectors   20   ragent-specs        18
ragent-server        14  ragent-bench         8   ragent-surface       8
```

Each top-level file is a separate crate root, so each is a separate link step
and a separate test binary.

## 2. What makes consolidation possible

A single integration target can include every former file as a child module if
the target is an explicit `[[test]]` with `autotests = false` and each child is
brought in through `#[path = "..."]`:

```toml
# crates/<crate>/Cargo.toml
[package]
autotests = false          # stop auto-discovering the individual files

[[test]]
name = "all"
path = "tests/all.rs"
```

```rust
// crates/<crate>/tests/all.rs
#[path = "test_activity_event.rs"] mod test_activity_event;
#[path = "test_automation_run.rs"] mod test_automation_run;
// ... one line pair per former test file
```

`#[path]` is the key detail: it makes the child's own file path the anchor, so
the nested constructs the workspace relies on keep working unchanged:

| Construct | Occurrences (top-level test files) | Behaviour under `#[path]` inclusion |
| --- | --- | --- |
| `mod support;` / `mod x_support;` (plain) | 227 | resolves via the child's own `tests/` dir - works |
| `#[path = "support/mod.rs"]` | 114 + 44 | anchored at the child file - works |
| `#[path = "../src/....rs"]` source re-import | 153 | anchored at the child file - works |
| `include_str!("support/....")` | 5 | anchored at the child file - works |
| `#![forbid(unsafe_code)]`, `#![allow(...)]`, `#![cfg(feature=...)]` | 139 / 34 | inner attributes legal on a child module - works |
| `#![cfg(test)]` | 19 | works |
| `#![allow(unsafe_code)]` | 50 | still applies to the whole consolidated binary |

Verified in a standalone probe covering all of the above, plus two files
declaring the same helper name (no collision - each file is a module).

## 3. Pilot result (ragent-types, 39 files)

```
cargo test -p ragent-types --no-run        (39 top-level test files)

before: 39 test binaries, 628.1 MB total (avg 16.1 MB)
after :  1 test binary,    30.2 MB
built :  394 tests, 393 passed
rebuild on src/lib.rs touch: 1838 ms vs 1774 ms baseline (no regression)
```

Binary count for the crate: 39 -> 1 (2.6% of current). Test bodies are unchanged
in every file.

### 3.1 The one failure - and what it teaches

`test_secret_registry_cap::registry_is_bounded_at_the_named_cap` failed after
consolidation and passes in isolation. Cause:

* The secret registry is process-wide.
* `test_secret_registry_cap.rs` uses a **file-local** `Mutex` and its header
  comment states the assumption explicitly: *"Cargo runs each test file as its
  own binary, so the registry these tests touch is not shared with other test
  binaries."*
* Three other files (`test_sanitize_helpers`, `test_sanitize_redact`,
  `test_unified_redaction`) call `clear_secret_registry()` / `register_secret` on
  the same global registry. Once they share a binary they race.

Fix (2 lines, and it matches the existing pattern in `test_resource.rs`):
`serial_test` is already a dev-dependency of `ragent-types`, so tag the mutating
tests `#[serial]`.

**This is the whole risk class for the rollout:** the 904-binary model currently
*isolates* ~904 process-global resources (env vars, cwd, static registries) from
each other. Consolidation merges them. Measured exposure:

| Crate | globals: env-var mutation | cwd mutation | `#[serial]` in use | binary count |
| --- | --- | --- | --- | --- |
| ragent-tui | 16 | 24 | 0 | 187 |
| ragent-agent | 6 | 0 | 5 | 182 |
| ragent-tools-extended | 2 | 1 | 0 | 82 |
| ragent-config | 14 | 10 | 13 | 47 |
| ragent-tools-vcs | 5 | 0 | 0 | 30 |
| ragent-llm | 4 | 0 | 0 | 28 |
| ragent-plugins | 4 | 0 | 0 | 36 |
| ragent-connectors | 1 | 1 | 0 | 20 |
| ragent-tools-core | 1 | 0 | 0 | 32 |
| (remaining 9 crates) | 0 | 0 | 1 | 296 |

`ragent-tui` is the high-risk crate: 18 files carry `#![cfg(feature = ...)]`, 17
carry `#![allow(unsafe_code)]`, and 40 files mutate env/cwd **with zero
serialisation**. Feature gates also mean a `--features` build changes the binary
content, so a single `all.rs` per crate is the only configuration that keeps one
binary per crate.

## 4. Recommended rollout

One consolidated `tests/all.rs` per crate (2 crates max if feature gating demands
it). Projected end state:

* 904 -> **18 binaries** for the integration suite (2.0% of current, well under
  the 10% target).
* Plus one unit-test binary per crate lib (18) and the root bins - ~40 discrete
  binaries in total against ~922 today.

Ordered by risk, and each step independently verifiable:

1. **Low-risk crates first** (no env/cwd/global mutation): `ragent-codeindex`,
   `ragent-research`, `ragent-storage`, `ragent-specs`, `ragent-server`,
   `ragent-surface`, `ragent-bench`, `ragent-telemetry` (+12 `cfg(feature)`
   files). 221 binaries -> 8. **DONE - see section 7.**
2. **Medium**: `ragent-types` (apply the `#[serial]` fix from 3.1),
   `ragent-tools-core`, `ragent-tools-extended`, `ragent-tools-vcs`,
   `ragent-llm`, `ragent-plugins`, `ragent-connectors`, `ragent-config`
   (`#[serial]` already in use in 13 files).
3. **High**: `ragent-agent` (182), `ragent-tui` (187) - add `#[serial]` to every
   env/cwd-mutating test, or keep 2-3 binaries grouped by feature gate.

Per crate the mechanical steps are:

1. Generate `tests/all.rs` with one `#[path = "test_x.rs"] mod test_x;` per file.
2. Add `autotests = false` to `[package]` and a single `[[test]]` entry.
3. `cargo test -p <crate>`; reconcile any cross-file global-state failures with
   `#[serial]` (never by weakening an assertion).
4. Record the before/after binary count and wall time.

## 5. Costs, and the honest case against

* **Not the big disk win.** The 581 GB `target/` bloat was 412 GB of incremental
  caches and 141 GB of stale `CARGO_TARGET_DIR` override dirs - both already
  pruned. Consolidation's disk saving is modest (~0.6 GB per ragent-types-scale
  crate, more for the big two), plus it removes 904 link steps.
* **Build time is roughly neutral.** Measured: `src/lib.rs` touch -> 1838 ms
  consolidated vs 1774 ms baseline; touch-all-test-sources -> 1835 ms vs
  1236 ms, because the single binary is 5.4x larger and a changed test now
  recompiles and relinks everything in its crate (test isolation / build
  parallelism lost). The gain shows up in clean builds (904 -> 18 link steps)
  and in CI matrix fan-out, not in incremental edit-test cycles.
* **AGENTS.md tension.** AGENTS.md says test files "should be placed in the root
  `tests/` folder" and cites `docs/reports/testconsolidate-completion.md`. That
  report does not exist in the repo. Under the current literal model (one file
  per target) the 10% target is unreachable: you cannot have 904 top-level files
  and <= 90 binaries. Consolidation requires files to stop being target roots,
  which is a deliberate departure from the current test-organisation guideline
  and should be recorded in AGENTS.md as such.
* **CI references.** 9 workflows invoke `cargo test -p <crate> --test <name>`
  for specific files (e.g. `--test test_shared_guards`,
  `--test test_metric_toggles`). After consolidation those target names no longer
  exist; each becomes `--test all <module::filter>` (a `-v` list can supply the
  exact module path).
* **`--test` ergonomics.** `cargo test -p X --test test_foo` stops working; you
  filter instead: `cargo test -p X --test all test_foo::`. Test *names* keep their
  file prefix, so `cargo test -p X test_foo` still matches.

## 6. Recommendation

Proceed, in the risk order above. Start with step 1 (241 binaries -> 8, no
global-state exposure) so the mechanical tooling and the `autotests`/`[[test]]`
wiring are proven at scale before the two 180+ file crates. Treat the
`#[serial]` reconciliation as part of the change, not a follow-up.

Blocking question: AGENTS.md currently mandates the one-file-per-target layout.
Consolidation is only possible if that guideline is amended (the 904-file layout
and the 10%-binary target are mutually exclusive).

## 7. Step 1 executed - the 8 safe crates

Rollout step 1 is complete. The 8 low-risk crates (no env/cwd/global mutation)
were consolidated to one `tests/all.rs` target each:

| Crate | top-level files before | binaries after |
| --- | --- | --- |
| ragent-codeindex | 49 | 1 |
| ragent-research | 56 | 1 |
| ragent-storage | 45 | 1 |
| ragent-specs | 18 | 1 |
| ragent-server | 14 | 1 |
| ragent-surface | 8 | 1 |
| ragent-bench | 8 | 1 |
| ragent-telemetry | 23 | 1 |
| **total** | **221** | **8** |

Verification method: `cargo test -p <crate> -- --list` was captured before the
change and `cargo test -p <crate> --test all -- --list` + `--lib -- --list`
after (the two binaries that replace each crate's former set). Stripping the
`test_<file>::` module prefix from the consolidated names, the test-name sets are
**identical for all 8 crates** (0 missing, 0 extra; doctests excluded - they are
unchanged). A full `cargo test` over all 8 crates passes: 0 failures.

Two mechanical issues surfaced (both fixed, neither is a behaviour change):

1. **`crate::<x>` shims.** `tests/test_session_fallback.rs` (ragent-research)
   `#[path]`-includes `src/session/fallback.rs`, which refers to
   `crate::document` / `crate::source`. Those shims worked because the file was
   a binary root; as a child module they must sit at the consolidated binary
   root, so they moved to `crates/ragent-research/tests/all_prelude.rs`
   (inlined at the top of `all.rs`).
2. **`use crate::TelemetryError`.** `tests/test_subsystem.rs` (ragent-telemetry)
   used a `crate::`-relative import that only resolved as a binary root; changed
   to the explicit `ragent_telemetry::TelemetryError` crate path (the crate
   re-exports it).
3. **`clippy::duplicate_mod`.** `ragent-codeindex` (16 children declare
   `#[path = "support/mod.rs"] mod support;`) and `ragent-research`
   (a shared `scoreboard_fixture.rs`) load one helper file as a module of several
   children. `clippy -D warnings` rejects that, so those two `tests/all.rs` carry
   a narrow `#![allow(clippy::duplicate_mod)]` with the reason inline; the other
   six crates need no allow. Hoisting the helper to `all.rs` would mean editing
   every child file, which the surgical-change rule argues against.

Verified after the change: `cargo test` over all 8 crates passes (0 failures,
re-run three times to shake out ordering-dependent races - none found);
`cargo clippy` over all 8 with `--all-targets -D warnings` is clean;
`cargo fmt --check` is clean; the nine `scripts/check-*.sh` guards all pass.

One pre-existing, unrelated warning is not a regression: under
`RUSTFLAGS='-D unreachable_pub ...'` `crates/ragent-codeindex/tests/support/mod.rs`
has a `pub fn` that is unreachable *in every one of its former 16 binaries too*
(confirmed by re-running the check with the change stashed). CI's dead-code job
only runs `cargo check --workspace --lib --all-features`, which does not compile
test targets, so this never fired in CI; it is left as-is (surgical-change rule).

CI: the three `--test <name>` invocations that named crate-local test files were
updated to the filter form (`cargo test -p ragent-research --test all
test_search_retry_clamp::`, likewise ragent-specs / ragent-telemetry); the
`ragent-types` and `ragent-tools-vcs` / `ragent-config` / `ragent-plugins` /
`ragent-tools-extended` invocations are untouched because those crates are
scheduled for steps 2-3.

## 8. Steps 2 and 3 executed - every crate, plus the root crate

The rollout is complete. All 19 targets (18 crates + the root `ragent` package)
now build **one** integration-test binary each:

| Crate | top-level test files | integration binaries |
| --- | --- | --- |
| ragent-agent | 182 | 1 |
| ragent-tui | 187 | 1 |
| ragent-tools-extended | 82 | 1 |
| ragent-research | 56 | 1 |
| ragent-codeindex | 49 | 1 |
| ragent-config | 47 | 1 |
| ragent-storage | 45 | 1 |
| ragent-types | 39 | 1 |
| ragent-plugins | 36 | 1 |
| ragent-tools-core | 32 | 1 |
| ragent-tools-vcs | 30 | 1 |
| ragent-llm | 28 | 1 |
| ragent-telemetry | 23 | 1 |
| ragent-connectors | 20 | 1 |
| ragent-specs | 18 | 1 |
| ragent-server | 15 | 1 |
| ragent-bench | 8 | 1 |
| ragent-surface | 8 | 1 |
| root `ragent` | 6 | 1 |
| **total** | **911** | **19** |

(`cargo test -p <crate> --no-run` reports 2 executables per crate: the test
binary and the crate's own `--lib` unit-test binary, which is unchanged.)

### 8.1 The real risk was process state, not linking

Merging the binaries merges the process. Every crate's `tests/all.rs` therefore
defines one binary-wide, poison-tolerant lock
(`global_env_mutex()` / `global_env_lock()`), and every test that mutates or
observes a process-global takes it. The per-crate failures below were all found
this way and fixed at the test, never by weakening an assertion.

| Failure class | Crates | Fix |
| --- | --- | --- |
| `crate::` in a `#[path]`-included `src/*.rs` module resolved at the child, not the binary root | ragent-agent, ragent-tools-core, ragent-research, ragent-telemetry | re-export the crate paths used by the shimmed source from `tests/all.rs` (agent: `llm`, `event`, `permission`, `session::loop_state`, `tool`; tools-core: `event`/`resource`/`sanitize`/`askpass`/`CanonicalPathCache`/`jsonl_log`) |
| file-local `mod shim` became unreachable from the binary root | ragent-tools-core (`test_bash_wrapper_quoting`) | make the stub module `pub` and re-export only its `Tool`/`ToolContext`/`ToolOutput` for the child's own `super::` imports |
| process-global secret registry raced | ragent-types | `#[serial]` on the four registry-touching files |
| process-global `XDG_CONFIG_HOME` raced (marketplace add/remove) | ragent-plugins | shared lock + shared marketplace-document registry lock |
| process-global `PATH` (recording `git` shim) leaked into every other git test | ragent-tools-vcs | shared lock in both directions |
| process-global SSRF test bypass leaked into the validation tests | ragent-tools-extended | fetch tests hold the lock; validation tests run under a `without_ssrf_bypass` guard |
| `OPENROUTER_API_KEY` / `OLLAMA_API_KEY` / `AWS_*` raced | ragent-llm | shared lock (the async-mutex variants became the same std lock) |
| ~24 cwd-mutating files in one binary | ragent-tui | all file-local cwd/env mutexes now delegate to `crate::global_env_mutex()` |
| tests inherited the real process cwd / real `~/.config` | ragent-tui, root | pin `app.cwd`/`app.cwd_path` (plugin-store browser) and take the lock where the value must be stable (`/prompt`, `/osinfo`) |
| a manual diagnostic permanently `chdir`ed into the workspace | ragent-tools-extended (`test_mf_orchestrator_diag`) | dropped the `set_current_dir` |

### 8.2 clippy under one binary

`clippy -D warnings` surfaced three new classes, each with a narrow, documented
allow:

* `clippy::duplicate_mod` — a shared `support/mod.rs` (or fixture) loaded as a
  module of several children. Allowed in `tests/all.rs`; hoisting would mean
  editing every child.
* `clippy::await_holding_lock` — the shared lock is a *std* mutex held across
  `await` deliberately (serialisation, not parallelism). Allowed per file on the
  tests that need it (`ragent-tui` osinfo, `ragent-tools-extended` fetch/supplement,
  `ragent-llm` openrouter/offline coverage, `ragent-tools-vcs` git suites).
* `dead_code` on stub/shim modules that only mirror a signature.

### 8.3 Verification

* `cargo test` over all 19 targets: **0 failures** (ragent-tui re-run twice to
  shake out ordering races; ragent-plugins and ragent-tools-vcs re-run twice).
  Counts: types 394, config 384, connectors 361, llm 195 (+2 ignored), plugins 492,
  tools-core 394, tools-vcs 397, tools-extended 1806 (+11 ignored), agent 1605
  (+1 ignored), tui 2719, bench 98, codeindex 447 (+2), research 471, server 192,
  specs 427, storage 346, surface 70, telemetry 231, root 44.
* `cargo clippy --workspace --all-targets -- -D warnings` (with CI's existing
  `-A` list): clean.
* `cargo fmt --all --check`: clean.
* All 11 `scripts/check-*.sh` guards pass.
* Every test-name set was compared before/after (`-- --list`, prefix stripped):
  identical per crate, 0 missing, 0 extra.

### 8.4 Known pre-existing issues left alone

* *(resolved)* `CI` invoked `--test test_channels` for `ragent-tools-extended`,
  a file that has not existed since 1.0.130. The consolidation converted it to
  the filter form `test(test_channels::)`, which selects 0 tests and so makes
  `cargo nextest` exit 4 (`error: no tests to run`) - a build failure, not the
  no-op it was thought to be. The `ms0-regression` job now runs the M0.5 cover
  that actually exists: `test_mf_security::`, `test_legacy_web_ssrf::` and
  `test_mf_supplement::` (the last needs `--features test-ssrf-bypass` for its
  loopback servers).
* `crates/ragent-codeindex/tests/support/mod.rs` still trips
  `-D unreachable_pub` under an explicit `RUSTFLAGS`; it did so in all 16 of its
  former binaries too.
