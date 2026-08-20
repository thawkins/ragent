# GitHub Actions Reliability Improvement Plan

## 1. Executive Summary

This plan analyses the last 50 GitHub Actions workflow runs for the `ragent`
repository and proposes concrete steps to reduce CI failure rates. The current
failure pattern is concentrated almost entirely in the `CI` workflow. Other
workflows (`Security Audit`, `Build and Release`, `CI Benchmarks`) have
historically been stable.

| Workflow          | Runs (last 50) | Success | Failure | Other |
| ----------------- | -------------- | ------- | ------- | ----- |
| CI                | 12             | 4       | 7       | 1     |
| Build and Release | 13             | 11      | 0       | 2     |
| Security Audit    | 13             | 13      | 0       | 0     |
| CI Benchmarks     | 12             | 11      | 0       | 1     |

The `CI` workflow failure rate of **≈ 58%** is driven by a small set of
preventable lint / build issues that are cheap to catch locally.

## 2. Failure Pattern Analysis

### 2.1 Job-level failure counts

| Failing Job                    | Occurrences | Root Cause Category                                      |
| ------------------------------ | ----------- | -------------------------------------------------------- |
| Dead-code reason check         | 4           | Undocumented `#[allow(dead_code)]` attributes committed |
| Clippy                         | 3           | Clippy warnings pushed without local verification       |
| Dead-code lint                 | 2           | Vendored / newly-introduced code triggers `-D` lints    |
| Check & Test (default features)| 2           | Compilation errors from incomplete refactors             |
| Rustfmt                        | 1           | Unformatted code pushed without `cargo fmt`             |

### 2.2 Detailed root causes observed

1. **Dead-code reason check**
   - `scripts/check-dead-code-reasons.sh` requires every
     `#[allow(dead_code)]` attribute in `crates/*/src` to have an explanatory
     comment within two lines.
   - Failures are caused by contributors adding suppressions without the
     required `// reason: ...` comment.
   - Recent examples: `crates/ragent-tools-extended/src/finance/tools/mod.rs`,
     `crates/ragent-agent/src/session/archive.rs`.

2. **Clippy**
   - The workflow runs `cargo clippy --workspace -- -D warnings` with a small
     allow-list.
   - Failures are from new code that introduces warnings not on the allow-list
     (e.g., `useless_format`, `collapsible_if`, `needless_raw_string_hashes`,
     `vec_init_then_push`).
   - Recent examples: `crates/ragent-llm/src/providers/openai_responses.rs`,
     `crates/ragent-agent/src/template/mod.rs`.

3. **Dead-code lint**
   - This job sets
     `RUSTFLAGS='-D unreachable_pub -D dead_code -D unused_imports'` and runs
     `cargo check --workspace --lib --all-features`.
   - It flags items that are `pub` but not re-exported, dead code, and unused
     imports.
   - A recent failure came from the vendored `pdf-extract` crate, whose
     public-but-unused lookup tables are not part of the workspace project
     code.

4. **Check & Test (default features)**
   - The most severe category because it blocks the test suite.
   - Recent example: a field `no_scholarly` was removed from a clap enum
     variant in `crates/ragent-research/src/cli.rs` but two pattern matches
     still referenced it.
   - Also generated dead-code warnings in `ragent-tools-core` tests, which are
     harmless but indicate test code is not kept in sync with source changes.

5. **Rustfmt**
   - A single failure from `crates/ragent-tools-core/src/apply_patch.rs` being
     committed without running `cargo fmt`.

## 3. Strategy

The overarching goal is to **shift-left**: catch the same errors before commit
using automation that runs on the developer machine and, where possible, in a
pre-merge check. The CI failures above are all machine-detectable and should
not reach `main`.

### 3.1 Guiding principles

1. **Local-first**: make it trivial to run the exact CI checks locally.
2. **Fail fast in CI**: reorder jobs so the cheapest checks run first.
3. **Protect the vendored boundary**: do not apply project-wide lint policy to
   third-party vendored code.
4. **Document everything**: every lint suppression must carry a reason.
5. **Merge-gate with required checks**: ensure no PR can merge until `CI` is
   green.

