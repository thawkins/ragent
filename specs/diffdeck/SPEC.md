---
status: implemented
audit:
  - { time: 1793747766, from: "none", to: "draft", actor: "system" }
  - { time: 1794443733, from: "draft", to: "in_review", actor: "cli" }
  - { time: 1794443733, from: "in_review", to: "approved", actor: "cli" }
  - { time: 1794443734, from: "approved", to: "in_progress", actor: "cli" }
  - { time: 1794443734, from: "in_progress", to: "implemented", actor: "cli" }
---
# Research Diff-Deck Specification

## Overview

The research system stores one report per run under `research/<name>/` as
`RESEARCH-<YYYY-MM-DD>.md` (see `crates/ragent-research/src/io.rs`). Because a
re-run of an existing item (`/research update`) writes a new dated file instead
of overwriting the previous one, an item accumulates a chronological series of
reports. Today there is no way to consume that series as a *change narrative*:
a reader who wants to know what moved between two runs has to open both reports
side by side and diff them by eye.

This specification defines **`/research deck <name>`** (CLI: `ragent research deck <name>`), which reads a research item, selects two of its dated reports -
by default the two most recent - and writes a single self-contained deck
document `DECK-<YYYY-MM-DD>.md` into the item directory. The deck is a Markdown
document laid out as a slide deck:

- a **cover slide** carrying the item title and the coverage window (the two
  contributing report dates), and
- one **slide per concept**, each carrying the concept's definition and listing
  the findings that belong to it, every finding labelled with its change status
  (`NEW`, `ADDED`, `UPDATE`, `OLD`),
- one **slide per other changed element** (the Executive Summary, the
  Cross-References, the References Index), each listing every item belonging to
  that element.

The deck's grouping is the report's `## Concepts` section (the "major elements",
extracted by the LLM): a concept is the key category and every finding is
grouped under the concept it belongs to. The report's `## Top 10 Implications`
and `## Open Questions` are reader-facing narrative, not deck elements, so the
deck renders neither. 

The deck is **deterministic and LLM-free**: it is a pure function of the two
input report files, so the same pair of reports always yields byte-identical
output, the run costs no tokens, and it works with no provider configured.

## Requirements

### Functional Requirements

**FR-001 (Ubiquitous)**
The system shall provide a `/research deck <name>` TUI slash command and a
`ragent research deck <name>` CLI sub-command that both produce a diff deck for
the research item `<name>`.

**FR-002 (Ubiquitous)**
The system shall write the deck to `research/<name>/DECK-<YYYY-MM-DD>.md`, where
`<YYYY-MM-DD>` is the UTC date on which the deck was generated, using the same
write-then-rename atomic path (`ResearchIo::atomic_write`) that every other
research artifact uses.

**FR-003 (Ubiquitous)**
The system shall render the deck as Markdown slide sections separated by a
`---` horizontal rule, in this fixed order: exactly one cover slide, then zero
or more element slides ordered by a fixed element precedence (Executive Summary,
Concepts, Findings, Cross-References, References), then one closing summary
slide. The Concepts element emits one slide per concept (FR-026) rather than one
slide for the element.

**FR-004 (Ubiquitous)**
The system shall label every rendered item with exactly one status tag drawn
from the closed set `NEW`, `ADDED`, `UPDATE`, `OLD`, and shall define the tags
as: `NEW` - the item exists only in the newer report; `ADDED` - the item exists
only in the newer report and has at least one source the older report did not
cite; `UPDATE` - the item exists in both reports with changed condensed text;
`OLD` - the item exists in both reports with unchanged condensed text.

The sources an item "has" are, for `## Findings`, the entries of its
`**Sources:**` block, and for every other element the `[#N]` citation markers
its own text carries. Those markers are therefore recorded on the item, so a
paragraph, implication or question that gained a citation is observable as
`ADDED` rather than always reading `NEW`.

**FR-005 (Ubiquitous)**
The cover slide shall show the research item title, the deck generation date,
the two contributing report dates as a coverage window
(`Coverage: <older-date> to <newer-date>`), the research id, and the count of
element slides, change items, and unchanged (`OLD`) items in the deck.

