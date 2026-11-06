# Implementation Plan — hyperresearch Deep-Research Engine

**Spec ID:** hyperesearch
**Status:** draft

## Overview

The crate is built bottom-up: models and configuration first, then the vault and
note layer, then the index and search, then the fetch and scholarly provider
layers (which depend on the vault for persistence), then the graph, claims, and
citation-checking features, then the four surfaces (CLI, MCP, serve, export),
and finally the embedded skills plus the documentation and plugin manifests.

Every task is independently reviewable. Where a task exposes new behaviour, it
carries its own tests rather than deferring them to a later testing task.

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Create `crates/ragent-hyperresearch` crate: `Cargo.toml` with workspace metadata, edition 2024, `[[bin]] hyperresearch`, and library target | FR-001, FR-033 | S | Critical | Pending | — |
| T-002 | Write `src/lib.rs` with `#![deny(unsafe_code)]`, module declarations for all 12 modules, and crate-level `//!` docs | FR-002, FR-003 | S | Critical | Pending | T-001 |
| T-003 | Write thin `src/main.rs`: `anyhow`-based entry, argument collection, dispatch to `cli::run`, non-zero exit on error | FR-002, FR-024, NFR-006 | S | Critical | Pending | T-002 |
| T-004 | Implement `models` module: `Note`, `Frontmatter`, `Source`, `Claim`, `GraphEdge`, `RunRecord`, `SearchHit`, `ExportDocument` as serde types with `Debug` | FR-005, FR-018, FR-020, FR-021 | M | Critical | Pending | T-002 |
| T-005 | Implement `core::config`: three-layer resolution (file, env, flags), `Config` struct, profile selection, provider key fields, timeout and size limits | FR-004, FR-010 | M | Critical | Pending | T-004 |
| T-006 | Implement `core::frontmatter` parse/render with byte-identical round-trip, including key order, quoting, and trailing newline preservation | FR-005 | M | Critical | Pending | T-004 |
| T-007 | Implement `core::note`: load, render, create, rename, list; stable id derivation and collision detection returning a typed error naming both ids | FR-005, FR-006 | M | Critical | Pending | T-006 |
| T-008 | Implement `core::vault`: vault root resolution, note enumeration, per-note assets directory creation and attachment copy | FR-007 | M | High | Pending | T-007 |
| T-009 | Implement `core::db` SQLite bootstrap (WAL, busy timeout) and `core::migrations` version table with forward-only migration application before any read or write | FR-021 | M | Critical | Pending | T-005 |
| T-010 | Implement `search::fts` index build over titles, bodies, and tags with incremental update on note write | FR-017, FR-032 | M | High | Pending | T-008, T-009 |
| T-011 | Implement `search::filters` evidence filters (minimum quality, study design, year range) applied to the result set | FR-017 | S | High | Pending | T-010 |
| T-012 | Implement `core::untrusted` fence builder carrying source URL and emitting explicit untrusted delimiters; add escaping helpers | FR-015, FR-016 | S | Critical | Pending | T-004 |
| T-013 | Implement `web::safe_http`: TLS validation never disabled, post-redirect final-URL recheck, body size truncation, timeout, optional robots refusal | FR-009 | L | Critical | Pending | T-005 |
| T-014 | Define `web::base::WebProvider` trait and `FetchOutcome` enum (success, empty, junk, transport failure) | FR-008, FR-010 | S | Critical | Pending | T-012, T-013 |
| T-015 | Implement `web::builtin`: fetch, HTML-to-Markdown extraction, junk detection heuristics | FR-008, FR-010 | L | Critical | Pending | T-014 |
| T-016 | Implement `web::pdf` PDF text lane and wire it into the builtin pipeline | FR-008 | M | High | Pending | T-015 |
| T-017 | Implement `web::crawl4ai_provider` and `web::parallel_provider` against their HTTP APIs | FR-010 | M | Medium | Pending | T-014 |
| T-018 | Implement `web::exa_provider`, `web::tavily_provider`, `web::serply_provider` | FR-010 | M | Medium | Pending | T-014 |
| T-019 | Implement `core::fetcher` orchestration (fetch → extract → PDF lane → junk) producing structured results | FR-008 | M | Critical | Pending | T-015, T-016 |
| T-020 | Implement `fetch_many` fan-out with per-URL failure recording, provider-chain fallback, and batch-level continuation | FR-011 | M | Critical | Pending | T-017, T-018, T-019 |
| T-021 | Implement `scholar::base::ScholarlyProvider` trait, shared record model, and `scholar::registry` | FR-012 | M | High | Pending | T-004 |
| T-022 | Implement per-provider modules for OpenAlex, Crossref, ClinicalTrials, DOAB, EDGAR, FRED, REPEC and core, each with a recorded-fixture response parser | FR-012 | L | High | Pending | T-021 |
| T-023 | Implement `scholar::dedup`: DOI-first, then normalised title plus year, merging provenance across providers | FR-013 | M | High | Pending | T-022 |
| T-024 | Implement `core::oa` open-access resolution with alternate-source and landing-page recovery, reporting per-record outcome | FR-014 | M | Medium | Pending | T-023 |
| T-025 | Implement `core::enrich`, `core::quality`, `core::independence`, `core::similarity` scoring signals | FR-013, FR-017 | M | Medium | Pending | T-021 |
| T-026 | Implement `core::claims` claim extraction, persistence, and verification-state recomputation | FR-018 | M | High | Pending | T-008, T-009 |
| T-027 | Implement `core::citecheck` reporting unresolved citations, uncited notes, and unreachable stored source URLs | FR-019 | M | High | Pending | T-026 |
| T-028 | Implement `graph` module: edge derivation (`cites`, `supports`, `contradicts`, `same-source`) and `core::graphrank` centrality ranking | FR-020 | L | Medium | Pending | T-026 |
| T-029 | Implement `core::linker` and `core::runs` run records persisted on completion | FR-020, FR-021 | M | Medium | Pending | T-028 |
| T-030 | Implement `core::profiles`, `core::escalation`, `core::levers` research depth/effort controls applied to a run and recorded | FR-031 | M | Medium | Pending | T-029 |
| T-031 | Implement `core::embed` optional embedding-based similarity, gated on configuration | FR-017 | M | Low | Pending | T-025 |
| T-032 | Implement `core::render` Markdown assembly and `core::templates` / `core::patterns` reusable research templates | FR-030 | M | Medium | Pending | T-026 |
| T-033 | Implement `indexgen::generator` producing `INDEX.md` grouped by topic with id, title, last-updated | FR-032 | S | High | Pending | T-010 |
| T-034 | Implement `cli::_output` shared plain/JSON formatter and `cli::main` argument dispatch for all 35 commands | FR-023, FR-024, NFR-005 | L | Critical | Pending | T-003, T-005 |
| T-035 | Implement `cli::note`, `cli::fetch`, `cli::fetch_batch`, `cli::search`, `cli::index`, `cli::lint` | FR-023, FR-006, FR-011, FR-017 | M | Critical | Pending | T-034, T-020, T-010 |
| T-036 | Implement `cli::research` and `cli::batch` end-to-end run orchestration writing run records | FR-021, FR-023, FR-031 | L | Critical | Pending | T-035, T-030 |
| T-037 | Implement `cli::repair` rebuilding a drifted index and reporting each repaired item | FR-022 | M | High | Pending | T-010, T-035 |
| T-038 | Implement `cli::scholar`, `cli::claims`, `cli::citecheck`, `cli::dedup`, `cli::oa`-backed fetch reporting | FR-013, FR-018, FR-019, FR-023 | M | High | Pending | T-036, T-027 |
| T-039 | Implement `cli::graph`, `cli::link`, `cli::export`, `cli::import`, `cli::archive` | FR-020, FR-023, FR-030 | M | Medium | Pending | T-038, T-028 |
| T-040 | Implement `cli::config`, `cli::profile`, `cli::install`, `cli::setup`, `cli::assets`, `cli::template`, `cli::topic`, `cli::tag`, `cli::vault_tag`, `cli::watch`, `cli::git` | FR-023, FR-004, FR-007, FR-031 | L | Medium | Pending | T-034 |
| T-041 | Implement `cli::embed`, `cli::escalation`, `cli::levers` subcommands | FR-023, FR-031 | S | Low | Pending | T-040, T-031 |
| T-042 | Implement `mcp::server`: stdio JSON-RPC loop, schema validation, and tools `fetch_url`, `update_note`, `search`, `read_note`, `create_note` with structured errors | FR-025 | L | Critical | Pending | T-020, T-033 |
| T-043 | Implement `serve::renderer` with HTML escaping of every interpolated field and `serve::server` bind/serve for note, index, and report pages | FR-016, FR-026 | M | High | Pending | T-032, T-033 |
| T-044 | Author the 18 numbered pipeline skill markdown files plus umbrella skill under `src/skills/`, embedded with `include_str!` and exposed by name | FR-027 | L | High | Pending | T-002 |
| T-045 | Author `skills/deep-research/SKILL.md` and `skills/deep-research/agents/openai.yaml` as the human-facing entry points | FR-029 | S | Medium | Pending | T-044 |
| T-046 | Author `.claude-plugin/plugin.json`, `.codex-plugin/plugin.json`, and `.agents/plugins/marketplace.json` declaring the `deep-research` skill | FR-029 | S | Medium | Pending | T-045 |
| T-047 | Implement the golden-fixture check for embedded skill and prompt text, failing with the changed skill's name | FR-028 | M | High | Pending | T-044 |
| T-048 | Author `CHANGELOG.md`, `CONTRIBUTING.md`, `LICENSE`, `docs/source-search.md`, and `docs/roadmap-2.0/` phase documents | FR-033 | M | Medium | Pending | — |
| T-049 | Add consolidated test target: `autotests = false`, `[[test]] name = "all"`, `tests/all.rs` with `global_env_lock()`, `target/temp` scratch root, and `test_cli` submodules | NFR-002, NFR-004 | M | Critical | Pending | T-035 |
| T-050 | Add `test_core` submodules: frontmatter round-trip, hooks, levers, profiles, render, runs, migrations, sync, untrusted, vault, verification, source ranking, prompt goldens | NFR-002 | L | Critical | Pending | T-049, T-047 |
| T-051 | Add `test_web` submodules: certificate refusal, fencing, PDF lane, junk detection, final-URL recheck, settings, each provider, fetch-many fallback and gating | FR-009, FR-011, NFR-002 | L | Critical | Pending | T-049, T-020 |
| T-052 | Add `test_scholar`, `test_search`, `test_graph`, `test_mcp`, `test_serve` submodules including the serve XSS case | FR-013, FR-016, FR-017, FR-020, FR-025, NFR-002 | L | High | Pending | T-051 |
| T-053 | Add `.config/nextest.toml` profile and run `cargo fmt`, `cargo clippy`, and `cargo nextest run -p ragent-hyperresearch` to green | NFR-001, NFR-003 | M | Critical | Pending | T-050, T-052 |

