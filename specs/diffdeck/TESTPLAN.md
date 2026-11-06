---
status: draft
---
# Research Diff-Deck Manual Test Plan

Manual verification plan for `specs/diffdeck/SPEC.md`. Each case is executed by a
person against a real research item fixture; no automated test code is involved.

## Preconditions Common to All Cases

- A working ragent binary is built from the branch under test.
- The fixture root is a scratch research root under `target/temp/deckfixtures/`,
  set in the ragent config so every command below reads it instead of the live
  research tree.
- Fixture item `deckspec` contains exactly two dated reports:
  - `target/temp/deckfixtures/research/deckspec/RESEARCH-2025-01-10.md` (older)
  - `target/temp/deckfixtures/research/deckspec/RESEARCH-2025-02-14.md` (newer)
- The older report body is:

  ```
  # Deck Spec Fixture

  ## Executive Summary

  Retrieval augmented generation reduces hallucination in long-form answers.

  ## Top 10 Implications

  1. Index quality matters more than model size.
  2. Chunking strategy dominates retrieval recall.

  ## Open Questions

  - How much does chunk overlap matter?

  ## Findings

  ### **Finding 1** - Vector indexes accelerate similarity search

  Approximate nearest-neighbour indexes keep latency flat as corpora grow. [#1]

  ### **Finding 2** - Rerankers improve precision

  A cross-encoder reranker lifts precision at rank 10. [#2]

  ## In-Project Cross-References

  - `src/index.rs` implements the similarity search.

  ## References Index

  [#1] Indexing Survey, https://example.org/indexing-survey
  [#2] Reranking Notes, https://example.org/reranking
  ```

- The newer report body is:

  ```
  # Deck Spec Fixture

  ## Executive Summary

  Retrieval augmented generation reduces hallucination in long-form answers.

  ## Top 10 Implications

  1. Index quality matters more than model size.
  2. Chunking strategy dominates retrieval recall.
  3. Hybrid lexical and dense retrieval beats either alone. [#3]

  ## Open Questions

  - How much does chunk overlap matter?
  - When does a reranker cost more than it is worth?

  ## Findings

  ### **Finding 1** - Vector indexes accelerate similarity search

  Approximate nearest-neighbour indexes keep latency flat as corpora grow. [#1]

  ### **Finding 2** - Rerankers improve precision

  A cross-encoder reranker lifts precision at rank 10 and recall at rank 50. [#2]

  ### **Finding 3** - Hybrid retrieval closes the lexical gap

  Combining BM25 with dense vectors recovers rare-term queries. [#3]

  ## In-Project Cross-References

  - `src/index.rs` implements the similarity search.

  ## References Index

  [#1] Indexing Survey, https://example.org/indexing-survey
  [#2] Reranking Notes, https://example.org/reranking
  [#3] Hybrid Retrieval Report, https://example.org/hybrid-retrieval
  ```

- The `[#N]` citation numbers in the newer report deliberately differ from the
  older one for `[#3]` versus the older `[#2]` position, so identity matching is
  exercised against citation renumbering.

## Test Cases

### TC-001 Default run produces a cover, changed-element slides, and a summary

**Requirement:** FR-001, FR-002, FR-003, FR-005, FR-006, FR-010

**Preconditions:** Common fixture only; no `DECK-*.md` exists in the item directory.

**Steps:**
1. In the TUI, run `/research deck deckspec`.
2. Locate the file reported on the message line.
3. Open `target/temp/deckfixtures/research/deckspec/DECK-<today>.md`.

**Test data:** `/research deck deckspec`

**Expected results:**
- The reported path is
  `research/deckspec/DECK-<YYYY-MM-DD>.md`, and the stamp is today's UTC date.
- The file exists and was created through the atomic write-then-rename path (no
  `*.tmp` artifact remains in the item directory).
- The first slide is the cover and states the item title `Deck Spec Fixture`, the
  generation date, `Coverage: 2025-01-10 to 2025-02-14`, the research id
  `deckspec`, and the element-slide / change / `OLD` item counts.
- Slide sections are separated by a lone `---` line.
- Slides appear in the fixed precedence order: Executive Summary is absent (no
  change), Top Implications, Open Questions, Findings, Cross-References absent
  (no change), References.
- The last slide is the closing summary listing three element slides, the
  per-status counts, and the element names that produced a slide.

### TC-002 Status tags are assigned exactly per FR-004

