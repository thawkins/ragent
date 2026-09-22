# Implementation Plan: Research Findings Relationship Diagram

**Spec ID:** `rsearchdiag`
**Spec status:** draft

## Overview

This plan implements the findings relationship diagram specified in
`specs/rsearchdiag/SPEC.md`. The work is confined to the
`ragent-research` crate, primarily `src/document.rs` and a new `src/diagram.rs`
module, with a new test file under `tests/`.

The implementation reuses the existing `Finding N` dependency parsing logic
from `reorder_findings_by_dependency` in `analysis.rs` (same regex
`\bfinding\s+(\d+)\b`) but does **not** modify that function — the diagram
generator is a new, independent pure function.

## Architecture

```
ResearchDocument (findings: Vec<String>)
        │
        ▼
assemble_document()  ──────────────►  document.rs (existing)
        │
        │  after rendering ## Findings
        │
        ▼
render_findings_diagram(&findings) ──► diagram.rs (NEW)
        │
        │  1. Parse Finding N refs from each finding's
        │     Cross-reference / Dependencies paragraph
        │  2. Build edge list (referer → referenced)
        │  3. Classify edge strength from phrasing
        │  4. Compute in-degrees for font-size class
        │  5. Emit Mermaid flowchart TD block
        │
        ▼
## Findings Relationship Diagram section in RESEARCH.md
```

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Create `diagram.rs` module with `render_findings_diagram` pure function | FR-003, FR-004, FR-006, NFR-003 | M | Critical | completed | — |
| T-002 | Implement edge extraction from `Finding N` references in dependency paragraphs | FR-006, FR-009, FR-010 | M | Critical | completed | T-001 |
| T-003 | Implement edge strength classification (strong / weak / default) from dependency phrasing | FR-007 | S | High | completed | T-002 |
| T-004 | Implement node font-size class assignment based on in-degree | FR-008 | S | High | completed | T-002 |
| T-005 | Implement Mermaid label escaping for special characters | FR-015 | S | High | completed | T-001 |
| T-006 | Handle zero-findings placeholder (no Mermaid block) | FR-005, FR-013 | S | Critical | completed | T-001 |
| T-007 | Handle single-finding with no edges (single node, no edges) | FR-016 | S | Medium | completed | T-001 |
| T-008 | Wire `render_findings_diagram` into `assemble_document` between `## Findings` and `## In-Project Cross-References` | FR-001, FR-002, FR-012 | S | Critical | completed | T-001, T-006 |
| T-009 | Add `## Findings Relationship Diagram` section to `render_skeleton` path | FR-013 | S | High | completed | T-008 |
| T-010 | Verify diagram generation for all `OutputFormat` variants | FR-014 | S | Medium | completed | T-008 |
| T-011 | Add `diagram` module to `lib.rs` | — | S | Critical | completed | T-001 |
| T-012 | Write unit tests for `render_findings_diagram` (multi-finding, edges, linkStyle, classDef, escaping, self-loops, out-of-range) | FR-004, FR-006, FR-007, FR-008, FR-009, FR-010, FR-015 | M | High | completed | T-001, T-002, T-003, T-004, T-005 |
| T-013 | Write integration test verifying `## Findings Relationship Diagram` section appears in assembled `RESEARCH.md` | FR-001, FR-002, FR-012 | S | High | completed | T-008 |
| T-014 | Write integration test for skeleton document containing the diagram section placeholder | FR-013 | S | Medium | completed | T-009 |
| T-015 | Run `cargo test -p ragent-research` and `cargo clippy -p ragent-research` to confirm no regressions | NFR-001, NFR-002 | S | Critical | completed | T-012, T-013, T-014 |
## Task detail

### T-001 — Create `diagram.rs` module

Create `crates/ragent-research/src/diagram.rs`. The module exposes one public
function:

```rust
/// Render a Mermaid `flowchart TD` diagram showing the dependency
/// relationships between the supplied findings.
///
/// Each finding is rendered as a node `F<n>["Finding <n> — <headline>"]`.
/// Each `Finding N` reference in a finding's **Cross-reference /
/// Dependencies** paragraph becomes a directed edge `F<referer> --> F<N>`.
///
/// Returns an empty `String` when `findings` is empty (the caller is
/// responsible for emitting the placeholder text).
pub fn render_findings_diagram(findings: &[String]) -> String
```

