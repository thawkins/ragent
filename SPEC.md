<div style="page-break-after: always; text-align: center; padding-top: 15em;">

<h1 style="font-size: 3em; margin-bottom: 0.2em;">ragent</h1>
<h2 style="font-size: 1.5em; font-weight: normal; color: #555; margin-top: 0;">Technical Specification</h2>        <p style="margin-top: 4em; font-size: 1.1em;">        <strong>Version:</strong> 1.0.158</p>
        <p style="font-size: 1.1em;">
          <strong>Date:</strong> 2026-10-30
      </p>
  <p style="font-size: 1.1em;">
    <strong>Author:</strong> Tim Hawkins &lt;tim.thawkins@gmail.com&gt;
  </p>
<p style="margin-top: 6em; font-size: 0.9em; color: #777;">
    <a href="https://github.com/thawkins/ragent">github.com/thawkins/ragent</a>
  </p>

</div>
---

## Executive Summary

Ragent is an open-source AI coding agent for the terminal, written entirely in
Rust and distributed as a single statically-linked binary with zero external
runtime dependencies. It orchestrates multiple LLM providers — Anthropic,
OpenAI, GitHub Copilot, Google Gemini, Hugging Face, Ollama (local and cloud), xAI
Grok, Generic OpenAI-compatible endpoints, Azure AI Foundry, Azure Resource (File),
Amazon Bedrock, and a Model Router provider — behind a unified streaming interface,
giving developers a powerful,
provider-agnostic assistant that runs wherever a terminal does.

### What It Does

Ragent bridges the gap between conversational AI and hands-on software
engineering. An agent can read and write files, execute shell commands, search
codebases, manage Git and GitHub workflows, query language servers, read PDF
documents, and coordinate with other agents — all through a
large built-in tool library organised across multiple categories. Every tool
invocation passes through a multi-layered security and permission system that
gives the user full control over what the agent can and cannot do.

### How It Works

At its core, ragent follows a **session → agent → tool** loop. A session
processor manages the conversation with the LLM provider, the agent system
defines personality and capabilities via profiles, and the tool registry
dispatches execution requests. An asynchronous event bus (built on tokio)
connects all components, enabling real-time streaming of tokens, tool results,
and status updates to both the TUI and the HTTP API.

```mermaid
graph LR
    User[User] --> TUI[TUI / HTTP API]
    TUI --> SP[Session Processor]
    SP --> LLM[LLM Provider]
    SP --> AP[Agent Profile]
    SP --> TR[Tool Registry]
          TR --> Tools[File ops, Bash, GitHub,<br/>code index, memory, teams,<br/>PDF, web, ...]

```

**Figure 1:** Core Execution Loop — High-level session/agent/tool flow

### Key Capabilities

| Capability | Summary |
|-----------|---------|
| **Multi-provider LLM** | 12 providers with automatic model discovery, health monitoring, streaming, vision, and reasoning levels |
| **Terminal UI** | Full-screen ratatui interface with streaming markdown, syntax highlighting, slash commands, and image support |
| **HTTP Server** | REST + SSE API (Axum) for headless operation and external integrations |
| **Tool System** | Broad tool coverage across file ops, shell, search, GitHub, GitLab, code index, memory, teams, sub-agents, PDF, web, and MCP |
| **Code Intelligence** | Tree-sitter parsing (15+ languages), Tantivy FTS, symbol/reference search, and code index queries |
| **Persistent Memory** | Three-tier system — file blocks, structured SQLite store, and optional embedding-based semantic search — with automatic extraction, decay, compaction, and a knowledge graph |
| **Teams & Swarms** | Multi-agent coordination with named teammates, shared task lists, mailbox messaging, and swarm decomposition for parallel work |
| **Security** | Permission rules (allow/deny/ask), 7-layer bash safety, file-path guards, secret redaction, resource limits, and YOLO mode for trusted environments |
| **Skills** | Loadable skill packs (bundled or custom YAML) that inject tools, prompts, and file context into agent sessions |
| **Custom Agents** | OASF-based agent profiles with configurable models, tools, permissions, and personality |
| **Autopilot** | Autonomous operation mode with configurable iteration limits and permission auto-approval |

### Who It's For

Ragent is designed for software developers and teams who want an AI assistant
that lives in their terminal, respects their security boundaries, and learns
from their workflow over time. It is equally suited to interactive pair-programming
sessions and headless CI/CD integration via its HTTP API.

### Technology

| Aspect | Detail |
|--------|--------|
| **Language** | Rust (edition 2024) |
| **Async runtime** | tokio |
| **TUI framework** | ratatui + crossterm |
| **HTTP framework** | Axum |
| **Database** | SQLite (rusqlite, compiled-in) |
| **Full-text search** | Tantivy |
| **Code parsing** | tree-sitter (15+ grammars compiled-in) |
| **Embeddings** | ONNX Runtime (optional, `all-MiniLM-L6-v2`) |
| **Binary size** | Single static binary, ~50 MB release |
| **Platforms** | Linux, macOS, Windows (cross-compiled) |

### Project Status

Ragent is in **beta** (v1.0.158). The core architecture, tool system,
TUI, HTTP server, memory system, spec management, skills system, research system,
multi-agent coordination, security layer, telemetry, code index semantic graph,
plugin system, connector system (§19C), execution backends (§19D), the Agent
Client Protocol client and server (§19E), the automation service (§19F), and
release packaging are functional and under active development. The specification
below documents the current state of all subsystems.

**Current Release Highlights (spec `openhands`; v1.0.130 → v1.0.131):**

- **OpenHands parity delta shipped (spec `openhands`, FR-001..FR-038 / T-001..T-019)** —
  the v1.0.131 release tags the six structural gaps between ragent and
  OpenHands: pluggable execution backends (`local`/`docker`/`podman`/`remote`,
  §19D) with a health-visible backend registry and a TUI `/backend` switcher; an
  ACP client and a feature-gated ACP server (§19E); an OpenAI-compatible inbound
  surface plus a generated OpenAPI 3.1 contract and HTTP MCP transport (§7.2.1,
  §7.4.1); the automation service with webhook ingress, scheduler, run history,
  and backend confinement (§19F); AgentSkills (`.agents/skills/`) discovery
  (§12.2); and an LLM security analyzer (§19D.8). The 7-layer bash validation,
  path containment, and permission system are unchanged and gate every new
  surface (T-018). The full requirement-by-requirement mapping is in
  `specs/openhands/`.
- **Container execution backend (spec `openhands` FR-001, FR-002, FR-009,
  FR-020, FR-026, FR-031, FR-035, T-002)** — a pluggable execution backend now
  selects *where* a session's tools run. `ContainerBackend` provisions one
  long-lived `podman`/`docker` sandbox per session and dispatches each tool call
  into it with `<runtime> exec`; the workspace is a container-named volume, never
  a host bind mount, so a sandboxed tool cannot read or write the host working
  directory (§19D). `Bash` is gated by the host 7-layer validator, a file/search
  subset is translated to confined sandbox scripts, and every other tool is
  refused with no host fallback (FR-031). Runtime detection probes `PATH` with
  `podman` as the default (FR-026); a provisioning failure fails the turn rather
  than running the tool on the host.
- **Plugin-store category navigator (spec `catnav`) and `browser` removal
  (v1.0.130, commit `e617bcef`)** — the `/plugins codex` and `/plugins claude` browse
  panels gain a left-hand category navigator (§6.5): `ALL` plus the distinct
  categories the fetched index declares, driven by keyboard (`Tab`
  focus, `Up`/`Down` move-and-apply, `Enter` apply, `c` clear) and mouse
  (click/scroll); a `--category <name>` launch argument pre-selects a row; the
  `StoreEntry` model gains an optional `category` field. The `browser` tool and
  the whole `ragent-tools-extended::browser` CDP module are removed, along with
  `BrowserConfig`, so the registered tool count falls 152 -> 151 across 22
  categories. `/mcp discover` scans the new ragent-native
  `~/.config/ragent/servers/` directory first, and a startup failure is echoed to
  stdout when the TUI stderr spool is active so it is never silent. Store
  marketplace failures render as clean
  `<Store> plugin marketplace is unavailable (<plain cause>)` rows
  (spec `pluginstores` FR-013).
- **Office / LibreOffice document tools removed (v1.0.126)** — the six
  `office_*` / `libre_*` document tools and their whole module set
  (`office_common`, `office_write`, `office_info`, `libreoffice_common`,
  `libreoffice_write`, `libreoffice_info`) are gone; only the two PDF tools
  (`pdf_read`, `pdf_write`) remain, sharing the new `pdf_common` helper module.
  The unused `ragent-tools-extended::docio` helper and the OOXML/ODF
  `DocumentFormat::{Docx,Xlsx,Pptx,Odt,Ods,Odp}` variants are removed (those
  extensions now return an actionable error from `detect_document_format`). The
  dependencies `docx-rust`, `calamine`, `ooxmlsdk`, `zip`, and
  `spreadsheet-ods` are dropped, the registered tool count falls 158 -> 152
  (23 categories), and the sample `assets/officedocs/testword1.docx` is
  deleted.
- **`/simplify all` code-quality pass (v1.0.126)** — a review-and-fix sweep over
  the changed set: `pdf_write` now confines an `image` element's `image_path`
  through `ctx.check_path_within_workspace` (SEC-tools-extended-002) and reports
  `page_count`; `pdf_read` parses each PDF once and falls back to the
  whole-document extraction only when the per-page pass yields nothing;
  `os_info::render_json` runs once; the TUI `/memory clear` handler scopes the
  delete to `self.cwd_path`; and `connectors::store::descriptor_by_id` resolves a
  reference in a single allocating pass (exact id > case-insensitive id >
  case-insensitive name). Dead `theme::think_summary` /
  `memory_clear_confirm_area` items and several stale doc references are removed.
- **Plugin and connector marketplace browsers (spec `pluginstores`
    FR-047..FR-050, `connectors` FR-042..FR-046; v1.0.132)** - the
    `/plugins codex|claude` and `/connectors claude` browse panels
    each merge their store's base endpoint with every additional marketplace
  registered through the new `/plugins marketplace` / `/connectors marketplace`
  sub-subcommand families (`show`, `<store> list|add <source> [--force]|remove
  <name>`), recorded in the user-global `plugins.stores.marketplaces` /
  `connectors.stores.marketplaces` arrays by an atomic config splice. Each result
  row now shows the plugin/connector id, its version (plugin store), the
  **marketplace it came from**, and its description; the connector panel pins a
  dark-on-light column-heading row above its scrolling list, and both panels
  resolve each entry's blank description (and, for the plugin store, its version)
  from the entry's own repository manifest off the event loop via a bounded thread
  pool with an injectable `ManifestSource` seam (`ragent_plugins::store_enrich`,
  `ragent_connectors::catalogue_enrich`). Every connector and plugin-store list is
  now sorted in ascending alphabetical order of id, case-insensitively with a
  byte-wise tiebreak (`compare_connector_ids` / `compare_store_ids`), applied
  before any query or category filter so the survivors keep their order. The
  plugin-store navigator no longer derives rows from entry `tags`/`keywords`
  (`catnav` FR-021 withdrawn): it lists only the `category` values the index
  declares, so a plugin's long `keywords` list can no longer flood it.


- **`/memory clear` (§ Memory)** — `/memory clear` opens a
  `Clear this project's memory?` `Yes`/`No` dialog and removes nothing until the
  user confirms. `No` is the default selection, so a stray `Enter` cannot clear
  memory; `Left`/`Right` (or `Tab`) move the selection, `Enter` selects, and
  `Esc` cancels. On `Yes`, `Storage::clear_memories_for_project(&Path)` deletes
  every structured memory whose `project` column equals the current directory's
  full path or its basename (the same scope `count_memories_for_project` /
  `list_memories_for_project` use), removing the FTS rows and base rows in one
  transaction; other projects' memories are never touched. The dialog reports
  `Cleared N memory entries for this project.` in the transcript. The
  `command_catalog` and `SLASH_COMMANDS` `/memory` entries list exactly the
  implemented subcommands (`show`, `clear`, `help`).
- **Shared surface helpers (spec `plugins`/`connectors`, plan `code-audit` M5
  T-505)** — a new `ragent-surface` crate gives the `/plugins` and `/connectors`
  command families one implementation of the surface glue that had drifted into
  parallel copies: the `From: <trigger> <subcommand>` attribution line and the
  first-token subcommand tokeniser (`ragent_surface::help`), the two-root
  store-directory resolution and the symlink-containment guard
  (`ragent_surface::store`), and the harness step model with its schema-sample
  generator and result truncator (`ragent_surface::harness`). `ragent-plugins`
  and `ragent-connectors` delegate to it, so the two `/test` and `help` surfaces
  cannot diverge.
- **Connector system (spec `connectors`, §19C)** — a new `ragent-connectors`
  crate adds a
  Claude-connector-equivalent integration catalogue over MCP: a connector is a
  named integration (Google Drive, Slack, GitHub, Git, Postgres, Puppeteer, and
  the wider community catalogue) carrying a category, an auth shape, and one or
  more MCP servers, resolved onto the existing `McpClient` and the durable
  `mcp_state.json` enable ledger. It is driven by the twelve-subcommand
  `/connectors` family plus the `/connectors marketplace` sub-subcommand family
  (with an interactive off-the-event-loop catalogue browser
  `/connectors claude [query] [--category <name>] [--refresh]`, an isolated
  `test` harness, endpoint-provenance
  reporting via `stores [--check]`, and a `/connectors marketplace` family that
  manages the additional catalogues the browser fetches and merges into one
  panel with per-row attribution), a `ragent connectors` CLI parity surface,
  and a `connectors` config block (master switch, store dir, catalogue endpoints
  and fetch budgets, credential-name mapping). Spec `connectors` FR-001..FR-044
  / NFR-001..003, plan T-001..T-026, plus `assets/connectors/fixtures/` and an
  acceptance walk.
- **ANTIPAT M1 ASCII sweep (TUI)** — the message-window tool-category icons and
  the status-bar service icons are now plain ASCII marker prefixes (`[file]`,
  `[dir]`, `[search]`, `[exec]`, `[git]`, `[team]`, ...) and short text tags
  (`CDX:`, `LOG:`, `AUTO:`, `EDIT:`, `TELE:`, `YOLO:`, `GCF:`, `BUSY: `)
  instead of emoji; `shorten_middle` / `truncate_with_ellipsis` budget the
  single-character ellipsis glyph (U+2026).
- **`/yolo` persists to the user-global config only** — the toggle always writes
  `~/.config/ragent/ragent.json`, never the project-local `.ragent/ragent.json`
  (SECTASKS T-011). When the process is started with `ragent --config <file>`,
  that explicit file is the only config loaded, so it becomes the write target
  instead (the user-global file is never read in that mode).
- **`/yolo on|off|status` are real subcommands** — `on`/`off` set and persist the
  state (repeating either is idempotent), `status` reports it without changing
  it, a bare `/yolo` toggles, and an unknown subcommand is refused.
- **Spec plan parser** — dependency-range expansion accepts `T-001..T-014` and
  `T-001...T-014` in addition to the existing dash and `through` spellings.
- **`os_info` host-introspection tool and `/osinfo` command (spec `osinfo`)** —
  a new read-only `os_info` tool (registry 171 -> 172) reports the host OS and
  distro, CPU, graphics adapters (integrated/discrete/virtual) and graphics-API
  support with version numbers, physical hardware
  (system/chassis/motherboard/BIOS identity plus storage devices and network
  interfaces), memory, uptime, and the process environment; the
  `/osinfo show [--no-probe]` slash command renders the same report through the
  tool's collector/renderer, `/osinfo help` documents both modes, and the
  `probe` parameter defaults to `true` so every graphics API gets a version.
  Never writes, spawns only the allowlisted bounded diagnostics, and reads no
  serial numbers, UUIDs, or asset tags.
- **Research scholarly-engine default flip** — scholarly backends (OpenAlex)
  are excluded from research web sweeps by default; `--papers` replaces
  `--no-papers` / `--no-scholarly` at every entry point (root CLI, TUI,
  `POST /research`), `ResearchRunRequest`/the config carry `papers`, and
  `research.exclude_academic_engines` now only persists the exclusion.
- **Three new tracked research items** — `research/codemigrate/`,
  `research/connectors/`, and `research/vendormarketplace/`, each with
  `RESEARCH.md`, `CORPA.md`, and a `sources/` tree (`research/INDEX.md` lists
  15 items).
- **Guards** — new `scripts/check-connector-endpoint-literal.sh` (FR-038,
  NFR-001) wired into `pre-flight.sh` and CI; the retired
  `scripts/check-non-ascii.sh` guard removed.

**Earlier Release Highlights (v1.0.107 → v1.0.122):**

- **v1.0.122 — Security and anti-pattern remediation sweep.** The `ANTIPAT.md`
  milestones M0 and M2-M7 plus the `SECTASKS.md` remediation programme through
  MS-05: the two shipping defects and highest-severity containment holes (M0),
  standards conformance / shared-helper de-duplication / silent-error suppression
  / vocabulary unification / structural debt / dependency hygiene (M2-M7 - major
  bumps `rmcp` 3.5, `rusqlite` 0.40, `ratatui` 0.30, ..., and the retired
  `ragent-team` shim crate, 17 -> 16 crates), and the shared `ragent_types::guard`
  module with a single `ragent_types::sanitize` redaction chokepoint and four
  `security-guards` CI gates (MS-05). Only `ANTIPAT.md` M1 (ASCII conformance)
  remained open, now addressed by the v1.0.128 work above.

- **Earlier highlights (v1.0.107 → v1.0.121)** — `/simplify` phases, config rules
  and fixes, `/spec govcreate`, research output limits and engine exclusion,
  `--url-cloak` source defanging, the message input queue, plugin component
  bridging (skills/MCP/commands/agents/hooks), durable global MCP
  enable/disable state, sessionful Streamable-HTTP MCP support, and the
  introspection tools `tool_info` / `commands_info` (169 -> 171 tools in
  v1.0.121).

- **Uncommitted (older, on top of v1.0.109)** — TUI paint-safety fix: `should_render`
  (`crates/ragent-tui/src/lib.rs`) now also forces the safety-interval paint
  when a message-cache group is still pending (`message_cache_dirty_from <
  messages.len()`), closing the PERF-042 throttle-tail stall where a tool-call
  row left unpainted by the throttle stayed invisible while the status bar
  already showed it running (regression test
  `test_pending_message_cache_group_forces_safety_paint` in
  `crates/ragent-tui/tests/test_perf_render_idle.rs`). Dependency hygiene:
  unused `dirs`/`tokio`/`tempfile` dependencies dropped and four stale
  `deny.toml` advisory ignores removed. New spec draft: **`plugins`**
  (`specs/plugins/`) — a plugin system loading Codex- and Claude
  Code/Desktop-dialect plugins onto an embedded, budget-sandboxed JavaScript
  engine behind a versioned host API (`register_tool`, `register_command`,
  `config.get`, `message.*`, `log`, `plugin.read_text_file`), managed through
  the six-subcommand `/plugins` family (`list`, `add`, `remove`, `enable`,
  `disable`, `help`, `test`).

- **`/simplify` final phase over v1.0.106..v1.0.108 (v1.0.109)** — dead
  `utf8_prefix_len` removed from the LLM HTTP client, `config_agents_dir`
  collapsed into `global_agents_dir`, byte-wise `extract_http_status` parsing,
  and one shared `lock_conn()` helper with `SourceVaultError::LockPoisoned`
  replacing eight poisoned-lock sites in `ragent-research`.

- **Config rules and fixes (v1.0.108)** — GitLab legacy credential migration
  scans the real legacy `~/.ragent/` root again (regression pinned), the
  team-blueprint "global" fallback restores the legacy
  `~/.ragent/blueprints(/teams)` scan, `/spec govcreate` mutex-lock sites
  uniformly recover poisoned locks, `/config show` renders unavailable global
  dirs as "(unavailable)", `acquire_local` no longer misreports exact-cap
  natural completion as budget exhaustion, and research helpers
  (`num_prefix_re`, `join_text`) are deduplicated.

- **`/spec govcreate` — spec authoring from an architecture document
  (v1.0.107)** — `/spec govcreate <spec-id>
  <content-ref> <target-folder>` (spec `govdoc`, FR-001..FR-020) authors a draft
  spec from an architecture document (local folder/file or public URL),
  orchestrating acquire → extract → author → write stages with live
  `[ .. ]/[ ok ]/[fail]` TUI progress, Escape cancellation, an FR-011
  non-empty-target guard (`--force` to override), and CLI parity via
  `ragent spec govcreate`. The same worktree carries HEAD~3 review fixes:
  SSE chunk coalescing flushes pending UTF-8 bytes via
  `Utf8Error::error_len()` (no surrogate splitting; six regression tests),
  secret redaction captures key+separator with a base64/base64url value
  charset, spec dependency-range expansion is capped at 1000 IDs,
  `delete_memories_by_filter` reports real row counts, masterfetch cache
  deletes corrupt rows, and the TUI Markdown renderer normalises ragged pipe
  tables (`preprocess_markdown_tables`). Documentation: `docs/howtos/tools.md`
  is replaced by 26 per-category tool pages in `docs/howtos/tools/` with
  argument tables and examples, each with a generated PDF.

**Earlier highlights (through v1.0.106):**

- **`--url-cloak` research source defanging (v1.0.106)** —
  `/research create` gained `--url-cloak`, which writes the report's web source
  URLs as defanged plain text instead of clickable links (scheme rewritten
  `https://` -> `hxxps://` / `http://` -> `hxxp://`, every dot bracketed
  `example.com` -> `example[.]com`, wrapped in a Markdown code span). It applies
  to the `**Sources:**` bullets under each finding and the `References Index`
  table in `RESEARCH.md` (plus the `Sources Reference` table in `CORPA.md`),
  leaving non-URL rows untouched. Available on the root CLI, the TUI, and
  `POST /research` (`url_cloak`), recorded in frontmatter (`url_cloak: true`)
  for `/research update` replay, and off by default.

- **Research output limits, engine exclusion, and progress-table detail
  (v1.0.105)** — `/research create` caps its `## Concepts`
  and `## Findings` lists at 5 and 20 by default, reordering
  most-relevant-first (an entry's relevance is the highest `[#N]` source rank
  it cites; ties break by cited count, then original model order) before
  truncation and renumbering the survivors contiguously (spec `researchmax`).
  The limits are set per run with `--max-concepts N` / `--max-findings N` on
  the root CLI, the TUI, and `POST /research` (new `max_concepts` /
  `max_findings` fields, round-tripped through the invocation summary), or
  persistently via `research.max_concepts` / `research.max_findings`; `0` means
  unbounded and non-integers are rejected. `mf_search` gained an
  `exclude_engines` array (spec `researchnoacc`) that removes named backends
  before any request is dispatched; research excludes scholarly backends
  (OpenAlex) by default, with `--papers` opting them back in (config
  `research.exclude_academic_engines` persists the exclusion), routing through
  it, so OpenAlex consumes no search budget and cannot shadow general-web URLs
  in dedup, and `POST /research` gained a `papers` field. `--oa-enable` /
  `--no-oa` toggle open-access recovery per run. The per-engine progress table
  breaks exclusions out by reason (`papers`/`pdf`/`relev`/`short`/`fetch`) and
  fetch failures out by cause (timeout/network/block/http/wall/js/extraction).
  `/spec impl` expands a task-range Dependencies cell (`T-001–T-014`) into every
  spanned ID; the TASKS panel row reads `[STATUS] <id> title`; every toggled
  side panel takes 50% of the window width.

- **Functional anti-pattern remediation (FUNC-038..069, 080..082)** — the
  FUNCPLAN.md second pass closes the plan's M1 tail and all of M2-M5:
  `mf_search` keyless engines surface a dead engine as an error instead of a
  zero-result success; `github_merge_pr` rejects an unknown merge method
  (extracted `parse_merge_method`) instead of silently merging; the last three
  production poison-lock panics recover via `PoisonError::into_inner`; GitLab
  GETs retry `429` honouring `Retry-After` with bounded request/entry budgets
  and full pagination; research `GatherLog` append/flush run through
  `block_in_place`; `move_file` renames first (no orphan dirs), `copy_file`
  refuses a self-copy, `append_file` flushes; GitHub `post`/`put`/`patch`
  honour `base_url`; percent-encoding is byte-wise (UTF-8 correct) across
  GitHub/GitLab; codeindex `parent_id` resolves multi-level nesting to a
  fixpoint; the keyword verifier no longer reports an empty/uncited analysis
  as `passed`; the server auth comparison hashes fixed-length digests and the
  rate limiter enforces exactly 60/min; and a new
  `scripts/check-poison-locks.sh` guard (with `--self-test`) is wired into
  `pre-flight.sh` and CI.
- **Agent per-turn hot path (PERF-032..040 + PERF-048)** — the
  per-turn allocation and clone load in the agent loop is removed: the
  provider-facing transcript is held and handed out behind an
  `Arc<Vec<ChatMessage>>` (no per-turn deep clone, PERF-032); a pure history
  append converts only the new tail (`SessionState::take_cached_for_append` +
  `record_history_base`, PERF-033); the subagent tool surface is cached behind
  the tool-registry version (PERF-034); `LoopTracker` is `Copy` (PERF-035); a
  `RequestTokenTracker` makes the per-step compaction token estimate
  O(changed message) instead of O(history) (PERF-036); tool/result pairing is a
  single pass (PERF-037); the compaction prompt is assembled into one buffer
  (PERF-038); memory-entry token costs are memoised and prompt builders use
  `write!` (PERF-039); the activity log is written by one background task per
  process fed by a bounded queue (PERF-040); and the TUI viewers retain a single
  copy of their rendered rows (PERF-048). New benches `turn_loop` /
  `m3_hot_paths`; new guards `test_activity_writer` and `test_no_percall_regex`.
  `rustls` bumped 0.23.43 -> 0.23.45 (RUSTSEC-2026-0285).
- **Data-layer, network, TUI, and regex performance milestones** — the M2..M6
  performance plan is complete: the TUI render loop repaints nothing at idle,
  the async runtime no longer blocks workers, network clients and git spawns are
  reused, every page/candidate regex is hoisted to a `OnceLock`/`LazyLock`
  static, and the storage/code-index/MCP/research data paths no longer copy work
  on hot paths (`docs/PERFPLAN.md`, `docs/agentorch.md` §10).
- **Research-crate simplify pass (v1.0.88)** — a `/simplify`
  audit over all `ragent-research` sources: fixed a `parse_subject_summary`
  slice panic (`}`-before-`{`), made `AnalysisEngine::with_brief` a required
  trait method, `OnceLock`-cached hot regexes, shared section-emitter layout
  module for the report/IMRaD `RESEARCH.md` assemblers, deterministic sorted
  search-engine ordering on both gather paths, single-pass media-type
  classification and pdf/youtube tallies.
- **Goal-driven loop programming (`/loop`, spec agentloop)** — a goal-driven
  agentic loop that runs a `LoopSpec` to a stop condition: `completed` (goal
  achieved or verification passed), `budget_exhausted`, `error`, or
  `interrupted`. Includes a verification gate for verify commands (600 s
  timeout), restriction layers checked in `deny_reason` order (tool set ->
  read-only -> scope, invalid globs failing closed), pre-loop snapshot/git
  capture, forced checkpoints, and a post-loop rollback flow. Tunables live in
  the `loop` section of `ragent.json` (`loop.max_steps` 512, `cost_limit`,
  `error_retry_allowance` 3, `checkpoints` true, `checkpoint_timeout_secs`
  120); documented in `docs/howtos/loopprogramming.md` (v1.0.81 cycle,
  uncommitted).
- **How-to documentation set** — `docs/howtos/reactagent.md` (core per-turn
  ReACT loop), `docs/howtos/loopprogramming.md` (goal-driven loops), and
  `docs/howtos/office.md` (the PDF tool family, format matrix, and
  configuration).

- **Research web-search quota controls** — `--max-search-calls N` places a hard, run-scoped cap on total web-search calls per research run, shared via `Arc` across every supervisor/competitive researcher and gather pass; a run-scoped query cache memoises identical sub-queries so parallel researchers reuse cached hits instead of re-issuing paid calls (FR-016 of specs/opendeepresearch).
- **`--depth` bounds web volume by default** — the effective web-source budget is derived from the selected depth (shallow 6 / standard 9 / deep 15) unless `--max-web-results` is passed explicitly, so `--depth shallow` now actually limits search/fetch volume.
- **`/research update <name>` invocation replay** — re-runs a research item by replaying its recorded invocation line, overwriting `RESEARCH.md` and associated files with freshly gathered results; available via CLI, TUI, and HTTP `PUT /research/{name}`.
- **`--mode competitive` defaults `--format` to `comparison-table`** — competitive runs no longer require an explicit `--format comparison-table`.
- **GitHub `blob/` URLs no longer fail research gathering** — file-view URLs are rewritten to `raw.githubusercontent.com` and non-HTML content bypasses the readability gate, eliminating guaranteed `readability extraction failed` rejections.
- **`/clip` slash command** — copies the rendered message-window transcript to the system clipboard in one step.
- **`/research list` renders a human-readable table again** — the fixed-width `NAME/TITLE/STATUS/CREATED/MODIFIED` table is restored as the default output, with JSON behind the `--json` flag.
- **Supervisor/competitive research modes** — `/research create` now supports `--mode tiered|supervisor|competitive`. `supervisor` runs a lead supervisor that spawns parallel sub-topic researchers; `competitive` compares a set of entities and defaults to `--format comparison-table` unless an explicit `--format` is supplied (FR-009/FR-014 of specs/opendeepresearch).
- **Research brief and clarification** — deterministic research-brief generation and a single clarifying question for ambiguous topics, surfaced via `--brief`/`--clarify` and `SessionEvent::NeedsClarification` (FR-004/FR-005).
- **Allowed roots for path escape checking** — `dirs.allowed_roots` in `ragent.json` permits reading/writing files in multiple project directories (FR-017).
- **Per-page web summarisation and self-evaluation scorecard** — optional page summariser (`--summarization-model`) and deterministic quality scorecard (`--evaluate`/`research.evaluate.enabled`) extend research reports (FR-002/FR-015).
- **Documentation refresh for v1.0.77** — updated `CHANGELOG.md`, `README.md`, `SPEC.md`, `STATS.md`, `QUICKSTART.md`, `TUI-QUICKSTART.md`, and how-to docs to reflect the latest release (v1.0.77).
- **Research evaluation scorecard configuration** — new `research.evaluate` section in `ragent.json` for self-evaluation scorecard settings (FR-015 of specs/opendeepresearch). Includes `ResearchEvaluateConfig` and `enabled` default.
- **Clippy `for_kv_map` fix** — Ollama provider iteration now uses `values()` instead of destructuring a key-value pair; the LangSearch merge test expectation was also corrected (v1.0.74).
- **Sub-agent / teammate step visibility** — TUI step log now shows tool calls from tracked sub-agents and teammates with an `[agent-tag]` prefix; rebuilt step tags for lagged event-bus bursts prevent undercounting (v1.0.73).
- **Tool-permit handling fix** — `SessionProcessor` now surfaces an explicit error when the per-tool resource permit cannot be acquired (v1.0.73).
- **Token counting fixes** — TUI context panel percentages now use a consistent bytes-to-tokens conversion so they align with the status-bar usage figure (v1.0.72).
- **Context side panel** — toggleable `Alt+C` panel showing live, quantified context-window occupancy, with non-blocking refresh (v1.0.71).
- **Research clustering** — `/research cluster` concept extraction and a new `cluster.rs` payload builder (v1.0.71).
- **One-shot agent runner** — lightweight, provider-agnostic single-prompt execution path without spinning up a full agent loop (v1.0.71).
- **Code index semantic graph** — New `codeindex_godnodes`, `codeindex_path`,
  `codeindex_explain`, and `codeindex_communities` tools; typed edge graph with
  `calls`, `imports`, `inherits`, `references`, `mixes_in`, `implements` edges;
  community detection via label propagation; `/codeindex graph build` sub-command
  (v1.0.43)
- **Bang commands** — Prefix any prompt with `!` to run a shell command and
  have the model review its output; works in TUI and `ragent run` (v1.0.42)
- **Compaction fix** — `select()` now forces at least one message into the head
  when there are 2+ messages, preventing the "nothing to summarise" stuck state
  (v1.0.43)
- **Research panic fixes** — Vendored `html2text` with `saturating_sub` patches;
  `extract_pdf_text` now runs on a dedicated OS thread with `panic_guard` (v1.0.40)
- **Start-of-turn compaction** — Uses persisted provider-reported input token
  count so it aligns with the TUI usage percentage (v1.0.34)

---


## Table of Contents

- [Executive Summary](#executive-summary)

### Part I: Foundation & Basics

1. [Overview](#overview)
2. [Architecture](#architecture)
3. [Core Features](#core-features)
4. [Security & Permissions](#security-permissions)
5. [Configuration](#configuration)

### Part II: User Interface & API

6. [Terminal User Interface (TUI)](#terminal-user-interface-tui)
7. [HTTP Server & API](#http-server-api)

### Part III: Data & Knowledge Systems

8. [Code Index](#code-index)
9. [Memory System](#memory-system)
10. [Spec Management](#spec-management)
11. [Research System](#research-system)
11A. [Project Scaffolding (`/new`)](#11a-project-scaffolding-new)

### Part IV: Agent Customization & Extension

11. [Custom Agents](#custom-agents)
12. [Skills System](#skills-system)

### Part V: Multi-Agent Coordination

14. [Teams](#teams)
15. [Swarm Mode](#swarm-mode)
16. [Autopilot Mode](#autopilot-mode)
17. [Orchestrator & Multi-Agent Coordination](#orchestrator-multi-agent-coordination)

### Part VI: External Integrations

18. [GitHub & GitLab Integration](#18-github--gitlab-integration)
19. [MCP Integration (Model Context Protocol)](#mcp-integration-model-context-protocol)

### Part VII: Operations & Reference

20. [Auto-Update Mechanism](#auto-update-mechanism)

**Appendices**

- [Appendix A: Version History](#appendix-a-version-history)
- [Appendix B: Documentation](#appendix-b-documentation)
- [Appendix C: Project Contact & Repository](#appendix-c-project-contact--repository)
- [Appendix D: Changelog (v0.1.0-alpha.104 → v0.1.0-alpha.116)](#appendix-d-changelog-v010-alpha104--v010-alpha116)
- [Appendix E: Earlier Changelog (2025-01-16 → 2025-04-21)](#appendix-e-earlier-changelog-2025-01-16--2025-04-21)
- [Appendix F: Changelog (v0.1.0-beta.1 → v0.1.0-beta.28)](#appendix-f-changelog-v010-beta1--v010-beta28)

### List of Diagrams

| # | Diagram | Section | Description |
|---|---------|---------|-------------|
| 1 | [Core Execution Loop](#how-it-works) | Executive Summary | High-level session/agent/tool flow |
| 2 | [System Architecture](#2-architecture) | Architecture | Full crate and component topology |
| 3 | [Crate Dependency Graph](#22-crate-dependency-graph) | Workspace Crates | Inter-crate dependency relationships |
| 4 | [Event Bus Flow](#23-event-bus-flow) | Architecture | Internal pub/sub message routing |
| 5 | [Session & Tool Execution Flow](#35-session--tool-execution-flow) | Core Features | LLM call → permission → tool dispatch loop |
| 6 | [Provider Selection Flow](#36-provider-selection-flow) | Core Features | Multi-provider routing and health checks |
| 7 | [TUI Component Architecture](#44-tui-component-architecture) | Terminal User Interface | UI layout and event wiring |
| 8 | [HTTP API Request Flow](#54-http-api-request-flow) | HTTP Server & API | REST + SSE lifecycle |
| 9 | [Code Index Pipeline](#62-architecture) | Code Index | File scan → parse → index → search |
| 10 | [Permission Security Layers](#41-permission-security-layers) | Security & Permissions | 5-layer defense-in-depth |
| 11 | [Bash Security — 7 Layers](#42-bash-security--7-layers) | Security & Permissions | Bash command defense flow |
| 12 | [Permission Request Flow](#43-permission-request-flow) | Security & Permissions | From tool call to user decision |
| 13 | [Permission Rules Evaluation](#44-permission-rules-evaluation) | Security & Permissions | Rule matching and resolution |
| 14 | [Agent Execution Loop Phases](#37-agent-execution-loop-phases) | Core Features | One complete turn from input to response |

---



---

# Part I: Foundation & Basics

---

## 1. Overview

Ragent is an AI coding agent for the terminal, built in Rust. It provides multi-provider LLM orchestration, a built-in tool system, terminal UI, and client/server architecture — all compiled into a single statically-linked binary.

### 1.1 Key Characteristics

| Characteristic | Description |
|----------------|-------------|
| **Single binary** | Statically linked, zero runtime dependencies beyond OS libraries |
| **Multi-provider** | 13 first-class LLM provider IDs with auto-discovery and health checks |
| **Tool-rich** | 149 registered tools across 21 categories in the default build (`mf_screenshot` is engine-gated, spec `openhands` FR-029) |
| **Local-first** | SQLite, Tantivy, and tree-sitter compiled in; no external services required |
| **Streaming** | Real-time token, tool, and event streaming via TUI and HTTP SSE |
| **Extensible** | Custom agents, skills, MCP servers, and provider modules |
| **Secure by default** | Permission system, bash safety, file guards, secret redaction |

### 1.2 Terminology

| Term | Meaning |
|------|---------|
| **Session** | A single conversation/run with an agent, persisted to SQLite |
| **Agent Profile** | A named configuration of model, system prompt, tools, and permissions |
| **Tool** | A typed, invokable capability (file ops, shell, search, etc.) |
| **Event Bus** | Tokio broadcast channel used for cross-component real-time updates |
| **Code Index** | Tree-sitter + Tantivy index of the project codebase |
| **Memory** | Three-tier persistence: file blocks, structured SQLite, optional embeddings |
| **Skill** | A YAML pack that injects tools, prompts, and file context |
| **Team** | A named multi-agent workspace with shared tasks and mailbox messaging |
| **Swarm** | Parallel task decomposition across spawned agents |
| **Autopilot** | Autonomous mode with auto-approval and iteration limits |
| **Initiative** | Durable project-scoped goal with milestones |
| **Telemetry** | OpenTelemetry metrics export for operator observability |

---

## 2. Architecture

Ragent is organised as a Cargo workspace of focused crates. Each crate owns a
single concern, and the top-level `src/main.rs` wires them together behind a clap
CLI.

```mermaid
graph TB
    subgraph "Workspace Crates"
        A[ragent-agent]
        B[ragent-bench]
        C[ragent-codeindex]
        D[ragent-config]
        E[ragent-llm]
        G[ragent-server]
        H[ragent-specs]
        I[ragent-storage]
        K[ragent-tools-core]
        L[ragent-tools-extended]
        M[ragent-tools-vcs]
        N[ragent-tui]
        O[ragent-types]
        P[ragent-research]
        S[ragent-surface]
    end

    Q[src/main.rs CLI entry]
    Q --> A
    Q --> N
    Q --> G
    A --> O
    A --> D
    A --> E
    A --> K
    A --> L
    A --> J
    A --> H
    A --> P
    N --> A
    N --> O
    G --> A
    G --> O
    J --> O
    K --> O
    L --> O
    M --> O
    C --> O
    I --> O
    E --> O
    D --> O
    B --> O
    H --> O
    P --> O
```

**Figure 2:** System Architecture — Full crate and component topology

### 2.1 Workspace Crates

| Crate | Responsibility | Approx. Lines |
|-------|----------------|---------------|
| `ragent-types` | Shared IDs, events, messages, errors, sanitisation | ~2,700 |
| `ragent-config` | Configuration loading, defaults, permission rules | ~1,850 |
| `ragent-connectors` | Connector descriptors, store/ledger, catalogue providers, MCP bridge, `/connectors` surface | ~16,400 |
| `ragent-storage` | SQLite persistence, snapshots, encrypted credentials | ~2,800 |
| `ragent-llm` | Provider clients and model/provider registry | ~6,700 |
| `ragent-tools-core` | File, shell, search, and utility tools | ~4,100 |
| `ragent-tools-extended` | Memory, code index, PDF, web tools | ~4,300 |
| `ragent-tools-vcs` | GitHub and GitLab tool surface | ~5,200 |
| `ragent-agent` | Session processor, agent resolution, tool registry, memory, MCP; team runtime + 20 team tools (former `ragent-team` shim folded in) | ~12,500 |
| `ragent-codeindex` | Tree-sitter parsing, SQLite/Tantivy index, file watcher | ~4,000 |
| `ragent-server` | Axum HTTP routes and SSE streaming | ~2,400 |
| `ragent-tui` | Ratatui terminal interface | ~8,900 |
| `ragent-bench` | Criterion benchmarks shared between TUI and CLI | ~900 |
| `ragent-specs` | Spec lifecycle management, SDD artifact generation, consistency validation, constitution parsing | ~3,200 |
| `ragent-research` | Research types, gatherers, and plan-dep parser | ~1,600 |
| `ragent-plugins` | Plugin discovery, dialect manifests, sandboxed JS runtime, lifecycle, skills/MCP/commands/agents/hooks bridges, `/plugins` surface | ~6,200 |
| `ragent-surface` | Shared surface helpers for the plugin/connector families: attribution, subcommand tokeniser, store-dir resolution, harness step model, schema samples | ~440 |

### 2.2 Crate Dependency Graph

```mermaid
graph TD
    T[ragent-types]
    D[ragent-config]
    S[ragent-storage]
    L[ragent-llm]
    TC[ragent-tools-core]
    TE[ragent-tools-extended]
    TV[ragent-tools-vcs]
    A[ragent-agent]
    CI[ragent-codeindex]
    SV[ragent-server]
    TU[ragent-tui]
    SP[ragent-specs]
    RS[ragent-research]
    B[ragent-bench]
    PL[ragent-plugins]
    CN[ragent-connectors]
    SU[ragent-surface]

    T --> D
    T --> S
    T --> L
    T --> TC
    T --> TE
    T --> TV
    T --> CI
    T --> SP
    T --> RS
    T --> CN
    D --> L
    D --> A
    D --> TU
    D --> CN
    S --> A
    S --> TU
    L --> A
    L --> TU
    L --> SV
    TC --> A
    TE --> A
    TV --> A
    A --> TU
    A --> SV
    CI --> A
    SP --> A
    RS --> A
    B --> TU
    B --> SV
    PL --> A
    PL --> TU
    PL --> SU
    CN --> TU
    CN --> SU
```

**Figure 3:** Crate Dependency Graph — Inter-crate dependency relationships

Per-crate `fn`-definition counts (lexical lower bounds, re-measured 2026-10-21;
the v1.0.142 `countreport.md` inventory records 25,823 on its tree):
`ragent-tui` 6,054 · `ragent-agent` 4,630 · `ragent-tools-extended` 2,956 ·
`ragent-research` 2,513 · `ragent-codeindex` 1,130 · `ragent-llm` 1,102 ·
`ragent-tools-core` 1,001 · `ragent-plugins` 991 · `ragent-specs` 981 ·
`ragent-tools-vcs` 941 · `ragent-connectors` 907 · `ragent-config` 787 ·
`ragent-types` 590 · `ragent-storage` 585 · `ragent-telemetry` 550 ·
`ragent-bench` 412 · `ragent-server` 363 · root `ragent` binary 159 ·
`ragent-surface` 114 — 26,768 workspace-wide (8,985 in `src/`, 17,667 in
`tests/`, 66% test code).

### 2.3 Event Bus Flow

The event bus is a tokio broadcast channel (`tokio::sync::broadcast`). Every
component can publish and subscribe to strongly-typed events.

```mermaid
graph LR
    SP[Session Processor] --> EB[Event Bus]
    TUI[TUI] --> EB
    HTTP[HTTP Server] --> EB
    Tools[Tool Executors] --> EB
    EB --> SP
    EB --> TUI
    EB --> HTTP
    EB --> Storage[Storage / Memory]
    EB --> Teams[Team Manager]
```

**Figure 4:** Event Bus Flow — Internal pub/sub message routing

Key event types include:

| Event | Purpose |
|-------|---------|
| `AssistantChunk` | Streaming LLM token or reasoning block |
| `ToolRequested` / `ToolExecuted` | Tool call lifecycle |
| `PermissionRequested` / `PermissionReplied` | Permission dialog flow |
| `QuestionRequested` / `QuestionAnswered` | User question tool |
| `AgentNotice` | Non-blocking status/information message |
| `CompressionStarted` / `CompressionFinished` | Context compaction lifecycle (auto pre-send summarisation and emergency overflow compaction) |
| `TeammateIdle` / `TeammateFailed` / `TeammateResumed` | Team coordination |
| `SubagentSpawned` / `SubagentCompleted` / `SubagentKilled` | Background agents |
| `SubagentSuspended` / `SubagentResumed` | Cooperative suspension of a background sub-agent (spec `agentui` FR-035) |
| `RouterClassification` / `RouterTierSelected` | Model-router routing decisions |
| `RunCostSummary` | Per-run token/cost summary emitted at session run end |
| `FromFileBodyPreview` | Preview of text extracted from a `--from-file` research seed |
| `SynthesizeResult` | Research LLM synthesis outcome (success/fallback/error) |

---

## 3. Core Features

### 3.1 LLM Providers

Ragent supports multiple providers through a common trait interface. Providers
are registered in a runtime registry, and the session processor selects one
based on the active agent/model.

#### Supported Providers

| Provider | Local/Cloud | Discovery | Streaming | Vision | Tools | Notes |
|----------|-------------|-----------|-----------|--------|-------|-------|
| `anthropic` | Cloud | Static catalog | Yes | Yes | Yes | Claude models, thinking blocks |
| `openai` | Cloud | Static + dynamic | Yes | Yes | Yes | GPT-4o, o1, etc. |
| `copilot` | Cloud | Static | Yes | No | Yes | GitHub Copilot token-based |
| `gemini` | Cloud | Static + dynamic | Yes | Yes | Yes | Google Gemini, up to 2M context |
| `huggingface` | Cloud | Dynamic | Yes | Yes | Some | HF Inference API |
| `ollama` | Local | Dynamic | Yes | Yes | Yes | Local Ollama server |
| `ollama_cloud` | Cloud | Dynamic | Yes | Yes | Yes | Managed Ollama endpoints |
| `generic_openai` | Either | Config-driven | Yes | Yes | Yes | Any OpenAI-compatible endpoint |
| `azure_foundry` | Cloud | Dynamic | Yes | Yes | Yes | Microsoft Azure AI Foundry |
| `azure_resource` | Cloud | File-based | Yes | Yes | Yes | Reads `azureresources.json` |
| `xai` | Cloud | Static | Yes | Yes | Yes | xAI Grok API |
| `bedrock` | Cloud | Dynamic | Yes | Yes | Yes | AWS Bedrock, SigV4 signing |
| `openrouter` | Cloud | Dynamic | Yes | Yes | Yes | OpenAI-compatible model aggregator; single API key unlocks 100+ upstream models |
| `router` | Cloud | Static | Yes | Yes | Yes | Model Router with 15-dimension classifier |

(The registry exposes 14 provider IDs; the `router` is a virtual provider that
selects a downstream model based on request characteristics.)

#### Provider Features

| Feature | Description |
|---------|-------------|
| **Health checks** | `Provider::health()` returns `Healthy`, `Degraded`, or `Unhealthy` |
| **Model discovery** | `Provider::discover_models()` returns available models with capabilities |
| **Streaming** | All providers emit `StreamEvent::Chunk` and `StreamEvent::Done` |
| **Tool calling** | Providers that support function calling receive tool schemas |
| **Vision** | Image inputs passed when the model reports vision capability |
| **Reasoning levels** | Anthropic/OpenAI thinking blocks map to `low`/`medium`/`high` |
| **Model routing** | `router` provider classifies prompts and picks a downstream provider/model/tier |
| **OpenRouter integration** | `openrouter` provider discovers and streams from 100+ OpenAI-compatible models with a single API key |

#### Anthropic Models

| Model | Context | Notes |
|-------|---------|-------|
| `claude-sonnet-4-20250514` | 200k | Default reasoning model |
| `claude-opus-4-20250514` | 200k | High capability |
| `claude-haiku-4-20250514` | 200k | Fast/cheap |

#### OpenAI Models

| Model | Context | Notes |
|-------|---------|-------|
| `gpt-4o` | 128k | General purpose, vision |
| `o3-mini` | 200k | Reasoning |
| `o1` | 200k | High reasoning |

#### Ollama Cloud Provider

- Endpoint configured via `OLLAMA_CLOUD_HOST` or `ragent.json`
- Dynamic model discovery against `/api/tags`
- Supports vision models when the Ollama model reports `vision`

#### Ollama (Local) Provider

- Default endpoint `http://localhost:11434`
- Dynamic discovery; no API key required
- Vision support depends on the pulled model

#### Google Gemini Provider

- API key via `GEMINI_API_KEY`
- Massive context windows (up to 2M tokens)
- Discovery via Gemini models API

#### Hugging Face Provider

- Token via `HF_TOKEN` or `HUGGING_FACE_HUB_TOKEN`
- Public `/v1/models` discovery; falls back to static catalog on empty results
- Rate-limit tracking

#### Azure AI Foundry Provider

- API key via `AZURE_AI_FOUNDRY_API_KEY` (or `AZURE_AI_FOUNDRY_API_KEY`)
- Base URL via `AZURE_AI_FOUNDRY_BASE`
- OpenAI-compatible endpoints with `api-key` header
- Dynamic model discovery, tool calling, vision, reasoning levels
- HTTP 429 retries with `Retry-After` and exponential backoff

#### Azure Resource (File) Provider

- Reads endpoint definitions from `azureresources.json` in `~/.config/ragent/` or `.ragent/`
- Supports per-endpoint API keys, custom context windows, capability tags, thinking config
- Can route to Anthropic Messages (`/anthropic/v1/messages`) or OpenAI (`/openai/v1/chat/completions`) API shape via `api_type`

#### OpenRouter Provider

- API key via `OPENROUTER_API_KEY` or `ragent auth openrouter <key>`
- Single `GET /api/v1/models` discovery call returns all 100+ upstream models
- OpenAI-compatible `POST /api/v1/chat/completions` streaming with tool-call and reasoning deltas
- Per-model metadata (context window, pricing, vision/reasoning flags) mapped from discovery
- Supports vendor-slug model ids such as `openrouter/anthropic/claude-sonnet-4`

#### Unified Tool-Calling Wire Path (spec `uniformtools` FR-002..FR-004)

Every provider encodes tool definitions, tool results and streamed tool-call
deltas through one declared contract, so no provider module builds its own tool
JSON. A client holds a `ToolWireProfile` - nine required decisions, no `Default`
and no optional field - and dispatches on it:

| Field | Values | What it decides |
|-------|--------|-----------------|
| `tool_envelope` | `openai_function`, `anthropic_tool`, `gemini_function_declaration`, `bedrock_tool_spec`, `responses_flat_function` | the wrapper `wire::encode::encode_tools` puts a definition in |
| `result_carrier` | `tool_role_message`, `user_tool_result_block`, `gemini_function_response`, `responses_function_call_output` | where `encode_message` puts a tool result |
| `id_field` | a JSON key such as `tool_call_id`, or `synthesised` | which field a call id occupies |
| `args_encoding` | `json_string`, `json_object` | whether accumulated arguments are a string or an object |
| `stream_keying` | `by_index`, `by_call_id` | how `wire::decode::ToolCallAccumulator` attributes a delta |
| `supports_parallel` | `yes` / `no` | whether the step may dispatch concurrently (see the gate below) |
| `supports_is_error` | `yes` / `no` | native error flag, or the `ERROR: ` prefix |
| `echo_reasoning` | `yes` / `no` | whether a signed reasoning block is replayed |
| `synthetic_ids` | `yes` / `no` | whether the harness mints the call id |

The six families (`openai_chat`, `anthropic_messages`, `gemini`,
`bedrock_converse`, `openai_responses`, `ollama_native`) each have one `const`
profile, and `wire::profile_table::PROVIDER_WIRE` names the family of every
registered provider id; `profile_for` is the one lookup the encoders, the
accumulator, the parallel gate and the wire matrix share. There is no catch-all
row: an id with no declared contract fails the coverage assertion in
`crates/ragent-llm/tests/test_wire_profile_table.rs` at build time, so a
provider cannot join the build without stating its tool-calling terms.

`ragent --wire-matrix` prints that contract per registered provider as one
aligned ASCII table (ten columns - the provider id plus the nine fields, one row
per id, sorted, no blank or `-` cell) and exits without contacting a provider or
executing a tool; a registered id with no row is reported by name and exits
non-zero rather than printing a hole in the table.

### 3.2 Tool System

The tool system is the primary way agents interact with the world. Each tool
has a JSON schema, a permission category, and an async `execute` method.

#### File Operations Tools (16)

| Tool | Purpose |
|------|---------|
| `apply_patch` | Apply a Codex-style patch with add/delete/update and file moves |
| `read` | Read file contents with line-range support |
| `Write` / `create` | Create or overwrite a file |
| `Edit` | Replace one exact string occurrence; optional `collapse_whitespace` relaxes matching (whitespace-run collapse + `\t`/`\n`/`\r`/`\\` escape decoding) |
| `multiedit` | Apply multiple edits atomically across one or more files; each edit honours its own `collapse_whitespace` flag |
| `patch` | Apply a unified diff |
| `rm` | Delete a single file |
| `move` / `move_file` | Move or rename a file/directory |
| `copy` / `copy_file` | Copy a file |
| `mkdir` / `make_directory` | Create directories |
| `append_to_file` | Append text to the end of a file |
| `file_info` | File/directory metadata |
| `diff_files` | Unified diff between files or strings |
| `Glob` | Find files matching a glob pattern |
| `list` | List directory contents |

#### File Operation Aliases

| Alias | Maps to |
|-------|---------|
| `read_file` | `read` |
| `write_file` | `Write` |
| `update_file` | `Write` |
| `delete_file` | `rm` |
| `apply_patch` | `patch` |
| `multiedit` | `multi_edit` (legacy parameter normalisation) |

#### Execution Tools (5)

| Tool | Purpose |
|------|---------|
| `Bash` | Run a shell command with 7-layer safety |
| `run_code` | Alias for `Bash`; accepts `code`/`command` |
| `bash_reset` | Reset persistent shell state |
| `calculator` | Evaluate mathematical expressions |
| `open` | Open/reveal files, folders, or URLs in the desktop environment |

#### Interactive Tools (2)

| Tool | Purpose |
|------|---------|
| `AskUserQuestion` | Ask the user a question (text or multiple-choice, with an optional default used on timeout) |
| `think` | Record a reasoning note |

#### Task Management Tools (4)

| Tool | Purpose |
|------|---------|
| `TaskCreate` | Create a session-scoped task with subject, description, optional owner, active_form, metadata, and blocked_by dependencies |
| `TaskUpdate` | Update a task's status (pending, in_progress, completed), subject, owner, metadata, or dependencies; auto-evaluates blocked tasks on completion |
| `TaskGet` | Retrieve the full record of a single task by ID |
| `TaskList` | List all session tasks, optionally filtered by status |

#### Utility Tools (6)

| Tool | Purpose |
|------|---------|
| `get_env` | Read non-sensitive environment variables |
| `calculator` | Evaluate mathematical expressions |
| `ragent_info` | Report the running ragent version, build time, git commit, compiler, and execution details (pid, parent pid, start time, uptime, executable, working directory, memory, thread count) |
| `os_info` | Report read-only host OS/hardware introspection (OS identity and distro, CPU, GPU adapters and graphics-API versions, physical hardware, memory, uptime, process environment); also `/osinfo show` |
| `tool_info` | Return a JSON-encoded dump of the tool registry (name, description, parameters schema, permission category, source, hidden state) |
| `commands_info` | Return a JSON-encoded catalog of every slash command (trigger, description, subcommands, flags) including plugin-contributed commands |

### 3.2.1 Tool System Categories Summary

| Category | Tools | Count |
|----------|-------|-------|
| File operations | `read`, `Write`/`create`/`update_file`, `Edit`, `multi_edit`/`multiedit`, `apply_patch`, `patch`, `rm`, `move`, `copy`, `mkdir`, `append`, `file_info`, `diff`, `Glob`, `list` | 18 |
| Shell / execution | `Bash`, `run_code`, `bash_reset`, `calculator`, `open` | 5 |
| Search | `Grep`, `codeindex_*` | 6 |
| Web / MasterFetch | `webfetch`, `websearch`, `http_request`, `mf_fetch`, `mf_crawl`, `mf_search`, `mf_cache_clear`, `mf_version` (+ `mf_screenshot`, engine-gated per FR-029) | 8 |
| Memory | `memory_read`, `memory_write`, `memory_replace`, `memory_store`, `memory_recall`, `memory_forget`, `memory_search`, `memory_migrate`, `conversation_search`, `session_search` | 10 |
| Code index | `codeindex_search`, `codeindex_symbols`, `codeindex_references`, `codeindex_dependencies`, `codeindex_status`, `codeindex_reindex` | 6 |
| Code graph | `codeindex_explain`, `codeindex_path`, `codeindex_communities`, `codeindex_godnodes` | 4 |
| Teams | 19 team lifecycle/task/message tools | 19 |
| Sub-agents | `new_agent`, `cancel_agent`, `list_agents`, `wait_agents`, `agent_complete` | 5 |
| VCS | 48 Git local, GitHub, and GitLab issue/PR/MR/pipeline tools | 48 |
| PDF | `pdf_read/write` | 2 |
| MCP | `mcp_tool` | 1 |
| Planning | `EnterPlanMode`, `ExitPlanMode` | 2 |
| Background tasks | `bg` | 1 |
| Initiatives / skills | `initiative`, `Skill` | 2 |
| Interactive | `AskUserQuestion`, `think` | 2 |
| Task management | `TaskCreate`, `TaskUpdate`, `TaskGet`, `TaskList` | 4 |
| Utility | `get_env`, `calculator`, `ragent_info`, `os_info`, `tool_info`, `commands_info` | 6 |

#### Team Tools (20)

| Tool | Purpose |
|------|---------|
| `team_create` | Create a named team |
| `team_spawn` | Spawn a teammate for a single task |
| `team_status` | Team status: members with their last five status transitions, undelivered mailbox messages, plans awaiting a verdict, and hook failures |
| `team_message` | Send a direct message |
| `team_broadcast` | Message all active teammates |
| `team_read_messages` | Read unread mailbox messages |
| `team_memory_read` / `team_memory_write` | Team-scoped memory access |
| `team_submit_plan` | Submit a plan for lead approval |
| `team_approve_plan` | Lead approve/reject a plan |
| `team_task_create` | Add a team task |
| `team_task_claim` | Claim a task |
| `team_task_complete` | Mark a team task done (owner or lead only) |
| `team_task_cancel` | Lead withdraws a pending/in-progress task |
| `team_assign_task` | Lead assigns a task directly |
| `team_idle` | Signal no more work |
| `team_shutdown_teammate` | Request teammate shutdown |
| `team_shutdown_ack` | Acknowledge shutdown request |
| `team_wait` | Block until teammates finish |
| `team_cleanup` | Delete team on-disk state |

#### Background Shell Task Tool (1)

| Tool | Purpose |
|------|---------|
| `bg` | Spawn, monitor, tail, and cancel long-running shell commands as **shell tasks** (never sub-agents, whose vocabulary is the execution contract). Actions: `spawn`, `list`, `status`, `output`, `tail`, `cancel`, `wait`, `cleanup`. `session_id` on `list`/`cleanup` must name the calling session - a foreign id is refused (`reason_code: wrong_session`) and deletes nothing. |

#### Agent Search Tool (1)

| Tool | Purpose |
|------|---------|

#### Durable Initiatives & Skill Management Tools (2)

| Tool | Purpose |
|------|---------|
| `initiative` | Create, checkpoint, list, and close durable project-scoped goals with milestones. |
| `Skill` | List, read, load, and reload skill packs at runtime. |

### 3.2.1 Tool-Call Wire Path and the One Dispatch Pipeline (spec `uniformtools`)

Four rules apply to every provider and every turn, whichever surface drove it.

**A provider declares its contract, it does not build tool JSON.** The nine
profile fields in the section above are the contract
(`ragent_llm::provider::wire::ToolWireProfile`), and one encoder each reads them:
`encode_tools` for the outbound definition list (its bytes are cached per
`(ToolFormat, fingerprint)` and a cache hit is byte-identical to the miss),
`encode_message` for a persisted message - a tool result lands in the profile's
carrier, keyed by the profile's id field - and `wire::decode::ToolCallAccumulator`
for streamed deltas, which attributes a fragment by the frame's content-block or
choice index (or by call id) rather than to whichever call opened last. A
provider whose stream supplies no id has one minted as
`<provider>_<counter>_<name>`, deterministic across the step, because a call id
is replayed in the next request and a fabricated one that changes shape between
requests breaks result pairing. Two calls carrying the same id in one response
are reconciled by occurrence order and left as two calls; neither is dropped.
Tool-use argument JSON is decoded by one shared helper,
`ragent_types::tool_args::decode_tool_arguments`, which accepts a JSON object, a
JSON string that decodes to one, and empty or whitespace text (as `{}`), is
called at most once per call per step, and is not called in the provider layer at
all. A decode failure produces one refusal value
(`session::tool_input_error::ToolInputError`) that owns its sentence, its record
status and therefore its `is_error` flag, so the same failure cannot be worded or
flagged two ways.

**One ordered dispatch pipeline serves both execution modes.** A step's tool
calls - sequential or concurrent - are prepared into one list of slots in the
provider's order (a future for a call that runs, a settled outcome for one that
is refused) and drained in a single ordered pass, so
`experimental.parallel_tool_calls` changes the concurrency of a step and never
the history it records. The stages are named once in `session::dispatch`:
decode arguments, validate schema, unknown-tool check, repeat guard, pre-tool
hooks, permission verdict, backend dispatch, result normalisation.

**An unknown name never executes.** A call whose name resolves to no registered
tool returns a corrective tool result naming what was asked for and the closest
registered name when one is within the shared similarity threshold - the one
ranking in `session::tool_suggestion` - and no tool runs. A call whose
*arguments* fail to decode or validate is refused the same way, before
dispatch, and the refusal is flagged as an error so the model can correct it.

**A tool result is never indistinguishable from a success.** The canonical
`ContentPart::ToolResult` carries `is_error`; providers with native support
transmit the flag, and a profile without it prefixes the text with `ERROR: `, so
a failed run and a refused call reach the model behind the same marker. A
result is appended only for a call whose permission verdict resolved, and a
provider that reports a tool-use finish while the step parsed no complete call is
treated as a truncation rather than a clean answer (FR-022).

### 3.2.2 Completion and Suggestion Surfaces (FR-033, spec `uniformtools`)

Every surface that offers a tool name is built from the tool **registry**, which is
the only catalogue of names: the slash-command and tool-name menus, the
did-you-mean candidate pool, and the lists printed by `--dry-run`, `tool_info` and
`commands_info`. A retired `before` spelling appears in none of them, and typing one
produces no completion entry - only the ordinary unknown-tool refusal with the
did-you-mean hint when a live name is close.

| Surface | Source |
|---------|--------|
| did-you-mean pool | `session::tool_suggestion::suggest_tool_name` over `ToolRegistry::list` (one implementation, also re-exported by `session::dispatch`) |
| `tool_info` | the `ToolContext::tool_registry` it was handed |
| `--dry-run` tools section | `ToolRegistry::list` + each tool's `permission_category()` |
| system-prompt tool reference | `ToolRegistry::definitions()` |
| slash-command subcommand menu | one table per command; `/tools` reads `command_catalog::TOOLS_SUBS` |
| `commands_info` | `command_catalog::COMMAND_CATALOG` (`/tools` publishes `TOOLS_SUBS`) |
| `@` file picker | project **paths** only - this field offers no tool name and FR-033 does not add one |

The `/tools` row is the one subcommand table with four readers: the autocomplete menu,
the parameter hint, the handler (its help page and every usage warning are generated
from it), and `commands_info`. Adding a `/tools` form means adding it to `TOOLS_SUBS`
and nowhere else.

### 3.2.3 Tool Surface Agreement (FR-030, spec `uniformtools`)

The registered tool surface is **149 tools in 21 permission families** in the default
build, and every enumeration of it is checked against the registry rather than
against a remembered number:

| Enumeration | What it reads | Guard |
|-------------|---------------|-------|
| `ToolRegistry::list`, `definitions`, `all_tools` | the registry keys and tool objects | `test_fr030_surface_agreement.rs` case 1 |
| the sorted name list and its digest | the FR-023 `after` spellings (13 uppercase) | case 2 |
| `--dry-run` `tools` section (human and `--json`) | `list()` + `permission_category()`, `total` then one row per family | cases 3 and 4 |
| `tool_info` | `ToolContext::tool_registry.all_tools()` | case 5 |
| `commands_info` | `command_catalog::COMMAND_CATALOG` (`/tools` publishes `TOOLS_SUBS`) | case 6 |
| outbound provider `ChatRequest.tools` | `definitions()`, via `get_cached_tool_definitions` | case 7 |
| `/tools list` | `definitions()` | TUI case 1 |
| `/bug-report` `Tools registered` line | `list().len()` | TUI case 2 |

`/tools list` reports every tool the registry holds - the same set `definitions()`
returns and the same set sent to the provider - so its row count is the tool count.
The family is the leading segment of `permission_category()`, never the tool name:
`team_wait` reports `agent:spawn`, so the split is `agent` 5 / `team` 20 and a
name-prefix reading gives the wrong 3 / 21 for the same 150 total. (`agent` grew
4 -> 5 when `list_agents` moved from `none` to `agent:read`, spec `agentssetup`
FR-005; the family count stayed 21 because the category was re-spelled inside an
existing family.)

### 3.3 Agent System

Agents are typed profiles that define what model, system prompt, tools, and
permissions a session uses.

#### Parallel Tool Dispatch Gate (FR-010, spec `uniformtools`)

Each provider family declares a tool-calling wire profile, and one of its fields
is `supports_parallel` - whether the provider can carry several tool calls in one
step. The agent loop reads that field once per step, before any call is prepared:
when the user's `experimental.parallel_tool_calls` is on **and** the active
provider's profile does not permit concurrency, the flag is demoted to `false`
for the step and one `tracing::info!` names the provider and the reason. Every
call of that step then runs through the sequential arm of the one ordered
dispatch pipeline, which since the pipeline consolidation is the same pipeline
with the same recorded order - so the gate changes the concurrency of a step and
never the history it produces.

Two boundaries are part of the contract. The gate is a **demotion only**: `true`
means concurrency is permitted, never switched on, and only the user's own opt-in
enables it, so a provider that supports parallel calls is untouched. And an id
with **no** declared profile is gated too (the condition is
`is_none_or(|p| !p.supports_parallel)`), because a profile table with no
catch-all means an unnamed provider declares nothing rather than inheriting
agreement. The `router` provider is gated by its own row and not by the
downstream provider its classification picks - that selection happens inside the
router's `chat`, after the decision.

#### Reasoning Echo (FR-011, spec `uniformtools`)

Anthropic's extended thinking attaches an opaque **signature** to every signed
`thinking` block and requires that block, signature included, to be replayed
unchanged on the turn that answers a tool call. The block is therefore stored as
one canonical part with two halves kept apart - the reasoning text and the
provider's signature over it - and a profile field decides whether it is
replayed:

- **Recording.** The streamed `ReasoningSignature` frame is kept in its own
  field beside the reasoning text (never appended to it, or it would be shown to
  the user and lost as a signature), and both halves are written to the saved
  `MessagePart::Reasoning`. A retry clears the signature with the text it signed,
  because a provider verifies the pair together and a fresh block carrying the
  previous attempt's signature is rejected rather than merely unverified.
- **Replaying within the turn.** The tool phase writes the reasoning part into
  the step's assistant message *above* its text, so it precedes both the text and
  the `tool_use` blocks of that turn. This is not the same path as a history
  rebuild: the request that answers the call is built from the in-turn message
  list, so a block that exists only in the persisted record would arrive one turn
  too late.
- **Omitting.** `echo_reasoning` is declared per family (`true` for Anthropic
  Messages, `false` for the other five). The encoder writes
  `{"type":"thinking","thinking":<text>}` plus `signature` - and writes no
  `signature` key at all for a block recorded without one, rather than a `null` -
  only where the profile declares it. Every other family omits the part, and that
  omission is a declared term of the contract rather than the silent drop it was
  before.

Neither history converter consults `echo_reasoning`: the omission is a property
of the wire contract, so deciding it anywhere but the encoder would re-implement
the profile table.

#### Unresolved Approvals Block the Result (FR-013, spec `uniformtools`)

A tool call's permission verdict is a state, not an event. While it is
unresolved - the interactive prompt is open and nobody has answered, a client has
not replied, a checkpoint timer has not fired - the call has no outcome, and the
model must not be handed one: a turn that continued on an unapproved call's
output would have acted without approval while reading as if it had been allowed.

That state is tracked rather than assumed. `session::pending::PendingVerdicts`
holds, per session, the call ids currently at a prompt. The dispatch pipeline's
permission stage marks a call immediately before it prompts and settles it the
moment the verdict resolves; a timeout is a denial, so it settles too, and an
`Ask` that somehow survives the layer is refused as an error rather than reported
as a success. Before the tool phase appends anything it asks the same tracker,
through `result_is_appendable`. On an unsettled verdict the call's
`MessagePart::ToolCall` record is still written - status `Pending`, no output, no
error, so the provider's `tool_use` is never orphaned - but no
`ContentPart::ToolResult` is appended and one warning names the call, because a
silently dropped result would hide a wedged verdict.

Three details are part of the contract. The tracker is keyed by **session and
call**, because call ids are not globally unique (a provider restarts its
`call_1` every response, and sessions run concurrently), so one session's verdict
can never unblock another's call. A **second mark needs no second settle**: the
bash path raises one prompt per sub-command under a single call id, and the
verdict is one verdict. And a **denial is a resolved verdict**, so a denial's
error observation is appended normally - FR-013 forbids appending an unresolved
call's output, not a refusal.

#### Built-in Agents

| Agent | Role |
|-------|------|
| `general` | General-purpose assistant |
| `coder` / `rust-coder` / `python-coder` / `go-coder` / `typescript-coder` | Code specialist |
| `architect` | High-level design |
| `ask` | Question-answering |
| `debug` | Debugging assistant |
| `code-review` | Review-focused |
| `orchestrator` | Multi-agent coordination |
| `plan` | Planning and decomposition |
| `explore` | Codebase exploration |
| `title` / `summary` | Short-lived helper agents |

#### Agent Features

- **Model binding** — Each agent can specify a `provider/model` or auto-resolve the first available model
- **Tool surface** — Every registered tool is advertised to the model: the
  request's `tools` array is `definitions()` and the system prompt lists the same
  set. A tool is absent only when the subsystem that registers it is inactive
  (the code index registers its ten tools only while enabled), so there is no
  per-family filter to configure.
- **Permissions** — Per-agent permission rules merge with global config
- **Custom prompts** — Markdown/OASF profiles in `~/.ragent/agents/` or `.ragent/agents/`

### 3.3.1 Git Platform Integrations

| Platform | Coverage |
|----------|----------|
| **GitHub** | Issues, pull requests, comments, reviews, pipelines |
| **GitLab** | Issues, merge requests, pipelines, jobs, projects |

Authentication uses environment tokens (`GITHUB_TOKEN`, `GITLAB_TOKEN`) or
configured credentials, with auto-detection from the local git remote.

### 3.5 Session & Tool Execution Flow

```mermaid
graph LR
    Input[User Input] --> SP[Session Processor]
    SP --> Agent[Resolve Agent + Model]
    SP --> LLM[LLM Call]
    LLM --> ToolCall[Tool Call Request]
    ToolCall --> Permission[Permission Check]
    Permission -->|ask| UI[User Prompt]
    UI --> Decision[Allow / Deny / Always]
    Decision --> Execute[Tool Execution]
    Execute --> Result[Tool Result]
    Result --> SP
    SP --> Output[Assistant Response]
```

**Figure 5:** Session & Tool Execution Flow — LLM call → permission → tool dispatch loop

### 3.6 Provider Selection Flow

```mermaid
graph TD
    Agent[Agent Profile] --> ModelRef[ModelRef or None]
    ModelRef -->|explicit| Registry[Provider Registry]
    ModelRef -->|None| DefaultResolver[Resolve first available model]
    CLI[--model override] --> Registry
    Config[default_provider] --> Registry
    Registry --> Health[Health Check]
    Health -->|healthy| Use[Use Provider]
    Health -->|unhealthy| Fallback[Fallback / Error]
```

**Figure 6:** Provider Selection Flow — Multi-provider routing and health checks

### 3.7 Agent Execution Loop Phases

```mermaid
graph LR
    A[Start Turn] --> B[Build Context]
          B --> C[Compact if Over Threshold]    C --> D[Send to LLM]
    D --> E[Stream Tokens / Tool Calls]
    E --> F[Execute Tools]
    F --> G[Record Results]
    G --> H[End Turn]
    H -->|next turn| A
```

**Figure 7:** Agent Execution Loop Phases — One complete turn from input to response

#### Phase Descriptions

| Phase | Responsibility |
|-------|----------------|
| Build Context | Assemble system prompt, memories, code index, conversation history |
| Compact | Summarise conversation history when estimated request tokens exceed the configured percentage of the model's context window (`threshold`, default 70%) or, when `threshold` is null, `context_window - max(output_tokens, compaction.buffer)` (FR-003); emergency compaction on provider context-overflow errors (FR-004) |
| LLM Call | Send request to selected provider |
| Stream | Emit assistant chunks and tool call events |
| Execute Tools | Check permissions, run tools, record results |
| Record | Append assistant message + tool results to SQLite |

Between steps the loop also runs two gates:

- **Sub-agent suspension gate (spec `agentui` FR-035)** — a child session
  parked by `AgentManager::suspend_task` waits here (a 200 ms poll that
  breaks on the cancel flag) until `resume_task` clears its flag or the turn
  is cancelled; sessions not spawned by the manager skip the gate. The
  suspension is cooperative: the LLM call or tool execution in flight when
  the request lands finishes first. A separate hard watchdog kills any
  single tool call and terminates the run when it stalls past
  `TOOL_WATCHDOG_TIMEOUT` (5000 s; see `session/processor.rs`).
- **Text-format tool-call recovery** — when the step produced no native tool
  calls, the loop (in `session/loop_steps.rs`, before callers save the
  buffers) extracts pending calls from the text buffer first and then the
  reasoning buffer, blanking any recovered spans so the narration is not
  re-sent. The recogniser accepts the Qwen private-tag JSON dialect, the XML
  `function=` dialect, the plain `<toolcall>` pair (including a slash-dropped
  close-tag variant that produces two identical tags), the underscore-spelled
  `tool_call` pair, and a bare tool-call JSON object/array; ordinary prose
  never matches, and recovered calls flow through the normal dispatch
  pipeline (loop restrictions, hooks, permission checks, registry).

### 3.8 Autonomous Loop Tools

| Tool | Use |
|------|-----|
| `new_agent` | Spawn a background sub-agent (optional `detached: true` for a fire-and-forget task excluded from `list_agents`/`wait_agents` whose result is not injected back) |
| `cancel_agent` | Request cancellation of a running sub-agent; reports `cancel_requested` plus the task's current status, never a terminal state it has not reached |
| `list_agents` | Inspect active sub-agents (reports `limit`/`occupancy`/`headroom`, the `waiter_count` of a task in-flight `wait_agents` callers hold - shown only when greater than zero - and its `status` filter accepts every declared status: `running`, `suspended`, `completed`, `failed`, `cancelled`, `timed_out`) |
| `wait_agents` | Block until sub-agents finish |
| `agent_complete` | **Terminal signal** that ends the autonomous loop |

`agent_complete` takes **only** `summary`. `team_task_complete` takes `team_name` + `task_id`.

**`/cancel <prefix>` delegates to the same registry.** The slash command is not a
separate cancellation path: it resolves the prefix against the Agents window's
`active_tasks` projection, then routes a single match to
`AgentManager::cancel_agent` - the method behind `cancel_agent` - and reports the
*request*, never an outcome it has not seen (FR-023). It does not remove the row:
the entry stays `running` until the agent loop observes the cooperative cancel flag
at its next step boundary, and the row is retired by the `SubagentCancelled` event
or the registry reconciliation, so the window cannot show a cancellation that did
not happen. A prefix matching more than one live task is refused with every match
listed as `<short id> (<agent>, <status>)` (FR-024); a prefix matching none reports
`No task matching '<prefix>'` and lists the live ids, or says plainly that there are
none (FR-041). A prefix matching one *finished* task reports
`Task <short id> is already <status>` rather than a cancellation. Only `running`,
`suspended` and `terminating` count as live; `completed`, `failed`, `cancelled` and
`timed_out` are never offered as cancel targets. The benchmark-first dispatch
(`/cancel <prefix>` against an active `/bench` run) is unchanged and still takes
precedence over sub-agent tasks. Covered by
`test_cancel_slash::cancel_requests_through_the_registry_and_keeps_the_row`,
`::cancel_refuses_an_ambiguous_prefix_and_lists_the_matches`,
`::cancel_names_the_live_ids_when_no_task_matches` and
`::cancel_reports_the_registry_status_of_a_finished_task`.

### 3.9 Failure Capture: Panic Reports, Crash Dumps, and the stderr Spool

Three complementary mechanisms cover the three ways a ragent process can die.

| Failure | Captured by | Where the evidence lands |
|---------|-------------|--------------------------|
| Unwinding panic (in-process) | `panic_hook::install` — writes the payload, location, environment, and a forced backtrace, then chains to the default hook | `log/panics/panic-<timestamp>.log` |
| Abort — stack overflow, `SIGKILL`, `std::process::abort` | OS core dump (no in-process hook can run) | `coredumpctl` entry / `core_pattern` file; detection marker at `log/panics/last-crash.json` |
| Raw stderr while the TUI owns the alternate screen | `stderr_spool::install` — fd-2 redirection drained into a truncating spool | `log/logwindow/stderr-<timestamp>.log` plus the log panel |

**Stack overflows cannot be trapped in-process.** The Rust runtime prints
`thread '...' has overflowed its stack` and calls `abort()`; nothing after that
point runs. ragent therefore does not try. Instead, `src/crash_dump.rs` stamps
`log/panics/last-crash.json` at startup (pid, exe, args, cwd, thread,
`status: running`, and a platform-specific core-dump retrieval hint), overwrites
it with `clean exit` on a normal return, and — when the next start finds a
still-`running` record whose pid is gone — prints a warning naming the marker,
the dead pid, and the host's core-dump command. The hint is `coredumpctl
list/info/debug <pid>` when `/proc/sys/kernel/core_pattern` pipes into a helper
(typical `systemd-coredump` hosts), otherwise the `core_pattern` file path or
the macOS `/cores` convention. A clean exit never reports.

**The stderr spool** exists because the TUI paints the alternate screen: the
default panic hook, `eprintln!`, and C-library diagnostics would be overwritten
by the next frame. In TUI mode the binary replaces fd 2 with the write end of a
pipe and drains it on a dedicated thread into
`ragent_types::stderr_spool::Spool` (`SPOOL_MAX_LINES = 1000`); the TUI mirrors
each chunk into the log panel as `stderr: <line>`. The spool counts newlines per
write and, once the 1000-line cap is exceeded, rewrites the file with only its
newest 1000 lines — the rewrite is skipped entirely for a write containing no
newline, so the common case costs one append. Non-TUI modes (`--no-tui`,
headless `run`, `serve`) leave stderr attached to the terminal.

---

## 4. Security & Permissions

### 4.1 Permission Security Layers

```mermaid
graph TD
    ToolCall[Tool Call] --> Hardwired[Hardwired Rules]
    Hardwired -->|codeindex / agent_complete / list_agents| Allow[Always Allow]
    Hardwired -->|others| Rules[Configured Rules]
    Rules --> Match[Last Match Wins]
    Match -->|allow| Allow
    Match -->|deny| Deny
    Match -->|ask / no match| Prompt[User Prompt]
    Prompt -->|always / once| Allow
    Prompt -->|deny| Deny
    Prompt -->|timeout| Deny
```

**Figure 10:** Permission Security Layers — 5-layer defense-in-depth

### 4.2 Bash Security — 7 Layers

```mermaid
graph TD
    Cmd[Command String] --> L1[Layer 1: Safe Command Whitelist]
    L1 -->|safe| Auto[Auto-approve]
    L1 -->|not safe| L2[Layer 2: Banned Commands]
    L2 -->|banned| Reject
    L2 -->|ok| L3[Layer 3: Denied Patterns]
    L3 -->|denied| Reject
    L3 -->|ok| L4[Layer 4: Directory Escape Check]
    L4 -->|escape| Reject
    L4 -->|ok| L5[Layer 5: Syntax Validation]
    L5 -->|invalid| Reject
    L5 -->|ok| L6[Layer 6: Obfuscation Detection]
    L6 -->|obfuscated| Reject
    L6 -->|ok| L7[Layer 7: User Allow/Deny Lists]
    L7 -->|deny| Reject
    L7 -->|allow/ok| Permission[Permission Check]
```

**Figure 11:** Bash Security — 7 Layers — Bash command defense flow

### 4.3 Permission Request Flow

```mermaid
sequenceDiagram
    participant Tool as Tool Executor
    participant SP as Session Processor
    participant Bus as Event Bus
    participant UI as TUI / HTTP API
    participant User as User
    Tool->>SP: request permission
    SP->>Bus: PermissionRequested
    Bus->>UI: render dialog
    User->>UI: decision (allow/deny/always)
    UI->>Bus: PermissionReplied
    Bus->>SP: resume execution
```

**Figure 12:** Permission Request Flow — From tool call to user decision

Permission requests include:

| Field | Description |
|-------|-------------|
| `id` | Unique request ID |
| `permission` | Permission type (`file:write`, `bash`, etc.) |
| `action` | Requested operation description |
| `path` | Target path (for file operations) |
| `command` | Command string (for `Bash`) |
| `created_at` | Unix timestamp |
| `timeout_secs` | Timeout before auto-deny (default 120 s) |

### 4.4 Permission Rules Evaluation

Beyond the rule evaluation below, a **tool-repeat guard** (FR-044) sits on
every dispatched call: five consecutive identical calls (same tool + same
argument hash) pass through untouched; the sixth raises a `tool:repeat`
permission prompt in interactive runs and is auto-denied with a corrective
observation in unattended runs (subagent, `--yes`, YOLO). A different call
or a user "allow" resets the consecutive counter.

```mermaid
graph LR
    Rules[Rules List] --> Iterate[Evaluate in order]
    Iterate --> Match{Glob Match?}
    Match -->|yes| Record[Record Match]
    Match -->|no| Next[Next Rule]
    Record --> Next
    Next --> More{More Rules?}
    More -->|yes| Iterate
    More -->|no| Last[Last Match Wins]
    Last --> Action[Allow / Deny / Ask]
```

**Figure 13:** Permission Rules Evaluation — Rule matching and resolution

Rules are evaluated top-to-bottom; the last matching rule determines the action.
Built-in default rules include:

| Default | Action |
|---------|--------|
| `read` | allow |
| `Edit`, `Bash`, `web`, `EnterPlanMode` | ask |
| `todo` | allow |

### 4.5 File Path Guards

- File-write tools must target paths inside the working directory unless the user
  explicitly allows escaping.
- Path normalization resolves `..` and symlinks before permission checks.
- The `dirs` config block can whitelist additional writable roots via `allowlist`/`denylist` glob patterns.
- `dirs.allowed_roots` declares *additional* directory roots that are valid for path escape checking; the session's `working_dir` is always a valid root, and files under it or under any listed root are reachable by the `read`/`Write`/`list`/`Grep` tools (FR-017). The `allowed_roots` list extends the working directory; it never replaces it.

### 4.6 Secret Redaction

Environment variables matching `*KEY*`, `*SECRET*`, `*TOKEN*`, `*PASSWORD*`
are redacted from tool outputs and logs.

### 4.6a Critical-Hardening Guards (SECTASKS MS-01)

Five path/argument-injection guards close the Critical findings in
`SECTASKS.md`. Each is a named helper (not an inline check) so later
Milestone MS-05 work can migrate stragglers onto it.

| Guard | Where it lives | What it rejects |
|-------|----------------|-----------------|
| `validate_team_name` | `ragent-agent::team::store` | anything outside `^[a-z0-9][a-z0-9-]{0,63}$`; called by `TeamStore::create`, `find_team_dir`, `find_team_dir_cached`, `team_cleanup`, and the `/team create`/`/team delete` slash commands |
| Team-name slugify (`slugify_team_name`) | `ragent-agent::team::store` | blueprint/timestamp-derived names are slugified before use so generation can never produce an invalid name |
| `is_safe_git_argument` | `ragent-plugins::add` | a `git+` ref or subpath that is not `[A-Za-z0-9._/-]+`, begins with `-`, or contains `..`; `git_run` also passes `--` before every positional |
| `sanitize_declared_id` / `confined_dest_dir` | `ragent-plugins::manifest` / `add` | a manifest `id` that is not a single `Component::Normal` matching `[A-Za-z0-9._-]+` (no separators, no `..`, not absolute) |
| `check_path_within_allowed_roots_cached` | `ragent-tools-core` | a `multi_edit` target outside the workspace — the same containment check `Edit`/`read`/`Write` already applied |

Two further Critical classes are handled in place rather than by a helper:

- **Blueprint seed execution** — every tool named by `.ragent/blueprints/teams/<bp>/task-seed.json`
  or `spawn-prompts.json` is dispatched through `dispatch_seed_tool`, which consults
  the session permission checker: a category that is not explicitly `Allow` (or
  `Ask`-with-no-checker) refuses the seed and reports why, so a repository-supplied
  blueprint can no longer reach `Bash`/`Write`/`rm` unpermissioned.
- **`multi_edit` containment** — see the table row above; the check runs for every
  edit in the batch before any file is read or written, so a mixed batch is atomic
  and writes nothing.

### 4.6b Host & Sandbox Hardening (SECTASKS MS-02)

The High-severity findings. Each is a named helper or an explicit bound so the
later milestones can migrate remaining call sites onto the same shapes.

| Guard | Where it lives | What it enforces |
|-------|----------------|------------------|
| `hook_approved` on `PreToolUseResult::Allow` | `ragent-agent::hooks` / `session::processor` | a `pre_tool_use` hook approval is distinguishable from "no hook"; it may satisfy an `Ask` but never an explicit `Deny`. Plugin-contributed `pre_tool_use` hooks are dropped by `merge_hook_configs` |
| `unpack_archive_checked` | `ragent-agent::session::archive` | session-archive import rejects escaping entry paths and caps entries (10 000) and total decompressed bytes (512 MiB); `manifest.session_directory` is accepted only inside the working directory |
| `exec_guard` (`is_allowed_fixture_program`) | `ragent-bench` | a benchmark fixture may only spawn an allowlisted toolchain program (bare name, no path, no shell) |
| `index_file` containment | `ragent-codeindex` | the watcher-driven path canonicalises and refuses anything outside the project root, refuses symlinks, and enforces `max_file_size` |
| Project-config privilege stripping | `ragent-config` | the cwd `.ragent/ragent.json` may not enable YOLO, remove a global deny rule, or widen `allowed_roots`/bash allow-lists |
| `x-goog-api-key` header | `ragent-llm::providers::gemini` | the Gemini key is sent as a header, registered with the secret registry, and never interpolated into a logged URL |
| `redirect::Policy::none()` | `ragent-llm::providers` | authenticated LLM requests do not follow redirects, so a non-standard auth header cannot be replayed cross-host |
| `ResearchIo::template_path` -> `Option` | `ragent-research::io` | the `--template` name must be a single `[A-Za-z0-9_-]` component; an invalid name yields no template plus a warning |
| `ConfigResponse` credential masking | `ragent-server::routes` | `GET /config` redacts every credential field before serialising |
| `check_path_within_root` on `from_files` | `ragent-research` / `ragent-server` | research file seeds are canonicalised and rejected outside the project root |
| ChaCha20-Poly1305 `v3:` credential store | `ragent-storage` | a per-install 0600 key file, an authenticated construction with a fresh nonce, and a hard error on a corrupt row (no empty-string downgrade) |
| `sender_session_id` on `MailboxMessage` | `ragent-agent::team::mailbox` | the authenticated sender session id is recorded; a message claiming `from: "lead"` without one is rejected at the write boundary |
| `tls-roots` OTLP transport | `ragent-telemetry` | the gRPC exporter is compiled with TLS, and a non-loopback `https://` endpoint is refused when TLS is unavailable |
| Telemetry privilege stripping | `ragent-config` | telemetry enablement and endpoint overrides come from the user-global config only |
| OTLP export health monitor | `ragent-telemetry` | every export outcome is classified and counted; 5 consecutive failures disable export instead of retrying a dead endpoint, and each failure surfaces as an `Agent Notice` with the masked endpoint |
| Async-runtime periodic reader | `ragent-telemetry::subsystem::build_provider` | the OTLP `reqwest-client` exporter must be driven by `periodic_reader_with_async_runtime` on the Tokio runtime; the OS-thread reader panics with "there is no reactor running" on the first export |
| Thread-aware TUI panic hook | `ragent-tui::run_tui` | terminal teardown runs only when the panicking thread is the UI thread, so a background-worker panic cannot destroy the alternate screen |
| Safe-command allowlist + in-crate calculator | `ragent-tools-core` | the allowlist is read-only commands only, interpreters always go through the permission gate, banned tools are consulted first, and `calculator` evaluates arithmetic in-process (no `python3 -c`) |
| VCS argument hardening | `ragent-tools-vcs` | `reject_option_like` on every ref/branch/tag/remote/path, `--` before positionals, `validate_clone_directory` confines the clone target, and a `gitlab:<host>` host is accepted only when it matches the configured instance |

### 4.6c Network & Secret Hardening (SECTASKS MS-03)

The Medium-severity findings. The recurring shapes are a **byte cap**, a
**page/entry budget**, a **clamped delay**, and **origin-scoped credential
attachment**.

| Guard | Where it lives | What it enforces |
|-------|----------------|------------------|
| `read_body_capped` / MCP body cap | `ragent-agent::mcp::http` | MCP HTTP responses are read under a fixed byte cap |
| Benchmark download caps | `ragent-bench::data` | client timeout, streamed byte cap, pagination page/record budget, and `contained_join` for manifest `case_file`/`relative_path` |
| `MAX_RETRY_AFTER` / `MAX_SSE_BUFFER_BYTES` / `MAX_ERROR_BODY_BYTES` | `ragent-llm::providers::http_client` | `Retry-After` is clamped to 30 s, the SSE accumulation buffer aborts the stream past 1 MiB, and error bodies are read under 64 KiB |
| `MAX_TREE_REQUESTS` / `MAX_TREE_ENTRIES` / `MAX_JOB_PAGES` | `ragent-tools-vcs` | the GitHub tree walk and the GitLab jobs pagination are bounded |
| `read_response_capped` / `MAX_FETCH_BODY_BYTES` / `MAX_CRAWL_PAGE_BYTES` | `ragent-tools-extended::masterfetch` | responses are streamed under a byte budget; `crawl_urls` is SSRF-validated at the tool boundary |
| Credential env seeding | binary startup | every credential env var is registered with the secret registry before any log can be written |
| `redact_secrets` on tool output | `ragent-tools-core::bash`, `ragent-tools-vcs::github_actions` | bash partial/completed output and CI log excerpts are redacted before they reach the model or the session store |
| `is_denied_request_header` | `ragent-tools-extended::http_request` | routing/credential headers (`Host`, `Cookie`, `Authorization`, `Proxy-*`, ...) are rejected |
| `mask_secret` | `ragent-tui::layout` | the provider-setup dialog masks the API key and the GitLab PAT |
| `check_path_within_workspace` | `ragent-tools-extended::ToolContext` | `pdf_write` confines its output path |
| `build_exclude_matcher` / `clear_index_dir` | `ragent-codeindex` | the configured exclusion globs are applied and the FTS recovery wipe refuses a symlinked directory |
| Scratch-dir confinement (`bash_scratch_dir`, `restrict_to_owner`) | `ragent-tools-core::bash` | the wrapper script and `export -p` state file live in a 0700 directory as 0600 files |
| `MAX_CANDIDATES_PER_REF` / `MAX_EDGES_PER_PASS` | `ragent-codeindex::graph::edges` | edge derivation carries a per-reference fan-out cap and a global edge budget |
| `MAX_GAP_EXPANSION` / `MAX_CONTRADICTION_REQUIREMENTS` | `ragent-specs::validate` | numbering-gap expansion and contradiction detection are bounded |
| `MAX_MESSAGE_BYTES` / `validate_hook_command` / `HOOK_TIMEOUT` | `ragent-agent::team` | mailbox messages are size-capped, team hook commands are validated, hooks time out at 30 s with a process-group kill, and feedback is capped |
| `positive_duration_secs` / checked arithmetic | `ragent-types::cron` | `CronSchedule` returns `None` instead of panicking on an invariant violation and uses `checked_mul` / `saturating_mul` |
| `SPOOL_MAX_BYTES` | `ragent-types::stderr_spool` | the spool enforces a byte ceiling on every write, including newline-free ones |
| `split_bash_command` substitution depth | `ragent-agent::session::permissions` | a `;`/`&&` inside `$( ... )` does not split the command into a separately-judged sub-command |
| `same_origin_as` | `ragent-tools-vcs::github` | the Bearer token is attached only when the resolved URL matches the configured API origin |

### 4.6d Defence in Depth (SECTASKS MS-04)

The Low-severity findings, delivered as the same guard shapes at the remaining
call sites: an explicit **cap/budget**, a **containment check**, a **char-safe
split**, or an **owner-only file mode**.

| Guard | Where it lives | What it enforces |
|-------|----------------|------------------|
| `unpack_archive_checked` (`ARCHIVE_MAX_ENTRIES`, `ARCHIVE_MAX_TOTAL_BYTES`) | `ragent-agent::session::archive` | session-archive import refuses an entry-count or total-decompressed-size overrun and rejects an entry path that escapes the extraction directory |
| `MAX_BENCH_SAMPLES` | `ragent-bench::model` | `--samples` is clamped to 100 before both the `Vec` pre-allocation and the generation loop |
| `MAX_DOWNLOAD_BYTES` / `MAX_HUMANEVALPACK_PAGES` / `MAX_HUMANEVALPACK_ROWS` | `ragent-bench::data` | a dataset download is streamed under a 64 MiB cap and the pagination loop carries a page and row budget |
| `base_url_override` run note | `ragent-bench::runner` | the resolved provider base URL is no longer written into the shareable workbook or run-state sidecar |
| `contained_join` | `ragent-bench::data` | manifest `case_file` and `relative_path` are joined inside the canonicalised data root |
| read-then-check size | `ragent-codeindex::scanner` | the stored byte size is derived from the bytes actually read, closing the stat/read TOCTOU past `max_file_size` |
| `build_exclude_matcher` | `ragent-codeindex::scanner` | `ScanConfig::extra_exclude_patterns` is compiled into a glob matcher and applied during the walk |
| `clear_index_dir` symlink refusal | `ragent-codeindex::search` | `FtsIndex::open_or_create` refuses a symlinked index directory before the recovery wipe and surfaces deletion failures |
| `BUILTIN_DENYLIST` merge | `ragent-config::dir_lists` | the built-in system-directory patterns are merged into the enforced denylist, not merely advertised |
| parse-diagnostic redaction | `ragent-config::config`, `src/main.rs` | the echoed problematic source line is passed through `redact_secrets` before it is printed or logged |
| `is_contained_relative_path` | `ragent-plugins::manifest` | `entry` / `main` / `server.entry` must be a contained relative path inside the plugin root |
| `https_only_redirects` (shared) | `ragent-plugins::store_fetch` → `add` | the package download stops any non-`https` redirect |
| `is_single_normal_component` | `ragent-plugins::marketplace` | the wrapper-directory name derived from the install source must be a single normal component |
| `neutralise_fenced_body` | `ragent-research::document` | backtick runs are broken and `#### Source [#N]` / `**Sources:**` lines escaped before a fetched body enters a fenced synthesis prompt |
| `mask_url_credentials` | `ragent-research::document` → `gather_log` | recorded gather-log URLs drop userinfo and query strings |
| session-bound permission reply | `ragent-agent::session::permissions` | an `Event::PermissionReplied` is honoured only when its `session_id` matches the session awaiting the request |
| `internal_error_response` | `ragent-server::routes` | internal error detail is logged; the client receives `internal server error` |
| `SpecId` validating `Deserialize` | `ragent-specs::spec` | every deserialisation routes through `SpecId::new`, so a traversal id cannot be constructed |
| `write_govcreate_spec` re-validation | `ragent-specs::commands` | the public write API validates the id before joining it into the output path |
| `/spec create` / `/spec specify` name validation | `ragent-specs::commands` | an invalid spec name is a usage error |
| char-boundary diff split | `ragent-storage::snapshot` | `apply_unified_diff` splits on the first char; a multibyte diff line no longer panics |
| `is_safe_memory_component` | `ragent-agent::team::config` | `resolve_memory_dir` rejects a multi-component or `..` agent name |
| `CardinalityCache::resolve` sanitisation | `ragent-telemetry::cardinality` | values are passed through `sanitize_attr_value` inside `resolve` and a poisoned lock fails closed to the unknown bucket |
| `parse_alog_run_id_yes` validation | `ragent-tui::app::helpers` | `/alog export` accepts only `[A-Za-z0-9_-]+` run ids |
| log-window spool mode | `ragent-tui::app::session_ops` | the spool is created 0600 and the mode is re-asserted on every open |
| `bash_scratch_dir` private fallback | `ragent-tools-core::bash` | a failed private-dir creation falls back to a process-private 0700 directory, never to shared temp |
| tree-walk budget | `ragent-tools-vcs::github` | the GitHub recursive tree walk carries a `MAX_TREE_REQUESTS`/`MAX_TREE_ENTRIES` budget, matching the GitLab equivalent |
| `redacted_event_debug` / SSE device-code masking | `ragent-server::sse` | credential-bearing events expose presence-only fields on the SSE stream and a redacted rendering for log sites |
| `CrawlFetcher` SSRF contract | `ragent-tools-extended::masterfetch::crawl` | the SSRF obligation is stated on the trait and validated at the `mf_crawl` tool boundary |

### 4.6e Shared Guards and Recurrence Prevention (SECTASKS MS-05)

Every mitigation from 4.6a-4.6d is one of a small number of guard *shapes*. MS-05
gives each shape a single implementation so a new call site cannot silently
weaken it, and adds CI gates that fail a build which reintroduces a class.

**`ragent_types::guard`** (re-exported from the crate root, and as
`ragent_tools_core::guard` for the tools crates) owns:

| Helper | Enforces |
|--------|----------|
| `reject_option_like(value, label)` | non-empty and no leading `-`, so a `git` operand cannot be parsed as an option |
| `is_safe_operand(value)` | the predicate form: `[A-Za-z0-9._/-]+` and no leading `-` |
| `validate_identifier(value, label)` | `^[A-Za-z0-9._-]+$`, at most `MAX_IDENTIFIER_LEN` (64) bytes, no `.`/`..`/separators |
| `validate_relative_component(value, label)` | relative, no `\`, no `..`, no Windows drive or UNC prefix |
| `contained_join(root, relative, label)` | the above plus a canonicalised-root check that catches a symlink escape; it canonicalises the **deepest existing ancestor** of the joined path, so a not-yet-created leaf can no longer escape through a symlinked intermediate component (SEC-059) |
| `reject_symlink_components(path, label)` | no component of `path` (walking upward) is a symlink - refuses to write or delete through one (SEC-006/SEC-030/SEC-059) |
| `reject_symlink_escape(path, root, label)` | `path` is not a symlink and canonicalises inside `root` (SEC-006) |
| `open_owner_only(path)` | creates/truncates `path` owner-only (`0o600`), so a secret-bearing temp file is not world-readable (SEC-022/SEC-027) |
| `clamp_retry_after(hint)` | the 1 s floor and `MAX_RETRY_AFTER` (30 s) ceiling on a server-supplied delay |
| `cap_read(bytes, max)` | a UTF-8-boundary-safe prefix of a buffered read |

Call-site-preserving adapters delegate to it:
`ragent-tools-vcs::git::reject_option_like`,
`ragent-plugins::add::is_safe_git_argument`, and
`ragent-bench::data::contained_join`. Regression suite:
`crates/ragent-types/tests/test_shared_guards.rs`.

**One redaction implementation.** `ragent-agent`, `ragent-storage`, and
`ragent-tools-core` re-export `ragent_types::sanitize` instead of holding a
second secret registry, so a credential registered anywhere is masked
everywhere: `GET /config`, telemetry attributes, log lines, SSE payloads, and
tool output all call the same `redact_secrets`.

`Event` no longer derives `Debug`. Its hand-written impl renders through an
internal `DebugProxy` that reports credential-bearing fields
(`CopilotDeviceFlowComplete.token`, `CopilotDeviceFlowStartResult.device_code`)
as presence flags and then pipes the whole rendering through `redact_secrets`,
so `tracing::debug!("{event:?}")` cannot print an OAuth credential. Regression
suite: `crates/ragent-types/tests/test_unified_redaction.rs`.

**CI gates** (job `security-guards` in `.github/workflows/ci.yml`, and
`pre-flight.sh`). Each gate has a `--self-test` that seeds a violation and
asserts the gate fails it:

| Gate | Command | Fails when |
|------|---------|-----------|
| Redaction tests | `cargo test -p ragent-types --test all test_unified_redaction::` | a second registry or a leaking `Event` `Debug` appears |
| File-tool containment | `scripts/check-file-tool-containment.sh` | a registered file tool no longer calls the `allowed_roots`-aware `check_path_within_allowed_roots_cached` helper (a working-dir-only `check_path_within_root_cached` call no longer passes; see 4.6f) |
| Panic-free production paths | `scripts/check-security-unwraps.sh` | a file exceeds its `.unwrap()`/`.expect()` baseline in `scripts/security-unwrap-baseline.txt` |
| One guard per rule | `scripts/check-shared-guards.sh` | a crate re-defines a shared guard or a second secret registry |
| Git-argument guard | `scripts/check-vcs-duplication.sh` | the leading-dash check is re-derived or the shared delegation is dropped |

The panic-free gate is a **baseline** gate: the 362 pre-existing panicking calls
across 90 files are recorded per file and grandfathered, so the gate is
enforceable today while still rejecting every *new* occurrence. An intentional
call is exempted with a same-line `// no-panic-ok: <reason>` comment, which makes
it visible in review. The grandfathered set is an accepted-risk entry in
`SECTASKS.md`.

### 4.6f M0 Critical Defects and Security Holes (ANTIPAT M0)

The first milestone of the workspace-wide anti-pattern remediation plan
(`ANTIPAT.md`) closes the two shipping defects and the highest-severity
containment gaps found by the 18-crate audit.

**Two shipping defects.**

- **Crash-marker ordering.** `async_main` now reads
  `crash_dump::previous_unclean_exit` *before* stamping the new `running`
  marker. Previously the marker was overwritten first, so its pid always
  matched the live process and the "previous session exited without unwinding"
  warning was unreachable (ANTIPAT A-01).
- **Search-retry shift overflow.** `WebGatherer::with_search_max_retries` clamps
  its argument to `MAX_SEARCH_RETRIES` (10) and the backoff uses
  `checked_shl` capped by `MAX_SEARCH_RETRY_DELAY_MS` (60 s). A CLI-supplied
  `--search-max-retries >= 64` previously panicked on `1u64 << 64`, and values
  in the 40-63 range slept for days (ANTIPAT F-02).

**Containment and argument-injection guards.**

| Guard | Where it lives | What it rejects |
|-------|----------------|-----------------|
| `git::reject_option_like` on every operand | `ragent-tools-vcs::git` | `git_checkout` branch/source, every `git_cherry_pick` commit, every `git_add` path, and `git_remote` name/url — all 18 git tools now reject a leading-dash operand |
| `GitHubClient::validate_repo_segment` | `ragent-tools-vcs::github` | an `owner`/`repo` that is not `[A-Za-z0-9._~-]+`, so it cannot escape the request path |
| `resolve_url` for `fetch_readme` | `ragent-tools-vcs::github` | a hardcoded `api.github.com`, which previously ignored a configured GitHub Enterprise `base_url` and sent the Bearer token to the public origin |
| `check_path_within_allowed_roots_cached` in every file tool | `ragent-tools-core` | `diff_files`, `file_info`, `Glob`, `open`, and `apply_patch` previously checked `working_dir` only, so a whitelisted `allowed_roots` entry was rejected for the same path every other tool accepted (FUNC-068, ANTIPAT F-06) |
| `masterfetch::http::read_body_capped` / `read_bytes_capped` | `ragent-tools-extended` | an unbounded response body from search/robots/youtube reads (16 MiB ceiling; 512 KiB for small text bodies) |
| `write_govcreate_spec` target-folder containment | `ragent-specs::commands` | a `target-folder` that climbs out of the invoking root, checked lexically *before* any directory is created (ANTIPAT H-1) |
| `merged_dir_lists_for` / `builtin_only_dir_lists` | `ragent-config::dir_lists` | a config-load failure that previously returned an empty denylist, silently dropping the mandatory built-in system-directory protection (ANTIPAT H-3) |
| plugin `content_digest` | `ragent-plugins::add` | an install with no integrity record: a SHA-256 over the staged tree is recorded in the store ledger, so a substituted archive or checkout is detectable |
| aggregate archive cap (`MAX_EXTRACTED_BYTES`) | `ragent-plugins::add` | a decompression bomb made of many just-under-the-per-entry-cap entries |
| symlink refusal in `copy_dir_recursive` | `ragent-plugins::add` | a symlink inside a source directory, previously dereferenced and copied into the store |
| `read_body_capped` on every provider error path | `ragent-llm` | an unbounded 4xx/5xx error body (16 provider sites); malformed SSE frames in five parsers are now logged rather than silently dropped (FUNC-032 parity) |

**Redaction and file permissions.**

- `Config` no longer derives `Debug`; a hand-written impl renders a JSON form
  routed through `ragent_types::sanitize::redact_secrets`, so an API key,
  GitLab token, bot token, webhook URL, or client secret cannot be printed
  (ANTIPAT H-2).
- Crash markers, panic reports, and the stderr spool are created owner-only
  (`0o600`), and the `/bug-report` dump is written `0o600` too. The crash
  record's argv is redacted before serialisation, because
  `ragent auth openai sk-...` would otherwise persist a live credential to a
  world-readable file (ANTIPAT A-02/A-03, MEDIUM-3).

**Silent-failure fixes.**

- Storage: the memory access-count bump logs its failure; the four
  `conversation_stats` counts propagate instead of `unwrap_or(0)`; a legacy
  nonce that cannot be converted now errors instead of decrypting under an
  all-zero key (ANTIPAT D-1/D-2/D-4).
- Server: ~23 HTTP 500 responses route through `internal_error_response`, so a
  SQLite or filesystem error string is no longer returned to a caller; the
  `store_memory` fetch-after-write propagates its failure instead of answering
  `201` with an empty body (ANTIPAT F-H3/F-H2).
- Agent: a sub-agent report that cannot be persisted to
  `log/subagents/<task-id>.md` now warns with the task id and target directory,
  instead of silently reporting `output_file: None`.
- Telemetry: `ToolRecorder`, `SessionRecorder`, `CoordinatorRecorder`, and
  `CompressionRecorder` honour the FR-027 per-metric toggles, and
  `InstrumentRegistry::noop()` builds from an explicit locally-owned provider
  rather than the process-global one, so a disabled subsystem cannot export
  (ANTIPAT HIGH-4/HIGH-5).

**Regression suites.** `crates/ragent-research/tests/test_search_retry_clamp.rs`,
`crates/ragent-tools-vcs/tests/test_ms0_git_arg_guards.rs`,
`crates/ragent-config/tests/test_ms0_config_guards.rs`,
`crates/ragent-plugins/tests/test_add.rs`,
`crates/ragent-specs/tests/test_govcreate_authoring.rs`,
`crates/ragent-telemetry/tests/test_metric_toggles.rs` (all wired into the
`ms0-regression` CI job).

### 4.6g Standards Conformance: Tests, Docs, Unwraps (ANTIPAT M2)

The second milestone of `ANTIPAT.md` closes the test-location, module-docblock,
and production-`unwrap` debt found by the 18-crate audit.

**Inline-test gate (M2.17).** `scripts/check-inline-tests.sh` is now a wrapper
over `scripts/check_inline_tests.py`, which distinguishes the two `#[cfg(test)]`
shapes:

| Shape | Verdict |
|-------|---------|
| `#[cfg(test)] #[path = ".../tests/inline/<name>.rs"] mod x;` | allowed (external body) |
| `#[cfg(test)] mod x { ... }` | rejected (inline body) |

The scanner covers `crates/*/src` **and** root `src/` (the old guard scanned
neither the root tree nor the `#[path]` distinction), keeps a shrink-only
baseline of `0`, and ships a `--self-test` that proves an inline body fails the
gate while a `#[path]` hook passes. It runs in the `security-guards` CI job and
in `pre-flight.sh`.

**Test relocation.** The sole remaining genuine inline body
(`ragent-tui/src/app/loop_dialog/loop_dialog_tests.rs`) moved under
`tests/inline/`. Five `ragent-agent` modules that had shared a single
`mod_tests.rs` now each own a per-module `*_mod_tests.rs` restored from history.

**Production unwraps.** The nine `FtsIndex::from_index` schema lookups use a
`resolve(name)?` helper; the five per-provider tool-cache `.expect` calls collapse
into one `push_tool` helper that logs and skips on failure; `apply_patch`,
`replace`, and `read` no longer panic on their production paths; and the TUI
`/spec`/`/team`/`/triggers` handlers use `let Some(..) = .. else`. Remaining
constant-regex `expect`/`unwrap` sites carry `// INVARIANT:` or `no-panic-ok`
markers.

**Docs.** `error.rs`/`spec.rs`/`router_client.rs` gained `//!` headers, the stale
"populated by later tasks / T-00x" comments left `graph/mod.rs`, and
`ragent-codeindex`, `ragent-bench`, `ragent-specs`, and `ragent-research` build
with zero rustdoc warnings.

**Vocabulary.** `sanitise_fts_query` was renamed to `sanitize_fts_query`, and
`AGENTS-RUST.md` now documents the CLI presentation-surface exception to the
`println!`/`eprintln!` tracing rule.

### 4.7 YOLO Mode

YOLO mode bypasses Layers 2, 3, and 6 of bash security and auto-approves all
permissions. It is intended for trusted local environments and is persisted
to `ragent.json`.

```bash
/yolo        # toggle in TUI
--yes        # CLI flag
```

#### 4.7.1 Automatic sub-agent YOLO scope

An **unattended sub-agent run** - any run with `agent.mode == Subagent`,
whether spawned by `new_agent` (foreground, background or detached), `/spawn`,
cron or the server - gets a YOLO-equivalent prompt scope **for its write-capable
tools only**, regardless of the global `/yolo` flag. A sub-agent has no attached
user, so an interactive prompt is not a safeguard but a stall: the call would
wait out the watchdog and the run would be lost.

The scope is a property of the run, resolved once per turn from the tool
surface that run was offered:

| Question | Answer | Where |
|---|---|---|
| Is the run unattended? | `agent.mode == AgentMode::Subagent` | `session/processor.rs`, threaded as `ToolDispatchContext::agent_is_subagent` |
| Can the called tool write? | its `permission_category()` is not in `READ_ONLY_PERMISSION_CATEGORIES` | `session/permissions.rs::is_write_capable_permission_category` |
| What does that buy? | the interactive **prompt** is skipped, exactly where the global flag skips it | `session/permissions.rs::check_permission_with_prompt` |

The membership set is built per turn from the run's model-facing tool surface
(`SessionProcessor::write_capable_tools`), so a read-only run (`explore`, a plan
reviewer, a loop restricted to reads) yields an empty set and keeps the ordinary
policy, and a name the run was never offered cannot be waved through.

**Containment.** The scope is narrower than the global flag and widens nothing:

- the primary session never gets it - the predicate is gated on the run being a
  sub-agent;
- only write-capable tools are covered; every read a sub-agent makes stays under
  the ordinary policy;
- the loop tool-set restriction, the path guards, the 7-layer bash
  `is_safe_command` stage (which runs *before* the prompt), the tool-repeat guard
  and every pre-tool hook still apply;
- an explicit policy `Deny` still denies, and a forced destructive-action
  checkpoint still prompts (FR-024) - the automatic scope suppresses a prompt,
  never a refusal.

Because isolation between a sub-agent and its parent is by process, the
process-wide parts of YOLO (bash denied patterns, dynamic-context allowlist, MCP
config validation) are still governed by the single global flag and are never
entered by a sub-agent alone. The automatic scope adds no process-wide state: it
is a per-turn function of the run's own tool surface.

---

## 5. Configuration

### 5.1 Configuration Sources

Ragent loads configuration from (highest priority first):

1. `--config <PATH>` CLI argument
2. `.ragent/ragent.json` (or `ragent.jsonc`) in the working directory
3. `~/.config/ragent/config.json`
4. Built-in defaults

The format is compatible with OpenCode's `opencode.json`.

**Project overlays are untrusted (SEC-001/SEC-002/SEC-005).** A project-local
file may contribute the ordinary sections, but `Config::merge_project` strips the
privileged keys before the merge and logs each rejected key at `warn!`:

| Rejected project key | Reason |
|---|---|
| `yolo`, `permissions` allow-rules, bash/dirs lists | would auto-approve or widen the sandbox |
| `telemetry` | could point the OTLP endpoint at an attacker host |
| `hooks` | runs `sh -c` outside the permission system |
| `openai` | enables the inbound OpenAI-compatible surface |
| `mcp` | a stdio server spawns a subprocess with no prompt (RCE) |
| `acp` | an ACP agent spawns verbatim, and the section controls the editor-facing server (RCE) |
| `provider.<id>.api` | redirects the provider credential and prompt stream to an attacker endpoint |

These keys are honoured only from the user-global config. The rest of a project
provider entry (`thinking`, `env`, `models`) still merges.

### 5.2 Configuration Schema

```jsonc
{
  "provider": {
    "anthropic": {
      "env": ["ANTHROPIC_API_KEY"],
      "thinking": { "enabled": true, "level": "low" },
      "models": {
        "claude-sonnet-4-20250514": {
          "thinking": { "enabled": true, "level": "high", "budget_tokens": 16000 }
        }
      }
    },
    "router": {
      "enabled": true,
      "tiers": {
        "SIMPLE": { "models": [{"provider": "ollama", "model": "phi4"}] },
        "MEDIUM": { "models": [{"provider": "anthropic", "model": "claude-sonnet-4-20250514"}] },
        "COMPLEX": { "models": [{"provider": "openai", "model": "gpt-4o"}] },
        "REASONING": { "models": [{"provider": "anthropic", "model": "claude-opus-4-20250514"}] }
      }
    },
    "azure_resource": {
      "env": ["AZURE_RESOURCE_API_KEY"]
    }
  },
  "defaultAgent": "coder",
  "permissions": [
    { "permission": "file:write", "pattern": "src/**", "action": "allow" }
  ],
  "memory": {
    "enabled": true,
    "structured": { "enabled": true },
    "semantic": { "enabled": false, "dimensions": 384 }
  },
  // OpenCode-derived summarisation compaction (recommended).
  // Compaction triggers when estimated request tokens exceed the configured
  // percentage of the model's context window. `threshold: 0.7` means the
  // summarisation runs once usage crosses 70 % of the available context,
  // regardless of the model's absolute context size. When `threshold` is
  // null, the fraction-based fallback `context_window - max(output_tokens,
  // buffer)` is used instead.
  "compaction": {
    "auto": true,
    "threshold": 0.7,
    "buffer": 0.10,
    "keep": { "tokens": 0.20 }
  },
  // Token-efficient GCF encoding of JSON tool results (default off; the
  // section is omitted entirely while disabled).
  "gcf": {
    "enabled": false
  },
  // Deprecated legacy Headroom compression block. Still parsed for
  // one-release migration: `compression.enabled` maps to
  // `compaction.auto` when `compaction.auto` is not set explicitly.
  "compression": {
    "enabled": true,
    "mode": "default",
    "auto_threshold": 0.80
  },
  // Tool-name alias resolution (spec `uniformtools` FR-015). `aliases: true`
  // enables the built-in table of provider spellings; an object adds explicit
  // `"<provider spelling>": "<ragent tool>"` entries on top of it. An entry
  // naming a retired ragent spelling is a load error (FR-031). `expose_alias_names`
  // additionally advertises each spelling in the outbound tool list, and
  // `wire_trace` dumps each step's redacted request body under
  // `target/temp/wire-trace/` (FR-017). Omitted entirely while all off.
  "tools": {
    "aliases": { "WebFetch": "webfetch" },
    "expose_alias_names": false,
    "wire_trace": false
  },
  "yolo": false,
  "stream": {
    "timeout_secs": 120,
    "initial_response_timeout_secs": 300
  },
  // Connector subsystem (spec `connectors`). `enabled: false` makes the whole
  // subsystem inert (no discovery, catalogue fetch, or connection).
  "connectors": {
    "enabled": true,
    "store_dir": null,
    "stores": {
      "timeout_ms": 10000,
      "max_index_bytes": 2097152,
      "cache_ttl_secs": 3600
    },
    "credentials": {}
  },
  "dirs": {
    "allowlist": ["src/**", "tests/**"],
    "denylist": ["secrets/**", ".env"],
    "allowed_roots": ["/home/alice/shared"]
  },
  // UI internationalisation (spec `openhands` FR-027). Opt-in and omitted
  // entirely while disabled: user-facing strings (the status-bar labels, the
  // `/help` header, and the connector-catalogue footer) then render in English.
  "i18n": {
    "enabled": true,
    "locale": "fr"
  },
  // Execution backend (spec `openhands` FR-001, FR-026). Omitted = `local` (host
  // execution, FR-019). A `podman`/`docker` label runs shell/file/search tools
  // inside a container whose workspace is a named volume (never a host bind
  // mount). `backends` declares the registry entries the container backend reads.
  // A `credentials` list names encrypted-store credentials to inject as container
  // environment variables at spawn time (FR-010); the value is never in this file.
  "execution_backend": "podman",
  "backends": [
    { "id": "sandbox", "name": "Podman sandbox", "kind": "podman",
      "image": "ghcr.io/yourorg/ragent-sandbox:latest", "workspace": "/projects",
      "credentials": ["EXAMPLE_API_KEY"] }
  ],
  // LLM security analyzer (spec `openhands` FR-006, FR-017). Opt-in; a
  // tightening-only layer over the static permission rules (a `deny` is hard,
  // an `allow` never overrides an explicit policy `deny`, and a failure degrades
  // to the normal interactive prompt).
  "security_analyzer": {
    "enabled": true,
    "model": { "provider_id": "ollama", "model_id": "qwen2.5:1.5b" }
  },
  // OpenAI-compatible inbound surface (spec `openhands` FR-012, FR-024, FR-032).
  // Omitted the /v1 endpoints stay behind the native bearer-token check. When
  // `enabled` is true, a `token` (or the ambient `RAGENT_TOKEN`) is required or
  // the server refuses to start the surface (FR-032).
  "openai": {
    "enabled": true,
    "token": "sk-ragent-local"
  }
}
```

### 5.3 Environment Variables

| Variable | Provider / Purpose |
|----------|--------------------|
| `ANTHROPIC_API_KEY` | Anthropic |
| `OPENAI_API_KEY` | OpenAI |
| `GITHUB_TOKEN` | GitHub / Copilot |
| `GEMINI_API_KEY` | Google Gemini |
| `HF_TOKEN`, `HUGGING_FACE_HUB_TOKEN` | Hugging Face |
| `AZURE_AI_FOUNDRY_API_KEY`, `AZURE_AI_FOUNDRY_BASE` | Azure AI Foundry |
| `AZURE_RESOURCE_API_KEY` | Azure Resource |
| `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, `AWS_SESSION_TOKEN`, `AWS_PROFILE`, `AWS_REGION` | Amazon Bedrock |
| `XAI_API_KEY` | xAI Grok |
| `RAGENT_FOUNDRY_LOCAL_FORCE_WEB` | Force Foundry Local web-service path |
| `LANGSEARCH_API_KEY` | Web search (LangSearch) |
| `TAVILY_API_KEY` | Web search (Tavily) |

### 5.4 Compaction Configuration

The `compaction` block controls OpenCode-derived summarisation-based context-window
compaction, which replaces the older Headroom `compression` scheme. It satisfies
FR-008 (auto toggle) and FR-011 (user-overridable threshold, buffer, keep-tokens,
and summary output-token values).

```jsonc
{
  "compaction": {
    // Enable automatic pre-send summarisation. When false, only emergency
    // overflow compaction runs on provider context-overflow errors (FR-008).
    "auto": true,
    // Fraction of the model's context window at which automatic pre-send
    // compaction triggers (0.0–1.0). Default: 0.7 (70 %). This is the
    // recommended, model-independent trigger: 70 % of a 32k window is 22.4k
    // tokens, while 70 % of a 200k window is 140k tokens. The threshold is
    // never below 70 %, even when a fraction-based fallback is used.
    "threshold": 0.7,
    // Response/safety buffer as a fraction of the model's context window.
    // Only used when `threshold` is null. Compaction then triggers when
    // estimated request tokens exceed context_window - max(output_tokens,
    // buffer). Default: 0.10 (10 %, FR-011).
    "buffer": 0.10,
    // Recent conversation turns kept verbatim after compaction.
    "keep": {
      // Fraction of the context window reserved for recent user/assistant/tool turns to preserve.
      // Default: 0.20 (20 %, FR-011).
      "tokens": 0.20
    },
    // Optional dedicated fast/cheap model for the summarisation call
    // (avoids paying the session model's latency for compaction).
    "model": { "provider_id": "ollama", "model_id": "qwen2.5:1.5b" },
    // Maximum tokens requested for the compaction summary output.
    // Default: 1500 (was 4096; smaller budget = proportionally lower latency).
    "summary_tokens": 1500,
    // Truncation limit per tool output serialised into the summarisation prompt.
    // Default: 2000 characters.
    "tool_output_max_chars": 2000
  }
}
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `auto` | bool | `true` | Enable automatic pre-send compaction (FR-008) |
| `threshold` | f64 | `0.7` | Fraction of context window that triggers compaction; model-independent |
| `buffer` | f64 | `0.10` | Fallback response/safety buffer as a fraction of the context window when `threshold` is null (FR-011) |
| `keep.tokens` | f64 | `0.20` | Fraction of context window reserved for recent turns preserved verbatim (FR-011) |
| `model` | object | none | Optional fast/cheap model override for the summarisation call (`/compact` and `compact_session` only); the session's primary model is used when unset. The pre-send auto-compaction path always uses the session's primary model and publishes an `AgentNotice` when an override is configured |
| `summary_tokens` | usize | `1500` | Maximum tokens requested for the compaction summary output (FR-011) |
| `tool_output_max_chars` | usize | `2000` | Truncation limit for each tool output serialised into the summarisation prompt |

The compaction trigger threshold is always raised to at least 70 % of the
model's context window, so automatic pre-send compaction never fires on routine
prompts that fill less than 70 % of the available context. Emergency overflow
compaction is not subject to this floor.

The compaction summary output length defaults to `1500` tokens (configurable
via `compaction.summary_tokens`) and tool outputs are truncated to `2000`
characters (`compaction.tool_output_max_chars`) before being serialised into
the summarisation prompt. The prompt itself is capped adaptively at
`min(60,000 chars, context_window * 4 / 2)` so small local models are not
swamped. The summarisation stream has a hard 180-second overall cap plus a
60-second per-chunk stall timeout, and progress is reported every 10 seconds
with the elapsed time and character count received so far. When
`compaction.model` is set, the `/compact` path builds a dedicated client for
the override model instead of reusing the session's primary model. The
pre-send auto-compaction path always uses the session's primary model and
publishes an `AgentNotice` when an override is configured.

The legacy `compression` block (`compression.enabled`, `compression.mode`,
`compression.auto_threshold`) is still parsed for one-release migration:
`compression.enabled` is treated as `compaction.auto` when `compaction.auto` is
not set explicitly. New configurations should use `compaction`.

Trigger the one-shot compaction manually with the `/compact` slash command
(`/compress` is a deprecated alias — FR-009).

### 5.5 Thinking Configuration

Thinking/reasoning is configured per model:

```jsonc
{
  "provider": {
    "anthropic": {
      "models": {
        "claude-sonnet-4-20250514": {
          "thinking": { "enabled": true, "level": "medium" }
        }
      }
    }
  }
}
```

Levels: `auto`, `off`, `low`, `medium`, `high`. `budget_tokens` is optional.

---

## 6. Terminal User Interface (TUI)

### 6.1 TUI Layout

The TUI is a ratatui full-screen interface with these panels:

| Panel | Purpose |
|-------|---------|
| Chat | Conversation stream with markdown and syntax highlighting |
| Log | Step-numbered tool calls with pretty-printed JSON |
| Status bar | Provider/model, context usage, compression indicator, YOLO state |
| Teammate strip | One row of pills under the status bar, one per active teammate (status glyph, clipped name, focused highlight, `Alt+^v:cycle` hint); omitted when no team is active (spec `agentui` FR-027) |
| Agents window | `Agents`-button popup listing the primary agent and an ancestor-indented sub-agent tree with status, elapsed time, step count, `[stop]`/`[x]` status glyphs, and a background-shell section; openable from the keyboard with `Alt+A` (the chord resolves to `InputAction::ToggleAgentsWindow` and calls the same `App::toggle_agents_window` the button click does, so both share one visibility path, one no-subject no-op and one redraw flag - spec `agentssetup` FR-022); keyboard-first: a block cursor (`Up`/`Down`/`PgUp`/`PgDn`) selects a row, `Space` toggles suspend/resume, `Del` arms the kill confirmation, `Enter` opens the row's output view, and clicking a row selects it; the FR-077 key legend sits in the footer (spec `agentui` FR-006..FR-016) |
| Teams window | `Teams`-button popup listing the lead plus every teammate with focus markers, status words from `MemberStatus::display_label()`, model column (inherited global model suppressed), and claimed/done/sent/recv counters; the block cursor plus `Enter`/`Space`/`Del` drive focus, suspend and shutdown; clicking a teammate selects it (spec `agentui` FR-017..FR-026) |
| Input | Command-line with slash-command autocomplete |
| Permission dialog | Modal approval dialog with live countdown (120 s) |
| Question dialog | Modal user question (free-text or multiple-choice) |
| Row-action dialog | Yes/No confirmation for destructive row actions (sub-agent kill, background-shell cancel, teammate suspend/shutdown) drawn over the windows, keyboard and mouse answered (spec `agentui` T-009) |

### 6.2 Slash Commands

| Command | Description |
|---------|-------------|
| `/quit` / `/q` | Exit ragent |
| `/provider` | Open provider setup dialog (always allows editing the stored key) |
| `/model` | Select model (jumps to model list when a provider is already configured) |
| `/agent` | Select agent |
| `/agents` | List loaded agents and diagnostics |
| `/websearch show` | Show web-search engine diagnostics (enabled / in-use / failed) |
| `/websearch help` | Show `/websearch` subcommand help |
| `/websearch test` | Test configured web-search backends |
| `/webapi enable\|disable\|help` | Manage the HTTP REST API |
| `/tools` | List every registered tool: `/tools list` (alias `show`) prints the registry with each tool's source and description, `/tools help` prints help |
| `/codeindex on\|off` | Enable/disable code index |
| `/codeindex lang <language>` | Filter code index by language |
| `/gcf on\|off\|show\|help` | Toggle GCF token-efficient tool-result encoding; `show` reports state and source |
| `/compact` | Summarise and compact the conversation history (one-shot LLM summarisation; FR-009). `/compress` is a deprecated alias |
| `/memory` | Structured-memory panel and summary: `/memory` / `/memory show` (same), `/memory clear` (a `Yes`/`No` dialog clears this project's memories), `/memory help` |
| `/yolo` | Toggle YOLO mode (`on`/`off` set and persist the state, `status` reports it, a bare `/yolo` toggles; the toggle writes the user-global config, or the `--config` file when one was loaded) |
| `/todo` `/task` | Open the TASKS side panel (also Alt+T); subcommands delegate to task tools |
| `/team create <name>` | Create a team |
| `/team open <name>` | Re-open existing team |
| `/team close` | Close current team |
| `/team clear` | Reset current team state |
| `/team delete <name>` | Delete team on disk |
| `/team cleanup` | Tear down current team |
| `/team message ...` | Send team message |
| `/swarm <prompt>` | Decompose prompt into parallel subtasks |
| `/swarm status` | Show active swarm tasks |
| `/swarm kill` | Cancel active swarm |
| `/spawn <agent> <prompt...>` | Launch a detached fire-and-forget background sub-agent from the input line (spec `spawnagent`); not listed by `list_agents`, not awaitable via `wait_agents`, its result never injected back into the chat |
| `/autopilot on\|off` | Toggle autonomous mode |
| `/spec create\|specify\|plan\|tasks\|update\|add\|feedback\|jtbd\|list\|search\|show\|validate\|status\|task\|impl\|coverage\|activate\|deactivate\|delete` | Spec lifecycle and SDD commands |
| `/plugins list\|add\|remove\|enable\|disable\|test\|stores\|marketplace\|help` | Manage sandboxed Codex/Claude plugins; `/plugins list [--verbose]` opens an interactive panel (the plugin table pinned at the top - a fixed column heading and separator - above a scrollable block-cursor list, with the capability blocks listed at the end of the same body; `Up`/`Down`/`Home`/`End` move the cursor, `Space` toggles the highlighted plugin's enabled state (enable/disable, reporting the outcome on a feedback line), and `Esc` closes); `/plugins test` runs an isolated harness; `/plugins codex [query] [--category <name>] [--refresh]` and `/plugins claude [query] [--category <name>] [--refresh]` browse each store's official marketplace with a left-hand category navigator (`ALL` plus the distinct categories the fetched index declares) operated by keyboard and mouse, its own document shape normalised by that store's `StoreProvider`, and each row showing the plugin id, version, the marketplace it came from, and the description; `/plugins stores [--check]` reports each store's effective endpoint and its source, and with `--check` also contacts each store to report availability and plugin count; `/plugins marketplace show` lists the marketplaces configured for both stores in one section each (Codex, Claude) as a name/url/description table; `/plugins marketplace <codex|claude> add <source> [--force]|remove <name>|list` manages additional marketplaces (from an `owner/repo` slug, an SSH remote, an `https://` URL, or a local directory) in the user-global `plugins.stores.marketplaces`, which the two browse panels fetch and merge into the one panel; `/plugins add` accepts a `git+<https-url>#<ref>[:<subpath>]` git source; CLI parity via `ragent plugins` |
| `/connectors list\|claude\|add\|remove\|enable\|disable\|connect\|disconnect\|auth\|test\|stores\|marketplace\|help` | Manage MCP-backed connectors (named integrations carrying a category, an auth shape, and one or more MCP servers); `/connectors claude [query] [--category <name>] [--refresh]` opens the interactive catalogue browser (the single catalogue surface), which merges the base catalogue endpoint and every marketplace added with `/connectors marketplace claude add`, shows each row's id, category, the marketplace it came from, and its description, and resolves blank descriptions from each entry's own repository manifest off-loop with a `loading details...` title hint; `/connectors marketplace show` lists the marketplaces configured for the Claude catalogue and `/connectors marketplace claude list\|add <source> [--force]\|remove <name>` manages them in the user-global `connectors.stores.marketplaces`; `/connectors test` runs an isolated connect-and-invoke harness; `/connectors stores [--check]` reports each catalogue endpoint's provenance; `/connectors add` accepts a catalogue id, a local directory, a local `.zip`/`.tar.gz`, or an `https://` package URL; CLI parity via `ragent connectors` (spec `connectors`, section 19C) |
| `/backend [<kind>]` | Open the execution-backend switcher (active backend + each backend's health) or switch to a kind/id directly; `/backend help`. See section 19D (spec `openhands` FR-008) |
| `/security on\|off\|status\|revert\|help` | LLM security-analyzer mode (spec `openhands` FR-006, FR-017). `security-analyzer` is accepted as an alias. See section 19D.7 |
| `/automation [list]\|runs <id>\|run <id>\|help` | Manage the automation service (spec `openhands` FR-013, FR-014, FR-018). See section 19F |
| `/i18n on\|off\|status\|list\|<locale>\|help` | UI internationalisation: enable translation, set the locale, list installed catalogs (spec `openhands` FR-027) |
| `/queue list\|clear\|next\|help` | Inspect the message input queue (messages and slash commands submitted while the agent executes; spec `inputqueue` FR-013/FR-017 amendment) |
| `/toolchain list [lang] [--json]` | Language toolchain report |
| `/blueprints help\|list` | List installed team blueprints |
| `/skill help\|list\|<name> [args]` | Load or inspect skill packs: list them, or load and invoke `<name>` |
| `/osinfo show [--no-probe]\|help` | Read-only host OS/hardware report; routes through the `os_info` tool's collector/renderer (spec `osinfo`) |
| `/research create\|list\|show\|search\|cluster\|archive\|delete\|update` | Research commands; `create` supports `--from-file`, `--from-url`, `--use-low-relevance`, `--papers`, `--oa-enable`/`--no-oa`, `--max-concepts N`, `--max-findings N`, `--url-cloak`, `--age <SPAN>` |
| `/config show` | Show resolved configuration |
| `/config save` | Snapshot global `ragent.json` to `saves/` (atomic, timestamped) |
| `/config list` | Interactive picker to restore a saved backup |
| `/init config` | Create a default `ragent.json` in the global config directory |
| `/startup` | Show per-stage startup timing instrumentation |
| `/prompt help\|primary [agent]\|subagent [agent]\|list\|<agent>` | Read-only agent system-prompt inspector (no LLM call) |
| `/telemetry help\|on\|off\|setup\|counters` | Manage OpenTelemetry metrics export |
| `/dirs` | Show configured writable directories |
| `/profile` / `/status` / `/mouse` | UI preferences |
| `/skill` / `/skills` | Load or inspect skill packs |
| `/mcp [status]\|config\|discover\|connect <id>\|disconnect <id>` | MCP server status, discovery location report, and enable/disable |
| `/update` / `/update install` | Auto-update (reserved; not implemented) |

### 6.3 TUI Component Architecture

```mermaid
graph LR
    App[App State] --> EventLoop[Event Loop]
    EventLoop --> Terminal[Terminal]
    App --> Chat[Chat Widget]
    App --> Log[Log Widget]
    App --> Input[Input Widget]
    App --> Status[Status Bar]
    App --> Windows[Agents / Teams Windows + Output View Legend]
    App --> Dialogs[Permission / Question Dialogs]
    EventBus --> App
```

**Figure 7:** TUI Component Architecture — UI layout and event wiring

### 6.5 Plugin-Store Category and Marketplace Navigators

The plugin-store browse panel opened by `/plugins codex` and `/plugins claude`
(spec `catnav`, `pluginstores` FR-052) splits its inner area into a left-hand
navigator column and the existing result column, and the navigator column is split
**vertically** into a **category navigator** (upper half) and a **marketplace
navigator** (lower half). Each navigator lists the `ALL` sentinel first, then the
distinct non-empty values the fetched store index declares (sorted
case-insensitively, folding case-only duplicates): the categories from each entry's
`category`, and the marketplaces from each entry's `marketplace` label. An index that
declares no category or no marketplace renders `ALL` only for the corresponding
navigator and still browses normally (FR-018). Every navigator row appends the number
of loaded entries under that value as `Name (N)`, counted over the full fetched index
so the count is stable while a query or filter narrows the result list; the `ALL` row
counts the whole index (FR-053). The active category and marketplace are
shown in the panel title (`-- category <name> -- marketplace <name>`) and painted in a
distinct colour; the row under each navigator's cursor carries a block cursor
independent of the result-list cursor.

The category navigator additionally carries an **`Installed` pseudo-category** in
row 1, directly below `ALL` and above the index's own categories (spec `pluginstores`
FR-054). Selecting it narrows the results to the plugins already present in the store
scan, matched on the plugin id rather than on a declared category, so an installed
plugin that declares no category still appears; the row's `Name (N)` count is the
number of loaded entries the scan matches. It composes with the query and marketplace
filters like any other navigator row, and `c` clears back to `ALL`.

The panel has three focusable panes and is fully operable by keyboard: `Tab`/`BackTab`
cycle focus forward/backward through the result list, the category navigator, and the
marketplace navigator; `Up`/`Down` move the focused pane's cursor (moving a navigator
cursor applies the highlighted value on the same keypress and scrolls that navigator to
keep the cursor visible); `Enter` over a navigator
applies the value and hands focus back to the result list (never installing),
and `c` clears the focused navigator's filter to `ALL`. Mouse input is an additive
convenience: a
left-click on a navigator row selects and applies it and focuses that pane, a
wheel over a navigator scrolls its viewport without changing the active filter,
and a left-click on a result row moves the result cursor while keeping the active
filters. Every mouse path is a convenience over the keyboard path, so a terminal
without mouse reporting still reaches every value (FR-023), and no navigator
path ever installs, downloads, or executes plugin JavaScript (FR-024, FR-025).

Selecting a category or a marketplace re-derives the visible result set from the full
fetched index
in memory, resets the result cursor to the first survivor, and issues no network
request (FR-014, FR-028); the category and marketplace predicates compose with each
other and with the free-text query
(a result is visible only when it matches all active filters). An entry with a blank
marketplace is visible only under `ALL`. A value no fetched entry
declares is never presented as a row and a selection that vanishes after a re-fetch
falls back to `ALL` (FR-027). A `--category <name>` launch argument holds the
requested filter until the index lands, then validates it against the derived rows
exactly like an in-panel selection, reporting an unknown name in the footer while
leaving every row visible (FR-022).

Each result row renders the plugin id, its version, **the marketplace the entry came
from**, and its description, and the panel clamps to a 140-column ceiling (a third wider
than the original 100) so the marketplace column has room. The marketplace column names
the added marketplace an entry was fetched from (the `name` it was added under, matched by
resolved URL), or the store label (`Codex`/`Claude`) for the store's base endpoint, so a
merged multi-marketplace panel shows each row's source (FR-048). The marketplace and
description columns share the flexible remainder, so a longer marketplace name is shown
in full when the terminal has room and clipped without pushing the fixed id/version
columns or the one-column installed lane off the row when it does not. That lane sits
beside the `> ` cursor gutter and carries a bold green `*` on a row already installed;
every other row paints a blank there so the columns stay aligned.

The panel's bottom inner row carries a **search box** (FR-051): a leading `search:` label,
the live filter field, and a right-aligned `[x] clear` button. The field mirrors the
browser's query byte-for-byte, so the box and the result filter cannot drift; a query
wider than the field is clipped from the left so the newest characters stay visible. A
left-click on the clear button clears the search filter through the same in-memory path
as the keyboard editor, restoring the full result set while leaving the active category
untouched; the button is inert when the query is already empty. A row too narrow to hold
both the label and the button drops the button rather than squeezing the field below a
usable width. The panel is framed with the same Unicode box-drawing borders every other
panel uses, over a plain-ASCII body.

A vendor marketplace is a *listing* rather than a native index: the Codex catalogue
carries neither a version nor a description, and the Claude catalogue carries a
description but only sometimes a version. After the index lands, the panel resolves each
plugin's `version` and `description` from the plugin's **own manifest**, fetched off the
event loop from the raw URL derived from its install source when that source is a
`github.com` git source - `https://raw.githubusercontent.com/<owner>/<repo>/<ref>/<subpath>/<manifest>`,
with the store's dialect-appropriate manifest name (`.codex-plugin/plugin.json` for
Codex, `.claude-plugin/plugin.json` for Claude) tried first and a bare `plugin.json` as a
fallback. The provider transform (`transform_vendor_entry`) reads what the listing
declares, including the entry's `interface.displayName`/`shortDescription`/
`longDescription`, and leaves an absent version blank rather than fabricating a `0`. The
enrichment (`ragent-plugins/src/store_enrich.rs`) is best-effort and never fatal (a
missing manifest, a transport failure, non-JSON bytes, or an oversized body yields no
update), runs on a bounded thread pool, caches each URL's result (including a negative
result) across opens, and is injectable so the tests run offline. The title shows
`loading details...` while it runs; a version that stays unknown renders an explicit `-`
rather than a blank cell, and enrichment never overwrites a value the index declared.

The result list itself is rendered in ascending alphabetical order of plugin id,
case-insensitively with a byte-wise tiebreak, derived from the id alone rather than the
order the store index declared the entries in or the order the configured marketplaces
were merged (FR-050). The sort is applied when the index lands, before the filter is
derived, so a free-text query and an active category both narrow an already-sorted set
and the survivors keep their alphabetical order. One comparator is shared by every store
surface (`compare_store_ids`/`sort_store_entries` in `ragent-plugins`), mirroring the
connector system's `compare_connector_ids`. The navigator's category rows keep their own
case-insensitive sort, and a case-insensitive category collision (for example `Utility`
and `utility`) keeps the first spelling the index declares, so the result sort never
relabels a category.

### 6.4 Permission Dialog Countdown

The permission dialog displays a live countdown:

- Format: `M:SS` (e.g., `1:45`)
- Shows `EXPIRED` when the 120-second timeout is reached
- The event loop redraws continuously so the timer decrements without requiring keyboard input

---

## 7. HTTP Server & API

### 7.1 Starting the Server

```bash
ragent serve --port 9100 --host 127.0.0.1
```

### 7.2 REST Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/health` | Health check |
| GET | `/openapi.json` | OpenAPI 3.1 document (public, no auth) |
| POST | `/sessions` | Create a session |
| GET | `/sessions` | List sessions |
| GET | `/sessions/{id}` | Get session details |
| POST | `/sessions/{id}/messages` | Send a message (returns SSE stream) |
| GET | `/sessions/{id}/events` | SSE event stream |
| GET | `/models` | List available models |
| GET | `/providers` | List providers |
| GET | `/agents` | List agents |
| GET | `/v1/models` | OpenAI-compatible model list (spec `openhands` FR-003) |
| POST | `/v1/chat/completions` | OpenAI-compatible chat completion; `stream:false` returns JSON, `stream:true` returns `chat.completion.chunk` SSE (spec `openhands` FR-003, FR-011, FR-012) |
| POST | `/auto/{id}` | Public automation webhook ingress: enqueue a run with the request body as context (spec `openhands` FR-013) |
| GET | `/automation` | List configured automations (bearer) |
| GET | `/automation/runs/{id}` | List an automation's run history (bearer) |
| POST | `/automation/{id}/run` | Enqueue a manual run now (bearer) |
| GET | `/research` | List research items |
| POST | `/research` | Create + run a research gathering session (returns `202 Accepted` with `Location` header) |
| GET | `/research/{name}` | Show one research item (supports `?full=true` for extended metadata) |
| DELETE | `/research/{name}` | Delete a research item (requires `?confirm=delete-{name}`) |
| GET | `/research/{name}/events` | SSE stream of live research events for a background run |

#### 7.2.1 OpenAPI Contract and Typed Client (spec `openhands` FR-007)

The full REST surface is described by an OpenAPI 3.1 document served at
`GET /openapi.json`. The document is rendered from a single route table
(`crates/ragent-server/src/routes/openapi.rs::ROUTES`) so it cannot drift from the
served routes; a companion integration test asserts the table and the router agree
in both directions (it scrapes every `.route(...)`/`.nest(...)` literal from
`src/routes/` and probes every documented parameterless `GET` through the live
router). Every protected operation declares the native bearer `security` scheme;
`/openapi.json` itself is served without the token because the document carries no
runtime data or secrets.

The same route table generates a dependency-free typed TypeScript client, checked
in at `docs/openapi/ragent-client.d.ts` and reproduced by:

```bash
ragent openapi              # print the OpenAPI 3.1 document
ragent openapi --client     # print the generated typed TypeScript client
```

For `ragent openapi`, tracing is routed to stderr so the machine-readable stdout
stays clean.

MCP servers configured with the `http` transport (`"type": "http"`, `"url"`)
connect over Streamable-HTTP request/response alongside the existing `stdio`
transport (FR-025).

### 7.3 SSE Events

The server streams the following event types:

| Event | Description |
|-------|-------------|
| `assistant_chunk` | Token or reasoning block |
| `tool_requested` / `tool_executed` | Tool lifecycle |
| `permission_requested` / `permission_replied` | Permission flow |
| `question_requested` / `question_answered` | Question tool |
| `compression_started` / `compression_finished` | Context compaction lifecycle |
| `subagent_*` / `teammate_*` | Multi-agent lifecycle |
| `agent_notice` | Status messages |
| `research` | Research session events (phase, web capture, synthesis, analysis, done) — streamed via `GET /research/{name}/events` |

### 7.4 Authentication

The HTTP API uses Bearer token authentication:

```bash
curl -H "Authorization: Bearer $RAGENT_TOKEN" ...
```

`RAGENT_TOKEN` is read from the environment. If unset, the server may allow
local requests without authentication depending on build configuration.

The OpenAI-compatible surface (`/v1/models`, `/v1/chat/completions`) uses the
same bearer token as the native API (FR-024) - ragent runs one server token. The
`openai.token` config key (or the ambient `RAGENT_TOKEN`) supplies it. When the
surface is explicitly enabled (`openai.enabled: true`) without a token, the
server **refuses to start** rather than serve the surface unauthenticated
(FR-032).

#### 7.4.1 OpenAI-compatible surface (spec `openhands` FR-003, FR-011, FR-012, FR-038)

`POST /v1/chat/completions` bridges the last user message to one
`SessionProcessor` turn and answers in the OpenAI dialect:

- `stream:false` returns one `chat.completion` JSON object (FR-011).
- `stream:true` returns a `text/event-stream` of `chat.completion.chunk` events -
  an opening `delta.role = "assistant"` chunk, one chunk per assistant
  `TextDelta`, and a terminal chunk carrying the mapped `finish_reason`
  (`stop` / `length` / `tool_calls` / `content_filter`) - terminated by a
  `data: [DONE]` sentinel (FR-012).

Both paths run every tool call through the same permission layer as a native
request (`auto_approve` is never forced); a tool needing confirmation raises the
ordinary `PermissionRequested` prompt and, with no interactive client attached,
times out to a denial (FR-038).

### 7.5 HTTP API Request Flow

```mermaid
sequenceDiagram
    participant Client
    participant Axum
    participant SP as Session Processor
    participant LLM
    Client->>Axum: POST /sessions/{id}/messages
    Axum->>SP: dispatch message
    SP->>LLM: streaming request
    LLM-->>SP: chunks / tool calls
    SP-->>Axum: events
    Axum-->>Client: SSE stream
```

**Figure 8:** HTTP API Request Flow — REST + SSE lifecycle

---

# Part III: Data & Knowledge Systems

---

## 8. Code Index

### 8.1 Overview

The code index provides fast, local, read-only code intelligence. It is built
on tree-sitter parsing and Tantivy full-text search, with a SQLite metadata
store.

### 8.2 Supported Languages

Rust, Python, TypeScript/JavaScript, Go, C/C++, Java, OpenSCAD, Terraform,
CMake, Gradle, Maven, and more (15+ grammars compiled-in).

### 8.3 Architecture

```mermaid
graph LR
    Scan[File Scanner] --> Parse[Tree-sitter Parser]
    Parse --> Store[(SQLite Metadata)]
    Parse --> Index[Tantivy FTS Index]
    Query[Tool Query] --> Index
    Query --> Store
```

**Figure 9:** Code Index Pipeline — File scan → parse → index → search

### 8.4 Code Index Tools

| Tool | Purpose |
|------|---------|
| `codeindex_search` | Full-text search across indexed code |
| `codeindex_symbols` | Search for symbols by name |
| `codeindex_references` | Find references to a symbol |
| `codeindex_dependencies` | Query file-level imports/dependents |
| `codeindex_status` | Show index status |
| `codeindex_reindex` | Trigger a full re-index |

All code index tools are **hardwired always-allowed** because they are read-only
and local-only.

### 8.5 Semantic Code Graph

The code index also maintains a **semantic code graph** — a typed edge graph
over indexed symbols that captures relationships such as `calls`, `imports`,
`inherits`, `references`, `mixes_in`, and `implements`. Edges are either
`EXTRACTED` (from tree-sitter parse data) or `INFERRED` (heuristic).

The graph is built on demand via `/codeindex graph build` (or
`/codeindex graph lang <language>` for a per-language subgraph) and persisted
in the `graph_edges` SQLite table. Graph builds run on a dedicated OS thread
(`codeindex-graph-build`) via `CodeIndex::spawn_graph_build`, so the TUI event
loop stays responsive while the derivation runs; a double-build guard refuses
overlapping builds.

Graph builds use a phased lock discipline: the store mutex is taken only for
two brief windows - a read-only snapshot of the derivation inputs (symbols,
refs, imports, files) and the final single-transaction persist into
`graph_edges`. The CPU-heavy derivation runs with no locks held, so FTS search
and the other store readers remain available while the graph builds; only the
short write transaction touches the `graph_edges` table.

#### Graph Tools

| Tool | Purpose |
|------|---------|
| `codeindex_godnodes` | Top-N most-connected symbols (highest degree) |
| `codeindex_path` | Shortest path (by hop count) between two symbols |
| `codeindex_explain` | Node metadata and incoming/outgoing edges for a symbol |
| `codeindex_communities` | Community detection via label propagation |

All graph tools use non-blocking `try_*` variants and return a
`codeindex_busy` response when the index is locked (e.g. during a reindex).
Graph name resolution (`codeindex_path`, `codeindex_explain`) prefers exact
name matches over the underlying substring query and ranks definition kinds
(struct/function/trait/class/enum/interface) above impl/module containers, so
a trait named `CachedSessionProcessor` no longer shadows the struct
`SessionProcessor`.

#### Graph Status

`/codeindex show` reports graph-level statistics alongside index stats:
edge count (total, extracted, inferred), node count, per-kind edge counts,
and community count. While a graph build is in flight, `/codeindex show`
reports `**Graph:** building...` with per-file progress (`done/total`), and
the `codeindex_status` tool reports `graph_state: "building"` plus
`graph_building`/`graph_done`/`graph_total` metadata. `codeindex_status`
never blocks on the store lock: when the lock is held it returns an immediate
busy report built from the lock-free progress atomics (metadata `busy: true`,
`error: "codeindex_busy"`). The `IndexStats` type carries `graph_total_edges`/
`graph_nodes`/`graph_communities` so status surfaces can report "graph built"
vs "not built" without a separate graph query.

### 8.6 Code Index Status Indicators

The TUI status bar (second line, top-right) shows two bold busy indicators
while the code index works:

- `IDX` glyph (`idx`) -- the FTS/store indexes are being updated (a reindex
  holds the store/FTS locks or the reindex progress counters are active).
- `GRAPH` glyph (`graph`) -- the semantic edge graph is being (re)built.

Both busy tags render in every responsive mode and are polled lock-free from
`CodeIndex` atomics (`reindex_progress()` / `graph_busy()` /
`graph_build_progress()`) by `App::refresh_code_index_stats`, so they stay
live even while the index mutexes are held by the background work.

Attribution rule: when the store lock is unavailable (`try_status()` returns
`None`) and `graph_busy` is set, the lock is attributed to the graph build --
the `idx` indicator is not latched and is cleared if stale, so the two
indicators track their own phases and never vanish simultaneously at build
completion.

### 8.7 Incremental Updates

A file watcher detects changes and incrementally updates the index. Language
filtering is available via `/codeindex lang <language>`.

---

## 9. Memory System

### 9.1 Three Tiers

| Tier | Storage | Use Case |
|------|---------|----------|
| **File blocks** | Markdown files in `.ragent/memory/` or `~/.ragent/memory/` | Long-form notes, patterns, preferences |
| **Structured store** | SQLite via `memory_store` / `memory_recall` | Facts, errors, workflows with tags/confidence |
| **Semantic search** | Optional embeddings (`all-MiniLM-L6-v2`) | Similarity-based recall |

### 9.2 Memory Operations

| Tool | Purpose |
|------|---------|
| `memory_read` | Read a memory block file |
| `memory_write` | Write/append to a memory block file |
| `memory_replace` | Replace a string in a named block |
| `memory_store` | Store a structured memory entry |
| `memory_recall` | Full-text search structured memories |
| `memory_search` | Semantic/keyword search across memories |
| `memory_forget` | Delete memories by filter |
| `memory_migrate` | Split a flat MEMORY.md into blocks |
| `conversation_search` | Keyword/turn-range/stats search over the current session |
| `session_search` | Cross-session full-text search with filters and context |

### 9.3 Automatic Extraction

After each assistant turn the system can extract:

- Project facts and patterns
- Errors and their resolutions
- User preferences
- Workflows and standard operating procedures

Extracted memories are tagged with category, confidence, and source.

### 9.4 Decay, Compaction, and Knowledge Graph

- Memories decay in relevance over time unless accessed or reinforced.
- Compaction summarises old memories.
- A lightweight knowledge graph links related memories by tag and reference.

---

## 10. Spec Management

### 10.1 Overview

Ragent includes a built-in specification lifecycle for tracking features,
requirements, and implementation tasks.

### 10.2 Directory Layout

Specs live in `specs/<SpecId>/` (58 spec directories as of v1.0.141). Only
`SPEC.md` is required: a directory holding just a `SPEC.md` is discoverable and
readable, and is a valid target for `/spec update`, `/spec plan` and
`/spec validate`. Every other file in the layout below is optional and derived.
The newest is `specs/agentui/` — a draft spec for the Agent and Team Member
panel (the `Agents` and `Teams` popups, the teammate strip, and their live
event plumbing):

```text
specs/
└── testspec/
    ├── SPEC.md          # EARS requirements and status (required)
    ├── PLAN.md          # Implementation tasks and Phase -1 gates
    ├── TASKS.md         # Ordered task list extracted from PLAN.md
    ├── TESTPLAN.md      # Manual test-plan with TC-NNN test cases
    ├── CONSTITUTION.md  # Optional: project constitution with nine articles
    ├── data-model.md    # Optional: data-model artifact (gated by sdd.data_model)
    ├── contracts/       # Optional: API contracts directory (gated by sdd.contracts)
    ├── quickstart.md    # Key validation scenarios derived from SPEC.md
    ├── FEEDBACK.md      # Optional: production feedback notes
    ├── JTBD.md          # Optional: Jobs-To-Be-Done analysis
    └── REPORT.md        # Optional audit/completion report
```

### 10.3 Spec Status Lifecycle

```mermaid
graph LR
    draft[draft] --> in_progress[in_progress]
    in_progress --> implemented[implemented]
    implemented --> in_review[in_review]
    in_review --> verified[verified]
    in_review --> draft
    verified --> archived[archived]
```

### 10.4 Spec Tools

| Tool | Purpose |
|------|---------|
| `spec_list` | List specifications |
| `spec_read` | Read a spec by ID |
| `spec_search` | Search specs by keyword |
| `spec_coverage` | Generate requirement coverage report |
| `spec_task_update` | Update a plan task status; mirrors the status into the session task tracker (creates the tracker task if none exists, updates it if it does) |

### 10.5 Slash Commands

| Command | Purpose |
|---------|---------|
| `/spec create <name> <title> [--from-research <name>] [--folder <path>]` | Create a new spec (SPEC.md + PLAN.md + TESTPLAN.md). `--folder` targets another project's `specs/` root (used by `/spec reverse --folder` chaining) |
| `/spec specify <name> <feature> [--from-research <name>]` | Generate SPEC.md only (EARS spec with `[NEEDS CLARIFICATION]` markers); optionally creates a git branch when `sdd.branch_per_spec` is enabled |
| `/spec plan <name> <tech-context>` | Generate or regenerate PLAN.md from existing SPEC.md using technology context as guidance |
| `/spec tasks <name>` | Generate TASKS.md (ordered task list from PLAN.md) and quickstart.md (validation scenarios from SPEC.md) |
| `/spec update <specname>` | Re-read `SPEC.md` and regenerate `PLAN.md` + `TESTPLAN.md` (preserves unchanged task IDs) |
| `/spec add <id> <requirement>` | Add an incremental requirement to an existing spec |
| `/spec feedback <spec-id> <note>` | Append a production feedback note to `FEEDBACK.md` |
| `/spec impl <id>` / `/spec implement <id>` | Transition spec to `in_progress` and generate plan |
| `/spec jtbd <specname> [--force] [--agent <name>]` | Perform JTBD analysis and write `JTBD.md` in the spec folder |
| `/spec list [--status <status>] [--prefix <prefix>]` | List specs with optional filtering |
| `/spec search <query>` | Search specs by keyword |
| `/spec show <id>` | Read a spec |
| `/spec validate [specname]` | Validate EARS compliance, clarification markers, and consistency (ambiguity, contradiction, gap detection) |
| `/spec status <id> [<new-status>]` | Show or transition spec status |
| `/spec task <id> [<task-id>] [<new-status>]` | List, show, or update task status |
| `/spec coverage <id>` | Generate requirement coverage report |
| `/spec activate <id>` | Activate a spec for context injection into agent prompts |
| `/spec deactivate` | Deactivate the currently active spec |
| `/spec delete <id> [--yes]` | Delete a spec directory |

### 10.6 Research Linkage

Specs can reference research outputs via the `--from-research` flag on
`/spec create` and `/spec specify`. When provided, the spec's `SPEC.md`
receives a `research:` key in its YAML frontmatter linking to the research
artifact, and a `## Related Research` section is injected into the body
summarising the linked research output.

### 10.7 SDD Configuration Flags

The following configuration flags in `ragent.json` under the `sdd` key control
opt-in Spec-Driven Development capabilities:

| Flag | Default | Purpose |
|------|---------|---------|
| `branch_per_spec` | `false` | Create a `spec/<name>` git branch during `/spec specify` |
| `data_model` | `false` | Generate `data-model.md` during `/spec plan` |
| `contracts` | `false` | Generate `contracts/` directory during `/spec plan` |
| `feedback_loop` | `false` | Surface `FEEDBACK.md` notes during `/spec plan` regeneration |

### 10.8 Consistency Validation

`/spec validate` performs three categories of consistency checks in addition
to EARS compliance:

- **Ambiguity detection** — flags vague terms (e.g. "fast", "efficient") and
  undefined references in requirement text.
- **Contradiction detection** — identifies conflicting requirements that
  specify opposing constraints.
- **Gap detection** — identifies requirements lacking acceptance criteria.

Consistency warnings are included in the validation report alongside EARS
errors and `[NEEDS CLARIFICATION]` marker warnings.

### 10.9 Clarification Markers

During `/spec specify`, the LLM is instructed to insert
`[NEEDS CLARIFICATION]` markers for ambiguous or underspecified requirements.
These markers are detected during validation and block the `approved` status
transition until resolved.

### 10.10 Production Feedback Loop

`/spec feedback <spec-id> <note>` appends advisory notes to
`specs/<spec-id>/FEEDBACK.md`. When the `sdd.feedback_loop` config flag is
enabled, these notes are automatically surfaced in the `/spec plan` prompt
so that production feedback (metrics, incidents, user reports) informs plan
regeneration.

### 10.10a Task Status Symbols and Coverage Report

Task statuses carry shared ASCII symbols from `TaskStatus::symbol()`:
`[wait]` pending, `[sync]` in-progress, `[ok]` completed, `[stop]` blocked.
The `/spec coverage <id>` command and the `spec_coverage` agent tool both
render the requirement-coverage report through a single shared
`Spec::coverage_report()` implementation (overall coverage percentage, a
per-requirement `[ok]`/`[  ]` line with linked-task completion counts, and a
per-task line with status symbol and linked requirement IDs), so the two
consumers can never drift apart.

### 10.10b Spec File Write Semantics

Spec files are written atomically (unique temp file, flush + sync, rename), and
auxiliary review files follow a clear-on-empty rule: when `REVIEW.md` or
`FEEDBACK.md` content becomes empty the file is deleted rather than written as
an empty placeholder. Frontmatter regeneration via `update_frontmatter`
preserves keys the lifecycle manager does not model (e.g. `research:`
linkage), quoting values that contain YAML-special characters.

### 10.10c Automatic Task Completion on Spec-Directory Writes

When an agent turn writes files inside the active spec's directory
(`specs/<id>/`) and the session has an active spec, tasks in the `in_progress`
state are automatically advanced to `completed` with a completion timestamp.
The heuristic is guarded by `writes_in_spec_dir`: only file-writing tools
(`Write`, `Edit`, `multiedit`, `patch`, `apply_patch`, `create`,
`append_to_file`) whose resolved target path sits inside the spec directory
count. Writes elsewhere in the workspace (docs, scratch files, snapshots)
never complete spec tasks, preventing unrelated edits from silently corrupting
`PLAN.md`.

### 10.10d Session Task Tracker Sync

`spec_task_update` calls are mirrored into the session task tracker: a
tracker task tagged with the same `spec_id`/`spec_task_id` metadata is
updated when it exists, or created (seeded from the PLAN.md task row,
status mapped from the spec task status) when it does not. The
`/spec impl` driver no longer pre-creates milestone tracker rows; the
tracker is populated lazily by the agent's `spec_task_update` calls.

### 10.11 Constitutional Amendment Process

When a spec directory contains a `CONSTITUTION.md`, amendments require
explicit rationale documentation, a backwards-compatibility assessment, and
a dated changelog entry within the file. The constitution parser validates
the nine-article structure and the amendment process.

### 10.12 Project Scaffolding (`/new`)

The `/new` slash command scaffolds a new project in the current working
directory (spec `newproj`); see section 11A for the full behaviour. The
pipeline is flag validation -> empty-directory guard -> no-silent-overwrite
file emission -> git init + initial commit -> optional GitHub/GitLab remote
create + push with failure containment -> summary report. In the TUI the
blocking emit/git/remote steps run on a worker thread while streamed
progress lines update a single in-place progress message.

---

# Part III: Data & Knowledge Systems (continued)

## 11. Research System

### 11.1 Overview

The `/research` slash command and `ragent research` CLI create structured
research items under `research/<name>/`. Each item contains captured sources
(web pages, local files, prior specs) and a single `RESEARCH.md` document.

### 11.2 Source Gathering

A research session runs in phases:

1. **Setup** — validate name and create the skeleton `RESEARCH.md`.
2. **Web** — issue `websearch` queries and fetch pages via `webfetch`. Every
   HTML page captured as a web source must have been extracted by the
   `readability-rs` crate (the research web-gather phase verifies the
   `extraction_method` signal reported by `mf_fetch`, or re-runs readability
   on the raw HTML when the legacy `webfetch` tool is used). Pages where
   readability extraction failed — and would only be available via the
   html2text / raw tag-strip fallbacks — are rejected with a
   `readability extraction failed …` fetch error instead of being accepted.
   `github.com/{owner}/{repo}/blob/…` file-view URLs are rewritten to their
   `raw.githubusercontent.com` counterparts before fetching, since the blob
   page is GitHub application chrome that readability cannot extract as an
   article. PDF and YouTube sources bypass readability entirely by design,
   as do non-HTML content types (e.g. `text/plain` served by
   `raw.githubusercontent.com`) — their bodies are the verbatim document
   content, so no fallback extraction is involved. YouTube
   sources are captured as the video's caption transcript: `mf_fetch`
   parses the watch page's `ytInitialPlayerResponse` (brace-balanced,
   string-aware JSON extraction), reads the caption tracks from
   `captions.playerCaptionsTracklistRenderer.captionTracks`, and fetches
   the default (or first English) track. A fetch that fails at the tool
   level (e.g. `mf_fetch` reporting `content_ok = false` or an `error`,
   such as "no caption tracks available for this YouTube video") is
   rejected as an explicit `FetchFailed` event carrying the real reason —
   placeholder error text never enters the research corpus as a source
   body.
3. **Local** — scan project files with `Glob`/`Grep`/`read`. Each captured
   file produces an excerpt showing the matching lines plus one line of
   context on either side, and a `relevance` note that names the matched
   keyword(s) and the first matching line.
4. **Specs** — cross-reference prior specs under `specs/`.
5. **Synthesize** — (TUI only, when an active model is configured) send the
   captured source bodies to the LLM and ask for Summary, Findings,
   In-Project Cross-References, and Open Questions. The session emits a
   `SynthesizeResult` event so the UI can distinguish LLM-synthesized output
   from the mechanical fallback.
6. **Assemble** — combine frontmatter, analysis, and References Index.
7. **Finalize** — mark the item `Complete` and refresh `research/INDEX.md`.

Supporting files under `research/<name>/sources/` (e.g. `web-01.md`,
`local-02.md`) contain the **actual** captured body — web pages render into
plain text, local files become keyword-anchored excerpts — so a reader can
audit the evidence without re-running the search.

### 11.2.1 Execution Modes and Per-Page Summarisation

The `--mode` flag selects the high-level research strategy while reusing
the same gathering and synthesis pipeline. Supported modes are:

| Mode | Description |
|------|-------------|
| `tiered` | Default single adversarial pipeline. |
| `supervisor` | Supervisor/researcher graph: a lead supervisor spawns parallel sub-topic researchers (FR-009). |
| `competitive` | Supervisor graph specialised for comparing a set of entities. Defaults to `--format comparison-table` unless an explicit `--format` is supplied (FR-014). |

`--max-concurrent-research-units N` caps the number of parallel
researcher agents in `supervisor` and `competitive` modes. The default is
5 and can be changed in `ragent.json` under `research.supervisor.max_concurrent_research_units`.

`--summarization-model <provider:model>` selects a lightweight model for
per-page webpage summaries. When omitted, the active default model is
used with a short-output limit. This is independent of the per-phase
overrides `--research-model`, `--compression-model`, and `--final-report-model` (FR-013).

`--no-clarify` (default) and `--clarify` control whether the pipeline
asks a single clarifying question when the topic looks ambiguous (FR-005,
FR-017). Clarification is disabled by default; `--clarify` opts in.
`--brief <TEXT>` supplies an explicit research brief used as
the mission statement for synthesis (FR-004).

`--evaluate` appends a deterministic self-evaluation scorecard
(quality, relevance, groundedness, completeness, structure) to the
assembled `RESEARCH.md` (FR-008, FR-015). It can also be enabled by
default via `research.evaluate.enabled: true`.

### 11.3 AI-Driven Synthesis

When the TUI builds a `ResearchSession` with an active provider/model, an
`LlmAnalysisEngine` is wired in. The engine sends a single prompt containing
truncated source bodies and requests structured markdown sections. The result
is parsed and used to populate the final `RESEARCH.md`.

If the LLM call fails or returns empty output, the session falls back to a
**mechanical digest** that is still useful:

- **Summary** — names the captured web titles (top 3), local file paths
  (top 3), and cross-referenced specs, and is transparent that no LLM
  analysis was applied.
- **Findings** — one bullet per source with the matched keywords, an
  excerpt, and a relevance note.
- **In-Project Cross-References** — table of local files with relevance
  notes.
- **Open Questions** — suggests gaps (missing web sources, missing local
  files, etc.) and recommends re-running with a configured LLM.

Synthesis failures are logged at `error` level (not `warn`) and surfaced via
the `SynthesizeResult { outcome: FallbackError, detail }` event so the user
can see why the LLM path failed.
### 11.4 Output Document

Every `RESEARCH.md` contains:

- YAML frontmatter with `name`, `title`, `status`, `created`, `modified`, `sources`
  (the `title` is a reduced-length version of the `## Summary` content, capped at
  80 characters, so the headline reflects the synthesis rather than the original
  prompt) plus an optional `invocation` line recording the verbatim front-end
  command (`ragent research create ...` from the CLI, `/research ...` from the
  TUI, or a `POST /research` summary from the HTTP API) so the
  `/research update` command can replay the run
  - `# Title:` heading
  - `## Topic`
  - `## Summary`
  - `## Findings` (numbered)
  - `## In-Project Cross-References` (table)
  - `## Open Questions`
  - `## References Index` (table)

  Section order in the report layout: Topic → Search Queries → Executive
  Summary → **Top 10 Implications → Open Questions** → Findings → Findings
  Relationship Diagram → In-Project Cross-References → References Index.
  Open Questions sits directly under Top 10 Implications so unresolved gaps
  surface immediately after the ranked consequences.

  The quality-assurance render sections (Contradiction Graph, Loci Analysis,
  Depth Investigation, Cross-Locus Reconcile, Source Tensions, Synthesis
  Audit, Corpus Critic) are written to a per-research companion file
  `research/<name>/CORPA.md` instead of `RESEARCH.md`. The companion is
  created alongside the skeleton and rewritten on every `write_document`;
  it ends with a `## Sources Reference` copy of the References Index table so
  `[#N]` source indices resolve in both files.

  When an LLM is configured, `/research create` also runs a concept-extraction
  step (spec researchcluster) after synthesis: the gathered corpus is assembled
  into a context-bounded payload whose blocks are headed `--- [#N] Title ---`
  with `N` equal to the References Index position, and the model returns a
  numbered concept list (`### N. Name` subsections with `[#N]` citations).
  The normalized list is rendered as `## Concepts` directly above `## Findings`
  (report layout) or `### Concepts` directly above `### Findings` inside
  Results (IMRaD layout). When no LLM engine is wired, or the extraction call
  fails or returns no sections, the section is omitted entirely and the run
  completes normally; the step is surfaced in the UI as a `concepts` `RunStep`
  event (started / completed / skipped / failed).

### 11.5 Slash Commands

| Command | Purpose |
|---------|---------|
| `/research create ... --mode tiered|supervisor|competitive` | Select research execution strategy |
| `/research create ... --summarization-model <provider:model>` | Use a lightweight model for per-page webpage summaries |
| `/research create ... --research-model <provider:model>` | Model used by sub-topic research agents |
| `/research create ... --compression-model <provider:model>` | Model used to compress intermediate findings |
| `/research create ... --final-report-model <provider:model>` | Model used to write the final report |
| `/research create ... --max-concurrent-research-units N` | Limit parallel researchers in supervisor/competitive modes |
| `/research create ... --papers` | Include academically-classified backends (OpenAlex) in the search sweep. Scholarly engines are excluded by default; config `research.exclude_academic_engines` persists the exclusion |
| `/research create ... --oa-enable` / `--no-oa` | Force open-access recovery on/off for the run, overriding `research.open_access_recovery` |
| `/research create ... --max-concepts N` | Cap the `## Concepts` list (default 5; `0` = unbounded); entries are ordered most-relevant-first before truncation |
| `/research create ... --max-findings N` | Cap the `## Findings` list (default 20; `0` = unbounded); entries are ordered most-relevant-first before truncation |
| `/research create ... --url-cloak` | Emit web source URLs in `Sources` bullets and the `References Index` (and `CORPA.md` `Sources Reference`) as defanged plain text (`hxxps://host[.]tld/…` in a code span) instead of clickable links |
| `/research create ... --age <SPAN>` | Reject web sources published longer ago than `SPAN` (`30d`, `6m` = 30 days/month, `2y` = 365 days/year). A source with no publication date is rejected too, because the caller asked for recent material. The window's start date (`now - span`, ISO `YYYY-MM-DD`) is appended to every decomposed search query as `after:YYYY-MM-DD` (`SEARCH_QUERY_AGE_OPERATOR`) so the engines steer towards material inside the window, and the same cut-off instant serves the post-fetch rejection test (`ExclusionReason::TooOld`, the `age` column of the progress table). Off by default; recorded in frontmatter and replayed by `/research update`. |
| `/research create ... --evaluate` | Append a deterministic quality scorecard to `RESEARCH.md` |
| `/research create ... --brief <TEXT>` | Provide an explicit research brief |
| `/research create ... --clarify` | Ask the single clarifying question (disabled by default) |
| `/research create <name> <topic>` | Gather sources and write `RESEARCH.md` |
| `/research create <name> <topic> --iterations N --depth shallow|standard|deep --format ...` | Iterative research with controls |
| `/research create <name> --from-url <URL>` | Fetch the URL, use it as the research subject and capture it as the primary source |
| `/research create <name> --from-file <PATH>` | Extract a local document and use it as the research subject |
| `/research create ... --use-low-relevance` | Retain low-relevance web sources |
| `/research continue <name> [message]` | Resume an in-progress research item |
| `/research update <name>` | Replay the recorded invocation for a run and overwrite `RESEARCH.md` |
| `/research list` | List research items |
| `/research open <name>` | Show the path to `RESEARCH.md` |
| `/research show <name>` | Show metadata |
| `/research search <query>` | Search across all `RESEARCH.md` files |
| `/research delete <name>` | Delete a research item |
| `/research archive <name>` | Archive a research item |

### 11.6 Iterative Research Loop (`researchext`)

The research engine now supports an iterative, stateful loop:

1. **Plan** — decompose the topic into focused sub-questions.
2. **Gather** — capture sources for pending sub-questions in parallel.
3. **Synthesize** — produce structured findings from evidence.
4. **Verify** — ensure each claim traces back to a supporting source.
5. **Critique** — score the result and detect missing-link gaps.
6. **Iterate or stop** — continue until complete, out of budget, or no longer improving.

State is persisted to `research/<name>/state.json` so sessions can be resumed with
`/research continue`. The loop emits structured events for plan updates, sub-question
status changes, source captures/failures, critic scores, verification results,
follow-up queries, and iteration completion.

### 11.7 Output Formats

The `--format` flag selects the artifact produced at the end of a session:

| Format | Description |
|---|---|
| `report` | Full multi-section `RESEARCH.md` (default). |
| `executive-summary` | One-page summary. |
| `comparison-table` | Comparison table across key entities. |
| `source-bibliography` | Standalone bibliography of all captured sources. |
| `imrad` | Introduction, Methods, Results, Discussion structure for academic-style reports. |

### 11.8 Research Tiers

The `--tier` flag selects the depth of analysis:

| Tier | Description |
|---|---|
| `light` | Minimal analysis — web gathering + mechanical digest only. |
| `full` | Full analysis pipeline — contradiction graph, loci analysis, reconcile, corpus critic, synthesis, QA (default). |
| `dissertation` | Extended pipeline — includes depth investigations, source tensions, evidence digest, and width sweeps. |

### 11.9 HTTP Research API

The research system is exposed via REST endpoints (all auth-protected):

- `GET /research` — list all research items as JSON (`{items: [...], count: N}`).
- `POST /research` — create + run a research session. Returns `202 Accepted`
  immediately with a `Location` header pointing to `GET /research/{name}/events`.
  The research run executes in a background `tokio::spawn` task.
- `GET /research/{name}` — show a single research item. Pass `?full=true` to
  include extended metadata fields (`topic`, `queries`, `output_format`,
  `model`, `mode`, `summarization_model`, `evaluate`, `brief`).
- `DELETE /research/{name}?confirm=delete-{name}` — delete a research item
  (requires confirmation token).
- `GET /research/{name}/events` — SSE stream of live research events. When no
  active run exists but the item is on disk, returns a JSON status blob. Each
  SSE event is a JSON object `{"kind": "...", "payload": {...}}` with event
  type `research`.

The `POST /research` request body mirrors `ResearchRunRequest`:
`name`, `topic`, `title`, `from_urls`, `from_files`, `sources_dir`,
`template`, `depth`, `tier`, `iterations`, `format`, `mode`,
`summarization_model`, `research_model`, `compression_model`,
`final_report_model`, `max_concurrent_research_units`, `use_local`,
`use_specs`, `use_low_relevance`, `papers`, `exclude_academic_engines`,
`open_access_recovery`, `use_pdf`, `evaluate`,
`brief`, `clarify`, `fetch_concurrency`, `local_concurrency`,
`fetch_timeout_secs`, `web_phase_timeout_secs`,
`local_phase_timeout_secs`, `search_max_retries`, `search_retry_base_delay_ms`,
`max_web_results`, `max_search_calls`,
`max_concepts`, `max_findings`,
`max_local_sources`,
`max_synthesis_sources`. `web_phase_timeout_secs` (CLI `--web-time`) defaults
to 180 seconds (`DEFAULT_WEB_PHASE_TIMEOUT_SECS`); a value of `0` disables the
deadline and allows the web phase to run to natural completion (FR-007). When
the web-gathering phase exceeds the deadline the run ingests everything gathered
so far and proceeds to analysis/synthesis with the partial source set, emitting
a single `web_deadline` `RunStep` diagnostic that carries the effective deadline
and the number of sources captured (FR-004). No new search is started
after the deadline: the decomposer call and each sub-query search-result wait
are deadline-bounded (FR-008). The fetch stage is never gated on or cancelled by
the deadline: every candidate produced by the search stage is fetched to
completion and in-flight fetches always run to completion (or their own
`fetch_timeout_secs` cap). At the
start of a web-gathering phase the system emits a `web_phase_start` `RunStep`
carrying the remaining effective deadline in seconds; the TUI renders this as a
live `web:M:SS` countdown in the status-bar wait segment, computed from a stored
wall-clock `Instant` at render time and updated at least once per second while
the phase is active (FR-009, FR-010, FR-013). The same deadline is applied to
every iteration of the iterative research engine (FR-006).

Web-search quota controls: `max_search_calls` (CLI `--max-search-calls N`)
places a hard cap on the total web-search calls a single run may issue. The
cap is shared via `Arc` across every supervisor/competitive researcher and
every gather pass (main gather, engine iterations, gap-fill), so N parallel
researchers draw from one per-run pool; when the cap is reached remaining
sub-queries are skipped, the run proceeds with the sources gathered so far,
and a `search_budget` `RunStep` diagnostic reports the used/limit counts.
Within a supervisor/competitive run, a query-result cache keyed on the
normalized query text serves identical sub-queries (common when per-entity
researchers decompose the same comparison dimensions) from the cache instead
of re-issuing paid search calls. In competitive mode the researcher count is
capped at `max_concurrent_research_units` (one sub-topic per extracted
entity, truncated to the cap). Without an explicit `--max-web-results`, the
web-source budget is derived from `--depth` (shallow 6 / standard 9 / deep 15
sources per the depth preset) instead of defaulting to the 500-source
ceiling; `--depth` therefore bounds web volume by default.

---

## 11A. Project Scaffolding (`/new`)

The `/new` slash command (and the `ragent new` CLI subcommand / `ragent run
"/new ..."` surface) scaffolds a new project in the current working
directory (spec `newproj`). The pipeline is: flag validation (`--language`
rust/python/go/typescript, `--type` library/cmdline/tui/gui/webapp, optional
`--stack`, mutually exclusive `--github`/`--gitlab`) -> empty-directory
guard (refuses non-empty targets; ragent artifacts `.ragent/`, `log/`,
`target/` are allowlisted) -> file emission with no silent overwrite
(existing files are reported and left untouched) -> local git init + initial
commit -> optional GitHub/GitLab remote create + push with failure
containment (a failed remote step never undoes the local scaffold; an
existing remote repository is reused instead of failing) -> summary report
(created layout, generated files, language/type/stack, git outcome, remote
status).

Generated content: the ragent workspace (`.ragent/config.json`,
`.ragent/agents/README.md`, `specs/README.md`, `log/`, language-specific
`.gitignore`, `AGENTS.md` stub), a runnable hello-world artifact set from
per-language recipes (`Cargo.toml`/`src/main.rs`, `pyproject.toml`/`main.py`,
`go.mod`/`main.go`, `package.json`/`src/main.ts`; `library` types swap the
binary entry for `src/lib.rs` / `src/<name>/__init__.py` / `<name>.go` /
`src/index.ts`), optional stack overlays (Rust: `axum`, `warp`, `raylib`;
unknown stacks warn and fall back to the base layout), and the documentation
scaffold (`README.md`, `QUICKSTART.md`, `STATS.md`, `docs/`) rendered from
the same recipe data as the code artifacts. In the TUI the command runs as a
foreground operation with streamed progress lines in the message window (a
worker thread performs the blocking emit/git/remote steps; no sub-agent and
no LLM turn is involved). `/new help` (and bare `/new`) prints a usage page
whose language/type value lists are derived from the scaffolder's own
registries. See `docs/howtos/newproj.md` for the user manual.

The TUI and CLI surfaces share one pipeline engine:
`project_scaffold::plan_and_emit()` in
`crates/ragent-tools-extended/src/project_scaffold/mod.rs` performs flag
validation, the empty-directory guard, file emission, stack overlay, and
git init, returning the scaffold summary plus the stack note; each surface
then attaches only its own git/hosting outcome lines to the streamed
progress. The GitHub and GitLab flows share a
`register_origin_and_push(root, url)` helper in `project_scaffold/remote.rs`.

---

# Part IV: Agent Customization & Extension

---

## 11. Custom Agents

### 11.1 Storage Locations

Custom agents can be defined in JSON using the Open Agentic Schema Framework
(OASF):

- `~/.ragent/agents/` — user-global (legacy location)
- `~/.config/ragent/agents/` — user-global (XDG config location, higher priority)
- `.ragent/agents/` — project-local (highest priority)

### 11.2 Agent Schema

```json
{
  "name": "rust-reviewer",
  "description": "Reviews Rust code for idioms and safety",
  "model": "anthropic/claude-sonnet-4-20250514",
  "system_prompt": "You are a meticulous Rust reviewer...",
  "tools": ["read", "Edit", "Bash", "codeindex_search"],
  "permissions": [
    { "permission": "file:write", "pattern": "crates/**", "action": "allow" }
  ]
}
```

### 11.3 Template Variables

System prompts and file contexts support variables:

| Variable | Expansion |
|----------|-----------|
| `{{WORKING_DIR}}` | Current working directory |
| `{{FILE_TREE}}` | Project file tree |
| `{{AGENTS_MD}}` | Discovered AGENTS.md content |
| `{{DATE}}` | Current UTC date |

### 11.4 Loading and Diagnostics

Use `/agents` to list loaded agents, view diagnostics, and confirm custom
profiles are parsed correctly. Custom agents display a `[custom]` badge in the
agent picker.

---

## 12. Skills System

### 12.1 Overview

Skills are loadable YAML packs that inject tools, system prompts, and file
context into an agent session. They are useful for domain-specific workflows
(e.g., Rust embedded development, Terraform infrastructure).

At startup the agent builds a `SkillCatalog` — a lightweight, metadata-only
list of skill names, descriptions, and trigger phrases. The catalog is cheap to
produce and lets the agent advertise available skills without loading full
skill bodies into the system prompt. Full `SkillInfo` bodies are loaded on
demand when the skill is invoked (`SkillRegistry::catalog()` for discovery,
`SkillRegistry::get()` for activation).

### 12.2 Skill Locations

- Bundled skills in `assets/skills/`
- Custom skills in `~/.ragent/skills/` or `.ragent/skills/`
- OpenSkills in `~/.agent/skills/`, `~/.agents/skills/`, and `~/.claude/skills/`
  (global), plus `.agent/skills/`, `.agents/skills/`, and `.claude/skills/`
  (project), each scanned for `<skill>/SKILL.md` under the
  `openskills-global` / `openskills-project` scopes

### 12.3 Skill Format

```yaml
---
name: rust-embedded
description: You are an expert in embedded Rust (no_std, RTIC, embassy).
allowed-tools:
  - read
  - Edit
  - Bash
  - codeindex_search
---

The skill body follows the closing `---` and is what the model receives.
```

### 12.4 Activation

Skills can be loaded via slash command: `/skills` (or `/skill` / `/skill list`)
lists the registered packs, `/skill <name> [args]` loads and invokes a named
pack, and a bare `/<name>` trigger invokes a user-invocable pack directly.
AgentSkills packs discovered under `.agents/skills/` (project) and
`~/.agents/skills/` (user) appear in the list and are loadable by their
frontmatter name; on a name clash the existing higher-scope pack is preferred.

### 12.5 Tool restrictions (`allowed-tools`)

A `SKILL.md` frontmatter `allowed-tools` key (a single string or a list) limits
the tools a skill run may call. Entries resolve by **exact** name comparison
against the `ToolRegistry`: `tool::build_allowed_tool_set` stores each entry
verbatim and the per-step filter keeps a definition whose name equals an entry.

There is deliberately **no translation layer**. Because the tool rename adopted
the Anthropic spellings (`AskUserQuestion`, `Bash`, `Edit`, `Glob`, `Grep`,
`EnterPlanMode`, `ExitPlanMode`, `Skill`, `TaskCreate`, `TaskGet`, `TaskList`,
`TaskUpdate`, `Write`), a manifest written for Claude Code resolves on the
registry lookup itself. Nothing consults the `(before, after)` rename table, so a
retired spelling is unresolved rather than mapped, and `Read` - rated too close to
`read` and not adopted - is unresolved rather than silently served by `read`.

An entry that matches no registered tool is reported once per `(entry, declarer)`
by `tool::allowed_tools::report_unresolved_allowed_tools_once`, naming the entry
and the declaring skill, from the three places a skill's list becomes a
restriction: `Skill` (`action=read|load`), the TUI `/skill` (and bare `/<name>`)
trigger, and the forked-skill resolver. The per-turn prompt path deliberately does
not report - it visits every discovered manifest, including plugin skills the
session never runs, and a per-turn scan adds cost for an entry that constrains
nothing that turn (NFR-006).

---

## 14. Teams

### 14.1 Core Concepts

A **team** is a named workspace with:

- A roster of teammates (each with an agent type and task)
- A shared task list stored in `tasks.json`
- A per-member mailbox stored in `mailbox.json`
- A team configuration in `team.json`

### 14.2 Team Lifecycle

```mermaid
graph LR
    Create[team_create] --> Spawn[team_spawn]
    Spawn --> Work[Tasks + Messages]
    Work --> Idle[team_idle]
    Work --> Shutdown[team_shutdown_teammate]
    Idle --> Cleanup[team_cleanup]
    Shutdown --> Cleanup
```

Mailbox writes are validated at the boundary: a message authored by the lead
(the `team_shutdown_teammate` shutdown request, lead-to-teammate chat) must
carry the lead session id (`MailboxMessage::with_sender_session`,
SEC-ragent-team-003), so a lead-authored message pushed without one is rejected
at the write boundary rather than silently forwarded.

### 14.3 Team Tools (20)

| Tool | Purpose |
|------|---------|
| `team_create` | Create team |
| `team_spawn` | Spawn teammate |
| `team_status` | Team status (status history, undelivered messages, pending plans, hook failures) |
| `team_message` | Direct message |
| `team_broadcast` | Broadcast to active teammates |
| `team_read_messages` | Read mailbox |
| `team_memory_read` / `team_memory_write` | Team memory access |
| `team_submit_plan` | Submit plan for approval |
| `team_approve_plan` | Lead approve/reject plan |
| `team_task_create` | Add task |
| `team_task_claim` | Claim task |
| `team_task_complete` | Complete task (owner or lead only) |
| `team_task_cancel` | Cancel a pending/in-progress task (lead only) |
| `team_assign_task` | Lead assigns task |
| `team_idle` | Signal idle |
| `team_shutdown_teammate` | Shutdown teammate (graceful/immediate) |
| `team_shutdown_ack` | Acknowledge shutdown |
| `team_wait` | Block until teammates settle |
| `team_cleanup` | Remove team state |

### 14.4 Team Slash Commands

| Command | Purpose |
|---------|---------|
| `/team create <name>` | Create a new team |
| `/team open <name>` | Open an existing team |
| `/team close` | Close current team |
| `/team clear` | Reset current team state |
| `/team delete <name>` | Delete a team |
| `/team cleanup` | Tear down current team |
| `/team message <to> <text>` | Send direct message |

### 14.5 Task Dependencies

Tasks can declare `depends_on` task IDs. A task cannot be claimed until all its
dependencies are completed.

### 14.6 Plan Approval Workflow

1. Teammate calls `team_submit_plan`.
2. Lead reviews and calls `team_approve_plan` (approved=true/false).
3. On approval, teammate proceeds with implementation.

### 14.7 Blueprints

#### Blueprint Storage Locations

- `~/.ragent/blueprints/teams/<name>/`
- `~/.config/ragent/blueprints/teams/<name>/` (higher priority than the legacy location)
- `.ragent/blueprints/teams/<name>/` (highest priority)

#### Blueprint Directory Structure

```
<blueprint>/
├── spawn-prompts.json   # Agent type definitions
├── task-seed.json       # Initial task list
└── context.md           # Shared context
```

#### spawn-prompts.json Format

```json
{
  "agents": [
    {
      "name": "security-reviewer",
      "agent_type": "general",
      "prompt": "Review the target files for security issues..."
    }
  ]
}
```

#### task-seed.json Format

```json
{
  "tasks": [
    { "title": "Scan for injection risks", "description": "..." }
  ]
}
```

#### Blueprint Slash Commands

| Command | Purpose |
|---------|---------|
| `/team create <name> --blueprint <bp>` | Create team from blueprint |

#### Work Context Propagation

When a team is created with a blueprint, the work context is prepended to every
teammate's spawn prompt. This context includes target directories, expected
outputs, and any constraints.

#### Blueprint Best Practices

- Keep prompts under ~500 words.
- Reference files by path rather than pasting content.
- Spawn one teammate per independent work item.
- Use `/team_wait` after spawning to avoid racing ahead.

### 14.8 Team state machines and their on-disk record

Four rules the team runtime now enforces rather than merely documents (spec
`teamssetup`). Each is tightening-only: no capability is removed, and every
refusal is visible in the event stream and the tool result rather than silent.

#### 14.8.1 One member-status writer

`TeamManager::set_member_status` is the single writer of a member's status. It
validates the move against the member status transition table, applies the
destination status's side effects, persists the roster once, appends one record
to the team's `status-history.jsonl`, and publishes exactly one
`Event::TeammateStatusChanged { from, to, cause }`. Nine direct
`member.status = ...` sites were consolidated onto it, so a transition cannot be
persisted without being logged and cannot be logged without being published.

`MemberStatus` exposes the predicates every guard now uses instead of
re-spelling a status list: `is_terminal` (`Stopped`, `Failed` - no session
behind it, only a respawn revives it; `ShuttingDown` is deliberately **not**
terminal because its session still exists and is awaiting a shutdown ack),
`is_parked` (`Idle`, `PlanPending`, `Blocked`, `Suspended` - alive but not
attached to work, so time spent there is not evidence of a wedge and the
watchdog must skip it) and `is_live` (`Spawning`, `Working`, `ShuttingDown`).

`status-history.jsonl` carries one JSON object per line
(`{timestamp, agent_id, name, from, to, cause}`), so `team_status` can answer
"why did this teammate stop?" *after* the events that answer it have gone past -
a tool call is not a listener. The file is append-only and bounded: before an
append that would pass `MAX_HISTORY_BYTES` it is renamed whole to
`status-history.jsonl.1` and the new record starts a fresh live file.
`team_status` renders the last `STATUS_HISTORY_LIMIT` (5) transitions per member,
oldest first, under a `Status history` block beneath each roster row.

#### 14.8.2 One task-status transition table

`TaskStatus::can_transition_to` and `transition_task` are the single writer of
`Task::status`, so the four claim paths and both completion paths cannot
disagree about which moves are possible, and an illegal move is refused with a
message naming the task, its actual status, and the table row that forbids it.

`TaskList::validate()` runs before every persisted write (`TaskStore::write_locked`,
the one function every list passes through, cached or not) and refuses duplicate
task ids, a `depends_on` edge naming no task in the list, and a dependency cycle
(detected by a three-colour iterative DFS), naming the offending task, the
missing id plus the ids that exist, or the closing chain
(`task-001 -> task-002 -> task-001`). The refusal is prefixed
`refusing to write invalid <path>: ...` and leaves `tasks.json`
byte-identical, so a task that could never be claimed is never persisted.

#### 14.8.3 Completion requires ownership, adoption requires consent

`team_task_complete` accepts a completion only from the task's `assigned_to`
agent or the lead; a task assigned elsewhere is refused and **not**
re-assigned, with `rejection_reason` set to `not_owner`, `not_claimed` (still
`pending`) or `already_completed`, and the current `owner` reported alongside.
Every refusal leaves the task list untouched. A `TeamTaskCompletionRejected`
event carries the same facts, so a remote consumer sees the refusal and the rule
that fired.

A second session that opens a team directory must take the team over explicitly
(`AdoptTeam::Adopt`, i.e. `adopt: true`); without it the session gets an error
naming the owning session rather than quietly competing for the same task list.
A successful adoption publishes `TeamAdopted` with the previous lead session id
and the task ids the adoption reset.

#### 14.8.4 Hook failures are recorded, never mistaken for approval

A configured quality-gate hook that cannot run - its command fails
`validate_hook_command`, fails to spawn, or exceeds the 30-second
`HOOK_TIMEOUT` - appends one JSON object per line to the team's
`hook-failures.jsonl` (`{timestamp, event, hook, reason, detail}`, with `reason`
one of `invalid`, `spawn_failed`, `timeout`). The file is append-only and
bounded: at 1 MiB it is rotated whole to `hook-failures.jsonl.1` (replacing the
previous generation). Recording is best-effort, so the ledger can never change a
hook's verdict - but the count is surfaced in `/team show <name>` and
`team_status`, so an inert hook is never mistaken for one that ran and approved.

#### 14.8.5 Parked members and blocked exits

The watchdog's eligibility test is `MemberStatus::is_live()`, so a parked member
is never failed for taking no progress, and a team whose config cannot be read
yields "not live" rather than being failed on a guess. `resume` accepts a
`Blocked` member as well as a `Suspended` one, and when a task completes every
`Blocked` member whose last unmet dependency is now satisfied is moved to `idle`
automatically and published as `TeammateResumed`, so the common case needs no
operator verb. `team_wait` reports parked members distinctly instead of waiting
out its clock on them, and `team_cleanup`'s guard blocks only on live or
shutting-down members (parked and terminal members never block; `force: true` is
the only override, and `/team cleanup` and `/team delete` share the guard).

---

## 15. Swarm Mode

### 15.1 How Swarm Works

The `/swarm <prompt>` slash command decomposes a user request into parallel
subtasks and spawns multiple agents to work on them concurrently. A synthesis
agent combines the outputs into a final answer.

### 15.2 Swarm Slash Commands

| Command | Purpose |
|---------|---------|
| `/swarm <prompt>` | Decompose and run parallel swarm |
| `/swarm status` | Show active swarm tasks |
| `/swarm kill` | Cancel active swarm |

### 15.3 Decomposition Format

Swarm decomposition returns a JSON or Markdown list of subtasks:

```json
{
  "tasks": [
    { "id": "1", "agent": "explore", "prompt": "Find all auth-related files" },
    { "id": "2", "agent": "code-review", "prompt": "Review auth logic for issues" }
  ]
}
```

### 15.4 Swarm State

Swarm state is tracked in memory and via the team store when teammates are
used. The TUI active-agents panel shows per-agent status and progress.

### 15.5 Limitations

- Swarm requires a provider with enough throughput for parallel calls.
- Cloud providers may hit rate limits; exponential retry backoff mitigates this.
- Synthesis depends on the quality of subtask outputs.

---

## 16. Autopilot Mode

### 16.1 How Autopilot Works

Autopilot runs the agent loop without requiring user approval for each step. It
is useful for long-running, low-risk tasks.

### 16.2 Autopilot Slash Commands

| Command | Purpose |
|---------|---------|
| `/autopilot on` | Enable autopilot |
| `/autopilot off` | Disable autopilot |
| `/autopilot on --max-tokens N` | Enable with token budget |
| `/autopilot on --max-time N` | Enable with time budget (seconds) |

### 16.3 Safety Limits

| Limit | Default | Description |
|-------|---------|-------------|
| Max iterations | 50 | Hard cap on tool/turn loops |
| Max tokens | configurable | Total token budget |
| Max time | configurable | Wall-clock budget |
| Permissions | auto-approve | Only within allowed rule set |

### 16.4 Completion Signalling

Autopilot ends when:

- The agent calls `agent_complete(summary)`.
- A safety limit is reached.
- The user sends `/autopilot off` or `Esc`.

### 16.4.1 `new_agent` parameter contract

`new_agent` spawns a sub-agent with a bounded task. The sub-agent MUST finish
with `agent_complete(summary)` as its final action — every sub-agent run,
without exception (including failed or empty runs, where the summary describes
what was attempted). The sub-agent system prompt carries a mandatory
"Sub-Agent Completion Protocol" section enforcing this; a run that ends
without the call leaves the task entry running and stalls `wait_agents`.
The mandatory section title itself is "Sub-Agent Completion Protocol
(MANDATORY - HARD REQUIREMENT)" and the per-turn `agent_complete` tool
description repeats the requirement so it is visible in the tool
reference as well.

A separate "Deliverable Enforcement (FILE-WRITE TASKS)" section requires a
sub-agent whose prompt names a file to call `Write`/`create` and verify the
file BEFORE `agent_complete`; an `agent_complete` whose promised file was
never written is a failed run. Every completed sub-agent run (detached or
not) also persists its FULL output to `log/subagents/<task-id>.md`, and the
`SubagentComplete` event carries the real loop `finish_reason`
(`stop`/`truncation`/`length`/`cancelled`/`error`) recorded by the session
loop so a provider-side cut is reported as truncation, not a healthy finish.
A provider that reports a **tool-use finish with no parsed tool call** is
treated the same way (spec `uniformtools` FR-022): the step is reclassified as
`Truncation` so the text-format tool-call recovery gets its chance, and a
reply with no recoverable markup ends the run with the "reply may be
incomplete" notice instead of being presented as a clean answer.
`new_agent` selects exactly one of three execution contracts from
`background` and `detached` (spec `agentssetup` FR-001, FR-003): *foreground*
(`background` absent or `false`, blocking, response returned inline),
*background* (`background: true, detached: false`) and *detached*
(`background: true, detached: true`). The contract is derived once into an
`Execution` enum so no code path can silently degrade a detached request. A
call with `detached: true` and `background` absent or `false` is **refused**
with an error naming both parameters and the two legal combinations: there is
no detached-foreground contract. A detached task is excluded from
`list_agents` and `running_background_count`,
cannot be awaited with `wait_agents`, and its completion is reaped without a
chat injection while remaining visible in the Agents panel and
`tasks_snapshot`.

The Agents panel paints that contract per row (spec `agentssetup` FR-020). The
`Event::SubagentStart` variant carries `detached` alongside `background`, the
task registry fills both from the entry it actually spawned, and the TUI stores
them on the painted row, so the panel's type column reads `fg`, `bg` or
`detached` from the registry instead of inferring it from `background` (which a
detached run also sets). The flag travels to remote consumers in the SSE
`subagent_start` payload for the same reason.

**The system prompt teaches the same contract (FR-025).** The `## Sub-Agent
Spawning` block a *primary* agent receives carries four named teaching
obligations, in the tool descriptions' vocabulary rather than a paraphrase of
it, so a model that plans a batch is not left to infer the rules from a refusal:

- **Three execution contracts.** Each is named with the `background` /
  `detached` shape that selects it (`background` absent or `false`;
  `background: true, detached: false`; `background: true, detached: true`), the
  background contract's three consequences are stated (listed by `list_agents`,
  awaitable with `wait_agents`, injected on completion) against the detached
  contract's inverse (fire-and-forget), and the illegal combination is named as
  **refused** rather than degraded.
- **The limit, its occupancy, and how to free a slot.** The block states the
  configured limit (substituted for the `MAX_BG_TASKS` placeholder from
  `experimental.max_background_agents`, so the prose and the enforcement cannot
  drift), that occupancy counts `running`, `suspended` **and** `terminating`
  background tasks while never counting a detached or foreground one, and the
  cancel-to-free rule in those words: *to free a slot, CANCEL - do not suspend*,
  because suspending frees no headroom (FR-030). It also says a cancel is only
  *requested* when `cancel_agent` returns, with the slot freeing when the
  registry reaches the terminal `cancelled` status (FR-013), and points at
  `list_agents`' `limit` / `occupancy` / `headroom` report, which describes the
  whole session even under a `status` filter (FR-011, FR-035).
- **The await contract.** `wait_agents` blocks until the awaited tasks settle;
  `task_ids` and `timeout_secs` are both named; every settled task reports its
  canonical registry status (`completed`, `failed`, `cancelled`, `timed_out`)
  so a cancelled task is never reported as a failure (FR-014); `suspended` is
  *not* settled and `timed_out` *is* (FR-015, FR-017); and a detached task can
  never be awaited (FR-044) while a foreground task is not awaited at all
  because it returned inline.
- **The durable report path.** `log/subagents/<task-id>.md`, surfaced as
  `output_file` in the `wait_agents` output, its metadata `results` entries and
  the `list_agents` table, plus the absence case: a finished task with no durable
  copy is reported as in-memory only rather than having the field omitted
  (FR-018, FR-032), so a caller never reads a path that does not exist.

**Durable-copy absence is a reported fact, not a missing field (FR-018, FR-032,
FR-043).** `persist_task_output` returns `None` when the report cannot be written
(an unwritable or read-only `log/subagents/`, a full disk, a failed rename). The
task still completes with its result in memory, so before this change the caller
saw only the *absence* of `output_file` and could not tell "the write failed and
the only copy is in memory" from "this task has no report" - a model reading the
first shape as the second `read`s a path that was never created and loses the
findings when the text is truncated. Both surfaces now name the condition through
one shared constant, `ragent_agent::tool::wait_agents::IN_MEMORY_REPORT_NOTE`:

- `list_agents` list rows print `-> [memory] <status> - report is <note>` where a
  persisted task prints `-> [file] Full report: <path>`, and the detail view
  prints `Output File: none - <note>` instead of the `Output File (full
  untruncated report)` heading it prints for a real path (FR-032).
- `wait_agents` prints `[memory] No durable copy: <note>` in the content where a
  persisted task prints `[file] Full report: ...`, and its `results` entries
  carry `report_persisted: false` plus `report_location: "in_memory_only"` (with
  `output_file` present and `null`, so a presence check cannot mistake the
  absence for a shape change); the `tasks` roll-up repeats the flag (FR-043).
- Only a *settled* task reports the verdict: a running or suspended task has not
  had a write attempted yet, so neither surface claims one failed. The verdict
  is derived in `durable_copy_absence_note`, which both tools call, so the two
  cannot drift into describing the same condition differently.

**A detached task is refused by contract, not reported as unknown (FR-044).** A
detached task (`new_agent` with `background: true, detached: true`) is
fire-and-forget: `AgentManager::list_agents` excludes it, `wait_agents`' wait set
is seeded from that listing, and nothing ever injects its completion into the
session - so a wait on its id can never settle. Before this change such an id
fell through the settled-set scan, the wait set came out empty, and the caller
was told "No running background tasks to wait for": the tool described the *one*
thing that was not wrong (the id is real) and named no way forward.

`wait_agents` now checks every requested id against the whole registry -
`list_agents` cannot be asked, precisely because it hides detached entries -
before it subscribes to the event bus or counts a waiter, so a refused call
leaves the registry exactly as it found it. A detached id returns an error
carrying the machine-readable reason `task_detached` (`REASON_TASK_DETACHED`)
whose message names, in one pass: the task (short id, full id, agent name), the
contract as the spawn shape that produced it (`background: true, detached:
true`), why a wait cannot settle (excluded from `list_agents`, never awaited,
never injected), and the durable report file `log/subagents/<task-id>.md` - the
recorded `output_file` when the run has finished, the expected path when it has
not - together with the `read` tool call that recovers it. It ends by naming
both ways forward: `list_agents` for the tasks this session *can* await, and
`new_agent(background: true, detached: false)` for a sub-agent whose result
should come back here. The word "unknown" appears nowhere in it, which is the
distinction the requirement is about.

The refusal is scoped to `detached: true`: an ordinary background task is
awaitable exactly as before, and a detached id listed alongside an awaitable one
refuses the whole call rather than awaiting the half it can see.

The block is scoped to `AgentMode::Primary`; a sub-agent receives the completion
protocol instead and cannot spawn. Because the block is rendered prose it cannot
be guarded by FR-016's schema-scan, so
`test_subagent_prompt_contract::the_prompt_uses_the_same_vocabulary_as_the_tool_descriptions`
asserts agreement in both directions against the *live* `description()` strings
rather than a hand-copied word list: a term the descriptions teach and the prompt
does not (or vice versa) is a build failure. The four teaching obligations are
pinned by `::the_prompt_teaches_the_three_execution_contracts`,
`::the_prompt_teaches_the_limit_and_how_a_slot_is_freed`,
`::the_prompt_teaches_the_await_contract` and
`::the_prompt_teaches_the_durable_report_path`, and the primary-only scoping by
`::the_contract_block_is_scoped_to_primary_agents`.

Every tool in the family (`new_agent`, `cancel_agent`, `list_agents`,
`wait_agents`) and `bg` declares a closed parameter set
(`additionalProperties: false`) and enforces it through one shared validator,
`ragent_agent::tool::param_schema::validate_tool_input` (spec `agentssetup`
FR-004, FR-038). A call carrying a property the schema does not declare is
refused with an error naming every unknown key and listing the declared ones,
instead of being silently ignored - a misspelled `backgound: true` no longer
degrades to the foreground contract. The check runs first in each tool's
`execute`, before any `AgentManager` / service lookup, so a bad call shape is
reported as a bad call shape rather than as a missing environment. `bg`
additionally refuses a *declared* property its selected action does not read
(`lines` on `status`, `timeout` on `spawn`), and `wait_agents` refuses a
`task_ids` element that is not a string instead of dropping it, so every
declared parameter is honoured or refused (FR-002).

The optional `model` override accepts one documented spelling set
(`provider/model` and `provider:model`), and every surface that documents it
says the same thing (FR-006). `provider/model` is **preferred**: the resolver
tries `/` before `:`, and a string carrying neither separator is ignored with a
warning rather than guessed at. The four surfaces are the `model` property's
schema description, the `new_agent` tool description, the primary-agent system
prompt, and `README.md`; `test_model_override_spellings.rs` pins the first
three so drift becomes a build failure, and the same test asserts the two
documented shapes are the ones the resolver accepts.

Every precondition failure in the family is an **error**, not a successful
result whose content is prose (FR-009): a caller cannot mistake "you may not do
this" for an answer. In particular, `new_agent` called from a session with an
active team context is refused with the machine-readable reason
`team_context_active` (FR-045, FR-009). The refusal names the team and this
session's role and keeps the previous guidance text verbatim: a lead is pointed
at `team_spawn`, `team_task_create`, `team_assign_task` and `team_message`; a
teammate at `team_read_messages`, `team_task_claim`, `team_task_complete` and
`team_idle`. No sub-agent task starts.

**Reason codes (FR-010, NFR-005).** Every sub-agent tool failure a caller could
branch on carries a stable machine-readable code beside its human message, so a
caller never has to pattern-match English prose:

| Code | Raised by | Condition |
|------|-----------|-----------|
| `limit_reached` | `new_agent` (via the registry) | the background limit is full |
| `contract_refused` | `new_agent` | `detached: true` without `background: true` |
| `team_context_active` | `new_agent` | a team context owns this session |
| `invalid_timeout_secs` | `new_agent` | `timeout_secs` is not a positive integer |
| `manager_unavailable` | all four | the tool ran where no `AgentManager` is wired |
| `task_not_found` | `list_agents`, `cancel_agent` | the id names no task for this session |
| `wrong_session` | `cancel_agent` | the task exists but belongs to another session |
| `cancel_failed` | `cancel_agent` | the registry rejected the cancel request |
| `invalid_status_filter` | `list_agents` | `status` is outside the declared set |
| `no_tasks` | `list_agents` | the listing matched nothing |
| `invalid_task_ids` | `wait_agents` | `task_ids` is not an array of strings |
| `wait_timed_out` / `wait_settled` | `wait_agents` | the wait outcome |
| `unknown_action` / `parameter_not_accepted` / `service_unavailable` | `bg` | the shell-task tool's own refusals |

Codes travel in the shape the failure takes. An **error** carries it at the head
of its message (`reason_code: <code>: <explanation>`), read back with
`tool::param_schema::reason_in`; a failure reported as a **result** (the
`cancel_agent` unknown-task case, an empty listing, the wait outcome) carries it
as `metadata.reason`. The task registry prefixes its own code with
`param_schema::prefixed` and `new_agent` lifts it into the tool-error envelope,
so one code is visible whichever layer refused. `bg`'s unknown-action refusal
also names the supported action set, and every refusal names at least one
supported next action (NFR-005).

**The limit is reported, not just enforced (FR-011, FR-029, FR-035, NFR-003).**
`AgentManager::limit_report()` returns one
`BackgroundLimitReport { limit, occupancy, headroom, occupying_ids }` from the
same `DashMap` scan the spawn path already performs, counting through the single
`occupies_limit` predicate that also defines `running_background_count`, so a
listing and a spawn refusal can never disagree and no storage read is added.
Occupancy counts `running` **and** `suspended` background tasks (a suspended task
keeps its wall-clock budget and can resume, FR-030), excludes `detached` ones, and
is manager-wide because the limit is. `headroom` saturates at zero, so a limit
lowered below the occupancy never reports a negative number of free slots.
Suspending a task therefore frees **no** headroom: the spawn refusal for a full
limit names the suspended task among the occupying ids and says
`suspending does not free a slot`, so cancellation - not suspension - is the
supported way to free one early.

`list_agents` carries `limit`, `occupancy`, `headroom` and `occupying_task_ids`
in the metadata of **every** exit path - populated listing, empty listing, detail
hit and detail miss - and prints one content line,
`Background limit: 1/2 slots in use, headroom 1. Occupying tasks: <id>, ...`
(the `Occupying tasks` clause appears only while a slot is in use), so a model
reading only prose still sees the headroom. The figures are computed **before** the
`status` filter is applied, so a filtered listing describes the whole session
rather than the subset it shows (FR-035). When `new_agent` is refused for the
limit, the error names the limit, the occupancy, the headroom and the occupying
task ids as well as its `limit_reached` code (FR-029), so the caller can cancel a
specific task instead of guessing. The system prompt's concurrency block points at
the headroom for the same reason. Covered by
`test_task_registry_bookkeeping::limit_report_counts_running_and_suspended_background_tasks`,
`::spawn_refusal_names_limit_occupancy_and_occupying_tasks`,
`test_list_agents_tool::limit_occupancy_and_headroom_are_reported`,
`::limit_report_composes_with_a_status_filter` and
`::detail_mode_reports_the_limit_on_hit_and_miss`, and
`test_task_suspend_resume::suspended_task_still_occupies_the_limit` drives the
real `suspend_task` transition and asserts occupancy is unchanged by it.

`waiter_count` - the registry's tally of in-flight `wait_agents` callers - is
reported per task by `list_agents` (FR-012): in a listing row
(`-> N active waiter(s)`), in the `task_id` detail view (`Active Waiters: N`)
and in the detail metadata (`waiter_count`), and on the matching Agents-panel row
as a `[wN]` suffix. It is reported **only when greater than zero** and only for a
task that has not finished, so a task nobody awaits never paints a `0` that reads
as a defect and a completed task's leftover tally is not mistaken for a live
queue. The Agents panel takes the count from the registry snapshot it already
merges, so a suspended task keeps showing its queued waiters. Covered by
`test_list_agents_tool::detail_mode_reports_the_waiter_count_of_an_awaited_live_task`,
`::detail_mode_omits_the_waiter_count_when_nobody_is_waiting`,
`::detail_mode_drops_the_waiter_count_after_the_wait_returns`,
`::list_mode_marks_only_the_awaited_live_task`,
`::a_finished_task_never_reports_waiters`,
`test_agents_window_table::an_awaited_live_row_paints_its_waiter_count`,
`::an_unawaited_row_paints_no_waiter_marker` and
`::a_suspended_awaited_row_still_paints_its_waiters`. The waiter marker is fitted
into the label column, so
`::the_waiter_marker_does_not_move_the_pinned_controls` asserts the `[stop]` and
`[x]` columns stay pinned after it.

Every settled task carries a `status` string that is the canonical
`TaskStatus::as_str()` label the registry recorded - `completed`, `failed`,
`cancelled` or `timed_out` - in `wait_agents` content (`[...] **<agent>** (task
<id>, <status>)`), in its `results[]` entries and in its `tasks[]` entries
(FR-007, FR-014). The status is taken from the registry entry, **not** re-derived
from the `success` boolean: a task that was cancelled is reported as `cancelled`
and one that failed as `failed`, so the two terminal states never collapse into
one label. The same rule applies on the event path - a `SubagentComplete` carrying
`success: false` whose entry is `Cancelled` reports `cancelled`, because the
registry, not the event, is what distinguishes them. A cancelled task publishes
`SubagentCancelled` rather than `SubagentComplete`, so it reaches the wait only
through the registry scan; that asymmetry is exactly why the label is sourced
from the entry. `success` is kept alongside `status` as its boolean refinement
(and always agrees with it) so callers written against the older metadata shape
keep working. The implicit wait seeds every settled background task, matching on
"terminal" (`is_settled`) instead of a hardcoded status list, so a task that hit
its per-call time bound (FR-017) is reported like one that was cancelled or
failed rather than silently vanishing from the count. Covered by
`test_wait_agents_results_meta::test_wait_agents_reports_cancelled_not_failed`,
`::test_wait_agents_status_uses_the_canonical_task_status_vocabulary`,
`::test_wait_agents_keeps_success_boolean_as_a_refinement` and
`::test_wait_agents_event_path_prefers_registry_status_over_the_event_boolean`.

A task that has **not** settled when the wait returns is reported by its real
state too, so a parked task is never described as running (FR-015). The
partial-result headline separates the two: `N task(s) still running: <ids>` names
only tasks that are actually working, and `N task(s) suspended (parked - nothing
progresses until they are resumed or cancelled): <ids>` names the parked ones; a
wait holding only running tasks keeps the original wording. The metadata splits
the same way, with `still_running` / `still_running_count` carrying only live
work and `suspended` / `suspended_count` carrying the parked ids, while
`unsettled` lists every unsettled task with its canonical `status` (`suspended`
for a parked one). This required two corrections inside the wait: the
pre-subscription collection and the periodic scan both treated "not `Running`"
as "settled", which collected a parked task as an empty completion, and
`AgentManager::increment_waiter` refused a non-`Running` task, which is what made
the parked task look collectable - it now increments for `Running` **and**
`Suspended`, because a wait parked on a suspended task is an active waiter
(FR-030, and the `waiter_count` FR-012 reports). Covered by
`test_wait_agents_suspended::test_wait_agents_reports_suspended_task_as_suspended`,
`::test_wait_agents_unsettled_entries_carry_the_suspended_status`,
`::test_wait_agents_keeps_a_running_task_out_of_the_suspended_set` and
`::test_wait_agents_mixed_running_and_suspended_are_reported_separately`.

The four tools read and write one object (`AgentManager::tasks`), so they carry
one permission taxonomy (spec `agentssetup` FR-005) - three categories over one
registry, never `none`:

| Tool | `permission_category()` | Why |
|------|-------------------------|-----|
| `new_agent` | `agent:spawn` | the only action that creates a task |
| `cancel_agent` | `agent:control` | a control action on a task that already exists |
| `wait_agents` | `agent:control` | the same control action, blocking instead of terminal |
| `list_agents` | `agent:read` | a read-only observation of the registry |

`cancel_agent` previously reported `agent:spawn`, which conflated "start work"
with "stop work": a policy could not grant spawning while denying cancellation.
`list_agents` previously reported `none`, which short-circuits the permission
layer entirely (`processor.rs` returns before any rule lookup), so a policy
written against its category never ran - and a read of another agent's work is
something a policy must be able to deny without denying the spawn. The
classification tightened and the interactive outcome did not: all four remain in
`AUTO_APPROVED_AGENT_TOOLS`, so no sub-agent call gains a prompt. The `--dry-run`
family table keeps its count (the `agent` family grows 4 -> 5, the family total
stays 21), and `test_agent_permission_taxonomy.rs` pins all four categories as
literals so a regression cannot agree with itself.

### 16.4.2 Live sub-agent output overlay

The TUI output-view overlay (click a row in the Agents panel, or `Alt+A` /
the Agents popup) renders the target session's persisted transcript from
SQLite, so a running sub-agent's steps are visible mid-run, not only after
completion. Two mechanisms keep the overlay in step with the run:

- the session loop's interim assistant save persists completed tool-call
  parts into the child session's message row as each step finishes (the
  M-008 save gate is a total-parts count, so a tool-only step still saves;
  FTS stays untouched until the final save), and
- the overlay's line-cache generation key mixes in the same per-session
  step / tool-call counters the Agents panel shows, so the cache rebuilds
  whenever the agent advances and otherwise reuses the rendered rows
  (PERF-048 invariant).

Before the counter mix, the generation key (message count + `edit_seq`,
neither of which changes mid-run) kept the overlay frozen at whatever it
rendered when opened; before the total-parts gate, a tool-only step skipped
the interim save entirely, so nothing new was visible until the final save.

### 16.5 Status Display

Autopilot state is shown in the status bar as `AUTOPILOT` with iteration and
token counters.

---

## 16A. Cron Scheduling

Ragent supports scheduling agent runs with a cron-like system. Users can
schedule one-shot or repeating events, each with a designated agent type and an
initial prompt. Events are persisted to SQLite and evaluated by a background
scheduler while the TUI session is running.

### 16A.1 Schedule Grammar

Three schedule forms are supported:

| Form | Behaviour | Example |
|------|-----------|---------|
| `at <timestamp>` | One-shot. Fires once at the specified time. | `/cron add nightly general at 2025-01-15T09:00 "Run tests"` |
| `from <timestamp> every <duration>` | Repeating. First fire at the timestamp, then every duration. | `/cron add nightly general from 2025-01-15T09:00 every 30m "Run tests"` |
| `every <duration>` | Repeating, no explicit start. First fire is duration from now. | `/cron add nightly general every 2h "Run tests"` |

Durations are a positive integer + unit:

| Unit | Meaning | Aliases |
|------|---------|---------|
| `m` | minutes | `min`, `mins` |
| `h` | hours | `hr`, `hrs` |
| `d` | days | `day`, `days` |
| `w` | weeks | `wk`, `wks` |
| `mo` | months (30 days) | `month`, `months` |

Timestamps accept ISO-8601 (e.g. `2025-01-15T09:00:00Z` or
`2025-01-15T09:00:00+02:00`) or natural-language shortcuts resolved against
the user's local timezone:

| Shortcut | Meaning |
|----------|---------|
| `5pm` | Next 5pm (today if not yet passed, else tomorrow) |
| `5:30pm` / `5:30 pm` | Next 5:30pm |
| `17:00` | Next 17:00 (24-hour clock) |
| `5am tomorrow` | 5am on the following day |
| `12pm` | Noon |
| `12am` | Midnight |

### 16A.2 Slash Commands

| Command | Description |
|---------|-------------|
| `/cron add <cronname> <agent> <schedule> "<prompt>"` | Schedule a new event (`cronname` becomes the event ID) |
| `/cron remove <event_id>` | Remove a scheduled event |
| `/cron enable <event_id>` | Enable a previously disabled event |
| `/cron disable <event_id>` | Disable an event (skipped by the scheduler) |
| `/cron list` | List all events with human-readable schedule descriptions |
| `/cron detail <event_id>` | Show full details of a single event (untruncated prompt) |
| `/cron log [event_id]` | Show execution log (optionally filtered by event id) |
| `/cron help` | Show usage |

The model can also manage cron events directly via the LLM-callable tools
`cron_add`, `cron_remove`, `cron_list`, `cron_enable`, and `cron_disable`.

### 16A.3 Scheduler

A background scheduler task runs while the TUI session is active, ticking every
30 seconds. On each tick, it queries all enabled events whose `next_due` has
passed and spawns a background agent run for each. For repeating events,
`next_due` is advanced by one duration interval. For one-shot events, the event
is marked as fired.

### 16A.4 Execution Logging

Every execution is logged as a single JSON line appended to
`<working_dir>/log/cron-<timestamp>.jsonl`, mirroring the edit-log convention.
Each entry records:

- Event id
- Agent type
- Prompt
- Outcome (`"success"`, `"error"`, or `"skipped"`)
- Error message (if any)
- Completion timestamp

### 16A.5 Guards

- **Disabled events** are skipped with a `"skipped"` outcome.
- **Unknown agent types** are logged with an `"error"` outcome.
- **Double-fire prevention**: if a repeating event's previous execution is
  still running, the current due cycle is skipped and logged as `"skipped"`.

### 16A.6 Past-Start Advancement

When a `from <timestamp> every <duration>` event has a start timestamp in the
past, `next_due` is advanced by whole duration intervals until it is strictly in
the future. The original `start_at` timestamp is preserved unchanged.

---

## 17. Orchestrator & Multi-Agent Coordination

### 17.1 Core Components

| Component | Responsibility |
|-----------|----------------|
| `Coordinator` | High-level task decomposition and dispatch |
| `AgentPool` | Manages active sub-agents |
| `ConflictResolver` | Detects and resolves conflicting edits |
| `TransportAdapters` | Bridges TUI, HTTP, and mailbox transports |

### 17.2 Coordinator API

The coordinator exposes methods for:

- `decompose(prompt)` → list of subtasks
- `dispatch(subtasks)` → spawn agents
- `collect(results)` → aggregate outputs
- `synthesize(results)` → final response

### 17.3 HTTP API Endpoints

Orchestrator operations are available via `/sessions/{id}/messages` when the
session is configured with the `orchestrator` agent.

### 17.4 Conflict Resolution

When multiple agents edit the same file, the conflict resolver:

1. Detects overlapping edits.
2. Applies non-overlapping edits highest-end-offset-first.
3. Reports conflicts for manual resolution.

### 17.5 Transport Adapters

| Adapter | Use |
|---------|-----|
| TUI | Real-time UI updates |
| HTTP SSE | Remote clients |
| Mailbox | Team teammate notifications |

### 17.6 Current Status

The orchestrator is functional for swarm decomposition and simple parallel
workflows. Advanced conflict resolution across arbitrary file sets is under
active development.

---

# Part VI: External Integrations

---

## 18. GitHub & GitLab Integration

### 18.1 GitHub Authentication & Slash Commands

Authentication uses `GITHUB_TOKEN`. GitHub Enterprise is supported via the
`github_api_base` config.

### 18.2 GitHub Issue Tools

| Tool | Purpose |
|------|---------|
| `github_list_issues` | List repository issues |
| `github_get_issue` | Read an issue |
| `github_create_issue` | Create an issue |
| `github_update_issue` | Update an issue |
| `github_close_issue` | Close an issue |

### 18.3 GitHub Pull Request Tools

| Tool | Purpose |
|------|---------|
| `github_list_prs` | List PRs |
| `github_get_pr` | Read a PR |
| `github_create_pr` | Create a PR |
| `github_merge_pr` | Merge a PR |
| `github_close_pr` | Close a PR |

### 18.4 GitHub Auto-Detection

The local git remote is inspected for `github.com` or a configured GitHub
Enterprise host. When detected, GitHub tools are exposed automatically.

### 18.5 GitLab Authentication & Slash Commands

Authentication uses `GITLAB_TOKEN` or `gitlab.com` OAuth. Self-managed GitLab
is supported via `gitlab_api_base`.

### 18.6 GitLab Issue Tools

| Tool | Purpose |
|------|---------|
| `gitlab_list_issues` | List project issues |
| `gitlab_get_issue` | Read an issue |
| `gitlab_create_issue` | Create an issue |
| `gitlab_close_issue` | Close an issue |

### 18.7 GitLab Merge Request Tools

| Tool | Purpose |
|------|---------|
| `gitlab_list_prs` | List MRs |
| `gitlab_get_pr` | Read an MR |
| `gitlab_create_pr` | Create an MR |

### 18.8 GitLab CI/CD Pipeline Tools

| Tool | Purpose |
|------|---------|
| `gitlab_get_pipeline` | Read a pipeline |
| `gitlab_list_jobs` | List pipeline jobs |
| `gitlab_get_job` | Read a job |
| `gitlab_retry_job` | Retry a job |
| `gitlab_cancel_job` | Cancel a job |

### 18.9 GitLab Project Tools

| Tool | Purpose |
|------|---------|
| `gitlab_list_projects` | List accessible projects |
| `gitlab_get_project` | Read project metadata |

### 18.10 GitLab Auto-Detection

GitLab tools are auto-enabled when a `gitlab.com` or configured self-managed
host is detected in the git remote.

---

## 19. MCP Integration (Model Context Protocol)

### 19.1 Discovery

Ragent can discover MCP servers automatically. Known server types include:

- `filesystem`
- `git`
- `github`
- `fetch`
- `sqlite`
- `postgres`
- `brave-search`
- `everything`
- `puppeteer`

### 19.2 Configuration

MCP servers are configured in `ragent.json`:

```jsonc
{
  "mcp": {
    "servers": [
      {
        "name": "fetch",
        "command": "uvx",
        "args": ["mcp-server-fetch"]
      }
    ]
  }
}
```

### 19.3 Tool Bridging

MCP tools are wrapped as `mcp_tool` and exposed to the agent with dynamically
generated JSON schemas. The wrapper handles:

- stdio transport
- request/response correlation
- error translation

### 19.4 Status

MCP support is functional for stdio and HTTP servers. Auto-discovery and
configuration editing are supported via `/mcp` slash commands.

#### 19.4.1 Server enable/disable state

Whether an MCP server is actually started is a persisted, user-owned choice
rather than an implicit side effect of being listed in `ragent.json`. The choice
lives in a **global ledger** (`<global state dir>/mcp_state.json`) shared by the
connect path, the permission gate, and the `/mcp` and `/plugins` surfaces:

- A server id **absent** from the ledger is enabled, so a newly added server -
  written into `ragent.json` or bridged from a plugin's `mcpServers` section -
  starts enabled with no extra step.
- `mcp.<id>.disabled: true` in `ragent.json` always disables the server (the
  config flag is the harder switch), even if the ledger says otherwise.
- A disabled server is registered with `McpStatus::Disabled` and **no child
  process is spawned**; `/mcp` still lists it and can re-enable it.
- `/mcp connect <id>` enables and connects a server live, registering its tools
  immediately; `/mcp disconnect <id>` disables and disconnects it live. Both
  persist the choice globally, so it survives a restart and applies to every
  project.
- `McpToolWrapper::execute` refuses to call a disabled server's tools even when
  the tool is still registered from an earlier connection, naming the command
  that re-enables it.

#### 19.4.1a Removing a definition (`/mcp remove`)

`/mcp remove <id>` is the destructive counterpart of connect/disconnect: it
disconnects the server and deletes its entry from the `mcp` section of the
**user-global** `ragent.json`, then clears the server's enable-ledger row so a
re-added server starts enabled again rather than inheriting a stale disable. The
session-side disconnect runs *before* the file write, so a failed write leaves
the running state consistent with the file; a server that was not connected
simply has nothing to disconnect.

The edit goes through `Config::edit_global_config(edit)`, which reads the file as
raw `serde_json::Value`, hands the top-level object to the caller's closure, and
writes it back only when the closure reports a change. It exists because
serialising the *merged* config would drop every key this build does not model,
so an unrelated key in the operator's file would be lost by an edit that only
meant to touch `mcp`. Every other key survives, an `mcp` object emptied by the
removal is deleted rather than left as `{}`, and the load cache is invalidated so
the next `Config::load` sees the new value.

**A bridged server is refused.** A server whose definition lives in an installed
plugin's `mcpServers` (or the external `mcp.json` that entry names) or in a
connector's `connector.json` belongs to that owner: pruning it would leave the
owner installed declaring a server list it no longer has, which the next bridge
scan reports as drift. The refusal names the owning command - `/plugins remove
<id>` or `/connectors remove <id>` - while `/mcp disconnect <id>` still stops the
server without uninstalling its owner. Both owners resolve through one
`app::mcp_bridge` module (`merged_mcp_servers`, `bridged_servers`,
`resolve_config`, `find_bridged`, `is_bridged_source`), which `/mcp`,
`/mcp config`, `/mcp discover` and `/mcp connect` all share, so a
connector-bridged id is accepted everywhere a plugin-bridged one is.

#### 19.4.2 Transports, sessions, and startup reporting

A server is reached over the transport its config declares. Only the bundled
MongoDB server gets a default: ragent does **not** start a second copy of a
server it expects to be long-lived, so the shipped `mongodb` plugin, whose
`mcp.json` declares no transport, is bridged to `McpTransport::Http` at
`http://127.0.0.1:3000/mcp`. If a server is already listening there it is
adopted; otherwise the HTTP endpoint (not a stdio child) is what the server is
started as. Every other plugin entry and every `ragent.json` entry keeps the
transport it declares (default `stdio`).

The HTTP client performs the MCP `initialize` handshake before `tools/list` and
replays an issued `mcp-session-id` on every later request, because a sessionful
Streamable-HTTP server (the MongoDB server on its 2025-era path) rejects
requests on a session it did not open. Replies may be bare JSON-RPC or an
`event: message` SSE frame; both are accepted, and a frame whose JSON payload
is split across multiple `data:` lines is re-joined with `\n` per the SSE
grammar rather than truncated to the first line.

A stdio child is never adoptable - its pipes are private to the parent that
spawned it - so a config declaring `stdio` always results in a fresh child.
A stdio child leads its **own process group** (`process_group(0)` at spawn, the
same pattern as the bash-timeout harness), because launcher commands (`npx`,
`npm`, `bunx`) fork the real server (`node`) and exit immediately - killing
only the recorded launcher pid would orphan the process actually holding the
stdio pipes. Process teardown (`shutdown`) therefore SIGKILLs the *group*
(`killpg`) after attempting the graceful rmcp `cancel()` — the kill is
unconditional and ESRCH-safe, so a connection that `/mcp disable` already
tore down needs no special-casing — and drains the connections map atomically
so a second ragent instance starting at the same
time never reaps a child another instance is still using. Before spawning,
`connect` sweeps `/proc` (Unix only) for **orphaned** earlier
copies of the same `command` + `args` and SIGKILLs them, because an orphaned
stdio server can no longer serve anyone yet keeps holding file locks, ports it
also binds, API rate-limit slots, and memory. A process counts as orphaned only
after being re-parented away from its spawner: `/proc/<pid>/stat` field 4
(`Ppid`) equals `1`, or the parent is the per-user service manager
(`systemd --user`, matched on `/proc/<parent>/comm` or argv[0]). A same-command
process whose own parent is still alive belongs to a *running* ragent instance
and is never killed, so concurrent sessions sharing one MCP stdio command do
not sever each other's pipes (`Transport closed`). Desktop sessions re-parent a
dead ragent's launcher tree to the user manager rather than to init, so the
`Ppid == 1` check alone missed exactly the orphans this sweep exists to reap.
Matching is per process: any supervisor-reparented process whose own command
line matches is swept, and its process group is signalled, so the launcher
(`npm exec mongodb-mcp-server@<3>`) and the `node` child it forked are both
reclaimed even though only the child carries the package token. The
current process is always
excluded, matching keys on the executable basename (`/usr/bin/npx` == `npx`)
plus, for launcher commands (`npx`, `npm`, `bunx`, `pnpx`), the package token
with any `@<version>` specifier stripped - so `npx -y mongodb-mcp-server@<3`
and the node child it spawns (whose argv only mentions the package inside the
`node_modules` path) are both swept, while an unrelated `npx -y other-server`
and any same-named process outside the package tree survive. A launcher
invocation with no positional package argument matches nothing (killing every
`npx` on the machine would be a bug, not cleanup). When the stdio command line
declares an HTTP port (`--httpPort <n>` / `--port <n>`), that port is probed
with a short-timeout `initialize` first and a live MCP server there is adopted
instead of spawning a child; a bare TCP listener that fails the handshake is
rejected as not-MCP.

The startup connect loop publishes the shared `McpClient` **before** connecting
the first server, so every reader (the tool registry, `/mcp`, the startup
report) sees the live handle from the beginning; a one-shot run no longer races
the loop. Startup does not block on the loop: the TUI adopts whatever state the
loop has published, then prints its one-shot per-server report off the loop's
completion sentinel (an empty-id `McpStatusChanged { status: "initialized" }`
event), once, after every server has settled. The report prints one `[mcp]` line
per server — `Starting <id> (<transport>)` plus `Connected ... via <transport>`,
or `Skipping ... (<transport>, disabled)`, or `Failed to connect ... : <reason>`,
or `... needs authentication` — where `<transport>` is the config's declared wire
protocol (`stdio` / `sse` / `http`, via `Display for McpTransport`).
`report_mcp_startup` latches its own one-shot; a `try_read` miss (the connect loop
holds the client write lock across a connect) defers the report without latching,
so the next sentinel retries it. Every `connect` failure path — a config rejected
by `validate_mcp_config`, a closed spawn semaphore, or a failed handshake —
records the server as `McpStatus::Failed { error }` before returning, so a startup
error is reported alongside the servers that came up instead of only being logged.
The connect loop publishes this client-derived status (`connected`, else
`failed: <reason>`) on its per-server `McpStatusChanged` event, so `/mcp` shows the
reason as well.

The TUI reconciles the session tool registry against the live client whenever a
connect settles or a server is enabled/disabled: a registered MCP tool whose
`mcp_<sanitized-server>_` name prefix no longer matches any Connected server is
removed (`ToolRegistry::remove_all`), so a multi-segment id like
`mongodb.mongodb` (`mcp_mongodb_mongodb_*`) is recognised by prefix match
rather than by its first `_`-segment, and disabling a server leaves no dead
wrappers for the model to call. The registered name itself comes from one
place, `McpToolWrapper::ragent_name_for`, which the wrapper constructor and the
TUI both use.

#### 19.4.3 Discovery-location report (`/mcp config`)

`/mcp config` reports **where** ragent looks for MCP server definitions, how many
it finds at each place, and the individual servers discovered. It is backed by
`McpClient::discover_locations`, which returns a `Vec<McpDiscoveryLocation>`
instead of the flat, de-duplicated server list `discover` yields. Each location
carries a label, the scanned path (absent for the `PATH` and npm-global rows),
whether the path exists, and the definitions found there, before de-duplication.

The scanned locations, in order, are: `PATH` (the known server executables),
npm global `node_modules`, and each registry directory — the ragent-native
`~/.config/ragent/servers/` first so its definitions win de-duplication, then
`~/.claude/mcp-servers/`, `~/.cline/mcp-servers/`, `~/.mcp/servers/`,
`<config>/mcp/servers/`, and `<config>/claude/mcp-servers/`. The command renders
a markdown table of every location (label, path with a `(missing)` marker for an
absent directory, and a count), a total, and a second table of the discovered
servers (id, the location it came from, its launch command, and the file that
declares it). `discover` is
unchanged: it flattens the location results and de-duplicates by id.

The **declaring file** column is what tells a `ragent.json` entry apart from a
bridged one before `/mcp remove` is run (§19.4.1a): a `PATH`/npm/registry
discovery reports `-` (it has no definition file until it is adopted), while a
plugin- or connector-bridged server reports the absolute path of the manifest
that declares it - the plugin manifest, the external `mcp.json` its `mcpServers`
entry names (resolved and confinement-checked against the plugin root), or the
connector's `connector.json`. The discover dialog carries the same information in
its `Source / Defined in` column, and its width ceiling is 170 columns so an
absolute path is not clipped away. `/mcp` prints the same mapping as a separate
`Defined in:` block plus a `Managed by:` block naming each bridged server's owner,
because a seventh table column wrapped every header cell past the message width.


---

## 19B. Durable Initiatives & Skill Management

### 19B.1 Initiative Tool (`initiative`)

The `initiative` tool manages durable, cross-session goals with milestone
tracking. Unlike session-scoped `todo_*` items, initiatives are persisted in
the project-scoped `initiatives` SQLite table and survive compaction, session
restarts, and machine reboots. Any session running in the same working
directory sees the same initiatives.

Actions: `create`, `read`, `update`, `checkpoint`, `list`, `close`.

```jsonc
initiative action="create" id="api-v2" title="Ship API v2"
           milestones=["design","implement","deprecate v1"]
initiative action="checkpoint" id="api-v2" milestone="ms-1" progress=33
           note="Design doc merged"
initiative action="list"                       // active only (default)
initiative action="list" status="all"          // includes closed
initiative action="close" id="api-v2" status="completed"
```

`checkpoint` marks a milestone complete (recording `completed_at`), bumps the
overall progress percentage, and appends the free-text `note` as a timestamped
`Checkpoint:` line in the description so there is a durable audit trail.
`close` requires `status` of `completed` (auto-fills progress to 100) or
`abandoned` (keeps the recorded progress).

Active initiatives are injected into the system prompt on every turn under
`## Active Initiatives`, listing id, progress, and the next pending milestones
so the agent stays aware of long-term goals across turns and sessions.

Registered under the `storage:write` permission category.

### 19B.2 Skill Management Tool (`Skill`)

The `Skill` tool exposes runtime control of the skill registry
(SPEC §12) without requiring a session restart.

Actions:

- `list` — enumerate registered skills (name, scope, invocation flags,
  description). Optional `scope` filter; `include_bodies=true` also prints each
  skill's prompt body.
- `read` — return one skill's fully processed prompt body. `arguments` are
  substituted into `$ARGUMENTS`-style placeholders, exactly as a `/skill`
  invocation would.
- `load` — (re)discover skills from disk and return the named skill's prompt,
  the same content the model receives on `/skill` invocation. Use it to
  "inject" a skill added or edited after session start.
- `reload` — drop cached skill bodies, re-discover from disk, and report
  which skills were added/removed since the previous scan, plus the bundled
  baseline count.

```jsonc
Skill action="list"
Skill action="read" name="rust-error-handling" arguments="ctx"
Skill action="load" name="rust-error-handling"
Skill action="reload"
```

Registered under the `skill:manage` permission category.

---

## 19C. Connector System (Claude-Connector-Equivalent Integrations over MCP)

Spec: `specs/connectors/` (`SPEC.md` FR-001..FR-041, NFR-001..003; `PLAN.md`
T-001..T-021; `TESTPLAN.md`). Crate: `crates/ragent-connectors` (17th workspace
crate).

A **connector** is a named, user-facing integration (Google Drive, Slack,
GitHub, Git, Postgres, Puppeteer, and the wider community catalogue) that reaches
an external system through the **Model Context Protocol (MCP)**. A connector is
a catalogue entry carrying a display name, a category, an authentication
requirement, and one or more MCP servers; the MCP server remains the transport
ragent already speaks (`McpClient`, §19). The connector system is therefore a
**catalogue and management layer** over machinery that already exists (the MCP
client, the durable `mcp_state.json` enable ledger, and the encrypted credential
store), adding the connector abstraction and one coherent surface.

### 19C.1 Descriptor model

A connector is a normalised `ConnectorDescriptor` holding its id, display name,
category, one or more server definitions, an authentication shape, and an
optional credential *name*. Validation (FR-002, FR-025) rejects a descriptor
that cannot be expressed as one or more MCP servers (an unexpressible transport
such as `grpc` is recorded with the label `server transport` rather than treated
as fatal). A malformed catalogue entry is skipped and counted, never fatal
(FR-031).

### 19C.2 Store and enable ledger

The connector store is discovered under `.ragent/connectors/` (project),
falling back to `~/.config/ragent/connectors/` (user-global) (FR-001), with an
optional `connectors.store_dir` override. A connector install is
**path-traversal-safe**: an archive member that escapes the store root (for
example `../../escape.json`) is refused (FR-029), and a non-`https` package URL
is refused (FR-028). A freshly installed connector is recorded **enabled** and
is ready to use; `/connectors enable <id>` connects its servers now in a running
session (FR-011). The durable enable state is the **existing**
`ragent_agent::mcp::enable_state` ledger (`mcp_state.json`), so a connector
disabled once stays disabled everywhere and the connector crate adds no second
enable file (FR-018). `remove` is refused while the connector is enabled
(FR-030), and a duplicate connector id is refused (FR-027).

### 19C.3 Catalogue, providers, and the default endpoint

A catalogue index is fetched over `https` under a byte cap, a per-fetch
wall-clock timeout, and a TTL cache (FR-024, FR-031). Each catalogue is
normalised through a `ConnectorProvider` shaped like
`ragent_plugins::store_provider::StoreProvider` (tolerant per-entry skip with a
`skipped` counter), so the browser domain stays catalogue-agnostic. A compiled
default Claude connector-catalogue endpoint constant and a pure, offline
provenance resolver (FR-034..FR-038, NFR-001..003) mirror the plugin store's
`DEFAULT_CLAUDE_STORE_URL` / `effective_endpoint_with_source` discipline; the
literal is declared exactly once, enforced by
`scripts/check-connector-endpoint-literal.sh` (FR-038, NFR-001).
`/connectors stores [--check]` reports each endpoint with its provenance
(`default` or `config`) and, with `--check`, probes it (FR-036).

### 19C.4 Bridge to the MCP client

The bridge resolves every enabled connector's server definitions into
`(bridged_server_id, McpServerConfig)` pairs (FR-003). Bridged server ids are
`<connector-id>.<server>`, the same collision-avoidance shape the plugin MCP
bridge uses (spec `plugins` FR-030); a collision is refused (FR-033). The
session feeds those configs to the existing `McpClient`, so no new transport is
written: every connector resolves to stdio, SSE, or HTTP exactly as a configured
MCP server does, and its tools surface under the normal
`mcp_<sanitized-server>_<tool>` names (FR-020). A bridged server that fails to
connect or returns a tool error is contained and named (FR-016); a disabled
connector starts nothing and is invisible to the model (FR-018).

### 19C.5 Authentication

A connector's secrets (tokens, client secrets, refresh tokens) live in the
existing **encrypted credential store**; the connector manifest stores only the
credential *name*, never the value (FR-005). `/connectors auth <id>` reports a
connector's auth state and lets a token be supplied (FR-014); a connector that
needs authentication and has no valid credential reports `needs auth` and its
servers are refused until it is satisfied (FR-022, FR-023, FR-032), with no
retry loop on an auth failure. The `connectors.credentials` config block maps a
connector id to non-secret credential **names** for environments where the
credential must be resolved from an environment variable.

### 19C.6 The `/connectors` command family

Thirteen subcommands, dispatched through
`crates/ragent-tui/src/app/connector.rs`, registered in `SLASH_COMMANDS`, and
listed in the command catalog and the `/` autocomplete menu (seeded from the
crate's `CONNECTOR_SUBCOMMANDS` token list so the menu cannot drift from the
usage block) (FR-004):

| Subcommand | Purpose |
|------------|---------|
| `/connectors list [--verbose] [--category <name>]` | List installed connectors with state, auth state, category, and bridged server/tool counts (FR-009) |
| `/connectors claude [query] [--category <name>] [--refresh]` | Open the interactive catalogue browser (FR-010) |
| `/connectors add <id\|source> [--force]` | Install a connector (recorded enabled; FR-011) |
| `/connectors remove <id>` | Uninstall (refused while enabled; FR-030) |
| `/connectors enable <id>` | Enable and connect the connector's servers now (FR-012) |
| `/connectors disable <id>` | Disable and disconnect (FR-013) |
| `/connectors connect <id>` | Connect an enabled connector's servers without a restart (FR-019) |
| `/connectors disconnect <id>` | Disconnect, leaving the connector enabled (FR-019) |
| `/connectors auth <id>` | Manage the credential and report auth state (FR-014) |
| `/connectors test <id>` | Isolated connect-and-invoke harness (FR-015, FR-016) |
| `/connectors stores [--check]` | Report each catalogue endpoint's provenance (FR-036) |
| `/connectors marketplace show` / `marketplace claude list\|add\|remove` | Manage the additional catalogues the browser fetches (FR-042, FR-043) |
| `/connectors help` | Usage block (FR-017) |

A bare `/connectors`, `/connectors help`, and an unrecognised subcommand all
render the same usage block. Every report carries a `From: /connectors <sub>`
attribution line and is ASCII-only (FR-006). The category filter (FR-039,
FR-040, FR-041) restricts the displayed rows; `ALL` (any case) or a blank value
clears it, and an unknown category is refused with an `[err]` row and changes no
state. The interactive `claude` browser is the single catalogue surface (FR-010):
its `--category <name>` launch argument and its in-panel `c` cycle key select the
filter over the categories the fetched catalogue declares, so there is no
standalone `search` subcommand and the browser and the textual `list` report
share one filter model. The single-reference subcommands (`remove`, `enable`,
`disable`, `connect`, `disconnect`, `auth`, `test`) accept a connector by
canonical id, slug, or display name; the reference is resolved to the installed
connector's canonical id before the lifecycle call.

The TUI drives a single published lifecycle session. `src/main.rs` resolves the
enabled connectors through the bridge into the same background connect loop that
starts the configured `ragent.json` and plugin-contributed servers, then
publishes a `ConnectorSession` and its status snapshot on the session processor
(type-erased, so `ragent-agent` does not depend on `ragent-connectors`).
`/connectors enable|disable|connect|disconnect` drives that session and
reconciles the MCP tool registry afterwards, and `/connectors list` takes its
per-server tool counts from the live MCP client, so a bridged server that
connected after startup reports a real count (T-008).

### 19C.7 Catalogue browser (`/connectors claude`)

A modal overlay in `crates/ragent-tui/src/layout.rs`, wearing the same design
language as the plugin-store panel (spec `connectors` FR-047): a calm slate accent
frame capped at 140x41; a structured title line (catalogue name badge, accent
counts, and the active `search`/`category`/`marketplace` filters; a
`loading details...` hint is appended while the manifest-description enrichment is
still running); a left column split into a `Categories` navigator above a
`Marketplace` navigator, each under a reversed-video title band with every row
labelled `Name (N)`; a pinned dark-on-light column-heading row above zebra-banded
result rows carrying a `> ` cursor gutter, a one-column bold-green `*` lane on an
installed connector, the id and category columns, the marketplace in the accent
colour (or `-`), and a description running to the panel's right edge; a centred
footer of key hints; and a bottom ` search: ` box with a clickable `[x] clear`
button. The category navigator lists `ALL`, an `Installed` pseudo-category matched
against the store scan, then the catalogue's declared categories. Three panes are
focusable (`Results` initial, then the two navigators, cycled by `Tab`/`BackTab`
with only the focused pane's cursor filled); `Up`/`Down` move the focused pane and
apply a navigator row, `ENTER` over a navigator applies and returns focus to the
results, `c` clears the focused navigator's filter to `ALL`, and `Backspace`/`Esc`
edit the query (`Esc` on an empty query dismisses the panel). The catalogue fetch
and the `ENTER` install both run **off the event loop** on a blocking worker and
deposit their result for the UI thread to drain, so the fetch never blocks the
loop or an agent turn. The launch parser
removes both `--refresh` and `--category <name>` from the query tokens (so a
pre-filled query may contain spaces) and validates the category against the
categories the fetched catalogue actually declares, falling back to an
unfiltered launch when it is unknown; a refused category is reported in the
footer while every row stays visible (FR-041).

### 19C.7a Catalogue marketplaces and description enrichment

The panel fetches the base catalogue endpoint and every marketplace added
through `/connectors marketplace claude add`, merging their entries into the one
list (`store_ops::fetch_all_catalogues`,
`CatalogueKind::effective_endpoints_with_source`); a duplicate id keeps the first
occurrence (the base endpoint's) and a refused added URL is skipped and surfaced
as a warning in the message window rather than blinding the panel. Each merged
entry carries the display name of the marketplace it came from in its
`marketplace` field (`marketplace_name_for`: the added marketplace's `name`, or
the catalogue label for the base endpoint), rendered in the browse row.

After the catalogue lands, each entry whose `source` is a `github.com` `git+`
source has its **blank** description resolved from its own repository manifest off
the event loop (`crates/ragent-connectors/src/catalogue_enrich.rs`, ported from
`ragent_plugins::store_enrich`): the raw URL
`https://raw.githubusercontent.com/<owner>/<repo>/<ref>/<subpath>/connector.json`
is fetched (with `.claude-plugin/connector.json` tried after it) through a
bounded thread pool, best-effort and never fatal, cached per URL, and behind the
injectable `ManifestSource` seam (the production `NetworkManifestSource`).
`/connectors marketplace show` and `/connectors marketplace claude list|add|remove`
manage the `connectors.stores.marketplaces` array in the user-global config
(atomic splice, duplicate name refused without `--force`) (FR-042..FR-044).

### 19C.8 Configuration

The `connectors` block in `ragent.json` (§5.2) controls the master switch
(`connectors.enabled`, default `true`), the store directory override, the
catalogue endpoints and shared fetch budgets (`timeout_ms`, `max_index_bytes`,
`cache_ttl_secs`), and the non-secret credential-name mapping. The section
merges overlay-wins (project over user-global) (FR-007, FR-021, FR-024). With
`connectors.enabled: false` the subsystem performs no discovery, no catalogue
fetch, and no connection, and every subcommand other than `help` reports that it
is disabled (FR-021).

### 19C.9 CLI parity

The same operations are available from a shell via the `ragent connectors <sub>`
CLI parity surface (`src/connectors.rs`), mirroring `src/plugins.rs`: the
session-free subcommands, the management subcommands (over an ephemeral
lifecycle session), and the `test` harness. The report is re-spelled for the CLI
surface (`From: /connectors X` -> `ragent connectors X`).

---

## 19D. Execution Backends (`local` / `docker` / `podman` / `remote`)

A pluggable execution backend selects **where** a session's tool invocations run
(spec `openhands` FR-001, FR-019, FR-020, FR-026, FR-031, FR-035). The backend
dispatch leaf is `crates/ragent-agent/src/backend/`; the agent loop, LLM
providers, tool registry, permission system, and event bus are unchanged by the
choice - only the tool-execution leaf routes through the `ExecutionBackend`
trait.

### 19D.1 Kinds and defaults

`ExecutionBackendKind` covers `local`, `docker`, `podman`, and `remote`. With no
`execution_backend` key configured (or an unknown value) the effective kind
resolves to `local`, the host adapter that runs the tool exactly as before
(FR-019). `local` is a thin pass-through, so selecting it is behaviour-preserving.

### 19D.2 Container backend (T-002)

`ContainerBackend` provisions one long-lived sandbox per session and dispatches
each tool call into it with `<runtime> exec`:

```text
<podman|docker> run -d --name ragent-sandbox-<hash> \
        -v ragent-sandbox-vol-<hash>:/projects -w /projects \
        <image> sleep infinity
```

- **Runtime detection (FR-026).** `crates/ragent-agent/src/backend/detection.rs`
  probes `PATH` for `podman`/`docker` and never spawns a process. `podman` is the
  default container runtime when none is specified; a container backend is only
  offered when a runtime is present.
- **Workspace isolation (FR-020, FR-035).** The workspace is a **container-named
  volume, never a host bind mount**, so a sandboxed tool cannot read or write the
  host working directory. The descriptor's `workspace` field names the
  container-side mount point when it is an absolute path (default `/projects`).
- **Dispatch.** `Bash` is validated by the host 7-layer validator before it runs
  (FR-002, FR-037); a file/search subset (`read`, `Write`, `create`,
  `append_to_file`, `rm`, `make_directory`, `list`, `Glob`, `Grep`) is translated
  to confined sandbox `exec` scripts; every other tool is refused with no host fallback
  (FR-031).
- **Fail without fallback (FR-031).** A provisioning failure (missing runtime,
  missing or unpullable image) fails the turn with a `Provision` error and never
  runs the tool on the host.

The runtime is selected from `Config::effective_backend_config()`, which resolves
both the bare-label spelling and an inline descriptor (image and workspace). The
processor caches one `ContainerBackend` per `(kind, id, image, workspace, working
dir)` so a session's container is provisioned once and reused (FR-009).

### 19D.3 Remote backend (T-003)

`RemoteBackend` drives a **second ragent server** over its REST+SSE API instead of
running tools locally (FR-021). The transport, stream decoder, and remote-session
cache live in `crates/ragent-agent/src/backend/remote.rs`; the session-level bridge
that mirrors the remote stream onto the local bus lives in
`crates/ragent-agent/src/session/remote_dispatch.rs`.

- **Turn flow.** The processor opens (or reuses) a remote session at
  `POST <url>/sessions` with the local working directory, posts the prompt to
  `POST <url>/sessions/{id}/messages`, and consumes the server's
  `text/event-stream`. Each frame is decoded into a `RemoteUpdate` and re-published
  on the **local** event bus under the local session id, so `text_delta`,
  `tool_call_start`/`tool_call_end`, `tool_result`, `agent_notice`,
  `permission_requested`, and `question_requested` render exactly like a local turn
  (FR-021). The assistant placeholder is persisted and updated with the mirrored
  text, so a remote turn is resumable like any other.
- **Auth and descriptor (FR-004, FR-030).** A `remote` descriptor carries `url`
  (base URL) and `api_key`; the key is sent as a bearer token on every request and
  is redacted from diagnostics. `Config::effective_backend_config()` resolves both
  the bare-label spelling and an inline/registry descriptor.
- **Reuse (FR-009).** One `RemoteBackend` is cached per `(id, url, api_key)`, and
  the remote session id is cached per `(backend id, local session id)`, so a later
  turn continues the same remote conversation over one HTTP connection pool.
- **Fail without fallback (FR-031, FR-034).** A remote backend with no `url`, or a
  server that cannot be reached before the turn, fails the turn with a `Provision`
  error. A stream that drops **part-way through** a turn is reported as a `Protocol`
  error, surfaced as an `AgentError`, and never re-runs the turn's tools locally. The
  relay is bounded by a turn timeout and polls the cancellation flag, so an
  unresponsive server cannot hang the session. `RemoteBackend::execute_tool` refuses
  every local tool dispatch, so a leaked dispatch cannot execute on the host either.

### 19D.4 Sandbox secret injection (T-006; FR-010, FR-023)

A `docker`/`podman`/`remote` descriptor may list `credentials` - non-secret
*names* stored in the encrypted credential store (`crates/ragent-storage`, the
same store `/auth` writes to). The value is resolved **at spawn time** and never
carried in `ragent.json`:

- **Container (FR-010).** Each named value is resolved when the sandbox is
  provisioned and injected as a container environment variable
  (`<runtime> run ... -e NAME=VALUE ...`), never written into the image or the
  workspace. The value is registered with the shared redaction registry, so a
  later rendering of tool output masks it (FR-035). A name the store does not
  hold fails the provision with a `Provision` error - the sandbox is never
  started without a required credential.
- **Remote (FR-010).** A remote descriptor that names a credential (and has no
  literal `api_key`) resolves its bearer key from the same store at turn start; a
  missing credential fails the turn rather than relaying it unauthenticated. A
  literal `api_key` still works and takes precedence, so existing configs are
  unchanged.
- **Workspace persistence (FR-023).** The sandbox workspace is a named volume
  derived from the *project* identity `(runtime, id, image, workspace, host
  working dir)`, so it survives replacement of the container: recreating the
  container always re-mounts the same volume. The *container* name additionally
  folds in the descriptor's credential names, so changing the credential set
  provisions a fresh container environment while preserving the workspace.

The credential store enters the backend through the `SecretResolver` trait
(`crates/ragent-agent/src/backend/secrets.rs`): the production
`StorageSecretResolver` reads the encrypted SQLite store, and `MapSecretResolver`
is the in-memory test seam. `resolve_backend_with_secrets` wires the store into
the container and remote adapters; a backend built without a store fails a
spawn that names a credential rather than starting without it.

### 19D.5 Durable backend registry (T-004; FR-004, FR-030)

`crates/ragent-agent/src/backend/registry.rs` exposes every configured execution
backend as a durable registry entry. The **config file is the durable record**;
the registry is its resolved read model, rebuilt from [`Config`] on demand, so no
separate store can drift from the configured backends. Each
`BackendRegistryEntry` carries:

| Field      | Meaning                                                                 |
| ---------- | ----------------------------------------------------------------------- |
| `id`       | A stable id from the descriptor's `id` (or the kind label when empty).  |
| `name`     | The display name (`name`, defaulting to `id`).                          |
| `kind`     | `local`, `docker`, `podman`, or `remote` (`ExecutionBackendKind`).      |
| `connection` | A secret-free `ConnectionDescriptor` (see below).                     |
| `health`   | A `HealthState` (`ok`, `unavailable`, `unknown`) plus a `detail`.       |

- **`ConnectionDescriptor`** is secret-free (FR-035): `Local`, `Container
  { runtime, image, workspace, container, volume, credential_names }`, or
  `Remote { url, has_api_key, credential_names }`. It records key *presence* and
  credential *names* only; `summary()` is safe to render in any log or panel.
- **Construction.** `BackendRegistry::from_config(config, host_working_dir)` always
  registers the built-in `local` entry first (FR-019), then each entry in
  `Config::backends`, then the active backend (an inline `execution_backend`
  descriptor or the registered entry it names), and finally a synthesized entry for
  a bare kind label with no matching descriptor, so the active backend is always
  visible. Duplicate ids are collapsed.
- **Health (FR-004).** `local` is always `ok`. A `docker`/`podman` entry is `ok`
  when its runtime binary resolves on `PATH` (FR-026) and `unavailable` with a
  detail naming the missing binary otherwise. A `remote` entry is `ok` when a base
  URL and a key are configured (FR-030), `unavailable` when either is absent, and
  refined by `refresh_health` / `probe_remote`, which probe the server's **public**
  `/health` endpoint (no bearer key materialised or sent). `refresh_health`
  re-probes every entry.
- **Remote registration (FR-030).** `BackendRegistry::register_remote` adds a
  `remote` backend by URL and key, replacing any entry with the same id in place.
- **Active resolution.** `BackendRegistry::active_id(config)` returns the id of the
  backend the config resolves to, preferring the active descriptor's id, then the
  first entry whose kind matches the effective kind (FR-019).

The registry is consumed by the TUI backend surface (T-005, FR-008) to show the
active backend and the health of every registered backend.

The TUI surface is the `/backend` family (`crates/ragent-tui/src/app/backend_panel.rs`,
`App::handle_backend_command`): with no argument (or `show`/`list`) it opens an
interactive switcher panel that renders the active backend and a health row for
every registered backend, and the status bar's second line opens with
`Backend:<name>` in cyan naming the active backend. Moving the
cursor and pressing `Enter` switches the active backend (persisting
`execution_backend`), after which its health is re-reported. `/backend <kind|id>`
switches directly; `/backend help` renders the usage table. A container backend is
offered only when its runtime resolves on `PATH` (FR-026), and the actual tool
dispatch still resolves the backend from the live config, so the panel is a view
over the registry rather than a second source of truth.

### 19D.8 LLM security analyzer (T-015; FR-006, FR-016, FR-017)

The security analyzer is an opt-in permission mode (`security_analyzer.enabled`,
or the TUI `/security on` / `security-analyzer` alias, which persists the flag).
Instead of deciding a proposed tool action from the static rules alone, ragent
asks a model for an `allow` / `ask` / `deny` verdict with a rationale, published
as `Event::SecurityVerdict` *before* the action is permitted or refused (FR-017)
so the user sees the reasoning.

- **Tightening-only (FR-006).** `crates/ragent-agent/src/security/mod.rs` builds a
  no-tools analyzer request and parses the verdict (`parse_verdict`); the decision
  is applied at the permission gate (`crates/ragent-agent/src/session/permissions.rs`):
  an analyzer `deny` is a hard denial reported to the model, an analyzer `allow` can
  satisfy a bare `Ask` (no explicit policy rule) but never overrides an explicit
  policy `Deny` or a forced destructive-action checkpoint, and an analyzer `ask` -
  or any failure (missing model, provider error, unparseable output, timeout) -
  degrades to the normal interactive prompt via `SecurityVerdict::fail_safe`, so a
  broken analyzer can never silently broaden access.
- **Model (FR-006).** `security_analyzer.model` routes the analyzer to a
  fast/cheap model; `security_analyzer.timeout_secs` (default 20) bounds one call.
- **Hook exit codes (FR-016).** `PreToolUse` hook semantics are independent and
  unchanged: exit code 2 blocks and returns the hook's stderr as the tool result,
  exit code 1 warns and continues.

```jsonc
{
  "security_analyzer": {
    "enabled": true,
    "model": { "provider_id": "ollama", "model_id": "qwen2.5:1.5b" },
    "timeout_secs": 20
  }
}
```

### 19D.6 Configuration

The `execution_backend` key accepts a bare label or an inline descriptor, and the
`backends` array declares the registry entries the switcher (T-005) and the
container backend read. See §5.2 / `docs/howtos/config.md` §7.36.

### 19D.7 Hardening and secret-safety invariants (T-018; FR-002, FR-024, FR-032, FR-035, FR-037, FR-038)

The new execution/interop surfaces add no security layer and weaken none; the
invariants below hold across every one of them and are pinned by hermetic tests
that fail on any host, with or without a container runtime.

- **The security decision is upstream of the backend (FR-002, FR-037).** The
  permission check, the file-path containment guard, the always-allowed
  codeindex hardwiring, and the 7-layer bash validator all run in the session
  tool-dispatch path (`crates/ragent-agent/src/session/`) *before* the leaf is
  handed to an `ExecutionBackend`. The trait carries no permission or approval
  token (`BackendToolCall { tool, input, ctx }`), so a backend can only run an
  already-approved call - selection changes *where* a tool runs, never *whether*
  it is allowed (assumption A1).
- **Sandbox bash is gated by the canonical host validator (FR-002, FR-037).**
  `ContainerBackend::execute_tool` calls
  `ragent_tools_core::bash::validate_shell_command` on every `Bash` call before
  it provisions or touches a container, so a banned tool, a denied command name,
  a denied pattern, a directory escape, or an obfuscated command is rejected with
  the tool never executing on the host. The refusal precedes provisioning, so it
  is identical with or without a runtime present. Covered by
  `crates/ragent-agent/tests/test_hardening_invariants.rs`.
- **A sandbox cannot reach the host working directory (FR-035).** The workspace
  is a container-managed named volume mounted at `/projects`, never a host bind
  mount; the host working directory is only an input to the sandbox *name hash*,
  never a tool working directory or mount source
  (`test_hardening_invariants.rs::sandbox_workspace_is_the_container_mount_not_the_host_working_directory`).
  A sandbox file tool whose path escapes the workspace is rejected before any
  container is touched
  (`test_container_backend.rs::container_file_tools_reject_paths_outside_the_workspace`).
- **No secret reaches a rendered surface (FR-035).** A resolved credential value
  is registered with the shared redaction registry (`ragent-types::sanitize`) at
  resolution time; `RemoteBackend`'s hand-written `Debug` renders the bearer key
  as `[REDACTED]`; the secret-free backend `ConnectionDescriptor` records key
  *presence* and credential *names* only; the OpenAPI document declares the
  `bearerAuth` scheme with no value; `GET /config` redacts every credential-keyed
  field by shape. Covered by `test_hardening_invariants.rs`,
  `test_backend_registry.rs`, `test_sandbox_secrets.rs`, `test_openapi_document.rs`,
  and `ragent-server`'s `test_config_redaction.rs`.
- **The OpenAI-compatible surface inherits the native auth and never bypasses
  permissions (FR-024, FR-038).** `openai_routes()` is merged into the
  auth-protected router, so `/v1/models` and `/v1/chat/completions` require the
  same bearer token as the native API; the surface never sets `auto_approve`, so
  a tool that needs confirmation raises the ordinary prompt
  (`test_openai_surface.rs`).
- **Refuse to serve unauthenticated (FR-032).** When `openai.enabled` is true and
  no token resolves from `openai.token` or the ambient `RAGENT_TOKEN`, `ragent
  serve` refuses to start the surface with a non-zero exit and an actionable
  message rather than serving it unauthenticated (`src/main.rs`, the `Serve` arm).
  Covered by `tests/test_serve_openai_gate.rs`.

## 19E. Agent Client Protocol (`ragent-agent/src/acp/`)

ragent speaks ACP (Agent Client Protocol) in both directions (spec `openhands`
FR-015, FR-022, FR-028, FR-036).

### 19E.1 ACP client (T-007; FR-015, FR-022, FR-036)

`crates/ragent-agent/src/acp/mod.rs` drives an external coding agent (Claude
Code, Codex, Gemini CLI, or a custom ACP server) as a **subprocess** that speaks
JSON-RPC 2.0 over its stdin/stdout. `AcpClient::spawn` starts the command (the
child is `kill_on_drop`), `initialize` and `session/new` perform the handshake,
and `session/prompt` relays one turn, rendering each streamed `session/update`
through a callback. A turn dispatched to an ACP agent is relayed to it instead of
a local ragent LLM provider (FR-022); the bridge from `AcpUpdate`s onto the local
event bus lives in `crates/ragent-agent/src/session/acp_dispatch.rs`.

Failure handling is first-class (FR-036): a non-zero exit, a malformed stdout
frame, a JSON-RPC error reply, or a silent subprocess that outlives its turn
budget all fail the turn - the relay never blocks indefinitely. Configuration is
the `acp.agents` registry plus an optional `acp.default_agent`.

### 19E.2 ACP server on stdio (T-008; FR-022, FR-028)

`crates/ragent-agent/src/acp/server.rs` is the mirror image: it **serves** an
ACP-capable editor (Zed, VS Code, JetBrains) over JSON-RPC on ragent's own
stdin/stdout. Each editor session maps onto a local ragent session, and the
session's agent-loop events are streamed back as ACP `session/update`
notifications (FR-022):

| Request            | Behaviour                                                                                    |
| ------------------ | -------------------------------------------------------------------------------------------- |
| `initialize`       | Negotiates the ACP protocol version (`1`).                                                     |
| `session/new`      | Opens a local ragent session rooted at the editor's `cwd` (or the process working dir).        |
| `session/prompt`   | Runs one ragent turn, streaming `agent_message_chunk` / `agent_thought_chunk` / `tool_call` / `tool_call_update` updates, then replies with the mapped `stopReason`. |
| `session/cancel`   | Raises the turn's cancellation flag.                                                           |

**Feature-gated and disabled by default (FR-028).** The endpoint compiles only
behind the `acp-server` Cargo feature and runs only when `acp.server_enabled` is
`true` in the configuration. It is reachable only through the `ragent acp-server`
subcommand; when either gate is closed the command prints a diagnostic and exits
with a usage error (code 2) rather than serving. A malformed request frame is
skipped (never tears the connection down) and an unknown request method gets a
JSON-RPC "method not found". `acp.server_agent` selects the driven agent,
resolving `server_agent` -> `default_agent` -> the top-level `defaultAgent`.

```bash
cargo build --features acp-server
ragent acp-server
```

---

## 19F. Automation Service (`ragent-agent/src/automation/`)

The automation service turns a named, schedulable or webhook-triggered agent run
into a durable, backend-confined unit with run history (spec `openhands` T-016;
FR-013, FR-014, FR-018, FR-033). It builds on the existing agent loop and the
cron schedule grammar rather than replacing either (assumption A6).

### 19F.1 Definitions and triggers

An automation is declared under the `automation` block in `ragent.json`
(`crates/ragent-config/src/automation.rs`, `AutomationConfig`). The service is
inert unless the block sets `enabled: true` (`Config::automation_enabled`), so a
project that never opted in starts no scheduler and accepts no webhook. Each
`AutomationDefinition` carries an id, an agent, a prompt template (with
`{{payload}}` substituted at run time), a trigger, a backend, and optional
dispatch targets:

| Trigger | Fires from |
| ------- | ---------- |
| `{ "kind": "webhook" }` | An inbound `POST /auto/<id>` (FR-013) |
| `{ "kind": "schedule", "schedule": "every 2m" }` | The scheduler's due-time tick (FR-014) |

### 19F.2 Webhook ingress, scheduler, and run history

- **Webhook ingress (FR-013).** `crates/ragent-server/src/routes/automation.rs`
  registers `POST /auto/{id}` as a **public** route (like `/health`), because an
  external system such as a GitHub webhook has no bearer token. The request body
  is compacted to JSON and supplied to the run as prompt context.
- **Scheduler (FR-014).** `AutomationService::spawn_scheduler` ticks every
  `scheduler_tick_secs`, enqueues a run when a schedule automation's due time
  passes (skipping one whose previous run is still in flight), and advances its
  next-due time with `CronSchedule::advance_next_due`. The TUI (`ragent-tui`)
  and `ragent serve` each start one scheduler when the config enables the
  service.
- **Run history (FR-018).** `ragent-storage`'s `automation_runs` table records
  one row per run (`Storage::insert_automation_run` / `finish_automation_run` /
  `list_automation_runs` / `prune_automation_runs`). A terminal run captures the
  automation id, trigger kind, start/end times, outcome, selected backend, and
  an output reference; history is pruned to `run_history_cap`.

### 19F.3 Backend confinement (FR-033)

A run executes only in its configured execution backend. `AutomationService`
installs the selected descriptor as a per-session override
(`register_session_backend`) before the run's first turn; the session loop
applies it (`apply_session_backend_override` in
`crates/ragent-agent/src/session/loop_steps.rs` and
`remote_dispatch.rs`), so a container/remote run is routed there and a relay
failure never silently re-runs it on the host (FR-031). No part of the run
executes outside the selected backend.

### 19F.4 Third-party dispatch (FR-013)

`crates/ragent-agent/src/automation/dispatch.rs` builds a provider-shaped payload
(`payload_for`) for each `DispatchTarget` and POSTs it on run completion. The
supported kinds are `slack`, `github`, `linear`, `notion`, and `webhook` (a
generic JSON POST). A target names its credential's environment variable
(`token_env`); the value is read at dispatch time and never stored or logged
(FR-035). A target with no `url` is inert.

### 19F.5 Surfaces

| Surface | Command / path |
| ------- | -------------- |
| TUI | `/automation [list]`, `/automation runs <id>`, `/automation run <id>`, `/automation help` |
| CLI | `ragent automation list\|runs <id>\|run <id>\|help` |
| HTTP | `POST /auto/{id}` (public), `GET /automation`, `GET /automation/runs/{id}`, `POST /automation/{id}/run` (bearer) |
| OpenAPI | The three admin routes and the webhook route are in the route table (`crates/ragent-server/src/routes/openapi.rs`) |

See `docs/howtos/slashcommands/automation.md` for the full manual.

---

# Part VII: Operations & Reference

---

## 20. Auto-Update Mechanism

An automatic update check is on the roadmap. The TUI already reserves the `/update`
and `/update install` slash commands, but they currently return a "not implemented"
message. When completed, this subsystem will:

- Query a GitHub releases endpoint for the latest version tag.
- Compare against the compile-time version baked into the binary.
- Offer a one-command download-and-replace flow for supported platforms.

Until then, users should update via `cargo install` or by downloading the latest
release binary manually.

---

## 21. Harness Enhancements

This section captures cross-cutting harness-layer behaviours that the
implementation exposes for operators and CI/CD pipelines.

### 21.1 Skill Catalog (`SkillCatalog`)

The agent does not load full skill bodies into the system prompt at session
start. Instead, `SkillRegistry::load()` first discovers skill metadata and
produces a compact `SkillCatalogEntry` for each skill via
`SkillRegistry::catalog()`. The catalog contains only the skill name,
description, trigger phrase, scope, and invocation flags; it is cheap to build
and keeps the startup context small. Full skill bodies (prompts, context
files, and tool lists) are loaded lazily when a skill is invoked.

### 21.2 Per-Run Cost Summary (`RunCostSummary`)

At the end of each `process_user_message` turn, the agent accumulates token
usage records, applies the built-in (or configured) per-model price table, and
publishes `Event::RunCostSummary` on the event bus. Consumers such as the TUI
and the HTTP SSE stream use this event to surface per-run spend without having
to accumulate per-request `TokenUsage` events themselves. The summary includes
input tokens, output tokens, estimated USD cost, model identifier, and
wall-clock duration.

### 21.3 Dry-Run Readiness Report

The `ragent --dry-run` / `ragent config check` flow builds a
`ReadinessReport` by loading and merging configuration, resolving provider/model
auth state, discovering skills via the skill catalog, enumerating visible tools,
and performing lightweight MCP connectivity checks — all without invoking the
LLM or executing any tool. The report is rendered as human-readable text by
default and as JSON when `--json` is supplied. See `QUICKSTART.md` for usage
examples.

### 21.4 Test Layout: One Test Binary per Crate

`cargo`'s default autotest discovery turns each top-level `tests/*.rs` file into
its own binary. At this workspace's scale that was ~948 integration binaries and
~948 link steps per full `cargo test`. Every crate (and the root `ragent`
package) therefore sets `autotests = false` and declares exactly one integration
target:

```toml
[package]
autotests = false

[[test]]
name = "all"
path = "tests/all.rs"
```

```rust
// crates/<crate>/tests/all.rs
#[path = "test_agent_memory_deep.rs"] mod test_agent_memory_deep;
#[path = "test_automation_dispatch.rs"] mod test_automation_dispatch;
// ... one `#[path]` line per former top-level test file
```

`#[path]` inclusion is what makes this a pure packaging change: it anchors
module resolution at the child file, so plain `mod support;`, nested
`#[path = "../src/<module>.rs"]` source re-imports, `include_str!("support/..")`
and inner attributes (`#![forbid(...)]`, `#![cfg(feature = ...)]`) all keep
working. Test names keep a `test_<file>::<fn>` shape, so one file is run with
the filter form `cargo test -p <crate> --test all test_foo::`. The workspace
compiles **19 integration test targets instead of ~948**, and `BUILDSTATS.md`
records the resulting `cargo test --workspace --all-targets` cost (13,862
passed, 0 failed, 16 ignored across 40 reporting targets) and the re-measured
coverage (86.58% workspace line coverage, all 18 crates above 85%).

Three consequences are part of the contract:

- **Merging binaries merges process state.** Each `tests/all.rs` defines a
  binary-wide, poison-tolerant `global_env_mutex()` / `global_env_lock()`, and
  every test that mutates or observes a process-global (cwd,
  `XDG_CONFIG_HOME`, `HOME`, `PATH`, a `static` registry) contends on it.
  `#[serial]` remains acceptable for globals that need no value control.
- **`crate::` in a `#[path]`-included source module resolves at the binary
  root**, not at the crate under test. Where a child includes `src/foo.rs` that
  refers to `crate::bar`, `tests/all.rs` re-exports `bar` (the shims at the top
  of `crates/ragent-agent/tests/all.rs` are the pattern) rather than duplicating
  the module.
- **Tests must not write into the repository.** Three write paths were made
  injectable so a test run leaves the working tree byte-identical: the research
  vault root is explicit `AppState::research_root` state (production passes
  `<working dir>/research`, tests pass `target/temp/ragent-server-research/`),
  `session::history::set_subagent_log_root` redirects diverted `wait_agents`
  batches, and `ragent_tui::app::set_log_root_override` /
  `use_scratch_log_root_for_tests` redirect the TUI log-window spools. All three
  are `#[doc(hidden)]` `OnceLock` setters — production never calls them, so the
  shipped defaults (`<working dir>/log`, `<working dir>/research`) are unchanged.

The review, the measurement method and the per-crate rollout record live in
`docs/reports/testconsolidation-review.md`.

---

# Appendices

---

## Appendix A: Version History

| Version | Date | Highlights |
|---------|------|------------|
| v1.0.158 | 2026-10-30 | **CI `Dead-code lint` fix; no behaviour change.** The job runs `cargo check --workspace --lib --all-features` with `RUSTFLAGS='-D unreachable_pub -D dead_code -D unused_imports'`, and 57 items were declared `pub` inside private modules with no public re-export path, so `unreachable_pub` failed the build (`ragent-research` `analysis::{parser,prompt}` 17, `session::{fallback,topic}` 16, `web_gatherer::{relevance,title}` 7 and `polarity` 5; `ragent-agent` `session::refusal_outcome` 4 — the `ToolOutcome` alias and `refusal`/`denial`/`synthesised`, superseded by `ToolDispatchPipeline`; `ragent-tui` `app::slash` 6 MCP helpers, `app::plugin::toggle_plugin_enabled`, `app::event_handler::QUESTION_TIMEOUT_SECS`). Each was demoted to `pub(crate)`, the exact reachability it had; nothing is referenced from another crate, a bench, or a test through a public path. Verification: `cargo check --workspace` clean, `cargo audit` exit 0 across 867 dependencies with the single `paste` unmaintained notice the only allowed warning, the exact CI lint command at exit 0, `cargo nextest run -p ragent-research` 1175/1175, `-p ragent-agent` 2560 passed / 1 skipped, `-p ragent-tui` 3901/3901, CI's clippy invocation (`-D warnings -A clippy::used_underscore_items -A clippy::redundant_pub_crate -A clippy::wildcard_imports`) clean, `cargo fmt --check` clean, and the twelve `scripts/check-*.sh` guards passing. |
| uncommitted | 2026-10-31 | **OTLP export failures become visible and self-healing: an `Agent Notice` per failed export, and telemetry off after 5 consecutive failures.** `ragent-telemetry::export_health::ObservedMetricExporter` decorates the live `opentelemetry_otlp::MetricExporter` inside `build_provider` and records every export outcome in a process-wide `ExportHealth` monitor: success (resets the streak), non-2xx collector response, transport error, or export timeout (classified from the SDK error text). The wrapped result is forwarded unchanged, so the SDK's own `otel_error!` logging is untouched. `App::poll_telemetry_export_health` (driven from the TUI event loop) drains the monitor and appends one `Agent Notice` bubble per failure with the consecutive count, the reason (e.g. `HTTP export failed with status code: 503`), and the userinfo-masked endpoint; on the 5th consecutive failure it runs the `/telemetry off` path (`set_telemetry_enabled(false)`, which persists `telemetry.otel.enabled: false` and reconfigures the live subsystem), clears the latched flag, and sets the status to `telemetry: disabled after repeated export failures`. `TelemetrySubsystem::reconfigure` clears the monitor, so `/telemetry on` retries with a clean count. `/telemetry counters` and the telemetry panel open with an `Export health` line (`N consecutive failure(s), M total`) once anything has failed, and `/telemetry help` documents the policy. This release also carries the two fixes verified in the same session: the periodic reader must be the async-runtime variant (`periodic_reader_with_async_runtime::PeriodicReader` with `runtime::Tokio`), because the `reqwest-client` OTLP exporter's export future calls `tokio::time::sleep` and the plain OS-thread reader panicked with "there is no reactor running" on the first export, and the TUI panic hook only tears down the terminal when the panicking thread is the UI thread, so a helper-thread panic no longer destroys the alternate screen. Verified live against a mock collector answering `503` (24 responses served, exactly 5 notices, `TELE:` `✓` -> `✗`, config persisted `enabled: false`), plus `cargo nextest run -p ragent-telemetry -E 'test(test_export_health::)'` 13/13 and `-p ragent-tui -E 'test(test_telemetry_export_notice::)'` 4/4. |
| v1.0.157 | 2026-10-30 | **CI fixes and code-audit follow-up; one runtime fix, otherwise behaviour-neutral.** Autopilot no longer keeps running after the primary agent calls `agent_complete`: the `Event::MessageStart` arm cleared the `last_task_completed_at` latch unconditionally, and the lifecycle events the agent loop publishes after the completion could let a later `MessageStart` wipe the latch, so the next `poll_autopilot_continue` wake saw no completion with a continuation pending and started another turn; the clear is now guarded on `autopilot_enabled`, with regression test `test_event_handler_more::message_start_after_agent_complete_keeps_autopilot_off`. The `test_openai_surface::test_tool_call_raises_permission_prompt_and_is_not_auto_approved` wait is de-flaked: a single 30-second deadline replaces the per-event 200 ms window that broke on the first expiry (CI failed at 0.251 s), `Lagged` is drained rather than terminal, and the request future is raced so a genuine FR-038 regression still fails in milliseconds. Every clippy `redundant_pub_crate` finding is cleared workspace-wide by narrowing `pub(crate)` items to the visibility their module actually grants (`ragent-tools-vcs` `github`/`gitlab` `helpers`, `ragent-research` `analysis::{parser,prompt}`, `session::{topic,fallback}`, `web_gatherer::{relevance,title}` and `polarity`, `ragent-agent` `session::refusal_outcome` plus `session::{permissions,prompt_builders}` and `tool::team_memory_common`, `ragent-tui` `app::{event_handler,plugin,slash,state}`), and `App::discovered_mcp_enabled` gains a `BuildHasher` parameter to clear `implicit_hasher`; no behaviour change. Verification: `cargo check --workspace` clean, `cargo audit` exit 0 across 867 dependencies with the single `paste` unmaintained notice the only allowed warning, `cargo nextest run --workspace --no-fail-fast` 15,140 passed / 0 failed / 16 skipped, `cargo test --workspace --doc` 316 passed, clippy `-D warnings`, `cargo fmt --check` and the twelve `scripts/check-*.sh` guards all clean. |
| v1.0.150 | 2026-10-26 | **The `uniformtools` spec lands: one canonical tool-call model across every provider, Claude-compatible tool spellings, and per-provider wire profiles.** A single ordered dispatch pipeline (`session::dispatch`) decodes a call's arguments exactly once and owns the tool-call lifecycle end to end, so no provider builds its own message array (FR-014) and a tool result's wire form is produced in one place; the rename table (`tool/alias_table.rs`, `tool/rename.rs`) makes the 13 adopted Claude names canonical while every inbound alias still resolves (`scripts/check_tool_rename.py` guards the set), and `ToolWireProfile` carries the per-provider capabilities (`supports_parallel`, extended-thinking interplay) that were previously declared and ignored. Provider-specific fixes: Anthropic's extended thinking requires a specific message shape (FR-011); the Responses-API input list had no item for two shapes (FR-018); several providers report usage differently and are now normalised (FR-022); no alias survives the rename on any inbound path. Also in this release: `.config/nextest.toml` consolidates the test suite onto `cargo nextest` with per-test timeouts and cross-process test groups, CI moves to the `ms0-regression` job shape, the `_npx` cache repair takes a cross-process `flock`, `cargo deny`/audit config moves to `.cargo/audit.toml`, and `AGENTS-LANGUAGE.md` absorbs the Rust and test-layout rules. |
| v1.0.151 | 2026-10-27 | **Workspace-wide security hardening from the fresh `docs/reports/code-audit.md` (81 findings, 21 high), plus `quick-xml` 0.42, a `rust-version` floor of 1.91, `/mcp remove`, and a stable `ms0-regression` CI job.** A credential no longer reaches a log line, an SSE stream or a `{:?}` dump: `InputField` and `AzureResourceEntry` lose their derived `Debug` (a hand-written impl reports `<redacted>` plus a length, and the key as set-or-not) because both sat inside `Debug`-derived app state; the sudo askpass password is masked in the TUI question dialog and replaced outright in `event_to_parts` rather than pattern-redacted (a new `secret: bool` on `QuestionRequested`/`QuestionAnswered`, `serde(default)`, with the in-process event keeping the real value the helper reads from the bus); hook `stderr`, dynamic-context `stderr` and TaskCreated-hook feedback route through `redact_secrets`; user memory content is logged as a char count and hash instead of a prefix; the startup secret registry gains `GEMINI_API_KEY`/`GOOGLE_API_KEY`/`XAI_API_KEY`/`OPENROUTER_API_KEY`/`HF_TOKEN`/`AZURE_AI_FOUNDRY_API_KEY`/`AWS_ACCESS_KEY_ID`/`AWS_SECRET_ACCESS_KEY`/`AWS_SESSION_TOKEN`/`RAGENT_TOKEN`; a new `CONNSTRING_PATTERN` second pass masks `scheme://user:pass@host` (and `get_env` covers `URL`/`DATABASE`/`DSN`/`CONN` by name); `.gitignore` covers `azureresources.json`, `.mcp.json`/`mcp.json`, root `ragent.json`, `*.crt`/`*.cer`/`*.der`, `node_modules/` and `*.log`. **The masterfetch SSRF test bypass is compile-time**, behind the new `test-ssrf-bypass` feature of `ragent-tools-extended` (absent from `default` and from every release build; `pre-flight.sh` and the CI workspace test pass it explicitly), so a shipped binary cannot be talked out of its guard by `RAGENT_TOOLS_EXTENDED_TEST_NO_SSRF`. **`/mcp remove <id>`** lands on a new `Config::edit_global_config(edit)` (raw-JSON read/mutate/write so unrelated keys survive), refuses a plugin- or connector-bridged id by name (`/plugins remove` / `/connectors remove`), and clears the enable-ledger row; `/mcp`, `/mcp config` and the discover dialog now name the manifest that declares each server (`McpDiscoverySource::Plugin`/`Connector` plus `DiscoveredMcpServer::definition_file`), and one `app::mcp_bridge` module resolves both owners so `/mcp connect` accepts a connector-bridged id. **Fixes:** native Ollama rejected any request with a text-free message (`joined_text` emitted `[]`; Ollama declares `content` as a Go `string`) - the value is always a string now; the `_npx` cache repair runs once per process before the first child spawns and takes a cross-process `flock` (fixing the `connection closed: initialize response` that a poisoned retired directory produced); a dead background reader, a failed Prometheus scrape write, a failed `!cmd` and a failed agent resume log at `warn!` with structured fields instead of `debug!`; `encrypt_key_with_context` carries the AEAD error and AAD length into its log; the `ms0-regression` M0.5 step filtered on the `test_channels::` suite deleted in 1.0.130 (`cargo nextest` exits 4 on an empty filter) and now runs `test_mf_security::` + `test_legacy_web_ssrf::` + `test_mf_supplement::` (139 tests); five test files that no crate `tests/all.rs` included are compiled again. **Dependencies:** `quick-xml` 0.39 -> 0.42 clears RUSTSEC-2026-0194/-0195 with both suppressions removed from `.cargo/audit.toml` and `deny.toml` (the cited `ooxmlsdk` constraint was stale - it was never a dependency), `tokio` 1.54.1, `libc` 0.2.191, `uuid` 1.29.0. The status bar loses its hardcoded `HEALTHY` git dot and `render_status_bar_v2` becomes `render_status_bar`. Verification: `cargo check --workspace` and `--all-targets --features ragent-tools-extended/test-ssrf-bypass` clean, `cargo fmt --check` clean, `cargo audit` clean (867 dependencies), `cargo deny check advisories` ok. |
| v1.0.156 | 2026-10-30 | **Version bump only; publishes the `docs/reports/code-audit.md` remediation pass prepared under the `1.0.155` version string.** No source, dependency or behaviour change: the four duplicated guards stay collapsed onto their shared implementations (`providers::http_client::append_chunk_guarded`, `providers::ANTHROPIC_VERSION`, `ragent_surface::marketplace_ledger`, `ragent_server::DEFAULT_LISTEN_ADDR`), the strengthened `check_shared_guards.py` and `check_security_unwraps.py` stay in force, `/inputdiag` stays removed (registry at 150 tools across 21 families), `#![deny(unsafe_code)]` covers all 19 crate and binary roots, and the runtime-flag toggles keep persisting through `RuntimeFlag::persist_to_global`. The user-facing documentation is brought into line (`STATS.md` re-measured to 1,893 Rust files / 733,659 lines / 1,198 test files and ~15,092 crate tests plus 67 in the root `src/`+`tests/` tree, 15,159 workspace-wide; `SPEC.md`, `README.md`, `QUICKSTART.md`, `TUI-QUICKSTART.md`). Verification: `cargo check --workspace` clean, `cargo audit` exit 0 across 867 dependencies with the single `paste` unmaintained notice the only allowed warning, `cargo nextest run --workspace --no-fail-fast` 15,139 passed / 0 failed / 16 skipped, `cargo test --workspace --doc` 339 passed, clippy `-D warnings`, `cargo fmt --check`, the dead-code lint, `cargo machete` and `cargo deny check` all clean. |
| v1.0.155 | 2026-10-30 | **The `docs/reports/code-audit.md` remediation pass lands: four duplicated guards collapse onto shared implementations, the two guards that police that duplication are strengthened so it stays collapsed, and the audit's logging and containment nits are cleared. No user-facing capability changes beyond the removal of one debug-only slash command. **Duplication collapsed.** Every streaming provider spelled out the same two steps per chunk - decode with `append_stream_chunk`, then check `sse_buffer_exceeded` and log-then-fail - and nine copies had drifted (the warning named "SSE buffer" in eight and "stream buffer" in Gemini, and a new provider could omit the check entirely); the new `providers::http_client::append_chunk_guarded(buffer, pending, chunk, provider_label)` folds decode and cap into one call returning the exact terminal-error text, adopted by `anthropic`, `azure_resource`, `bedrock`, `copilot`, `gemini`, `huggingface`, `ollama`, `ollama_cloud`, `openai_responses` and `sse`. `providers::ANTHROPIC_VERSION` replaces the `"2023-06-01"` literal that `anthropic`, `azure_resource` and Bedrock each carried. The two `marketplace_ops.rs` files were near-identical ports of each other - `matches_entry`, `global_config_path`, `read_or_empty_object`, `marketplaces_array_mut`, `add_marketplace`, `remove_marketplace`, differing only in the JSON root key - so they now live once in the new `ragent_surface::marketplace_ledger`, parameterised by `root_key`, with each crate keeping a thin wrapper (`ROOT_KEY = "plugins"` / `"connectors"`) plus the marketplace-document path that genuinely differs (`ragent-plugins/src/marketplace_ops.rs` drops 165 lines). `ragent_server::DEFAULT_LISTEN_ADDR` replaces the `"127.0.0.1:3000"` literal in the `serve --addr` default. **Guards strengthened.** `scripts/check_shared_guards.py` gains four duplicate patterns (a local `fn truncate_chars`, `fn truncate_bytes`, `fn marketplaces_array_mut`, `fn read_or_empty_object`) naming `ragent-types/src/strutil.rs` and `ragent-surface/src/marketplace_ledger.rs` as the single permitted homes, and `DELEGATION_TARGETS` was extended to all four so an allowlisted adapter site is accepted only when its function body actually references the shared helper - a re-declared wrapper cannot defeat the rule. `scripts/check_security_unwraps.py` no longer counts doc-comment lines (`///`, `//!`): their illustrative examples such as `parse(s).unwrap()` are not call sites, and counting them let a file with many examples absorb a genuine new panic and stay under its grandfathered baseline (a netting-out bypass, audit Medium 4); the baseline was regenerated against the corrected counter, dropping 65 lines of stale entries. `deny.toml` and `.cargo/audit.toml` agree on an empty ignore set - the `RUSTSEC-2024-0436` (`paste`) suppressions were removed from both because no advisory matches the current tree and cargo-deny reported them as `advisory-not-detected`, which is dead configuration that only masks a future re-introduction. `#![deny(unsafe_code)]` is now on all 19 crate and binary roots (`ragent-types`, `ragent-plugins`, `ragent-connectors`, `ragent-agent`, the rest of the workspace and `src/main.rs`), matching what `AGENTS-LANGUAGE.md` already documented as universal, so a new `unsafe` block fails to compile until a scoped `#[allow(unsafe_code)]` with a `// SAFETY:` invariant is added. **Removed and corrected.** `/inputdiag` - a troubleshooting leftover that dumped internal widget, cursor and selection geometry into the transcript - is gone from `SLASH_COMMANDS`, the `app/slash.rs` dispatch, the agent-side `tool/command_catalog.rs` mirror and the four how-tos that documented it (`docs/howtos/slashcommands/inputdiag.md` is deleted); the registry stays at 150 tools in 21 families. New `strutil::truncate_chars_no_ellipsis`/`truncate_bytes_no_ellipsis` are the full-budget siblings of `truncate_chars` (which reserves three of the budget for `...`), and `ragent-tui/src/widgets/message_widget.rs` plus `ragent-agent/src/task/mod.rs` move onto them - fixing their visible-character budget so the bash tool-call header keeps a true 250 command characters plus the marker rather than 247. `.gitignore` gains `*.db-journal`/`*.sqlite-journal`/`*.sqlite3-journal` (rollback journals hold the same plaintext pages mid-write that `*.db-wal`/`*.db-shm` do), `credentials.toml` (Cargo registry tokens), `id_rsa*`/`id_ecdsa`/`id_dsa`/`*.ppk`, and `assets/config/ragent.json{,. *}` - the archive there unpacks to the same credential-bearing file the root `ragent.json` anchor covers and which that root anchor does not reach. **Logging hygiene.** `handle_loop_error` logged one failure twice at two levels (an `error!` detail line plus `terminate_loop_with`'s own `info!`); the duplicate is gone. The cancellation path's `warn!` and separate `info!` timing line are merged into one `warn!` carrying `step` and the `total_ms`/`model_wait_ms`/`other_ms` split. The pre-send compaction decision - a six-field struct logged on every step of every turn - now emits one condensed line and only when `should_compact` is true. **Runtime-flag persistence.** `/editlog off` and Alt+E no longer revert at the next start: the toggles shared one persistence helper that wrote back to whichever config file the session had loaded (the project `.ragent/ragent.json` whenever one existed), which is not the file `Config::load` is guaranteed to read, so a session started in another directory saw the old value; `edit_log`, `activity_log` and `yolo` now persist through `RuntimeFlag::persist_to_global` into the user-global config with every other key byte-identical, `gcf` keeps the `save_to_source` path, and the old behaviour survives as `RuntimeFlag::persist_to_source` for callers that must edit a project file. A project `.ragent/ragent.json` that omits `activity_log` no longer re-enables logging: the overlay only applies the value when it explicitly disables it, so an explicit `false` wins and an omitted key leaves the lower layer alone. **Build and CI.** `pre-flight.sh` no longer fails on the agent-loop bench guard when no baseline exists - `check-bench-regression.sh` exits 2 for "no saved baseline", which is its documented not-applicable code and not a regression, and since `target/criterion` is git-ignored and never checked in that step failed on every run; the exit status is now mapped (2 skips, 0 passes, anything else fails), so a genuine regression still fails and the skip keeps pre-flight no stricter than CI, which does not run the guard at all. The `undelivered_messages` doctest's scratch root now resolves from `CARGO_MANIFEST_DIR` and plants its own `.ragent` so `find_project_teams_dir` stops there, instead of landing the team in the checked-in tree and failing the second run with "team 'doc-team' already exists". `.config/nextest.toml` gains a `clipboard` group (`max-threads = 1`, both profiles) because the clipboard cases each write the one host clipboard and read it back and a file-local `static Mutex` cannot order them across nextest's per-test processes. Verification: `cargo check --workspace` clean, `cargo audit` exit 0, `cargo fmt --check` clean, `pre-flight.sh` green both in `--quick` (4 passed / 0 failed / 2 skipped) and in full (7 passed / 0 failed / 1 skipped), `check-shared-guards.sh` green with `--self-test` still failing a seeded duplicate, and `cargo nextest` at 933/933 for plugins+connectors+surface and 6,460/6,460 for tui+agent. |
| v1.0.154 | 2026-10-29 | **CI fixes only; no production capability changes.** Two failing 1.0.153 CI jobs are fixed at their cause. **`TeamManager`'s watchdog no longer touches disk when it has nothing to police.** `tokio::time::interval` fires its first tick immediately, and that tick's first act was a config read whose companion lock file is opened with `create(true)` - so a freshly created team, whose handle map is empty and which therefore has nothing to police, still performed one `open` per tick forever, and that `open` *recreated a file* inside a tree `remove_dir_all` was walking; the removal then failed with `ENOTEMPTY` (`test_slash_arms_deep::team_forcecleanup_with_missing_directory_reports_failure`). The tick now returns before any disk access when no teammate handle is registered, which removes the pointless per-tick `open` and closes the race with a concurrent teardown. **`scripts/check-team-duplication.sh` accepts the two ledgers 1.0.153 added** (`hook_ledger.rs`, `status_history.rs`): its `RUNTIME_MODULES` allowlist still listed the pre-1.0.153 set, so both tripped the "unexpected file in the team runtime directory" check in the `Security guards` job - the guard was correctly reporting drift and the allowlist was what needed to move; `--self-test` still fails a seeded extra runtime file and a re-introduced `ragent-team` shim crate, so it has not been weakened. **The clipboard tests no longer race over the one host clipboard** (`.config/nextest.toml`): each writes the single system clipboard and reads it back, so two running at once see each other's text, and the file's own `CLIPBOARD_TEST_LOCK` could not order them because nextest gives each test process a private copy of that `static Mutex`; a `clipboard` group with `max-threads = 1` now holds the exclusion across processes in both the `default` and `ci` profiles. Also folds in the dead-code cleanup that made `cargo check --tests` warning-free (a stray `team_support` module declaration removed, three shared helpers given documented `#[allow(dead_code)]` because each `#[path]` inclusion is its own module) and restores `test_teams_tui::test_team_delete_of_a_parked_roster_needs_the_explicit_force`, which had lost its `#[tokio::test]` attribute and had therefore never been running. Verification: `cargo check --workspace` clean, `cargo audit` clean across 867 dependencies, clippy and `cargo fmt --check` clean, every CI guard script green, and the full `cargo nextest run --workspace --no-fail-fast` suite at 15,113 tests with 0 failures. |
| v1.0.153 | 2026-10-29 | **The `teamssetup` / `agentssetup` state machines are tightened to rules the previous release only documented, and the Agents panel gets a config section.** `TeamManager::set_member_status` becomes the single member-status writer: it validates the move against the transition table, applies the destination status's side effects, persists the roster once, appends one record to the team's new `status-history.jsonl`, and publishes exactly one `Event::TeammateStatusChanged { from, to, cause }` (nine direct `member.status = ...` sites consolidated); `MemberStatus` gains `is_terminal` / `is_parked` / `is_live`, and every "is this teammate still active?" guard uses one of them. `TaskStatus::can_transition_to` + `transition_task` become the single task-status writer behind one legal-transition table. `team_task_complete` now requires ownership (the task's `assigned_to` or the lead) and refuses with `rejection_reason` `not_owner` / `not_claimed` / `already_completed` without re-assigning; team adoption requires explicit consent (`adopt: true`), so two sessions driving one team directory no longer quietly compete. `team_task_cancel` lands (lead-only, `pending`/`in_progress` only, owner notified, task kept not deleted), taking the registry to 150 tools in 21 families. `TaskList::validate()` refuses duplicate ids, dangling `depends_on` edges and dependency cycles before any `tasks.json` write, naming the task and chain and leaving the file byte-identical. A hook that cannot run is recorded in `hook-failures.jsonl` rather than silently reported as `Allow`, and its count surfaces in `/team show` and `team_status`. `team_status` also gains per-member status history (last five transitions from `status-history.jsonl`), `Undelivered messages` for a terminated member's stranded mailbox, and `Plans awaiting a verdict`. Four new lifecycle events reach the bus and SSE: `TeammateStopped`, `TeammateStatusChanged`, `TeamTaskCompletionRejected`, `TeamAdopted`. Every sub-agent tool (`new_agent`, `cancel_agent`, `list_agents`, `wait_agents`, `bg`) now enforces its declared closed parameter set through the shared `tool::param_schema::validate_tool_input`, so a misspelled `backgound: true` fails by name instead of silently running the foreground contract. TUI: `Alt+A` / `Alt+T` toggle their windows unconditionally, `/spawn` moves onto a shared bounded launch set (`agents.max_launching`, default 4), and the Agents panel's completed-history section is capped by `agents.completed_history` (default 20). Fixes: the `--web-time` invalid-integer diagnostic now echoes the spelling the caller typed, and `pulldown-cmark` is moved 0.13.4 -> 0.13.5 to clear RUSTSEC-2026-0414. Verification: `cargo check --workspace` clean, `cargo audit` clean across 867 dependencies. |
| v1.0.152 | 2026-10-28 | **Typed `--age` publication window, a `DateTime` tool, and the `Alt+O` autopilot toggle.** `/research create --age <SPAN>` carries a parsed `AgeSpan` (`ragent-research/src/source_age.rs`, `[n]d`/`[n]m` = 30 days/`[n]y` = 365 days) instead of a raw string: it derives `Deserialize` through `#[serde(try_from = "String")]`, so a JSON request body carries the command-line spelling and an invalid span is rejected - `POST /research` answers 422 rather than serving an unfiltered run, and the root CLI rejects one with `CliExit::usage()` (exit 2) via `parse_age_span`. `InvocationParseError::InvalidAgeSpan` is replaced by `InvalidArgument(String)`, so a recorded bad flag replays as a named diagnostic instead of the misleading `MissingName`. The window's start date is appended to every decomposed query as `after:YYYY-MM-DD` (`SEARCH_QUERY_AGE_OPERATOR`, public, with `search_query_age_hint`); the LLM decomposer is now told today's date and forbidden from writing training-data years or relative phrases, because the model has no clock and was steering every engine towards stale material. `mf_search` gains `published_after` (`SearchOptions::with_published_after`), an exact `YYYY-MM-DD` bound the coarse `freshness` enum cannot express; Exa's `startPublishedDate`, Tavily's `start_date` and Serper's `cdr:1,cd_min:` use it in preference to `freshness`, and `AgentWebSearchTool::search_with_date_bound` reaches it together with `exclude_engines` in one call. A value that is not `YYYY-MM-DD` is dropped with a warning rather than becoming a literal engine parameter. `mf_fetch`'s `parse_mf_fetch_output` returns a named `ParsedFetch` struct instead of a seven-element tuple behind `#[allow(clippy::type_complexity)]`, and a new `envelope_published_at` reads `metadata.published_time` (falling back to `modified_time`) so PDF and YouTube bodies with no readable `<head>` still date themselves for the filter. New `DateTime` tool (family `none`, registry 148 -> 149 in 21 families): the RFC 3339 UTC instant, the same instant at an optional fixed UTC offset, date/time fields, day of week and Unix timestamp, read from the system clock and deliberately independent of `TZ` and the host tz database; the TUI summaries it as a `UTC+HH:MM` zone. `Alt+O` toggles autopilot (was the Telemetry panel) through new shared `App::start_autopilot`/`App::stop_autopilot` helpers that `/autopilot on|off` was also refactored onto, applying a default 1800 s limit so a stray chord cannot start an unbounded run; the `AUTO:` indicator is bold with an `(Alt+O)` hint and `push_indicator` gained a `hint` parameter plus `colors::HINT`. Every research integer flag now reports `"{flag} {value}: expected an integer"`. The duplicated four-arm cache-key `match (exclude_engines, published_after)` collapsed into `search_cache_key` joined with `CACHE_KEY_SEP = '\u{1f}'`. `cargo check` clean; `cargo audit` reports no advisories (867 crates, 1380 advisories loaded). |
| uncommitted | 2026-10-22 | **Tool visibility removed.** The per-family `tool_visibility` config block (`ToolVisibilityConfig`, `ToolVisibilitySpecified`, `iter_switches`, `tool_family_names`, and the sync between `/codeindex on|off` and `tool_visibility.codeindex`) is deleted, together with the `ToolRegistry::set_hidden` / `hidden()` / `hidden_definitions()` surface and the `/tools <switch> on|off` dispatch arm. Every registered tool is now advertised to the model unconditionally: the definitions sent with each request are the registry's definitions and the system prompt lists the same set, so a tool is absent only when the subsystem that registers it is inactive. `/tools` is reduced to a read-only registry report - `/tools` (help), `/tools list` (alias `show`), `/tools help`; `TOOLS_SUBS` is `["list", "show", "help"]`. The orphaned name-level filter goes with it: the `hidden_tools` config field, `Config::effective_hidden_tools()`, its `merge` union and the `/config show` union-field label are deleted, so a config carrying the key now has it ignored as an unknown key. `DryRunInputs` loses its `hidden_tools` field, so `ragent run --dry-run` no longer reports a hidden-tool count. All documentation that described the switches is removed or rewritten (`docs/howtos/tool-visibility.md` is deleted; `README.md`, `SPEC.md`, `QUICKSTART.md`, `TUI-QUICKSTART.md`, `docs/howtos/{config,permissions,reactagent,office,codeindex,plugins,tutorial}.md` and `docs/howtos/slashcommands/{tools,INDEX,config,reload,codeindex,plugins}.md` follow). |
| v1.0.149 | 2026-10-21 | **The `/provider` and `/model` setup dialogs are rebuilt on the catalogue list chrome, `/connectors claude` adopts the same design language, and the test suite gains a process-global isolation guard (specs `agentui`, `connectors` FR-047, `pluginstores` FR-054).** Every list-bearing `ProviderSetupStep` (provider, model, router-tier model, agent, configured-provider, provider-config and reset steps) now shares one composition: a slate accent frame titled with the step and subject, a heading strip with a live count, a pinned reversed-video column band, a cursor-filled zebra row list, and a muted key-hint footer; `SelectProvider` gains provider-name/provider-id/`Kind`/`Status` columns, `SelectModel`/`SelectRouterModel` right-align `Context` and `Cost` and spell out the `R`/`V`/`T` legend, and every selector windows around its cursor (the thinking-level list two lines at a time) so a list longer than the dialog keeps the highlighted row painted with `Showing a-b of N` in the legend. A mouse notch over an open setup dialog steps the cursor through the same `handle_provider_setup_key` path and is swallowed elsewhere. The `/connectors claude` browser drops its bespoke layout for the shared `STORE_*` chrome (renamed from `PLUGIN_STORE_*` and now used by both panels), gains a `Categories` navigator above a `Marketplace` navigator with `Name (N)` rows and an `Installed` pseudo-category, three focusable panes (`CataloguePane::Results`/`Categories`/`Marketplaces`), a pinned column-heading row, a one-column bold-green installed lane, and a bottom search box with `[x] clear`; the plugin-store navigators gain the same `Installed` pseudo-category (FR-054). `scripts/check-test-env-isolation.sh` (with `--self-test`) enforces test env isolation across the per-crate `tests/all.rs` binaries, wired into `pre-flight.sh` and CI. `rustls-webpki` 0.103.16 and `webpki-root-certs` 1.0.10 clear RUSTSEC-2026-0382/0383 with no suppression added. Tests: `test_setup_dialog_chrome` (14), `test_setup_selector_scroll` (7), `test_plugin_store_installed_category`, the 54-case connector-catalogue set, and the reworked window/footer suites; `cargo test --workspace` 14,418 passed / 0 failed / 22 ignored, `cargo fmt --check`, clippy, `cargo audit` and `cargo deny check` all clean. **Also released in this version:** the `Esc`-only modal dismissal - the Agents, Teams and output-view overlays lose the bordered ` Close ` bar, `window_frame` derives a one-row legend band instead, and every FR-077 legend ends with `Esc close` (FR-007, FR-018, FR-032, FR-074, FR-077); the keyboard-first Agents/Teams block cursor with the `/team forcecleanup` shutdown race fix (`seed_only` seeding, awaited per-member shutdown) and the 5000 s `TOOL_WATCHDOG_TIMEOUT` (FR-033..FR-046, FR-065, FR-069..FR-070); and the Agent and Team Member panel (FR-006..FR-026) with cooperative sub-agent suspension (FR-035), the widened text-format tool-call recovery (three more dialects plus reasoning-channel scanning), `/yolo on|off|status` with the `--config` write target, the discoverable `SPEC.md`-only spec directory, SEC-ragent-team-003, and the `same-file` Windows path-alias containment. |
| v1.0.141 | 2026-10-18 | Version bump. Session screenshot renamed to `assets/screens/ragentsession.png` and re-embedded in the README. |
| v1.0.140 | 2026-10-18 | Version bump. README screenshot additions: `ragentmcpstartup.png` and the session image. |
| v1.0.139 | 2026-10-18 | Status bar names the active execution backend in full (`Backend:<name>`, fixed cyan via `colors::BACKEND`); README gains screenshots of the provider, model, MCP-discover and plugin-selector panels; sandbox-backends how-to and status-bar assertions follow. |
| v1.0.138 | 2026-10-18 | **Test-binary consolidation (§21.4): every crate builds one integration-test target.** Setting `autotests = false` plus a single `[[test]] name = "all"` (`tests/all.rs`) that `#[path]`-includes each former top-level `tests/test_*.rs` file cuts the workspace from ~948 integration binaries to **19** (one per crate plus the root package) - the test-name sets are verified identical to the pre-change sets for all 19. Each `tests/all.rs` also defines one binary-wide, poison-tolerant `global_env_mutex()`/`global_env_lock()`, and every test that mutates or observes a process-global contends on it; `ragent-research` moved its `crate::document`/`crate::source` shims to `tests/all_prelude.rs`, `ragent-telemetry` uses the explicit `ragent_telemetry::TelemetryError` path, and `ragent-agent`/`ragent-tools-core` re-export the crate paths their `#[path]`-included sources import. CI's remaining `--test <name>` invocations were converted to the filter form `--test all <module::>`. **Tests no longer write into the repository:** the research vault root is explicit `AppState::research_root` state (production `<working dir>/research`; tests use `target/temp/ragent-server-research/` via the new `tests/research_sandbox.rs`), `session::history::set_subagent_log_root` redirects diverted `wait_agents` batches, and `ragent_tui::app::set_log_root_override` / `use_scratch_log_root_for_tests` redirect TUI log-window spools - all three are `#[doc(hidden)]` `OnceLock` setters so shipped defaults are unchanged. **Build:** incremental compilation is disabled workspace-wide (`.cargo/config.toml` `[build] incremental = false`, plus `[profile.dev]` and a new `[profile.test]` with `debug = "line-tables-only"`), since the `sccache` wrapper cannot cache incremental artifacts; `BUILDSTATS.md` records the measured per-step cost (release 7m 37s / 1.80 GiB, debug 2m 41s / 2.40 GiB, full test 9m 12s / 14.92 GiB, coverage 9m 58s / 15.82 GiB; 13,862 passed, 0 failed, 16 ignored across 40 reporting targets; workspace line coverage re-measured at 86.58%). **Coverage:** 17 new `ragent-tui` deep-coverage suites (642 tests, ~16.1k lines) exercise the slash dispatch arms, event swarm/research paths, input editing, layout/scroll maths, schedule helpers, message-widget summaries and session/model state. **Repo hygiene:** `examples/` is deleted (agent profiles now live in `.ragent/agents/`, team blueprints in `.ragent/blueprints/teams/` - `code-review`, `parallel-feature`, `debate`), `.ragent/Instructions/` joins the `.gitignore` exception list so the per-language instruction packs are tracked, and `AGENTS.md`/`AGENTS-LANGUAGE.md` gain the "Test Layout" contract with `docs/reports/testconsolidation-review.md` as the review record. Verification: `cargo check --workspace` clean, `cargo audit` clean (1338 advisories loaded, 866 crate dependencies scanned, no new findings). |
| v1.0.137 | 2026-10-17 | Test-coverage and documentation release (tests only, no production logic changed). The rebuilt `README.md` opens with a captured TUI screenshot and a feature list re-verified against the tree (15 registry providers, 148 tools across 21 categories, 17 built-in agents, 14 tree-sitter grammars / 50 language ids), and the accumulated release highlights list (about 830 lines) is replaced with a pointer to `CHANGELOG.md`; `AGENTS-RUST.md` is renamed to `AGENTS-LANGUAGE.md`. The `zip 7.2.0` advisories (RUSTSEC-2026-0367/0368/0369) are suppressed with a documented reason (no upgrade path; the affected extraction APIs are never called). Added ~80 new `tests/` suites - roughly 80 under `crates/ragent-agent/tests/` plus suites across `ragent-surface`, `ragent-types`, `ragent-telemetry`, `ragent-storage`, `ragent-codeindex`, `ragent-config`, `ragent-research`, `ragent-tools-vcs`, `ragent-bench`, `ragent-server`, `ragent-tools-core`, `ragent-tools-extended`, `ragent-llm`, and `ragent-tui` - and a set of inline test files in crates that already used that convention, taking 13 of the 18 crates to >=85% line coverage (workspace 69.8% -> 73.9%). `ragent-agent` 65.1% -> 85.6%, `ragent-tui` 54.1% -> 75.0%. Three `pub(crate)` items in `ragent-tools-vcs` (`run_status`, `filter_log_text`, GitLab `make_client`/`detect_project`/`detect`) were widened to `pub` so their helpers could be exercised directly. Adds `serial_test` as a `ragent-agent` dev-dependency and marks the inline perf tests `#[serial_test::serial]` (the `perf::tests` mutate process-global profiling atomics and raced under the multi-threaded `cargo llvm-cov` run). Fixes three coverage-test defects: `test_mcp_discover_panel.rs` could leak a test-binary MCP server into the developer's real `~/.config/ragent/ragent.json` (runs now pin `XDG_CONFIG_HOME` at a throwaway `target/temp/xdg-*` dir), a `Glob` test asserted the wrong walk-path shape (matches are reported relative to the containing directory), and a `gitlab` README-fallback test stubbed the wrong body shape (the `file_path` fallback fetches through the JSON `get()` path, so the wiremock body must be `set_body_json`). `cargo check` clean; `cargo audit` reports the pre-existing `zip 7.2.0` advisories (RUSTSEC-2026-0367/0368/0369) unchanged by this work. |
| v1.0.136 | 2026-10-16 | Interactive `/plugins list` panel (spec `plugins` FR-009, FR-030). `/plugins list [--verbose]` opens a `/mcp discover`-style overlay: the plugin table is pinned at the top (a fixed column heading and separator above a scrollable block-cursor list) and the capability blocks (contributions, unsupported capabilities, errors, totals summary, and - verbose - telemetry) are listed at the end of the same scrollable body. `Up`/`Down` (or `k`/`j`), `Home`/`End`, and `Esc` operate it and `Space` toggles the highlighted plugin's enabled state (enable/disable, persisted through the shared control path, with the row tinted yellow when enabled and the outcome shown on a feedback line, re-resolving the session's plugin-contributed slash commands); a scrollbar appears when the body overflows. The panel resolves live MCP tool counts exactly as the text report did, so the `MCP` / `MCP Tools` columns stay accurate; `/plugins list --mcp` and the disabled-subsystem notice stay text reports. One discovery path is shared with the CLI: `ragent_plugins::control` gains `build_list_view` / `PluginListView` (with a per-row `PluginListRow` id/enabled list) and `ragent_plugins::surface` gains `run_plugin_list_view`, and `render_list_with_mcp_tools` is now `build_list_view(...).render()`. New test `crates/ragent-tui/tests/test_plugins_list_panel.rs` (7 cases). |
| v1.0.135 | 2026-10-15 | Security-hardening pass closing the critical/high band of `docs/reports/security-audit.md` (a fresh workspace-wide read-only audit: 4 critical, 15 high, 34 medium, 29 low). **Untrusted project config (SEC-001/002/005):** `Config::merge_project` now drops a project overlay's `mcp` servers (spawned as subprocesses with no permission prompt), its `acp` section (agents spawned verbatim with attacker `command`/`env`/`cwd`, plus the editor-facing server), and every provider's `api` block (which redirects the credential and prompt stream); rejected keys log at `warn!`. **Benchmark argv (SEC-003):** `validate_fixture_command` validates the whole argv against a deny-list of eval/stdin entry points (`-c`, `-e`, `-r`, `--eval[=…]`, `-m`, `-`, `/dev/stdin`, `go run`) so an allowlisted interpreter cannot evaluate inline fixture text. **Connector boundary (SEC-009/071):** a `stdio` `command` is allowlisted (no shell/launcher, absolute path must name a known runtime) and an `env` credential must be namespaced to the connector id. **Symlink defeat (SEC-006/018/027/028/059):** new `ragent_types::guard::{reject_symlink_components, reject_symlink_escape, open_owner_only}`; `contained_join` canonicalises the deepest existing ancestor; applied to the specs writer, config/credential temp files, the updater, the crash marker/panic report/stderr spool (`O_NOFOLLOW` + atomic replace), askpass, and the TUI image source; `list`/`Glob` skip a symlinked directory. **Bounded HTTP I/O (SEC-014/032/033/036/042/065/066):** connect/whole-request timeouts, capped redirect chains, and 8 MiB streaming read caps in the MCP HTTP client, masterfetch fetcher, GitHub client, and open-access lookup; the SSRF guard re-runs per redirect hop; the GitHub client refuses a cross-origin URL and streams byte-capped downloads. **Verified update (SEC-043):** `download_and_replace` requires a SHA-256 (`RAGENT_UPDATE_SHA256` or `ReleaseInfo::sha256`) and refuses an unverified artifact. **Header allow-list (SEC-035):** `http_request` accepts only `accept`/`accept-language`/`content-type`/`user-agent`. **Bounded sinks (SEC-031):** plugin host-call sinks cap entries (1000) and entry length (16 KiB). **Telemetry (SEC-052/053/055):** `tool.name` shape-checked (64, `[A-Za-z0-9_.-]`), else exported as `unknown`; the OTLP endpoint is parsed once with `url`. **Owner-only writes (SEC-022/027/076):** secret temp files and the askpass response are `0o600`; the crash marker read is capped (64 KiB). Adds `ragent --version`/`-V` (answered before any side effect) and the shared `strutil::{is_http_url, bearer_auth_header, path_file_name, HTML2TEXT_WIDTH}`, `sanitize::is_secret_json_key`, and `ragent_surface::store::{read_ledger_bytes, quarantine_corrupt_ledger}` helpers, routed through every call site; `deny.toml` drops the stale `Unicode-DFS-2016`/`OpenSSL` license entries. `cargo check --workspace` clean; `cargo audit` reports no new advisories. |
| v1.0.134 | 2026-10-14 | `/simplify all` quality pass over the changed working tree, on top of the v1.0.133 delta. The 18th crate `ragent-surface` gains four shared modules: `config_splice` (`write_atomic_json`, a temp-file + `File::create` + `write_all` + `sync_all` + rename write so the payload is durable before the rename can expose it), `marketplace` (the one marketplace-source resolution shared by the `/plugins marketplace` and `/connectors marketplace` command families), `net` (the `https`-only redirect policy and endpoint validator), and `scan` (the generic store-scan failure type). The plugin/connector `marketplace_ops` writers delegate to `write_atomic_json` and the plugin `store_fetch::https_only_redirects` wrapper is deleted. The four hand-copied redacting `Debug` impls in `ragent-config` collapse into one `macro_rules! redacted_debug_impl!`; `sanitize::redact_json_value` routes through the borrow-or-own `redact_secrets_cow`; `check_path_within_allowed_roots_cached` takes a fast path when `dirs.allowed_roots` is empty; `Config::load`'s M-025 cache key reads `XDG_CONFIG_HOME` through `read_env`; the `masterfetch` `langsearch_key` read shares the env-reading `pick` closure; and the TUI de-duplicates the question-expiry helpers (`utils::now_unix_ms`, `QuestionRequest::resolved_default_index`, `state::resolve_default_index`, `strutil::format_mmss`) and removes the `unreachable!()` panic from the expiry poll. Also lands the `/mcp config` discovery-location report (§19.4.3, `McpClient::discover_locations`) and the `AskUserQuestion` `default`/fallback option with the 120-second inactivity timeout (FR-045). |
| v1.0.133 | 2026-10-12 | Plugin-store browse-panel refinements and a `git+` install fix (spec `pluginstores` FR-004, FR-051; `plugins` FR-024). Each navigator on the `/plugins` browse panel now carries a one-row reversed-video header (`Categories` over the category navigator, `Marketplace` over the marketplace navigator) directly above its `ALL` row, and the navigator's cached hit-test area is now the row area below the header, so a click or wheel on a header row is inert and never mis-mapped onto `ALL`. A bottom-edge search box with a live filter field and a click-to-clear `[x] clear` button was already introduced with the three-pane navigator work. Separately, `git_clone_stage` in `ragent-plugins` ran `git checkout -- FETCH_HEAD`; the `--` separator made `git checkout` treat `FETCH_HEAD` as a *pathspec* file filter rather than a commit-ish, so every install from a `git+<url>#<ref>` source naming a non-default ref (for example the `/plugins claude` store's `git+https://github.com/mongodb/agent-skills#main:plugins/mongodb`) aborted after the fetch. The checkout is now `git checkout FETCH_HEAD`; `FETCH_HEAD` is a name ragent supplies itself and the ref that filled it is already validated to not begin with `-`, so no option can be injected. Covered by the new `add_checks_out_a_non_default_ref` case in `test_store_git_source.rs`. |
| v1.0.132 | 2026-10-12 | Merged connector/plugin marketplace browsers and alphabetical catalogue ordering (spec `pluginstores` FR-047..FR-050, `connectors` FR-042..FR-046). The `/plugins codex\|claude` and `/connectors claude` browse panels each merge their store's base endpoint with every additional marketplace registered through the new `/plugins marketplace` / `/connectors marketplace` sub-subcommand families (`show`, `<store> list\|add <source> [--force]\|remove <name>`), recorded in the user-global `plugins.stores.marketplaces` / `connectors.stores.marketplaces` arrays by an atomic config splice. Each result row shows the plugin/connector id, its version (plugin store), the marketplace it came from, and its description; the connector panel pins a dark-on-light column-heading row above its scrolling list, and both panels resolve each entry's blank description (and, for the plugin store, its version) from the entry's own repository manifest off the event loop via a bounded thread pool with an injectable `ManifestSource` seam (`ragent_plugins::store_enrich`, `ragent_connectors::catalogue_enrich`). Every connector and plugin-store list is sorted in ascending alphabetical order of id, case-insensitively with a byte-wise tiebreak (`compare_connector_ids` / `compare_store_ids`), applied before any query or category filter. The plugin-store category navigator no longer derives rows from entry `tags`/`keywords` (`catnav` FR-021 withdrawn). Registry fixes: the `/events` SSE connection cap builds on stable Rust (`compare_exchange_weak` loop replacing `AtomicUsize::try_update`); connectors install enabled; `catnav` FR-021 withdrawn retaining its number. |
| v1.0.131 | 2026-10-10 | OpenHands-parity release. The pluggable execution backends (`local`/`docker`/`podman`/`remote`, §19D) with a health-visible registry and TUI `/backend` switcher, sandbox secret injection resolved from the encrypted store at spawn time, the ACP client plus the feature-gated ACP server (§19E), the OpenAI-compatible inbound surface (`/v1/chat/completions`, `/v1/models`) with a generated OpenAPI 3.1 contract at `/openapi.json` and HTTP MCP transport, the automation service (webhook ingress `POST /auto/{id}`, scheduler, durable run history, backend confinement, §19F), AgentSkills (`.agents/skills/`) discovery, and the opt-in LLM security analyzer (`/security`) now ship as a tagged release. The 7-layer bash validation, path containment, and permission system are unchanged and gate every new surface. Registry fixes: the fast-path schema tests assert the crate's exported `SCHEMA_VERSION` (now 2, the `automation_runs` table); `PluginStoreBrowser::set_index` keeps document order; the `mask_key` doctest expects `[REDACTED]`; `machine_output` routes tracing to stderr for the report subcommands so machine-readable stdout stays clean; and the unused `ragent-storage` dependency is dropped from `ragent-tools-extended`. |
| v1.0.130 | 2026-10-09 | OpenHands parity delta (spec `openhands`, FR-001..FR-038 / T-001..T-019). Pluggable execution backends (`local`/`docker`/`podman`/`remote`) with a health-visible registry and TUI `/backend` switcher (§19D); sandbox secret injection resolved from the encrypted store at spawn time; an ACP client and a feature-gated ACP server (§19E); an OpenAI-compatible inbound surface (`/v1/chat/completions`, `/v1/models`) plus a generated OpenAPI 3.1 contract at `/openapi.json` and HTTP MCP transport; the automation service with webhook ingress (`POST /auto/{id}`), scheduler, durable run history, and backend confinement (§19F); AgentSkills (`.agents/skills/`) discovery; and an opt-in LLM security analyzer (`/security`, tightening-only over the static rules). Plugin-store category navigator (`catnav`) for `/plugins codex|claude` and removal of the `browser` CDP tool (registered count 152 -> 151 across 22 categories). The engine-gated `mf_screenshot` capability (FR-029) is registered only when a drivable headless browser is present, so the default build advertises 148 tools across 21 categories. Also fixes the stable-toolchain `/events` SSE connection cap (`AtomicUsize::try_update` -> `compare_exchange_weak` loop). |
| v1.0.129 | 2026-10-07 | Code-audit remediation complete through M9 (`docs/plans/code-audit.md`), landed as uncommitted working-tree work. **M5 dead-code removal:** the unreferenced `ragent-agent::snapshot` module (a stale near-copy of `ragent-storage::snapshot`) and the duplicated inline `ragent-tools-core` schema test suite are deleted; the placeholder `test_precompiled_regexes` bodies gain real assertions; the dead connector `NeverProbe`/`NoProbe` fallbacks and the `probe`/`workdir` parameters they alone used are removed, and `run_connector_subcommand_env` now takes `(&mut env, sub, rest)`; and the new 18th workspace crate `ragent-surface` owns the shared `/plugins`/`/connectors` surface glue (`ragent_surface::help` attribution + subcommand tokeniser, `ragent_surface::store` two-root store-dir resolution + symlink-containment guard, `ragent_surface::harness` step model with `sample_for_schema`/`schema_type`/`truncate`/`step`). **M6 test hygiene:** test scratch paths move off `/tmp` to `target/temp` (31 sites / 24 files); `ragent-plugins` and `ragent-connectors` each gain one `tests/support` `TempTree` (replacing 18 and 10 copy-pasted definitions); `test_mf_orchestrator_diag`/`test_fts_diag` and the live-network `test_ollama_cloud_real` are `#[ignore]`-gated; the three per-key config test files collapse to the table-driven `test_api_key_config_fields.rs`; the three scoreboard tests share `tests/support/scoreboard_fixture.rs`; weak `.is_some()` assertions bind concrete values; and four test files are renamed to `test_<component>_<scenario>`. **M7 new coverage:** new `tests/` suites for the calculator (24 tests), `agent_complete`/`bash_reset`/`xlsx`, the Azure AI Foundry provider (5 tests), the agent-loop step harness (3 tests), and the orchestrator `Coordinator` (13 tests) - the last fixing a real production bug where `active_jobs` was incremented after spawning and never on the synchronous paths, underflowing `AtomicU64` to `u64::MAX` (new `ActiveJobsGuard::enter` increments exactly once before the guard is installed). **M8 standards:** GitHub/GitLab acronym casing standardised across `ragent-tools-vcs` (31 tool types + shared helpers), the three user-facing/external-input unwraps removed, emoji/box-drawing glyphs stripped from production-source comments, six named modules gain `//!` headers, `MultiPlEAdapter` -> `MultipleAdapter`, the four `Regex::new(...)` sites in `slash.rs` use `.expect("valid ... regex")`, the `CollectingTierRouterObserver` test double is a real `pub` type, and `test_codeindex_backward_compat` asserts against the `SLASH_COMMANDS` data model instead of scraping source. **M9 dependency upkeep:** the yanked `yoke-derive@0.8.3` is cleared; the workspace converges on `thiserror 2`, `reqwest 0.13` (the `ragent-llm` pin now uses `workspace = true`), `rand 0.10`, and `criterion 0.8`; `ragent-storage` moves from `rand 0.8` to `0.10`; `lopdf` converges on `0.44` and the dedicated `vendor/lopdf` crate is deleted (the `MAX_OBJECT_DEPTH` recursion guard is no longer needed); `opentelemetry`/`opentelemetry_sdk`/`opentelemetry-otlp` migrate 0.29 -> 0.33 (the Prometheus scrape endpoint enables `experimental_metrics_custom_reader`); and `notify` 7 -> 8.2, `rquickjs` 0.10 -> 0.14, `tree-sitter` 0.26 -> 0.27, `chacha20poly1305` 0.10 -> 0.11, and `which` 7 -> 8 land. **M1 secret hardening:** `.gitignore` gains `*.db-wal`/`*.db-shm` (SQLite sidecars can hold plaintext credential-store and session pages) and the certificate/credential patterns `*.pem`, `*.p12`, `*.pfx`, `service-account.json`, `credentials.json`; a tracked `.env.example` template lists every credential env var ragent reads. Also fixes `Config::write_config_if_changed` folding a corrupt existing file to `Value::Null` (now logs and forces a rewrite). |
| v1.0.128 | 2026-10-06 | Config durability, bounded MCP connect, and non-blocking TUI startup. (1) Atomic config writes: every `ragent.json` write (`Config::save`, `Config::save_to_source`, the runtime-flag toggles, `/config save`) now lands in a uniquely named temp file in the config directory, is `fsync`-ed, and is renamed over the target, so a crash/kill/power-loss between truncate and write can no longer leave a partial config that `Config::load` silently falls back on (the intermittent "YOLO keeps going off after a few restarts" report). (2) Runtime-flag persistence edits only the single top-level key in the raw global file (`Config::set_global_bool_key`), so the project overlay and unmodelled keys are never folded into the user's global config; every such write logs the key, value, path, and a forced backtrace, and startup logs the effective `yolo`/`edit_log`/`activity_log`/`gcf` state with the contributing config paths. (3) Bounded MCP connect: every `McpClient::connect` runs under a per-attempt timeout (default 30 s, `RAGENT_MCP_CONNECT_TIMEOUT_SECS`) and is retried once on timeout only, tearing down a partially-started stdio child before the retry. (4) MCP connect failures are recorded as `McpStatus::Failed { error }` on every failure path and reported in the startup report and `/mcp`. (5) TUI startup no longer blocks on the MCP connect loop: `run_tui` adopts whatever state the loop has published and prints its one-shot per-server report off the loop's completion sentinel; every event-loop read of the shared client is non-blocking (`try_read`). (6) `ragent_info` now reports runtime execution details (pid, parent pid, start time, uptime, executable, working directory, user, resident/virtual memory, thread count) alongside build metadata. (7) `/config list` restore drops the cached config, re-reads it, and resyncs runtime flags. (8) `/spec reverse --folder` runs the local scaffold before the GitHub token gate so a token-less runner still populates the target folder. |
| v1.0.126 | 2026-10-04 | Office / LibreOffice document tools removed. The six `office_*` / `libre_*` tools and their module set (`office_common`, `office_write`, `office_info`, `libreoffice_common`, `libreoffice_write`, `libreoffice_info`) are deleted; only the two PDF tools (`pdf_read`, `pdf_write`) remain, sharing a new `pdf_common` helper module. The `tool_visibility.office` switch and its `ToolVisibilityConfig` / `ToolVisibilitySpecified` fields, the unused `docio` helper, and the OOXML/ODF `DocumentFormat` variants are removed, and the dependencies `docx-rust`, `calamine`, `ooxmlsdk`, `zip`, and `spreadsheet-ods` are dropped. Registered tool count falls 158 -> 152 (23 categories); `assets/officedocs/testword1.docx` is deleted; `/tools` switch lists and `tool_visibility` tables drop `office`; configs carrying `office` are ignored. A `/simplify all` pass over the changed set fixes a `pdf_write` image-path containment hole (SEC-tools-extended-002), halves `pdf_read` PDF parsing, computes `os_info` JSON once, scopes the TUI `/memory clear` delete to `self.cwd_path`, and makes `connectors::store::descriptor_by_id` a single allocating pass. |
| v1.0.125 | 2026-10-04 | `os_info` host-introspection tool and `/osinfo` slash command family, plus the research scholarly-engine default flip. `os_info` (registry 171 -> 172, spec `osinfo` FR-019..FR-025) reports OS identity and Linux distribution, CPU, graphics adapters (`/sys/class/drm` + `pci.ids`, integrated/discrete/virtual classification), graphics-API support with best-effort versions (`Direct3D`, `DirectX`, `Metal`, `OpenGL`, `OpenGL ES`, `Mesa`, `Vulkan`, `OptiX`, `CUDA`, `ROCm`), physical hardware (DMI system/chassis/motherboard/BIOS identity, `/sys/block` storage devices, `/sys/class/net` interfaces), memory, uptime, and the process environment. The `probe` parameter defaults to `true`, so the fixed, timeout-bounded allowlisted graphics diagnostics (`vulkaninfo`, `glxinfo -B`, `nvidia-smi`, `rocminfo`, `system_profiler`) confirm support and supply versions for OpenGL, OpenGL ES, and Mesa; `{"probe": false}` (or `/osinfo show --no-probe`) is the fully process-free path. Read-only throughout: no writes, no network, and never serial numbers, UUIDs, or asset tags. `/osinfo show [--no-probe]` renders through the tool's own collector/renderer, `/osinfo help` documents both modes, and autocomplete offers `show`, `--no-probe`, `help`. Research: scholarly backends (OpenAlex) are excluded from the web sweep by default; `--papers` replaces `--no-papers`/`--no-scholarly` at every entry point (root CLI, TUI, `POST /research`), `ResearchRunRequest` carries `papers`, and `research.exclude_academic_engines` only persists the exclusion. Three new tracked research items (`codemigrate`, `connectors`, `vendormarketplace`). |
| v1.0.124 | 2026-10-02 | Connector system follow-up, `/memory clear`, and TUI render fixes. (1) `/memory clear` opens a `Clear this project's memory?` `Yes`/`No` confirmation dialog (no removal until confirmed; `No` selected by default) and, on `Yes`, calls `Storage::clear_memories_for_project` to delete only the current project's structured memories (full path or basename, FTS + base rows in one transaction), reporting `Cleared N memory entries for this project.`; the `command_catalog`/`SLASH_COMMANDS` `/memory` entries now list exactly `show`, `clear`, `help`. (2) The connector catalogue has a single surface: the standalone `/connectors search` subcommand is retired, `/connectors claude [query] [--category <name>] [--refresh]` carries the `--category` launch filter and an in-panel `c` cycle key over the categories the fetched catalogue declares (FR-010, FR-041), and `list` keeps the same category model. (3) The TUI drives the session-start connector lifecycle: `src/main.rs` bridges the enabled connectors into the shared connect loop and publishes a `ConnectorSession` plus its status snapshot on the session processor (type-erased); `/connectors enable\|disable\|connect\|disconnect` drive that session and reconcile the MCP tool registry, `/connectors list` reads live tool counts from the MCP client, and a single-reference subcommand accepts a connector by id, slug, or display name (`canonical_id`). (4) TUI-019 render fixes: the tool name renders immediately after the step counter (new `split_summary_icon` helper, both the message-widget and layout arms) and `/mcp` renders one server per row as a markdown/ASCII table with `\|`-escaped cells; `/tools` gains `/tools list` (alias `/tools show`) and a `/tools help` block. (5) The MCP orphan sweep reaps stdio servers re-parented to the per-user service manager (`systemd --user`) as well as init, and matches per process, so `/mcp` no longer shows `npx -y mongodb-mcp-server@<3>` stuck on every startup. Earlier work folded in: a new `ragent-connectors` crate (17th workspace crate), the ANTIPAT M1 ASCII sweep, `/yolo` persisting to the user-global config only, spec plan parser `T-001..T-014` ranges, and the `check-connector-endpoint-literal.sh` guard. |
| v1.0.122 | 2026-09-30 | Security and anti-pattern remediation sweep, folding in the staged working tree on top of `6cf0b60f` (MS-04). `ANTIPAT.md` M0 (the two shipping defects - crash-marker ordering and search-retry shift overflow - plus the highest-severity containment holes) and M2-M7 complete: `ragent-team` shim crate deleted (17 -> 16 workspace crates), major dependency bumps (`rmcp` 3.5, `rusqlite` 0.40, `ratatui` 0.30, ...), and `SECTASKS.md` MS-05 "prevent recurrence" - new `ragent_types::guard` (re-exported as `ragent_tools_core::guard`) owns `reject_option_like`, `is_safe_operand`, `validate_identifier`, `validate_relative_component`, `contained_join`, `clamp_retry_after` and `cap_read`; `ragent-agent`/`ragent-storage`/`ragent-tools-core` re-export `ragent_types::sanitize` so there is one secret registry and one redaction chokepoint (`Event` no longer derives `Debug`); the `security-guards` CI job runs `check-file-tool-containment.sh`, `check-security-unwraps.sh`, `check-shared-guards.sh` and `check-vcs-duplication.sh` (each with a `--self-test`); `SECTASKS.md` records the accepted-risk register (T-071). `SPEC.md` §4.6a-§4.6e document the guards and call sites. Earlier commits on the tree: MS-03 network/secret hardening (`da83d927`) and MS-04 defence in depth (`6cf0b60f`). Verification: `cargo check --workspace`, `cargo clippy --all-targets`, `cargo fmt --all -- --check`, `cargo audit` clean; workspace test suite green. |
| v1.0.121 | 2026-09-27 | Introspection and MCP hygiene release. New read-only, hardwired auto-approve tools `tool_info` (JSON dump of the tool registry: name, description, parameters schema, permission category, source family, hidden state, MCP server/tool provenance) and `commands_info` (JSON catalog of every slash command — built-in TUI set from a drift-tested static mirror plus plugin-contributed commands resolved live) take the registry from 169 to 171 tools; the TUI `/tools` report and `tool_info` now share one source classifier built on `Tool::mcp_wrapper_info`. MCP lifecycle: `McpClient::connect` sweeps orphaned stdio server processes before spawning (a process counts as an orphan only when re-parented to init — `/proc/<pid>/stat` field 4 — so a live sibling ragent's server is never killed, fixing cross-instance `Transport closed`), and `shutdown` kills the whole spawned process group (`process_group(0)` + `killpg`) so no `npx`/`node` children survive exit. Fixes: post-loop rollback removes the capture from `active_loop_captures` only after the restore fully completes (a failed restore stays pending for retry — CI flake `test_rollback_accept_restores_snapshot`); OpenSkills discovery covers `~/.agents/skills/` and `.agents/skills/`; the Claude store's self-describing `*-lsp` stubs (e.g. `rust-analyzer-lsp`) install by materialising the marketplace document's inline `lspServers` manifest into `.claude-plugin/plugin.json` (recorded under `sha256(origin-url + bytes)`, never overwriting an existing manifest); plugins shipping a conventional `skills/` directory without a `skills` manifest section now bridge those skills. A `/simplify all` pass over the 50-file changed set: scoped-block lock release in `ToolRegistry::remove_all`, single SHA-256 + no JSON round-trip in the store provider, shared `merge_scanned_skills` in the plugins manifests, awaited (no nested `block_in_place`/`block_on`) `/mcp connect|disconnect|discover` arms, and validation-before-ledger in `set_mcp_server_enabled`. Verification: `cargo check --workspace --all-targets`, full `ragent-agent`/`ragent-plugins`/`ragent-tui` test suites, `cargo fmt --all -- --check`, `cargo audit` all green. |
| v1.0.119 | 2026-09-26 | User headline "mcp fixes": a sessionful Streamable-HTTP MCP server (the MongoDB MCP server) now reports its tools — `HttpMcpClient::initialize` negotiates the session, replays the returned `mcp-session-id`, advertises `text/event-stream`, and unwraps SSE frames, and `McpClient::adopt_connected` adopts an already-running server through that client; the HTTP client is built lazily so it no longer panics outside a Tokio runtime. The TUI startup MCP report awaits the shared client lock with a `MCP_STARTUP_GRACE = 3s` connect wait, `/plugins list` resolves live MCP tool counts via `run_control_command`, and `/plugins list --mcp` sorts server ids for a deterministic contributions block. A `/simplify all` pass over v1.0.116–v1.0.118 fixed the `/swarm status` progress-bar overflow, the `/spawn` pending-marker race, `/plugins <non-list> --mcp` handling, swarm unblock persistence, and `/alog` error propagation. Rust-hygiene sweep green: `cargo check --workspace --all-targets`, `cargo check --tests --workspace`, `cargo test --workspace`, dead-code lint and reason checks, `cargo clippy --workspace --all-targets`, `cargo fmt --all -- --check`, `cargo audit`, `cargo deny check`. |
| v1.0.118 | 2026-09-25 | Durable, global MCP server enable/disable state: whether an MCP server is started is now a persisted choice (`<global state dir>/mcp_state.json`, a shared `McpEnableLedger`) rather than an implicit effect of being listed in `ragent.json`. A server id absent from the ledger is enabled; `mcp.<id>.disabled: true` always wins; a disabled server is registered with `McpStatus::Disabled` and no child process is spawned. `/mcp connect <id>` enables and connects live (registering tools immediately) and `/mcp disconnect <id>` disables and disconnects live, both persisting globally; `McpToolWrapper::execute` refuses a disabled server's tools. `/mcp` lists plugin-contributed servers (built from the merged `plugin_mcp_servers` set) with `enabled yes/no` and `tools: N`; `/plugins list` gains `MCP` / `MCP Tools` columns and an `mcp [...]` contributions line (`?` when a count is unknown, never `0`), and `/plugins list --mcp` keeps the full per-server tool inventory with registry names. Plugin `mcpServers` entries (inline or the Claude `"mcpServers": "./mcp.json"` file reference) are bridged as `<plugin-id>.<server>` and connected by default. `/tools` now lists visibility-disabled tools (`Visible Tools (N total, M disabled)` + a `Disabled by visibility` section) via the new `ToolRegistry::hidden_definitions()` / `hidden()`, and the slash-output extractor keeps every fenced block. A `/simplify` code-quality pass follows (dead `plugin_contributed_mcp_servers` and `McpEnableLedger::enabled_servers` removed, `McpEnableLedger::to_map` added, single config load in `set_mcp_server_enabled`, `impl Display for McpStatus`). |
| v1.0.117 | 2026-09-25 | `/spec reverse --folder` now scaffolds and hosts the target project: the parsed scaffold request and `--folder` are handed straight to the reverse handler (not re-rendered through the `/reverse` text grammar that does not carry them), the shared `archdoc::run_govcreate_scaffold` engine runs before the async fetch, and a chained `--create <name>` writes the spec into `<folder>/specs/<name>/` via the new `/spec create --folder` flag. A `/spec reverse` invocation whose first argument is a flag, or whose flag tail is invalid (missing `--type`, unregistered `--language`, duplicate/empty value, `--github` with `--gitlab`), now reports the specific cause instead of the bare usage line; `next_value` rejects empty or `--`-prefixed values. The scaffolder gains `webapp` as a first-class registered `--type` value for `/new`, `/spec reverse`, and `/spec govcreate` (tiny dependency-free HTTP-server starter for `rust`/`python`/`go`/`typescript`/`javascript`, manifest-only elsewhere). The vendored `lopdf` crate carries crate-level allowances (matching `vendor/pdf-extract`) so the dead-code lint and `cargo-machete` are green. All verification checks pass: `cargo check --workspace`, `cargo-machete`, `cargo check --tests --workspace`, `cargo test --workspace`, the dead-code lint and reason checks, `cargo clippy --workspace -- -D warnings`, `cargo fmt --all -- --check`, `cargo audit`, and `cargo deny check`. |
| v1.0.116 | 2026-09-23 | Maintenance release over the v1.0.115 tree: agent and plugin updates plus a code-quality pass. The four content-sized TUI modal renderers (queue menu, queue show panel, plugin-store panel, queue clear confirmation) now share one `centered_rect_fixed(width, height, area)` helper in `ragent-tui/src/utils.rs` (byte-for-byte identical geometry); `persist_task_output` in `ragent-agent/src/task/mod.rs` streams the header plus the reply body straight into a `BufWriter<File>` then renames, halving peak memory while the full untruncated sub-agent report is written, with identical on-disk bytes. `cargo check --workspace`, the dead-code lint and reason checks, `cargo clippy --workspace -- -D warnings`, `cargo fmt --all -- --check`, `cargo audit`, `cargo deny check`, and the full `cargo test --workspace` suite all pass. |
| v1.0.115 | 2026-09-23 | Detached fire-and-forget sub-agents (spec `spawnagent`, FR-001..FR-005): the TUI `/spawn <agent> <prompt...>` slash command and the matching `new_agent` `detached: true` parameter launch a background sub-agent that **nothing ever waits on** — it runs concurrently and shows in the Agents panel / event log, but is excluded from `list_agents` and `running_background_count`, cannot be awaited via `wait_agents` (with or without `task_ids`), is untouched by `team_wait`, and its completion is reaped by `drain_completed` without injecting anything into the parent chat while `tasks_snapshot` still shows it; `TaskEntry.detached` (serde default `false`) and `AgentManager::spawn_detached` share one `spawn_background_mode` implementation with `spawn_background`, and a second `/spawn` while one is still registering is refused. Follow-up fixes (FR-004a/b/c): every completed sub-agent run (detached or not) now persists its FULL output to `log/subagents/<task-id>.md` under the parent session's working directory (atomic temp-file + rename) and records the path on `TaskEntry.output_file`; the session loop records its terminal `MessageEnd` reason into `SessionProcessor::last_message_end_reason` and the completion event maps it to the real loop `finish_reason` (`stop`/`truncation`/`length`/`cancelled`/`error`) so a provider-side cut is flagged as TRUNCATED in the Agents panel instead of looking like a healthy finish; and the Subagent system prompt gained a *Deliverable Enforcement* section requiring a file-writing sub-agent to call `Write` before `agent_complete`. Also: `/new` stack overlay now strips the base `fn main` so a generated binary declares exactly one entry point (FR-007); the research gather log moved to the singular `log/research/`; and a `/simplify` + `/rust-hygiene` pair consolidates the plugin bridge filters, the scaffold remote helpers, and the TUI queue-clear dialog helpers, keeping `cargo fmt`, `clippy -D warnings`, the dead-code lint and reason checks, `cargo audit`, and `cargo deny` green. |
| v1.0.114 | 2026-09-22 | Missing plugin components: the plugin system now bridges every non-tool surface for both Codex- and Claude-dialect plugins — skills, MCP servers, slash commands, agents, and hooks (T-021..T-023, T-026), including the full Claude `hooks.json` declaration surface and a blocking Stop continuation bounded to three continuations; the descriptor recognises the nested `.codex-plugin/plugin.json` (T-024) and resolves a directory shipping both nested manifests to the Claude dialect (T-025); `/plugins list` counts and details skills/agents/hooks (FR-009) and `/plugins add` installs a plugin **enabled** (FR-007). Also: `/new --github` resolves the GitHub token through the shared `ragent_config::github` chain (`GITHUB_TOKEN` -> `~/.config/ragent/github_token` -> `gh` CLI) with an app-token downgrade (FR-008); a slash command submitted while the agent runs is queued and drains at the next turn boundary (inputqueue FR-017 amendment); the new read-only `ragent_info` tool reports the running version, build time, git commit, and compiler (168 -> 169 tools); and a `/simplify all` pass applies seven verified code-quality fixes across `ragent-agent`, `ragent-plugins`, `ragent-tui`, and `src/` with no behaviour change (three further proposed refactors rejected as adding complexity). |
| v1.0.113 | 2026-09-21 | Message input queue (spec `inputqueue`, `specs/inputqueue/`, T-001..T-027 complete): the TUI message-window input field stays editable while the primary agent executes — each `Enter` appends the submission to a bounded FIFO queue (default 32, `input_queue_capacity` config clamped `1..=99`) instead of rejecting it, a two-digit zero-padded counter renders before the `> ` prompt, and the oldest entry runs at each turn boundary (drain on `MessageEnd`/`AgentError`, deferred by the shared guard while processing/compacting). Control surfaces: the four-row `Alt+Q` queue-control menu (`Next` / `Stop`/`Resume` / `Clear` / `Show`, navigated with `Up`/`Down`/`Enter`; `Yes`/`No` clear-confirmation dialog defaulting to `No`; the `Show` row opens a scrollable queue-entry panel where `Enter` moves an entry toward the front and `Del` removes it — FR-038..FR-043, NFR-012) and the `/queue [list\|clear\|next\|help]` slash command. FR-017 was amended so a slash command (`/…`) is queued the same way and runs at the next turn boundary (a synchronous command leaves the boundary free, so consecutive queued commands run back-to-back; at capacity it is rejected and restored to the field), while only bang commands (`!…`) and teammate-targeted sends keep their busy refusal; the queue is in-memory only and never persisted (NFR-005). Layout change: side panels move from a 50/50 to a 60/40 messages/panel split. 19 new test files (234 new test attributes); `test_busy_send_guard.rs` rewritten. A `/simplify all` quality pass over the diff and the HEAD~3 plugins/research/config work follows (de-duplication, dead-code removal, per-frame allocation removal; no behaviour change). Docs: `TUI-QUICKSTART.md` §4, `docs/howtos/tutorial.md`, new `docs/howtos/slashcommands/queue.md`, and `docs/howtos/config.md` §7.38 `input_queue_capacity`. Followed by a full-workspace CI hygiene pass: the non-existent `clippy::assert_is_empty` allow removed from 244 files and `cargo clippy --all-targets -D warnings` made clean. (Uncommitted follow-up on the same tree: **Claude plugin hooks** — the `hooks.json` file and the group `{matcher, hooks: [...]}` shape are now read (T-026, FR-033), a plugin hook receives `CLAUDE_PLUGIN_ROOT` and the Claude event JSON on stdin, a `PostToolUse` `additionalContext` is folded into the tool result, and a blocking Stop hook feeds findings back to the model before the turn ends (bounded to three continuations); `/plugins list` counts and details skills/agents/hooks (FR-009); `/plugins add` installs a plugin **enabled** (FR-007); a multi-target directory shipping both nested manifests resolves to the Claude dialect instead of being refused as ambiguous (T-025); the GitHub credential chain gains a `gh` CLI fallback with an app-token downgrade so `/new --github` can create a hosting repository when the stored token is a GitHub App token (FR-008); the new read-only `ragent_info` tool reports the running version, build time, git commit, and compiler (168 -> 169 tools); and a `/simplify all` pass applies seven code-quality fixes across `ragent-agent`, `ragent-plugins`, `ragent-tui`, and `src/` with no behaviour change (three further proposed refactors were rejected as adding complexity).) |
| v1.0.109 | 2026-09-18 | Final phase of the `/simplify all` pass over v1.0.106..v1.0.108: dead `utf8_prefix_len` helper removed from the LLM HTTP client (superseded by the FUNC-033 `Utf8Error::error_len()`-driven flush), `config_agents_dir` collapsed into `global_agents_dir`, `extract_http_status` parses the three status digits byte-wise without an intermediate `String`, and all eight `ragent-research` `SourceVault` poisoned-lock sites now share one `lock_conn()` helper carrying a dedicated `SourceVaultError::LockPoisoned` variant (no behaviour change). (Uncommitted follow-up on the same tree: PERF-042 throttle-tail paint fix in `should_render` + `test_pending_message_cache_group_forces_safety_paint`, dependency/advisory hygiene drops, and the new `plugins` spec draft in `specs/plugins/` — sandboxed JavaScript plugin runtime loading Codex/Claude manifests + six-subcommand `/plugins` family, spec only.) |
| v1.0.108 | 2026-09-18 | Config rules and fixes from a five-agent explore review of the v1.0.105..v1.0.107 window: GitLab legacy credential migration scans the real legacy `~/.ragent/` root again (`migrate_legacy_files` self-scan silent no-op fixed; `test_gitlab_legacy_migration.rs` pins it); team-blueprint "global" fallback restored to the true legacy `~/.ragent/blueprints(/teams)` scan (TUI `blueprints.rs` + `team_create.rs`); the four `/spec govcreate` mutex-lock sites uniformly recover poisoning via `into_inner`; `/config show` renders unavailable global memory/agent dirs as "(unavailable)"; `acquire_local` gains a `stopped_early` flag so exact-cap natural completion is not misreported as budget exhaustion; research helpers `num_prefix_re` and `join_text` deduplicated into single shared copies; `global_skills_dir().unwrap_or_default()` empty-path leg fixed in the session processor. |
| v1.0.107 | 2026-09-18 | `/spec govcreate` — spec authoring from an architecture document (spec `govdoc`, FR-001..FR-020): `/spec govcreate <spec-id> <content-ref> <target-folder>` and CLI parity `ragent spec govcreate` acquire a document from a local file/folder or public URL, extract content, author `SPEC.md`/`PLAN.md`/`TESTPLAN.md` via the configured LLM, and write the new spec with staged `[ .. ]/[ ok ]/[fail]` in-place TUI progress, per-run `CancellationToken` on Escape, and an FR-011 non-empty-target guard (`--force` overrides). The window also carries HEAD~3 review fixes: SSE chunk coalescing driven by `Utf8Error::error_len()` (no multi-byte splitting; six regression tests), secret redaction captures key+separator with a base64/base64url value charset, spec dependency-range expansion capped at 1000 IDs, `delete_memories_by_filter` reports real row counts, masterfetch cache deletes corrupt rows, and the TUI Markdown renderer gains `preprocess_markdown_tables` for ragged pipe tables. Documentation: the 612-line `docs/howtos/tools.md` replaced by 26 per-category tool how-tos in `docs/howtos/tools/` (+ generated PDFs). |
| v1.0.106 | 2026-09-17 | `--url-cloak` research source defanging (`/research create --url-cloak`, TUI, `POST /research` `url_cloak`): web source URLs are written as defanged plain text (`https://` -> `hxxps://`, `http://` -> `hxxp://`, dots bracketed `example[.]com`, wrapped in a Markdown code span) in the `**Sources:**` bullets and `References Index` table of `RESEARCH.md` (plus the `Sources Reference` table in `CORPA.md`), leaving non-URL rows untouched; off by default and recorded in frontmatter (`url_cloak: true`) for `/research update` replay. Includes a `/simplify` code-quality pass over the change set (shared `SourceVault::run_blocking` helper + `TaskPanic` error, `AssembleInput` struct replacing an 18-parameter `assemble_and_write`, hardened `extract_http_status`/`lowercase_cow`/`cloak_url`, `Arc`-keyed query map, and test-literal collapse onto `minimal_request`). |
| v1.0.105 | 2026-09-16 | Research output limits (`research.max_concepts`/`research.max_findings`, `--max-concepts`/`--max-findings`, `POST /research` fields; defaults 5/20, `0` = unbounded, most-relevant-first reordering before truncation), scholarly-engine exclusion (`mf_search` `exclude_engines`, research `--no-papers`/`--no-scholarly`, `research.exclude_academic_engines`), per-run open-access toggles (`--oa-enable`/`--no-oa`), a progress table that breaks exclusions and fetch failures down by reason/cause, `/spec impl` task-range Dependencies expansion, and TUI panel changes (TASKS panel `[STATUS] <id> title` rows, 50%-width side panels). |
| v1.0.104 | 2026-09-14 | Functional anti-pattern remediation (FUNC-038..069, 080..082): the FUNCPLAN.md second pass closes the plan's M1 tail and all of M2-M5 -- `mf_search` keyless engines surface a dead engine as an error instead of a zero-result success (FUNC-035), `github_merge_pr` rejects an unknown merge method (FUNC-038), the last three production poison-lock panics recover via `PoisonError::into_inner` (FUNC-040/041/042), no production `unreachable!`/poison-expect remains (FUNC-043/044/045), GitLab GETs retry 429 honouring `Retry-After` with bounded request/entry budgets and full pagination and research blocking reads move behind `block_in_place` (FUNC-050/052/053), and the TUI/tools/VCS/codeindex/server/research correctness fixes (FUNC-060..069) plus a new `scripts/check-poison-locks.sh` guard wired into `pre-flight.sh` and CI (FUNC-080/081/082). |
| v1.0.103 | 2026-09-14 | M1 agent per-turn hot path (PERF-032..040 + PERF-048): the provider-facing transcript is held behind an `Arc<Vec<ChatMessage>>` (no per-turn deep clone, PERF-032), a pure history append converts only the new tail (`take_cached_for_append` + `record_history_base`, PERF-033), the subagent tool surface is cached behind the tool-registry version (PERF-034), `LoopTracker` is `Copy` (PERF-035), the new `RequestTokenTracker` makes the per-step pre-send token estimate O(changed message) instead of O(history) (PERF-036), tool/result pairing is a single pass (PERF-037), the compaction prompt is assembled into one buffer (PERF-038), memory-entry token costs are memoised (PERF-039), the activity log is written by one background task per process (PERF-040), and the TUI viewers retain a single copy of their rendered rows (PERF-048). New benches `turn_loop`/`m3_hot_paths`; new guards `test_activity_writer`/`test_no_percall_regex`. Security: `rustls` 0.23.43 -> 0.23.45 (RUSTSEC-2026-0285). |
| v1.0.102 | 2026-09-14 | Second `/simplify` sweep across the search, research, agent, and TUI crates: the API-key `mf_search` engines (tavily/perplexity/exa/serper/langsearch) share `engine_http_client`/`api_engine_preflight`/`finish_json_search`/`mask_api_key` plus `truncate_snippet`/`truncate_query_to` (~150 dup lines removed), `strip_disallowed_quotes` is a single allocation, `search_with_retry` caps the backoff shift at 31, and `diversity_truncate` takes/returns owned vectors and re-syncs `total_merged_results`; the research web-gatherer volume policy is one `volume_policy()` helper and the vestigial `deadline_fired` flag is gone. |
| v1.0.101 | 2026-09-13 | Simplify pass over four crates: deterministic engine-merge output (sorted `source` strings) with positions renumbered after truncation, poison-tolerant mutexes in `masterfetch::search`, and further deduplication in `ragent-research`. |
| v1.0.100 | 2026-09-13 | Serper (Google) added as a fifth optional API-backed `mf_search` engine (`serper_api_key`); engine-level retry (`search_with_retry`, 2 retries, 1 s/2 s backoff) for transient Wikipedia 429 / HTTP 5xx / transport failures with account-level quota blocks reported as `blocked_engines`; the orchestrator staggers engine starts by 120 ms (fixes the "only LangSearch results" `/websearch search` symptom); the research web-gatherer fetches to completion instead of cancelling at the `--web-time` deadline and the consecutive-failure search circuit breaker was removed in favour of the retry policy; `/research` gained a TUI clarification gate. |
| v1.0.99 | 2026-09-13 | Width-sweep volume cap removed except in competitive mode (uncapped gather by default; the 60 s phase deadline and `--fetch-concurrently` bound the work), pre-fetch relevance filter gained morphological term matching, and merge diversity bounds any single engine to half the merged slate. |
| v1.0.98 | 2026-09-13 | GCF tool-result encoding (spec `gcf`, FR-001..FR-009): opt-in `gcf.enabled` config, lossless re-encoding of large JSON tool results inside `[BEGIN GCF generic]` blocks. |
| v1.0.97 | 2026-09-12 | `/toolchain list` hang fix (FR-012): the probe worker thread is detached after a receive timeout instead of being joined. |
| v1.0.96 | 2026-09-12 | `/prompt` system-prompt inspector (read-only TUI slash command rendering the assembled system prompt an agent would receive in primary or subagent mode with the effective tool surface; report cap, tool-free agents render `(no tools)`; five test files), tool-repeat guard FR-044 (after five consecutive identical tool calls the sixth raises a `tool:repeat` permission prompt in interactive runs and auto-denies with a corrective observation in unattended runs), edit-tool line-structure guard FR-045 (the whitespace-flexible fallback lane can no longer join or split lines; regression tests), edit-guidance prompt rewrite with failure-recovery instructions matching the actual error machinery, redundant slash commands removed (`/opt` with its `ragent-prompt_opt` crate and `POST /opt` endpoint, `/tasks` alias of `/task list`, `/theme` registered but never dispatched), and the slashcommands how-to doc set (75 command docs + INDEX.md + PDFs). |
| v1.0.95 | 2026-09-11 | AgentNotice chat-bubble separation (TUI event handler forces a new assistant message before and after appending a notice so consecutive notices render as their own yellow bubbles; regression tests in `test_agent_notice_separation.rs`) and `/toolchain list` fixed-width table (columns fixed at 10/10/10/50 characters — FR-017, constant 93-column grid; Language/Runtime cells clip, Status and Version cells word-wrap onto continuation grid lines; `COLUMN_WIDEN_CHARS` replaced by `TABLE_COLUMN_WIDTHS`; renderer tests, TUI regression tests, and the toolchain howto updated). |
| v1.0.94 | 2026-09-11 | Documentation and statistics refresh: regenerated STATS.md from the live tree, updated CHANGELOG/README/QUICKSTART/TUI-QUICKSTART and all 20 howto PDFs for the v1.0.94 release. |
| v1.0.88 | 2026-09-08 | `/simplify` pass over the `ragent-research` crate (five parallel audit agents across all 56 source files; +419/-400 in 7 files, no behaviour change intended). Correctness: `parse_subject_summary` no longer panics when an LLM response places `}` before `{` (inverted `trimmed[start..=end]` slice replaced with `trimmed.get(start..=end)?`, mechanical fallback); `AnalysisEngine::with_brief` promoted from a panicking `unimplemented!()` default to a required trait method; `SearchCallOutcome::Ok` dropped its unread `retries` field and stale `#[allow(dead_code)]`; no-op `mark_in_progress_for_state` deleted from `continue_item` (item status lives in RESEARCH.md frontmatter); `REQUIRED_SECTIONS` doc comment corrected 10 -> 9 sections; `cited_date_span` computes min/max cited years with running vars; web pages classify `media_type` once and reuse it at all three render sites. Performance/hygiene: `OnceLock`-cached regexes in `analysis/parser.rs` (citation, bare citation, ISO date, finding-dependency) and `session/fallback.rs` (implication); `merge_chunk_results` dedup sets now borrow `&str` instead of cloning into `HashSet<String>`; `document.rs` gains a shared `layout` module of eight section-push emitters used by both `assemble_report_body` and `assemble_imrad_body` (~150 dup lines removed; report-layout Search Engine Summary + Search Provider Requests remain level-2 `###` sub-headings by design); `web_gatherer.rs` gains `is_sole_engine_hit`/`collect_engines` helpers (engine ordering now sorted/deterministic on both vault and observer gather paths) and deletes the `fence_captured_body` pass-through; `session.rs` gains `MediaCounts::of` (single-pass pdf/youtube tallies replacing 4x `matches!` scans at three sites) and `planner_or_default`/`critic_or_default` helpers (5 sites). Verified: cargo check/clippy (`-D warnings`)/fmt/dead-code lint clean; `cargo test -p ragent-research` 41 test binaries green (662 lib tests). |
| v1.0.87 | 2026-09-08 | UI polish and tool-calling fixes: status bar top line gains a centred bracketed last-prompt tag (first 32 characters with a `....` truncation suffix, rendered between the git branch and the session status; the working directory is shortened to fit and the layout falls back to the previous cwd/branch/status arrangement when the tag would clip); `App.last_prompt` tracks the most recent submitted prompt across chat, slash, bang, and loop entries; the message window no longer emits a leading blank line before the first `You:` prompt (blank-line separation now only applies when the transcript already has rendered lines); `websearch` diagnostic arms (`test`/`show`) share one `websearch_diag_ctx()` helper; a `is_help_args()` helper replaces repeated `args.trim() == "help"` checks in slash arms; `/spec` arms reuse `spec_manager()`; and the `text_toolcalls` recovery helpers are tightened to `pub(crate)` visibility. New tests: `test_statusbar_last_prompt.rs` (tag truncation, centring, and fallback). |
| uncommitted | 2026-09-07 | Goal-driven loop programming (`/loop`, spec agentloop): `LoopSpec`/`LoopTracker`/`StopCondition` state machine with stop conditions `completed`/`budget_exhausted`/`error`/`interrupted`; verification gate for verify commands (600 s timeout, head 8000 + tail 2000 capture); restriction layers via `deny_reason` (tool set -> read-only -> scope, invalid globs fail closed); pre-loop snapshot/git capture + rollback (`loop_capture.rs`, FR-018/FR-020); `LoopConfig` in `ragent.json` (`loop.max_steps` 512, `cost_limit`, `error_retry_allowance` 3, `checkpoints` true, `checkpoint_timeout_secs` 120); SSE `LoopTerminated`/`LoopChangeSummary`; TUI setup dialog + rollback flow (`loop_dialog.rs`, `test_loop_dialog.rs`, `test_rollback_flow.rs`). One-shot `/loop` flags: `--max-steps N`, `--cost_limit N`/`--cost-limit N`, `--timeout N` (any position, `--flag value` or `--flag=value`, per-run overrides resolved in `start_loop`). Fixes: `stock_recommendations` added to the finance tool-visibility family (8 tools, was hidden-family escapee); `/spec reverse` accepts quoted `--tech` values via a shell-like tokenizer (`tokenize_reverse_args`, single/double quotes grouped and stripped). How-to documentation set: `docs/howtos/reactagent.md` (core per-turn ReACT loop, NOT `/loop`), `docs/howtos/loopprogramming.md` (`/loop` feature), `docs/howtos/office.md` (office + PDF tool families: `office_read/write/info`, `libre_read/write/info`, `pdf_read/write`, format matrix, JSON examples, `tool_visibility.office` configuration); all 18 how-tos re-verified against source (accuracy sweep) and the A4 xelatex PDF set regenerated. |
| v1.0.84 | 2026-09-08 | /simplify all pass (shipped in v1.0.84): research `comparison.rs` lowercase-slicing panic fixed (byte offsets from `to_lowercase()` sliced the original string; U+0130 lengthens under lowercase causing char-boundary violations) with per-profile `PreparedSummary` + 3 new tests; TUI `tui_event_lag` double-Arc dead-code fix (single shared Arc now drives the lag reconcile); codeindex background result slot typed as `Option<Result<(String,String),String>>` replacing the `"\n\nSTATUS:"` magic marker; NEW `rollback_result` poll slot so the /loop rollback flow no longer sticks at "rolling back..."; `rollback_to_capture` takes `LoopCapture` by value and metadata-filters before `take_snapshot`; `deny_reason` compiles glob matchers once per check; verification zombie reaped via `child.wait()` in the try_wait-error branch; config `from_value` deep clone removed while preserving line/col caret diagnostics (two-pass parse); telemetry `into_inner`/`flush` reworked with `Option<Arc>` + `take()`; TUI helpers `plural`/`files_preview`/`current_working_dir` replace 63 silent `unwrap_or_default()` cwd resolutions; codeindex store gains `list_files_with_entries()` single-scan and `get_symbol_by_exact_name()` exact-match-first lookup; 30-test `test_slash_help.rs` covering every slash-command help arm; bang-command `sh -c` wrapped in `spawn_blocking`; `/autopilot` flag errors surfaced; `/spec list` real count, `/spec activate` reuses the SpecManager Arc. |
| v1.0.79 | 2026-09-06 | Research web-search quota controls (`--max-search-calls`, run-scoped `SearchBudget` + `SharedQueryCache`); `--depth` bounds web volume by default (shallow 6 / standard 9 / deep 15); competitive researcher count capped at `max_concurrent_research_units`; `/research update <name>` invocation replay (CLI/TUI/HTTP `PUT /research/{name}`); `--mode competitive` defaults `--format` to `comparison-table`; GitHub `blob/` URLs rewritten to `raw.githubusercontent.com` and non-HTML content bypasses the readability gate; `/clip` slash command; `/research list` human-readable table restored (JSON behind `--json`); `/research create` TUI progress display simplified; provider-search-request counting (`search_providers` RunStep) |
| uncommitted | 2026-09-06 | Config load-cache coherence fix (M-025 CI failure run 34023696563): `Config::load` re-snapshots candidate mtimes after `load_uncached`, `save`/`save_to_source` call the new public `Config::invalidate_load_cache`, cache moved into a shared `load_cache_slot()` so saves are immediately visible to loads. `codeindex_path`/`codeindex_explain` name-resolution fix (exact-match + definition-kind ranking over the substring query). `codeindex_status` tool non-blocking busy report from lock-free atomics (no `with_retry`). `IndexStats` gains `graph_total_edges`/`graph_nodes`/`graph_communities`. Pinned nightly toolchain (`rust-toolchain.toml`) + `TOOLCHAIN.md` review. |
| uncommitted | 2026-09-06 | `plot_*` tool family (`plot_line`/`plot_scatter`/`plot_bar`/`plot_histogram`/`plot_pie`/`plot_heatmap`) via `ratatui-plt` 0.0.2 (GPL-3.0 accepted, allow-listed in `deny.toml`); inline ANSI plot rendering in the TUI message window (`plot_output_lines` + `ansi_line_to_styled`); tool input summaries for code-index graph, `model_info`, and plot tools; 31 new tests (25 plot tools + 6 TUI rendering); how-to PDF set completed (15 A4 xelatex PDFs) |
| v1.0.28 | 2026-08-14 | SDD back-fill: `/spec specify` (SPEC.md only with clarification markers), `/spec plan` (PLAN.md from tech context), `/spec tasks` (TASKS.md + quickstart.md), `/spec feedback` (FEEDBACK.md notes); consistency validation (ambiguity, contradiction, gap detection); `CONSTITUTION.md` with amendment process; `data-model.md` and `contracts/` artifacts; SDD config flags (`sdd.branch_per_spec`, `sdd.data_model`, `sdd.contracts`, `sdd.feedback_loop`); production feedback loop in `/spec plan`; research frontmatter linking with `## Related Research` section |
| v1.0.23 | 2026-08-11 | `/spec update` regenerates `PLAN.md` + `TESTPLAN.md` from edited `SPEC.md`; `/spec create` emits `TESTPLAN.md` manual test plan; `/spec add` regenerates plans after incremental add; `/spec jtbd` Jobs-To-Be-Done analysis; research readability extraction mandatory; YouTube transcript capture fixed |
| v1.0.22 | 2026-08-09 | Fixed time-sensitive `test_parse_natural_time_5pm_tomorrow` CI failure (date-based assertion) |
| v1.0.21 | 2026-08-09 | Fixed CI clippy failure (`#[allow(clippy::too_many_arguments)]` on `log_cron_execution`) |
| v1.0.20 | 2026-08-09 | LLM-callable cron tools (`cron_add`/`cron_remove`/`cron_list`/`cron_enable`/`cron_disable`); `/cron` slash-command enhancements; natural-language timestamp parsing; sub-agent model resolution fix |
| v1.0.19 | 2026-08-09 | Cron capability added (`/cron` slash command with scheduler, execution logging, and guards) |
| v1.0.18 | 2026-08-08 | Perplexity Sonar backend for `mf_search`; edit-log per-tool success/failure analysis |
| v1.0.17 | 2026-08-08 | `collapse_whitespace` matching for `Edit`/`multi_edit`; persistent edit-log toggle (Alt+E); model-independent context-compaction trigger; per-turn "compression skipped" guard |
| v0.1.0-beta.28 | 2026-08-01 | Fixed GitHub release workflow permissions (`contents: write`); reverted `check-and-test` to debug builds and added swap/timeout to avoid OOM; `memory_store` now returns a clear `stored` result |
| v0.1.0-beta.27 | 2026-08-01 | CI runner optimisation: `ubuntu-latest-4-cores`, disabled debuginfo, free-disk-space cleanup |
| v0.1.0-beta.26 | 2026-08-01 | Version bump |
| v0.1.0-beta.25 | 2026-08-01 | Disabled `.rpm` package builds in release workflow |
| v0.1.0-beta.24 | 2026-08-01 | Disabled `.deb` package builds in release workflow |
| v0.1.0-beta.23 | 2026-08-01 | Added `.github/workflows/release.yml` triggered on `v*` tags; packages built with `cargo-deb`/`cargo-generate-rpm` and published via `softprops/action-gh-release@v2` |
| v0.1.0-beta.22 | 2026-07-31 | `/provider` always allows editing API keys; `/model` jumps straight to model list; key/token fields unmasked; research `--use-low-relevance` |
| v0.1.0-beta.21 | 2026-07-30 | Compaction bail paths publish `AgentNotice`; post-compaction continuation nudge; autopilot stops after `agent_complete`; router downstream model/tier status bar; synthetic `Finish`; autopilot status indicator |
| v0.1.0-beta.20 | 2026-07-29 | Research `--from-file` local-document seeding; control-character sanitisation; `html2text` panic catch-up |
| v0.1.0-beta.19 | 2026-07-28 | Clean trailing newlines on startup messages |
| v0.1.0-beta.18 | 2026-07-28 | Startup blocking fixed: MCP, code-index, provider health checks run in background; `/startup` timings; first keystroke after cost banner not swallowed; code-index WAL mode and direct `file_id` queries |
| v0.1.0-beta.17 | 2026-07-27 | `@<path>` instruction-file includes; `open` tool completion; durable `initiative` and `Skill` tools |
| v0.1.0-beta.16 | 2026-07-26 | `conversation_search` and `session_search` tools; message FTS5 and optional embeddings |
| v0.1.0-beta.15 | 2026-07-25 | `browser` tool with CDP automation (14 actions) |
| v0.1.0-beta.14 | 2026-07-24 | `apply_patch` Codex-style patch tool; TUI read-tool header fixes |
| v0.1.0-beta.13 | 2026-07-23 | JCode cost accounting; tool widget fixes |
| v0.1.0-beta.12 | 2026-07-22 | `Event::RunCostSummary` per-run cost summary with persistence and `--include-cost` export; research `mf_fetch`/PDF/YouTube/excluded counts |
| v0.1.0-beta.11 | 2026-07-21 | Tavily backend moved into `mf_search` multi-engine framework |
| v0.1.0-beta.10 | 2026-07-20 | Version bump |
| v0.1.0-beta.9 | 2026-07-19 | `/websearch` diagnostics; Tavily and LangSearch search backends; research source provenance |
| v0.1.0-beta.8 | 2026-07-18 | Version bump |
| v0.1.0-alpha.130 | 2026-07-04 | TODO side panel (Alt+T); agentic-loop performance upgrade (PERFPLAN.md milestones A–F, 26 findings + 5 measurement tasks); MockLlmClient criterion benchmarks; `/perf` TUI alias |
| v0.1.0-alpha.129 | 2026-07-04 | Compression made permanent (removed `compression`/`compression-ml` feature flags); context-compression pipeline always compiled in |
| v0.1.0-alpha.128 | 2026-07-04 | Eliminated all 279 compiler warnings across build, tests, benches, and examples |
| v0.1.0-alpha.116 | 2026-06-23 | Agent-loop persistence/performance fixes; COMMSPLAN team unification, mailbox delivery semantics, unified whitespace-tolerant `Edit`/`multiedit`/`memory_replace` matcher, swarm retry backoff |
| v0.1.0-alpha.113 | 2026-06-20 | Research-system TUI integration wired real gatherers and completion reporting; improved keyword matching and table rendering; corrected spec status to `in_progress` |
| v0.1.0-alpha.112 | 2026-06-20 | Corrected research-system spec status from `implemented` to `draft` |
| v0.1.0-alpha.111 | 2026-06-20 | `AskUserQuestion` promoted to standalone event-driven tool with multiple-choice `options`; removed standalone `question` tool |
| v0.1.0-alpha.110 | 2026-06-20 | Removed internal LLM subsystem (Candle, LiteRT-LM, `/internal-llm` commands, `internal_llm` feature) |
| v0.1.0-alpha.109 | 2026-06-16 | In-process Foundry Local backend, `in_process` provider option, task tool family guidance |
| v0.1.0-alpha.108 | 2026-06-15 | Foundry Local internal-LLM backend; fixed empty SSE stream by polling `/models/loaded` |
| v0.1.0-alpha.107 | 2026-06-15 | Compression pipeline threshold gating on `auto_threshold` |
| v0.1.0-alpha.106 | 2026-06-14 | Microsoft Foundry Local provider integration; Headroom compression lifecycle events |
| v0.1.0-alpha.105 | 2026-06-08 | Headroom compression pipeline, Model Router Provider, compression config, YOLO persistence, `/compress`, TUI toggle persistence |
| v0.1.0-alpha.104 | 2026-07-15 | Amazon Bedrock provider, xAI Grok provider, `/spec impl` and `/spec implement` slash commands |
| v0.1.0-alpha.88 | 2025-01-22 | Fixed context compaction bug in `compact_history_with_atomic_tool_calls`; resolved premature loop break when trimming tool call pairs |
| v0.1.0-alpha.87 | 2025-01-22 | Fixed `read` tool instructions (clarified `end_line` is absolute); strengthened remote push prohibitions in `AGENTS.md`; reorganized SPEC.md sections and numbering |
| v0.1.0-alpha.86 | 2025-01-21 | Azure Resource (File) provider — file-based endpoint catalog, `azureresources.json` schema, TUI integration, integration tests |
| v0.1.0-alpha.82 | 2026-05-20 | Azure AI Foundry provider fixes, `/config show` slash command |
| v0.1.0-alpha.79 | 2026-05-18 | Azure endpoint logging in TUI log panel |
| v0.1.0-alpha.76 | 2026-05-18 | Azure AI Foundry provider added |
| v0.1.0-alpha.75 | 2026-05-18 | SPEC.md mermaid diagram syntax fixes |
| v0.1.0-alpha.73 | 2026-05-18 | `/model` selection fix, version number display |
| v0.1.0-alpha.72 | 2026-05-18 | gen-spec-pdf.sh script, Spec Management section |
| v0.1.0-alpha.71 | 2026-05-18 | Startup ASCII art banner with compile timestamp |
| v0.1.0-alpha.70 | 2026-05-17 | Concurrency improvements, todo fixes |
| v0.1.0-alpha.68 | 2026-05-15 | `/codeindex lang` filtering, benchmark data cleanup |
| v0.1.0-alpha.61 | 2025-01-17 | Instruction file discovery logging |
| v0.1.0-alpha.60 | 2025-01-16 | `.local/share/ragent` in AGENTS.md search path |
| v0.1.0-alpha.57 | 2025-01-16 | MS Office/LibreOffice fixes, todo summary updates |
| v0.1.0-alpha.49 | 2025-01-17 | Permission dialog live countdown, config parse error enhancement, codeindex hardwired permissions, crate extraction milestones |
| v0.1.0-alpha.48 | 2025-01-17 | Permission milestones complete, bash security layers, more permissions fixes |
| v0.1.0-alpha.47 | 2025-01-17 | Crate reorganisation (`ragent-types`, `ragent-config`, `ragent-storage`, `ragent-llm`) |

---

## Appendix B: Documentation

All documentation markdown files are located in `docs/` except for these root files:

| File | Purpose |
|------|---------|
| `README.md` | Project overview |
| `QUICKSTART.md` | Quick start guide |
| `SPEC.md` | This specification |
| `AGENTS.md` | Agent guidelines |
| `CHANGELOG.md` | Change log |
| `RELEASE.md` | Release notes |
| `STATS.md` | Project statistics |

---

## Appendix C: Project Contact & Repository

- **Repository:** https://github.com/thawkins/ragent
- **License:** MIT
- **Author:** Tim Hawkins

---

## Appendix D: Changelog (v0.1.0-alpha.104 → v0.1.0-alpha.116)

### Added
- `@<path>` include directive for instruction files — `AGENTS.md`/`CLAUDE.md`/`.ragent.md`/`INSTRUCTIONS.md` can pull in other markdown files with `@path/to/file.md` (or a quoted form for paths with spaces). The `@` must appear in the first column of the line (no leading whitespace); a leading `@@` is an escape sequence that collapses to a single literal `@`. Directives are expanded transitively before the content is loaded into the system prompt; paths resolve relative to the containing file's directory. Cycle detection (visited-path set) and a depth cap (`MAX_INCLUDE_DEPTH = 16`) prevent infinite loops; absolute paths and `../` escapes outside the working dir / global ragent data dir are rejected with an inline marker comment; missing/unreadable files emit a marker comment rather than failing.
- COMMSPLAN team subsystem hardening (M1–M4)
  - M1: Advisory-lock-protected file stores (`*.json.lock` + UUID temp files) for `Mailbox`, `TaskStore`, and `TeamStore`
  - M2: Single-source team implementation — team runtime (7 modules) and 20 team tools are native to `ragent-agent`; `ragent-team` was a thin re-export shim, **folded into `ragent-agent` in v1.0.121 (ANTIPAT M7.8)**. CI guard `scripts/check-team-duplication.sh`
  - M3: `team_wait` liveness fixes, `team_idle` publishes `TeammateIdle`, unified shutdown path with `immediate` parameter
  - M4: Mailbox peek/ack at-least-once semantics, `team_assign_task` notifications, `team_broadcast` per-recipient results, `team_message` recipient validation, `team_read_messages` snake_case schema
- Unified whitespace-tolerant replacement matcher — `Edit`, `multiedit`, and `memory_replace` now share `ragent_tools_core::replace`, tolerating CRLF, trailing/leading whitespace, collapsed whitespace, blank-line edges, and final-newline mismatches
- Swarm teammate retry backoff — exponential with jitter (`1 s, 2 s, 4 s, 8 s` capped at 30 s)
- Microsoft Foundry Local provider integration — first-class TUI support, `[local]` badge, health checks, config merging (`auto_start`, `device`, `models_path`)
- Microsoft Foundry Local internal-LLM backend (`FoundryLocalExecutor`) and in-process backend (`FoundryLocalInProcClient`)
- Headroom compression pipeline — BM25 scoring, CCR store, aggressive/conservative/default modes, `/compress` slash command
- Headroom compression lifecycle events — `CompressionStarted`/`CompressionFinished` with `original_tokens`, `compressed_tokens`, `compression_ratio`, `did_compress`
- Model Router Provider — 15-dimension classifier for automatic model selection
- Amazon Bedrock provider — SigV4 signing, dual Anthropic Messages / Converse APIs, 9 default models, credential discovery
- xAI Grok provider — `xai` provider for the xAI Grok API
- Research System — `/research` slash commands, `ragent research` CLI, `GET/POST/DELETE /research` HTTP endpoints, spec linkage via `--from-research` and `research:` lines in `PLAN.md`
- `ragent-research` crate — `ResearchName`, `Source`, `ResearchStatus`, `ResearchItem`, web/local gatherers, plan-dep parser
- `AskUserQuestion` standalone tool — event-driven question prompts with optional multiple-choice `options`
- `/spec impl` and `/spec implement` slash commands
- YOLO mode persistence — saved to `ragent.json` and restored on startup
- `compression` config block in `ragent.json`
- `/config save` and `/config list` slash commands — snapshot global `ragent.json` to `saves/` and restore from an interactive picker, with atomic writes and config-cache invalidation
- Unicode-safe truncation in compaction serializer — fixes a char-boundary panic that could crash the agent loop on multi-byte input
- `RAGENT_FOUNDRY_LOCAL_FORCE_WEB` environment escape hatch

### Removed
- Internal LLM subsystem — Candle GGUF, LiteRT-LM, `/internal-llm` slash commands, `InternalLlmConfig`, `internal_llm` Cargo feature, and all related tests/docs
- Standalone `question` tool (functionality moved into `AskUserQuestion`)

### Changed
- `AskUserQuestion` promoted from alias to standalone event-driven tool; permission auto-approval key renamed from `question` to `AskUserQuestion`
- `multiedit` now resolves every edit against original content, detects byte-range overlap, and applies non-overlapping edits highest-end-offset-first so JSON order no longer matters
- `Edit` / `multiedit` / `memory_replace` diagnostics now report edit index, file, last matching pass, and a best-effort closest-line hint
- Relative indentation preservation — `reindent_with` now uses the common leading whitespace of all matched file lines and leaves blank lines untouched
- `team_read_messages` now peeks unread messages and acknowledges only after building `ToolOutput` (at-least-once delivery)
- `team_broadcast` reports per-recipient `succeeded`/`failed` arrays instead of aborting on first failure
- `team_assign_task` rejects `Stopped`/`Failed` teammates and pushes a mailbox notification to the assignee
- `team_message` validates recipient state before delivery
- `team_shutdown_teammate` gained `immediate: bool` parameter (default `false`)
- Foundry Local provider routing branches on `provider.foundry_local.in_process` (default `false`)
- `provider.foundry_local.device` values validated to `auto`, `cpu`, `gpu`, or `npu`
- HuggingFace provider discovery now tolerates missing token and falls back to the static catalog on empty discovery
- `agent_complete` and `list_agents` are hardwired auto-approved
- Built-in agents no longer hardcode Anthropic Claude; they auto-resolve the first available model
- Provider setup dialog scrolls and supports all 12 providers
- TUI `/codeindex`, `/internal-llm` (before removal), and `/tools` toggles persist to project-local `.ragent/ragent.json`
- Compression pipeline gates on `auto_threshold` (default 0.80) before running
- `last_input_tokens` now updates directly from `CompressionFinished` events

### Fixed
- Agent-loop persistence and performance issues — fixed session/cache/storage races and reduced session-processor hot-path overhead (alpha.116)
- Research-system spec status corrected from overstated `implemented` to `draft` (alpha.112)
- Research-system spec status corrected from `draft` to `in_progress` to reflect shipped framework tasks (alpha.113)
- `/research create` now wires real web/local/spec gatherers and reports completion in the TUI
- Research keyword matching improved for punctuation-heavy topics
- `/research list|show|search` table rendering preserves fixed-width formatting
- Microsoft Foundry Local empty SSE stream after model preparation — readiness now polls `/models/loaded`
- Context window display lag after compression — `ctx:` refreshes immediately from `CompressionFinished` events
- `old_str not found` on blank-line / final-newline edge differences
- Collapsed-whitespace false `MultipleMatches` in replacement matcher
- Swarm synthesis task timeouts on cloud LLM providers due to lockstep linear retries
- Team subsystem TOCTOU data-loss races via lock-file-protected stores

## Appendix E: Earlier Changelog (2025-01-16 → 2025-04-21)

### Added (v0.1.0-alpha.82 → v0.1.0-alpha.86)
- Azure Resource (File) provider — New `azure_resource` provider reads endpoint definitions from `azureresources.json` in `~/.config/ragent/` or `.ragent/`
- TUI integration for Azure Resource provider — Dedicated picker in provider setup dialog, last-selection persistence, stale-selection cleanup
- Azure Resource integration tests — Provider listing, persistence round-trip, ModelInfo conversion, backend resolution
- Azure Resource documentation — `docs/userdocs/azure-resource.md` with schema reference and troubleshooting
- File format specification — `specs/AzureResource/FILEFORMAT.md` documenting the complete `azureresources.json` format
- `agent_complete` summary display — TUI widget output now shows task completion summaries

### Added (v0.1.0-alpha.76 → v0.1.0-alpha.82)
- Azure AI Foundry provider — New `azure_foundry` provider for Microsoft Azure AI Foundry models
- Azure endpoint logging — Full resolved endpoint URL displayed in TUI log panel
- `/config show` slash command — Displays current resolved configuration
- gen-spec-pdf.sh script — Pandoc + Chromium-based Markdown-to-PDF conversion
- Startup ASCII art banner — Application name in ASCII art with compile timestamp
- `/codeindex lang` filtering — Optional language parameter for code index results
- Instruction file discovery logging — Tracks AGENTS.md-style file discovery with summary

### Changed (v0.1.0-alpha.82 → v0.1.0-alpha.86)
- Provider count updated from 10 to 12 (added `bedrock`, `xai`)

### Changed (v0.1.0-alpha.68 → v0.1.0-alpha.75)
- Improved TUI slash-command autocomplete with safe `Esc` handling
- SPEC.md mermaid diagram syntax fixes — All 14 diagrams now render correctly
- Benchmark data cleanup — Removed unused dataset files

### Fixed (v0.1.0-alpha.76 → v0.1.0-alpha.82)
- Azure endpoint logging now shows full URL with model name
- `/model` selection handling fixed

### Earlier Changes (v0.1.0-alpha.47 → v0.1.0-alpha.68)
- Permission dialog countdown timer with live TUI updates (120-second timeout)
- Config parse error reporting with file path, line, column, and caret marker
- Codeindex tools hardwired as always-allowed (read-only, no permission prompts)
- Crate extraction milestones: `ragent-types`, `ragent-config`, `ragent-storage`, `ragent-llm`
- `ollama_cloud` provider with dynamic model discovery and vision support
- `gemini` provider with massive context windows (up to 2M tokens)
- `huggingface` provider with dynamic model discovery and rate limit tracking
- GitLab integration with issues, merge requests, pipelines, and jobs
- Team coordination tools (20 tools for team lifecycle, tasks, messaging)
- MCP (Model Context Protocol) client support
- Skills system for loadable skill packs
- Custom agent profiles via OASF format
- Autopilot mode for autonomous operation
- `/prompt` read-only agent system-prompt inspector (primary/subagent modes, roster, per-agent override, no LLM call)
- Tool-repeat guard (FR-044) blocking agent loops that replay identical tool calls
- Memory system with three tiers (file blocks, SQLite store, semantic search)
- Journal system for insights and decisions
- Background agent spawning and management
- Swarm mode for parallel task decomposition
- Plan agent with human-in-the-loop approval
- Enhanced bash security with 7 layers and word-boundary matching
- Permission system now supports per-agent rules and YOLO mode
- **Research System** (`/research` slash command, `ragent research` CLI, `GET/POST/DELETE /research` HTTP endpoints, with spec linkage via `--from-research` and `research:` lines in `PLAN.md`)

---

## Research Configuration

The `/research create` synthesis prompt (in `crates/ragent-research/src/analysis.rs`)
honours several optional `ragent.json` keys under a top-level `research` object.
All are opt-in and default to the legacy behaviour, so existing
configurations continue to work unchanged.

### `research.few_shot` (boolean, default `false`)

When `true`, the synthesis prompt appends up to two short exemplar findings
after the output-template instructions so the model can calibrate the exact
label structure, `[#N]` citations, and **Sources Cited / Date Spread**
paragraph. Exemplars are format-calibration only — the prompt instructs the
model not to copy their content into its answer and to derive findings from
the supplied sources. Keep this `false` on small-context models to avoid
consuming the context window.

### `research.analysis_persona` (string, optional)

When present, replaces the default `system` message persona
(`"You are a careful research analyst. ..."`) verbatim. Use this to tailor
voice, audience, and domain framing, e.g.
`"You are a senior security research analyst for a venture-capital audience."`.
When absent, the default analyst persona is used.

### `research.models.*` (string, optional)

Per-phase default model overrides. All fields are optional; when omitted the
phase falls back to the configured default model.

- `research_model` — model used by research agents / sub-topic workers.
- `compression_model` — model used to compress or summarise intermediate findings.
- `final_report_model` — model used to write the final report.
- `summarization_model` — lightweight model used for per-page webpage summaries (FR-002).

### `research.supervisor.max_concurrent_research_units` (integer, default 5)

Maximum number of researcher agents that may run concurrently in `supervisor`
and `competitive` modes (FR-012).

### `research.evaluate.enabled` (boolean, default `false`)

When `true`, every research run appends a deterministic quality scorecard to
the assembled `RESEARCH.md` (FR-015). This is equivalent to passing
`--evaluate` on every `/research create` or `ragent research create` call.

### Example stanza

```jsonc
{
  "research": {
    "few_shot": true,
    "analysis_persona": "You are a senior research analyst for a venture-capital audience. Read the provided sources and produce a structured markdown analysis. Use only the evidence in the sources; do not invent facts.",
    "models": {
      "summarization_model": "ollama:phi4",
      "compression_model": "ollama:phi4",
      "research_model": "openrouter/anthropic/claude-sonnet-4-20250514",
      "final_report_model": "openrouter/anthropic/claude-sonnet-4-20250514"
    },
    "supervisor": {
      "max_concurrent_research_units": 3
    },
    "evaluate": {
      "enabled": true
    }
  }
}
```

### Implementation status

The prompt-builder knobs (`SynthesisPromptConfig.few_shot_examples`,
`SynthesisPromptConfig.persona`) and the `LlmAnalysisEngine::with_persona`
builder are implemented in `crates/ragent-research/src/analysis.rs`. Model
overrides, supervisor concurrency, and evaluation are propagated from
`ResearchRunRequest` through `build_session_config` in
`crates/ragent-research/src/run_request.rs` to the engine phases. The
`ragent.json` `research.*` keys are read via `Config::merge` in
`crates/ragent-config/src/config.rs` and propagated through
`build_session_config` so CLI/TUI defaults match the config file. Callers can
still override per run with the corresponding flags.

## Appendix F: Changelog (v0.1.0-beta.1 → v0.1.0-beta.28)

### Added (beta.2–beta.28)
- **Release packaging** — Tag-triggered `release.yml` workflow builds the `ragent` binary and generates `.deb`/`.rpm` packages; the release body is extracted from `CHANGELOG.md` and published via `softprops/action-gh-release@v2` (beta.23)
- **CI hardening** — `ubuntu-latest-4-cores` runner, disabled debuginfo, `free-disk-space` cleanup, and `contents: write` permission fix for GitHub Release creation (beta.25–beta.28)
- **Provider setup UX** — `/provider` always opens the key-entry dialog so existing API keys can be edited; `/model` skips the provider picker when a provider is already configured; API-key and GitLab-token fields shown unmasked in a wider dialog (beta.22)
- **Research seeding & enrichment** — `--from-file` local-document seeding (PDF, DOCX, XLSX, PPTX, ODT, ODS, ODP, TXT, MD); `--from-url` URL seeding; `--use-low-relevance` flag; PDF and YouTube text extraction; excluded-source counts; `search_tool`/`search_engine` provenance; `mf_fetch` used as the preferred fetch path (beta.9–beta.22)
- **Research resilience** — Control-character sanitisation in `RESEARCH.md`; `html2text` panic caught and degraded to raw text; source citations stripped of redundant `mf_fetch:` header (beta.12–beta.20)
- **Context compaction reliability** — Bail paths publish `AgentNotice` instead of silently failing; post-compaction continuation nudge threaded across loop iterations; compaction prompt cap reduced to 60 k chars; pre-serialised head reused for token-cost calculation (beta.18, beta.21)
- **Autopilot & router fixes** — Autopilot auto-continue suppressed after `agent_complete`; TUI status bar shows downstream model/tier for `router`; synthetic `Finish { Stop }` injected when a provider stream ends without terminal signal; autopilot status indicator (beta.21)
- **Startup responsiveness** — MCP server connections, code-index open/watcher/reindex, and provider health checks moved to background tasks; `/startup` slash command shows per-stage timing; first printable keystroke after the run-cost banner is no longer swallowed (beta.18–beta.19)
- **Code-index performance** — SQLite `WAL` mode, `synchronous = NORMAL`, `temp_store = MEMORY`; direct `file_id` symbol queries; reindex chunk yield reduced to 1 ms; per-phase timing logs (beta.18)
- **Tool expansion** — `apply_patch` Codex-style patch tool; `open` cross-platform reveal/URL tool; `browser` CDP automation tool; `conversation_search`/`session_search`; `bg` background shell task manager; `initiative` durable goals; `Skill` runtime skill control; six `mf_*` MasterFetch tools (beta.12–beta.17)
- **Cost accounting** — `Event::RunCostSummary` published per turn; persisted to SQLite `run_cost_summaries`; TUI banner overlay; `--include-cost` session export flag; HTTP SSE `run_cost_summary` event (beta.12)
- **Telemetry & operator tools** — OpenTelemetry metrics export, `/telemetry` slash family, ALT-O telemetry panel, `sudo` askpass broker, `askpass` environment wiring (beta.2–beta.5)
- **Instruction-file includes** — `@<path>` directive with cycle detection, depth cap, escape sequences, and path containment checks (beta.17)

### Changed (beta.2–beta.28)
- Provider registry now exposes 13 IDs including `router`; `foundry_local` is no longer a standalone registry entry (its functionality is accessed via the router/backend configuration)
- Tool count grew to ~150 registered tools across 18 categories, including aliases for commonly hallucinated names (`update_file`, `run_code`, `AskUserQuestion`, `multiedit`)
- Web-search stack now prefers the multi-engine `mf_search` tool (DuckDuckGo, Brave, Tavily, LangSearch) over legacy `websearch`
- `.deb` and `.rpm` packaging temporarily disabled in CI while build paths are reviewed (beta.24–beta.25)

### Fixed (beta.8–beta.28)
- `memory_store` tool output now includes `"stored": true`, and the TUI result summary falls back to `id` presence so successful writes are not reported as "memory not stored"
- CI `check-and-test` job reverted from `--release` to debug builds (`cargo check/test --workspace`) and gained an 8 GiB swapfile step plus a 45-minute timeout to avoid runner OOM kills
- Skill discovery tests isolated by `SkillScope` and `bundled_count()` to avoid test fragility
- Doctest build breakages in `session::permissions` and `tool::ToolRegistry`
- TUI read-tool header now uses pending args when `ToolCallStart` is dropped, and shows `📄 missing path` for malformed calls
- `RUSTSEC-2025-0052` (`async-std` discontinued) advisory ignored in `cargo-deny` configuration (beta.26)
```jsonc
    // Plugin subsystem (spec `plugins`). `enabled: false` makes the whole
    // subsystem inert (no discovery or loading). Budgets default to 5 s per
    // tool, 10 s per entry point, 64 MiB per JS context.
    "plugins": {
      "enabled": true,
      "max_execution_ms": 5000,
      "max_entry_ms": 10000,
      "max_memory_mb": 64
    },
    "yolo": false,
    "stream": {
      "timeout_secs": 120,
      "initial_response_timeout_secs": 300
    },
    "dirs": {
      "allowlist": ["src/**", "tests/**"],
      "denylist": ["secrets/**", ".env"],
      "allowed_roots": ["/home/alice/shared"]
    }
  }
}
```

### 5.3 Environment Variables

| Variable | Provider / Purpose |
|----------|--------------------|
| `ANTHROPIC_API_KEY` | Anthropic |
| `OPENAI_API_KEY` | OpenAI |
| `GITHUB_TOKEN` | GitHub / Copilot |
| `GEMINI_API_KEY` | Google Gemini |
| `HF_TOKEN`, `HUGGING_FACE_HUB_TOKEN` | Hugging Face |
| `AZURE_AI_FOUNDRY_API_KEY`, `AZURE_AI_FOUNDRY_BASE` | Azure AI Foundry |
| `AZURE_RESOURCE_API_KEY` | Azure Resource |
| `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, `AWS_SESSION_TOKEN`, `AWS_PROFILE`, `AWS_REGION` | Amazon Bedrock |
| `XAI_API_KEY` | xAI Grok |
| `RAGENT_FOUNDRY_LOCAL_FORCE_WEB` | Force Foundry Local web-service path |
| `LANGSEARCH_API_KEY` | Web search (LangSearch) |
| `TAVILY_API_KEY` | Web search (Tavily) |

### 5.4 Compaction Configuration

The `compaction` block controls OpenCode-derived summarisation-based context-window
compaction, which replaces the older Headroom `compression` scheme. It satisfies
FR-008 (auto toggle) and FR-011 (user-overridable threshold, buffer, keep-tokens,
and summary output-token values).

```jsonc
{
  "compaction": {
    // Enable automatic pre-send summarisation. When false, only emergency
    // overflow compaction runs on provider context-overflow errors (FR-008).
    "auto": true,
    // Fraction of the model's context window at which automatic pre-send
    // compaction triggers (0.0–1.0). Default: 0.7 (70 %). This is the
    // recommended, model-independent trigger: 70 % of a 32k window is 22.4k
    // tokens, while 70 % of a 200k window is 140k tokens. The threshold is
    // never below 70 %, even when a fraction-based fallback is used.
    "threshold": 0.7,
    // Response/safety buffer as a fraction of the model's context window.
    // Only used when `threshold` is null. Compaction then triggers when
    // estimated request tokens exceed context_window - max(output_tokens,
    // buffer). Default: 0.10 (10 %, FR-011).
    "buffer": 0.10,
    // Recent conversation turns kept verbatim after compaction.
    "keep": {
      // Fraction of the context window reserved for recent user/assistant/tool turns to preserve.
      // Default: 0.20 (20 %, FR-011).
      "tokens": 0.20
    },
    // Optional dedicated fast/cheap model for the summarisation call
    // (avoids paying the session model's latency for compaction).
    "model": { "provider_id": "ollama", "model_id": "qwen2.5:1.5b" },
    // Maximum tokens requested for the compaction summary output.
    // Default: 1500 (was 4096; smaller budget = proportionally lower latency).
    "summary_tokens": 1500,
    // Truncation limit per tool output serialised into the summarisation prompt.
    // Default: 2000 characters.
    "tool_output_max_chars": 2000
  }
}
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `auto` | bool | `true` | Enable automatic pre-send compaction (FR-008) |
| `threshold` | f64 | `0.7` | Fraction of context window that triggers compaction; model-independent |
| `buffer` | f64 | `0.10` | Fallback response/safety buffer as a fraction of the context window when `threshold` is null (FR-011) |
| `keep.tokens` | f64 | `0.20` | Fraction of context window reserved for recent turns preserved verbatim (FR-011) |
| `model` | object | none | Optional fast/cheap model override for the summarisation call (`/compact` and `compact_session` only); the session's primary model is used when unset. The pre-send auto-compaction path always uses the session's primary model and publishes an `AgentNotice` when an override is configured |
| `summary_tokens` | usize | `1500` | Maximum tokens requested for the compaction summary output (FR-011) |
| `tool_output_max_chars` | usize | `2000` | Truncation limit for each tool output serialised into the summarisation prompt |

The compaction trigger threshold is always raised to at least 70 % of the
model's context window, so automatic pre-send compaction never fires on routine
prompts that fill less than 70 % of the available context. Emergency overflow
compaction is not subject to this floor.

The compaction summary output length defaults to `1500` tokens (configurable
via `compaction.summary_tokens`) and tool outputs are truncated to `2000`
characters (`compaction.tool_output_max_chars`) before being serialised into
the summarisation prompt. The prompt itself is capped adaptively at
`min(60,000 chars, context_window * 4 / 2)` so small local models are not
swamped. The summarisation stream has a hard 180-second overall cap plus a
60-second per-chunk stall timeout, and progress is reported every 10 seconds
with the elapsed time and character count received so far. When
`compaction.model` is set, the `/compact` path builds a dedicated client for
the override model instead of reusing the session's primary model. The
pre-send auto-compaction path always uses the session's primary model and
publishes an `AgentNotice` when an override is configured.

The legacy `compression` block (`compression.enabled`, `compression.mode`,
`compression.auto_threshold`) is still parsed for one-release migration:
`compression.enabled` is treated as `compaction.auto` when `compaction.auto` is
not set explicitly. New configurations should use `compaction`.

Trigger the one-shot compaction manually with the `/compact` slash command
(`/compress` is a deprecated alias — FR-009).

### 5.6 GCF Encoding Configuration

The `gcf` block controls GCF (Graph Compact Format) encoding of eligible JSON
tool results in the LLM view. GCF is a token-efficient, lossless
key=value text format: when enabled, tool results of at least 200 characters
that parse as a JSON object or array are re-encoded inside a
`[BEGIN GCF generic]` ... `[END GCF]` labelled block, but only when the GCF
output is at least 10 % smaller than the raw JSON (results that save nothing
stay raw). Any encode failure falls back to the raw JSON, so the model always
receives the full information content (FR-007). TUI rendering, the activity
log, memory extraction, hooks, and compaction keep seeing the raw JSON; only
the model-facing copy is re-encoded (FR-005). While GCF is enabled the system
prompt also gains a short reading primer so the model can decode the block
format (FR-006).

```jsonc
{
  // Default off; the whole section is omitted from the saved file while
  // disabled (skip_serializing_if default rule, FR-001).
  "gcf": {
    "enabled": false
  }
}
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `enabled` | bool | `false` | Enable GCF encoding of eligible JSON tool results in the LLM view (FR-004) |

Toggle at runtime with `/gcf on` / `/gcf off` (persists to the loaded config
source), inspect with `/gcf show`, and view usage with `/gcf help` (FR-002,
FR-003). `Alt+G` toggles GCF with the same persist semantics, and a status-bar
indicator on line 2 (compression-clamp icon, left of the codeindex icon) shows
the enabled/disabled state.

### 5.5 Thinking Configuration

Thinking/reasoning is configured per model:

```jsonc
{
  "provider": {
    "anthropic": {
      "models": {
        "claude-sonnet-4-20250514": {
          "thinking": { "enabled": true, "level": "medium" }
        }
      }
    }
  }
}
```

Levels: `auto`, `off`, `low`, `medium`, `high`. `budget_tokens` is optional.

---

## 6. Terminal User Interface (TUI)

### 6.1 TUI Layout

The TUI is a ratatui full-screen interface with these panels:

| Panel | Purpose |
|-------|---------|
| Chat | Conversation stream with markdown and syntax highlighting |
| Log | Step-numbered tool calls with pretty-printed JSON |
| Status bar | Provider/model, context usage, compression indicator, YOLO state |
| Teammate strip | One row of pills under the status bar, one per active teammate (status glyph, clipped name, focused highlight, `Alt+^v:cycle` hint); omitted when no team is active (spec `agentui` FR-027) |
| Agents window | `Agents`-button popup listing the primary agent and an ancestor-indented sub-agent tree with status, elapsed time, step count, `[stop]`/`[x]` status glyphs, and a background-shell section; keyboard-first: a block cursor (`Up`/`Down`/`PgUp`/`PgDn`) selects a row, `Space` toggles suspend/resume, `Del` arms the kill confirmation, `Enter` opens the row's output view, and clicking a row selects it; the FR-077 key legend sits in the footer (spec `agentui` FR-006..FR-016) |
| Teams window | `Teams`-button popup listing the lead plus every teammate with focus markers, status words from `MemberStatus::display_label()`, model column (inherited global model suppressed), and claimed/done/sent/recv counters; the block cursor plus `Enter`/`Space`/`Del` drive focus, suspend and shutdown; clicking a teammate selects it (spec `agentui` FR-017..FR-026) |
| Input | Command-line with slash-command autocomplete |
| Permission dialog | Modal approval dialog with live countdown (120 s) |
| Question dialog | Modal user question (free-text or multiple-choice) |
| Row-action dialog | Yes/No confirmation for destructive row actions (sub-agent kill, background-shell cancel, teammate suspend/shutdown) drawn over the windows, keyboard and mouse answered (spec `agentui` T-009) |

### 6.2 Slash Commands

| Command | Description |
|---------|-------------|
| `/quit` / `/q` | Exit ragent |
| `/provider` | Open provider setup dialog (always allows editing the stored key) |
| `/model` | Select model (jumps to model list when a provider is already configured) |
| `/agent` | Select agent |
| `/agents` | List loaded agents and diagnostics |
| `/websearch show` | Show web-search engine diagnostics (enabled / in-use / failed) |
| `/websearch help` | Show `/websearch` subcommand help |
| `/websearch test` | Test configured web-search backends |
| `/webapi enable\|disable\|help` | Manage the HTTP REST API |
| `/tools` | Tool-family visibility: `/tools list` (alias `show`) prints the table, `/tools <switch> on\|off` toggles one, `/tools help` prints help |
| `/codeindex on\|off` | Enable/disable code index |
| `/codeindex lang <language>` | Filter code index by language |
| `/gcf on\|off\|show\|help` | Toggle GCF token-efficient tool-result encoding; `show` reports state and source |
| `/compact` | Summarise and compact the conversation history (one-shot LLM summarisation; FR-009). `/compress` is a deprecated alias |
| `/memory` | Structured-memory panel and summary: `/memory` / `/memory show` (same), `/memory clear` (a `Yes`/`No` dialog clears this project's memories), `/memory help` |
| `/yolo` | Toggle YOLO mode (`on`/`off` set and persist the state, `status` reports it, a bare `/yolo` toggles; the toggle writes the user-global config, or the `--config` file when one was loaded) |
| `/todo` `/task` | Open the TASKS side panel (also Alt+T); subcommands delegate to task tools |
| `/team create <name>` | Create a team |
| `/team open <name>` | Re-open existing team |
| `/team close` | Close current team |
| `/team clear` | Reset current team state |
| `/team delete <name>` | Delete team on disk |
| `/team cleanup` | Tear down current team |
| `/team message ...` | Send team message |
| `/swarm <prompt>` | Decompose prompt into parallel subtasks |
| `/swarm status` | Show active swarm tasks |
| `/swarm kill` | Cancel active swarm |
| `/spawn <agent> <prompt...>` | Launch a detached fire-and-forget background sub-agent from the input line (spec `spawnagent`); not listed by `list_agents`, not awaitable via `wait_agents`, its result never injected back into the chat |
| `/autopilot on\|off` | Toggle autonomous mode |
| `/spec create\|specify\|plan\|tasks\|update\|add\|feedback\|jtbd\|list\|search\|show\|validate\|status\|task\|impl\|coverage\|activate\|deactivate\|delete` | Spec lifecycle and SDD commands |
| `/plugins list\|add\|remove\|enable\|disable\|test\|stores\|marketplace\|help` | Manage sandboxed Codex/Claude plugins; `/plugins list [--verbose]` opens an interactive panel (the plugin table pinned at the top - a fixed column heading and separator - above a scrollable block-cursor list, with the capability blocks listed at the end of the same body; `Up`/`Down`/`Home`/`End` move the cursor, `Space` toggles the highlighted plugin's enabled state (enable/disable, reporting the outcome on a feedback line), and `Esc` closes); `/plugins test` runs an isolated harness; `/plugins codex [query] [--category <name>] [--refresh]` and `/plugins claude [query] [--category <name>] [--refresh]` browse each store's official marketplace with a left-hand category navigator (`ALL` plus the distinct categories the fetched index declares) operated by keyboard and mouse (its own document shape is normalised by that store's `StoreProvider`), and each row showing the plugin id, version, the marketplace it came from, and the description; `/plugins stores [--check]` reports each store's effective endpoint and its source, and with `--check` also contacts each store to report availability and plugin count; `/plugins marketplace show` lists the marketplaces configured for both stores in one section each (Codex, Claude) as a name/url/description table; `/plugins marketplace <codex|claude> add <source> [--force]|remove <name>|list` manages additional marketplaces (from an `owner/repo` slug, an SSH remote, an `https://` URL, or a local directory) in the user-global `plugins.stores.marketplaces`, which the two browse panels fetch and merge into the one panel; `/plugins add` accepts a `git+<https-url>#<ref>[:<subpath>]` git source; CLI parity via `ragent plugins` |
| `/queue list\|clear\|next\|help` | Inspect the message input queue (messages and slash commands submitted while the agent executes; spec `inputqueue` FR-013/FR-017 amendment) |
| `/todo` `/task` | Open the TASKS side panel |
| `/skill help\|list\|<name> [args]` | Load or inspect skill packs: list them, or load and invoke `<name>` |
| `/osinfo show [--no-probe]\|help` | Read-only host OS/hardware report; routes through the `os_info` tool's collector/renderer (spec `osinfo`) |
| `/research create\|list\|show\|search\|cluster\|archive\|delete\|update` | Research commands; `create` supports `--from-file`, `--from-url`, `--use-low-relevance`, `--papers`, `--oa-enable`/`--no-oa`, `--max-concepts N`, `--max-findings N`, `--url-cloak`, `--age <SPAN>` |
| `/config show` | Show resolved configuration |
| `/config save` | Snapshot global `ragent.json` to `saves/` (atomic, timestamped) |
| `/config list` | Interactive picker to restore a saved backup |
| `/init config` | Create a default `ragent.json` in the global config directory |
| `/startup` | Show per-stage startup timing instrumentation |
| `/prompt help\|primary [agent]\|subagent [agent]\|list\|<agent>` | Read-only agent system-prompt inspector (no LLM call) |
| `/telemetry help\|on\|off\|setup\|counters` | Manage OpenTelemetry metrics export |
| `/dirs` | Show configured writable directories |
| `/profile` / `/status` / `/mouse` | UI preferences |
| `/skill` / `/skills` | Load or inspect skill packs |
| `/mcp [status]\|config\|discover\|connect <id>\|disconnect <id>` | MCP server status, discovery location report, and enable/disable |
| `/update` / `/update install` | Auto-update (reserved; not implemented) |

### 6.3 TUI Component Architecture

```mermaid
graph LR
    App[App State] --> EventLoop[Event Loop]
    EventLoop --> Terminal[Terminal]
    App --> Chat[Chat Widget]
    App --> Log[Log Widget]
    App --> Input[Input Widget]
    App --> Status[Status Bar]
    App --> Windows[Agents / Teams Windows + Output View Legend]
    App --> Dialogs[Permission / Question Dialogs]
    EventBus --> App
```

**Figure 7:** TUI Component Architecture — UI layout and event wiring

### 6.4 Permission Dialog Countdown

The permission dialog displays a live countdown:

- Format: `M:SS` (e.g., `1:45`)
- Shows `EXPIRED` when the 120-second timeout is reached
- The event loop redraws continuously so the timer decrements without requiring keyboard input

---

## 7. HTTP Server & API

### 7.1 Starting the Server

```bash
ragent serve --port 9100 --host 127.0.0.1
```

### 7.2 REST Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/health` | Health check |
| POST | `/sessions` | Create a session |
| GET | `/sessions` | List sessions |
| GET | `/sessions/{id}` | Get session details |
| POST | `/sessions/{id}/messages` | Send a message (returns SSE stream) |
| GET | `/sessions/{id}/events` | SSE event stream |
| GET | `/models` | List available models |
| GET | `/providers` | List providers |
| GET | `/agents` | List agents |
| GET | `/research` | List research items |
| POST | `/research` | Create + run a research gathering session (returns `202 Accepted` with `Location` header) |
| GET | `/research/{name}` | Show one research item (supports `?full=true` for extended metadata) |
| DELETE | `/research/{name}` | Delete a research item (requires `?confirm=delete-{name}`) |
| GET | `/research/{name}/events` | SSE stream of live research events for a background run |

### 7.3 SSE Events

The server streams the following event types:

| Event | Description |
|-------|-------------|
| `assistant_chunk` | Token or reasoning block |
| `tool_requested` / `tool_executed` | Tool lifecycle |
| `permission_requested` / `permission_replied` | Permission flow |
| `question_requested` / `question_answered` | Question tool |
| `compression_started` / `compression_finished` | Context compaction lifecycle |
| `subagent_*` / `teammate_*` | Multi-agent lifecycle |
| `agent_notice` | Status messages |
| `research` | Research session events (phase, web capture, synthesis, analysis, done) — streamed via `GET /research/{name}/events` |

### 7.4 Authentication

The HTTP API uses Bearer token authentication:

```bash
curl -H "Authorization: Bearer $RAGENT_TOKEN" ...
```

`RAGENT_TOKEN` is read from the environment. If unset, the server may allow
local requests without authentication depending on build configuration.

### 7.5 HTTP API Request Flow

```mermaid
sequenceDiagram
    participant Client
    participant Axum
    participant SP as Session Processor
    participant LLM
    Client->>Axum: POST /sessions/{id}/messages
    Axum->>SP: dispatch message
    SP->>LLM: streaming request
    LLM-->>SP: chunks / tool calls
    SP-->>Axum: events
    Axum-->>Client: SSE stream
```

**Figure 8:** HTTP API Request Flow — REST + SSE lifecycle

---

# Part III: Data & Knowledge Systems

---

## 8. Code Index

### 8.1 Overview

The code index provides fast, local, read-only code intelligence. It is built
on tree-sitter parsing and Tantivy full-text search, with a SQLite metadata
store.

### 8.2 Supported Languages

Rust, Python, TypeScript/JavaScript, Go, C/C++, Java, OpenSCAD, Terraform,
CMake, Gradle, Maven, and more (15+ grammars compiled-in).

### 8.3 Architecture

```mermaid
graph LR
    Scan[File Scanner] --> Parse[Tree-sitter Parser]
    Parse --> Store[(SQLite Metadata)]
    Parse --> Index[Tantivy FTS Index]
    Query[Tool Query] --> Index
    Query --> Store
```

**Figure 9:** Code Index Pipeline — File scan → parse → index → search

### 8.4 Code Index Tools

| Tool | Purpose |
|------|---------|
| `codeindex_search` | Full-text search across indexed code |
| `codeindex_symbols` | Search for symbols by name |
| `codeindex_references` | Find references to a symbol |
| `codeindex_dependencies` | Query file-level imports/dependents |
| `codeindex_status` | Show index status |
| `codeindex_reindex` | Trigger a full re-index |

All code index tools are **hardwired always-allowed** because they are read-only
and local-only.

### 8.5 Semantic Code Graph

The code index also maintains a **semantic code graph** — a typed edge graph
over indexed symbols that captures relationships such as `calls`, `imports`,
`inherits`, `references`, `mixes_in`, and `implements`. Edges are either
`EXTRACTED` (from tree-sitter parse data) or `INFERRED` (heuristic).

The graph is built on demand via `/codeindex graph build` (or
`/codeindex graph lang <language>` for a per-language subgraph) and persisted
in the `graph_edges` SQLite table. Graph builds run on a dedicated OS thread
(`codeindex-graph-build`) via `CodeIndex::spawn_graph_build`, so the TUI event
loop stays responsive while the derivation runs; a double-build guard refuses
overlapping builds.

Graph builds use a phased lock discipline: the store mutex is taken only for
two brief windows - a read-only snapshot of the derivation inputs (symbols,
refs, imports, files) and the final single-transaction persist into
`graph_edges`. The CPU-heavy derivation runs with no locks held, so FTS search
and the other store readers remain available while the graph builds; only the
short write transaction touches the `graph_edges` table.

#### Graph Tools

| Tool | Purpose |
|------|---------|
| `codeindex_godnodes` | Top-N most-connected symbols (highest degree) |
| `codeindex_path` | Shortest path (by hop count) between two symbols |
| `codeindex_explain` | Node metadata and incoming/outgoing edges for a symbol |
| `codeindex_communities` | Community detection via label propagation |

All graph tools use non-blocking `try_*` variants and return a
`codeindex_busy` response when the index is locked (e.g. during a reindex).
Graph name resolution (`codeindex_path`, `codeindex_explain`) prefers exact
name matches over the underlying substring query and ranks definition kinds
(struct/function/trait/class/enum/interface) above impl/module containers, so
a trait named `CachedSessionProcessor` no longer shadows the struct
`SessionProcessor`.

#### Graph Status

`/codeindex show` reports graph-level statistics alongside index stats:
edge count (total, extracted, inferred), node count, per-kind edge counts,
and community count. While a graph build is in flight, `/codeindex show`
reports `**Graph:** building...` with per-file progress (`done/total`), and
the `codeindex_status` tool reports `graph_state: "building"` plus
`graph_building`/`graph_done`/`graph_total` metadata. `codeindex_status`
never blocks on the store lock: when the lock is held it returns an immediate
busy report built from the lock-free progress atomics (metadata `busy: true`,
`error: "codeindex_busy"`). The `IndexStats` type carries `graph_total_edges`/
`graph_nodes`/`graph_communities` so status surfaces can report "graph built"
vs "not built" without a separate graph query.

### 8.6 Code Index Status Indicators

The TUI status bar (second line, top-right) shows two bold busy indicators
while the code index works:

- `IDX` glyph (`idx`) -- the FTS/store indexes are being updated (a reindex
  holds the store/FTS locks or the reindex progress counters are active).
- `GRAPH` glyph (`graph`) -- the semantic edge graph is being (re)built.

Both busy tags render in every responsive mode and are polled lock-free from
`CodeIndex` atomics (`reindex_progress()` / `graph_busy()` /
`graph_build_progress()`) by `App::refresh_code_index_stats`, so they stay
live even while the index mutexes are held by the background work.

Attribution rule: when the store lock is unavailable (`try_status()` returns
`None`) and `graph_busy` is set, the lock is attributed to the graph build --
the `idx` indicator is not latched and is cleared if stale, so the two
indicators track their own phases and never vanish simultaneously at build
completion.

### 8.7 Incremental Updates

A file watcher detects changes and incrementally updates the index. Language
filtering is available via `/codeindex lang <language>`.

---

## 9. Memory System

### 9.1 Three Tiers

| Tier | Storage | Use Case |
|------|---------|----------|
| **File blocks** | Markdown files in `.ragent/memory/` or `~/.ragent/memory/` | Long-form notes, patterns, preferences |
| **Structured store** | SQLite via `memory_store` / `memory_recall` | Facts, errors, workflows with tags/confidence |
| **Semantic search** | Optional embeddings (`all-MiniLM-L6-v2`) | Similarity-based recall |

### 9.2 Memory Operations

| Tool | Purpose |
|------|---------|
| `memory_read` | Read a memory block file |
| `memory_write` | Write/append to a memory block file |
| `memory_replace` | Replace a string in a named block |
| `memory_store` | Store a structured memory entry |
| `memory_recall` | Full-text search structured memories |
| `memory_search` | Semantic/keyword search across memories |
| `memory_forget` | Delete memories by filter |
| `memory_migrate` | Split a flat MEMORY.md into blocks |
| `conversation_search` | Keyword/turn-range/stats search over the current session |
| `session_search` | Cross-session full-text search with filters and context |

### 9.3 Automatic Extraction

After each assistant turn the system can extract:

- Project facts and patterns
- Errors and their resolutions
- User preferences
- Workflows and standard operating procedures

Extracted memories are tagged with category, confidence, and source.

### 9.4 Decay, Compaction, and Knowledge Graph

- Memories decay in relevance over time unless accessed or reinforced.
- Compaction summarises old memories.
- A lightweight knowledge graph links related memories by tag and reference.

---

## 10. Spec Management

### 10.1 Overview

Ragent includes a built-in specification lifecycle for tracking features,
requirements, and implementation tasks.

### 10.2 Directory Layout

Specs live in `specs/<SpecId>/`. Only `SPEC.md` is required; every other file in
the layout is optional and derived, so a spec whose plan has not been generated
yet is still listed, readable, and a valid `/spec update` / `/spec plan` target:

```text
specs/
└── testspec/
    ├── SPEC.md          # EARS requirements and status (required)
    ├── PLAN.md          # Implementation tasks and Phase -1 gates
    ├── TASKS.md         # Ordered task list extracted from PLAN.md
    ├── TESTPLAN.md      # Manual test-plan with TC-NNN test cases
    ├── CONSTITUTION.md  # Optional: project constitution with nine articles
    ├── data-model.md    # Optional: data-model artifact (gated by sdd.data_model)
    ├── contracts/       # Optional: API contracts directory (gated by sdd.contracts)
    ├── quickstart.md    # Key validation scenarios derived from SPEC.md
    ├── FEEDBACK.md      # Optional: production feedback notes
    ├── JTBD.md          # Optional: Jobs-To-Be-Done analysis
    └── REPORT.md        # Optional audit/completion report
```

### 10.3 Spec Status Lifecycle

```mermaid
graph LR
    draft[draft] --> in_progress[in_progress]
    in_progress --> implemented[implemented]
    implemented --> in_review[in_review]
    in_review --> verified[verified]
    in_review --> draft
    verified --> archived[archived]
```

### 10.4 Spec Tools

| Tool | Purpose |
|------|---------|
| `spec_list` | List specifications |
| `spec_read` | Read a spec by ID |
| `spec_search` | Search specs by keyword |
| `spec_coverage` | Generate requirement coverage report |
| `spec_task_update` | Update a plan task status; mirrors the status into the session task tracker (creates the tracker task if none exists, updates it if it does) |

### 10.5 Slash Commands

| Command | Purpose |
|---------|---------|
| `/spec create <name> <title> [--from-research <name>] [--folder <path>]` | Create a new spec (SPEC.md + PLAN.md + TESTPLAN.md). `--folder` targets another project's `specs/` root (used by `/spec reverse --folder` chaining) |
| `/spec specify <name> <feature> [--from-research <name>]` | Generate SPEC.md only (EARS spec with `[NEEDS CLARIFICATION]` markers); optionally creates a git branch when `sdd.branch_per_spec` is enabled |
| `/spec plan <name> <tech-context>` | Generate or regenerate PLAN.md from existing SPEC.md using technology context as guidance |
| `/spec tasks <name>` | Generate TASKS.md (ordered task list from PLAN.md) and quickstart.md (validation scenarios from SPEC.md) |
| `/spec update <specname>` | Re-read `SPEC.md` and regenerate `PLAN.md` + `TESTPLAN.md` (preserves unchanged task IDs) |
| `/spec add <id> <requirement>` | Add an incremental requirement to an existing spec |
| `/spec feedback <spec-id> <note>` | Append a production feedback note to `FEEDBACK.md` |
| `/spec impl <id>` / `/spec implement <id>` | Transition spec to `in_progress` and generate plan |
| `/spec jtbd <specname> [--force] [--agent <name>]` | Perform JTBD analysis and write `JTBD.md` in the spec folder |
| `/spec list [--status <status>] [--prefix <prefix>]` | List specs with optional filtering |
| `/spec search <query>` | Search specs by keyword |
| `/spec show <id>` | Read a spec |
| `/spec validate [specname]` | Validate EARS compliance, clarification markers, and consistency (ambiguity, contradiction, gap detection) |
| `/spec status <id> [<new-status>]` | Show or transition spec status |
| `/spec task <id> [<task-id>] [<new-status>]` | List, show, or update task status |
| `/spec coverage <id>` | Generate requirement coverage report |
| `/spec activate <id>` | Activate a spec for context injection into agent prompts |
| `/spec deactivate` | Deactivate the currently active spec |
| `/spec delete <id> [--yes]` | Delete a spec directory |

### 10.6 Research Linkage

Specs can reference research outputs via the `--from-research` flag on
`/spec create` and `/spec specify`. When provided, the spec's `SPEC.md`
receives a `research:` key in its YAML frontmatter linking to the research
artifact, and a `## Related Research` section is injected into the body
summarising the linked research output.

### 10.7 SDD Configuration Flags

The following configuration flags in `ragent.json` under the `sdd` key control
opt-in Spec-Driven Development capabilities:

| Flag | Default | Purpose |
|------|---------|---------|
| `branch_per_spec` | `false` | Create a `spec/<name>` git branch during `/spec specify` |
| `data_model` | `false` | Generate `data-model.md` during `/spec plan` |
| `contracts` | `false` | Generate `contracts/` directory during `/spec plan` |
| `feedback_loop` | `false` | Surface `FEEDBACK.md` notes during `/spec plan` regeneration |

### 10.8 Consistency Validation

`/spec validate` performs three categories of consistency checks in addition
to EARS compliance:

- **Ambiguity detection** — flags vague terms (e.g. "fast", "efficient") and
  undefined references in requirement text.
- **Contradiction detection** — identifies conflicting requirements that
  specify opposing constraints.
- **Gap detection** — identifies requirements lacking acceptance criteria.

Consistency warnings are included in the validation report alongside EARS
errors and `[NEEDS CLARIFICATION]` marker warnings.

### 10.9 Clarification Markers

During `/spec specify`, the LLM is instructed to insert
`[NEEDS CLARIFICATION]` markers for ambiguous or underspecified requirements.
These markers are detected during validation and block the `approved` status
transition until resolved.

### 10.10 Production Feedback Loop

`/spec feedback <spec-id> <note>` appends advisory notes to
`specs/<spec-id>/FEEDBACK.md`. When the `sdd.feedback_loop` config flag is
enabled, these notes are automatically surfaced in the `/spec plan` prompt
so that production feedback (metrics, incidents, user reports) informs plan
regeneration.

### 10.10a Task Status Symbols and Coverage Report

Task statuses carry shared ASCII symbols from `TaskStatus::symbol()`:
`[wait]` pending, `[sync]` in-progress, `[ok]` completed, `[stop]` blocked.
The `/spec coverage <id>` command and the `spec_coverage` agent tool both
render the requirement-coverage report through a single shared
`Spec::coverage_report()` implementation (overall coverage percentage, a
per-requirement `[ok]`/`[  ]` line with linked-task completion counts, and a
per-task line with status symbol and linked requirement IDs), so the two
consumers can never drift apart.

### 10.10b Spec File Write Semantics

Spec files are written atomically (unique temp file, flush + sync, rename), and
auxiliary review files follow a clear-on-empty rule: when `REVIEW.md` or
`FEEDBACK.md` content becomes empty the file is deleted rather than written as
an empty placeholder. Frontmatter regeneration via `update_frontmatter`
preserves keys the lifecycle manager does not model (e.g. `research:`
linkage), quoting values that contain YAML-special characters.

### 10.10c Automatic Task Completion on Spec-Directory Writes

When an agent turn writes files inside the active spec's directory
(`specs/<id>/`) and the session has an active spec, tasks in the `in_progress`
state are automatically advanced to `completed` with a completion timestamp.
The heuristic is guarded by `writes_in_spec_dir`: only file-writing tools
(`Write`, `Edit`, `multiedit`, `patch`, `apply_patch`, `create`,
`append_to_file`) whose resolved target path sits inside the spec directory
count. Writes elsewhere in the workspace (docs, scratch files, snapshots)
never complete spec tasks, preventing unrelated edits from silently corrupting
`PLAN.md`.

### 10.10d Session Task Tracker Sync

`spec_task_update` calls are mirrored into the session task tracker: a
tracker task tagged with the same `spec_id`/`spec_task_id` metadata is
updated when it exists, or created (seeded from the PLAN.md task row,
status mapped from the spec task status) when it does not. The
`/spec impl` driver no longer pre-creates milestone tracker rows; the
tracker is populated lazily by the agent's `spec_task_update` calls.

### 10.11 Constitutional Amendment Process

When a spec directory contains a `CONSTITUTION.md`, amendments require
explicit rationale documentation, a backwards-compatibility assessment, and
a dated changelog entry within the file. The constitution parser validates
the nine-article structure and the amendment process.

---

# Part III: Data & Knowledge Systems (continued)

## 11. Research System

### 11.1 Overview

The `/research` slash command and `ragent research` CLI create structured
research items under `research/<name>/`. Each item contains captured sources
(web pages, local files, prior specs) and a single `RESEARCH.md` document.

### 11.2 Source Gathering

A research session runs in phases:

1. **Setup** — validate name and create the skeleton `RESEARCH.md`.
2. **Web** — issue `websearch` queries and fetch pages via `webfetch`. Every
   HTML page captured as a web source must have been extracted by the
   `readability-rs` crate (the research web-gather phase verifies the
   `extraction_method` signal reported by `mf_fetch`, or re-runs readability
   on the raw HTML when the legacy `webfetch` tool is used). Pages where
   readability extraction failed — and would only be available via the
   html2text / raw tag-strip fallbacks — are rejected with a
   `readability extraction failed …` fetch error instead of being accepted.
   `github.com/{owner}/{repo}/blob/…` file-view URLs are rewritten to their
   `raw.githubusercontent.com` counterparts before fetching, since the blob
   page is GitHub application chrome that readability cannot extract as an
   article. PDF and YouTube sources bypass readability entirely by design,
   as do non-HTML content types (e.g. `text/plain` served by
   `raw.githubusercontent.com`) — their bodies are the verbatim document
   content, so no fallback extraction is involved. YouTube
   sources are captured as the video's caption transcript: `mf_fetch`
   parses the watch page's `ytInitialPlayerResponse` (brace-balanced,
   string-aware JSON extraction), reads the caption tracks from
   `captions.playerCaptionsTracklistRenderer.captionTracks`, and fetches
   the default (or first English) track. A fetch that fails at the tool
   level (e.g. `mf_fetch` reporting `content_ok = false` or an `error`,
   such as "no caption tracks available for this YouTube video") is
   rejected as an explicit `FetchFailed` event carrying the real reason —
   placeholder error text never enters the research corpus as a source
   body.
3. **Local** — scan project files with `Glob`/`Grep`/`read`. Each captured
   file produces an excerpt showing the matching lines plus one line of
   context on either side, and a `relevance` note that names the matched
   keyword(s) and the first matching line.
4. **Specs** — cross-reference prior specs under `specs/`.
5. **Synthesize** — (TUI only, when an active model is configured) send the
   captured source bodies to the LLM and ask for Summary, Findings,
   In-Project Cross-References, and Open Questions. The session emits a
   `SynthesizeResult` event so the UI can distinguish LLM-synthesized output
   from the mechanical fallback.
6. **Assemble** — combine frontmatter, analysis, and References Index.
7. **Finalize** — mark the item `Complete` and refresh `research/INDEX.md`.

Supporting files under `research/<name>/sources/` (e.g. `web-01.md`,
`local-02.md`) contain the **actual** captured body — web pages render into
plain text, local files become keyword-anchored excerpts — so a reader can
audit the evidence without re-running the search.

### 11.2.1 Execution Modes and Per-Page Summarisation

The `--mode` flag selects the high-level research strategy while reusing
the same gathering and synthesis pipeline. Supported modes are:

| Mode | Description |
|------|-------------|
| `tiered` | Default single adversarial pipeline. |
| `supervisor` | Supervisor/researcher graph: a lead supervisor spawns parallel sub-topic researchers (FR-009). |
| `competitive` | Supervisor graph specialised for comparing a set of entities. Defaults to `--format comparison-table` unless an explicit `--format` is supplied (FR-014). |

`--max-concurrent-research-units N` caps the number of parallel
researcher agents in `supervisor` and `competitive` modes. The default is
5 and can be changed in `ragent.json` under `research.supervisor.max_concurrent_research_units`.

`--summarization-model <provider:model>` selects a lightweight model for
per-page webpage summaries. When omitted, the active default model is
used with a short-output limit. This is independent of the per-phase
overrides `--research-model`, `--compression-model`, and `--final-report-model` (FR-013).

`--no-clarify` (default) and `--clarify` control whether the pipeline
asks a single clarifying question when the topic looks ambiguous (FR-005,
FR-017). Clarification is disabled by default; `--clarify` opts in.
`--brief <TEXT>` supplies an explicit research brief used as
the mission statement for synthesis (FR-004).

`--evaluate` appends a deterministic self-evaluation scorecard
(quality, relevance, groundedness, completeness, structure) to the
assembled `RESEARCH.md` (FR-008, FR-015). It can also be enabled by
default via `research.evaluate.enabled: true`.
