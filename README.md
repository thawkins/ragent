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
- **Comprehensive tool system** — 169 registered tools across 25 categories:
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
      - **Utility** — calculator, get_env, ragent_info (reports the running
        version, build time, git commit, and compiler)
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
    a live permission countdown timer (120-second timeout with EXPIRED state), and a
    message input queue (`Alt+Q` menu / `/queue`) that keeps the input field editable
    while the agent executes
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
  server types, stdio client, and tool bridging; TUI commands (`/mcp` /
  `/mcp status`, `/mcp discover`, `/mcp connect <id>`, `/mcp disconnect <id>`) with
  durable global enable/disable state (`mcp_state.json`) and live connect/disconnect
- **Snapshot & undo** — file snapshots before edits so changes can be rolled back
- **Event bus** — internal tokio pub/sub for real-time UI updates across all components
- **Background agents** — spawn and run multiple sub-agents concurrently for parallel
  task execution, with REST API and TUI monitoring; `/spawn <agent> <prompt>`
  launches a **detached** fire-and-forget sub-agent that nothing ever waits on
  (no `list_agents` entry, not awaitable via `wait_agents`, result never
  injected into the chat); every completed sub-agent run (detached or not)
  also writes its FULL output to `log/subagents/<task-id>.md`, and the
  completion event carries the real loop `finish_reason` (`stop` /
  `truncation` / `length` / `cancelled` / `error`) so a provider-side cut is
  flagged in the Agents panel instead of looking like a healthy finish
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
- **GitHub repo reverse-engineering** — `/spec reverse <owner/repo | URL>` fetches
  a public repo's metadata, root file tree, and README via the GitHub API,
  then asks the currently selected LLM model to generate a synthetic creation
  prompt; the `/new` scaffold flags (`--language <lang> --type <type>
  [--stack <name>]`) steer the prompt towards a target language, app type, and
  framework stack, and with those flags `--folder <path>` scaffolds a real
  project (default: the current directory) while `--github` / `--gitlab`
  create a private remote and push it; `--create <name>` chains into
  `/spec create` to auto-generate a spec from the reverse-engineered prompt,
  written under the scaffolded project (`<folder>/specs/<name>/`) when
  `--folder` was used
- **Project scaffolding** — `/new --language <lang> --type <type>` scaffolds a
  new project in an empty directory: the ragent workspace (`.ragent/`, `specs/`,
  `log/`, `.gitignore`, `AGENTS.md`), a runnable hello-world artifact set for
  26 application languages (rust, python, go, typescript, shell, ...) with
  library/cmdline/tui/gui layouts (plus a webapp HTTP server for the five web
  languages) plus sample-document stubs for 20 data,
  markup, and build formats (json, yaml, sql, cmake, maven, ...) covering every
  codeindex scanner language, optional
  stack layers (`--stack axum`), starter docs (`README.md`, `QUICKSTART.md`,
  `STATS.md`, `docs/`), git init + initial commit, and optional GitHub/GitLab
  remote creation + push (`--github`/`--gitlab`); progress streams live in the
  message window; also available as the `ragent new` CLI subcommand
- **Research system** — `/research` slash command family and `ragent research` CLI for
  structured information gathering (web search + local file cross-referencing) with
  self-contained `RESEARCH.md` outputs and `GET/POST/DELETE /research` HTTP endpoints;
  concept/finding output limits (`--max-concepts`/`--max-findings`, `research.max_concepts`/
  `research.max_findings`), scholarly-engine exclusion (`--no-papers`, alias
  `--no-scholarly`, `research.exclude_academic_engines`), open-access toggles
  (`--oa-enable`/`--no-oa`), URL cloaking (`--url-cloak`) that defangs web
  source URLs in the `Sources` bullets and `References Index` so scanners do
  not flag them, a per-engine progress table that breaks exclusions
  and fetch failures out by reason/cause, and `mf_search` `exclude_engines`
- **Skills system** — loadable skill packs (bundled or custom YAML) that inject tools,
  prompts, and file context into agent sessions
