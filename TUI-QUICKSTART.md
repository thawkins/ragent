# TUI Quick Start Guide for ragent

A hands-on guide to using **ragent** through its full-screen terminal UI.

---

Release notes live in [CHANGELOG.md](CHANGELOG.md).

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
| `--model ollama_cloud/kimi-k2.7-code` | Start with a specific model                          |
| `--agent rust-coder`                 | Start with the `rust-coder` agent profile          |
| `--yes`                              | Auto-approve all permission prompts (YOLO-style)     |
| `--no-tui`                           | Skip the TUI and run a single prompt in the terminal |
| `--log`                              | Open the TUI with the log panel already visible      |

### Example

```bash
ragent --model ollama_cloud/kimi-k2.7-code --agent rust-coder
```

You can also open the provider setup directly from the input box:

```text
/provider
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
- Each crate builds one `tests/all.rs` target that `#[path]`-includes every
  `tests/test_*.rs` file; add a new test as one more `#[path] ... mod ...;` line
- Run tests with `cargo test --workspace`
- Run one file with `cargo test -p <crate> --test all test_foo::`

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

Every list in these dialogs (provider, model, router tier, thinking level, Azure
resource, router provider/bucket) wears the same calm-slate chrome as the
`/plugins` and `/connectors` catalogues: an accent frame titled with the step and
its subject, a heading strip with a live count on the left and the confirm key on
the right, a pinned column band, a cursor-filled zebra row list, and a muted
key-hint footer. Each list **windows around its cursor**, so a list longer than
the dialog always keeps the highlighted row painted and the legend reads
`Showing a-b of N`; the thinking-level list windows two lines at a time because
each level is two lines tall. A mouse wheel notch over an open dialog steps the
cursor one entry exactly like `Up`/`Down` (same handler), and a notch anywhere
else is swallowed so the chat and side panels behind the modal never scroll under
it.

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
ollama_cloud/kimi-k2.7-code ●
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
| Azure Resource          | per-entry `api_key_env`    | File-based Azure deployment config   |
| Amazon Bedrock          | AWS credentials              | AWS SigV4 signing                    |
| Generic OpenAI          | `GENERIC_OPENAI_API_KEY`   | Any OpenAI-compatible endpoint       |
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

## 3.2 Tool listing

Every tool the registry holds is advertised to the model. `/tools` shows the
help block and `/tools list` (alias `/tools show`) prints one row per registered
tool with its source and description.

```text
/tools
/tools list
```

A tool you expect but do not see is not registered: the subsystem that owns it is
inactive. The ten codeindex tools, for example, appear only while the code index
is on (`/codeindex on`).

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

`Alt+O` toggles autopilot without leaving the keyboard: it drives the same
state machine as `/autopilot on|off`, but because a key chord cannot carry flags
it applies a default 1800-second (`--max-time`) limit — a stray keystroke cannot
start an unbounded run. Use `/autopilot on` with no flags when you want a run
with no time limit. Pressing `Alt+O` again stops the run.

The status bar shows `AUTO:✓ (Alt+O)` when enabled and `AUTO:✗ (Alt+O)` when
disabled; the indicator is bold because it changes how the agent loop runs.

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
Only **bang commands** (`!…`) keep the `busy - wait for the current turn to
finish` refusal.

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
4. Write a self-contained dated `RESEARCH-<YYYY-MM-DD>.md` report.

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
- `--format report|executive-summary|comparison-table|source-bibliography|imrad|newsletter`
  — choose the output artifact. `newsletter` emits only the reader-facing
  sections (topic, executive summary, top 10 implications, open questions,
  concepts, findings, findings relationship diagram) and drops the audit-trail
  blocks, so the document reads as a newsletter rather than a report.

Reports are saved under `research/<name>/RESEARCH-<YYYY-MM-DD>.md`. The primary
report uses the item's creation date, so it is stable from creation onwards;
each later run (including a same-day `/research update`) writes its own dated
file and leaves the earlier one in place. Open the report in the TUI with
`/research open <name>` - it resolves whichever report exists rather than
assuming a file name. Quality-assurance detail (contradiction graph, loci
analysis, depth investigation, reconcile, tensions, audits) is kept in the
companion `CORPA-<YYYY-MM-DD>.md`; it includes a `Sources Reference` copy of the
References Index so `[#N]` citations resolve in both documents. An item created
by an older ragent keeps its undated `RESEARCH.md` / `CORPA.md`, which resolves
the same way.

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

## 12. TASKS panel — `Alt+I`

Press **`Alt+I`** to toggle the **TASKS panel** on the right side of the screen.

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

Create, update, or list tasks with the `TaskCreate` / `TaskUpdate` /
`TaskList` tools or the `/task` slash command:

```text
/task add Implement token bucket rate limiter
/task update task-abc123 --status completed
/task list
```

You can also use the tool directly in a prompt:

```text
Create a task list for adding OAuth2 login support
```

ragent will call `TaskCreate` to add the items, and the TASKS panel updates
immediately.

### Interacting with the TASKS panel

- **`Scroll`** with the mouse wheel.
- **`Drag`** the scrollbar thumb to move through a long task list.
- Press **`Alt+I`** again to hide the panel.

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

### Clearing this project's memories

`/memory clear` opens a `Clear this project's memory?` dialog with `Yes` / `No`
(`No` is selected by default, so a stray `Enter` cannot clear memory). Only
memories scoped to the current project directory are deleted; other projects'
entries are never touched.

```text
/memory clear
```

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
| `Alt+I` | TASKS    | Session tasks and status                         |
| `Alt+M` | Memory    | Project/user memory and structured-memory summary |
| `Alt+O` | Autopilot | Toggle autonomous operation                       |
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
| `Alt+I`                       | Toggle TASKS panel                           |
| `Alt+M`                       | Toggle Memory panel                         |
| `Alt+O`                       | Toggle autopilot mode                       |
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
