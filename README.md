# ragent

An AI coding agent for the terminal, built in Rust.

ragent is a Rust coding agent inspired by RooCode, Claude Code, Copilot CLI and
OpenCode. It provides multi-provider LLM orchestration, a comprehensive built-in
tool system, a terminal UI, and a client/server architecture — all compiled into a
single statically-linked binary with zero runtime dependencies.

It is implemented in Rust as a learning exercise for the author.

Read TUI-QUICKSTART for instructions on how to use the tool.

## Features

- **Multi-provider LLM support** — Anthropic, OpenAI, Google Gemini, Hugging Face,
  GitHub Copilot, Ollama (local and cloud), Generic OpenAI-compatible endpoints,
  Azure AI Foundry, Azure Resource (File) provider, Amazon Bedrock, Microsoft Foundry Local,
  OpenRouter, and a Model Router cluster out of the box, with an extensible provider trait for adding more
- **Local-first defaults** — when no model is explicitly configured, ragent resolves
  to the first available local/self-hosted provider (e.g. Ollama) rather than
  hard-wiring a cloud provider
- **Comprehensive tool system** — 168 registered tools across 25 categories:
  - **File operations** — read, write, create, edit, multiedit, apply_patch, patch, rm, move, copy,
    mkdir, append, file_info, diff, glob, list
  - **Shell** — bash, bash_reset, open (7-layer security with safe-command whitelist,
    banned commands, denied patterns, directory escape prevention, syntax validation,
    obfuscation detection, and user allowlist/denylist)
    - **Bang commands** — prefix any prompt with `!` (e.g. `! ls -la`) to run a
      shell command directly; the output is sent to the model for review and
      error resolution
    - **Search** — grep
    - **Web** — webfetch, websearch, http_request
      - **Browser automation** — browser (Chrome DevTools Protocol: open, snapshot,
        click, type, fill_form, select, wait, eval, scroll, upload, press,
        screenshot, status, setup)
      - **Code intelligence** — codeindex_search, codeindex_symbols, codeindex_references,
        codeindex_dependencies, codeindex_status, codeindex_reindex, codeindex_explain,
        codeindex_path, codeindex_communities, codeindex_godnodes (read-only,
        hardwired always-allowed)
      - **Memory** — memory_read, memory_write, memory_replace, memory_store,
        memory_recall, memory_forget, memory_search, memory_migrate,
        conversation_search, session_search
      - **Teams** — 20 tools for team lifecycle, tasks, messaging, and coordination
      - **GitHub & GitLab** — 29 native VCS tools for issues, PRs/MRs, pipelines, CI/CD,
        and repository management
      - **Office & PDF** — office_read/write/info, libre_read/write/info, pdf_read, pdf_write
      - **Sub-agents** — new_agent, cancel_agent, list_agents, wait_agents, agent_complete
      - **Planning** — plan_enter, plan_exit
      - **MCP** — mcp_tool (McpToolWrapper) for external Model Context Protocol servers
      - **Task Management** — task_create, task_update, task_get, task_list
      - **Interactive** — question, think
      - **Utility** — calculator, get_env
      - **Stocks & currency** — stock_quote, stock_history, stock_fundamentals,
        stock_search, stock_options, currency_rate, currency_history. Free Yahoo
        Finance is the default; Alpha Vantage can be configured in `ragent.json`.
      - **Plotting** — plot_line, plot_scatter, plot_bar, plot_histogram,
        plot_pie, plot_heatmap render ASCII-art graphs (with ANSI colours)
        inline in the message window via `ratatui-plt`
      - **Code search & navigation** — codeindex_search, codeindex_symbols,
      codeindex_references, codeindex_dependencies, codeindex_status, codeindex_reindex
    - **MasterFetch** — mf_fetch, mf_search, mf_crawl, mf_cache_clear for web content
      extraction, search, and crawling; `mf_search` runs DuckDuckGo, Brave,
      OpenAlex (scholarly works), and Wikipedia (encyclopedia summaries) keyless
      backends in parallel, plus optional LangSearch / Tavily / Perplexity /
      Exa API-backed engines when configured
    - **Gmail & messaging** — gmail, send_channel_message for external notifications
  - **Terminal UI** — full-screen ratatui interface with provider setup dialog,
    slash-command autocomplete, agent cycling, streaming chat with markdown and syntax
    highlighting, step-numbered tool calls with pretty-printed JSON in the log panel,
    and a live permission countdown timer (120-second timeout with EXPIRED state)
  - **HTTP server** — axum-based REST + SSE API so any frontend can drive the agent
  - **Session management** — persistent conversation history stored in SQLite;
    list, resume, export, and import sessions
  - **Permission system** — multi-layered defense-in-depth with hardwired rules  (codeindex tools always allowed), configurable allow/deny/ask rules, 7-layer bash
  security, file-path guards, and YOLO mode for trusted environments