The function returns the full Mermaid code block content (without the
surrounding ` ```mermaid ` fence — the caller adds the fence so it controls
the info string).

### T-002 — Edge extraction

Parse each finding body for the **Cross-reference / Dependencies** paragraph,
then scan that paragraph for `Finding N` references using
`Regex::new(r"(?i)\bfinding\s+(\d+)\b")`. Build an edge list of
`(from_idx, to_idx)` tuples where `from_idx` is the 0-based index of the
finding containing the reference and `to_idx` is `N - 1`.

Filter out:
- Self-loops (`from_idx == to_idx`) — FR-009.
- Duplicate edges (same `from_idx, to_idx` pair) — FR-009.
- Out-of-range references (`N == 0` or `N > findings.len()`) — FR-010.

### T-003 — Edge strength classification

After extracting the raw `Finding N` references, inspect the surrounding
text in the **Cross-reference / Dependencies** paragraph to classify the
edge:

- **Strong** (stroke-width:4px) — phrasing contains "builds on", "depends
  on", "prerequisite to", "requires".
- **Weak** (stroke-width:1.5px) — phrasing contains "contradicts", "relates
  to", "see also", "compare".
- **Default** (stroke-width:2px) — anything else, or when the phrasing
  cannot be classified.

Emit `linkStyle <index> stroke-width:<N>px` lines after the edge
declarations, where `<index>` is the 0-based position of the edge in
declaration order.

### T-004 — Node font-size class

Compute the in-degree of each node (number of edges pointing to it). Define
two `classDef` entries:

```text
classDef central font-size:15px,font-weight:bold;
classDef normal font-size:12px;
```

Apply `classDef central` to nodes with in-degree ≥ 2 (FR-008). Apply
`classDef normal` to all others.

### T-005 — Label escaping

Finding headlines may contain characters that break Mermaid node labels
(`"`, `|`, `[`, `]`, newlines). Sanitise the headline before embedding it in
the node label:

- Replace `"` with `'`.
- Replace `|` with `&#124;`.
- Replace `[` and `]` with `(` and `)` respectively.
- Truncate to 80 characters with `…` suffix if longer.
- Strip newlines (replace with space).

### T-006 — Zero-findings placeholder

When `findings.is_empty()`, `render_findings_diagram` returns an empty
`String`. The caller in `assemble_document` checks for this and emits the
placeholder text instead of a Mermaid block.

### T-007 — Single-finding handling

When `findings.len() == 1` and no edges are extracted, the diagram renders
a single `F1["Finding 1 — <headline>"]` node with no edges and no
`linkStyle` lines. The section is still emitted (not omitted).

### T-008 — Integration into `assemble_document`

In `assemble_document` (`document.rs`), after the `## Findings` rendering
loop and before the `## In-Project Cross-References` section, insert:

```rust
// ── Findings Relationship Diagram ───────────���────────────────────────
body.push_str("## Findings Relationship Diagram\n\n");
let diagram = crate::diagram::render_findings_diagram(&doc.findings);
if diagram.is_empty() {
    body.push_str("_(no findings yet — the gathering pass will populate this section)_\n\n");
} else {
    body.push_str("```mermaid\n");
    body.push_str(&diagram);
    body.push_str("```\n\n");
}
```

### T-009 — Skeleton document

`render_skeleton` constructs a `ResearchDocument` with empty findings and
calls `assemble_document`, so the placeholder path from T-008 covers this
automatically. Verify with a test (T-014).

### T-010 — Output format verification

`assemble_document` is called for all `OutputFormat` variants. The diagram
section is inserted unconditionally after `## Findings`, so all formats
receive it. Add a parametrised test that constructs a `ResearchDocument` for
each `OutputFormat` variant and asserts the section is present.

### T-011 — Module registration

Add `mod diagram;` to `crates/ragent-research/src/lib.rs` (alphabetically
between `cli` and `document`).

### T-012–T-014 — Tests

All tests go in `crates/ragent-research/tests/` per project convention:

- `test_diagram.rs` — unit tests for `render_findings_diagram`.
- Extend `test_research_create_synthesis.rs` or a new
  `test_diagram_integration.rs` — integration test asserting the section
  appears in an assembled document.

### T-015 — Validation

Run:

```bash
cargo test -p ragent-research -- --nocapture
cargo clippy -p ragent-research
cargo fmt --check -p ragent-research
```

## Risks

| Risk | Mitigation |
|------|------------|
| Mermaid syntax breakage from unusual finding headlines | T-005 escaping + T-012 escaping tests |
| Large finding sets (50+) produce unwieldy diagrams | NFR-001 performance target; diagram is still readable as Mermaid supports zoom |
| Dependency paragraph phrasing classification is imperfect | FR-007 defines clear keyword lists; default fallback is safe |
| Existing tests that assert exact `RESEARCH.md` body content break | T-015 runs full crate test suite; update any snapshot-style assertions |