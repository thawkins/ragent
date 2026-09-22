---
status: draft
audit:
  - { time: 1784701247, from: "none", to: "draft", actor: "system" }
---
# Specification: IMRaD-Compliant `/research` Report Output

## Overview

The `/research create` command currently produces a self-contained `RESEARCH.md`
document with a fixed eight-section structure. The report already contains all of
the factual ingredients needed for a scientific/technical research report:
objective (topic/search queries), evidence (findings with sources), analysis,
cross-references, open questions, and a references index. However, the section
headings and narrative order do not follow the **IMRaD** convention —
Introduction, Methods, Results, and Discussion — that is the de facto standard
for empirical and technical research findings documents.

This specification introduces an optional **IMRaD output format** for
`/research` reports. It restructures the existing content into IMRaD sections
while retaining every current feature: YAML frontmatter, topic/search queries,
numbered findings with their five required labeled paragraphs, the findings
relationship diagram, in-project cross-references, open questions, and the
references index. The conversion is a deterministic layout change over the
existing `ResearchDocument` content; it does not require a new LLM call or new
gathering pass.

The feature is exposed through a new `OutputFormat::Imrad` variant and a
matching CLI/TUI keyword (`imrad`), keeping the existing `report` format as the
default.

## Background

### Current `RESEARCH.md` structure

`assemble_document` in `crates/ragent-research/src/document.rs` emits these
sections in order:

1. `# Title: <title>`
2. `## Topic`
3. `## Search Queries`
4. `## Summary`
5. `## Findings`
6. `## Findings Relationship Diagram`
7. `## In-Project Cross-References`
8. `## Open Questions`
9. `## References Index`

### Mapping to IMRaD

| Current section | IMRaD role |
|-------------------|------------|
| Title + Topic | Introduction: research question and scope |
| Search Queries | Methods: how evidence was gathered |
| Summary | Results: high-level synthesis |
| Findings | Results: detailed results |
| Findings Relationship Diagram | Results: structural view of result dependencies |
| In-Project Cross-References | Discussion: relation to the local project |
| Open Questions | Discussion: remaining gaps |
| References Index | Back matter / Methods: corpus |

### Existing output formats

`OutputFormat` in `crates/ragent-research/src/run_config.rs` already supports
`Report`, `ExecutiveSummary`, `ComparisonTable`, and `SourceBibliography`.
Each format is selected at run time and drives both the synthesis prompt
(`analysis.rs`) and the document assembler (`document.rs`).

## Requirements

### FR-001 — IMRaD format availability (ubiquitous)

`/research create` **shall** support an `imrad` output format in addition to the
existing report formats.

> *Ubiquitous requirement — the new format option must always be available,
> regardless of provider, depth preset, or topic.*

### FR-002 — CLI/TUI keyword for IMRaD format (event-driven)

When the user selects `imrad` via `--format imrad`, `/research create --format
imrad`, or the TUI output-format picker, the system **shall** produce a report
whose body follows the IMRaD section order defined in this specification.

### FR-003 — Default format remains unchanged (state-driven)

When no output format is explicitly selected, the system **shall** continue to
default to the existing multi-section report format (`OutputFormat::Report`).

### FR-004 — IMRaD section headings (ubiquitous)

The IMRaD output **shall** contain exactly these top-level body sections, in
this order:

1. `# Title: <title>`
2. `## Abstract`
3. `## Introduction`
4. `## Methods`
5. `## Results`
6. `## Discussion`
7. `## References Index`

All sections are always present; empty sections render a placeholder so the
structure is stable for downstream tooling.

### FR-005 — Abstract content (event-driven)

When a non-empty summary is available, the `## Abstract` section **shall**
contain the summary text verbatim. When the summary is empty, the section **shall**
render a placeholder indicating that the abstract is awaiting the gathering pass.

### FR-006 — Introduction content (event-driven)

When the `ResearchDocument` contains a topic, the `## Introduction` section
**shall** contain the topic text, followed by a brief framing paragraph that
states the research objective. When the topic is empty, the section **shall**
render a placeholder.

### FR-007 ��� Methods content (event-driven)

When the `ResearchDocument` contains decomposed queries, the `## Methods`
section **shall** list them as search queries under a `### Search Queries`
sub-section. When the gathering pass was configured with `--disable-web`,
`--disable-local`, or a depth preset, the `## Methods` section **shall** include
a `### Research Configuration` sub-section describing the active constraints.
When no decomposed queries exist, the `### Search Queries` sub-section **shall**
render the same fallback note currently used in `## Search Queries`.

