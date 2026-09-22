---
status: draft
---

# Specification: Hermes — Hound Dependency-Security Tools

## Overview

[Hound MCP](https://github.com/tiluckdave/hound-mcp) is a dependency-security
toolkit that scans packages for vulnerabilities, license violations,
typosquatting, and supply-chain risk across seven ecosystems (npm, PyPI, Go,
Maven, Cargo, NuGet, RubyGems). It is currently distributed as a TypeScript
MCP server that calls two free, unauthenticated public APIs:

- **deps.dev** (`https://api.deps.dev/v3/`) — package metadata, licenses,
  dependency graphs, OpenSSF Scorecard.
- **OSV** (`https://api.osv.dev/v1/`) — vulnerability records, severity,
  fix versions, batch queries.

This specification ("**Hermes**") defines the integration of Hound's 12
dependency-security tools into ragent **as a set of native Rust tools** in the
`ragent-tools-extended` crate — **not** as an external MCP server. The tools
will be re-implemented in Rust using `reqwest` for HTTP, share a common API
client module, and register through ragent's existing `ToolRegistry` and
permission/visibility infrastructure.

The name "Hermes" is the internal spec/feature identifier. The tools keep
their original `hound_*` names so users and agents recognise them.

## Background

### Source codebase

The reference implementation lives at `~/Projects/hound-mcp` and is
structured as:

```
src/
├── api/
│   ├── depsdev.ts   # deps.dev client (GET endpoints)
│   ├── osv.ts       # OSV client (POST /query, /querybatch, GET /vulns)
│   └── retry.ts     # fetchWithRetry — 429/503 backoff, Retry-After cap 5s
├── constants/
│   ├── ecosystems.ts  # 9-ecosystem enum (7 supported by deps.dev)
│   └── licenses.ts    # COPYLEFT_LICENSES, NETWORK_COPYLEFT sets
├── parsers/
│   └── index.ts      # 13 lockfile parsers (package-lock, yarn, pnpm,
│                      #   requirements, poetry, Cargo.lock, go.sum,
│                      #   Gemfile.lock, Pipfile.lock, pubspec.lock,
│                      #   gradle.lockfile, packages.lock.json, composer.lock)
├── tools/            # 12 tool files, each exports register(server)
├── types/index.ts    # Shared TS types
└── utils/
    ├── getDefaultVersion.ts
    └── lockfileFormat.ts
```

### The 12 tools

| Tool | Inputs | Primary API |
|------|--------|-------------|
| `hound_audit` | `lockfile_name`, `lockfile_content` | OSV batch |
| `hound_score` | `name`, `version`, `ecosystem` | deps.dev + OSV |
| `hound_compare` | `package_a`, `package_b`, `ecosystem` | deps.dev + OSV |
| `hound_preinstall` | `name`, `version?`, `ecosystem` | deps.dev + OSV |
| `hound_upgrade` | `name`, `version`, `ecosystem` | deps.dev + OSV batch |
| `hound_license_check` | `lockfile_name`, `lockfile_content`, `policy` | deps.dev |
| `hound_vulns` | `name`, `version`, `ecosystem` | OSV |
| `hound_inspect` | `name`, `version`, `ecosystem` | deps.dev + OSV |
| `hound_tree` | `name`, `version`, `ecosystem`, `maxDepth` | deps.dev |
| `hound_typosquat` | `name`, `ecosystem` | deps.dev |
| `hound_advisories` | `id` | OSV + deps.dev |
| `hound_popular` | `ecosystem`, `packages?` | deps.dev + OSV batch |

### ragent tool architecture

ragent tools implement the `Tool` trait defined in
`crates/ragent-tools-extended/src/lib.rs`:

```rust
#[async_trait::async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters_schema(&self) -> Value;
    fn permission_category(&self) -> str;
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput>;
}
```

Tools are registered in `create_extended_registry()` (same file) and surfaced
to agents via `ToolRegistry::definitions()`. The agent crate's
`create_default_registry()` calls `register_extracted_extended_tools()` which
adapts each extended tool with `ExtractedExtendedToolAdapter`.

Tool visibility is controlled by `ToolVisibilityConfig` in
`crates/ragent-config/src/config.rs` with switches (`office`, `github`,
`gitlab`, `teams`, `agents`, `plan`, `codeindex`). The
`tool_family_names(switch)` function maps a switch name to the list of tool
names it controls. `effective_hidden_tools()` hides any tool whose family
switch is off.

Network tools (`webfetch`, `websearch`, `http_request`) use
`permission_category() -> "network:fetch"` or `"web"` and rely on `reqwest`
with a shared `USER_AGENT` constant.

### Design constraints

1. **No MCP dependency.** Tools are compiled into the ragent binary. No
   `npx`, no Node.js, no stdio transport, no `@modelcontextprotocol/sdk`.
2. **No API keys.** Both upstream APIs are free and unauthenticated. The
   integration must never require credentials, matching Hound's zero-config
   promise.
3. **Single binary.** All lockfile parsers, API clients, and tool logic must
   be pure Rust with no runtime subprocess.
4. **Reuse ragent infra.** Tools use the existing `Tool` trait,
   `ToolRegistry`, `ToolContext`, `reqwest` workspace dependency, and
   permission/visibility system.

## Requirements

### FR-001 — Native Rust re-implementation (ubiquitous)

The system **shall** provide all 12 Hound tools (`hound_audit`,
`hound_score`, `hound_compare`, `hound_preinstall`, `hound_upgrade`,
`hound_license_check`, `hound_vulns`, `hound_inspect`, `hound_tree`,
`hound_typosquat`, `hound_advisories`, `hound_popular`) as native Rust
structs implementing the `ragent_tools_extended::Tool` trait.

> *Ubiquitous requirement — applies to every tool in the Hermes set without
> exception.*

### FR-002 — Shared deps.dev client (ubiquitous)

The system **shall** provide a shared `depsdev` API client module that
wraps `https://api.deps.dev/v3/` with the following operations:
`get_version`, `get_package`, `get_dependencies`, `get_project`,
`get_advisory`, and `extract_project_id`. Ecosystem names shall be mapped to
deps.dev's lowercase system identifiers (npm, pypi, go, maven, cargo, nuget,
rubygems). Package and version path segments shall be percent-encoded.

