# Research System User Guide

The ragent Research System is the first stage of the standard ragent
workflow: **research → spec → implement**. It runs a gathering session
that combines web search and local file cross-referencing, then writes a
self-contained `RESEARCH.md` you can reference later from a spec.

## Quick Start

### TUI

```text
/research rust-async async/await idioms in stable Rust
```

The slash command streams JSON-line progress into the log panel:

```text
ragent-research: {"kind":"phase","payload":{"phase":"web"}}
ragent-research: {"kind":"web","payload":{"url":"https://...","title":"..."}}
ragent-research: {"kind":"phase","payload":{"phase":"local"}}
ragent-research: {"kind":"local","payload":{"path":"src/lib.rs","score":3}}
ragent-research: {"kind":"phase","payload":{"phase":"assemble"}}
ragent-research: {"kind":"done","payload":{"total_sources":7}}
```

### CLI

```bash
ragent research create rust-async "async/await idioms in stable Rust"
ragent research list
ragent research open rust-async
ragent research search "async"
ragent research show rust-async
ragent research archive rust-async
ragent research delete rust-async --yes
```

CLI output is the same `ragent-research:` JSON line protocol so you can
pipe through `jq` or other tools:

```bash
ragent research create rust-async "async/await" 2>&1 \
  | grep ragent-research \
  | jq -R 'fromjson? | .payload'
```

### HTTP

When the server is running (`ragent serve`), the research API is
mounted under the auth-protected prefix:

| Verb   | Path                | Body / Query                       | Description |
|--------|---------------------|------------------------------------|-------------|
| GET    | `/research`         | —                                  | List every item (excludes archived). |
| POST   | `/research`         | `{name, topic, sources_dir?, template?}` | Create + run a gathering session. |
| GET    | `/research/<name>`  | —                                  | Show a single item with related hits. |
| DELETE | `/research/<name>`  | `?confirm=delete-<name>`           | Delete an item (confirmation token required). |

Example:

```bash
curl -X POST http://127.0.0.1:9100/research \
  -H "Authorization: Bearer $RAGENT_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"name":"rust-async","topic":"async/await idioms in stable Rust"}'
```

## Slash Command Examples (TUI)

The `/research` slash command family mirrors the `ragent research` CLI
one-for-one. Every subcommand below works identically from the TUI chat line;
each emits the same `ragent-research:` JSON-line protocol into the log panel
and renders a human-readable summary into the assistant message stream.

Run `/research help` at any time to print the full subcommand table in the
chat panel:

```text
/research help
```

### `/research help` — show available subcommands

Prints the built-in help table (subcommands, flags, output format). Useful as
a quick refresher without leaving the TUI.

```text
/research help
```

### `/research create` — run a gathering session

Starts a research session under `research/<name>/`, streams JSON-line progress
(web → local → specs → assemble → finalize), and writes a self-contained
`RESEARCH.md`.

```text
/research create rust-async async/await idioms in stable Rust
```

Short form — omit `create` entirely and ragent treats the first token as the
name and the remainder as the topic:

```text
/research rust-async async/await idioms in stable Rust
```

With optional flags:

```text
/research create litertlm-port how to add a LiteRT-LM backend --sources-dir notes/
/research create deepdive-async tokio runtime internals --template deepdive --use-local --use-specs
/research create deepdive-async tokio runtime internals --iterations 5 --depth deep --format executive-summary
```

| Flag | Purpose |
|---|---|
| `--sources-dir <path>` | Pull additional local files from an extra directory (FR-019). |
| `--template <name>` | Use a template from `research/_templates/<name>.md` (FR-020). |
| `--use-local` | Enable local-file scanning (in-project + `--sources-dir`). |
| `--use-specs` | Enable prior-spec cross-referencing. |
| `--iterations N` | Override the default maximum number of research iterations. |
| `--depth shallow|standard|deep` | Choose a preset iteration/source budget (`shallow`=1 it., `standard`=3, `deep`=5). |
| `--format report|executive-summary|comparison-table|source-bibliography` | Select the output artifact (default: `report`). |

By default, only web sources are gathered. Local and spec phases must be
explicitly requested with `--use-local` and `--use-specs`.

### `/research continue` — resume an in-progress session

Loads the saved `state.json` for an item and runs the next iteration of the
research loop. You can optionally add a follow-up requirement, which is appended
to the topic and added as a new sub-question (FR-004, T-012, T-014).