### FR-008 — Results content (event-driven)

When findings exist, the `## Results` section **shall** contain:

- a `### Summary` sub-section with the one-paragraph summary;
- a `### Findings` sub-section with the numbered findings, each rendered with
  the five required labeled paragraphs (`Headline`, `Observation`, `Analysis`,
  `Cross-reference / Dependencies`, `Implication`), source lists, and date-range
  lines exactly as in the current `report` format;
- a `### Findings Relationship Diagram` sub-section with the Mermaid diagram
  currently emitted between `## Findings` and `## In-Project Cross-References`.

When no findings exist, the `### Findings` sub-section **shall** render the
existing "no findings yet" placeholder and the diagram sub-section **shall** render
the existing empty-finding placeholder.

### FR-009 — Discussion content (event-driven)

When in-project cross-references or open questions exist, the `## Discussion`
section **shall** contain them as `### In-Project Cross-References` and
`### Open Questions` sub-sections respectively, preserving their current
rendering. When either is empty, the corresponding sub-section **shall** render
the current placeholder.

### FR-010 — References Index preservation (ubiquitous)

The `## References Index` section in the IMRaD output **shall** be identical in
content and rendering to the `## References Index` section in the current
`report` output, including the same table columns and date handling.

### FR-011 — Skeleton document support (state-driven)

When `render_skeleton` is invoked for a new research item with
`OutputFormat::Imrad`, it **shall** emit the IMRaD section structure with the
same placeholders used for empty data.

### FR-012 — Synthesis prompt adaptation (event-driven)

When `OutputFormat::Imrad` is selected, the synthesis prompt instructing the
LLM **shall** tell the model to produce the standard four sections
(`Summary`, `Findings`, `In-Project Cross-References`, `Open Questions`) in its
raw response, because the assembler — not the model — is responsible for the
IMRaD layout. The prompt **shall** also ask the model to phrase findings as
results-oriented statements suitable for an IMRaD results section.

### FR-013 — Backward compatibility (unwanted)

Selecting `OutputFormat::Imrad` **shall not** change the contents of the
`ResearchDocument` fields (`summary`, `findings`, `cross_references`,
`open_questions`, `sources`). Existing tests and consumers that build a
`ResearchDocument` directly and call `assemble_document` **shall** continue to
receive the legacy report layout when `OutputFormat::Report` is used.

### FR-014 — No new external dependencies (unwanted)

The implementation **shall not** add new third-party crate dependencies solely
for IMRaD layout; the restructuring uses the existing document assembly
infrastructure.

### FR-015 — Format parity (optional)

When feasible, the other existing formats (`ExecutiveSummary`,
`ComparisonTable`, `SourceBibliography`) **may** also be emitted in IMRaD order
by wrapping their generated content into the corresponding IMRaD sections,
without changing their internal structure. This is optional and must not delay
the primary `imrad` format implementation.

## Non-functional requirements

### NFR-001 — Testability

The IMRaD layout **shall** be unit-testable through `assemble_document` by
constructing a `ResearchDocument` with controlled fields and asserting on the
section order and headings in the returned `AssembledDocument::body`.

### NFR-002 — Documentation

`document.rs` module and public functions involved in the new format **shall**
receive updated doc comments explaining the IMRaD section order and which
`ResearchDocument` fields feed each section.

### NFR-003 — No regressions

Running `cargo test -p ragent-research` and `cargo clippy -p ragent-research`
after implementation **shall** produce no new test failures or warnings compared
to the baseline.

## Scope

### In scope

- Adding `OutputFormat::Imrad`.
- Extending `OutputFormat::parse`/`as_str` with `imrad`.
- Updating the synthesis prompt for `imrad` to keep the model's raw output
  stable and results-oriented.
- Implementing `assemble_document` branching for `OutputFormat::Imrad`.
- Adding skeleton support for `OutputFormat::Imrad`.
- Unit and integration tests verifying section order and content preservation.
- Updating crate-level docs and the research format report note (if present).

### Out of scope

- Changing the structure of the numbered finding paragraphs.
- Adding new metadata fields to `ResearchItem` or `ResearchDocument`.
- External publication workflows (PDF, LaTeX, DOCX export).
- Multi-language IMRaD variants.
- Changing the default output format.