## 4. Milestones and Tasks

### Milestone 1 — Local verification script and pre-commit hook

**Objective:** ensure every developer can reproduce the full CI check suite in
one command, and block the most common failures before push.

- [ ] **T-001** Create `scripts/ci-check.sh` that runs, in order:
  1. `cargo fmt --all -- --check`
  2. `cargo clippy --workspace -- -D warnings -A clippy::used_underscore_items -A clippy::redundant_pub_crate -A clippy::wildcard_imports`
  3. `bash scripts/check-dead-code-reasons.sh`
  4. `RUSTFLAGS='-D unreachable_pub -D dead_code -D unused_imports' cargo check --workspace --lib --all-features`
  5. `cargo test --workspace`
- [ ] **T-002** Add a pre-commit hook (`.git/hooks/pre-commit`) or a
      `cargo-husky`-style managed hook that runs `scripts/ci-check.sh --staged`
      on staged Rust files. Document installation in `QUICKSTART.md`.
- [ ] **T-003** Update `AGENTS.md` / contributor docs to require `cargo fmt` and
      `cargo clippy` before pushing.

**Success criteria:** a fresh contributor can run one command and get the same
results as the `CI` workflow.

### Milestone 2 — Vendored code lint boundary

**Objective:** stop the `Dead-code lint` job from failing because of vendored
third-party code.

- [ ] **T-004** Decide the vendored lint policy:
  - Option A: exclude vendored crates from the `-D` RUSTFLAGS check by running
    `cargo check --workspace` and using per-crate `[lints.rust]` tables in the
    vendor `Cargo.toml` files.
  - Option B: keep the vendored crate under `-D` lints but add explicit crate
    level `allow` attributes (e.g., `#![allow(unreachable_pub, dead_code,
    unused_imports)]`) with a comment explaining the code is vendored.
- [ ] **T-005** Apply the chosen policy to `vendor/pdf-extract` (and any future
      vendored crates). Add a comment such as:
      ```rust
      // Vendored third-party crate: suppress lints that are project-level but
      // not relevant to verbatim imported code.
      #![allow(unreachable_pub, dead_code, unused_imports)]
      ```
- [ ] **T-006** Add a CI regression test that verifies the `Dead-code lint` job
      passes after the boundary change.

**Success criteria:** adding a new vendored crate cannot break the
`Dead-code lint` job unless workspace project code is responsible.

### Milestone 3 — Improve the dead-code reason check

**Objective:** reduce the most common failure (undocumented dead-code
suppressions) by making the script more helpful and enforceable locally.

- [ ] **T-007** Enhance `scripts/check-dead-code-reasons.sh` so it:
  - prints the offending line of code, not just `file:line`;
  - supports a `--fix` mode that adds a generic `// reason: ...` placeholder
    comment for suppressions that are missing one;
  - exits with a clear, actionable message.
- [ ] **T-008** Add a unit test for the reason-check script itself in
      `scripts/tests/` or under `tests/` so changes to the script are validated.
- [ ] **T-009** Run the reason check in the local verification script
      (Milestone 1) so contributors catch missing comments before push.

**Success criteria:** the `Dead-code reason check` job does not fail on `main`
for one full release cycle.

### Milestone 4 — Enforce clippy and rustfmt in the same CI job

**Objective:** reduce the time from push to first failure and reduce the
number of fix-up commits.

- [ ] **T-010** Reorder the `CI` workflow jobs so fast, deterministic checks run
      before slow, expensive ones:
  1. `fmt`
  2. `dead-code-reasons`
  3. `dead-code-lint`
  4. `clippy`
  5. `check-and-test`
- [ ] **T-011** Consider merging `fmt` and `clippy` into a single lightweight
      `lint` job so formatting failures are reported alongside clippy warnings,
      reducing total runner minutes.
- [ ] **T-012** Set the `fmt` and `dead-code-reasons` jobs as **required
      checks** in the GitHub branch-protection rules so they block merging.

**Success criteria:** average time-to-first-failure for preventable lint issues
is under two minutes.

### Milestone 5 — Branch protection and merge queue

**Objective:** guarantee that `main` is always green by preventing direct
pushes and untested merges.

