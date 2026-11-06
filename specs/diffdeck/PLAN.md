---
status: draft
---
# Research Diff-Deck Implementation Plan

## Overview

Implements `specs/diffdeck/SPEC.md`: a deterministic, LLM-free diff deck for a
research item. Two staged pure functions do the work -
`diff_reports(older, newer) -> DeckDiff` and `render_deck(&DeckDiff) -> String` -
and a thin orchestration layer resolves the item, picks the two report files, and
writes `DECK-<YYYY-MM-DD>.md` through `ResearchIo::atomic_write`. Both the TUI
slash command and the CLI sub-command call the same orchestration function so the
two surfaces cannot drift (ANTIPAT M3.10).

Placement: a new module `crates/ragent-research/src/deck.rs`, re-exported from
`crates/ragent-research/src/lib.rs`; the verb is added to
`ResearchCliCommand` in `crates/ragent-research/src/cli.rs`; the TUI arm goes in
`crates/ragent-tui/src/app/research.rs`; the catalogue entry for `/research`
sub-commands goes in `crates/ragent-agent/src/tool/command_catalog.rs`.

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Add `crates/ragent-research/src/deck.rs` with the element model: a `DeckElement` enum of the six section kinds, its `label()` and the FR-003 precedence ordering, and the `parse_report(text) -> ParsedReport` splitter that walks `## ` headings and collects each section's item units, recording a warning for unknown sections and refusing when no recognised top-level section exists | FR-003, FR-009, FR-019 | M | Critical | completed | — |
| T-002 | Implement per-section item extraction matching the element-to-unit mapping table: paragraph units for Executive Summary, numbered items for Top Implications, bullet items for Open Questions and In-Project Cross-References, `### **Finding N** -` blocks for Findings, and `[#N]` entries for References Index, reusing the headline extraction logic already in `document.rs` | FR-009 | M | Critical | completed | T-001 |
| T-003 | Implement `normalize_identity(item) -> String` (strip `[#N]` citations, Markdown emphasis, backticks, collapse whitespace, lowercase; findings keyed by extracted headline, references keyed by normalised URL) and the section-scoped `HashMap` index used to match items between reports | FR-009, NFR-001 | S | Critical | completed | T-002 |
| T-004 | Implement `condense_bullet(item) -> String`: first sentence of the item text, whitespace-collapsed, Markdown-significant characters (`\ | `, backtick, leading `#`) escaped, capped at 200 characters with a trailing ellipsis | M | Medium | completed | Pending |
| T-005 | Implement `diff_reports(older, newer) -> DeckDiff` assigning each item exactly one `ItemStatus` (`NEW`, `ADDED`, `UPDATE`, `OLD`) under the FR-004 definitions, including the source-set comparison that promotes a newer-only item to `ADDED` | FR-004, FR-008, FR-013 | M | Critical | completed | T-003 |
| T-006 | Implement `render_deck(&DeckDiff) -> String`: cover slide (title, generation date, coverage window, research id, element-slide/change/`OLD` counts), one `---`-separated slide per changed element in FR-003 precedence with every item of that element labelled, the `_No changes detected between the two reports._` notice for an empty diff, and the closing summary slide (element count, per-status counts, element names) | FR-003, FR-005, FR-008, FR-010, FR-013 | M | High | completed | T-004, T-005 |
| T-007 | Implement report selection: enumerate `research/<name>/`, classify dated versus legacy names via the existing `io::is_dated_research_file_name`, order lexicographically descending, and pick the default newest pair, the `--from` / `--to` pair with adjacent-report pairing, `RESEARCH.md` as oldest baseline, and the FR-011 / FR-012 / FR-020 refusals | FR-006, FR-007, FR-011, FR-012, FR-020 | M | Critical | completed | — |
| T-008 | Implement the orchestration entry point `ResearchManager::deck(&self, name, options) -> Result<DeckOutcome>`: item existence check, report selection, read both reports (UTF-8 + parse validity), diff, element filter, `--include-old` handling, `--force` guard, `--out` path resolution with parent creation, and `ResearchIo::atomic_write` to the resolved output path | FR-002, FR-011, FR-014, FR-015, FR-016, FR-017, FR-018, FR-019, FR-021 | M | Critical | completed | T-006, T-007 |
| T-009 | Add `Deck { name, from, to, elements, include_old, out, force }` to `ResearchCliCommand` with hand-written parsing in `cli.rs`, including `--elements` validation against the six element names and `--from` / `--to` `YYYY-MM-DD` stamp validation | FR-001, FR-014, FR-015, FR-016, FR-017, FR-020 | S | High | completed | T-001 |
| T-010 | Wire the `ResearchCliCommand::Deck` arm in `crates/ragent-tui/src/app/research.rs`: spawn the orchestration on the tokio handle, publish a `TextDelta` reporting `From: /research deck` with the written path and per-status counts, and publish an `AgentNotice` on success and on refusal | FR-001, FR-010 | S | High | completed | T-008, T-009 |
| T-011 | Wire the `ragent research deck` CLI sub-command in `src/cli.rs` to the same orchestration function, emitting the existing `ragent-research:` JSON event line plus a human summary, and keep both surfaces on one function | FR-001, FR-021 | S | High | completed | T-008, T-009 |
| T-012 | Extend the slash-command catalogue: add `deck` to `RESEARCH_SUBS` and the deck flags to `RESEARCH_FLAGS` in `command_catalog.rs`, and add the verb to the `/research help` text so completion and help agree with the parser | FR-001, FR-014, FR-015, FR-016, FR-017 | S | Medium | completed | T-009 |
| T-013 | Add tests under `crates/ragent-research/tests/` (one `test_deck_*.rs` file per concern, registered in `tests/all.rs`): status assignment for each of the four tags, slide precedence and completeness (changed plus `OLD` items), empty-diff rendering, bullet escaping and 200-character cap, default pair selection, legacy baseline handling, and every refusal path | FR-004, FR-006, FR-008, FR-011, FR-012, FR-013, FR-014, FR-017, FR-018, FR-019, FR-020, FR-022 | M | High | completed | T-005, T-006, T-007, T-008 |
| T-014 | Add a determinism and budget assertion: the same two input reports render byte-identically across two invocations (and across a provider-configured versus provider-free environment), and a diff over 2000 items completes inside the NFR-001 budget | FR-021, NFR-001, NFR-002 | S | Medium | completed | T-008, T-013 |
| T-015 | Confirm the no-`unsafe` / no-`unwrap`-on-production-path rule on every new code path and add the deck module's `//!` docblock naming its dependencies, with `diff_reports` / `render_deck` exposed as pure functions over in-memory strings | NFR-003, NFR-004 | S | Medium | completed | T-008 |
| T-016 | Update documentation: the `/research` section of `docs/howtos/research.md` and root `SPEC.md` (deck verb, status tag definitions, file naming, coverage window), plus a `CHANGELOG.md` entry | FR-001, FR-002, FR-004, FR-005 | S | Low | completed | T-011, T-012 |
## Sequencing