- **Agent presets** — general, coder, task, architect, ask, debug, code-review, and
  orchestrator agents with tailored system prompts
- **Custom agents** — user-defined agents via JSON (OASF format) or Markdown profiles
- **Project guidelines** — auto-loads `AGENTS.md` from the project root (and
  `~/.local/share/ragent/`) into the system prompt so agents follow project-specific
  conventions
- **MCP client** — Model Context Protocol support with auto-discovery of 9 known
  server types, stdio client, tool bridging, and TUI commands (`/mcp discover`,
  `/mcp list`, `/mcp call`)
- **Snapshot & undo** — file snapshots before edits so changes can be rolled back
- **Event bus** — internal tokio pub/sub for real-time UI updates across all components
- **Background agents** — spawn and run multiple sub-agents concurrently for parallel
  task execution, with REST API and TUI monitoring
- **Prompt inspector** — `/prompt` renders the assembled system prompt an agent
  would receive (primary or subagent mode, per-agent override, roster) with the
  effective tool surface — read-only, no LLM call
- **Tool-repeat guard** — after five consecutive identical tool calls the sixth is
  held for confirmation in interactive runs and auto-denied in unattended runs, so
  agent loops cannot hang replaying the same call (FR-044)
- **Code index** — automatic codebase indexing with tree-sitter parsing (15+ languages),
    full-text search via Tantivy, incremental updates via file watcher, and LLM-accessible
    tools; supports Rust, Python, TypeScript/JavaScript, Go, C/C++, Java, OpenSCAD,
    Terraform, CMake, Gradle, and Maven; enable/disable via `/codeindex on|off`,
    language filtering via `/codeindex lang <language>`; **semantic code graph** with
    `codeindex_godnodes` (top-N most-connected symbols), `codeindex_path` (shortest
    path between symbols), `codeindex_explain` (node metadata and edges), and
    `codeindex_communities` (community detection); graph built in the background
    via `/codeindex graph build` (phased lock discipline keeps search available
    during derivation; status-bar `idx`/`graph` busy indicators show progress)
- **Memory system** — three-tier system with file blocks, structured SQLite store,
  and optional embedding-based semantic search; automatic extraction, decay,
  compression, and knowledge graph support
- **Spec management** — `/spec` slash commands for creating, listing, searching,
  validating, and tracking specification lifecycles; `/spec jtbd` performs
  Jobs-To-Be-Done analysis on existing specs; `/spec update` regenerates
  `PLAN.md` and `TESTPLAN.md` from an edited `SPEC.md`; `/spec create` now
  also emits a manual `TESTPLAN.md` test-plan artifact
- **GitHub repo reverse-engineering** — `/reverse <owner/repo | URL>` fetches
  a public repo's metadata, root file tree, and README via the GitHub API,
  then asks the currently selected LLM model to generate a synthetic creation
  prompt; optional `--tech <stack>` constrains the technology stack and
  `--create <name>` chains into `/spec create` to auto-generate a spec from
  the reverse-engineered prompt
- **Project scaffolding** — `/new --language <lang> --type <type>` scaffolds a
  new project in an empty directory: the ragent workspace (`.ragent/`, `specs/`,
  `log/`, `.gitignore`, `AGENTS.md`), a runnable hello-world artifact set for
  26 application languages (rust, python, go, typescript, shell, ...) with
  library/cmdline/tui/gui layouts plus sample-document stubs for 20 data,
  markup, and build formats (json, yaml, sql, cmake, maven, ...) covering every
  codeindex scanner language, optional
  stack layers (`--stack axum`), starter docs (`README.md`, `QUICKSTART.md`,
  `STATS.md`, `docs/`), git init + initial commit, and optional GitHub/GitLab
  remote creation + push (`--github`/`--gitlab`); progress streams live in the
  message window; also available as the `ragent new` CLI subcommand
