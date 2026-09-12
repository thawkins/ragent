# /research

> Research system: /research create [--mode tiered|supervisor|competitive] ... | list | open | search | show | delete | archive | cluster

## Overview

`/research` drives the structured research pipeline: web search plus local file
cross-referencing, synthesis, and a self-contained RESEARCH.md output. Projects
are named, persisted, and can be listed, resumed, clustered, exported, and
imported. Create accepts a rich flag set covering depth, tier, mode, per-stage
models, concurrency, timeouts, and source limits. The TUI runs the pipeline on
a background task while the status bar shows `[wait] research: <name>...`.

## Syntax

```
/research help
/research create <name> <topic> [flags]
/research list [--all] [--json]
/research show <name> [--json]
/research open <name>
/research search <query> [--json]
/research delete <name> [--yes]
/research archive <name>
/research resume <name>
/research continue <name> [--message <text>]
/research update <name>
/research cluster <name> [--force]
/research export <name> [--output <path>]
/research import <path> [--name <name>]
/research config
```

The first positional after `create` is the project name; everything after it is
the topic. The parser splits on whitespace and honours double quotes, so a
topic can be quoted as one argument.

## Subcommands

| Form | Description |
|---|---|
| `create <name> <topic>` | Starts a research project. See the flag table below. |
| `list [--all] [--json]` | Lists projects; `--all` includes archived, `--json` emits JSON. |
| `show <name> [--json]` | Shows project state and findings; `--json` emits JSON. |
| `open <name>` | Opens the project's RESEARCH.md in the log panel. |
| `search <query> [--json]` | Searches stored research content. |
| `delete <name> [--yes]` | Deletes a project; `--yes` skips confirmation. |
| `archive <name>` | Moves a project to the archive. |
| `resume <name>` | Resumes an interrupted or archived project. |
| `continue <name> [--message <text>]` | Continues an existing project with an optional steering message. |
| `update <name>` | Refreshes an existing project's findings. |
| `cluster <name> [--force]` | Runs research clustering on the project; `--force` re-runs over existing clusters. |
| `export <name> [--output <path>]` | Exports the project bundle to a path. |
| `import <path> [--name <name>]` | Imports a project bundle, optionally renaming it. |
| `config` | Shows the resolved research configuration. |
| `help` | Prints the subcommand table. |

## Create flags

| Flag | Values / Default |
|---|---|
| `--from-url`, `--from-url` (repeatable) | Seed the run from one or more URLs. |
| `--from-file`, `--from-file` (repeatable) | Seed the run from local files. |
| `--iterations N` | Iteration budget for the gather loop. |
| `--depth` | `shallow` \| `standard` \| `deep`. |
| `--tier` | `light` \| `full` \| `dissertation`. |
| `--mode` | `tiered` \| `supervisor` \| `competitive`. `competitive` implies `--format comparison-table`. |
| `--summarization-model` | `<provider:model>` override for summarisation. |
| `--research-model` | `<provider:model>` override for the researcher stage. |
| `--compression-model` | `<provider:model>` override for compression. |
| `--final-report-model` | `<provider:model>` override for the final report. |
| `--max-concurrent-research-units N` | Parallel research unit cap. |
| `--format` | `report` \| `executive-summary` \| `comparison-table` \| `source-bibliography` \| `imrad`. |
| `--sources-dir <dir>` | Local sources directory to cross-reference. |
| `--template <name>` | Output template override. |
| `--fetch-concurrently N` | Concurrent fetch cap (default 10). |
| `--local-concurrently N` | Concurrent local-file reads (default 8). |
| `--fetch-timeout-secs N` | Per-fetch timeout (default 30). |
| `--web-phase-timeout-secs` / `--web-time N` | Web phase budget. |
| `--local-phase-timeout-secs N` | Local phase budget. |
| `--search-max-retries N` | Search retry count (default 2). |
| `--search-retry-base-delay-ms N` | Retry backoff base (default 200). |
| `--search-circuit-breaker-threshold N` | Consecutive failures before the breaker opens (default 3). |
| `--max-web-results N` | Web result cap. |
| `--max-search-calls N` | Search-call cap. |
| `--max-local-sources N` | Local source cap. |
| `--max-synthesis-sources N` | Sources admitted into synthesis. |
| `--brief <text>` | Provide the brief up front, skipping clarification. |
| `--use-local` | Include local file sources. |
| `--use-specs` | Include spec documents as sources. |
| `--use-low-relevance` | Keep low-relevance hits. |
| `--no-papers` | Exclude scholarly-paper engines. |
| `--use-pdf` | Enable PDF extraction. |
| `--clarify` / `--no-clarify` | Force on or off the clarification stage. |
| `--evaluate` | Enable evaluation scoring of findings. |

## Examples

```
/research create rust-async "Compare tokio and async-std task scheduling models"
```
Names the project `rust-async`, uses everything after it as the topic.

```
/research create deep-dive "tree-sitter incremental parsing" --tier dissertation --depth deep
```
Runs a deep, dissertation-tier project.

```
/research create db-compare "sqlite vs duckdb analytics" --mode competitive
```
Competitive mode forces `--format comparison-table` output.

```
/research create local-audit "unsafe usage" --sources-dir ./crates --use-local --use-specs
```
Cross-references local crates and spec documents.

```
/research create fast "tokio broadcast channels" --tier light --fetch-concurrently 20 --web-phase-timeout-secs 60
```
A light run with a raised fetch concurrency and web budget.

```
/research create rerun "existing project" --summarization-model ollama:qwen2.5:1.5b
```
Routes the summarisation stage to a fast local model.

```
/research list --all
```
Lists every project including archived ones.

```
/research continue rust-async --message "Focus on the cancellation paths"
```
Continues a project with steering guidance.

## Output

- While running, the status bar shows `[wait] research: <name>...` and the
  pipeline streams progress lines into the message window.
- On completion the project's RESEARCH.md is written and a summary block lists
  the report path, source counts, and synthesis highlights.
- `list` renders a project table; `--json` variants emit machine-readable
  output for scripting.
- Failures print the failing stage with a corrective message; the project
  remains resumable via `/research resume`.

## Related

- `ragent research` CLI runs the same engine outside the TUI.
- `GET/POST/DELETE /research` HTTP endpoints mirror these operations.
- `/research cluster` relies on the research clustering engine.
- See docs/research-phase2-plan.md and the research system docs for internals.