- **Plugin system** — load Codex-dialect (`codex-plugin.json` or the nested
  `.codex-plugin/plugin.json` the `openai/plugins` store ships) and Claude
  Code/Desktop-dialect (`.claude-plugin/plugin.json`) plugins — including
  multi-target trees that ship both nested manifests side by side (e.g.
  `mongodb/agent-skills`), which resolve to the Claude dialect — run their
  JavaScript entries on an embedded, budget-sandboxed engine (`rquickjs`, no
  Node/Deno required) behind a versioned `ragent` host API, and register their
  tools (`plugin_<id>_<tool>`) and slash commands; skill-only/MCP-only plugins
  (no JavaScript entry) install and load inertly, and their `skills` directories
  and `mcpServers` entries are bridged into the session (plugin skills join the
  skill-discovery roots; plugin MCP servers connect as `<plugin-id>.<server>`);
  a plugin's slash commands are bridged too, including the Claude `commands/*.md`
  prompt commands (each injected as a user turn with `$ARGUMENTS` substituted),
  registered under their bare name or the namespaced `plugin:<id>:<name>` trigger
  and listed in the `/` menu and `/help`; plugin `agents/*.md` profiles join
  agent discovery (YAML frontmatter accepted, project agents win name clashes),
  and plugin-declared `hooks` fire on the matching session lifecycle events
  (both `pre_tool_use` and Claude's `PreToolUse` spellings accepted; the
  `hooks.json` file and the group `{matcher, hooks: [...]}` shape are read, a
  plugin hook gets `CLAUDE_PLUGIN_ROOT` and the Claude event JSON on stdin, and
  a blocking Stop hook can feed findings back before the turn ends);
  `/plugins codex` and `/plugins claude`
  browse each store's official marketplace (`StoreProvider` normalises the Codex and
  Claude document shapes), and `/plugins add` also accepts a
  `git+<https-url>#<ref>[:<subpath>]` git source; a freshly installed plugin is
  recorded enabled and loads at the next session start (no JavaScript runs during
  the install); managed through
  `/plugins list|add|remove|enable|disable|test|stores|help` in the TUI and the
  `ragent plugins <sub>` CLI; `plugins.enabled: false` makes the subsystem inert
