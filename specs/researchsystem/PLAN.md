# Implementation Plan: Research System

## Overview

This plan implements the Research System defined in `SPEC.md`. The approach is incremental: build the core data structures and file I/O in a new `ragent-research` crate first, then add the gathering-orchestration engine that drives the web/local research session, then wire up the TUI `/research` slash command, the CLI sub-commands, the HTTP endpoint, and the spec-integration glue. Each milestone delivers a working increment that can be demonstrated and tested independently.

The implementation spans one new crate and integration points in four existing crates:

- `ragent-research` (new) — core logic: name validation, item CRUD, gathering orchestration, file I/O
- Integration points in `ragent-tui` — `/research` slash command, autocomplete, viewer
- Integration points in `ragent-agent` — agent driver that calls the gathering tools
- Integration points in `ragent-server` — `POST /research` HTTP endpoint
- Integration points in `ragent-specs` — `research:` dependency line in PLAN.md

---

## Milestones

### Milestone 1: Core Data Structures and File I/O

**Deliverable:** A standalone `ragent-research` crate that can create, read, update, and delete research items on disk.

- Define `ResearchName`, `ResearchStatus`, `Source`, `ResearchItem` structs
- Implement `ResearchName::new()` with full FR-002 validation
- Implement directory creation under `research/<name>/` with atomic writes
- Implement `RESEARCH.md` skeleton generation with frontmatter and empty sections
- Implement `ResearchManager` with `create`, `list`, `open`, `show`, `delete`, `archive` methods
- Maintain `research/INDEX.md` derived cache (FR-012)
- Unit tests for name validation, lifecycle, and atomic writes

### Milestone 2: Gathering Orchestration Engine

**Deliverable:** A research session driver that combines web and local information gathering and writes a fully populated `RESEARCH.md`.

- Implement `ResearchSession` with configurable topic and sources-dir
- Implement web-gathering phase using `websearch` + `webfetch` (FR-006, FR-007)
- Implement local-gathering phase using `glob` + `grep` + `read` (FR-008, FR-009)
- Implement cross-referencing that scores project resources by relevance (FR-009)
- Implement `RESEARCH.md` assembly with all eight required sections (FR-010)
- Implement References Index generation with sequential numbering (FR-011)
- Implement `--sources-dir` flag handling (FR-019)
- Implement template loading and variable substitution (FR-020)
- Integration tests against a fixture project

### Milestone 3: TUI Slash Command and Autocomplete

**Deliverable:** The `/research` slash command family works in the TUI, parallel to the existing `/spec` command.

- Register `/research` with all sub-commands (FR-003, FR-004)
- Implement autocomplete dropdown showing sub-commands and existing research names (FR-022)
- Implement `/research create` triggering `ResearchSession` and streaming progress to the log panel
- Implement `/research list` rendering a tabular view
- Implement `/research open` opening the markdown viewer
- Implement `/research search` performing full-text search across all `RESEARCH.md` files
- Implement `/research show` rendering item metadata
- Implement `/research delete` with confirmation prompt
- Wire up concurrent-edit detection in the viewer (FR-021)
- TUI snapshot tests

### Milestone 4: CLI and HTTP Endpoints

**Deliverable:** `ragent research <subcommand>` CLI and `POST /research` HTTP endpoint.

- Implement `ragent research create|list|open|search|show|delete|archive` (FR-014)
- Emit machine-readable progress JSON to stdout in CLI mode
- Implement `POST /research` accepting `{name, topic, sources_dir?, template?}`
- Implement `GET /research` returning the index as JSON
- Implement `GET /research/<name>` returning a single item
- Implement `DELETE /research/<name>` with confirmation token
- CLI integration tests using `assert_cmd`

### Milestone 5: Spec Integration and Hardening

**Deliverable:** Research items can be linked from specs and the full feature is production-ready.

- Parse `research: <name>` line in PLAN.md (FR-015)
- Add `Related Research` section template to `ragent-specs`
- Implement `--from-research <name>` flag on `/spec create` (FR-015)
- Surface research links in `/spec list` output
- Add security review: verify no code execution from web sources (NFR-006)
- Add performance benchmarks for the gathering engine (NFR-001)
- Update `SPEC.md`, `QUICKSTART.md`, and `README.md` with research workflow docs
- Add end-to-end test that runs `/research`, then `/spec --from-research`, then implements the spec