- **Research system** — `/research` slash command family and `ragent research` CLI for
  structured information gathering (web search + local file cross-referencing) with
  self-contained `RESEARCH.md` outputs and `GET/POST/DELETE /research` HTTP endpoints
- **Skills system** — loadable skill packs (bundled or custom YAML) that inject tools,
  prompts, and file context into agent sessions
- **Teams & Swarms** — multi-agent coordination with named teammates, shared task lists,
  mailbox messaging, and swarm decomposition for parallel work (`/swarm <prompt>`)
- **Autopilot mode** — autonomous operation with configurable iteration limits and
  permission auto-approval (`/autopilot on [--max-tokens N] [--max-time N]`)
- **Config error reporting** — actionable JSON parse diagnostics showing file path,
  line, column, problematic source line, and caret marker

## Installation

### From source

```bash
git clone https://github.com/thawkins/ragent.git
cd ragent
cargo build --release
# Binary is at target/release/ragent
```

Requires Rust 1.85+ (edition 2024).

## Quick Start

```bash
# Configure an API key
export ANTHROPIC_API_KEY="sk-..."
# or
export OPENAI_API_KEY="sk-..."
# or (for Azure AI Foundry)
export AZURE_AI_FOUNDRY_API_KEY="sk-..."
# or (for Generic OpenAI API provider)
export GENERIC_OPENAI_API_KEY="sk-..."
# or (for OpenRouter)
export OPENROUTER_API_KEY="sk-or-..."

# Launch the interactive TUI
ragent

# Run a one-shot prompt
ragent run "Explain this codebase"

# Start the HTTP server only
ragent serve --port 9100
```

Use OpenRouter models directly from the CLI:

```bash
ragent run --model openrouter/anthropic/claude-sonnet-4 "Refactor this function"
```

Generic OpenAI-compatible endpoint (including custom port) can be configured in
`ragent.json`:

```json
{
  "provider": {
    "generic_openai": {
      "env": ["GENERIC_OPENAI_API_KEY"],
      "api": { "base_url": "http://127.0.0.1:8080" }
    }
  }
}
```

## Usage

```
ragent [OPTIONS] [COMMAND]

Commands:
  run      Execute agent with a prompt
  serve    Start HTTP server only
  session  Manage sessions (list, resume, import, export)
  auth     Configure provider authentication
  models   List available models
  config   Show resolved configuration
  new      Scaffold a new project in the current directory

Options:
      --model <MODEL>          Override model (provider/model format)
      --agent <AGENT>          Override agent [default: coder]
      --log-level <LOG_LEVEL>  Log level [default: warn]
      --no-tui                 Disable TUI, use plain stdout
      --yes                    Auto-approve all permissions
      --config <CONFIG>        Path to config file
```

## Configuration

ragent reads configuration from `ragent.json` (or `ragent.jsonc`) in the `.ragent/`
directory, with fallback to `~/.config/ragent/config.json`. The format is compatible
with OpenCode's `opencode.json`.

```jsonc
{
  "provider": {
    "anthropic": {
      "thinking": { "enabled": true, "level": "low" },
      "models": {
        "claude-sonnet-4-20250514": {
          "thinking": { "enabled": true, "level": "high", "budget_tokens": 16000 }
        }
      }
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
      "compaction": {
        "auto": true,
        "threshold": 0.7,
        "buffer": 0.10,
        "keep": { "tokens": 0.20 },
        // Optional: route compaction to a fast/cheap model instead of the
        // session's primary model. Default: session model.
        // "model": { "provider_id": "ollama", "model_id": "qwen2.5:1.5b" },
        // Optional: cap summary output tokens (default 1500) and per-tool-output
        // truncation in the compaction prompt (default 2000).
        // "summary_tokens": 1500,
        // "tool_output_max_chars": 2000
      },
      "tool_visibility": {    "office": true,
    "github": true,
    "gitlab": true,
    "teams": true,
    "agents": true,
    "plan": true,
    "codeindex": true
  }
}
```