- **Input queue** — the TUI input field stays editable while the primary agent
  executes: each `Enter` appends the message to a bounded FIFO queue (default 32
  entries, configurable via `input_queue_capacity`), a two-digit counter appears
  before the prompt, and the oldest entry runs at each turn boundary; control it
  with the `Alt+Q` menu (`Next` / `Stop` / `Clear` / `Show`, navigated with
  `Up`/`Down`/`Enter`) or `/queue [list|clear|next|help]`; the `Show` row opens a
  scrollable panel of the queued entries where `Enter` moves the highlighted entry
  one step toward the front and `Del` removes it
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
  plugins  Manage plugins (the `/plugins` slash-command parity surface)

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
  },
  // Plugin subsystem (defaults shown). `enabled: false` makes the whole
  // subsystem inert (no discovery or loading).
  "plugins": {
    "enabled": true,
    "max_execution_ms": 5000,
    "max_entry_ms": 10000,
    "max_memory_mb": 64
  }
}
```

See the full configuration schema in [SPEC.md](SPEC.md).

## Custom Agents

You can define your own agents as JSON files using the
[Open Agentic Schema Framework (OASF)](https://oasf.agntcy.org/) standard.
Place them in:

- `~/.ragent/agents/` — user-global (all projects)
- `~/.config/ragent/agents/` — user-global (all projects; XDG config location,
  takes priority over `~/.ragent/agents/`)
- `.ragent/agents/` — project-local (this project, highest priority)

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
`library`/`cmdline`/`tui`/`gui`/`webapp` layouts, starter documentation (`README.md`,
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

The project is a Cargo workspace built from 17 focused crates:

| Crate                     | Purpose                                                                                                                                                                                           |
| ------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `ragent-agent`          | Agent/runtime layer: sessions, orchestration, MCP, memory, tool registry                                                                                                                          |
| `ragent-bench`          | Benchmark runner shared between TUI and CLI                                                                                                                                                       |
| `ragent-codeindex`      | Codebase indexing: tree-sitter parsing, SQLite store, Tantivy FTS, file watcher                                                                                                                   |
| `ragent-config`         | Configuration types, defaults, and parsing                                                                                                                                                        |
| `ragent-llm`            | Provider clients and model/provider registry (Anthropic, OpenAI, Gemini, Ollama, HuggingFace, Copilot, Generic OpenAI, Azure AI Foundry, Azure Resource, Amazon Bedrock, Microsoft Foundry Local) |
| `ragent-plugins`        | Plugin system: Codex/Claude dialect manifests, sandboxed JS runtime, lifecycle, `/plugins` surface                                                                                                |
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

**v1.0.118** — The core architecture, tool system (169 tools across 25 categories), TUI,
HTTP server, memory system, teams/swarm coordination, spec management, skills system,
research system, plugin system, and multi-layered security are functional and under
active development.

Recent highlights:

- **Durable MCP server enable/disable + plugin-bridged servers (v1.0.118)** —
  whether an MCP server is started is now a persisted choice
  (`<global state dir>/mcp_state.json`) rather than an implicit effect of being
  listed in `ragent.json`; a server absent from the ledger is enabled, and
  `mcp.<id>.disabled: true` always wins. `/mcp connect <id>` / `/mcp disconnect
  <id>` enable/disable a server live and the choice survives a restart; `/mcp`
  lists plugin-contributed servers with their live `enabled` state and tool
  count, and `/plugins list` gains `MCP` / `MCP Tools` columns. Plugin
  `mcpServers` entries are bridged as `<plugin-id>.<server>` by default. `/tools`
  now also lists the tools a visibility switch has hidden.
- **`/spec reverse --folder` scaffolds and hosts the target project (v1.0.117)** —
  `/spec reverse` now accepts `--folder <path>` plus `--github` / `--gitlab`
  (with the `/new` scaffold flags present) and creates the project in the target
  folder before synthesising the prompt; a chained `--create <name>` writes the
  spec into `<folder>/specs/<name>/`. Usage errors now state the specific cause
  instead of the bare usage line.
- **`webapp` app type (v1.0.117)** — `--type webapp` is a first-class registered
  value for `/new`, `/spec reverse`, and `/spec govcreate`; it generates a tiny
  dependency-free HTTP-server starter for `rust`, `python`, `go`, `typescript`,
  and `javascript`, and degrades to a manifest-only layout elsewhere.
- **`lopdf` joins the lint suite (v1.0.117)** — the vendored `lopdf` crate
  carries crate-level allowances (matching `vendor/pdf-extract`) so the dead-code
  lint and `cargo-machete` are green. Full CI hygiene (`cargo check`, the
  dead-code lint and reason checks, `clippy -D warnings`, `cargo fmt --check`,
  `cargo audit`, `cargo deny check`) and the entire `cargo test --workspace`
  suite are green.

- **Detached sub-agents + `/spawn` (v1.0.115)** — `/spawn <agent> <prompt...>`
  launches a sub-agent directly from the chat input as a **detached**
  fire-and-forget task: it runs concurrently and shows in the Agents panel, but
  `list_agents`/`wait_agents` never see it and its result is never injected back
  into the chat. Every completed sub-agent run (detached or not) now persists
  its FULL output to `log/subagents/<task-id>.md`, and the completion event
  carries the real loop `finish_reason` (`stop` / `truncation` / `length` /
  `cancelled` / `error`), so a provider-side cut is flagged in the Agents panel
  instead of looking like a healthy finish. The `new_agent` tool gained the
  matching optional `detached: true` parameter. Spec: `specs/spawnagent/`.

- **Plugin bridges (v1.0.114)** — plugin bridges for skills, MCP servers,
  slash commands, agents, and hooks (including the full Claude `hooks.json`
  declaration surface, a `.codex-plugin/plugin.json` recogniser, and the
  both-nested multi-target descriptor fix); the GitHub credential chain now has
  a shared `ragent_config::github` resolver with a `gh` CLI fallback and an
  app-token downgrade, so `/new --github` creates a hosting repository even when
  `/github login` stored a `ghu_` token; the input field queues slash commands
  while the agent runs; and a new read-only `ragent_info` tool reports the
  running version, build time, git commit, and compiler (169 tools).

- **Store providers + plugin-store installs** — `/plugins codex` and `/plugins claude`
  now browse the stores' own official marketplaces (`openai/plugins` and
  `anthropics/claude-plugins-official`). A `StoreProvider` trait with per-store Codex and
  Claude implementations normalises each vendor document shape (`name`-as-id, optional
  `version`, `local`/`url`/`git-subdir`/repo-relative sources) into the internal model, and
  the store kind is threaded through the fetch seam so every fetch parses with the right
  provider. A repo-relative source resolves against the origin's GitHub repository as an
  installable `git+<repo>#<ref>:<path>` source, and the install pipeline accepts a
  `git+<https-url>#<ref>[:<subpath>]` source via a shallow, sparse `git clone`, so a plugin
  hosted in a subdirectory of a large repository installs without downloading the whole
  tree.

