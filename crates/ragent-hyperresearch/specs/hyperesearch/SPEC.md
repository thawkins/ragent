---
status: draft
audit:
  - { time: 1786572767, from: "none", to: "draft", actor: "system" }
---
# hyperresearch — Deep-Research Engine Crate

**Spec ID:** hyperesearch

## Overview

This specification defines a Rust crate, `ragent-hyperresearch`, that implements a
deep-research engine for coding agents. It is a port of the Python project
`jordan-gibbs/hyperresearch` and turns an external agent (Claude Code, Codex, or
ragent itself) into a deep-research agent: it decomposes a question, searches and
fetches web and scholarly sources, deduplicates and ranks them, extracts claims,
builds a contradiction and citation graph across notes, and synthesises
long-form, fully cited output — all persisted in a wiki-style vault that grows
across sessions.

The crate provides four surfaces over one shared domain layer:

1. a **library API** (`ragent_hyperresearch::core`, `::models`, …),
2. a **CLI** binary named `hyperresearch`,
3. a **stdio MCP server** (`hyperresearch mcp`) exposing vault tools, and
4. a **small HTTP server** (`hyperresearch serve`) rendering vault notes and
   reports to HTML.

### Goals

- A persistent, searchable research vault of Markdown notes with YAML
  frontmatter, stable note ids, and an assets directory.
- A provider-abstracted fetch layer: a builtin safe-HTTP lane plus optional
  third-party providers, with a `fetch_many` fan-out that degrades rather than
  aborts.
- A scholarly provider registry (OpenAlex, Crossref, ClinicalTrials, DOAB,
  EDGAR, FRED, REPEC, core) with cross-provider deduplication.
- Claim extraction, citation checking, and a link/contradiction graph over the
  vault.
- An embedded, hashed skill pipeline (18 numbered stages) that is asserted
  stable by a golden test.
- Strict treatment of fetched content as untrusted everywhere it reaches a
  prompt or a rendered page.

### Non-Goals

- No LLM client or model inference inside this crate. Synthesis steps are
  expressed as embedded skill/prompt text consumed by the host agent.
- No GUI or TUI. The HTTP server renders HTML only.
- No `pyproject.toml` or Python packaging of any kind.

### Crate Layout

```
crates/ragent-hyperresearch/
├── Cargo.toml
├── src/
│   ├── lib.rs            # #![deny(unsafe_code)], pub mod re-exports
│   ├── main.rs           # thin: parse args, dispatch to cli::run
│   ├── cli/              # one module per subcommand group
│   ├── core/             # domain logic, no stdout I/O
│   ├── models/           # serde types only
│   ├── mcp/              # stdio JSON-RPC MCP server
│   ├── serve/            # HTTP + HTML renderer
│   ├── web/              # fetch providers
│   ├── scholar/          # scholarly providers + registry + dedup
│   ├── search/           # FTS index + evidence filters
│   ├── graph/            # link/contradiction graph
│   ├── export/           # report export formats
│   ├── indexgen/         # vault INDEX.md generation
│   └── skills/           # embedded skill markdown (include_str!)
├── docs/                 # source-search.md, roadmap-2.0/phase-*.md
├── skills/deep-research/ # human-facing SKILL.md + agents/openai.yaml
├── .claude-plugin/, .codex-plugin/, .agents/plugins/
└── tests/                # one consolidated `all` target
```

## Requirements

### FR-001: Crate Skeleton and Workspace Membership (Ubiquitous)

> The crate **shall** be a workspace member named `ragent-hyperresearch` with
> edition 2024, a library target at `src/lib.rs`, and a `[[bin]]` named
> `hyperresearch` at `src/main.rs`.

### FR-002: Crate-Level Unsafe Denial (Ubiquitous)

> The crate root **shall** declare `#![deny(unsafe_code)]`, and the binary entry
> point **shall** contain no business logic beyond argument parsing and dispatch
> to `cli::run`.

### FR-003: Module-Per-Concern Layout (Ubiquitous)

> The crate **shall** organise code into the module directories `cli`, `core`,
> `models`, `mcp`, `serve`, `web`, `scholar`, `search`, `graph`, `export`,
> `indexgen`, and `skills`, each declared from `lib.rs` and each carrying a
> `//!` module-level doc comment.

### FR-004: Layered Configuration (State-Driven)

> **While** a command is executing, configuration **shall** resolve from three
> layers in increasing precedence — the vault config file, environment
> variables, and command-line flags — and the resolved configuration **shall**
> expose at least: vault root, provider selection, provider API keys, request
> timeout, maximum fetch body size, and the active research profile.

### FR-005: Note Model and Frontmatter Round-Trip (State-Driven)

> **When** a note is loaded and written back without modification, the crate
> **shall** reproduce its YAML frontmatter byte-identically, including key
> order, quoting style, and trailing newline.

