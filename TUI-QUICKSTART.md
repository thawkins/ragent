# TUI Quick Start Guide for ragent

A hands-on guide to using **ragent** through its full-screen terminal UI.

---

## Highlights (v1.0.119)

- **Sessionful Streamable-HTTP MCP servers report their tools** — the HTTP MCP
  client now performs the `initialize` handshake, replays the returned
  `mcp-session-id` on every later request, advertises `text/event-stream` in
  `Accept`, and unwraps `event: message` SSE frames, so servers such as the
  MongoDB MCP server (2025-era sessionful path) no longer reject `tools/list`
  with HTTP 400. The client is built lazily, so it no longer panics outside a
  Tokio runtime.
- **Startup MCP report is reliable** — the TUI waits up to three seconds for
  background MCP connects and awaits the shared client lock, so the per-server
  `[mcp] Starting/Connected/Skipping ...` lines reflect the real outcome.
- **`/plugins list --mcp` is deterministic** — server ids are sorted
  alphabetically in the contributions block, so the rendered table is stable
  between runs. `/plugins list` resolves live MCP tool counts from connected
  servers.
- **`/mcp` list output tightened** — it prints `tools: N` per server instead of
  the full tool-name inventory; the inventory lives on `/plugins list --mcp`.
- **`/simplify all` fixes** — `/swarm status` progress bar no longer overflows;
  `/spawn` no longer overwrites a landed launch outcome; `/plugins <non-list>
  --mcp` executes the requested operation; the swarm unblock path persists task
  state; `/alog` delete propagates storage errors.

## Highlights (v1.0.118)

- **Durable, global MCP server enable/disable state** — whether an MCP server is
  actually started is a persisted choice (`<global state dir>/mcp_state.json`),
  not an implicit side effect of being listed in `ragent.json`. A server id
  **absent** from the ledger is enabled, so a newly added server (written into
  `ragent.json` or bridged from a plugin's `mcpServers` section) starts enabled
  with no extra step; `mcp.<id>.disabled: true` in `ragent.json` always
  disables a server. `/mcp connect <id>` enables and connects a server live
  (registering its tools immediately) and `/mcp disconnect <id>` disables and
  disconnects it live; both survive a restart and apply to every project.
- **`/mcp` lists plugin-contributed servers and their live status** — the
  display list is built from the same merged server set the connect path uses
  (`plugin_mcp_servers`), so a plugin-bridged `<plugin-id>.<server>` shows in
  `/mcp` even with no `ragent.json` entry. `/mcp` prints `enabled yes/no` and
  `tools: N` per server (the individual tool names live in
  `/plugins list --mcp`).
- **`/plugins list` shows MCP server and tool counts** — the table gains `MCP`
  and `MCP Tools` columns and the contributions block renders
  `mcp [<id> (<n> tools)] (S server(s), T tool(s))`. A server whose count is not
  yet known renders `?`, never `0`.
- **`/tools` lists visibility-disabled tools** — a family switched off
  (`/tools github off`) no longer vanishes from the report; the listing prints
  `Visible Tools (N total, M disabled)` followed by a `Disabled by visibility`
  section, backed by `ToolRegistry::hidden_definitions()`.
- **Plugin `mcpServers` entries are bridged by default** — a plugin's MCP-server
  transport section (inline, or the Claude `"mcpServers": "./mcp.json"` file
  reference) connects as `<plugin-id>.<server>` and is listed as an MCP server;
  `McpToolWrapper::execute` refuses to call a disabled server's tools, naming the
  command that re-enables it.

## Highlights (v1.0.117)

- **`/spec reverse --folder` scaffolds and hosts the target project** —
  `/spec reverse` accepts `--folder <path>` plus `--github` / `--gitlab` (with
  the `/new` scaffold flags present) and creates the project in the target folder
  before synthesising the prompt; a chained `--create <name>` writes the spec
  into `<folder>/specs/<name>/`. Usage errors now state the specific cause
  instead of the bare usage line.
- **`webapp` app type** — `--type webapp` is a first-class registered value for
  `/new`, `/spec reverse`, and `/spec govcreate`; it generates a tiny
  dependency-free HTTP-server starter for `rust`, `python`, `go`, `typescript`,
  and `javascript`, and degrades to a manifest-only layout elsewhere.
- **`lopdf` joins the lint suite** — the vendored `lopdf` crate carries
  crate-level allowances (matching `vendor/pdf-extract`) so the dead-code lint
  and `cargo-machete` are green. Full CI hygiene (`cargo check`, dead-code lint +
  reason checks, `clippy -D warnings`, `cargo fmt --check`, `cargo audit`,
  `cargo deny check`, `cargo test`) is green.

## Highlights (v1.0.115)

- **`/spawn` detached sub-agents** — `/spawn <agent> <prompt...>` launches a
  background sub-agent straight from the chat input as a **detached**
  fire-and-forget task. It runs concurrently and shows in the Agents panel, but
  nothing ever waits on it: it is absent from `list_agents`, cannot be awaited
  with `wait_agents`, and its result is never injected back into the chat. Use it
  for side-effect-only work; give the prompt a file to write, because the reply
  body is not returned. Cancellable with `/cancel <prefix>`; a second `/spawn`
  while one is still registering is refused. Every completed sub-agent run
  (detached or not) now persists its FULL output to `log/subagents/<task-id>.md`
  and the completion event carries the real loop `finish_reason` (`stop` /
  `truncation` / `length` / `cancelled` / `error`), so a provider-side cut is
  flagged in the Agents panel instead of looking like a healthy finish. The
  `new_agent` tool gained the matching optional `detached: true` parameter. See
  [`docs/howtos/slashcommands/spawn.md`](docs/howtos/slashcommands/spawn.md).

## Highlights (v1.0.110..v1.0.114)

- **Plugin bridges (v1.0.114)** — plugin bridge extensions: `/plugins list`
  counts and details skills/agents/hooks, `/plugins add` installs a plugin
  enabled, and an enabled plugin's skills, MCP servers, `commands/*.md` prompt
  commands, `agents/*.md` profiles, and declared (including `hooks.json`) hooks
  are bridged into the session; `/new --github` now creates the hosting
  repository through a shared GitHub credential chain with a `gh` CLI fallback;
  and a new read-only `ragent_info` tool reports the running version, build
  time, git commit, and compiler (169 tools).
- **Input queue + ALT-Q queue-control menu (v1.0.113)** — the input field
  stays editable while the agent runs: each `Enter` appends the message to a
  bounded FIFO queue (a two-digit counter appears before the `> ` prompt) and
  the oldest entry runs automatically at each turn boundary. Press **`Alt+Q`**
  during a run to open the four-row queue-control menu (`Next`, `Stop`/`Resume`,
  `Clear`, `Show`); the `Show` row opens a scrollable panel that lists the queued
  entries and lets you reorder (`Enter`) or remove (`Del`) them, and
  `/queue [list|clear|next|help]` exposes the same queue. A slash command
  (`/…`) is queued the same way while a turn runs (FR-017 amendment) and runs at
  the next turn boundary; only bang commands (`!…`) and teammate-targeted sends
  keep the busy refusal. See §4 and
  [`docs/howtos/slashcommands/queue.md`](docs/howtos/slashcommands/queue.md).
- **`/plugins` slash family (v1.0.112)** — the plugin system is now
  implemented, not just specified. `/plugins list|add|remove|enable|disable|test|stores|help`
  manages sandboxed Codex- and Claude Code/Desktop-dialect plugins; enabled
  plugins contribute `plugin_<id>_<tool>` tools, slash commands (including the
  Claude `commands/*.md` prompt commands), skills, MCP servers, agents, and
  hooks. `/plugins codex` and `/plugins claude` browse each store's official
  marketplace; `/plugins stores [--check]` reports each store's effective
  endpoint. Reports
  render in the message window with the `From: /plugins <sub>` header; a bare
  `/plugins` or an unknown subcommand prints the usage block. The subcommands
  are listed in the slash-command autocomplete menu. CLI parity:
  `ragent plugins <sub>`. Configure via the `plugins` block
  (`plugins.enabled: false` makes the subsystem inert). See
  [`docs/howtos/slashcommands/plugins.md`](docs/howtos/slashcommands/plugins.md).
- **Paint-safety fix (PERF-042 throttle tail, v1.0.110)** — when the stream
  throttle leaves a message group pending (for example a tool-call row that
  arrives while the window is open), the safety-interval wake now paints it,
  so the row no longer stays invisible while the status bar already shows the
  tool running (`should_render` in `crates/ragent-tui/src/lib.rs`;
  regression test `test_pending_message_cache_group_forces_safety_paint`).
- **Config rules and fixes (v1.0.108)** — `/config show` renders unavailable
  global memory/agent dirs as "(unavailable)"; `/spec govcreate` mutexes
  recover uniformly from poisoning; GitLab legacy credential migration and
  the legacy team-blueprint fallback scan the real `~/.ragent/` root again.
- **`/simplify` final phase (v1.0.109)** — code-quality-only cleanups across
  the LLM HTTP client, custom-agent discovery, and `ragent-research`; no
  behaviour change.

## Highlights (uncommitted, on top of v1.0.106)

- **`/spec govcreate` — author a spec from an architecture document** —
  `/spec govcreate <spec-id> <content-ref> <target-folder> [--language ..]
  [--type ..] [--stack ..] [--github|--gitlab] [--force]` acquires an
  architecture document from a local folder/file or public URL, extracts its
  content, authors `SPEC.md`/`PLAN.md`/`TESTPLAN.md` with the configured LLM,
  and writes the new spec. Progress streams into a single in-place message
  (`[ .. ]/[ ok ]/[fail]` per stage), Escape cancels the run, and a non-empty
  target folder is refused without `--force`. CLI parity: `ragent spec
  govcreate`. See `docs/howtos/spec.md` §5.19.
- **Markdown table rendering fix** — ragged pipe tables in model output
  (missing trailing pipes, uneven cells, broken separator rows) are
  normalised by a new `preprocess_markdown_tables` pass before rendering, so
  tables display instead of raw text.
- **Tool reference split** — the single `docs/howtos/tools.md` became 26
  per-category pages in `docs/howtos/tools/` with argument tables and worked
  examples per tool.
- **Input queue + ALT-Q queue-control menu** — the input field stays editable
  while the agent runs: each `Enter` appends the message to a bounded FIFO
  queue (a two-digit counter appears before the `> ` prompt) and the oldest
  entry runs automatically at each turn boundary. Press **`Alt+Q`** during a
  run to open the four-option queue-control menu (`Next`, `Stop`/`Resume`,
  `Clear`, `Show`); the `Show` row opens a scrollable panel that lists the queued
  entries and lets you reorder (`Enter`) or remove (`Del`) them. See §4.

## Highlights (v1.0.106)

- **`--url-cloak` research source defanging** — `/research create --url-cloak`
  writes the report's web source URLs as defanged plain text (`hxxps://host[.]tld/…`
  in a Markdown code span) in the `**Sources:**` bullets and the `References
  Index` table instead of clickable links, so automated URL scanners do not
  flag the document. Non-URL rows are unchanged; the flag is off by default
  and is recorded in frontmatter so `/research update` replays it.

