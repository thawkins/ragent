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
  and a Model Router cluster out of the box, with an extensible provider trait for adding more
- **Local-first defaults** — when no model is explicitly configured, ragent resolves
  to the first available local/self-hosted provider (e.g. Ollama) rather than
  hard-wiring a cloud provider
- **Comprehensive tool system** — ~150 registered tools across 18 categories:
  - **File operations** — read, write, create, edit, multiedit, apply_patch, patch, rm, move, copy,
    mkdir, append, file_info, diff, glob, list
  - **Shell** — bash, bash_reset, open (7-layer security with safe-command whitelist,
    banned commands, denied patterns, directory escape prevention, syntax validation,
    obfuscation detection, and user allowlist/denylist)
  - **Search** — grep
  - **Web** — webfetch, websearch, http_request
    - **Browser automation** — browser (Chrome DevTools Protocol: open, snapshot,
      click, type, fill_form, select, wait, eval, scroll, upload, press,
      screenshot, status, setup)
    - **Code intelligence** — codeindex_search, codeindex_symbols, codeindex_references,
      codeindex_dependencies, codeindex_status, codeindex_reindex (read-only,
      hardwired always-allowed)
    - **Memory** — memory_read, memory_write, memory_replace, memory_store,
      memory_recall, memory_forget, memory_search, memory_migrate,
      conversation_search, session_search
    - **Teams** — 20 tools for team lifecycle, tasks, messaging, and coordination
    - **GitHub & GitLab** — 29 native VCS tools for issues, PRs/MRs, pipelines, CI/CD,
      and repository management
    - **Office & PDF** — office_read/write/info, libre_read/write/info, pdf_read, pdf_write
    - **Sub-agents** — new_task, cancel_task, list_tasks, wait_tasks, task_complete
    - **Planning** — plan_enter, plan_exit
    - **MCP** — mcp_tool (McpToolWrapper) for external Model Context Protocol servers
    - **Interactive** — question, think, todo_read, todo_write
    - **Utility** — calculator, get_env
    - **Code search & navigation** — codeindex_search, codeindex_symbols,
      codeindex_references, codeindex_dependencies, codeindex_status, codeindex_reindex
    - **MasterFetch** — mf_fetch, mf_search, mf_crawl, mf_cache_clear for web content
      extraction, search, and crawling
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
- **Prompt optimization** — `/opt <method> <prompt>` transforms any prompt into structured
  frameworks (CO-STAR, CRISPE, CoT, DRAW, RISE, VARI, Q*, O1-STYLE, Meta Prompting) and
  platform adapters (OpenAI, Claude, Microsoft/Azure); also available via `POST /opt`
- **Code index** — automatic codebase indexing with tree-sitter parsing (15+ languages),
  full-text search via Tantivy, incremental updates via file watcher, and LLM-accessible
  tools; supports Rust, Python, TypeScript/JavaScript, Go, C/C++, Java, OpenSCAD,
  Terraform, CMake, Gradle, and Maven; enable/disable via `/codeindex on|off`,
  language filtering via `/codeindex lang <language>`
- **Memory system** — three-tier system with file blocks, structured SQLite store,
  and optional embedding-based semantic search; automatic extraction, decay,
  compression, and knowledge graph support
- **Spec management** — `/spec` slash commands for creating, listing, searching,
  validating, and tracking specification lifecycles
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

# Launch the interactive TUI
ragent

# Run a one-shot prompt
ragent run "Explain this codebase"

# Start the HTTP server only
ragent serve --port 9100
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
        "keep": { "tokens": 0.20 }
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

## Prompt Optimization

The `/opt` slash command (and `POST /opt` HTTP endpoint) transforms a plain prompt into
one of 12 structured frameworks — no LLM call needed, instant results.

```
/opt help                           # show method table
/opt co_star Explain Rust lifetimes
/opt cot     Solve the two-sum problem
/opt draw    A futuristic city at sunset
```

| Method          | Description                                             |
| --------------- | ------------------------------------------------------- |
| `co_star`     | Context, Objective, Scope, Task, Action, Result         |
| `crispe`      | Context, Role, Intent, Steps, Persona, Examples         |
| `cot`         | Chain-of-Thought step-by-step reasoning                 |
| `draw`        | Image prompt: subject, style, details, negatives        |
| `rise`        | Role, Intent, Scope, Examples                           |
| `o1_style`    | Stylized creative tokens and constraints                |
| `meta`        | Meta Prompting — generate the internal prompt          |
| `variational` | VARI — multiple prompt candidates + selection criteria |
| `q_star`      | Q* — iterative query refinement                        |
| `openai`      | OpenAI/GPT system+user adapter                          |
| `claude`      | Anthropic Claude adapter                                |
| `microsoft`   | Microsoft Azure AI adapter                              |

HTTP endpoint (requires Bearer token):

```bash
curl -s -X POST http://localhost:9100/opt \
  -H "Authorization: Bearer $RAGENT_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"method":"co_star","prompt":"Explain Rust lifetimes"}'
```

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

- Guide: [`docs/teams.md`](docs/teams.md)
- How-to manual: [`docs/howtos/howto_teams.md`](docs/howtos/howto_teams.md)
- Example bundles: [`examples/teams/`](examples/teams/)

