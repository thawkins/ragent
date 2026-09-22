# Implementation Plan: IMRaD-Compliant `/research` Report Output

**Spec ID:** `imradreport`
**Spec status:** draft

## Overview

This plan implements the IMRaD output format specified in
`specs/imradreport/SPEC.md`. The work is confined to the `ragent-research`
crate, primarily `src/run_config.rs`, `src/analysis.rs`, and `src/document.rs`,
plus a new integration test file under `tests/`.

The implementation reuses the existing `ResearchDocument` fields and rendering
helpers. It adds a new `OutputFormat` variant and a dedicated IMRaD document
assembly path. The model's raw response requirements remain unchanged; only the
final assembled document structure changes.

## Architecture

```
User selects --format imrad
         │
         ▼
SessionConfig.output_format = OutputFormat::Imrad
         │
         ├────────────────────────┐
         ▼                        ▼
SynthesisPromptBuilder         assemble_document()
(analysis.rs)                  (document.rs)
- keeps 4 raw sections          │
  (Summary, Findings,          ├─ OutputFormat::Report → legacy layout
   Cross-References,           └─ OutputFormat::Imrad  → assemble_imrad()
   Open Questions)
- adds results-orientation      
  guidance only
                               assemble_imrad()
                               ├─ ## Abstract      ← summary
                               ├─ ## Introduction  ← topic + objective
                               ├─ ## Methods       ← search queries + config
                               ├─ ## Results       ← summary + findings + diagram
                               ├─ ## Discussion    ← cross-references + questions
                               └─ ## References Index
```

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Add `OutputFormat::Imrad` variant and `imrad` parsing to `run_config.rs` | FR-001, FR-002 | S | Critical | completed | — |
| T-002 | Update synthesis prompt for `OutputFormat::Imrad` to keep raw sections stable and results-oriented | FR-012 | M | High | completed | T-001 |
| T-003 | Implement `assemble_imrad` pure layout function in `document.rs` | FR-004, FR-005, FR-006, FR-007, FR-008, FR-009 | M | Critical | completed | T-001 |
| T-004 | Wire `assemble_document` to dispatch to `assemble_imrad` for `OutputFormat::Imrad` | FR-002, FR-003 | S | Critical | completed | T-003 |
| T-005 | Add `OutputFormat::Imrad` support to `render_skeleton` | FR-011 | S | Medium | completed | T-003, T-004 |
| T-006 | Write unit tests for `assemble_imrad` section ordering and placeholder handling | NFR-001 | M | High | completed | T-003 |
| T-007 | Write integration test verifying IMRaD output from a full `/research create` path | FR-002, NFR-001 | M | High | completed | T-004, T-005 |
| T-008 | Run `cargo test -p ragent-research`, `cargo clippy -p ragent-research`, and `cargo fmt --check -p ragent-research` | NFR-003 | S | Critical | completed | T-006, T-007 |
| T-009 | Update crate docs and existing report format research note | NFR-002 | S | Low | completed | T-008 |
## Task detail

### T-001 — Add `OutputFormat::Imrad`

In `crates/ragent-research/src/run_config.rs`:

- Add `Imrad` to the `OutputFormat` enum.
- Extend `OutputFormat::parse` to accept `imrad`, `im-rad`, and `scientific`.
- Extend `OutputFormat::as_str` to return `"imrad"`.
- Add a unit test for parsing and the `as_str` round-trip.

### T-002 — Update synthesis prompt

In `crates/ragent-research/src/analysis.rs`, inside `render_output_template`:

- Add an explicit `Some(OutputFormat::Imrad)` branch before the fallthrough
  `_` branch.
- The branch emits the same four raw section instructions as the default
  (`Summary`, `Findings`, `In-Project Cross-References`, `Open Questions`) so
  the parser and downstream assembler stay unchanged.
- Append an extra paragraph: findings should be phrased as results-oriented
  statements suitable for the Results section of an IMRaD report.

### T-003 — Implement `assemble_imrad`

In `crates/ragent-research/src/document.rs`, add a new private helper:

```rust
fn assemble_imrad(doc: &ResearchDocument) -> String
```

Responsibilities:

- Build the body string without frontmatter (frontmatter is handled by the
  caller).
- Render sections in the order specified in FR-004.
- Reuse existing helpers where possible:
  - Topic and summary rendering.
  - Search query rendering.
  - `normalize_finding_labels`, `extract_headline`, `render_finding_sources`.
  - `crate::diagram::render_findings_diagram`.
  - `ResearchIo::render_references_index`.
- Preserve all placeholders exactly as in the legacy path.

### T-004 — Wire `assemble_document` dispatch

At the start of `assemble_document`, branch on `doc.output_format`:

- `OutputFormat::Report` (and other existing formats for now) → existing body
  assembly.
- `OutputFormat::Imrad` → use `assemble_imrad(doc)` for the body, then prepend
  frontmatter and build `AssembledDocument` as usual.

### T-005 — Skeleton support

In `render_skeleton`, accept `OutputFormat` as a parameter (or overload the
function with a second variant). The TUI/CLI creation path that writes the empty
skeleton must pass the configured output format. Update callers in `manager.rs`
and `session.rs` accordingly.

### T-006 — Unit tests

Add `crates/ragent-research/tests/test_assemble_imrad.rs`:

- Construct `ResearchDocument` with known fields.
- Assert `AssembledDocument::body` contains `## Abstract`, `## Introduction`,
  `## Methods`, `## Results`, `## Discussion`, and `## References Index` in order.
- Assert the legacy `## Topic` and `## Findings` headings do not appear as
  top-level headings in the IMRaD body.
- Assert findings with citations still render `**Sources:**` and
  `**Source date range:**` lines.
- Assert empty data renders placeholders for each section.

### T-007 — Integration test

Extend or mirror `crates/ragent-research/tests/test_research_create_synthesis.rs`:

- Create a research session with `OutputFormat::Imrad`.
- Use a mock `AnalysisEngine` returning a well-formed result.
- Assert the written `RESEARCH.md` contains the IMRaD top-level sections.

### T-008 — Validation

Run the standard validation commands and fix any new warnings or test
failures:

```bash
cargo test -p ragent-research -- --nocapture
cargo clippy -p ragent-research
cargo fmt --check -p ragent-research
```

### T-009 — Documentation

- Update the module doc comment in `document.rs` to mention the IMRaD layout.
- Update the `researchformat/RESEARCH.md` finding note about IMRaD to reference
  the new native output format once implemented.
- Add a short note to `docs/userdocs/research.md` if it exists; otherwise skip.

## Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| Existing tests assert exact `RESEARCH.md` body strings | High | T-008 runs full crate test suite; update only IMRaD-specific snapshot assertions |
| `render_skeleton` callers pass no output format | Medium | T-005 keeps `OutputFormat::Report` as the default argument in a wrapper |
| TUI format picker must display the new option | Low | T-001 exposes the new `as_str`; TUI format list uses the enum variants |
| Prompt changes shift model output quality | Medium | T-002 keeps raw section instructions identical; only adds phrasing guidance |