## Highlights (v1.0.105)

- **Research output limits** — `/research create` caps its `## Concepts` and
  `## Findings` lists at 5 and 20 by default, reordering most-relevant-first
  before truncation. Set them per run with `--max-concepts N` /
  `--max-findings N` (autocomplete and parameter hints updated; `0` =
  unbounded) or persistently via `research.max_concepts` /
  `research.max_findings`.
- **Scholarly-engine exclusion** — `/research create --no-papers` (alias
  `--no-scholarly`) excludes academically-classified backends (OpenAlex) before
  any search request is dispatched; persist it with
  `research.exclude_academic_engines`.
- **Per-engine progress table detail** — the width-sweep table now shows *why*
  candidates were dropped (five exclusion-reason columns) and *how* fetches
  failed (seven failure-kind columns), with the totals row carrying the
  breakdown.
- **`/spec impl` task-range dependencies** — a Dependencies cell such as
  `T-001–T-014` now expands into every spanned ID, so the final verification
  task is scheduled last instead of first.
- **Tasks panel and side panels** — the Alt+T tasks panel row now reads
  `[STATUS] <id> title`, and every toggled side panel (Alt+M/T/P/O/C and the
  log panel) takes 50% of the window width.

## Highlights (v1.0.103)

- **Faster agent turns (M1 performance pass)** — the agent loop no longer
  deep-clones its transcript or re-sums the whole history on every step. The
  provider-facing transcript is shared behind an `Arc`, a pure append converts
  only the new tail, the compaction token estimate is incremental, and the
  activity log is written by one background task per process. TUI-side, the
  `/research open`, Alt+M full-memory, and output-view overlays now retain a
  single copy of their rendered rows (PERF-048) and the status bar measures
  each span set once per frame. No user-visible behaviour change.
- **Second `/simplify` sweep (v1.0.101/v1.0.102)** — the API-key `mf_search`
  engines share preflight/finish/mask helpers, engine-merge output is
  deterministic, and the research web-gatherer volume policy is one helper.
- **`rustls` security bump** — rustls 0.23.43 -> 0.23.45 (RUSTSEC-2026-0285,
  a TLS 1.3 handshake boundary issue).

## Highlights (v1.0.96)

- **`/prompt` system-prompt inspector** — a new read-only slash command renders
  exactly what the LLM receives as its system prompt: `/prompt primary [agent]`,
  `/prompt subagent [agent]` (interactive tools excluded), `/prompt list`, and
  `/prompt <agent-name>`. No LLM call, no writes, no session mutation.
- **Tool-repeat guard (FR-044)** — five consecutive identical tool calls pass
  through; the sixth raises a `tool:repeat` confirmation in interactive runs
  and is auto-denied in subagent/`--yes`/YOLO runs with a corrective
  observation, so unattended loops cannot hang on the same call.
- **Redundant slash commands removed** — `/opt` (prompt optimization, with its
  `ragent-prompt_opt` crate and `POST /opt` endpoint), `/tasks` (alias of
  `/task list`), and `/theme` (registered but never dispatched) are gone;
  the workspace is back to 16 crates.

## Highlights (v1.0.95)

- **AgentNotice chat-bubble separation (v1.0.95)** — consecutive
  `Event::AgentNotice` notices now render as their own yellow bubbles: the
  TUI event handler forces a new assistant message before and after
  appending a notice, with the renderer's trailing blank line separating it
  from the next bubble and subsequent streamed text.
- **`/toolchain list` fixed-width table (v1.0.95)** — the report table's
  Language, Runtime, Status, and Version columns are fixed at 10, 10, 10,
  and 50 characters (a constant 93-column grid). Language and Runtime cells
  clip to the column width; Status and Version cells word-wrap onto
  continuation grid lines so no text is lost.
- **Stable toolchain** — the pinned `rust-toolchain.toml` moved from
  `nightly-2026-09-04` to the current stable channel; builds, CI, and user
  shells now compile with stable Rust (1.98.1 at time of release).

## Highlights (v1.0.90-1.0.93)

- **`/new` project scaffolding** — scaffold a brand-new project in an empty
  directory from the TUI (`/new --language rust --type cmdline`) or the CLI
  (`ragent new ...`): ragent workspace, runnable hello-world artifacts for
  26 application languages (rust/python/go/typescript/shell/...) plus
  sample stubs for data and build formats (library/cmdline/tui/gui), starter
  docs, git init,
  and optional `--github`/`--gitlab` hosting + push; steps stream live into
  the message window.
