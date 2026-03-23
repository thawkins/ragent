# FASTFILE: F17 — Concurrent File Operations

Feature ID: F17
Title: Concurrent file operations — Parallel file reads and edits for faster multi-file workflows
Status: Proposed

Overview
--------
Implement safe, efficient concurrent file operations within the codebase to enable parallel reading and editing of multiple files. This will speed up multi-file workflows (e.g., bulk refactors, mass edits, analysis passes) while preserving correctness and minimizing risk to repository integrity.

Goals
-----
- Provide a reusable concurrency abstraction for reading and editing files in parallel.
- Ensure edits are applied atomically and safely, with conflict detection and recoverable rollbacks.
- Integrate with existing task/test/CI workflows and follow repository coding, testing and documentation standards.
- Expose ergonomic APIs for higher-level tools (e.g., /simplify skill or code-mod utilities) to leverage parallelism.

Success criteria
----------------
- End-to-end benchmark: common multi-file operations complete at least 2x faster on multicore machines for medium-sized repos (100s of files).
- All changes pass CI with no data-loss incidents during feature rollout.
- Unit and integration tests for concurrency paths reach >90% coverage for core concurrency components.
- Clear docs and examples (examples/orchestration.rs or a new example) demonstrate usage.

Milestones
----------
M1 — Design and API (1 week)
- Deliverable: Design doc + API proposal, review-ready.
- Tasks:
  - T1.1: Survey existing file utilities and hotspots (lock usage, file model) — identify integration points.
  - T1.2: Draft concurrency model (threadpool vs tokio tasks), locking semantics, and edit-merge conflict strategy.
  - T1.3: Define public API surface (sync and async functions) and ergonomics for callers.

M2 — Core implementation (2 weeks)
- Deliverable: Library code implementing parallel reads and staged edits.
- Tasks:
  - T2.1: Implement a concurrent file reader optimized for batched reads with Rayon or tokio::fs (decide in M1).
  - T2.2: Implement a staging area abstraction: in-memory edits, checksums, and dry-run mode.
  - T2.3: Implement atomic write strategies (write to temp file + rename) and file locking for platforms supported.
  - T2.4: Implement conflict detection and a rollback mechanism.

M3 — Integration & API bindings (1 week)
- Deliverable: Integrations for /simplify, tools, and examples.
- Tasks:
  - T3.1: Provide wrappers for existing skills (/simplify) to call parallel operations safely.
  - T3.2: Add an example in examples/ showing a multi-file edit flow.
  - T3.3: Add crate-level features & docs for opt-in behavior.

M4 — Tests, benchmarks, and CI (1 week)
- Deliverable: Tests, benchmarks, CI job updates.
- Tasks:
  - T4.1: Unit tests for concurrency primitives and conflict scenarios.
  - T4.2: Integration tests under tests/ that simulate large multi-file runs (use fixtures/).
  - T4.3: Add benchmark tests (criterion or cargo bench) and CI profiling job.
  - T4.4: Run stress tests with many files to validate behavior.

M5 — Documentation, release prep & rollout (3 days)
- Deliverable: Docs, CHANGELOG entry, release checklist.
- Tasks:
  - T5.1: Write FASTFILE.md summary and move design doc to docs/ per guidelines.
  - T5.2: Update QUICKSTART.md or SPEC.md if user-facing APIs changed.
  - T5.3: Prepare migration notes and a rollback procedure.

Tasks (detailed)
----------------
- T1.1: Inventory files touched by existing bulk operations (owner: dev) — 1d
- T1.2: Decide runtime: Rayon for sync heavy CPU tasks, Tokio for async IO-bound tasks — 2d
- T2.1: Implement parallel reader with configurable concurrency and limited memory footprint — 3d
- T2.2: Create EditStaging struct: file path, original checksum, proposed content, status — 3d
- T2.3: Implement atomic write & cross-platform locking based on std::fs + tempfile — 4d
- T2.4: Conflict detection (checksum mismatch) and rollback loop — 2d
- T3.1: Add wrappers and feature flags, update /simplify to call new APIs behind flag — 2d
- T3.2: Write example usage and small tutorial — 1d
- T4.1: Unit & smoke tests — 2d
- T4.2: Integration: tests/fixtures multi-file scenario — 2d
- T4.3: Benchmarks and CI update — 2d
- T5.1: Docs & CHANGELOG — 1d

Estimates & timeline
--------------------
Total estimated effort: ~3–4 weeks (depending on review cycles).
Parallelization notes: Some tasks (tests, docs) can be done concurrently with implementation.

Acceptance criteria for each milestone
-------------------------------------
- M1 accepted when design doc is reviewed and API signatures are approved.
- M2 accepted when core primitives compile, pass unit tests and can run a dry-run multi-file edit with no disk writes.
- M3 accepted when example and wrapper compile and are exercised manually.
- M4 accepted when CI runs pass and benchmark shows performance improvement vs baseline.
- M5 accepted when docs are published and CHANGELOG & RELEASE.md updated.

Risk assessment
---------------
- Data loss from incorrect atomic writes: mitigate with strong tests, write-to-temp + rename and checksums.
- Cross-platform file locking differences: limit scope to POSIX+Windows supported semantics and document limitations.
- Deadlocks from improper locking order: use consistent lock ordering and prefer optimistic concurrency where possible.

Testing strategy
----------------
- Unit tests for each primitive (reader, staging, writer).
- Integration tests with isolated temporary repos (temp/ or tests/fixtures) including concurrent mutator simulation.
- Stress tests under CI with variable concurrency levels.
- Post-merge manual canary run on a non-critical repo.

Developer notes / Implementation guidance
----------------------------------------
- Prefer explicit types and use anyhow::Result in binaries to align with guidelines.
- Use tracing for structured logs and avoid println!.
- Follow formatting rules (4-space indent). Use rustfmt and clippy.
- Place new public docs in docs/ and keep FASTFILE.md as the planning artifact in repo root.

Checklist before merging
------------------------
- [ ] Design doc reviewed
- [ ] Unit tests passing
- [ ] Integration tests passing
- [ ] Benchmarks added and baseline recorded
- [ ] Docs updated in docs/
- [ ] CHANGELOG.md updated
- [ ] RELEASE.md updated with summary

Owners & stakeholders
---------------------
- Feature owner: (TBD)
- Reviewers: core maintainers

Notes
-----
This plan intentionally keeps the implementation choices (Rayon vs Tokio) deferred to the design phase to allow performance testing and alignment with the rest of the codebase. Prefer small incremental merges guarded by feature flags so rollout can be reversed quickly if issues are found.