## Architecture

The project is a Cargo workspace built from 15 focused crates:

| Crate                     | Purpose                                                                                                                                                                                           |
| ------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `ragent-agent`          | Agent/runtime layer: sessions, orchestration, MCP, memory, tool registry                                                                                                                          |
| `ragent-bench`          | Benchmark runner shared between TUI and CLI                                                                                                                                                       |
| `ragent-codeindex`      | Codebase indexing: tree-sitter parsing, SQLite store, Tantivy FTS, file watcher                                                                                                                   |
| `ragent-config`         | Configuration types, defaults, and parsing                                                                                                                                                        |
| `ragent-llm`            | Provider clients and model/provider registry (Anthropic, OpenAI, Gemini, Ollama, HuggingFace, Copilot, Generic OpenAI, Azure AI Foundry, Azure Resource, Amazon Bedrock, Microsoft Foundry Local) |
| `ragent-prompt_opt`     | Prompt optimization templates and completer abstraction                                                                                                                                           |
| `ragent-server`         | Axum HTTP routes and SSE streaming                                                                                                                                                                |
| `ragent-specs`          | Spec lifecycle management: discovery, validation, status transitions, review, archival                                                                                                            |
| `ragent-storage`        | SQLite-backed storage, snapshots, encrypted credentials                                                                                                                                           |
| `ragent-team`           | Team coordination runtime and team tools                                                                                                                                                          |
| `ragent-tools-core`     | Core shell/file/search tools                                                                                                                                                                      |
| `ragent-tools-extended` | Extended document/web/memory/codeindex tools                                                                                                                                                      |
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

**v0.1.0-beta.28** — The core architecture, tool system (~150 tools across 18 categories), TUI,
HTTP server, memory system, teams/swarm coordination, spec management, skills system,
research system, and multi-layered security are functional and under active development.

Recent highlights:

- `memory_store` now reports a clear `stored` result and the TUI summary correctly shows
  successful structured-memory writes
- Startup blocking fixes: MCP servers, code-index, and provider health checks moved to background tasks
- Startup timing instrumentation with `/startup` slash command
- Copilot `gh auth token` cached process-wide via `OnceLock`
- Code-index SQLite WAL mode and direct `file_id` symbol queries
- First printable keystroke after run-cost banner no longer swallowed
- Conversation search (`conversation_search`) and cross-session search (`session_search`) tools
- Browser automation tool (`browser`) with Chrome DevTools Protocol (CDP) backend — 14 actions
- TODO side panel added (`Alt+T`) with `/todo` slash alias
- Agentic-loop performance upgrade (PERFPLAN.md milestones A–F)
- All 279 compiler warnings eliminated (build, tests, benches, examples)
- Model Router cluster provider with downstream-model status bar and terminal-signal guarantee
- `/provider` now always prompts for the API key (pre-filled with the existing key) so keys can be edited without removing the provider
- `/model` jumps straight to the model list when a provider is already configured
- API-key and GitLab token fields shown unmasked with a wider dialog for full visibility
- `/config show`, `/config save`, and `/config list` slash commands for inspecting and
  snapshotting configuration
- Research `--use-low-relevance` flag retains low-relevance web sources instead of filtering them out
- Compaction bail paths now publish user-visible `AgentNotice` events instead of silently failing
- Post-compaction continuation nudge threaded across loop iterations (no repeated nudges)
- Autopilot auto-continue suppressed after `task_complete` (new `last_task_completed_at` guard)
- Router status bar shows the actual downstream model and tier: `Model Router ({model}) / {tier}`
- Autopilot status indicator (`AutoPilot:✓/✗`) in the TUI status bar
- Skill discovery tests isolated by `SkillScope` and `bundled_count()`
- Doctest build breakages fixed in `session::permissions` and `tool::ToolRegistry`
- Startup blocking fixes: MCP servers, code-index, and provider health checks moved to background tasks
- Startup timing instrumentation with `/startup` slash command
- Copilot `gh auth token` cached process-wide via `OnceLock`
- Code-index SQLite WAL mode and direct `file_id` symbol queries
- First printable keystroke after run-cost banner no longer swallowed
- Conversation search (`conversation_search`) and cross-session search (`session_search`) tools
- Browser automation tool (`browser`) with Chrome DevTools Protocol (CDP) backend — 14 actions
- TODO side panel added (Alt+T) with `/todo` slash alias
- Agentic-loop performance upgrade (PERFPLAN.md milestones A–F)
- All 279 compiler warnings eliminated (build, tests, benches, examples)
- Context compaction pipeline added (`/compact` slash command; `compaction` config block in `ragent.json`)
- `read` tool instructions clarified (`end_line` is absolute line number)
- Remote push prohibitions strengthened in `AGENTS.md`
- SPEC.md audited, reorganized, and updated for v0.1.0-beta.28
- Azure Resource (File) provider with `azureresources.json` support
- Azure AI Foundry provider with dynamic model discovery
- Amazon Bedrock provider with AWS SigV4 signing and dual API support
- Startup ASCII art banner with compile timestamp
- Instruction file discovery logging

See [CHANGELOG.md](CHANGELOG.md) for the full history.

## License

MIT
