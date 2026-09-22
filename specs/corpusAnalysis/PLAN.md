---
id: corpusAnalysis
status: draft
---

# PLAN: Source-Quality Scoreboard for /research Documents

Implementation plan for `specs/corpusAnalysis/SPEC.md`.

## Approach

All work lands in `crates/ragent-research`. The scoreboard is a new pure
rendering module that consumes `ResearchDocument` fields which already exist
(`corpus_critic`, `synthesis_audit`, `cite_check`, `sources`). `assemble_report_body`
(document.rs) and `assemble_imrad_body` (document.rs) each insert the rendered
block immediately after the title/frontmatter and before the Abstract / first
body section. No scoring code is modified and no LLM is invoked.

Key existing symbols to build on:

- `ResearchDocument` fields: `corpus_critic: Option<CorpusCriticReport>`,
  `synthesis_audit: Option<SynthesisAudit>`, `cite_check: Option<CitationCheckResult>`,
  `sources: Vec<Source>`, `output_format: OutputFormat` (document.rs)
- Renderer seam: `assemble_report_body` (document.rs:893),
  `assemble_imrad_body` (document.rs:1189)
- Data accessors: `Source::relevance_rank()`, `Source::has_body()`,
  `Source::published_at()`, `Source::type_str()` (source.rs)

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Add grade-band + meter-bar helpers to the scoreboard module | FR-002, FR-003, FR-016 | S | Critical | completed | — |
| T-002 | Implement the Corpus Quality Scoreboard renderer (score line, meter block, subscore line, source-facts line, tension/citation line) | FR-001, FR-004, FR-005, FR-006, FR-007, FR-009, FR-010 | M | Critical | completed | T-001 |
| T-003 | Insert scoreboard into `assemble_report_body` (report layout) at the FR-011 position | FR-011, FR-012 | S | High | completed | T-002 |
| T-004 | Insert scoreboard into `assemble_imrad_body` (IMRaD layout) at the FR-011 position | FR-011, FR-012 | S | High | completed | T-002 |
| T-005 | Apply abbreviated-format and local-only-run reductions per FR-013/FR-014 | FR-013, FR-014 | S | Medium | completed | T-002 |
| T-006 | Wire scoreboard fields through the TUI `/research open` overlay path | FR-008 | S | Medium | pending | T-003, T-004 |
| T-007 | Document the scoreboard in the research module docs | FR-001 | S | Low | pending | T-004 |
## Notes

- T-001/T-002 should be a single new module (e.g.
  `crates/ragent-research/src/scoreboard.rs`) declared in `lib.rs`, keeping
  `document.rs` changes limited to the two assembly insertion points.
- T-006 is expected to be a no-code-change verification in most cases: the
  overlay renders the assembled markdown verbatim; the task exists to prove it
  (manual test TC-006) and to fix any bypass-path regression.