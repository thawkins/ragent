# ragent Research Manual (TUI)

This guide explains how to use ragent's built-in research system from the interactive TUI. The same commands are available as `ragent research <subcommand>` on the CLI and as `POST /research` over the HTTP API, but this document is written for the terminal interface.

> **Scope:** `/research` slash commands, the Hyperresearch adversarial pipeline, source vaulting, local/spec cross-referencing, and the built-in markdown viewer. For web-search engine configuration and diagnostics, see `/websearch` in the TUI (`/websearch help`).

---

## 1. Quick start

Open ragent and run:

```text
/research create rust-async "Rust async/await patterns"
```

The TUI will:

1. Create a new research item under `research/rust-async/`.
2. Decompose the topic, search the web (DuckDuckGo, Brave, OpenAlex, Wikipedia, plus any configured API engines), scan local project files and specs, and synthesize a structured report.
3. Stream progress into the message window and status bar.
4. Write the final document to `research/rust-async/RESEARCH.md`.

When it finishes, open the result with:

```text
/research open rust-async
```

---

## 2. What research items are

A research item is a self-contained folder under `research/<name>/`:

```text
research/
├── INDEX.md
└── rust-async/
    ├── RESEARCH.md          # final report (frontmatter + markdown body)
    └── sources/
        ├── web-01.md
        ├── web-02.md
        ├── local-01.md
        └── spec-01.md
```

- `RESEARCH.md` is the deliverable. It contains an executive summary, findings, implications, diagrams, cross-references, open questions, and a numbered references index.
- `sources/` holds the captured body text of every web, local, and spec source so each citation can be traced back to the original text.
- `research/INDEX.md` is a derived table of all items, regenerated automatically on every create/delete/archive.

Research names must be 3–64 characters, start with a lowercase ASCII letter, and contain only lowercase letters, digits, and hyphens. This rule prevents path traversal and keeps directory names URL-safe.

---

## 3. The `/research` slash command family

All commands are entered in the TUI input box.

### 3.1 `/research help`

Show the command syntax, flags, and formats. Use this whenever you forget a flag.

### 3.2 `/research create`

Run a research session and write `RESEARCH.md`.

```text
/research create <name> [topic]
  [--from-url <URL>] [--from-file <PATH>]
  [--iterations N] [--depth shallow|standard|deep]
  [--tier light|full|dissertation]
  [--format report|executive-summary|comparison-table|source-bibliography|imrad]
  [--sources-dir <path>] [--template <name>]
  [--fetch-concurrently N] [--local-concurrently N]
  [--fetch-timeout-secs N]
  [--web-phase-timeout-secs N] [--local-phase-timeout-secs N]
  [--search-max-retries N] [--search-retry-base-delay-ms N]
  [--search-circuit-breaker-threshold N]
  [--use-local] [--use-specs] [--use-low-relevance] [--no-papers]
```

If the first argument after `/research create` is not a valid research name, the parser treats the whole line as `<name> <topic>`; this lets you type quickly when the name is valid.

#### Examples

```text
/research create rust-async "Rust async/await patterns"
/research create openai-reasoning "OpenAI o1-style reasoning" --tier full --depth deep
/research create local-only "Project error handling" --use-local --no-papers
/research create from-page --from-url https://example.com/article
/research create from-doc --from-file docs/design.md --use-local
/research create benchmark "Comparison of vector databases" --format comparison-table
/research create paper-review "Attention Is All You Need" --format imrad --tier full
```

### 3.3 `/research list`

List research items, newest first.

```text
/research list
/research list --all   # include archived items
```

### 3.4 `/research show <name>`

Print metadata for one item: title, status, timestamps, topic, and all captured sources with their provenance.

### 3.5 `/research open <name>`

Open the rendered `RESEARCH.md` in the TUI's markdown overlay. See section 9 for viewer controls.

### 3.6 `/research search <query>`

Full-text search across all `RESEARCH.md` files. Returns matching item names, titles, and snippets.

### 3.7 `/research delete <name> --yes`

Permanently delete a research item and its `sources/` directory. The `--yes` flag is required; without it the command prompts and refuses.

### 3.8 `/research archive <name>`

Mark an item as `archived`. Archived items are hidden from default `list` output but kept on disk.

### 3.9 `/research continue <name> [message]`

Resume an in-progress item (currently loads state and appends a follow-up sub-question). The optional message is added to the plan as a new sub-question.

---

## 4. Tiers: light, full, dissertation

The `--tier` flag selects how much adversarial quality assurance runs after gathering.

| Tier | Pipeline steps | Best for |
|------|------------------|----------|
| `light` | decompose → width sweep → evidence digest → triple draft → synthesize → cite check → polish | Quick orientation, fast answers |
| `full` | all 16 Hyperresearch steps, including contradiction graph, loci analysis, corpus critic, gap-fill fetch, surgical patch, readability audit | Thorough reports, decision documents |
| `dissertation` | `full` plus chapter partitioning into up to 12 chapters | Long-form deliverables |