## Sequencing Notes

1. **T-001 → T-012** delivers a compiling crate with configuration, the note and
   vault layer, the SQLite index, and the untrusted-content fence. This is the
   first reviewable milestone: a note can be created, listed, and searched.
2. **T-013 → T-025** delivers the network and scholarly layers. The safe-HTTP
   contract (T-013) lands before any provider so no provider can bypass it.
3. **T-026 → T-033** delivers claims, citation checking, the graph, runs, effort
   controls, and index generation — the analytical core.
4. **T-034 → T-041** delivers the CLI surface over the completed domain layer.
5. **T-042 → T-046** delivers the MCP server, the HTTP renderer, and the skill
   and plugin artifacts.
6. **T-047 → T-053** closes the loop with goldens, consolidated tests, and the
   formatting/lint gate.

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| Frontmatter byte-identical round-trip is hard with a generic YAML library | Preserve the original raw frontmatter text alongside the parsed form and re-emit it verbatim when no field changed (T-006). |
| Provider APIs change or are unavailable in CI | Every provider test uses a recorded fixture (T-022, T-051, NFR-002). |
| Untrusted content reaching HTML unnoticed | Escape in one renderer choke point and assert it with the XSS test (T-043, T-052). |
| Skill text drifting silently | Hash the embedded skills and fail the golden check with the changed name (T-047). |
| Consolidated test binary racing on process globals | One `global_env_lock()` plus `target/temp` scratch roots per test (T-049, NFR-004). |
| Batch fetch aborting on one bad URL | Per-URL error recording with chain fallback, exercised by the fetch-many tests (T-020, T-051). |