**FR-006 (Event-driven)**
When `/research deck <name>` is invoked without an explicit report selection,
the system shall select the two most recent dated reports of the item,
lexicographically greatest `RESEARCH-<YYYY-MM-DD>.md` first, and render the
older of the two as the baseline and the newer one as the current report.

**FR-007 (Event-driven)**
When `/research deck <name> --from <date>` or `--to <date>` is supplied, the
system shall use the named report(s) instead of the default most-recent pair,
interpreting each value as a `YYYY-MM-DD` stamp matching a
`RESEARCH-<stamp>.md` file, and when only one of the two is supplied it shall
pair the named report with the adjacent dated report (the next older for
`--to`, the next newer for `--from`).

**FR-008 (Event-driven)**
When the diff of an element yields at least one `NEW`, `ADDED` or `UPDATE`
item, the system shall emit one slide for that element containing **every**
item of that element from both reports - the changed items and the unchanged
(`OLD`) items - so a slide is a complete picture of the element rather than
only its deltas.

**FR-009 (Event-driven)**
When the system compares two corresponding items, it shall condense each to a
single bullet and match them by section-scoped normalised identity: for
`## Findings` by the extracted headline, for `## Concepts` by the concept's
declared name (its `### <n>. <name>` heading with the ordinal removed), and for
every other element by the normalised item text with citations, Markdown emphasis
and whitespace runs removed.

**FR-010 (Event-driven)**
When the closing summary slide is rendered, the system shall report the total
number of element slides, the per-status counts of every emitted item, and the
list of element names that produced a slide.

**FR-011 (State-driven)**
While the item directory contains fewer than two report files, the system shall
refuse the run with an error naming the item and the number of reports found,
and shall not create or modify any file in the item directory.

**FR-012 (State-driven)**
While the item directory contains report files in the legacy undated scheme
(`RESEARCH.md`) alongside dated reports, the system shall treat `RESEARCH.md` as
the oldest report of the series (baseline only), and shall refuse the run with a
message naming the legacy scheme when two reports are required but only the
legacy file exists.

**FR-013 (State-driven)**
While a pair of reports produces no changed items in any element, the system
shall still write the deck file, containing the cover slide, an explicit
`_No changes detected between the two reports._` notice in place of element
slides, and the closing summary slide.

**FR-014 (Optional)**
Where `--elements <list>` is supplied as a comma-separated list of element
names, the system shall restrict the deck to those elements and shall reject any
name that is not one of the five defined element names, naming the offending
value and the accepted values. `Top Implications` and `Open Questions` are not
element names and are rejected by name.

**FR-015 (Optional)**
Where `--include-old` is supplied, the system shall additionally emit a slide
for each element that contains only `OLD` items; without the flag such
no-change elements produce no slide.

**FR-016 (Optional)**
Where `--out <path>` is supplied, the system shall write the deck to that path
instead of the default `DECK-<YYYY-MM-DD>.md` inside the item directory, and
shall create missing parent directories.

**FR-017 (Optional)**
Where `--force` is supplied and the target deck file already exists, the system
shall overwrite it; without `--force` the system shall refuse with an error
naming the existing path and the flag that overwrites it.

**FR-018 (Unwanted)**
If the named research item does not exist, then the system shall refuse the run
with an error naming the item and shall not create any directory or file.

**FR-019 (Unwanted)**
If a selected report file cannot be read, is not valid UTF-8, or contains no
recognised top-level sections, then the system shall refuse the run with an
error naming the file path and the reason, and shall leave any existing deck
file byte-identical.

**FR-020 (Unwanted)**
If a `--from` or `--to` value is not a valid `YYYY-MM-DD` stamp or names no
existing `RESEARCH-<stamp>.md` file, then the system shall refuse the run with
an error naming the offending value and listing the report dates that do exist
for that item.