Default is `full`. If you only need a fast answer, use `--tier light`.

---

## 5. Depth and iterations

`--depth` selects source/iteration budgets:

| Depth | Iterations | Sources per sub-question | Concurrency |
|-------|-----------:|-------------------------:|------------:|
| `shallow` | 1 | 2 | 2 |
| `standard` | 3 | 3 | 4 |
| `deep` | 5 | 5 | 6 |

`--iterations N` overrides the default iteration count. Multi-iteration mode uses the iterative research engine and runs a planner/critic loop; it activates automatically when `--iterations` is set or `--depth deep` is used.

---

## 6. Output formats

`--format` changes the skeleton and final layout of `RESEARCH.md`.

| Format | Layout |
|--------|--------|
| `report` (default) | Topic, Search Queries, Executive Summary, Top 10 Implications, Open Questions, Findings, Findings Relationship Diagram, Cross-References, References Index |
| `executive-summary` | One-page summary of the same content |
| `comparison-table` | Comparison table across key entities |
| `source-bibliography` | Bibliography of all captured sources |
| `imrad` | Scientific/technical report: Abstract, Introduction, Methods, Results, Discussion, References Index |

The format is recorded in the item frontmatter and can be reopened later with `/research open`.

---

## 7. Seed sources and local context

### 7.1 Seed a research from a URL

```text
/research create from-url --from-url https://example.com/article
```

The page is fetched first; its extracted body is used to derive the topic, then normal web search runs on that topic. `--from-url` can be repeated for multiple seed pages.

### 7.2 Seed from a local file

```text
/research create from-doc --from-file docs/design.md --use-local
```

The file body is extracted as the primary `Other` source and used to derive the topic.

### 7.3 Include project files and specs

```text
/research create rust-async "Rust async patterns" --use-local --use-specs
```

- `--use-local` enables scanning the project root and any `--sources-dir` for files matching the topic keywords.
- `--use-specs` cross-references prior specifications under `specs/<id>/`.

Local files are scored by keyword matches and the top results are captured with excerpts.

---

## 8. Tuning gathering behavior

| Flag | What it does |
|------|--------------|
| `--fetch-concurrently N` | Number of candidate pages fetched in parallel (default 10) |
| `--local-concurrently N` | Parallel local-file scoring tasks (default 8) |
| `--fetch-timeout-secs N` | Per-page fetch timeout (default 30) |
| `--web-phase-timeout-secs N` | Wall-clock timeout for the entire web phase |
| `--local-phase-timeout-secs N` | Wall-clock timeout for the entire local phase |
| `--search-max-retries N` | Retries per failed sub-query (default 2) |
| `--search-retry-base-delay-ms N` | First retry delay, doubled each time (default 200 ms) |
| `--search-circuit-breaker-threshold N` | Consecutive search failures before the circuit breaker opens (default 3) |
| `--use-low-relevance` | Keep sources that would normally be filtered out as low-relevance |
| `--no-papers` | Disable scholarly backends (OpenAlex) so only general web results are captured |

---

## 9. Reading results in the TUI

`/research open <name>` opens the markdown viewer overlay.

| Key / mouse | Action |
|-------------|--------|
| `↑` / `↓` | Scroll one line |
| `PageUp` / `PageDown` | Scroll one page |
| `Ctrl+PageUp` / `Ctrl+PageDown` | Jump to start / end |
| `Mouse wheel` | Scroll inside the overlay when the cursor is over it |
| `Click outside` or `Esc` | Close the overlay |

The viewer renders headings, lists, code blocks, tables, and inline formatting. Tables (including the references index) are preserved as rows.

---

## 10. Following live progress

When a research session runs, the TUI shows progress in three places:

1. **Status bar** — a transient line such as `⏳ research: rust-async — web (▶) — running`. It updates per phase and becomes `research: rust-async complete — 12 sources` when finished.
2. **Message window** — a pinned `🔬 Research Progress — rust-async` log lists each phase (setup, web, local, specs, synthesize, assemble, finalize), captured sources, and audit results.
3. **Log panel** — every progress event is logged at `Info` level.

Research runs in the background; you can continue typing or start other work while it gathers.

---

## 11. The Hyperresearch pipeline in `full` tier

The `full` (and `dissertation`) tiers run an adversarial, deterministic quality pipeline after gathering:

1. **Contradiction graph** — detects opposing source claims on the same dimension and ranks them by strength.
2. **Loci analysis** — finds recurring research dimensions across the corpus.
3. **Depth investigation** — classifies each locus as `surface`, `moderate`, or `deep` based on source count.
4. **Cross-locus reconcile** — surfaces dimensions that share common sources.
5. **Source tensions** — lists contradictions, shallow evidence, and isolated sources.
6. **Evidence digest** — ranks claims from strongest to weakest support, marking contested ones.
7. **Triple draft** — produces three deterministic candidate summaries: consensus, skeptical, and gap-aware.
8. **Synthesis audit** — four internal critic subagents score coverage, logic, evidence, and readability.
9. **Gap-fill fetch** — issues targeted follow-up queries for evidence gaps found by the corpus critic.
10. **Surgical patch** — applies deterministic revisions to the draft based on audit and critic output.
11. **Citation check** — verifies every `[#N]` citation against the gathered sources; a failure blocks report shipment.
12. **Polish & readability audit** — cleans whitespace/control characters and scores the final draft.

Each step appears in `RESEARCH.md` when it produced results. Steps are skipped for `light` tier and run chapter-by-chapter for `dissertation`.

---

## 12. Web search engines

By default, research uses `mf_search`, which queries DuckDuckGo, Brave, OpenAlex, and Wikipedia in parallel. Optional API-backed engines can be added in `ragent.json`:

```json
{
  "langsearch_api_key": "...",
  "tavily_api_key": "...",
  "perplexity_api_key": "...",
  "exa_api_key": "...",
  "openalex_email": "you@example.com"
}
```

Use the TUI diagnostics to check availability:

```text
/websearch show   # enabled / in-use / failed per engine
/websearch test   # live probe each configured engine
```

The `--no-papers` flag disables OpenAlex for runs where scholarly results are not wanted.

---

## 13. Open-access recovery

For scholarly sources, ragent can attempt to recover a legal full-text copy via Unpaywall and Europe PMC when the fetched body is short.

Enable it in `ragent.json`:

```json
{
  "research": {
    "open_access_recovery": true,
    "contact_email": "you@example.com",
    "oa_min_full_text_chars": 1000
  }
}
```

`contact_email` is required by Unpaywall's terms of service. When a source is recovered from an OA copy, the references index notes the source and license.

---

## 14. Source vault and resumability

The source vault (`.ragent/research_vault/<run_tag>/`) persists raw captured source text and metadata. On a subsequent run with the same research name, the gatherer checks the vault before issuing new web searches, satisfying the requested tier's minimum source count. This avoids re-fetching already-captured sources.

`--tier light` needs 3 sources, `full` needs 8, and `dissertation` needs 15 before web search is skipped.

---

## 15. Templates

Place Markdown templates under `research/_templates/<name>.md`. They can use the placeholders `{{title}}`, `{{topic}}`, and `{{date}}`.

```text
/research create quarterly "Q3 performance review" --template quarterly
```

The template becomes the skeleton and the standard sections are appended below it.

---

## 16. Tips for good results

- **Use specific topics.** Broad topics produce broad findings. Narrow questions yield more actionable reports.
- **Pick the right tier.** `light` for quick lookups, `full` for decision documents, `dissertation` for long-form writing.
- **Enable `--use-local` and `--use-specs`** when the research should tie back to your own codebase or specifications.
- **Seed with `--from-url` or `--from-file`** when you want the report to start from a particular document.
- **Review the contradiction graph and source tensions** in `full` tier reports to spot conflicting evidence before acting on the findings.
- **Check `/websearch test`** if a run returns unexpectedly few web sources — a backend may be rate-limited or blocked.
- **Read the progress log** to see which sources were captured, which failed, and whether the synthesis used the LLM or the mechanical fallback.

---

## 17. CLI equivalents

The same commands work outside the TUI:

```bash
ragent research help
ragent research create rust-async "Rust async patterns" --tier full --use-local
ragent research open rust-async
ragent research list
ragent research search "vector database"
ragent research delete rust-async --yes
ragent research archive rust-async
```

CLI `create` emits machine-readable JSON lines prefixed with `ragent-research:` so you can pipe them through `jq`.

---

## 18. Troubleshooting

| Symptom | Likely cause | What to do |
|---------|--------------|------------|
| `0 sources` or very few | Search backends blocked / rate-limited | Run `/websearch test`; wait or add an API-backed engine |
| `LLM synthesis failed — using mechanical fallback` | No API key, model down, or output malformed | Check provider setup; the report still contains deterministic summaries |
| `CITATION_VERIFICATION_FAILED` | A citation marker `[#N]` points to a missing source | Run again with `--tier full`; the gate blocks shipment |
| `web phase timed out` | Slow network or many candidate pages | Raise `--web-phase-timeout-secs` or lower `--fetch-concurrently` |
| Name rejected | Invalid research name | Use 3–64 lowercase ASCII letters/digits/hyphens starting with a letter |
| `research/<name>` already exists | Duplicate create | Use `/research open <name>` or pick a new name |

---

*End of manual.*
