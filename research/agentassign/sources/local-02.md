# Local source (in-project)

- Path: CHANGELOG.md
- Relevance: 500 match(es) on: an, change, on, …(+26) — "# Changelog"
- Captured (UTC): 2026-06-25T05:53:23.379350771+00:00

```text
Excerpt — 500 keyword match(es)

▶    1:    1  # Changelog
     2:    2  
▶    3:    3  ## Version: 0.1.0-alpha.120 (unreleased)
     4:    4  
▶    5:    5  ### Fixed — `/research` now actually analyses the gathered sources
▶    6:    6  - **Supporting files contain the captured body, not a placeholder** — `Source::Web`,
▶    7:    7    `Source::Local`, and `Source::Other` gained an inline `body: String` field. The
▶    8:    8    `WebGatherer` now passes the fetched page text into `Source::Web.body`; the
▶    9:    9    `LocalGatherer` reads each candidate file and writes a context-aware excerpt
▶   10:   10    (matching lines plus one line on either side) into `Source::Local.body`.
▶   11:   11    `render_supporting_file` and the synthesis engine both consume the inline body,
▶   12:   12    so `research/<name>/sources/web-NN.md` and `local-NN.md` now contain the
▶   13:   13    actual evidence (with `▶` markers for exact matches and a ` ` marker for
▶   14:   14    context) instead of the legacy `(see WebGatherer for the captured body)`
▶   15:   15    placeholder. Old `RESEARCH.md` files without the new field deserialize with
▶   16:   16    `body == ""` thanks to `#[serde(default)]`.
▶   17:   17  - **Local source relevance note is now informative** — The previous "X keyword
▶   18:   18    match(es) for research topic" string has been replaced by a note that names
▶   19:   19    the matched keywords (truncated to 3, e.g. `…(+N)` for the tail) and a 120-char
▶   20:   20    snippet of the first matching line. Driven by the new
▶   21:   21    `LocalGatherer::build_relevance_note` and `collect_matched_terms` helpers.
▶   22:   22  - **Mechanical fallback summary/findings are useful, not skeletal** — When no
▶   23:   23    LLM synthesis is available (CLI, or TUI without an active model, or LLM call
▶   24:   24    failed), the default `Summary` now names the captured web titles and local
▶   25:   25    file paths grouped by type, the default `Findings` is one bullet per source
▶   26:   26    with a 240-char excerpt, and the default `Open Questions` suggests concrete
▶   27:   27    gaps and re-running with a configured LLM. The Summary is also transparent
▶   28:   28    that no LLM analysis was applied.
▶   29:   29  - **Synthesis errors are visible, not silently swallowed** — The
▶   30:   30    `ResearchSession::run` synthesize step now matches on the outcome and emits a

… (470 more match(es) elided)

```