**FR-021 (Unwanted)**
The system shall not write outside the research root (or the caller's `--out`
path) while producing a deck. The **default** run - `/research deck`, the CLI
sub-command without a wired provider, and every pure stage the deck is built
from - shall additionally perform no network request and no LLM call, so a
provider-configured and a provider-free environment produce byte-identical
output for the same report pair. The optional FR-023/FR-024 enrichment is the
one stage that may call the configured model; it is off unless a provider is
wired in, and it never changes the report pair, the diff, the counts, or the
slide structure.

**FR-023 (Ubiquitous)**
The system shall render each finding slide as the finding's headline followed by
one paragraph summarising both its `**Observation:**` and its `**Analysis:**`
prose, and shall render that paragraph as follows:

1. when a model is wired into the deck run, the paragraph is the model's
   condensation of those blocks, capped at 700 characters on a word boundary
   and reduced to a single line with `\`, backtick, `|` and a leading `#`
   escaped so it cannot break the slide structure;
2. otherwise - no model configured, a failed call, a timeout, or a reply with no
   alphanumeric content - the paragraph is the first sentence of the finding's
   observation followed by the first sentence of its analysis, so a deck built
   without a provider still explains the finding;
3. a finding whose report block carried no `**Observation:**` or
   `**Analysis:**` prose keeps the compact single-bullet form, so a headline-only
   finding is never rendered as an empty paragraph.

The `**Sources:**` bullet list and the `**Source date range:**` note are metadata
and shall never become paragraph text.

**FR-024 (Ubiquitous)**
The system shall group the findings under the concepts they belong to: each
concept claims at most three findings, and the grouping shall be:

1. when a model is wired into the deck run, the model's own grouping of the
   findings, read from a line-oriented `<concept>: <finding>[, <finding>...]`
   reply and resolved against the numbered lists the prompt named;
2. otherwise - or when the reply names no valid pair, the call fails, or no
   model is configured - a deterministic term-overlap ranking that scores each
   (concept, finding) pair by the content terms they share, counting a headline
   match double a body match and listing only findings that clear the floor of
   two shared terms, so an unrelated finding is never grouped under a concept.

**FR-025 (Ubiquitous)**
The system shall treat the FR-023 and FR-024 model stages as optional
enhancements: a deck run shall never be refused, and the deck shall never be
partially written, because a provider is unavailable, unconfigured, or returns
an unusable reply. Every model call is best-effort and degrades to the
deterministic value it replaces.

**FR-026 (Ubiquitous)**
The system shall render the Concepts element as one slide per concept:

1. the slide heading is the concept's name, followed by its change status and
   then its `**Definition:**` paragraph (or, when the block carried no
   recognised label, its first prose paragraph);
2. beneath the definition the slide lists each finding the concept claimed, most
   relevant first, each rendered as the finding's FR-004 status tag, its headline
   as a `###` sub-heading, and the FR-023 paragraph; a finding keeps the status
   the diff assigned it, so a concept that gained a finding still shows when that
   finding arrived;
3. a concept that claimed no finding renders its definition and the explicit
   line `_No finding maps to this concept._`, so an empty concept is visible
   rather than silently omitted;
4. every finding that no concept claimed is rendered on the Findings slide under
   the heading `Other Findings`, so each finding appears on the deck exactly
   once and no finding is lost to the grouping.

**FR-031 (Ubiquitous)**
The system shall report a deck run's stage progress to the calling surface, so a
run that spends real time - one model call per finding in the enrichment stage,
one server round trip per slide in the presentation export - is visible while it
runs instead of only when it finishes:

1. each stage the run executes reports once as it begins and once as it ends,
   naming the stage (`select`, `read`, `diff`, `destination`, `enrich findings`,
   `group concepts`, `render`, `export`), in run order;
2. a stage that does not apply to this run reports `skipped` with the reason
   (`no model selected`, `no presentation capability connected`) rather than
   going silent, so "not applicable" is distinguishable from "still working";
3. a countable stage additionally reports each unit as it is worked on, carrying
   the completed and total unit counts - one report per finding paragraph and one
   per exported slide - and the completed count shall never exceed the total;
4. a refusal reports the stage it stopped in and no later stage, so the surface
   never shows a stage that was not reached;