- **Message input queue (v1.0.113)** — the TUI input field
  stays editable while the primary agent executes: each `Enter` queues the
  message in a bounded FIFO (default 32, `input_queue_capacity` config, clamped
  `1..=99`) with a two-digit counter before the prompt, and the oldest entry runs
  at each turn boundary. An `Alt+Q` queue-control menu (`Next` / `Stop`/`Resume`
  / `Clear` / `Show`, navigated with `Up`/`Down`/`Enter`) and a
  `/queue [list|clear|next|help]` slash command expose the same queue; a
  `Yes`/`No` dialog (default `No`) guards clearing, and the `Show` row opens a
  scrollable queue-entry panel where `Enter` moves an entry one step toward the
  front and `Del` removes it. A slash command (`/…`) is queued the same way while
  a turn runs (FR-017 amendment) and runs at the next turn boundary; only bang
  commands (`!…`) and teammate-targeted sends keep the busy refusal.
  Spec `inputqueue` complete (T-001..T-027), 19 new test files (234 new test
  attributes). This release also lands the full-workspace clippy hygiene pass:
  the non-existent `clippy::assert_is_empty` allow is removed from 244 files
  and the `--all-targets` clippy gate is now clean.

- **Plugin system (v1.0.112)** — the **plugin system** goes from spec
  draft to full implementation: the new `ragent-plugins` crate (234 tests)
  discovers and normalises Codex- and Claude Code/Desktop-dialect manifests,
  runs plugin JavaScript on a budgeted embedded `rquickjs` engine behind the
  versioned `ragent` host API, registers `plugin_<id>_<tool>` tools and slash
  commands, and drives the `/plugins` six-subcommand family plus the
  `ragent plugins` CLI parity surface. A `plugins` config block
  (`enabled`, budget limits, `store_dir`, per-plugin permissions) with
  overlay-wins merge, eight acceptance/fixture plugins under
  `assets/plugins/fixtures/`, and a code-quality cleanup pass (shared
  `ScratchSurface`/`store_and_config`, single `help::attribution` header
  source, surfaced config-load warnings, hoisted prefix allocation) complete
  the change set.

- **Release mechanism fix (v1.0.111)** — the release workflow's changelog
  extractor now matches the Keep a Changelog `## [<version>] - <date>` header
  form (plus the two legacy forms), so GitHub releases carry their notes; the
  v1.0.107..v1.0.110 releases were backfilled.

- **TUI paint-safety fix (v1.0.110)** — `should_render` now
  paints a pending message-cache group at the safety interval, closing the
  PERF-042 throttle-tail stall where a tool-call row stayed invisible while
  the status bar showed it running (regression test
  `test_pending_message_cache_group_forces_safety_paint`), plus dependency and
  advisory hygiene (dropped unused `dirs`/`tokio`/`tempfile` dependencies;
  removed four stale `deny.toml` advisory ignores).

- **`/simplify` final phase over v1.0.106..v1.0.108 (v1.0.109)** — the last
  code-quality findings from the three-commit review window land: dead
  `utf8_prefix_len` helper removed from the LLM HTTP client (FUNC-033 made it
  redundant), `config_agents_dir` collapsed into `global_agents_dir`,
  `extract_http_status` parses status digits byte-wise without an intermediate
  allocation, and `ragent-research`'s eight poisoned-lock sites now share one
  `lock_conn()` helper with a dedicated `SourceVaultError::LockPoisoned`
  variant.