### FR-006: Note Id Collision Handling (Unwanted Behaviour)

> **If** a note create or rename operation would produce an id that already
> exists in the vault, **then** the crate **shall** return an error naming the
> conflicting id and the existing note path, **shall not** silently overwrite or
> auto-suffix the id, and the CLI **shall** exit with a non-zero status.

### FR-007: Vault Notes and Assets Directory (Ubiquitous)

> The vault **shall** store each note as a Markdown file with YAML frontmatter
> under the vault root, and **shall** store binary attachments referenced by
> notes beneath a per-note assets directory.

### FR-008: Fetch Pipeline Orchestration (Event-Driven)

> **When** a fetch request for a URL is received, the crate **shall** orchestrate
> the sequence fetch → content extraction → PDF lane (when the payload is a PDF)
> → junk detection, and **shall** return a structured result distinguishing
> success, empty content, junk content, and transport failure.

### FR-009: Safe HTTP Contract (Ubiquitous)

> `web::safe_http` **shall** enforce: TLS certificate validation is never
> disabled; the final URL after all redirects is re-checked against the fetch
> policy before its body is accepted; the response body is truncated at the
> configured maximum size; the request is subject to the configured timeout; and
> a `robots.txt` refusal is honoured when the policy enables it.

### FR-010: Web Provider Abstraction (State-Driven)

> **When** a web provider is configured with credentials or explicitly enabled,
> the crate **shall** instantiate it as a `WebProvider` implementation from the
> set `builtin`, `crawl4ai`, `exa`, `parallel`, `tavily`, and `serply`, and
> **shall** expose the selected provider list through configuration status
> output.

### FR-011: Provider Fallback in Batch Fetch (Unwanted Behaviour)

> **If** a provider fails, times out, or returns junk during `fetch_many`, **then**
> the crate **shall** record the failure for that URL, attempt the next provider
> in the configured chain, and **shall not** abort the remaining URLs in the
> batch.

### FR-012: Scholarly Provider Registry (Ubiquitous)

> The crate **shall** provide a `ScholarlyProvider` trait and a registry
> containing implementations for OpenAlex, Crossref, ClinicalTrials, DOAB,
> EDGAR, FRED, REPEC, and a core provider, each normalising records to one
> shared scholarly-record model.

### FR-013: Cross-Provider Deduplication (Event-Driven)

> **When** records from two or more scholarly providers are merged, the crate
> **shall** deduplicate them by DOI first, then by normalised title plus year,
> and **shall** retain the merged record's provenance listing every contributing
> provider.

### FR-014: Open-Access Resolution with Recovery (Unwanted Behaviour)

> **If** open-access resolution for a scholarly record fails, **then** the crate
> **shall** attempt the configured recovery path (alternate OA source, then
> publisher landing page) and **shall** report the resolution outcome per record
> without failing the whole batch.

### FR-015: Untrusted Content Fencing (Ubiquitous)

> Every string that originated from a network fetch **shall** pass through
> `core::untrusted` before being placed in a prompt or rendered page, and the
> fence **shall** delimit the content with an explicit untrusted marker carrying
> its source URL.

### FR-016: HTML Rendering Escaping (Ubiquitous)

> The serve renderer **shall** HTML-escape every interpolated note field, title,
> tag, and fetched-content string, and **shall not** emit raw fetched markup.

### FR-017: Search Index and Evidence Filters (Event-Driven)

> **When** a `search` command is issued, the crate **shall** query a
> full-text index over note titles, bodies, and frontmatter tags, and **shall**
> apply any supplied evidence filters (minimum source quality, study design,
> publication year range) to the result set.

### FR-018: Claim Extraction and Verification (State-Driven)

> **While** claims are stored for a note, the crate **shall** record for each
> claim its text, the supporting note and source reference, a verification
> state, and **shall** expose a verification command that recomputes the state
> against the current vault.

### FR-019: Citation Checking (Event-Driven)

> **When** `citecheck` runs against a report and the vault, the crate **shall**
> report every citation that resolves to no vault note, every vault note cited
> by nothing, and every cited note whose source URL is unreachable from the
> stored record.

### FR-020: Link and Contradiction Graph (State-Driven)

> **While** the graph is built, the crate **shall** derive note-to-note edges of
> kinds at least `cites`, `supports`, `contradicts`, and `same-source`, and
> **shall** expose ranking of notes by graph centrality.

### FR-021: Run History and Migrations (Event-Driven)

> **When** a research run completes, the crate **shall** persist a run record
> (id, query, profile, providers used, counts of sources fetched and claims
> extracted, terminal status), and **shall** apply pending schema migrations through
> `core::migrations` before any state is read or written.

### FR-022: Vault Repair (Optional)

> **Where** the vault index has drifted from the on-disk notes, `repair` **shall**
> be available to rebuild the index and **shall** report each repaired item.