---

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Define `ResearchName` newtype with FR-002 validation | FR-002 | S | Critical | completed | — |
| T-002 | Define `ResearchStatus` enum (draft, in-progress, complete, archived) | FR-013 | S | Critical | completed | — |
| T-003 | Define `Source` enum (Web/Local/Spec/Other variants) | NFR-007 | S | Critical | completed | — |
| T-004 | Define `ResearchItem` struct with frontmatter fields | FR-005 | S | Critical | pending | T-001, T-002, T-003 |
| T-005 | Create `ragent-research` crate skeleton with Cargo.toml | NFR-007 | S | Critical | completed | — |
| T-006 | Implement atomic directory + `RESEARCH.md` skeleton creation | FR-001, FR-005 | M | Critical | pending | T-004, T-005 |
| T-007 | Implement `ResearchManager::create` with name validation and FR-001 layout | FR-001, FR-002 | M | Critical | pending | T-006 |
| T-008 | Implement `ResearchManager::list` with status filtering and sort | FR-004, FR-013 | M | High | pending | T-006 |
| T-009 | Implement `ResearchManager::show` returning parsed item | FR-004 | S | High | pending | T-006 |
| T-010 | Implement `ResearchManager::delete` with confirmation hook | FR-004 | S | High | pending | T-006 |
| T-011 | Implement `ResearchManager::archive` setting status=archived | FR-013 | S | Medium | pending | T-008 |
| T-012 | Implement `research/INDEX.md` derived cache writer | FR-012 | M | High | pending | T-006 |
| T-013 | Implement atomic write-then-rename for all file operations | NFR-002 | S | High | pending | T-006 |
| T-014 | Implement web-gathering phase using websearch + webfetch | FR-006, FR-007 | L | Critical | pending | T-005 |
| T-015 | Implement `sources/web-<NN>.md` supporting-file writer | FR-007 | S | High | pending | T-014 |
| T-016 | Implement local-gathering phase using glob + grep + read | FR-006, FR-008 | L | Critical | pending | T-005 |
| T-017 | Implement `sources/local-<NN>.md` excerpt writer | FR-008 | S | High | pending | T-016 |
| T-018 | Implement in-project cross-referencing scoring | FR-009 | L | High | pending | T-016 |
| T-019 | Implement `ResearchSession` orchestrating all gathering phases | FR-006 | L | Critical | pending | T-014, T-016, T-018 |
| T-020 | Implement `RESEARCH.md` section assembly (8 required sections) | FR-010 | M | Critical | pending | T-019 |
| T-021 | Implement References Index table generation | FR-011 | M | Critical | pending | T-015, T-017, T-020 |
| T-022 | Implement numbering of references and cross-document citations | FR-010 | S | High | pending | T-021 |
| T-023 | Implement `--sources-dir` flag handling | FR-019 | S | Medium | pending | T-016 |
| T-024 | Implement research template loading and `{{var}}` substitution | FR-020 | M | Low | pending | T-020 |
| T-025 | Register `/research` slash command in TUI input handler | FR-003 | M | Critical | pending | T-007 |
| T-026 | Implement TUI autocomplete for sub-commands and names | FR-022 | M | High | pending | T-025, T-008 |
| T-027 | Implement `/research create` triggering `ResearchSession` with live log streaming | FR-003, FR-006 | L | Critical | pending | T-019, T-025 |
| T-028 | Implement `/research list` TUI view (tabular) | FR-004 | M | High | pending | T-008, T-025 |
| T-029 | Implement `/research open` opening markdown viewer | FR-004, FR-021 | M | High | pending | T-009, T-025 |
| T-030 | Implement `/research search` full-text search | FR-004 | M | High | pending | T-025 |
| T-031 | Implement `/research show` metadata view | FR-004 | S | High | pending | T-009, T-025 |
| T-032 | Implement `/research delete` confirmation flow | FR-004 | M | High | pending | T-010, T-025 |
| T-033 | Implement concurrent-edit detection in markdown viewer | FR-021 | M | Medium | pending | T-029 |
| T-034 | Implement `ragent research` CLI sub-commands | FR-014 | M | High | pending | T-007–T-011 |
| T-035 | Implement JSON progress emitter for CLI mode | FR-014 | S | Medium | pending | T-034 |
| T-036 | Implement `POST /research` HTTP endpoint | NFR-007 | M | High | pending | T-019, T-034 |
| T-037 | Implement `GET /research` index endpoint | NFR-007 | S | High | pending | T-012, T-034 |
| T-038 | Implement `GET /research/<name>` single-item endpoint | NFR-007 | S | High | pending | T-009, T-037 |
| T-039 | Implement `DELETE /research/<name>` with confirmation token | NFR-007 | M | Medium | pending | T-010, T-037 |
| T-040 | Parse `research: <name>` line in PLAN.md | FR-015 | S | Medium | pending | T-005 |
| T-041 | Add `Related Research` section template to ragent-specs | FR-015 | S | Medium | pending | T-040 |
| T-042 | Implement `--from-research <name>` flag on `/spec create` | FR-015 | M | Medium | pending | T-040, T-041 |
| T-043 | Surface research links in `/spec list` output | FR-015 | S | Medium | pending | T-040 |
| T-044 | Implement duplicate-name error path with suggestion (FR-016) | FR-016 | S | High | pending | T-007 |
| T-045 | Implement path-traversal rejection (FR-017) | FR-017 | S | High | pending | T-001 |
| T-046 | Implement "not found" error with three closest names (FR-018) | FR-018 | S | High | pending | T-009 |
| T-047 | Add `tracing::info!` calls for session lifecycle and source capture | NFR-004 | S | High | pending | T-019 |
| T-048 | Implement path sanitization for portable output | NFR-005 | S | High | pending | T-020 |
| T-049 | Implement untrusted-source escaping/fencing in RESEARCH.md | NFR-006 | M | High | pending | T-020 |
| T-050 | Add criterion benchmark for gathering engine | NFR-001 | M | Low | pending | T-019 |
| T-051 | Add unit tests for name validation (15+ cases) | FR-002 | S | High | pending | T-001 |
| T-052 | Add integration tests for full create→list→delete flow | FR-001, FR-004, FR-013 | M | High | pending | T-007, T-008, T-010 |
| T-053 | Add TUI snapshot tests for `/research` views | FR-004, FR-022 | M | Medium | pending | T-026, T-028 |
| T-054 | Add end-to-end test: research → spec → implement | FR-015 | L | Medium | pending | T-027, T-042 |
| T-055 | Update `SPEC.md`, `QUICKSTART.md`, `README.md` with research workflow | NFR-007 | S | High | pending | T-027, T-034 |
| T-056 | Add Research section to `docs/research.md` user guide | NFR-007 | S | Medium | pending | T-055 |
## Risks & Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Web search API rate limits interrupt long research sessions | Medium | Medium | Cache results in `sources/web-<n>.md`; allow resume via `/research create --resume <name>` (future) |
| LLM agent hallucinates file paths in cross-references | Medium | High | Always validate that a referenced path exists before writing it to the references index; mark unverified items with `verified: false` |
| Research items balloon in size for popular topics | Low | Medium | Implement per-source size cap (default 256 KiB) and warn when exceeded |
| `research/` namespace collides with future ragent features | Low | Low | Reserve the entire `research/` top-level directory in the SPEC; document the reservation in `SPEC.md` |
| Web sources contain malicious content attempting prompt injection | Medium | High | Per NFR-006, treat all web content as untrusted text; fence in ` ``` ` blocks in `RESEARCH.md`; never execute embedded code |
| Concurrent TUI viewer + CLI write leads to lost updates | Low | Medium | Atomic write-then-rename (NFR-002) + mtime-based reload detection (FR-021) |

---

## Definition of Done

- All FR-001 through FR-022 requirements implemented and verified by tests
- All NFR-001 through NFR-007 non-functional requirements measured and documented
- `ragent-research` crate builds cleanly on Linux, macOS, and Windows
- `cargo test -p ragent-research` passes with ≥ 90% line coverage
- `cargo bench -p ragent-research` runs and reports a baseline
- TUI `/research` slash command family is documented in the TUI help screen
- CLI `ragent research` sub-commands appear in `ragent --help`
- HTTP endpoints documented in `docs/api.md` (or equivalent)
- End-to-end test (T-054) passes
- `SPEC.md`, `QUICKSTART.md`, `README.md`, and `docs/research.md` updated
- The research workflow is demonstrated in a recorded or scripted walkthrough committed to `examples/research/`