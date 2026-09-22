# Implementation Plan: Hermes — Hound Dependency-Security Tools

**Spec ID:** `hermes`
**Spec status:** draft

## Overview

This plan implements the Hermes specification (`specs/hermes/SPEC.md`): the
integration of 12 Hound dependency-security tools into ragent as native Rust
tools in the `crates/ragent-tools-extended` crate. The work involves:

1. A shared API client layer (`depsdev`, `osv`) with retry/backoff.
2. A lockfile parser module covering 11+ formats.
3. Licence classification and ecosystem mapping constants.
4. 12 tool structs implementing the `Tool` trait.
5. Registration in `create_extended_registry()`.
6. A `hermes` tool-visibility switch in `ragent-config`.
7. Unit and integration tests.

No MCP server, Node.js, or external subprocess is involved.

## Architecture

```
crates/ragent-tools-extended/src/
├── hermes/
│   ├── mod.rs              # Module root, re-exports, shared types
│   ├── depsdev.rs          # deps.dev API client (GET endpoints)
│   ├── osv.rs              # OSV API client (POST /query, /querybatch, GET /vulns)
│   ├── http.rs             # Shared reqwest client + retry/backoff helper
│   ├── ecosystem.rs        # Ecosystem enum, deps.dev/OSV name mapping, validation
│   ├── licenses.rs         # COPYLEFT / NETWORK_COPYLEFT sets, classify_license()
│   ├── lockfile.rs         # 11+ lockfile parsers → Vec<ParsedDependency>
│   ├── typosquat.rs        # generate_typos() pure function
│   ├── score.rs            # Hound Score computation (pure functions)
│   ├── default_version.rs  # getDefaultVersion helper
│   └── tools/
│       ├── mod.rs           # Registers all 12 tools
│       ├── audit.rs         # hound_audit
│       ├── score_tool.rs    # hound_score  (avoids clash with score.rs)
���       ├── compare.rs       # hound_compare
│       ├── preinstall.rs    # hound_preinstall
│       ├── upgrade.rs       # hound_upgrade
│       ├── license_check.rs # hound_license_check
│       ├── vulns.rs         # hound_vulns
│       ├── inspect.rs       # hound_inspect
│       ├── tree.rs          # hound_tree
│       ├── typosquat_tool.rs# hound_typosquat
│       ├── advisories.rs    # hound_advisories
│       └── popular.rs       # hound_popular
└── lib.rs                  # + hermes module, + registration in create_extended_registry()

crates/ragent-config/src/config.rs  # + hermes visibility switch
```

### Data flow

```
Agent invokes hound_<tool>
        │
        ▼
Tool::execute(input, ctx)
        │
        ├─► hermes::depsdev::*  ──►  hermes::http::get_with_retry  ──►  api.deps.dev
        ├─► hermes::osv::*      ──►  hermes::http::post_with_retry ──►  api.osv.dev
        ├─► hermes::lockfile::parse_lockfile  (for audit / license_check)
        ├─► hermes::score / typosquat / licenses  (pure logic)
        │
        ▼
Formatted text report → ToolOutput { content, metadata }
```

## Tasks