```text
/research continue tokio-runtime
/research continue tokio-runtime focus on io_uring integration
```

After it finishes, the assistant message prints:

```text
From: /research create
📝 Gathering sources for `rust-async`…
Topic: async/await idioms in stable Rust
Tip: run `/research list` once finished, or `/research open rust-async` to view the result.
```

### `/research list` — list every item

Lists non-archived research items as a table (name, title, status, created,
modified). Add `--all` to include archived ones.

```text
/research list
/research list --all
```

Example output rendered in the chat panel:

```text
From: /research list

NAME                  TITLE                              STATUS       CREATED                  MODIFIED
----------------------------------------------------------------------------------------
rust-async            async/await idioms in Rust         complete     2025-01-17T10:00:00Z     2025-01-17T10:05:00Z
tokio-runtime        tokio runtime internals            in-progress  2025-01-17T10:10:00Z    2025-01-17T10:12:00Z
```

### `/research open` — show the RESEARCH.md path

Prints the absolute path to the item's `RESEARCH.md` so you can open it in an
editor or paste it into a spec.

```text
/research open rust-async
```

Output:

```text
From: /research open
• Name: `rust-async`
• Title: async/await idioms in stable Rust
• Status: complete
• Path: `/home/me/proj/research/rust-async/RESEARCH.md`
```

### `/research search` — full-text search

Searches every `RESEARCH.md` (including sources) and returns ranked hits with
snippets. The query is free text — wrap multiple words in the natural command
flow (no quotes required).

```text
/research search async
/research search tokio runtime
```

Output:

```text
From: /research search

• rust-async — async/await idioms in Rust
    …`async fn` returns a `Future`…
• tokio-runtime — tokio runtime internals
    …`tokio::spawn` schedules…
```

### `/research show` — print full metadata

Dumps metadata for a single item: name, title, topic, status, timestamps, and
the full captured-sources table.

```text
/research show rust-async
```

Example output:

```text
From: /research show

Research item: rust-async
Title:         async/await idioms in stable Rust
Topic:         async/await idioms in stable Rust
Status:        complete
Created (UTC): 2025-01-17T10:00:00Z
Modified (UTC):2025-01-17T10:05:00Z

References (7):
  # 0  [web         ] https://doc.rust-lang.org/async  async/await book chapter  (2025-01-17T10:01:00Z)
  # 1  [web         ] https://tokio.rs/tokio/tutorial  Tokio tutorial             (2025-01-17T10:01:30Z)
  # 2  [local       ] src/lib.rs                       crate root                 (2025-01-17T10:02:00Z)
  # 3  [local       ] src/session.rs                   async session loop         (2025-01-17T10:02:10Z)
  # 4  [spec        ] specs/async-await/SPEC.md        related spec               (2025-01-17T10:02:30Z)
```

### `/research archive` — mark as archived

Moves the item to the terminal `archived` state. Archived items are excluded
from `/research list` unless you pass `--all`.

```text
/research archive rust-async
```

### `/research delete` — remove an item

Deletes the entire `research/<name>/` directory. Requires `--yes` to skip the
confirmation prompt (the TUI refuses without it).

```text
/research delete rust-async --yes
```

### Common patterns

End-to-end research → spec flow entirely from the TUI:

```text
/research rust-async async/await idioms in stable Rust
/research list
/research open rust-async
/spec create async-await Add async/await ergonomics --from-research rust-async
```

Re-run an existing item to refresh its `RESEARCH.md` (the session overwrites
the prior document):

```text
/research rust-async async/await idioms in stable Rust
```

## File Layout

```text
research/
├── INDEX.md                       derived cache (regenerated on every change)
├── _templates/                    optional research templates
│   └── deepdive.md
└── <name>/
    ├── RESEARCH.md                YAML frontmatter + 6 required sections
    └── sources/
        ├── web-NN.md
        ├── local-NN.md
        ├── spec-NN.md
        └── other-NN.md
```

The directory name is the validated research name — lowercase ASCII
letters/digits/hyphens, 3–64 chars, starting with a letter.

## RESEARCH.md Sections

Every `RESEARCH.md` has six numbered sections plus a References Index:

1. **Topic** — the description that triggered the research.
2. **Summary** — a one-paragraph overview of the captured evidence.
3. **Findings** — numbered findings, each citing references like `[#1]`.
4. **In-Project Cross-References** — relevant project files with one-line relevance notes.
5. **Open Questions** — items the LLM should answer in a follow-up.
6. **References Index** — markdown table of every captured source.

## Templates

Place a template at `research/_templates/<name>.md`. The session
substitutes the standard placeholders:

| Placeholder | Value |
|---|---|
| `{{title}}` | the research item's title |
| `{{topic}}` | the topic description |
| `{{date}}` | current UTC date (YYYY-MM-DD) |
| `{{name}}` | the research name |

Then run:

```bash
ragent research create rust-async "topic" --template deepdive
```

## Iterative Research Loop

The `researchext` engine drives research in iterations:

1. **Plan** — decompose the topic into focused sub-questions.
2. **Gather** — capture sources for pending sub-questions in parallel.
3. **Synthesize** — produce findings from the evidence.
4. **Verify** — check that each finding cites a source that supports it.
5. **Critique** — score the result and detect missing-link gaps.
6. **Stop or iterate** — continue until complete, out of budget, or no longer
   improving.

Iterations stop when every sub-question is answered, the iteration budget is
exhausted, or the evaluation score stops improving (unless `--depth deep` or
`--iterations` forces more work).

## Persistence and Resume

Every in-progress research item writes a `state.json` file alongside its
`RESEARCH.md`. The file contains the current plan, sub-question statuses,
captured sources, evaluation score, iteration count, evidence gaps, and follow-up
queries. Use `/research continue <name>` to resume from this state instead of
starting over (FR-009, T-013). Add an optional follow-up message to refine the
plan in-flight:

```text
/research continue tokio-runtime focus on io_uring integration
```

## Output Formats

The `--format` flag selects the artifact produced at the end of a session:

| Format | Description |
|---|---|
| `report` | Full multi-section `RESEARCH.md` (default). |
| `executive-summary` | One-page summary. |
| `comparison-table` | Comparison table across key entities. |
| `source-bibliography` | Standalone bibliography of all captured sources. |

## Research Loop Events

The engine emits structured JSON-line events for every phase:

| `kind` | Payload | Meaning |
|---|---|---|
| `plan_updated` | `{sub_questions}` | Topic decomposed into sub-questions. |
| `sub_question_status_changed` | `{id, status}` | Sub-question lifecycle change. |
| `web` | `{url, title}` | Web source captured. |
| `local` | `{path, score}` | Local file captured. |
| `source_failed` | `{source?, error}` | A fetch failed and was recorded. |
| `critic` | `{score?, gaps}` | Evaluation score and new gaps. |
| `verification` | `{passed, issues}` | Claim-to-source verification result. |
| `follow_up_queries` | `{queries}` | Bridge queries for missing links. |
| `iteration_completed` | `{iteration, score?}` | One loop iteration finished. |
| `done` | `{total_sources}` | Session complete. |

## Linking a Spec to Research

Drop a line at the top of `specs/<id>/PLAN.md`:

```markdown
research: rust-async
research: tokio-runtime
```

or create a spec pre-populated with the research link:

```text
/spec create async-await Add async/await ergonomics --from-research rust-async
```

The spec's `SPEC.md` frontmatter records the dependency and the body
includes a `## Related Research` section linking to the captured
`RESEARCH.md`.

## Error Recovery

| Symptom | Likely cause | Fix |
|---|---|---|
| `research item 'foo' already exists` | duplicate name (FR-016) | use `/research open foo` or `ragent research delete foo --yes` |
| `research item 'foo' not found. Closest matches: …` | typo (FR-018) | re-run with one of the suggested names |
| `research name 'AB' …` | failed FR-002 validation | names must be ≥ 3 lowercase ASCII chars starting with a letter |
| `research name '../etc' …` | path traversal (FR-017) | never supply path-traversal sequences; pick a clean name |

## Status Tracking

Research items have four lifecycle states:

- `draft` — created, no gathering run yet
- `in-progress` — a session is mid-flight; `RESEARCH.md` is being written
- `complete` — `RESEARCH.md` is fully written and references are indexed
- `archived` — terminal; excluded from default list (pass `--all` to include)

Transition by running `/research archive <name>` (or
`ragent research archive <name>`). Re-running `/research create <name>`
on a non-archived item refreshes the existing document.