**Requirement:** FR-004, FR-008, FR-009

**Preconditions:** TC-001 has run and `DECK-<today>.md` exists.

**Steps:**
1. Open the Top Implications slide.
2. Open the Open Questions slide.
3. Open the Findings slide.
4. Open the References slide.

**Test data:** the fixture pair from the common preconditions.

**Expected results:**
- Top Implications slide lists all three items: items 1 and 2 (identical in both
  reports) tagged `OLD`, item 3 tagged `ADDED` because it is newer-only **and**
  cites `https://example.org/hybrid-retrieval`, a source the older report did not
  cite.
- Open Questions slide lists both questions: the overlap question tagged `OLD`,
  the reranker-cost question tagged `NEW` (newer-only, no new source cited).
- Findings slide lists all three findings: Finding 1 `OLD` (identical text),
  Finding 2 `UPDATE` (same headline, changed body text), Finding 3 `ADDED`
  (newer-only and cites `https://example.org/hybrid-retrieval`).
- References slide lists all three entries: `[#1]` and `[#2]` tagged `OLD`,
  `[#3]` tagged `ADDED`.
- No item carries more than one tag, and no tag outside `NEW` / `ADDED` /
  `UPDATE` / `OLD` appears anywhere in the file.

### TC-003 A changed element lists every item, not only the deltas

**Requirement:** FR-008

**Preconditions:** `DECK-<today>.md` from TC-001.

**Steps:**
1. Count the bullets on the Findings slide.
2. Count the bullets on the Top Implications slide.

**Test data:** the fixture pair from the common preconditions.

**Expected results:**
- The Findings slide contains three bullets (Finding 1 `OLD`, Finding 2
  `UPDATE`, Finding 3 `ADDED`), not one or two.
- The Top Implications slide contains three bullets (two `OLD`, one `ADDED`).
- Cross-References produces **no** slide, because its only item is unchanged and
  `--include-old` was not supplied.

### TC-004 `--include-old` restores the unchanged elements

**Requirement:** FR-014, FR-015

**Preconditions:** `DECK-<today>.md` from TC-001 exists.

**Steps:**
1. Run `/research deck deckspec --include-old --force`.
2. Open the regenerated file.

**Test data:** `/research deck deckspec --include-old --force`

**Expected results:**
- The Executive Summary slide now appears, with its single paragraph tagged
  `OLD`.
- The Cross-References slide now appears, with its single bullet tagged `OLD`.
- Every other slide is unchanged in content and order except the summary counts,
  which now include the two extra `OLD` items and report five element slides.

### TC-005 `--elements` restricts the deck and rejects unknown names

**Requirement:** FR-014

**Preconditions:** `DECK-<today>.md` from TC-001 exists.

**Steps:**
1. Run `/research deck deckspec --elements "Findings,References" --force`.
2. Open the regenerated file.
3. Run `/research deck deckspec --elements "Findings,Glossary" --force`.

**Test data:** `Findings,References` then `Findings,Glossary`

**Expected results:**
- Step 1 restricts the deck to two element slides, Findings then References, in
  precedence order; the summary reports two element slides.
- Step 3 is refused. The message names the offending value `Glossary` and lists
  the six accepted names: Executive Summary, Top Implications, Open Questions,
  Findings, Cross-References, References.
- The file written in step 1 is byte-identical after the refused step 3 run.

### TC-006 `--from` and `--to` select an explicit pair with adjacent pairing

**Requirement:** FR-007, FR-020

**Preconditions:** Add a third report
`target/temp/deckfixtures/research/deckspec/RESEARCH-2025-03-20.md` whose body is
byte-identical to `RESEARCH-2025-02-14.md`.

**Steps:**
1. Run `/research deck deckspec --to 2025-02-14 --force`.
2. Read the cover slide.
3. Run `/research deck deckspec --from 2025-02-14 --force`.
4. Read the cover slide.
5. Run `/research deck deckspec --from 2025-04-31 --force`.
6. Run `/research deck deckspec --from 2025-01-11 --force`.

**Test data:** dates `2025-02-14`, `2025-04-31`, `2025-01-11`

**Expected results:**
- Step 1 pairs `--to 2025-02-14` with the next older report, so the coverage
  window is `Coverage: 2025-01-10 to 2025-02-14`.
- Step 3 pairs `--from 2025-02-14` with the next newer report, so the coverage
  window is `Coverage: 2025-02-14 to 2025-03-20`; the diff is empty (identical
  bodies) and the deck carries the no-change notice.