1. **Model first (T-001..T-004).** The parser, identity, condenser, and escaping
   are pure and dependency-free; land them and their unit tests before any I/O
   exists.
2. **Diff then render (T-005, T-006).** Status assignment must be settled before
   rendering, because the cover and summary counts come straight from `DeckDiff`,
   and the slide set is exactly the elements with a non-`OLD` item.
3. **Selection and orchestration (T-007, T-008).** Report selection is the only
   filesystem-aware piece besides the final write; keep it behind
   `ResearchManager` so tests can drive it with a temp research root.
4. **Surfaces (T-009..T-012).** Parser, TUI arm, CLI arm, catalogue. All three
   call one orchestration function; no surface re-implements selection.
5. **Verification and docs (T-013..T-016).**

## Risks

| Risk | Mitigation |
| --- | --- |
| Findings headlines are model-generated, so a re-run can reword a finding with no substantive change, producing spurious `UPDATE` labels | Identity is the headline and `UPDATE` requires the normalised text to differ; the deck reports the per-status counts so a reader can judge. FR-004 defines the tags on observed text, so this is not a correctness bug for this spec |
| Section headings drift in older reports (e.g. `## Findings` renamed) | The parser ignores unknown sections and records a warning line on the cover slide; FR-019 fails only when *no* recognised section is found |
| A legacy-only item produces a one-report series | FR-012 covers it: `RESEARCH.md` is the oldest baseline, and a lone legacy file refuses with the scheme named |
| `--elements` naming drifts between parser, catalogue, and help text | T-009 and T-012 share one element-name list; a single test asserts the parser's accepted set matches the catalogue's advertised set |
| Deck grows unboundedly for a large report pair | NFR-001 bounds the work, not the size; bullets are capped at 200 characters (FR-022) and `OLD`-only elements are suppressed unless `--include-old` (FR-015) |
| A refusal leaves a half-written deck behind | Every refusal is decided before the single `ResearchIo::atomic_write` call, so an existing `DECK-*.md` stays byte-identical (FR-019) |

## Notes

- Do not add a new inline `#[cfg(test)]` module; every test file is registered
  via one `#[path = "test_x.rs"] mod test_x;` line in
  `crates/ragent-research/tests/all.rs`.
- Do not weaken the atomic-write guarantee to gain a faster path: one
  `ResearchIo::atomic_write` call, as every other research artifact uses.
- Element matching must be keyed by a hash map, never a nested linear scan
  (NFR-001).
- Run `cargo fmt` after every `.rs` edit and `cargo clippy` before finishing.