| ID | Title | Requirement | Effort | Priority | Dependencies |
|----|-------|-------------|--------|----------|--------------|
| T-001 | Create `hermes/` module structure with shared types (`Ecosystem`, `ParsedDependency`, `Severity`, `Vulnerability`, `HoundScore`) | FR-001, FR-005, NFR-004 | M | Critical | — |
| T-002 | Implement `hermes::ecosystem` — enum, deps.dev/OSV name maps, validation | FR-005 | S | Critical | T-001 |
| T-003 | Implement `hermes::http` — shared `reqwest::Client`, `USER_AGENT`, `get_with_retry` / `post_with_retry` with 429/503 backoff and 5s cap | FR-004, FR-024, NFR-002 | M | Critical | T-001 |
| T-004 | Implement `hermes::depsdev` client — `get_version`, `get_package`, `get_dependencies`, `get_project`, `get_advisory`, `extract_project_id`, `DepsDevError` | FR-002, FR-024 | M | Critical | T-002, T-003 |
| T-005 | Implement `hermes::osv` client — `query_vulns`, `query_vulns_batch`, `get_vuln`, `extract_severity`, `extract_cvss_score`, `extract_fix_versions`, `OsvError` | FR-003, FR-024 | M | Critical | T-002, T-003 |
| T-006 | Implement `hermes::licenses` — `COPYLEFT_LICENSES`, `NETWORK_COPYLEFT` sets, `classify_license()` returning permissive/weak-copyleft/copyleft/network-copyleft/unknown | FR-012 | S | High | T-001 |
| T-007 | Implement `hermes::default_version` — `get_default_version()` (prefers `is_default`, falls back to last) | FR-009, FR-010, FR-018 | S | Medium | T-004 |
| T-008 | Implement `hermes::lockfile` — `parse_lockfile()` dispatch + parsers for package-lock.json, yarn.lock, pnpm-lock.yaml, requirements.txt, poetry.lock, Cargo.lock, go.sum, Gemfile.lock, Pipfile.lock, packages.lock.json, gradle.lockfile | FR-006 | L | Critical | T-002 |
| T-009 | Implement `hermes::typosquat::generate_typos()` pure function — omission, transposition, doubling, hyphen/underscore, prefix/suffix, leet-speak | FR-016, NFR-003 | M | High | T-001 |
| T-010 | Implement `hermes::score` — `compute_hound_score()` pure function with 4-component weighting (vulns 40, scorecard 25, freshness 20, licence 15) and letter grade | FR-008, NFR-003 | M | High | T-005, T-006 |
| T-011 | Implement `hound_vulns` tool — OSV query, severity grouping, fix versions, aliases, formatted report | FR-013, FR-021, FR-023, FR-025 | M | Critical | T-005 |
| T-012 | Implement `hound_inspect` tool — parallel deps.dev + OSV fetch, profile report (published date, licence, vuln summary, advisories, GitHub stats, scorecard) | FR-014, FR-021, FR-023, FR-025 | M | High | T-004, T-005 |
| T-013 | Implement `hound_score` tool — wire `compute_hound_score` to live API data, render score + grade + breakdown | FR-008, FR-021, FR-025 | M | High | T-004, T-005, T-010 |
| T-014 | Implement `hound_audit` tool — parse lockfile, dedup, batch OSV (cap 100), severity-grouped report with clean count | FR-007, FR-021, FR-023, FR-025, FR-026 | L | Critical | T-005, T-008 |
| T-015 | Implement `hound_license_check` tool — parse lockfile, resolve licences via deps.dev (cap 50), classify, enforce policy, histogram | FR-012, FR-021, FR-023, FR-025 | L | High | T-004, T-006, T-008 |
| T-016 | Implement `hound_compare` tool — resolve defaults, gather data for both packages, side-by-side table with winner markers and recommendation | FR-009, FR-021, FR-025 | M | High | T-004, T-005, T-007 |
| T-017 | Implement `hound_preinstall` tool — resolve version, vuln check, typosquat heuristics, abandonment, licence, GO/CAUTION/NO-GO verdict | FR-010, FR-021, FR-025 | M | High | T-004, T-005, T-007 |
| T-018 | Implement `hound_upgrade` tool — fetch versions, filter newer, batch OSV (cap 50), return minimum + latest safe version | FR-011, FR-021, FR-025, FR-026 | M | High | T-004, T-005 |
| T-019 | Implement `hound_tree` tool — fetch dep graph, build adjacency, render indented tree with maxDepth truncation | FR-015, FR-021, FR-025 | M | Medium | T-004 |
| T-020 | Implement `hound_typosquat` tool — generate variants, check existence via deps.dev, report suspicious matches | FR-016, FR-021, FR-025 | M | Medium | T-004, T-009 |
| T-021 | Implement `hound_advisories` tool — OSV first, deps.dev fallback, render summary/severity/CWE/affected/references | FR-017, FR-021, FR-025 | M | Medium | T-004, T-005 |
| T-022 | Implement `hound_popular` tool — curated defaults per ecosystem, batch vuln scan, formatted report | FR-018, FR-021, FR-025 | M | Low | T-004, T-005, T-007 |
| T-023 | Register all 12 tools in `create_extended_registry()` and add `pub mod hermes` to `lib.rs` | FR-019 | S | Critical | T-011–T-022 |
| T-024 | Add `hermes` tool-visibility switch to `ToolVisibilityConfig`, `ToolVisibilitySpecified`, `iter_switches()`, `tool_family_names()`, `Default` (true), `Serialize`/`Deserialize`, and `merge()` | FR-020 | M | High | T-023 |
| T-025 | Write unit tests for `hermes::http` retry/backoff logic (429, 503, Retry-After, cap) | FR-004, NFR-003 | M | High | T-003 |
| T-026 | Write unit tests for `hermes::depsdev` — URL construction, ecosystem mapping, project-id extraction, error handling | FR-002, NFR-003 | M | High | T-004 |
| T-027 | Write unit tests for `hermes::osv` — severity extraction, fix-version extraction, batch response mapping | FR-003, NFR-003 | M | High | T-005 |
| T-028 | Write unit tests for `hermes::lockfile` — each parser with sample lockfile content, unsupported format returns None, empty/malformed handling | FR-006, NFR-003 | L | Critical | T-008 |
| T-029 | Write unit tests for `hermes::score::compute_hound_score` — boundary grades, component deductions, zero-vuln case | FR-008, NFR-003 | S | High | T-010 |
| T-030 | Write unit tests for `hermes::typosquat::generate_typos` — variant count, leet substitutions, no original in output | FR-016, NFR-003 | S | Medium | T-009 |
| T-031 | Write unit tests for `hermes::licenses::classify_license` — permissive, weak-copyleft, copyleft, network-copyleft, unknown | FR-012, NFR-003 | S | Medium | T-006 |
| T-032 | Write integration test verifying all 12 `hound_*` tools are registered in `create_extended_registry()` | FR-019, NFR-003 | S | Critical | T-023 |
| T-033 | Write integration test verifying `hermes` visibility switch hides/shows all 12 tools via `effective_hidden_tools()` | FR-020, NFR-003 | S | High | T-024 |
| T-034 | Write integration test verifying each Hermes tool returns `network:fetch` from `permission_category()` | FR-021, NFR-003 | S | High | T-023 |
| T-035 | Run `cargo test -p ragent-tools-extended`, `cargo test -p ragent-config`, `cargo clippy`, and `cargo fmt --check` to confirm no regressions | NFR-001, NFR-002 | S | Critical | T-025–T-034 |