> *Ubiquitous requirement — every deps.dev-backed tool depends on this
> client.*

### FR-003 — Shared OSV client (ubiquitous)

The system **shall** provide a shared `osv` API client module that wraps
`https://api.osv.dev/v1/` with the following operations: `query_vulns`
(POST `/query`), `query_vulns_batch` (POST `/querybatch`), `get_vuln`
(GET `/vulns/{id}`), `extract_severity`, `extract_cvss_score`, and
`extract_fix_versions`. Ecosystem names shall use OSV convention
(npm, PyPI, Go, Maven, crates.io, NuGet, RubyGems).

> *Ubiquitous requirement — every OSV-backed tool depends on this client.*

### FR-004 — HTTP retry with backoff (state-driven)

When an upstream API returns HTTP 429 or 503, the HTTP helper **shall**
retry the request with exponential backoff (initial 100 ms, then 400 ms)
up to a maximum of 2 retries, honouring the `Retry-After` header but
capping any single wait at 5 seconds to prevent agent stalls.

> *State-driven requirement — triggers when the upstream API is rate-limiting
> or temporarily unavailable.*

### FR-005 — Ecosystem validation (state-driven)

When a tool receives an `ecosystem` parameter, the system **shall** validate
it against the supported set. For deps.dev-backed operations, only the seven
deps.dev-supported ecosystems (npm, pypi, go, maven, cargo, nuget,
rubygems) shall be accepted; unsupported ecosystems shall produce a
user-readable error message rather than a panic.

> *State-driven requirement — triggers when the ecosystem parameter is
> supplied and is outside the supported set.*

### FR-006 — Lockfile parser module (ubiquitous)

