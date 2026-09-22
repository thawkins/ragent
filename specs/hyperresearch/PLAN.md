# PLAN: Hyperresearch integration for ragent /research

## Overview

This plan implements the `hyperresearch` specification by extending the existing `ragent-research` crate with tiered pipelines, a persistent source vault, adversarial audit steps, contradiction tracking, open-access recovery, and resumable run manifests. It reuses the existing `mf_search` infrastructure and SQLite storage.

## Architecture

```
User input (CLI or TUI /research)
    │
    ▼
ResearchOrchestrator
    ��
    ├─→ RunManifest (read/write state, resume)
    ├─→ SourceVault (SQLite + markdown files)
    ├─→ TierRouter (light / full / dissertation)
    │       ├─→ Pipeline steps
    │       ├─→ Critic agents (subagent calls)
    │       └─→ Cite-checker
    ├─→ WebGatherer (existing) with OA recovery
    └─→ Document renderer (RESEARCH.md)
```

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Add `--tier` CLI option and `/research --tier` slash parser with `full` default | FR-001 | S | High | completed | — |
| T-002 | Introduce `RunManifest` with step tracking and resume logic | FR-007 | M | High | completed | T-001 |
| T-003 | Implement `SourceVault` SQLite schema and file storage | FR-002, FR-003 | M | High | completed | — |
| T-004 | Wire vault lookup into `WebGatherer` before new search | FR-009 | M | High | completed | T-003 |
| T-005 | Build tier router (light / full / dissertation) with default-to-full behavior | FR-005, FR-008 | M | High | completed | T-001, T-002 |
| T-006 | Implement width sweep using `mf_search` parallel backends | FR-005 | M | High | completed | T-004 |
| T-007 | Add contradiction graph step and `ContradictionGraph` model | FR-005 | L | Medium | completed | T-006 |
| T-008 | Add loci analysis and depth investigation steps | FR-005 | L | Medium | completed | T-006 |
| T-009 | Add cross-locus reconcile and source tensions steps | FR-005 | L | Medium | completed | T-008 |
| T-010 | Add corpus critic and gap-fill fetch step | FR-005 | L | Medium | completed | T-009 |
| T-011 | Add evidence digest and triple draft steps | FR-005 | L | Medium | completed | T-006 |
| T-012 | Implement synthesis and 4-critic audit subagents | FR-005 | L | High | completed | T-011 |
| T-013 | Implement surgical patcher for draft revisions | FR-005 | L | Medium | completed | T-012 |
| T-014 | Implement cite-checker with failure gate | FR-006, FR-014 | M | Critical | completed | T-003, T-013 |
| T-015 | Implement polish and readability audit steps | FR-005 | S | Low | completed | T-014 |
| T-016 | Implement dissertation chapter partitioning with `--chapter-count` | FR-013 | L | Medium | completed | T-005 |
| T-017 | Implement open-access recovery via Unpaywall and Europe PMC | FR-010 | M | High | completed | T-003 |
| T-018 | Add `research.open_access_recovery` and `research.contact_email` config fields | FR-011, FR-012 | S | Medium | completed | T-017 |
| T-019 | Add OA recovery disclosure to `RESEARCH.md` frontmatter and CLI output | FR-015 | S | Medium | completed | T-017 |
| T-020 | Update `Document` renderer to emit contradiction graph, source tensions, and cite-check summary | FR-004, FR-005 | M | High | completed | T-007, T-009, T-014 |
| T-021 | Add sufficient-source check to avoid unnecessary new fetches | FR-016 | M | Medium | completed | T-004 |
| T-022 | Add manual test cases and update CHANGELOG | — | S | Low | completed | T-014, T-020 |
## Milestones

1. **Foundation** (T-001–T-005, T-018, T-021): tier routing, vault, manifest, config, vault-reuse guard. Acceptance: `/research --tier light` works, writes a vault, and does not over-fetch when sources already exist.
2. **Core pipeline** (T-006–T-015): full-tier steps and cite-check gate. Acceptance: `/research --tier full` produces an audited report with contradiction graph, source tensions, and cite-check summary.
3. **Advanced features** (T-016–T-022): dissertation mode, OA recovery, rendering, docs. Acceptance: paywalled DOI recovered and disclosed; dissertation mode partitions chapters; CHANGELOG updated.

## Risk register

| Risk | Mitigation |
|------|------------|
| Parallel subagent pipeline exceeds context budget | Each step is a separate subagent invocation with a minimal prompt and vault references, mirroring Hyperresearch's skill-per-phase approach. |
| Unpaywall API rate limits | Require user-supplied `contact_email`; cache responses; fall back to Europe PMC. |
| Cite-checker false positives | Make the gate user-approvable and log the exact source span that failed verification. |
| SQLite vault contention | Use the existing `ragent-storage` connection pool and WAL mode. |
| Default-to-full tier causes unexpectedly long runs | Document the default in help text; allow `--tier light` for quick runs; respect autopilot timeout limits. |

## Success criteria

- All acceptance criteria in `SPEC.md` pass during manual test execution.
- `cargo check -p ragent-research` succeeds.
- `cargo test -p ragent-research` passes for automated unit tests added as part of implementation.