## Task detail

### T-001 — Module structure and shared types

Create `crates/ragent-tools-extended/src/hermes/mod.rs`. Define the shared
types that mirror Hound's `src/types/index.ts`:

```rust
pub enum Ecosystem { Npm, Pypi, Go, Maven, Cargo, Nuget, RubyGems }

pub struct ParsedDependency {
    pub name: String,
    pub version: String,
    pub ecosystem: Ecosystem,
}

pub enum Severity { Critical, High, Moderate, Low, Unknown }

pub struct Vulnerability {
    pub id: String,
    pub summary: String,
    pub severity: Severity,
    pub cvss_score: Option<f64>,
    pub fixed_versions: Vec<String>,
    pub references: Vec<String>,
    pub published_at: String,
    pub aliases: Vec<String>,
}

pub struct HoundScore {
    pub total: u32,        // 0–100
    pub grade: char,       // A–F
    pub breakdown: ScoreBreakdown,
}
```

Add `pub mod hermes;` to `crates/ragent-tools-extended/src/lib.rs`.

### T-002 — Ecosystem module

`hermes/ecosystem.rs` maps `Ecosystem` to deps.dev's lowercase system names
and OSV's convention names (npm, PyPI, Go, Maven, crates.io, NuGet,
RubyGems). Provides `Ecosystem::from_str()` for parsing the tool input and
`validate_for_depsdev()` returning `Result` for the 7 supported ecosystems.

### T-003 — HTTP helper

`hermes/http.rs` builds a shared `reqwest::Client` with the ragent
`User-Agent` and 30s timeout. Exposes `get_with_retry(url)` and
`post_with_retry(url, body)` that retry on 429/503 with delays `[100ms,
400ms]`, honouring `Retry-After` (capped at 5s). All Hermes API calls go
through this helper.

### T-004 / T-005 — API clients

`hermes/depsdev.rs` and `hermes/osv.rs` mirror the TypeScript API clients.
Each function is `async`, uses the shared HTTP helper, percent-encodes path
segments, and returns typed Rust structs deserialised via `serde_json`.

### T-008 — Lockfile parsers

`hermes/lockfile.rs` dispatches on the filename basename. Each parser is a
pure function `fn parse_xxx(content: &str) -> Vec<ParsedDependency>`.
Malformed content returns an empty vec (matching the reference). The
dispatch function returns `Option<Vec<ParsedDependency>>` — `None` for
unrecognised formats.

### T-010 — Score computation

`hermes/score.rs` exposes `compute_hound_score(vulns, scorecard, days_since,
licenses) -> HoundScore` as a pure function. The weighting exactly matches
the reference: vulns 0–40 (deduct 20/10/5/2 by severity), scorecard 0–25,
freshness 0–20, licence 0–15. Letter grade: A≥90, B≥75, C≥60, D≥40, F<40.

### T-023 — Registration

In `create_extended_registry()`, after the existing registrations, add:

```rust
registry.register(Arc::new(hermes::tools::audit::HoundAuditTool));
// ... 11 more
```

### T-024 — Visibility switch

Add a `hermes: bool` field (default `true`) and `hermes: Option<bool>` to
the raw deserialiser. Add `"hermes"` to `iter_switches()`. Add a
`"hermes"` arm to `tool_family_names()` listing all 12 `hound_*` names.
Update `Serialize` (count = 7), `Default`, and `merge()` overlay logic.

## Risks

| Risk | Mitigation |
|------|------------|
| deps.dev / OSV API shape differs from TypeScript types | Pin to v3/v1 endpoints; validate with integration tests against live API (network-gated) |
| Lockfile parser edge cases (yarn v1, pnpm v9, gradle variants) | Port test fixtures from hound-mcp `tests/parsers/` directly |
| Retry logic stalls agent on persistent 503 | 5s cap per wait, max 2 retries, then return error (FR-004) |
| `hermes` switch default `true` changes existing user config | `true` matches codeindex precedent; hidden only if explicitly disabled |
| Name collision: `score.rs` module vs `hound_score` tool | Tool file is `score_tool.rs`; module is `score.rs` |