- **Sub-agent termination protocol** — a new sub-agent-mode system-prompt
  section instructs spawned sub-agents to always finish with a single
  `agent_complete` call, so background tasks report completion promptly
  instead of lingering in a running state.
- **Mandatory sub-agent completion (v1.0.93)** — the completion protocol is
  now a hard requirement: the sub-agent system prompt demands
  `agent_complete(summary)` as the FINAL action of EVERY run (including
  failed or empty runs), the `agent_complete` tool description repeats the
  requirement, and the mid-run summary nudge ends with the same instruction.
- **`/spec impl` tracker tasks (v1.0.93)** — `/spec impl` no longer
  pre-creates session tracker tasks; `spec_task_update` creates-or-updates
  the tracker task as each spec task progresses, seeded from the spec's
  `PLAN.md` entry.
- **Faster compaction (v1.0.93)** — `/compact` supports a
  `compaction.model` fast/cheap summariser override, the summary budget is
  configurable (`summary_tokens`, default 1500) and halved from 4,096, the
  summarisation stream has 180 s / 60 s caps with chunk-level cancellation,
  and the prompt cap adapts to the model's context window.
- **Compaction label disambiguation** — mid-run auto-compaction summaries
  render with a dim `[compaction]` prefix rather than the assistant marker,
  so they are no longer mistaken for a sub-agent's completion report.
- **Context panel rebind** — the context side panel moved from `Alt+X` to
  `Alt+C`, keeping the `Alt+<letter>` side-panel family (log, profiler,
  tasks, telemetry, context) mnemonic-consistent.

## Highlights (v1.0.88)

- **Status bar last-prompt tag** — the top status line now renders the most
  recent prompt (chat message, slash command, bang command, or `/loop` goal)
  as a bracketed tag — `[first 32 chars....]` when truncated — directly after
  the `Branch: `-labelled git branch, as one group immediately after the
  `Project:`-labelled working directory. The working directory shortens when
  the group plus the session status would not otherwise fit, and the layout
  falls back to the previous cwd/branch/status arrangement on narrow
  terminals.
- **Cleaner transcript start** — the message window no longer emits a blank
  line before the very first `You:` prompt, so the transcript does not start
  with a stray blank row.
- **Research runs unchanged for users** — an uncommitted internal simplify
  pass over `ragent-research` fixed a malformed-response panic, cached hot
  regexes, and deduplicated the report/IMRaD layout code; `/research create`
  output and progress display are unchanged (search-engine summary ordering
  in the progress log is now deterministic/sorted).

## Highlights (v1.0.86)

- **Spec-system semantics documentation** — `docs/howtos/spec.md` now covers
  the spec file write semantics (atomic writes, clear-on-empty
  `REVIEW.md`/`FEEDBACK.md`), the `/spec coverage` report format with
  task-status symbols (`[ok]`, `[wait]`, `[sync]`, `[stop]`), and the
  automatic task-completion heuristic guarded by `writes_in_spec_dir`
  (writes inside the active spec's directory auto-complete `in_progress`
  tasks; writes elsewhere never do).
- **Reliable tool calling** — the tool-calling audit remediation pass fixed
  every HIGH/MED finding from the audit: OpenAI Responses API tool calls no
  longer dropped (missing `ToolCallStart`), Gemini final-chunk `functionCall`
  parsed before the finishReason flush, malformed tool arguments fail fast
  with a corrective LLM-visible error instead of silently executing with
  empty args, loop restrictions fail closed, required-args schema validation
  runs before permissions/execution, and panicked tool tasks synthesise an
  error result so the conversation never keeps an orphaned `tool_use`.
- **Edit tools preserve line endings and BOMs** — CRLF files keep CRLF
  endings through `edit`/`multi_edit` (no more silent CRLF-to-LF conversion),
  a leading BOM no longer blocks first-line edits, and non-UTF-8 files get a
  precise encoding error.
- **Text-format tool-call recovery** — when a model narrates tool calls as
  text (Qwen-style `tool_call` JSON blocks, XML-parameter blocks, or a bare
  tool-call JSON object) and produces no native calls, the processor extracts
  and dispatches them through the full permission pipeline, then notifies the
  model.

## Highlights (v1.0.84)

- **Goal-driven loop programming (`/loop`)** — a new slash-command family that
  runs a goal-driven agentic loop: `/loop` opens an interactive setup dialog
  (agent, goal, success state, verify command, scope, tool set, budget), and
  `/loop <agent> <goal text...>` starts immediately with documented defaults.
  Loops end with a status banner (`completed` / `error` / `budget_exhausted` /
  `interrupted`), capture a pre-loop snapshot, and offer a rollback flow:
  Enter restores the pre-loop snapshot, Esc keeps the loop's changes. See
  `docs/howtos/loopprogramming.md`.
- **New how-to manuals** — `docs/howtos/reactagent.md` (the core per-turn
  ReACT loop), `docs/howtos/loopprogramming.md` (goal-driven loops with
  `/loop`), and `docs/howtos/office.md` (office + PDF tool families with
  format matrix, JSON examples, and `tool_visibility.office` configuration).

## Highlights (1.0.79)

- **`/research create --max-search-calls N`** — hard, run-scoped cap on total web-search calls per research run, shared across every supervisor/competitive researcher and gather pass.
- **`/research update <name>`** — re-runs a research item by replaying its recorded invocation line, overwriting `RESEARCH.md` and associated files.
- **`--mode competitive` defaults `--format` to `comparison-table`** — competitive runs no longer require an explicit `--format comparison-table`.
- **`--depth` bounds web volume by default** — the effective web-source budget is derived from the selected depth (shallow 6 / standard 9 / deep 15) unless `--max-web-results` is passed explicitly.
- **`/clip`** — copies the rendered message-window transcript to the system clipboard in one step.
- **`/research list` renders a human-readable table again** — the fixed-width `NAME/TITLE/STATUS/CREATED/MODIFIED` table is the default output, with JSON behind the `--json` flag.

## Highlights (1.0.84)

- **/simplify quality pass** — ~40 fixes across 48 files: research comparison
  lowercase-slicing panic fix, single shared `tui_event_lag` Arc so the TUI
  lag reconcile actually runs, typed codeindex background result slots (no
  more `"\n\nSTATUS:"` magic markers), a new rollback-result poll slot so the
  /loop rollback status no longer sticks at "rolling back...", loop glob
  matchers compiled once per check, config parse that keeps real line/col
  caret diagnostics without the deep clone, telemetry shutdown/flush rework,
  and `current_working_dir()` replacing 63 silent root-relative path
  resolutions in slash commands.
- **Slash-command help coverage** — every slash command with a help form is
  covered by 30 new tests in `test_slash_help.rs`; `/config`, `/init`,
  `/context`, `/profile`, `/model`, `/provider`, `/mcp`, `/autopilot`,
  `/github`, `/gitlab`, `/mouse`, `/yolo`, `/skills`, `/agent` and friends now
  all answer `help` without triggering side effects.
- **Complete slash-command help set** — help subcommands and `--help`/`-h`
  aliases added across the dispatcher (config, init, context, mcp, profile,
  model, provider, mode, autopilot, github, gitlab, update, mouse, yolo,
  skills, agent, cancel, system, undo, name, resume, doctor, history, tasks,
  template, plan), with `/init help` placed before the side-effecting bare
  form.

## Highlights (1.0.80)

- **`plot_*` tools render inline graphs** — `plot_line`, `plot_scatter`,
  `plot_bar`, `plot_histogram`, `plot_pie`, and `plot_heatmap` draw ASCII-art
  charts directly in the message window, with real ANSI colours for palette
  series, pie slices, and heatmaps (rendered off-screen via `ratatui-plt`).