- **Config rules and fixes (v1.0.108)** — code-quality pass over the
  v1.0.105..v1.0.107 window from five parallel explore reviews: GitLab legacy
  credential migration scans the real legacy `~/.ragent/` root again (silent
  no-op migration fixed, regression test pinned), the team-blueprint "global"
  fallback restores the true legacy `~/.ragent/blueprints(/teams)` scan so old
  installs keep working, the four `/spec govcreate` mutex-lock sites uniformly
  recover poisoned locks, `/config show` renders unavailable global dirs as
  "(unavailable)", `acquire_local` no longer misreports an exact-cap natural
  completion as budget exhaustion, and shared research helpers
  (`num_prefix_re`, `join_text`) are deduplicated into single copies.

- **`/spec govcreate` — spec authoring from an architecture document
  (v1.0.107)** — `/spec govcreate <spec-id>
  <content-ref> <target-folder>` (and the `ragent spec govcreate` CLI
  subcommand) acquires an architecture document from a local folder or URL,
  extracts its content, authors `SPEC.md`/`PLAN.md`/`TESTPLAN.md` via the
  configured LLM, and writes a new spec — with staged `[ .. ]/[ ok ]/[fail]`
  progress in the TUI, Escape cancellation, and an FR-011 guard that refuses
  non-empty targets without `--force`. The same window carries HEAD~3 review
  fixes: SSE chunk coalescing no longer splits multi-byte UTF-8 sequences
  driven by `Utf8Error::error_len()` (with six regression tests), secret
  redaction captures key+separator and widens the value charset to
  base64/base64url, spec dependency-range expansion is capped at 1000 IDs,
  `delete_memories_by_filter` reports real row counts, and masterfetch's cache
  deletes corrupt rows instead of poisoning hits. Documentation: the 612-line
  `tools.md` is replaced by 26 per-category how-tos in `docs/howtos/tools/`
  with argument tables and examples, each with a generated PDF.

- **`--url-cloak` research source defanging (v1.0.106)** —
  `/research create` gained `--url-cloak`, which emits web source URLs as
  defanged plain text rather than clickable links: the scheme is rewritten
  (`https://` -> `hxxps://`, `http://` -> `hxxp://`), every dot is bracketed
  (`example.com` -> `example[.]com`), and the result is wrapped in a Markdown
  code span. It applies to the `**Sources:**` bullets under each finding and
  the `References Index` table in `RESEARCH.md` (plus the `Sources Reference`
  table in `CORPA.md`), leaving non-URL rows untouched, so automated URL
  scanners do not flag the document. Available on the root CLI, the TUI slash
  command, and `POST /research` (`url_cloak`), recorded in frontmatter
  (`url_cloak: true`) for `/research update` replay, and off by default. A
  follow-up `/simplify` pass over the change set tightened
  `SourceVault`/`GatherLog` blocking offloads (shared `run_blocking` helper,
  new `SourceVaultError::TaskPanic`), defanged the `cloak_url` non-URL fallback,
  hardened `extract_http_status` against "500ms"/"404 bytes" false positives,
  and folded `assemble_and_write`'s 18 parameters into one `AssembleInput`
  struct; the how-to manuals were rebuilt to PDF.

- **Research output limits, scholarly-engine exclusion, and progress-table
  detail (v1.0.105)** — `/research create` caps its
  `## Concepts` / `## Findings` lists at 5 / 20 by default, reordering
  most-relevant-first (highest cited source rank, then cited count) before
  truncation; the limits are set with `--max-concepts N` / `--max-findings N`
  on the root CLI, TUI, and `POST /research` (`max_concepts` / `max_findings`
  fields), or persistently via `research.max_concepts` / `research.max_findings`
  (`0` = unbounded). `mf_search` gained an `exclude_engines` array that drops
  named backends before any request is dispatched, and research `--no-papers`
  (alias `--no-scholarly`, config `research.exclude_academic_engines`) routes
  through it so OpenAlex consumes no search budget and cannot shadow general-web
  URLs in dedup. `POST /research` gained `no_scholarly`; `--oa-enable`/`--no-oa`
  toggle open-access recovery per run. The per-engine progress table now shows
  *why* candidates were dropped (five exclusion-reason columns) and *how* fetches
  failed (seven failure-kind columns). `/spec impl` now expands task-range
  Dependencies cells (`T-001–T-014`) into every spanned ID, the TASKS panel shows
  the task ID after the status, and every toggled right-hand side panel takes 50%
  of the window width.