The system **shall** provide a `lockfile` parser module that detects the
lockfile format from its filename and returns a list of
`ParsedDependency { name, version, ecosystem }` entries. The module
**shall** support at minimum: `package-lock.json`, `yarn.lock`,
`pnpm-lock.yaml`, `requirements.txt`, `poetry.lock`, `Cargo.lock`,
`go.sum`, `Gemfile.lock`, `Pipfile.lock`, `packages.lock.json`, and
`gradle.lockfile`. Unrecognised filenames shall return `None` so the caller
can emit a helpful "unsupported format" message.

> *Ubiquitous requirement — `hound_audit` and `hound_license_check` depend on
> this module for every invocation.*

### FR-007 — `hound_audit` output format (ubiquitous)

The `hound_audit` tool **shall** parse the supplied lockfile, deduplicate
dependencies by `name@version`, batch-query OSV (capped at 100 packages per
batch), and render a text report grouped by severity (CRITICAL, HIGH,
MODERATE, LOW, UNKNOWN) with per-finding advisory ID, summary, and fix
version, followed by a count of clean packages.

> *Ubiquitous requirement — defines the canonical audit output for every
> lockfile scan.*

### FR-008 — `hound_score` scoring algorithm (ubiquitous)

The `hound_score` tool **shall** compute a 0–100 score from four weighted
components: vulnerabilities (0–40 pts, deducted by severity), OpenSSF
Scorecard (0–25 pts), release freshness (0–20 pts), and licence risk
(0–15 pts). The total **shall** map to a letter grade
(A ≥ 90, B ≥ 75, C ≥ 60, D ≥ 40, F < 40) and the output **shall** include a
per-component breakdown.

> *Ubiquitous requirement — the scoring formula is fixed and deterministic
> for a given set of upstream data.*

### FR-009 — `hound_compare` side-by-side output (event-driven)

When invoked with two package names and an ecosystem, the `hound_compare`
tool **shall** resolve the default version of each package, gather
vulnerability counts, scorecard, stars, recency, and licence data, and
render a side-by-side comparison table with a per-metric winner marker and
a final recommendation.

> *Event-driven requirement — triggers on each `hound_compare` invocation.*

### FR-010 — `hound_preinstall` verdict (event-driven)

When invoked with a package name (and optional version), the
`hound_preinstall` tool **shall** return a GO / CAUTION / NO-GO verdict
based on: known CRITICAL/HIGH vulnerabilities, typosquatting heuristics
(look-alike characters, very short names, unusual separators), project
abandonment (no release in 2+ years), and licence concerns (unknown or
network-copyleft). The output **shall** list every issue with its level
(block / warn / info).

> *Event-driven requirement — triggers on each `hound_preinstall`
> invocation.*

### FR-011 — `hound_upgrade` safe-version finder (event-driven)

When invoked with a package name and current vulnerable version, the
`hound_upgrade` tool **shall** fetch all published versions, filter to
those newer than the current version, batch-query OSV (capped at 50
candidates), and return the minimum safe version (no known vulns) and the
latest safe version, or a "no safe upgrade found" message with remediation
suggestions.

> *Event-driven requirement — triggers on each `hound_upgrade` invocation.*

### FR-012 — `hound_license_check` policy enforcement (state-driven)

When invoked with a lockfile and a policy (`permissive`, `copyleft`, or
`none`), the `hound_license_check` tool **shall** resolve the licence for
each dependency via deps.dev, classify each licence (permissive,
weak-copyleft, copyleft, network-copyleft, unknown), and flag packages that
violate the chosen policy. The output **shall** include a licence
breakdown histogram and the source (`deps.dev`).

> *State-driven requirement — the policy parameter selects the enforcement
> ruleset.*

### FR-013 — `hound_vulns` severity grouping (ubiquitous)

The `hound_vulns` tool **shall** query OSV for a single package version and
render vulnerabilities grouped by severity (CRITICAL → HIGH ��� MODERATE →
LOW → UNKNOWN) with advisory ID, summary, fix version(s), aliases (CVE,
GHSA), and publication date.