5. the reporting shall be additive: a run with no progress sink shall produce the
   same artifacts byte-for-byte as one with a sink, and a sink that fails to
   deliver a report shall never fail or alter the run.

**FR-022 (Ubiquitous)**
The system shall escape Markdown-significant characters in emitted item bullets
so a finding whose text contains `|`, backticks, or a leading `#` cannot break
the slide structure, and shall cap each condensed bullet at 200 characters with
a trailing ellipsis when truncated.

### Non-Functional Requirements

**NFR-001 (Performance)**
A deck run over two reports of up to 200 KiB each shall complete in under 100 ms
on the reference developer machine, with no network access and no allocations
proportional to the square of the item count (element matching shall be keyed by
a hash map, not a nested linear scan).

**NFR-002 (Determinism)**
For a fixed pair of input reports, a fixed flag set and a fixed enrichment input
(no model, or a model with a fixed reply), two runs shall produce byte-identical
deck files apart from the generation-date stamp, and within one calendar day
shall be byte-identical including it. The deterministic enrichment is itself
pure, so the model-free deck is fully reproducible.

**NFR-003 (Safety)**
The implementation shall contain no `unsafe` code and no `unwrap()`/`expect()` on
any production path; every fallible step shall return a `Result` carrying a
typed error.

**NFR-004 (Testability)**
The diff and render stages shall be exposed as pure functions over in-memory
strings (`diff_reports`, `render_deck`) so they are unit-testable without any
filesystem access.

## Element-to-Status Mapping

| Element           | Extracted from                   | Item unit                          | Identity key       |
| ----------------- | -------------------------------- | ---------------------------------- | ------------------ |
| Executive Summary | `## Executive Summary`         | Paragraph                          | Normalised text    |
| Concepts          | `## Concepts`                  | `### <n>. <name>` block            | Concept name       |
| Findings          | `## Findings`                  | `### **Finding N** - <headline>` | Extracted headline |
| Cross-References  | `## In-Project Cross-References` | Bullet item                        | Normalised text    |
| References        | `## References Index`            | `[#N]` entry                     | Normalised URL     |

Findings additionally carry their `**Observation:**`, `**Analysis:**` and
`**Implication:**` prose (`DeckItem::detail`), which FR-023 condenses into the
slide's paragraph and FR-024 ranks against each concept. A concept's `detail`
carries its definition prose, which FR-026 renders under the concept's name.

### Heading resolution

The report is the authority on how its own sections are spelled, so the parser
resolves a heading rather than requiring one literal spelling:

- **Concepts** matches `## Concepts` in the report layout and `### Concepts`
  under `## Results` in the IMRaD layout, and the same holds for **Findings**.
- **A `### ` sub-heading that names an element opens that element's section**,
  whether or not its parent is itself an element, so the IMRaD layout's nested
  `### Findings` and `### Concepts` reach the deck.

## Out of Scope

- Rendering the deck to PDF, PPTX, or any binary slide format.
- Re-running research, fetching sources, or invoking an LLM to summarise changes.
- Merging more than two reports into one deck (a multi-way diff).
- Automatic change notification or scheduling; the deck is produced on demand.
- Editing, versioning, or garbage-collecting existing `DECK-*.md` files.

## Acceptance Summary

The feature is complete when `/research deck <name>` and
`ragent research deck <name>` produce a `DECK-<YYYY-MM-DD>.md` for a multi-run
research item, the file opens with a cover slide naming the item and its
coverage window, each concept has a slide carrying its definition and the
findings grouped under it, every rendered item carries one of `NEW` / `ADDED` /
`UPDATE` / `OLD`, no finding appears on the deck twice and none is dropped, the
deck carries no `Top Implications` or `Open Questions` slide, every
FR-011..FR-021 refusal path is reproduced exactly as written, and the same input
pair renders identically across two runs. A run also narrates its stages as it
works (FR-031): the surface acknowledges the command before the first lookup
completes, shows `N of M` while it writes the finding paragraphs and the
presentation slides, and names the stage a refusal stopped in.