See the full configuration schema in [SPEC.md](SPEC.md).

## Custom Agents

You can define your own agents as JSON files using the
[Open Agentic Schema Framework (OASF)](https://oasf.agntcy.org/) standard.
Place them in:

- `~/.ragent/agents/` — user-global (all projects)
- `.ragent/agents/` — project-local (this project, higher priority)

ragent loads them automatically at startup. Use `/agents` to list loaded agents
and view diagnostics, or `/agent` to open the interactive picker (custom agents
are marked with a yellow `[custom]` badge).

See [docs/custom-agents.md](docs/custom-agents.md) for the full schema
reference, template variables (`{{WORKING_DIR}}`, `{{FILE_TREE}}`, `{{AGENTS_MD}}`,
`{{DATE}}`), permission rules, and worked examples. Ready-to-use example files
are in [`examples/agents/`](examples/agents/).

## Project Scaffolding

Scaffold a brand-new, agent-friendly project in an empty directory with one
command:

```bash
# TUI
/new --language rust --type cmdline

# CLI
ragent new --language rust --type cmdline
```

The command validates the directory is empty, generates the ragent workspace
(`.ragent/`, `specs/`, `log/`, `.gitignore`, `AGENTS.md`), a runnable
hello-world artifact set for `rust`/`python`/`go`/`typescript` in
`library`/`cmdline`/`tui`/`gui` layouts, starter documentation (`README.md`,
`QUICKSTART.md`, `STATS.md`, `docs/`), initialises git with an initial
commit, and — with `--github` or `--gitlab` — creates a private hosting
repository, sets it as `origin`, and pushes.

```bash
# Add a framework stack (Rust: axum, warp, raylib, gtk4, ratatui)
ragent new --language rust --type cmdline --stack axum

# Create the project on GitHub or GitLab (mutually exclusive)
ragent new --language python --type library --gitlab
```

See [`docs/howtos/newproj.md`](docs/howtos/newproj.md) for the full manual.

## Teams

Teams let one lead session coordinate multiple teammates with shared tasks and
mailbox messaging.

Quick flow:

- Create a team: `/team create <name>` (or `team_create`)
- Re-open an existing team: `/team open <name>`
- Spawn teammates: `team_spawn`
- Add/list/claim/complete tasks: `team_task_create`, `team_task_list`,
  `team_task_claim`, `team_task_complete`
- Communicate: `/team message ...` or `team_message`, plus `team_read_messages`
- Reset/close/delete team state: `/team clear`, `/team close`, `/team delete <name>`
- Cleanup when finished: `/team cleanup` or `team_cleanup`

Docs and examples:

- Guide: [`docs/userdocs/TEAMS.md`](docs/userdocs/TEAMS.md)
- How-to manual: [`docs/howtos/teams.md`](docs/howtos/teams.md)
- Example bundles: [`examples/teams/`](examples/teams/)

## Architecture

The project is a Cargo workspace built from 16 focused crates:

| Crate                     | Purpose                                                                                                                                                                                           |
| ------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `ragent-agent`          | Agent/runtime layer: sessions, orchestration, MCP, memory, tool registry                                                                                                                          |
| `ragent-bench`          | Benchmark runner shared between TUI and CLI                                                                                                                                                       |
| `ragent-codeindex`      | Codebase indexing: tree-sitter parsing, SQLite store, Tantivy FTS, file watcher                                                                                                                   |
| `ragent-config`         | Configuration types, defaults, and parsing                                                                                                                                                        |
| `ragent-llm`            | Provider clients and model/provider registry (Anthropic, OpenAI, Gemini, Ollama, HuggingFace, Copilot, Generic OpenAI, Azure AI Foundry, Azure Resource, Amazon Bedrock, Microsoft Foundry Local) |
| `ragent-research`       | Research system: web/local gathering, synthesis, RESEARCH.md output                                                                                                                               |
| `ragent-server`         | Axum HTTP routes and SSE streaming                                                                                                                                                                |
| `ragent-specs`          | Spec lifecycle management: discovery, validation, status transitions, review, archival, JTBD analysis                                                                                          |
| `ragent-storage`        | SQLite-backed storage, snapshots, encrypted credentials                                                                                                                                           |
| `ragent-team`           | Team coordination runtime and team tools                                                                                                                                                          |
| `ragent-telemetry`      | OpenTelemetry instrumentation and OTLP export                                                                                                                                                     |
| `ragent-tools-core`     | Core shell/file/search tools                                                                                                                                                                      |
| `ragent-tools-extended` | Extended document/web/memory/codeindex/plot tools                                                                                                                                                      |
| `ragent-tools-vcs`      | GitHub and GitLab tool surface                                                                                                                                                                    |
| `ragent-tui`            | Ratatui terminal interface                                                                                                                                                                        |
| `ragent-types`          | Shared IDs, events, messages, and sanitization primitives                                                                                                                                         |

The binary entry point (`src/main.rs`) wires these crates together behind a clap CLI.

```
User Input
    │
    ▼
┌──────────┐    ┌──────────────┐    ┌──────────────┐
│   TUI    │◄──►│  Event Bus   │◄──►│ HTTP Server  │
└────┬─────┘    └──────┬───────┘    └──────┬───────┘
     │                 │                   │
     ▼                 ▼                   ▼
┌─────────────────────────────────────────────┐
│              Session Processor              │
│  (agent loop → LLM call → tool execution)  │
└──────────────────┬──────────────────────────┘
                   │
         ┌─────────┼─────────┐
         ▼         ▼         ▼
    ┌─────────┐ ┌──────┐ ┌────────┐
    │Provider │ │Tools │ │Storage │
    │(LLM API)│ │      │ │(SQLite)│
    └─────────┘ └──────┘ └────────┘
```

## Performance

Criterion benchmarks currently ship with `ragent-tui`, `ragent-server`, and
`ragent-codeindex`. See [`docs/performance/benchmark-guide.md`](docs/performance/benchmark-guide.md)
for full instructions.

```bash
# Run crate benchmarks
cargo bench -p ragent-tui
cargo bench -p ragent-server
cargo bench -p ragent-codeindex
```

Key optimisations in the current release:

- **DashMap** replaces `RwLock<HashMap>` in the orchestrator, reducing lock contention
- **LRU file-read cache** (256-entry, mtime-keyed) avoids redundant disk I/O
- **Rayon parallel glob** walk for large directory trees
- **Incremental snapshots** store only changed files (via `similar` diffs)
- **Async storage writes** via `tokio::task::spawn_blocking` keep the executor free

## Project Status

**v1.0.96** — The core architecture, tool system (168 tools across 25 categories), TUI,
HTTP server, memory system, teams/swarm coordination, spec management, skills system,
research system, and multi-layered security are functional and under active development.

Recent highlights:

- **`/prompt` inspector, tool-repeat guard, redundant command removal
  (v1.0.96)** — the new read-only `/prompt` slash command
  renders the assembled system prompt an agent would receive (primary or
  subagent mode, roster, per-agent override) with its effective tool surface;
  a tool-repeat guard (FR-044) blocks agent loops stuck replaying the same
  tool call (sixth identical call prompts in interactive runs, auto-denies in
  unattended runs); and the redundant `/opt` (plus its `ragent-prompt_opt`
  crate and `POST /opt` endpoint), `/tasks`, and `/theme` slash commands were
  removed, returning the workspace to 16 crates.

- **AgentNotice chat-bubble separation + /toolchain fixed-width table
  (v1.0.94)** — consecutive `AgentNotice` notices now render as their own
  yellow bubbles (the TUI event handler forces a new assistant message
  before and after each notice), and the `/toolchain list` table switched
  to fixed 10/10/10/50 character columns (Language/Runtime clip, Status
  and Version word-wrap onto continuation grid lines) at a constant
  93-column width. Released in v1.0.95 with the docupdate pass.

- **Spec-impl tracker semantics + sub-agent completion hard requirement +
  compaction performance pass (v1.0.93)** — `/spec impl` no longer
  pre-creates session tracker tasks (`spec_task_update` now
  creates-or-updates them, seeded from the spec's `PLAN.md`), the
  sub-agent completion protocol is a mandatory hard requirement enforced in
  the system prompt, the `agent_complete` tool description, and the
  mid-run summary nudge, and compaction gained a `compaction.model`
  fast/cheap summariser override for `/compact`, configurable
  `summary_tokens` (1500) / `tool_output_max_chars` (2000), an adaptive
  prompt cap, 180 s / 60 s stream caps with chunk-level cancellation, and an
  allocation-free token estimator.

- **Subagent interactive-tool blocking (v1.0.92)** — sub-agent runs now deny
  `ask_user` with a corrective observation instead of stalling the run on an
  unbounded user-answer wait; a shared `denied_tool_call` helper covers all
  tool-call denial paths.

- **Scaffold + status-bar simplify pass (v1.0.90)** — the `/new` TUI command
  and the `ragent new` CLI subcommand now share one `plan_and_emit()`
  pipeline in the `project_scaffold` engine module (each surface attaches
  only its own git/hosting outcome lines), the GitHub/GitLab remote flows
  share a `register_origin_and_push` helper, and the status-bar last-prompt
  renderer is single-pass. No behaviour change; ~135 duplicated lines
  removed across the scaffolder and status bar.

- **Research-crate simplify pass (v1.0.88)** — a `/simplify` audit over
  all `ragent-research` sources fixed a `parse_subject_summary` slice panic
  (`}`-before-`{` responses no longer panic the analysis merge path), made
  `AnalysisEngine::with_brief` a required trait method, cached hot regexes
  behind `OnceLock` statics, deduplicated the two `RESEARCH.md` layout
  assemblers into a shared section-emitter module, unified search-engine
  ordering across gather paths, and consolidated per-page media-type
  classification and pdf/youtube tallies into single passes.

- **UI polish and tool-calling fixes (v1.0.87)** — the status bar's top line
  now renders the last submitted prompt as a centred bracketed tag (first 32
  characters, `....` when truncated) between the git branch and the session
  status, with the working directory shortened to fit and a graceful fallback
  to the previous layout when the terminal is too narrow; the message window
  no longer pushes the transcript down with a leading blank line before the
  first "You:" prompt; and the `text_toolcalls` recovery helpers were tightened
  to `pub(crate)` visibility.

- **Spec-system documentation and semantics polish (v1.0.86)** — the
  `/spec` how-to manual now documents the spec file write semantics (atomic
  temp-file + rename writes, clear-on-empty `REVIEW.md`/`FEEDBACK.md`), the
  `/spec coverage` report format (shared `Spec::coverage_report()` renderer
  with `[ok]`/`[wait]`/`[sync]`/`[stop]` task-status symbols), and the
  automatic task-completion heuristic guarded by `writes_in_spec_dir`
  (writes outside the active spec directory never complete spec tasks). The
  master spec gained sections 10.10a–10.10c covering the same semantics.

- **Tool-calling audit remediation (shipped in v1.0.85)** — fixes every
  HIGH/MED finding from the tool-calling + UTF-8 audit across provider stream
  parsers, tool dispatch, and the edit-tool family: OpenAI Responses API tool
  calls no longer silently dropped (missing `ToolCallStart`), Gemini
  final-chunk `functionCall` parsed before the finishReason flush, malformed
  tool arguments fail fast with a corrective LLM-visible error instead of
  silently executing with `{}`, loop restrictions fail closed, schema
  validation of required args before execution, panicked tool tasks synthesise
  error results (no orphaned `tool_use`), object-form `arguments` accepted from
  llama.cpp/vLLM-style servers, Anthropic/Azure parallel `tool_use` blocks keyed
  by SSE block index, HuggingFace tool-incapable models no longer receive a
  tools array, CRLF files keep their line endings through edits, BOM round-trip
  fixed, and a conservative text-format tool-call recovery extractor for models
  that narrate tool calls as prose.

- **Goal-driven loop programming (`/loop`)** — a goal-driven agentic loop that
  runs a `LoopSpec` to a stop condition (goal achieved, verification passed,
  budget exhausted, or interrupted), with a verification gate for verify
  commands, restriction layers (tool set, read-only globs, scope globs),
  pre-loop snapshot capture, and a post-loop rollback flow (Enter restores the
  pre-loop snapshot, Esc keeps changes). The one-shot form
  `/loop <agent> [flags] <goal>` accepts `--max-steps N`, `--cost_limit N`,
  and `--timeout N` overrides in any position. Configured via the `loop`
  section of `ragent.json` (`loop.max_steps` 512 default, `loop.cost_limit`,
  `loop.error_retry_allowance`, `loop.checkpoints`,
  `loop.checkpoint_timeout_secs`); see `docs/howtos/loopprogramming.md`
  (shipped in v1.0.82)
- **Agents panel reconciliation** — with many concurrent sub-agents the TUI
  Agents button previously could stay disabled with a zero count: the
  reconcile poll now also fires periodically (every 1.5 s) and merges the
  authoritative `AgentManager::tasks_snapshot`, so the panel self-heals
  regardless of broadcast-lag event loss (shipped in v1.0.82)
- **How-to documentation set** — three new how-to manuals in `docs/howtos/`:
  `reactagent.md` (the core per-turn ReACT loop), `loopprogramming.md`
  (goal-driven loops with `/loop`), and `office.md` (office + PDF tool
  families with format matrix, JSON examples, and configuration)

- **Code index status improvements** — the `codeindex_status` tool never
  blocks on the store lock: when a background reindex or graph build holds
  the mutex it returns an immediate busy report built from lock-free progress
  atomics; `IndexStats` now carries `graph_total_edges`/`graph_nodes`/
  `graph_communities`; and `codeindex_path`/`codeindex_explain` resolve
  symbols with exact-match + definition-kind ranking so impl/trait names no
  longer shadow the real definitions
- **Config save/load cache coherence (M-025)** — every `Config::save()`
  invalidates the on-disk load cache (`Config::invalidate_load_cache()`), and
  the cache key is snapshotted after the auto-creating first load, so TUI
  config writes (`/codeindex off`, `/tools`, ...) are immediately visible to
  subsequent loads in the same process — this fixes the v1.0.80 CI failure
  (run 34023696563)

- **`plot_*` tool family** — six scientific/terminal plotting tools
  (`plot_line`, `plot_scatter`, `plot_bar`, `plot_histogram`, `plot_pie`,
  `plot_heatmap`) render ASCII-art graphs — with real ANSI colours — inline in
  the TUI message window via `ratatui-plt`; GPL-3.0 dependency explicitly
  accepted and allow-listed in `deny.toml` (v1.0.80)

- **Research web-search quota controls** — `--max-search-calls N` places a hard,
  run-scoped cap on total web-search calls per research run, shared via `Arc` across
  every supervisor/competitive researcher and gather pass; a run-scoped query cache
  memoises identical sub-queries so parallel researchers reuse cached hits instead of
  re-issuing paid calls (v1.0.79)
- **`--depth` bounds web volume by default** — the effective web-source budget is
  derived from the selected depth (shallow 6 / standard 9 / deep 15) unless
  `--max-web-results` is passed explicitly, so `--depth shallow` now actually limits
  search/fetch volume (v1.0.79)
- **`/research update <name>` invocation replay** — re-runs a research item by
  replaying its recorded invocation line, overwriting `RESEARCH.md` and associated
  files with freshly gathered results; available via CLI, TUI, and HTTP `PUT /research/{name}`
  (v1.0.79)
- **`--mode competitive` defaults `--format` to `comparison-table`** — competitive
  runs no longer require an explicit `--format comparison-table` (v1.0.79)
- **GitHub `blob/` URLs no longer fail research gathering** — file-view URLs are
  rewritten to `raw.githubusercontent.com` and non-HTML content bypasses the
  readability gate, eliminating guaranteed `readability extraction failed` rejections
  (v1.0.79)
- **`/clip` slash command** — copies the rendered message-window transcript to the
  system clipboard in one step (v1.0.79)
- **`/research list` renders a human-readable table again** — the fixed-width
  `NAME/TITLE/STATUS/CREATED/MODIFIED` table is restored as the default output, with
  JSON behind the `--json` flag (v1.0.79)
- **Research evaluation scorecard configuration** — new `research.evaluate` section
  in `ragent.json` controls the self-evaluation scorecard appended to research
  reports (FR-015 of specs/opendeepresearch)
- **Clippy `for_kv_map` fix** — Ollama provider iteration now uses `values()`
  instead of destructuring a key-value pair; the LangSearch merge test
  expectation was also corrected (v1.0.74)
- **Sub-agent / teammate step visibility** — TUI step log now shows tool calls
  from tracked sub-agents and teammates with an `[agent-tag]` prefix; rebuilt
  step tags for lagged event-bus bursts prevent undercounting (v1.0.73)
- **Tool-permit handling fix** — `SessionProcessor` now surfaces an explicit
  error when the per-tool resource permit cannot be acquired (v1.0.73)
- **Token counting fixes** — TUI context panel percentages now use a consistent
  bytes-to-tokens conversion so they align with the status-bar usage figure
  (v1.0.72)
- **Context side panel** — toggleable `Alt+C` panel showing live, quantified
  context-window occupancy (v1.0.71)
- **Research clustering** — `/research cluster` concept extraction and a new
  `cluster.rs` payload builder (v1.0.71)
- **One-shot agent runner** — lightweight, provider-agnostic single-prompt
  execution path without spinning up a full agent loop (v1.0.71)
- **CPU-spin fixes** — Gemini SSE parser infinite loop fixed (`continue` to
  `break`), timed-out bash children now killed via `kill_on_drop`, and the
  TUI idle redraw interval raised from 250 ms to 2 s with dirty-flag gating
  (v1.0.60)
- **Performance and resource bounding** — removed `gh auth token` subprocess
  auto-discovery, replaced background-task polling with `tokio::sync::Notify`,
  code-index worker busy-spin with `recv_timeout`, capped TUI messages (500),
  log entries (1000), LLM request stats (1000), read-timestamp map (2000),
  and added WAL auto-checkpointing (v1.0.59)
- **Activity log system** — new append-only JSONL event store with
  `RunId`/`EventId` identifiers, projection types, rollback/resume support,
  and consistency validation (v1.0.59)
- **Storage schema-version fast path** — warm starts skip the full migration
  batch, reducing `Storage::open` from ~41 SQL round-trips to a single
  `CREATE TABLE` + `SELECT` (v1.0.59)
- **Research tooling overhaul** — New `polarity.rs`, `run_request.rs`,
  `contradiction.rs`, `corpus_critic.rs`, and `reconcile.rs` modules; unified
  `ResearchRunRequest` builder shared across CLI/TUI/HTTP; tier routing with
  `--tier` flag; IMRAD output format; new HTTP research routes with SSE events
  and 202+Location async behaviour; 37 new tests (v1.0.58)
- **Code index semantic graph** — Four new graph analysis tools
  (`codeindex_godnodes`, `codeindex_path`, `codeindex_explain`,
  `codeindex_communities`) with community detection, shortest-path traversal,
  and god-node identification; `/codeindex graph build` sub-command
- **Threaded codeindex graph build + status indicators** — graph builds run on
  a dedicated OS thread via `spawn_graph_build` with a phased lock discipline
  (brief snapshot, lock-free derivation, brief persist), so FTS search stays
  available during derivation; the TUI status bar shows `idx`/`graph` busy
  tags while a reindex or graph build runs (uncommitted, v1.0.80 cycle)
- **Bang commands** — Prefix any prompt with `!` to run a shell command and
  have the model review its output (v1.0.42)
- **Compaction fix** — Fixed compaction getting stuck when all messages fit
  the keep budget; now forces at least one message into the head
- **Code quality** — Extracted shared helpers for markdown rendering and
  agent dispatch, eliminated unnecessary clones, cleaned up noise comments
- **CI fixes** — Resolved clippy `needless_raw_string_hashes`, `vec_init_then_push`,
  and `useless_borrows_in_formatting` warnings

See [CHANGELOG.md](CHANGELOG.md) for the full history.

## License

MIT