> *Ubiquitous requirement — defines the canonical vulnerability listing
> format.*

### FR-014 — `hound_inspect` comprehensive profile (event-driven)

When invoked with a package name and version, the `hound_inspect` tool
**shall** fetch version metadata, vulnerabilities, and project info
(scorecard, stars, forks, open issues) in parallel, and render a single
profile covering: published date, licence, vulnerability summary, advisory
IDs, GitHub stats, and OpenSSF Scorecard with weak-check callouts.

> *Event-driven requirement — triggers on each `hound_inspect` invocation.*

### FR-015 — `hound_tree` dependency tree rendering (event-driven)

When invoked with a package name, version, ecosystem, and `maxDepth`
(1–10, default 3), the `hound_tree` tool **shall** fetch the resolved
dependency graph from deps.dev, build an adjacency list from the edge
list, and render an indented tree from the SELF node, truncating at
`maxDepth` with a notice when deeper transitive deps exist.

> *Event-driven requirement — triggers on each `hound_tree` invocation.*

### FR-016 — `hound_typosquat` variant generation (event-driven)

When invoked with a package name and ecosystem, the `hound_typosquat` tool
**shall** generate typosquatting variants using: character omission,
adjacent transposition, character doubling, hyphen/underscore confusion,
common prefix/suffix additions (node-, -js, -node), and leet-speak
substitution (l→1/i, o→0, e→3, a→4, s→5, i→1/l, t→7). It **shall** then
check which variants exist in the registry via deps.dev and report
suspicious matches.

> *Event-driven requirement — triggers on each `hound_typosquat`
> invocation.*

### FR-017 — `hound_advisories` multi-source lookup (event-driven)

When invoked with an advisory ID (GHSA, CVE, or OSV), the
`hound_advisories` tool **shall** query OSV first (richer data) and fall
back to deps.dev, rendering: summary, severity, CWE IDs, affected packages
with introduced/fixed ranges, truncated details, and up to 5 reference
URLs.

> *Event-driven requirement — triggers on each `hound_advisories`
> invocation.*

### FR-018 — `hound_popular` curated defaults (optional)

The `hound_popular` tool **may** use a curated list of popular packages per
ecosystem when the caller does not supply an explicit `packages` array.
The curated lists **shall** cover at least npm, pypi, go, maven, cargo,
nuget, and rubygems.

> *Optional requirement — the curated defaults are a convenience; callers
> can always supply their own package list.*

### FR-019 — Tool registration (ubiquitous)

All 12 Hermes tools **shall** be registered in
`create_extended_registry()` within `crates/ragent-tools-extended/src/lib.rs`
so they appear in every ragent session alongside the existing extended
tools.

> *Ubiquitous requirement — registration is mandatory for the tools to be
> usable.*

### FR-020 — Tool visibility switch (state-driven)

When the `hermes` tool-visibility switch is disabled, the system **shall**
hide all 12 `hound_*` tools from the agent's tool definitions. When the
switch is enabled (or absent), the tools **shall** be visible. The switch
**shall** be added to `ToolVisibilityConfig`, `ToolVisibilitySpecified`,
`iter_switches()`, `tool_family_names()`, the `Default` implementation
(default: `true`), the custom `Serialize`/`Deserialize` impls, and the
`merge()` overlay logic in `crates/ragent-config/src/config.rs`.

> *State-driven requirement — the visibility state of the `hermes` switch
> controls tool availability.*

### FR-021 — Permission category (ubiquitous)

Every Hermes tool **shall** return `"network:fetch"` from
`permission_category()` so that it participates in ragent's existing
network permission rules (allow / deny / ask / YOLO), consistent with
`http_request` and `webfetch`.

> *Ubiquitous requirement — all Hermes tools make outbound network calls and
> must be subject to network permission gating.*

### FR-022 — No API keys required (unwanted)

The Hermes tools **shall not** require any API key, token, or account
configuration. The system **shall not** read `HOUND_*` or
`DEPSDEV_*`/`OSV_*` environment variables for credentials. A missing
network connection or upstream API outage **shall** produce a readable
error, not a crash.