- **Functional anti-pattern remediation — FUNCPLAN.md (FUNC-038..069,
  080..082) complete (v1.0.104)** — the second pass closes the plan's M1 tail
  and all of M2-M5: `mf_search` keyless engines now surface a dead engine as
  an error instead of a zero-result success; `github_merge_pr` rejects an
  unknown merge method instead of silently merging; the last three production
  poison-lock panics recover via `PoisonError::into_inner`; GitLab GETs retry
  `429` honouring `Retry-After` with bounded request/entry budgets and full
  pagination; `move_file` renames first (no orphan dirs), `copy_file` refuses
  a self-copy, `append_file` flushes; GitHub `post`/`put`/`patch` honour
  `base_url`; percent-encoding is byte-wise (UTF-8 correct) across
  GitHub/GitLab; codeindex `parent_id` resolves multi-level nesting to a
  fixpoint; the keyword verifier no longer passes an empty/uncited analysis;
  the server auth comparison hashes fixed-length digests and the rate limiter
  enforces exactly 60/min; and a new `scripts/check-poison-locks.sh` guard is
  wired into `pre-flight.sh` and CI.

- **M1 agent per-turn hot path — PERF-032..040 + PERF-048 complete
  (v1.0.103)** — the per-turn allocation and clone load in the agent loop is
  removed: the provider-facing transcript is held and handed out behind an
  `Arc<Vec<ChatMessage>>` (no per-turn deep clone), a pure history append
  converts only the new tail (`SessionState::take_cached_for_append`), the
  subagent tool surface is cached behind the tool-registry version, `LoopTracker`
  is `Copy`, a new `RequestTokenTracker` makes the per-step token estimate
  O(changed message) instead of O(history), tool/result pairing is a single
  pass, the compaction prompt is assembled into one buffer, memory-entry token
  costs are memoised, the activity log is written by one background task per
  process, and the TUI viewers retain a single copy of their rendered rows.
  New benches `turn_loop` / `m3_hot_paths`; new guards `test_activity_writer`
  and `test_no_percall_regex`. `rustls` bumped 0.23.43 -> 0.23.45
  (RUSTSEC-2026-0285).

- **Simplify/quality pass across the search, research, agent, and TUI crates
  (v1.0.102)** — the second `/simplify` sweep deduplicated the API-key search
  engines behind shared preflight/finish/mask helpers, made engine-merge output
  deterministic with renumbered positions, resolved the web-gatherer volume
  policy through one `volume_policy()` helper, and removed the vestigial
  deadline flag. A third pass (v1.0.101) followed the same pattern across
  `masterfetch::search` and `ragent-research`.

- **Serper search engine, mf_search resilience, and research webgather
  cleanups (v1.0.100)** — `mf_search` gained a fifth optional API-backed
  engine (Serper, Google search) alongside LangSearch / Tavily /
  Perplexity / Exa, configurable via `serper_api_key` in `ragent.json`;
  transient engine failures (Wikipedia 429, HTTP 5xx, transport timeouts)
  are now retried inside the engine call path with exponential backoff
  (2 retries, 1 s/2 s), account-level quota blocks are reported as
  `blocked_engines` with reasons instead of retried, and the orchestrator
  staggers engine starts by 120 ms so keyless backends no longer burst at
  t=0 (this fixes the "only LangSearch results" `/websearch search`
  symptom). The research web-gatherer no longer cancels in-flight fetches
  at the `--web-time` phase deadline (every candidate is fetched to
  completion; the deadline bounds only the search stage) and the
  consecutive-failure search circuit breaker was removed in favour of the
  H-002 retry policy. `/research` gained an interactive clarification
  gate in the TUI.

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
