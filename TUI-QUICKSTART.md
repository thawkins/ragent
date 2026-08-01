# TUI Quick Start Guide for ragent

A hands-on guide to using **ragent** through its full-screen terminal UI.

This guide reflects **v0.1.0-beta.28** behaviour. Recent highlights include the
Model Router status bar, editable provider keys, configuration snapshots,
`/autopilot`, `/startup` timings, browser tool visibility, structured-memory
fixes, and research seeded from URLs or local files.

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

### Permission prompts

When ragent wants to run a shell command, write a file, or perform another
protected action, a permission dialog appears at the top of the screen. You
have 120 seconds to:

- **`y`** — allow once
- **`a`** — allow always for this session
- **`n`** — deny

The dialog title shows a live countdown (e.g. `Permission Required (1:45 remaining)`).

---

## 5. Research with `/research`

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
TUI with `/research open <name>`.

> Note: web research uses keyless search by default; a `TAVILY_API_KEY` or
> `LANGSEARCH_API_KEY` can be configured in `ragent.json` for higher-quality
> results.

---

## 6. Creating a specification and plan with `/spec`

The **`/spec`** slash command creates a tracked specification (`SPEC.md`) and
an implementation plan (`PLAN.md`) for a feature.

### Create a new spec

```text
/spec create add-user-authentication
```

ragent asks a few clarifying questions if needed, then generates:

- `specs/add-user-authentication/SPEC.md` — requirements, scope, acceptance criteria
- `specs/add-user-authentication/PLAN.md` — tasks, dependencies, effort estimates

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

## 7. Stopping the agent with the Escape key

While ragent is actively streaming a response or running tools, press
**`Escape`** to cancel the current operation.

- The LLM stream stops immediately.
- In-progress tool calls are abandoned.
- You can type a new prompt right away.

`Escape` does **not** quit the application; it only interrupts the current
agent step.

---

## 8. Quitting ragent

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

## 9. Log panel — `Alt+L`

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

## 10. Profile panel — `Alt+P`

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

The Profile panel is mutually exclusive with TODO and Memory panels: only one
of Log/Profile/TODO/Memory occupies the side column at a time (Log and Profile
can be shown together).

---

## 11. TODO panel — `Alt+T`

Press **`Alt+T`** to toggle the **TODO panel** on the right side of the screen.

The TODO panel lists TODO items for the current session, fetched from the
SQLite-backed storage on every render. Each row shows:

```text
[<STATUS>] <title>
```

Status colors:

| Status          | Color  |
| --------------- | ------ |
| `PENDING`     | Yellow |
| `IN_PROGRESS` | Cyan   |
| `DONE`        | Green  |
| `BLOCKED`     | Red    |

### Managing TODOs

Create, update, or remove TODOs with the `todo_write` tool or the `/todo` slash
command:

```text
/todo add Implement token bucket rate limiter
/todo status 3 DONE
/todo remove 2
/todo list
```

You can also use the tool directly in a prompt:

```text
Create a TODO list for adding OAuth2 login support
```

ragent will call `todo_write` to add the items, and the TODO panel updates
immediately.

### Interacting with the TODO panel

- **`Scroll`** with the mouse wheel.
- **`Drag`** the scrollbar thumb to move through a long TODO list.
- Press **`Alt+T`** again to hide the panel.

The scrollbar gutter runs along the right edge of the panel.

---

## 12. Memory panel — `Alt+M`

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

---

## Side panel quick reference

| Key       | Panel     | Purpose                                           |
| --------- | --------- | ------------------------------------------------- |
| `Alt+L` | Log       | Runtime events, tool calls, warnings, errors      |
| `Alt+P` | Profile   | Live agent-loop profiler output                   |
| `Alt+T` | TODO      | Session TODO items and status                     |
| `Alt+M` | Memory    | Project/user memory and structured-memory summary |
| `Alt+O` | Telemetry | OpenTelemetry metrics and counters                |

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
| `Alt+T`                       | Toggle TODO panel                           |
| `Alt+M`                       | Toggle Memory panel                         |
| `Alt+O`                       | Toggle Telemetry panel                      |
| `Alt+V`                       | Paste image from clipboard                  |
| `Alt+Y`                       | Toggle YOLO mode on/off                     |
| `@`                           | Open file mention picker                    |
| `/`                           | Open slash-command menu                     |
| `?`                           | Show keybindings help (when input is empty) |

---

## Next steps

- Read the full `QUICKSTART.md` for CLI, server, and configuration options.
- See `docs/custom-agents.md` to create your own agent profiles.
- See `docs/howtos/howto_teams.md` to coordinate multi-agent teams.
- Run `ragent --help` for a complete list of command-line options.