- **Threaded codeindex graph build** — `/codeindex graph build`,
  `/codeindex graph lang <l>`, and `/codeindex reindex` run in the background;
  the status bar animates `idx`/`graph` busy tags while the work runs and the
  completion message arrives when done. The step log also shows friendly
  one-liners for the code-index graph tools (`codeindex_godnodes`,
  `codeindex_path`, `codeindex_explain`, `codeindex_communities`) and
  `model_info`.
- **Graph-build visibility** — `/codeindex show` reports
  `**Graph:** building...` with per-file progress; the `codeindex_status` tool
  reports `graph_state`, `graph_building`/`graph_done`/`graph_total`, and the
  `IndexStats` graph counters (`graph_total_edges`/`graph_nodes`/
  `graph_communities`).

## Highlights (1.0.77)

- **Documentation refresh** — `CHANGELOG.md`, `README.md`, `SPEC.md`,
  `STATS.md`, `QUICKSTART.md`, `TUI-QUICKSTART.md`, and how-to docs updated to
  reflect the latest release.
- **Research evaluation scorecard** — configure `"research": { "evaluate": { "enabled": true } }`
  in `ragent.json` to append a deterministic quality scorecard (quality,
  relevance, groundedness, completeness, structure) to `/research create`
  reports.

## Highlights (1.0.76)

- **`--web-time` web-phase deadline (180 s default)** — `/research create` now
  caps the web-gathering phase at 180 seconds by default; when the deadline passes,
  everything gathered so far is ingested and the run proceeds to
  analysis/synthesis with the partial source set instead of discarding the phase.
- **No-new-work-after-deadline guarantee** — once the deadline elapses, no new
  search or fetch is started; the only overshoot is the completion of fetches
  already in flight (each capped by `--fetch-timeout-secs`), so the phase always
  returns (researchfix T-005, FR-008).
- **Concepts section in `/research create`** — the pipeline now extracts a
  cross-source concept list and embeds it in `RESEARCH.md` as a `## Concepts`
  section directly above `## Findings` (report layout) or `### Concepts` above
  `### Findings` (IMRaD layout).
- **Web-phase deadline observability and deduplication** — TUI status bar shows a
  live `web:M:SS` countdown during the web phase, and a single quantified notice is
  added to the research progress message when the deadline is reached
  (researchfix T-012).

## Highlights (1.0.74)

- **Clippy `for_kv_map` fix** — Ollama provider iteration now uses `values()`
  instead of destructuring a key-value pair, and the LangSearch merge test
  expectation was corrected.

## Highlights (1.0.73)

- **Sub-agent / teammate step visibility** — TUI step log now shows tool calls
  from tracked sub-agents and teammates with an `[agent-tag]` prefix; lagged
  event-bus bursts are rebuilt so visible steps stay in sync with the Agents/Teams
  panel counts.

## Highlights (1.0.72)

- **Token counting fixes** — TUI context panel percentages now use a consistent
  bytes-to-tokens conversion so they align with the status-bar usage figure.

## Highlights (1.0.70)

- **Multiple agent TUI fixes** — fixes for multi-agent TUI interactions, scroll pinning,
  active-agents button hit-rects, team panel layout, and idle-CPU hotspots
- **TUI scroll-pinning geometry and idle-CPU hotspots** — corrected scroll-offset semantics
  for output/research overlays and fixed crossterm reader exit hang and select-loop hot-spin

## Highlights (1.0.43)

- **Code index semantic graph** — Four new graph analysis tools
  (`codeindex_godnodes`, `codeindex_path`, `codeindex_explain`,
  `codeindex_communities`) with community detection via label propagation,
  shortest-path traversal, and god-node identification; `/codeindex graph
  build` sub-command; `/codeindex show` now reports graph-level statistics
- **Bang commands** — Prefix any prompt with `!` (e.g. `! ls -la`,
  `! cargo test --lib`) to run a shell command directly; the output is sent
  to the model for review and error resolution (v1.0.42)
- **Compaction fix** — Fixed compaction getting stuck when all messages fit
  inside the keep budget; `select()` now forces at least one message into the
  head when there are 2+ messages
- **Research panic isolation** — Vendored `html2text` with `saturating_sub`
  patches; `extract_pdf_text` runs on a dedicated OS thread with `panic_guard`
  (v1.0.40)
- **Stocks & currency tools** — `stock_quote`, `stock_history`,
  `stock_fundamentals`, `stock_search`, `stock_options`,
  `stock_recommendations`, `currency_rate`, `currency_history` (v1.0.36)
- **Start-of-turn compaction** — Uses persisted provider-reported input token
  count so it aligns with the TUI usage percentage (v1.0.34)

---

## 1. Starting the TUI in your project workspace

Open a terminal in the project you want to work on and run:

```bash
ragent
```

If `ragent` is not on your `PATH`, use the full path after building:

```bash
/path/to/ragent/target/release/ragent
```

The TUI opens with the input panel at the bottom, the message pane in the
middle, and status information at the top. You can start typing immediately.

### Useful startup flags

| Flag                                   | What it does                                         |
| -------------------------------------- | ---------------------------------------------------- |
| `--model ollamacloud/kimi-k2.7-code` | Start with a specific model                          |
| `--agent coder`                      | Start with the`coder` agent profile                |
| `--yes`                              | Auto-approve all permission prompts (YOLO-style)     |
| `--no-tui`                           | Skip the TUI and run a single prompt in the terminal |
| `--log`                              | Open the TUI with the log panel already visible      |

### Example

```bash
ragent --model ollamacloud/kimi-k2.7-code --agent coder
```

You can also open the provider setup directly from the input box:

```text
/provider ollamacloud
```

---

## 2. Creating an `AGENTS.md` file for your project

`AGENTS.md` is a project-level instruction file that ragent automatically
loads into its system prompt. It tells the agent how your codebase is
organised, what conventions to follow, and what tools to prefer.

Create it at the root of your project workspace:

```bash
# In the root of the project you are working on
ragent run "Create a concise AGENTS.md for this Rust project"
```

Or create it manually:

```bash
touch AGENTS.md
```

### Typical `AGENTS.md` contents

```markdown
# Project Guidelines

## Technology Stack
- Language: Rust (edition 2024)
- Build tool: Cargo
- Minimum Rust version: 1.85

## Code Style
- 4-space indentation, 100-column line limit
- Use snake_case for functions and variables, PascalCase for types
- Prefer explicit types and `Result<T, E>` error handling
- Use `tracing` for logging; avoid `println!` in library code

## Testing
- Tests live in each crate's `tests/` directory
- Use `#[test]` for sync tests, `#[tokio::test]` for async tests
- Run tests with `cargo test --workspace`

## Tool Preferences
- Use `codeindex_search` and `codeindex_references` for code symbol lookups
- Use `read` with `start_line` + `num_lines` for large files
- Prefer `multi_edit` when changing several files at once