### FR-023: CLI Command Surface (Ubiquitous)

> The `hyperresearch` binary **shall** expose the commands `note`, `fetch`,
> `fetch-batch`, `search`, `research`, `serve`, `mcp`, `graph`, `link`, `index`,
> `export`, `import`, `archive`, `lint`, `repair`, `config`, `profile`, `scholar`,
> `claims`, `embed`, `escalation`, `levers`, `template`, `topic`, `tag`,
> `vault-tag`, `watch`, `install`, `setup`, `batch`, `citecheck`, `dedup`, `git`,
> and `assets`, each returning `Result<_, CliError>`.

### FR-024: Uniform CLI Output Formatting (Ubiquitous)

> All command results **shall** be emitted through one shared output module
> supporting plain and JSON formats, and every failure **shall** produce a
> non-zero exit status without panicking on user input.

### FR-025: MCP Tool Server (Event-Driven)

> **When** the MCP server receives a `tools/call` JSON-RPC request, the crate
> **shall** validate the tool arguments against the tool's schema and **shall**
> expose at least `fetch_url`, `update_note`, `search`, `read_note`, and
> `create_note`; malformed arguments **shall** yield a structured JSON-RPC error,
> never a panic or a process abort.

### FR-026: HTTP Serving Surface (Event-Driven)

> **When** the serve command runs, the crate **shall** listen on the configured
> bind address and serve vault note pages, an index page, and a report page,
> performing no network fetch from the render path.

### FR-027: Embedded Skill Pipeline (Ubiquitous)

> The crate **shall** embed the 18 numbered pipeline stages (decompose,
> width-sweep, contradiction-graph, loci-analysis, depth-investigation,
> cross-locus-reconcile, source-tensions, corpus-critic, evidence-digest,
> triple-draft, synthesize, critics, gap-fetch, patcher, cite-check, polish,
> readability-audit, chapter-partition) plus the umbrella skill, using
> `include_str!`, and **shall** expose them by name through the `skills` module.

### FR-028: Skill Golden Stability (Unwanted Behaviour)

> **If** the embedded skill or prompt text changes, **then** the golden-fixture
> check **shall** fail and name the changed skill, so prompt drift is a visible
> diff rather than a silent change.

### FR-029: Agent Plugin Manifests (Ubiquitous)

> The crate tree **shall** include `.claude-plugin/plugin.json`,
> `.codex-plugin/plugin.json`, and `.agents/plugins/marketplace.json`, each
> declaring the `deep-research` skill, with `skills/deep-research/SKILL.md` and
> `skills/deep-research/agents/openai.yaml` as the skill's entry points.

### FR-030: Export Formats (Optional)

> **Where** an export format is selected, the crate **shall** support at least
> Markdown report export and a structured (JSON) export carrying notes,
> citations, and run metadata.

### FR-031: Research Effort Controls (Optional)

> **Where** the user supplies an effort level or depth lever, the crate **shall**
> apply the corresponding search-width and escalation settings to the run and
> **shall** record the applied values in the run record.

### FR-032: Vault Index Generation (Event-Driven)

> **When** the vault is indexed, the crate **shall** regenerate `INDEX.md`
> listing every note grouped by topic with its id, title, and last-updated
> timestamp.

### FR-033: Documentation Artifacts (Ubiquitous)

> The crate **shall** include `CHANGELOG.md` in Keep a Changelog form,
> `CONTRIBUTING.md`, `LICENSE`, `docs/source-search.md`, and
> `docs/roadmap-2.0/` phase documents covering config profiles, source ranking,
> dissertation-scale output, a browser fetch lane, and verification.

### NFR-001: No Panics on User Input (Ubiquitous)

> No user-supplied path, id, URL, or command argument **shall** reach
> `unwrap`, `expect`, or an indexing operation that can panic; all such paths
> **shall** return a typed error.

### NFR-002: Offline Determinism of Tests (Ubiquitous)

> The test suite **shall** pass with no network access: every provider test
> **shall** use recorded fixtures, and any test that mutates process-global state
> **shall** take the crate-wide `global_env_lock()`.

### NFR-003: Formatting and Lint Compliance (Ubiquitous)

> The crate **shall** pass `cargo fmt --check` and `cargo clippy` with no
> warnings, with a maximum line width of 100 columns.

### NFR-004: Test Isolation from the Repository (Ubiquitous)

> Tests **shall** write only beneath `target/temp/`, and a test run **shall**
> leave the checked-in tree byte-identical.

### NFR-005: Structured Logging (Ubiquitous)

> Library and core code **shall** log through `tracing`; `println!` and
> `eprintln!` **shall** appear only in the CLI presentation module.

### NFR-006: Error Typing (Ubiquitous)

> Domain errors **shall** use `thiserror`-derived types; `anyhow` **shall** be
> confined to the binary edge.