- [ ] **T-013** Configure branch protection for `main`:
  - require a pull request before merging;
  - require the `CI` workflow jobs to pass;
  - require the `Security Audit` workflow to pass (currently 100% success);
  - dismiss stale PR approvals when new commits are pushed;
  - require linear history.
- [ ] **T-014** Evaluate GitHub Merge Queue or require branches to be up to date
      before merging, so changes that pass in isolation but break together do
      not land on `main`.
- [ ] **T-015** Document the branch-protection settings in
      `docs/GHACTPLAN.md` and maintain a runbook for emergencies.

**Success criteria:** no commit reaches `main` unless all required checks pass.

### Milestone 6 — CI hygiene and observability

**Objective:** keep CI fast, reliable, and easy to diagnose.

- [ ] **T-016** Add a `continue-on-error: false` summary step to the `CI`
      workflow that uploads `Cargo.lock`, build logs, and test backtraces as
      artifacts on failure.
- [ ] **T-017** Add a CI run link and job-status summary as a comment on PRs
      using a lightweight action, or rely on GitHub’s native checks UI.
- [ ] **T-018** Review the use of `jlumbroso/free-disk-space@main` in the `CI`
      workflow. Pin it to a release SHA and measure whether the saved minutes
      justify the long setup time (~2 minutes) for every run.
- [ ] **T-019** Audit Node.js 20 deprecation warnings in `actions/checkout@v4`
      and related actions; update to versions that run natively on Node 24.

**Success criteria:** CI failures provide enough context that a contributor can
diagnose them without re-running locally.

## 5. Success Metrics

| Metric                              | Baseline (last 50 runs) | Target (after 50 runs) |
| ----------------------------------- | ----------------------- | ---------------------- |
| CI workflow failure rate            | ≈ 58%                   | < 10%                  |
| `Dead-code reason check` failures   | 4                       | 0                      |
| `Clippy` failures                   | 3                       | 0                      |
| `Dead-code lint` failures           | 2                       | 0                      |
| `Check & Test` failures             | 2                       | < 1                    |
| `Rustfmt` failures                  | 1                       | 0                      |
| Time-to-first-failure (preventable) | ~2–5 minutes            | < 2 minutes            |

## 6. Risks and Mitigations

| Risk                                                     | Mitigation                                                             |
| -------------------------------------------------------- | ---------------------------------------------------------------------- |
| Local check script becomes too slow for pre-commit       | Provide a `--fast` mode that runs only `fmt`, `dead-code-reasons`, and `check` |
| Branch protection blocks urgent hotfix pushes              | Document an override process for maintainers with admin privileges     |
| Vendored crates still need updates that conflict with `allow` | Pin vendored revisions and update them in dedicated PRs with full CI   |
| Reordering jobs hides slow failures that still take time   | Keep the slow `check-and-test` job as required and add timeouts        |
| Contributors bypass the hook                               | Branch protection and required checks remain as the final gate          |

## 7. Timeline

| Milestone | Estimated Effort | Owner        | Target Completion |
| --------- | ---------------- | ------------ | ----------------- |
| 1         | 1 day            | CI / Tooling | Week 1            |
| 2         | 0.5 days         | CI / Tooling | Week 1            |
| 3         | 1 day            | CI / Tooling | Week 2            |
| 4         | 0.5 days         | CI / Tooling | Week 2            |
| 5         | 0.5 days         | Repository Admin | Week 2        |
| 6         | 1 day            | CI / Tooling | Week 3            |

## 8. Next Steps

1. Implement **T-001** and **T-002** so contributors can run the full CI check
   suite locally.
2. Apply the vendored lint boundary (**T-004**–**T-006**) to prevent
   third-party code from breaking project lint jobs.
3. Improve the dead-code reason script (**T-007**–**T-009**) and enforce it in
   branch protection.
4. Reorder CI jobs and set required checks (**T-010**–**T-012**).
5. Configure branch protection and, if appropriate, a merge queue (**T-013**–
   **T-015**).
6. Add CI observability improvements (**T-016**–**T-019**) and measure the new
   failure rate over the next 50 runs.