## Documentation
- Use `///` doc comments for public functions
- Update `CHANGELOG.md` and `SPEC.md` when adding features
```

Keep it factual and concise. ragent reads it on every launch, so overly long
files can consume context window.

---

## 3. Selecting a provider and model

### Provider setup dialog

1. Start ragent.
2. Type `/provider` in the input box.
3. Select a provider from the list.
4. The API-key field is pre-filled with any existing stored key, shown unmasked
   in a wide dialog so you can edit or replace it. Press `Enter` to keep the
   current key, or type a new one. (Skip this step for providers that do not
   require a key, such as local Ollama.)
5. Choose a model from the provider's discovered model list.
6. Press `Enter` to confirm.

If a provider is already configured, `/model` jumps straight to that provider's
model list instead of asking you to pick a provider again. Use `/model show` to
print metadata for the active model.

The status bar at the top shows the selected provider, model, and a health
indicator:

- **● green** — provider reachable
- **● yellow** — health check in progress
- **✗ red** — provider unreachable or key missing

When the Model Router is active the status bar shows the actual downstream
model and tier, e.g. `Model Router (claude-sonnet-4-20250514) / complex`.

### Example: Ollama Cloud with `kimi-k2.7-code`

1. Type `/provider`.
2. Choose **Ollama Cloud**.
3. Enter your Ollama Cloud API key if prompted.
4. Select **`kimi-k2.7-code`** from the model list.
5. Confirm.

The status bar will update to something like:

```text
ollamacloud/kimi-k2.7-code ●
```

Other providers work the same way; only the available model list and the
required credentials differ.

### Available providers

ragent supports these providers out of the box:

| Provider                | Typical credential           | Notes                                |
| ----------------------- | ---------------------------- | ------------------------------------ |
| Anthropic               | `ANTHROPIC_API_KEY`        | Claude family                        |
| OpenAI                  | `OPENAI_API_KEY`           | GPT family                           |
| Google Gemini           | `GEMINI_API_KEY`           | Gemini models                        |
| Hugging Face            | `HF_TOKEN`                 | Open-source models via Inference API |
| GitHub Copilot          | IDE token (auto-discovered)  | No separate API key                  |
| Ollama                  | none (local)                 | Local or remote Ollama server        |
| Ollama Cloud            | `OLLAMA_API_KEY`           | Managed Ollama endpoints             |
| Azure AI Foundry        | `AZURE_AI_FOUNDRY_API_KEY` | Microsoft Azure-hosted models        |
| Azure Resource          | `AZURE_RESOURCE_API_KEY`   | File-based Azure deployment config   |
| Amazon Bedrock          | AWS credentials              | AWS SigV4 signing                    |
| Generic OpenAI          | `GENERIC_OPENAI_API_KEY`   | Any OpenAI-compatible endpoint       |
| Microsoft Foundry Local | local endpoint               | Local Windows AI backend             |
| XAI                     | `XAI_API_KEY`              | xAI / Grok models                    |
| Model Router            | cluster of providers         | Virtual provider that routes prompts |

Use `/provider show` to inspect currently configured providers and their
settings, and `/provider router` (or `/router help`) to set up the Model Router.

---

## 3.1 Configuration snapshots

The `/config` slash command family lets you inspect and back up your global
`ragent.json` without leaving the TUI.

```text
/config show          # show working dir, data/config dirs, storage, code index, memory, agents
/config save          # snapshot the current global ragent.json under ~/.config/ragent/saves/
/config list          # browse saved snapshots and restore one with Enter
```

`/config show` is useful when checking which config file is active or whether
the code index and memory directories exist.

---

## 3.2 Tool visibility toggles

Some large tool families are hidden from the model by default to keep prompts
small. Use `/tools` to list switches and `/tools <switch> on|off` to enable or
disable them persistently.

```text
/tools show
/tools browser on
/tools office on
/tools github off
```

Valid switches: `office`, `github`, `gitlab`, `teams`, `agents`, `plan`,
`codeindex`, `masterfetch`, `browser`. Changes are saved to `ragent.json`.

---

## 3.3 Autopilot

Autopilot lets the agent continue iterating autonomously after each turn until
it calls `task_complete`, hits a user-defined limit, or you run `/autopilot off`.

```text
/autopilot on
/autopilot on --max-tokens 16000 --max-time 300
/autopilot status
/autopilot off
```

The status bar shows `AutoPilot:✓` when enabled and `AutoPilot:✗` when
disabled.

---

## 3.4 Startup timings and run-cost banner

After launch, ragent records how long each startup stage took. Run `/startup`
at any time to see a breakdown such as config load, provider health check,
code-index startup, and session creation.

When an agent run completes, a transient banner appears at the top showing
`run complete · input+output tokens · $cost · duration`. The full details are
always written to the log panel. The banner is dismissed by any keypress; if
you start typing the next prompt, the first character is preserved rather than
being swallowed.

---

## 4. Issuing prompts to ragent

Click in the **Input** panel at the bottom and type your prompt.

Press **`Enter`** to send.

### Multi-line prompts

- Press **`Shift+Enter`** or **`Alt+Enter`** to insert a newline without sending.
- Press **`Enter`** on an empty line or at the end of your message to submit.

### Example prompts

```text
Explain the purpose of the ragent-types crate
```

```text
Refactor src/main.rs to split the CLI argument parsing into a separate module
```

```text
Add unit tests for the bash permission command-name extraction logic
```

### Mentions and attachments

- Type **`@`** to open the file picker and mention a file in your prompt.
- Press **`Alt+V`** to paste an image from the clipboard as an attachment.
- Pending attachments appear above the input box before you send.

### Clipboard and selections

- **Text selection** — click-and-drag (or hold Shift and use arrow keys) to
  select text in the input, message, log, or side panels.
- **`Ctrl+C`** — copy the current selection to the clipboard.
- **`Ctrl+X`** — cut the current selection from the input.
- **`Ctrl+V`** — paste at the cursor, replacing any active selection. Carriage
  returns (`\r`) are stripped so Windows-copied text behaves consistently.
- **Terminal bracketed paste** — if your terminal emits a bracketed-paste event,
  it behaves exactly like `Ctrl+V`: it strips `\r` and replaces the active
  selection.
- **Right-click** — opens a context menu for the current selection. In the input
  pane it offers **Copy**, **Cut**, and **Paste**. In provider-setup dialogs
  (EnterKey, GitLabSetup, TelemetrySetup) it pastes the clipboard into the
  active field.
- **`/clip`** — copies the entire rendered contents of the message window (the
  plain-text transcript rows you see on screen) to the system clipboard in one
  step.

### Permission prompts

When ragent wants to run a shell command, write a file, or perform another
protected action, a permission dialog appears at the top of the screen. You
have 120 seconds to:

- **`y`** — allow once
- **`a`** — allow always for this session
- **`n`** — deny

The dialog title shows a live countdown (e.g. `Permission Required (1:45 remaining)`).

### The input queue and the ALT-Q queue-control menu

The input field stays live while the agent is executing. Instead of rejecting
your message with `busy - wait for the current turn to finish`, pressing
**`Enter`** appends it to a bounded FIFO **input queue** (maximum 32 entries).
A two-digit, zero-padded counter appears immediately before the prompt, for
example:

```text
03> refactor the parser
```

The oldest queued entry runs automatically when the running turn ends, and the
counter decrements each time. No counter is shown when the queue is empty.

A queued entry is added to the **input history** the moment you press `Enter`
(so `Up` recalls it immediately), not when it runs, and it is not echoed into
the message window until it is actually dispatched.

A **slash command** (`/…`) is queued exactly like a plain message while a turn
is running (FR-017 amendment): pressing `Enter` on `/status`, `/agent`, or a
`queue`-style command appends it to the queue and it runs at the next turn
boundary (or immediately via `Alt+Q` → `Next` / `/queue next`). A synchronous
command leaves the boundary free, so a run of consecutive queued commands
executes back-to-back rather than stalling behind the first. At queue capacity
the command is rejected and restored to the input field, so nothing is lost.
Only **bang commands** (`!…`) and **teammate-targeted messages** keep the
`busy - wait for the current turn to finish` refusal.

Press **`Alt+Q`** during a run to open the **queue-control menu** — a centred
overlay with exactly four options. Use **`Up`**/**`Down`** to move the highlight
and **`Enter`** to activate the highlighted row:

| Option        | Behaviour |
| ------------- | --------- |
| `Next`        | Stops the running turn and immediately dispatches the **oldest** queued entry, then closes the menu. FIFO order is preserved. `Next` is shown as non-selectable when the queue is empty (the highlight skips it). |
| `Stop` / `Resume` | While a turn is executing the row reads **`Stop`**: it halts the agent exactly as pressing `Escape` would and does **not** advance the queue. When the agent is already stopped the same row reads **`Resume`**: it restarts the interrupted work. |
| `Clear`       | Opens a `Clear the input queue?` confirmation dialog with `Yes` / `No`. **`No` is the default**, so pressing `Enter` without changing the selection changes nothing; only `Yes` empties the queue. Selecting `No` or pressing `Esc` closes the dialog and leaves the queue, the input buffer, and the staged attachments untouched. |
| `Show`        | Opens a scrollable **queue panel** listing every queued entry oldest-first with a block cursor (see below). |

`Esc` dismisses the menu taking no action — the input buffer, staged
attachments, the queue, and the running turn are all left untouched. Queued
entries survive a cancelled turn, an agent error, or a compaction; they are
only removed when they are dispatched or explicitly cleared.

#### The `Show` queue panel

The `Show` row opens a panel that lists every queued entry, oldest first, with a
block cursor marking the highlighted entry. Use it to inspect, reorder, or prune
the queue without leaving the chat screen:

| Key      | Behaviour |
| -------- | --------- |
| `Up` / `Down` | Move the block cursor up and down the list; the panel scrolls to keep the highlighted entry visible. |
| `Enter`  | Moves the highlighted entry **one step toward the front** of the queue, so it runs sooner, and keeps the cursor on it. Press `Enter` again to walk it further forward; at the front it is a no-op. The order of every other entry is preserved. |
| `Del`    | Removes the highlighted entry from the queue (the counter decrements). |
| `Esc`    | Dismisses the panel. |

Only `Esc` closes the panel; `Enter`, `Del`, and the navigation keys keep it
open and repaint it immediately. The panel never touches your in-progress draft
or staged attachments.

There is also a `/queue` slash command that inspects the same queue from the
prompt:

| Sub-command  | Behaviour |
| ------------ | --------- |
| `/queue list` or `/queue` | Lists the queued entries in submission order (oldest first) with the queue depth. |
| `/queue clear` | Empties the queue immediately; the counter disappears. |
| `/queue next`  | Dispatching the oldest entry immediately. While a turn is still executing the action is deferred to the turn boundary (it never overlaps a running turn or a compaction); use `Alt+Q` → `Next` to stop the current turn and run it now. |
| `/queue help`  | Shows the sub-command usage. |

---

## 5. Goal-driven loops with `/loop`

The **`/loop`** slash command runs a goal-driven agentic loop: ragent picks
the agent you name, then iterates **plan → act → observe** — planning the
next action, executing tool calls, feeding each observation back into the
model's context — until a stop condition fires. Four stop conditions end the
loop, each rendered as a status-aware end banner with the iteration count:

| Termination status | Stop condition |
| ------------------ | -------------- |
| `completed` | Goal achieved (model responded without tool calls; when a verification command is configured it must exit successfully) |
| `error` | An unrecoverable stage failure (provider transport, tool panic, context overflow, permission hard-deny); the failed stage is **not** retried |
| `budget_exhausted` | The step limit or token-cost budget was consumed **before** the next LLM request |
| `interrupted` | You pressed `Esc` mid-loop; the session stays resumable and a rollback offer may follow |

### Usage

```text
/loop                              # open the interactive setup dialog
/loop <agent> <goal text...>       # start immediately with documented defaults
```

The one-shot form activates the documented defaults: step limit **512**
(or `loop.max_steps` from `ragent.json`), config cost limit, checkpoints
**on**, no verification command, and no restrictions. Three flags override
the run's budgets: `--max-steps N`, `--cost_limit N` (or `--cost-limit N`),
and `--timeout N` (checkpoint-prompt seconds). An empty or
whitespace-only goal never starts a loop — the dialog shows an error naming
the missing field (`goal`) and re-opens for correction; the one-shot form
prints usage help instead.

### Goal format

A goal is the structured contract the loop iterates against. All fields
except the success state are optional:

| Field | Meaning |
| ----- | ------- |
| **Success state** | What must be true for the loop to complete (plain text) |
| **Verification command** | Runs automatically when the model responds without tool calls; success completes the loop, failure appends its output as the next observation (steps remaining) or ends `budget_exhausted` (no steps left) |
| **Scope boundaries** | Path globs the loop's file operations must stay inside |
| **Constraints** | Read-only path globs — writes are denied and returned as observations (anti-cheat: never satisfy the goal by modifying them) |
| **Tool set** | Restricted tool surface; calls outside the set are denied with a scope observation |
| **Budget** | Maximum iterations and maximum accumulated tokens before `budget_exhausted` |

### Setup dialog

With no arguments, `/loop` opens a setup dialog with an agent selector
(arrow keys), goal field, verification-command field, scope field
(comma-separated globs), constraints field, tool-set field, step-limit field (default 512), cost-limit field (from
config), and a checkpoints toggle
(default on). `Esc` cancels without starting anything and preserves all
entered values for the next `/loop` in the same session.

### Budget + checkpoint tunables

```json
{
  "loop": {
    "max_steps": 512,
    "cost_limit": 200000,
    "error_retry_allowance": 3,
    "checkpoints": true,
    "checkpoint_timeout_secs": 120
  }
}
```

- `max_steps` — iterations before `budget_exhausted` (FR-013); a per-agent
  `agent.<name>.max_steps` wins over the loop-level budget.
- `cost_limit` — accumulated input + output tokens before
  `budget_exhausted` (FR-014); `null` disables the gate.
- `error_retry_allowance` — consecutive recoverable tool failures before the
  loop stops with `error` (FR-012).
- `checkpoints` — force a human-approval prompt before destructive actions
  (file deletion, config writes, dependency installation, destructive git
  operations) even when an allow rule would auto-approve them (FR-015); a
  prompt timeout is treated as denial.
- `checkpoint_timeout_secs` — seconds a checkpoint prompt waits before the
  safe-choice denial applies.

### Rollback flow

When a loop terminates, ragent renders the change summary as a diffstat line
and offers a **one-key rollback**: `Enter` accepts and restores the pre-loop
workspace snapshot; `Esc` (or any other key) declines and keeps the changes.
Rollback is snapshot-only when the workspace is not inside a git repository —
the loop warns you before writing anything in that case.

---

## 6. Research with `/research`

Use the **`/research`** slash command to gather information from the web and
cross-reference it with local files.

```text
/research Rust async runtime design patterns
```

ragent will:

1. Search the web for relevant pages.
2. Fetch key pages.
3. Cross-reference findings against your local codebase and project memory.
4. Write a self-contained `RESEARCH.md` report.

### Common forms

```text
/research <topic>                # general research
/research compare tokio vs async-std
/research best practices for headless browser testing
/research create rust-patterns "Rust design patterns" --use-local
/research create rust-patterns --from-url https://example.com/article
/research create rust-patterns --from-file ./path/to/document.pdf
/research list
/research open rust-patterns
/research search "async runtime"
```

`/research list` prints an aligned table (NAME, TITLE, STATUS, CREATED,
MODIFIED) in a preformatted block so the columns are not distorted by
markdown rendering; `/research search` prints a bullet list of matches.

The `create` form supports a number of optional flags:

- `--from-url <URL>` — fetch the URL and use its content as the research subject.
- `--from-file <PATH>` — extract text from a local document (PDF, DOCX, XLSX,
  PPTX, ODT, ODS, ODP, TXT, MD) and use it as the subject.
- `--use-local` — include local files and prior specs in the analysis.
- `--use-specs` — cross-reference existing `specs/`.
- `--use-low-relevance` — keep low-relevance web sources instead of filtering
  them out.
- `--depth shallow|standard|deep` — control how broadly ragent searches.
- `--format report|executive-summary|comparison-table|source-bibliography|imrad`
  — choose the output artifact.

Reports are saved under `research/<name>/RESEARCH.md`. Open the report in the
TUI with `/research open <name>`. Quality-assurance detail (contradiction
graph, loci analysis, depth investigation, reconcile, tensions, audits) is
kept in a companion file at `research/<name>/CORPA.md`; it includes a
`Sources Reference` copy of the References Index so `[#N]` citations resolve
in both documents.

While the run is gathering, the pinned **Research Progress** message in the
message window does not list each captured URL (that previously flooded the
window). Instead it maintains a compact summary table with one row per
backend search engine (duckduckgo, brave, openalex, wikipedia, tavily, ...)
showing the counts of captured files by type (`page` / `pdf` / `yt`) and a
`languages` cell listing every currently acquired language with its article
count:

```text
[captures] Captured sources by search engine:
+----------+------+-----+----+-------+---------------------+
| engine   | page | pdf | yt | total | languages           |
+----------+------+-----+----+-------+---------------------+
| brave    |    3 |   0 |  0 |     3 | ENGLISH:2, FRENCH:1 |
| openalex |    0 |   1 |  0 |     1 | ENGLISH:1           |
+----------+------+-----+----+-------+---------------------+
```

A `total` row aggregates every engine. Per-URL capture lines still appear in
the log panel (not the message window) so individual fetch failures remain
traceable.

> Note: web research uses keyless search by default; a `TAVILY_API_KEY` or
> `LANGSEARCH_API_KEY` can be configured in `ragent.json` for higher-quality
> results.

---

## 7. Creating a specification and plan with `/spec`

The **`/spec`** slash command creates a tracked specification (`SPEC.md`) and
an implementation plan (`PLAN.md`) for a feature.

### Create a new spec

```text
/spec create add-user-authentication
```

ragent asks a few clarifying questions if needed, then generates:

- `specs/add-user-authentication/SPEC.md` — requirements, scope, acceptance criteria
- `specs/add-user-authentication/PLAN.md` — tasks, dependencies, effort estimates
- `specs/add-user-authentication/TESTPLAN.md` — manual test plan with `TC-NNN` test cases

### Regenerate plans after editing a spec

```text
/spec update add-user-authentication
```

`/spec update` re-reads the existing `SPEC.md` and regenerates `PLAN.md` and
`TESTPLAN.md` to match the current requirements. The `SPEC.md` file is not
modified; existing task IDs are preserved where unchanged. Archived specs
cannot be updated.

### List and validate specs

```text
/spec list
/spec validate add-user-authentication
/spec status add-user-authentication
```

### Track progress

As you implement tasks, ragent can update the spec status automatically:

```text
/spec task add-user-authentication T-001 completed
```

Specs are stored in the `specs/` directory by default. They are intended to be
user-managed working documents, not part of the main git tree unless you choose
to commit them.

---

## 7a. Scaffolding a new project with `/new`

The **`/new`** slash command scaffolds a brand-new project in the current
directory (it must be empty apart from ragent artifacts). The same surface is
available as the `ragent new` CLI subcommand.

### Scaffold a minimal project

```text
/new --language rust --type cmdline
```

The command streams each step into the message window as it completes
(guard, file emission, git init, remote status), then prints a summary with
the created files. Generated content:

- the ragent workspace: `.ragent/`, `specs/`, `log/`, `.gitignore`, `AGENTS.md`
- a runnable hello-world artifact set for any of the 26 supported application
  languages (`rust`, `python`, `go`, `typescript`, `shell`, ...) in a
  `library`, `cmdline`, `tui`, or `gui` layout, or sample-document stubs for
  data and build formats (`json`, `yaml`, `sql`, `cmake`, `maven`, ...);
  `/new help` lists every accepted value
- starter docs: `README.md`, `QUICKSTART.md`, `STATS.md`, `docs/`
- a git repository with an initial commit

### Add hosting and stacks

```text
/new --language rust --type cmdline --stack axum --github
```

`--stack axum` layers an axum server starter over the base Rust layout
(`warp`, `raylib`, `gtk4`, and `ratatui` are also known stacks); `--github` creates a private
GitHub repository, sets it as `origin`, and pushes. Use `--gitlab` instead
for GitLab. The two hosting flags are mutually exclusive, and a failed
remote step never undoes the local scaffold.

### Help

```text
/new help
```

Prints the usage page with the flag table and the supported language/type
values (derived from the scaffolder's registries, so they cannot drift).

---

## 7b. Managing plugins with `/plugins`

The **`/plugins`** slash command manages sandboxed Codex- and Claude
Code/Desktop-dialect plugins from the TUI. Enabled plugins contribute tools
(registered as `plugin_<id>_<tool>`) and slash commands. The same operations
are available from a shell as `ragent plugins <sub>`.

```text
/plugins list --verbose          # list plugins, state, contributions, telemetry
/plugins add ./my-plugin         # install (enabled; loads next session)
/plugins enable my-plugin        # load and register its tools/commands
/plugins test my-plugin          # isolated harness: load + invoke each tool once
/plugins disable my-plugin       # unload and deregister (files kept)
/plugins remove my-plugin        # uninstall (refused while enabled)
/plugins help                    # usage block
```

Reports render in the message window with a `From: /plugins <sub>` header.
`/plugins test` prints one `[ ok ]`/`[fail]` line per harness step and confirms
the harness was unloaded with the live session untouched. Any of `/plugins`
with no arguments, `/plugins help`, or an unrecognised subcommand prints the
usage block. Plugins are discovered under `.ragent/plugins/` (project) or
`~/.config/ragent/plugins/` (user-global); set `plugins.enabled: false` to make
the subsystem inert.

An enabled plugin's non-tool contributions are bridged too: `commands/*.md`
prompt commands appear in the `/` menu, `skills/` directories join skill
discovery, `mcpServers` connect as `<plugin-id>.<server>`, `agents/*.md`
profiles join agent discovery, and `hooks` (declared inline or in a `hooks.json`
file) fire on the matching session lifecycle events. `list` shows a count column
for each kind; `add` records the plugin enabled.

See [`docs/howtos/slashcommands/plugins.md`](docs/howtos/slashcommands/plugins.md).

---

## 8. Stopping the agent with the Escape key

While ragent is actively streaming a response or running tools, press
**`Escape`** to cancel the current operation.

- The LLM stream stops immediately.
- In-progress tool calls are abandoned.
- You can type a new prompt right away.

`Escape` does **not** quit the application; it only interrupts the current
agent step.

---

## 9. Quitting ragent

To exit the TUI safely:

1. Make sure the input box is focused.
2. Press **`Ctrl+D`**.
3. If prompted, confirm with **`Ctrl+D`** again.

Or use the slash command:

```text
/quit
```

You can also press **`Ctrl+C`** to arm quit mode, then **`Ctrl+D`** to confirm.

---

## 10. Log panel — `Alt+L`

Press **`Alt+L`** to toggle the **Log panel** on the right side of the screen.

The Log panel shows a time-stamped, color-coded stream of runtime events:

| Prefix  | Meaning                  |
| ------- | ------------------------ |
| `INF` | General information      |
| `TUL` | Tool call / tool result  |
| `WRN` | Warning                  |
| `ERR` | Error                    |
| `CMP` | Context-compaction event |

Log entries include the short session ID and step number when available, e.g.
`[a3f7:12]`. This makes it easy to correlate log lines with message bubbles in
the main chat pane.

### Interacting with the Log panel

- **`Scroll`** with the mouse wheel.
- **`Drag`** the scrollbar thumb to jump through long logs.
- **`Click`** on a log line to start a text selection; copy with `Ctrl+C`.
- Press **`Alt+L`** again to hide the panel.

The scrollbar gutter runs along the right edge of the panel. Dragging it works
the same way as the scrollbar in the main message pane.

When both Log and Profile panels are open, the side area is split vertically
with Log on top.

---

## 11. Profile panel — `Alt+P`

Press **`Alt+P`** to toggle the **Profile panel** on the right side of the
screen.

The Profile panel shows live performance data collected by the agent-loop
profiler:

- **uptime** — how long the current session has been running
- **samples** — number of profiler samples taken
- **ops** — number of distinct operations measured
- A table of operations sorted by **self time**, showing:
  - count
  - average milliseconds
  - total milliseconds
  - self milliseconds
  - max milliseconds
  - last milliseconds
  - operation name

Use this panel to spot slow operations, repeated work, or unexpectedly long
tool calls.

### Interacting with the Profile panel

- **`Scroll`** with the mouse wheel.
- **`Drag`** the scrollbar thumb.
- Press **`Alt+P`** again to hide the panel.

The scrollbar gutter runs along the right edge of the panel and can be dragged
or clicked to jump to a position in the report.

The Profile panel is mutually exclusive with TASKS and Memory panels: only one
of Log/Profile/TASKS/Memory occupies the side column at a time (Log and Profile
can be shown together).

---

## 12. TASKS panel — `Alt+T`

Press **`Alt+T`** to toggle the **TASKS panel** on the right side of the screen.

The TASKS panel lists tasks for the current session, fetched from the
SQLite-backed storage on every render. Each row shows:

```text
[<STATUS>] <subject> (owner) [blocked by #id, …]
```

Status colors:

| Status          | Color  |
| --------------- | ------ |
| `pending`     | Yellow |
| `in_progress` | Cyan   |
| `completed`   | Green  |
| blocked (derived) | Red    |

A task is **blocked** when its `blocked_by` list is non-empty and at least one
blocker is not yet `completed`. Blocked-ness is derived at read time — there is
no `blocked` status value (FR-005). When a task is `in_progress` and has an
`active_form`, it is shown as an indented sub-line beneath the subject.

### Managing Tasks

Create, update, or list tasks with the `task_create` / `task_update` /
`task_list` tools or the `/task` slash command:

```text
/task add Implement token bucket rate limiter
/task update task-abc123 --status completed
/task list
```

You can also use the tool directly in a prompt:

```text
Create a task list for adding OAuth2 login support
```

ragent will call `task_create` to add the items, and the TASKS panel updates
immediately.

### Interacting with the TASKS panel

- **`Scroll`** with the mouse wheel.
- **`Drag`** the scrollbar thumb to move through a long task list.
- Press **`Alt+T`** again to hide the panel.

The scrollbar gutter runs along the right edge of the panel.

---

## 13. Memory panel — `Alt+M`

Press **`Alt+M`** to toggle the **Memory panel** on the right side of the screen.

The Memory panel surfaces three sources of project and user memory:

1. **Project Memory** — `.ragent/memory/MEMORY.md`
2. **Project Analysis** — `.ragent/memory/PROJECT_ANALYSIS.md` (if it exists)
3. **User Memory** — `~/.ragent/memory/MEMORY.md`

It also shows a summary line when the structured-memory SQLite store contains
entries:

```text
Structured memories: 42
```

Memory files are plain Markdown. ragent reads them automatically on startup
and can update them with `memory_write` / `memory_replace` during a session.

In addition, the `memory_store` tool writes **structured memories** to the
SQLite store with a category, tags, and confidence score. Successful writes now
report `stored: true`, and the TUI summary and Memory panel reflect the update.

### Typical uses

- Store project conventions that should persist across sessions.
- Keep a running analysis of the architecture.
- Remember user preferences (e.g. "always use `anyhow::Result`").

### Updating memory

```text
/memory write project "Use tokio::sync::RwLock for shared state in this codebase"
```

Or simply ask ragent:

```text
Remember that we prefer tracing over println in this project
```

### Interacting with the Memory panel

- **`Scroll`** with the mouse wheel.
- **`Drag`** the scrollbar thumb.
- Press **`Alt+M`** again to hide the panel.

The scrollbar gutter runs along the right edge of the panel.

## Context panel — `Alt+C`

Press **`Alt+C`** to toggle the **Context panel** on the right side of the
screen. It shows a live, quantified breakdown of what currently occupies the
active session's context window:

- **Context window** — the advertised capacity of the currently selected
  model, shown directly above the **Sent to model** row so the denominator
  for every percentage is explicit.
- **Sent to model** — the provider-reported input tokens of the most recent
  LLM request, i.e. the actual context size the model received last turn
  (the same figure the `ctx:` status-bar indicator shows). It updates as
  each message is sent and stays at `0tk` until the first turn completes.
- **System prompt** — the assembled prompt sent to the model (agent prompt,
  project context, memory injection, skills catalog), with indented
  sub-rows for the `skills`, `memory` and `agents.md` contributions.
- **Tool catalog** — the serialised size of the tool definitions (names,
  descriptions, parameter schemas) visible to the model.
- **Tool metadata** — the per-tool JSON wire-envelope overhead added on top
  of the raw catalog.
- **History** — the conversation history token estimate and message count.
- **Total** — the sum of the top-level partitions, plus the remaining
  free headroom.

Each row shows the estimate in tokens (raw byte count converted with the
standard ~4-bytes-per-token heuristic, so it stays comparable with the
`ctx:` indicator in the status bar, which shows the provider-reported prompt
tokens from the last turn) and a percentage bar
of the active model's advertised context window. When the provider does not
report a window size, rows show `unknown` alongside the absolute counts.
Values refresh automatically after messages, tool calls, model switches and
compaction.

### Interacting with the Context panel

- **`Scroll`** with the mouse wheel.
- **`Drag`** the scrollbar thumb.
- Press **`Alt+C`** again to hide the panel.

The Context panel is display-only: its contents are never sent to the model.

---

## Side panel quick reference

| Key       | Panel     | Purpose                                           |
| --------- | --------- | ------------------------------------------------- |
| `Alt+L` | Log       | Runtime events, tool calls, warnings, errors      |
| `Alt+P` | Profile   | Live agent-loop profiler output                   |
| `Alt+T` | TASKS    | Session tasks and status                         |
| `Alt+M` | Memory    | Project/user memory and structured-memory summary |
| `Alt+O` | Telemetry | OpenTelemetry metrics and counters                |
| `Alt+C` | Context   | Token breakdown of the context window             |

Log and Profile can be shown together (Log above, Profile below). The other
panels are mutually exclusive: opening one closes the others.

All side panels support mouse scrolling and scrollbar dragging. Press the same
shortcut again to close the panel.

---

## Common keybindings

| Key                             | Action                                      |
| ------------------------------- | ------------------------------------------- |
| `Enter`                       | Send prompt                                 |
| `Shift+Enter` / `Alt+Enter`   | New line in input                           |
| `Escape`                      | Cancel current agent operation              |
| `Ctrl+D`                      | Quit ragent                                 |
| `Ctrl+C`                      | Copy selection / arm quit                   |
| `Alt+L`                       | Toggle Log panel                            |
| `Alt+P`                       | Toggle Profile panel                        |
| `Alt+T`                       | Toggle TASKS panel                           |
| `Alt+M`                       | Toggle Memory panel                         |
| `Alt+O`                       | Toggle Telemetry panel                      |
| `Alt+C`                       | Toggle Context panel                        |
| `Alt+Q`                       | Open the queue-control menu (Next/Stop/Clear/Show) |
| `Alt+V`                       | Paste image from clipboard                  |
| `Alt+Y`                       | Toggle YOLO mode on/off                     |
| `@`                           | Open file mention picker                    |
| `/`                           | Open slash-command menu                     |
| `?`                           | Show keybindings help (when input is empty) |

---

## Next steps

- Read the full `QUICKSTART.md` for CLI, server, and configuration options.
- Browse the per-command howtos in `docs/howtos/slashcommands/` (INDEX.md lists every command).
- See `docs/custom-agents.md` to create your own agent profiles.
- See `docs/howtos/teams.md` to coordinate multi-agent teams.
- Run `ragent --help` for a complete list of command-line options.