> *Unwanted requirement — credential-gating is explicitly forbidden; this
> preserves Hound's zero-config promise.*

### FR-023 — Error handling (unwanted)

The Hermes tools **shall not** panic on upstream API errors, malformed
lockfile content, missing package versions, or empty result sets. Each
tool **shall** return a `Result<ToolOutput>` with a user-readable error
message or a graceful "no data found" text response, matching the
reference implementation's catch-and-return-text pattern.

> *Unwanted requirement — panics and unhandled exceptions are explicitly
> forbidden.*

### FR-024 — Shared HTTP client and user agent (ubiquitous)

The Hermes API clients **shall** use a shared `reqwest::Client` with a
`User-Agent` header of the form `ragent/{version}` and a default request
timeout of 30 seconds, consistent with the existing `webfetch` and
`websearch` tools.

> *Ubiquitous requirement — applies to every HTTP call made by the Hermes
> tools.*

### FR-025 — Output as text (ubiquitous)

Every Hermes tool **shall** return its result as a `ToolOutput` whose
`content` is a human-readable text string (the formatted report). Tools
**shall not** return raw JSON to the agent, matching the reference
implementation's "formatted text strings — never return raw JSON"
convention and ragent's existing tool output style.

> *Ubiquitous requirement — defines the output contract for all 12 tools.*

### FR-026 — Batch size caps (unwanted)

The `hound_audit` tool **shall not** query more than 100 dependencies per
batch, and the `hound_upgrade` tool **shall not** query more than 50
candidate versions per batch. When the input exceeds the cap, the tool
**shall** truncate and note the truncation in the output.

> *Unwanted requirement — unbounded batch requests that could overwhelm the
> upstream API or stall the agent are explicitly forbidden.*

## Non-functional requirements

### NFR-001 — Performance

Each Hermes tool **shall** complete within 30 seconds for a typical
single-package query and within 60 seconds for a full lockfile audit of
100 dependencies. Batch OSV queries **shall** use the `/querybatch`
endpoint to minimise round-trips.

### NFR-002 — No new external dependencies beyond workspace

The implementation **shall** use only crates already present in the
workspace (`reqwest`, `serde`, `serde_json`, `anyhow`, `regex`,
`percent-encoding`, `chrono`, `tokio`, `async-trait`, `tracing`). No new
workspace-level dependencies shall be added.

### NFR-003 — Testability

Each API client and lockfile parser **shall** be a pure function or struct
with injectable behaviour, enabling unit tests without live network calls.
Tool logic (scoring, variant generation, report formatting) **shall** be
extracted into testable pure functions.

### NFR-004 — Documentation

Every public function and module **shall** carry a `///` doc comment
describing its purpose, per the project's documentation standards. The
`hermes` module **shall** have a `//!` crate/module-level doc comment.

## Scope

### In scope

- Re-implementing all 12 Hound tools in Rust inside
  `crates/ragent-tools-extended`.
- Shared `depsdev` and `osv` API client modules.
- Shared HTTP helper with retry/backoff.
- Lockfile parser module (11+ formats).
- Licence classification constants.
- Ecosystem mapping and validation.
- Tool registration in `create_extended_registry()`.
- `hermes` tool-visibility switch in `ragent-config`.
- `network:fetch` permission category for all tools.
- Unit tests for API clients, parsers, scoring, and variant generation.
- Integration tests for tool registration and visibility.

### Out of scope

- Running hound-mcp as an MCP server (explicitly excluded by the feature
  request).
- Porting the 3 Hound MCP prompts (`security_audit`, `package_evaluation`,
  `pre_release_check`) — those are agent-level prompt templates, not tools.
- Pub and Packagist ecosystem support (deps.dev does not support them;
  `hound_license_check` already omits `pubspec.lock` and `composer.lock`).
- Caching of upstream API responses (may be a future enhancement).
- A TUI slash command for Hound (tools are agent-invoked; a `/hound`
  command is not part of this spec).