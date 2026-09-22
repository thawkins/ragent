---
id: corpusAnalysis
status: draft
---

# TESTPLAN: Source-Quality Scoreboard for /research Documents

Manual test plan for `specs/corpusAnalysis/SPEC.md`. A human runs each case;
none are automated. The goal is to verify the Corpus Quality Scoreboard renders
correctly in generated research documents, with the right grade bands, meter
bar, fallbacks, and placement, in both report and IMRaD layouts.

## Prerequisites

- ragent built from this workspace (`cargo build`); binary at
  `target/debug/ragent`.
- At least one configured and reachable LLM provider (e.g.
  `export ANTHROPIC_API_KEY=sk-...` or a running local Ollama), because every
  test case runs a real research build.
- Working directory is a scratch project folder with write access, containing
  some local text files to act as project sources (create two or three `.md`
  files with a few hundred words on a simple topic, e.g. "rust testing").
- No existing `research/<name>` folder with the names used below; delete any
  leftovers first.

## Test Cases

### TC-001 Scoreboard renders in a standard report-layout run

**Covers:** FR-001, FR-002, FR-003, FR-004, FR-011, FR-012
**Preconditions:** Prerequisites met; corpus critic + synthesis audit expected.

Steps:

1. In the scratch project folder, run `ragent --no-tui research create --name
   scoreboard-report --topic "rust testing practices"`.
2. Wait for the run to complete (CLI prints the completion summary).
3. Open `research/scoreboard-report/RESEARCH.md` in a text editor.

Test data: topic `rust testing practices`; name `scoreboard-report`.

Expected results:

- Near the top of the file (immediately after the frontmatter/title) there is
  a section `## Corpus Quality Scoreboard`.
- It is followed by a `Quality: <N>/100 - Grade <X> (<word>)` line, where N is
  0-100 and <X> is A (80-100), B (65-79), C (50-64) or D (0-49) and <word> is
  Excellent / Good / Adequate / Weak respectively.
- Below it is a fenced code block containing a meter bar
  `[####...----]` exactly 20 characters inside the brackets, proportional to N
  (e.g. 74 => 15 `#`, 5 `-`), followed by `  74/100`.
- A critic subscore line appears:
  `(coverage X | evidence Y | balance Z | tension T)`, each 0-100.
- A source-facts line appears with gathered / cited / full-text / distinct
  domains / average relevance (x/8) and a cited date span.
- The document still contains the existing Data Quality Summary section
  unchanged, later in the file.
- No emoji or box-drawing/block characters appear in the scoreboard block.

### TC-002 Scoreboard renders in an IMRaD-layout run

**Covers:** FR-001, FR-002, FR-003, FR-011 (IMRaD placement)

Steps:

1. Run `ragent --no-tui research create --name scoreboard-imrad --topic "rust
   testing practices" --format imrad`.
2. Open `research/scoreboard-imrad/RESEARCH.md`.

Test data: topic `rust testing practices`; format `imrad`.

Expected results:

- `## Corpus Quality Scoreboard` appears after the title/frontmatter and
  BEFORE the `## Abstract` section.
- Score line, meter bar, and subscores render as in TC-001.
- The Data Quality Summary appears inside Discussion, unchanged.

### TC-003 Meter bar proportionality spot-check

**Covers:** FR-003

Steps:

1. Using any document from TC-001/TC-002, note the score N and fill count F in
   the meter bar (`[F hashes, 20-F dashes]`).
2. Compute expected `F = round(N * 20 / 100)`; verify F matches.

Test data: the scores observed in TC-001 (e.g. 74 -> 15; 50 -> 10; 0 -> 0).

Expected results:

- For every document checked, filled-cell count equals
  `round(N * 20 / 100)`, clamped to 0-20.

### TC-004 Not-graded fallback (no critic, no audit)

**Covers:** FR-007

Steps:

1. Run a research create with a topic of a single very short local file, e.g.
   `ragent --no-tui research create --name scoreboard-nograde --topic "quick
   note"`. If the run still produces a synthesis audit, use a topic that fails
   early instead (e.g. point `--sources-dir` at an empty folder and use from
   local-only mode) so that neither critic nor audit is produced.
2. Open `research/scoreboard-nograde/RESEARCH.md`.

Test data: minimal local-only run, name `scoreboard-nograde`.

Expected results:

- Scoreboard shows `Quality: Not graded` (no numeric score, no grade band).
- Source-facts line still renders.
- No empty or malformed meter block; the meter block is absent.
- No panic/blank section; document remains valid markdown.

### TC-005 Abbreviated output format reduction

**Covers:** FR-013

Steps:

1. Run `ragent --no-tui research create --name scoreboard-brief --topic "rust
   testing practices" --format executive-summary`.
2. Open `research/scoreboard-brief/RESEARCH.md`.

Test data: format `executive-summary`.

Expected results:

- Scoreboard shows score line, meter bar, source-facts line.
- Critic-subscore line and tension/citation line are omitted.
- No non-ASCII glyphs.

### TC-006 Scoreboard fidelity in the TUI research overlay

**Covers:** FR-008

Steps:

1. Complete the TC-001 run so `research/scoreboard-report/RESEARCH.md` exists.
2. Launch `ragent` (TUI).
3. Type `/research open scoreboard-report` and press Enter.
4. In the overlay, scroll to the top and inspect the scoreboard block.

Test data: research name `scoreboard-report` from TC-001.

Expected results:

- The overlay shows `Quality: 74/100 - Grade B (Good)` (matching the file,
  whatever N was) with the same meter bar and same fact lines.
- The meter bar is not re-flowed or wrapped: the bracketed bar stays on one
  line.
- Closing the overlay (Esc / per on-screen hint) returns to the chat view.

### TC-007 Tension and citation lines react to critic/cite data

**Covers:** FR-009, FR-010

Steps:

1. Run a research create on a topic likely to find conflicting statements
   (e.g. compare two project docs that disagree on a recommendation), name
   `scoreboard-tension`.
2. Open `research/scoreboard-tension/RESEARCH.md`.
3. Compare the scoreboard tension line against the contradictions / citation
   sections deeper in the document.

Test data: name `scoreboard-tension`, a topic with contradictory local docs.

Expected results:

- If contradictions were found, the scoreboard shows a contradiction count and
  strongest strength (e.g. `Contradictions: 2 edges (strongest 78/100)`),
  consistent with the Contradictions row of the Data Quality Summary table.
- If a citation check ran, the line shows `Citation check: passed` or
  `Citation check: failed`, consistent with the Citation Check section.
- If no contradictions/citation check, the corresponding part is omitted
  without leaving a dangling label.

### TC-008 ASCII-only scoreboard

**Covers:** FR-016

Steps:

1. Open any scoreboard-bearing document from TC-001..TC-005 in a text editor.
2. Select the whole scoreboard section.

Test data: any document from TC-001..TC-005.

Expected results:

- Every character in the scoreboard section is ASCII (letters, digits, spaces,
  `-`, `[`, `]`, `|`, `(`, `)`, `/`, `:`, `.`, `#`). No emoji, no Unicode block
  glyphs, no smart quotes.
- The meter bar uses only `#` and `-` cells plus brackets.

## Cleanup

1. Remove scratch research runs:
   `rm -rf research/scoreboard-report research/scoreboard-imrad
   research/scoreboard-nograde research/scoreboard-brief
   research/scoreboard-tension` (adjust to names actually created).
2. Remove or archive scratch project source files created for the tests.
3. Unset provider environment variables in the test shell if desired
   (`unset ANTHROPIC_API_KEY` etc.).