- Step 5 is refused: `2025-04-31` is not a valid date. The message names the
  offending value.
- Step 6 is refused: the stamp is valid but no `RESEARCH-2025-01-11.md` exists.
  The message names the offending value and lists the report dates that do exist
  (`2025-01-10`, `2025-02-14`, `2025-03-20`).

### TC-007 Refusal paths leave the item directory untouched

**Requirement:** FR-011, FR-012, FR-018, FR-019

**Preconditions:** Three sibling items are prepared:
`oneshot` with a single `RESEARCH-2025-02-01.md`, `legacyonly` with only a
`RESEARCH.md`, and `corrupt` with `RESEARCH-2025-01-05.md` and a
`RESEARCH-2025-02-05.md` that contains no recognisable `## ` top-level sections.
A fourth item `frozen` holds two valid reports plus a pre-existing
`DECK-2025-02-20.md` whose bytes are recorded with a checksum; then its newer
report is replaced with a Latin-1 encoded file (invalid UTF-8).

**Steps:**
1. Run `/research deck oneshot`.
2. Run `/research deck legacyonly`.
3. Run `/research deck nosuchitem`.
4. Run `/research deck corrupt`.
5. Run `/research deck frozen --force`.
6. Record a directory listing and the checksums of every file in each item
   directory.

**Test data:** the four prepared items

**Expected results:**
- Step 1 is refused, naming `oneshot` and "1 report found".
- Step 2 is refused, naming the legacy scheme `RESEARCH.md` and stating that two
  reports are required.
- Step 3 is refused, naming `nosuchitem`; no directory was created.
- Step 4 is refused, naming the unreadable file's full path and the reason
  (no recognised top-level sections).
- Step 5 is refused, naming the file path and the reason (not valid UTF-8).
- Step 6 shows every listing and checksum identical to before the run: no
  `DECK-*.md` was created in `oneshot`, `legacyonly` or `corrupt`, and
  `frozen/DECK-2025-02-20.md` is byte-identical.

### TC-008 `--out` and `--force` control the destination

**Requirement:** FR-016, FR-017

**Preconditions:** `DECK-<today>.md` from TC-001 exists in the item directory.

**Steps:**
1. Run `/research deck deckspec --out target/temp/deckfixtures/out/nested/deck.md`
   twice, the second time without `--force`.
2. Run `/research deck deckspec` again without any flag.
3. Run `/research deck deckspec --force`.

**Test data:** the nested output path, then the default path

**Expected results:**
- Step 1's first run writes
  `target/temp/deckfixtures/out/nested/deck.md` and creates both missing parent
  directories; the second run is refused, naming the existing path and stating
  that `--force` overwrites it.
- Step 2 is refused, naming the existing
  `research/deckspec/DECK-<today>.md` and the flag that overwrites it.
- Step 3 succeeds and rewrites the default path.

### TC-009 Determinism, no-LLM guarantee, and Markdown safety

**Requirement:** FR-021, FR-022, NFR-001, NFR-002

**Preconditions:** The `deckspec` fixture, plus a second item `hostile` whose
newer report adds a Top Implications item reading
`Pipe | and ` + "`backtick`" + ` and a # leading hash followed by a very long
sentence of more than 200 characters, and a research root that has no LLM
provider configured.

**Steps:**
1. Run `ragent research deck deckspec --out target/temp/deckrun1.md --force`.
2. Run `ragent research deck deckspec --out target/temp/deckrun2.md --force`.
3. Compare the two files.
4. Configure a provider, restart ragent, and repeat step 1 writing
   `target/temp/deckrun3.md`.
5. Run `ragent research deck hostile --force` and open the Top Implications
   slide.
6. Time the `deckspec` run with `time`.

**Test data:** items `deckspec` and `hostile`

**Expected results:**
- Step 3 shows the two files are byte-identical, including the generation date
  (same calendar day).
- Step 4's file is byte-identical to step 1's, and no provider log line records
  an LLM request during any deck run.
- Step 5 shows the hostile bullet intact on a single bullet line: the pipe is
  escaped so it cannot start a table, the backtick is escaped so it cannot open a
  code span, the leading hash is escaped so it cannot create a heading, and the
  bullet is truncated to 200 characters ending in an ellipsis. The slide count
  and the following slide are unaffected.
- Step 6 reports a wall-clock time under 100 ms for the fixture pair.
