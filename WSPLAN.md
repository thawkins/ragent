# WSPLAN — `old_str not found` Remediation Plan

## 1. Executive Summary

The `edit` and `multiedit` tools (and the related `memory_write` / `memory_replace` tools) report `old_str not found` when the literal text supplied by the LLM no longer byte-matches the file content. The codebase already contains a five-pass matcher (`find_replacement_range` in `crates/ragent-tools-core/src/edit.rs`) that mitigates common whitespace quirks (CRLF, trailing spaces, dropped leading indentation, and collapsed internal whitespace). However, empirical history shows these passes were added reactively, and several structural weaknesses remain:

- `memory_write` and `memory_replace` in both `ragent-agent` and `ragent-tools-extended` use **exact-only** `String::matches` / `replacen`, with no whitespace fallback. They will fail on the same CRLF / indentation / trailing-space mismatches that `edit` handles.
- The edit matcher still has edge-case gaps around **leading/trailing blank lines**, **final-newline mismatches**, and **over-eager collapsed-whitespace matching** that can turn a solvable unique match into a false `MultipleMatches` error.
- `multiedit` applies edits sequentially in input order without overlap checking or dependency sorting. An earlier edit that touches a region later edits depend on can silently break later matches.
- Two stale copies of `edit.rs` and `multiedit.rs` exist in `crates/ragent-agent/src/tool/` but are **not** registered at runtime. They are a maintenance hazard and can mislead developers.
- There is no integration test coverage for `edit`/`multiedit` on real temp files, no `multiedit` tests at all, and no whitespace tests for `memory_write`/`memory_replace`.

This plan documents the root causes, prioritized fixes, and a validation roadmap to make whitespace-tolerant matching consistent across all replace-style tools and to prevent regressions.

---

## 2. Root Cause Analysis (with code references)

### 2.1 Active tool implementations live in `ragent-tools-core`

The runtime implementations are in `crates/ragent-tools-core/src/edit.rs` and `crates/ragent-tools-core/src/multiedit.rs` and are re-exported into the agent via `ExtractedCoreToolAdapter` (`crates/ragent-agent/src/tool/mod.rs:340-496`). `register_extracted_core_tools` (`crates/ragent-agent/src/tool/mod.rs:499-506`) registers the core registry, **not** the agent-local modules. The modules `crates/ragent-agent/src/tool/edit.rs` and `crates/ragent-agent/src/tool/multiedit.rs` are dead copies (they differ only stylistically from the core versions; `multiedit.rs` is identical, `edit.rs` differs only in indexed vs iterator loops). This duplication is a long-term bug vector.

### 2.2 The five-pass matcher and its current behavior

`find_replacement_range` (`crates/ragent-tools-core/src/edit.rs:173-312`) runs:

1. **Exact substring** (`content.matches(needle).count()`). Fast path; requires byte-identical strings.
2. **CRLF normalization** (`strip_cr`). Handles `\r\n` files vs `\n` needles.
3. **Trailing-whitespace strip** (`strip_trailing_ws`). Maps the match back to whole original lines via `byte_offset_of_line`.
4. **Leading-whitespace strip** (`trim_start` per line). Re-applies the original first-line indentation to `new_str` via `reindent_with`.
5. **Collapsed-whitespace** (`split_whitespace().collect::<Vec<_>>().join(" ")` per line). Re-applies original first-line indentation.

These passes were added reactively in commits `60ea246` (3-pass) and `172c0e3` (5-pass), evidenced by the CHANGELOG-style commit messages. The current unit tests pass (`cargo test -p ragent-tools-core edit` → 10/10 passed), but they cover only the designed cases.

### 2.3 Remaining whitespace edge cases

| Scenario | Why it still fails | Affected code |
|---|---|---|
| **Needle includes leading/trailing blank lines** | `content.lines()` and `needle.lines()` drop a trailing empty segment and may produce different line counts; the exact pass fails on blank-line differences. | `edit.rs:173-312` |
| **Final-newline mismatch** (file ends with `\n`, needle does not, or vice versa) | Exact pass fails; trailing-WS pass rejoins with `\n` and may match the wrong span or fail if the needle becomes empty. | `edit.rs:203-224` |
| **Collapsed pass over-normalizes** | Two different code blocks can collapse to the same signature, producing `MultipleMatches` and rejecting an otherwise unique edit. | `edit.rs:265-309` |
| **Needle with only whitespace differences inside a line** | Pass 5 is the only fallback; if it produces >1 collapsed match, the edit is rejected even though a single semantic match exists. | `edit.rs:265-309` |
| **Tab-vs-space + relative indentation** | `reindent_with` prepends the **first matched line's** indent to **every** line of `new_str`. If `new_str` already contains meaningful relative indentation, the result can be doubled or flattened. | `edit.rs:366-377` |

### 2.4 `multiedit` ordering and overlap risk

`MultiEditTool::execute` (`crates/ragent-tools-core/src/multiedit.rs:87-240`) reads each file once, validates each edit against the original content, then applies edits **sequentially in JSON order**, mutating the in-memory string between edits. Because matching is substring-based, later edits generally still succeed as long as their target text remains unchanged. However:

- There is **no pre-validation that edits are non-overlapping**.
- There is **no sorting** by offset, so an LLM can supply edits in an order that invalidates later needles (e.g., edit A replaces the surrounding block that contains edit B's target).
- Edits to the same file are applied to a shared mutable `String`, so earlier edits shift byte positions; the code uses `format!("{}{}{}", ..)` which is correct for in-memory content, but the error message only says `old_str not found`, giving no hint whether the failure is whitespace or ordering.

### 2.5 `memory_write` / `memory_replace` lack any whitespace tolerance

Both `crates/ragent-agent/src/tool/memory_write.rs:409-481` and `crates/ragent-tools-extended/src/memory_write.rs:409-481` use:

```rust
let count = block.content.matches(old_str).count();
block.content = block.content.replacen(old_str, new_str, 1);
```

There is no CRLF, trailing-space, leading-space, or collapsed-whitespace handling. Because memory blocks are often YAML/markdown with line-wrapped text and trailing spaces, these tools are highly likely to emit `old_str not found` for the same whitespace reasons `edit` already solves.

---

## 3. Prioritized Remediation Steps

### Priority 0 — Clarify ownership (structural, low risk)

- **Delete or deprecate the dead copies** in `crates/ragent-agent/src/tool/edit.rs` and `crates/ragent-agent/src/tool/multiedit.rs`. If deletion is too disruptive for the current branch, add `#[deprecated = "use ragent_tools_core implementations; registered via ExtractedCoreToolAdapter"]` comments and remove `pub mod edit;` / `pub mod multiedit;` from `crates/ragent-agent/src/tool/mod.rs:45,84`.  
  *Rationale:* Prevents future edits from being applied to the wrong file and eliminates a known source of confusion for the team.

### Priority 1 — Harden the matcher (medium risk, high impact)

1. **Normalize leading/trailing blank lines before the exact pass.**
   - Strip at most one leading `\n` and one trailing `\n` from both `content` and `needle` when the exact pass fails, then retry.
   - Preserve the original newline structure when computing the replacement span.

2. **Handle final-newline mismatch explicitly.**
   - Add a dedicated pass (or helper) that compares `content` and `needle` after normalizing trailing `\n` presence, similar to the CRLF pass.
   - Ensure `byte_offset_of_line` still returns the correct whole-line span when the match is on the last line.

3. **Make collapsed-whitespace matching stricter.**
   - Require the collapsed match to also agree on **non-whitespace token count per line** and, when ambiguous, prefer the match whose original whitespace is closest (e.g., Levenshtein distance) to the needle.
   - If collapsed matching yields >1 candidate but only one is unique under exact/CRLF/trailing/leading passes, fall back to that candidate rather than erroring.

4. **Fix `reindent_with` to preserve relative indentation.**
   - When re-applying detected indentation, compute the **common leading whitespace** of all matched file lines and the **common leading whitespace** of `new_str` lines; only prepend the difference.
   - Alternatively, document and test the current behavior, and add an optional `preserve_relative_indent` flag.

### Priority 2 — Share the matcher with memory tools (medium risk)

- Extract `find_replacement_range` (and its helpers) into a small shared module in `ragent-tools-core` (e.g., `ragent_tools_core::replace`) that is `pub` and re-usable.
- Replace the exact-only logic in:
  - `crates/ragent-agent/src/tool/memory_write.rs:438-454`
  - `crates/ragent-tools-extended/src/memory_write.rs:438-454`
- Use the same five-pass matcher and return the same `(start, end, effective_new_str)` tuple. This makes all replace-style tools behave identically.

### Priority 3 — Improve `multiedit` ordering and diagnostics (medium risk)

1. **Detect overlaps during validation.**
   - After all edits are validated against the original content, compute each edit's `(start, end)` byte range.
   - Reject or reorder any set of edits whose ranges overlap on the same file unless the later edit's `old_str` is present in the **post-edit** content (which is hard to prove). Recommended first step: fail fast with a clear error: `Edit N overlaps with Edit M in <path>; specify non-overlapping edits or split into separate calls`.

2. **Sort edits by byte offset before applying.**
   - Apply edits from the end of the file toward the beginning so earlier replacements do not shift later offsets. This is a robust ordering strategy for non-overlapping edits.
   - Keep per-file edit ordering stable; within the same offset, preserve JSON order.

3. **Better error messages.**
   - Distinguish `NotFound` caused by whitespace from `NotFound` caused by ordering by reporting the pass that failed and, when safe, show the closest matching snippet.

### Priority 4 — Test coverage (high value, low risk)

- Add `crates/ragent-tools-core/tests/test_edit_integration.rs` with temp-file tests for CRLF, trailing spaces, leading spaces, tab-vs-space, blank-line differences, final-newline mismatch, and multiple matches.
- Add `crates/ragent-tools-core/tests/test_multiedit.rs` covering:
  - two edits in one file,
  - edits across two files,
  - ordering independence (sorted vs input order),
  - overlap detection,
  - whitespace-tolerant matching in batch mode.
- Add `crates/ragent-tools-extended/tests/test_memory_replace_whitespace.rs` (and/or `ragent-agent`) to exercise CRLF, trailing spaces, and leading indentation in memory blocks.

---

## 4. Testing and Validation Plan

### 4.1 Unit tests

1. Extend `crates/ragent-tools-core/src/edit.rs` module tests with:
   - `leading_trailing_blank_lines_mismatch`
   - `final_newline_mismatch_file_needle`
   - `final_newline_mismatch_needle_file`
   - `collapsed_whitespace_unique_match`
   - `collapsed_whitespace_false_multiple`
   - `tab_vs_space_relative_indent`

2. Ensure all existing tests still pass before and after changes (`cargo test -p ragent-tools-core edit`).

### 4.2 Integration tests

1. **Edit integration** (`tests/test_edit_integration.rs`):
   - Create temp files with CRLF, trailing spaces, tabs, and missing final newline.
   - Invoke `EditTool.execute` via `Tool` trait with JSON input.
   - Assert file content, metadata, and that non-matching inputs produce the expected `old_str not found` error.

2. **Multiedit integration** (`tests/test_multiedit.rs`):
   - Apply 3 edits to the same file and assert all succeed.
   - Apply 2 edits where the second targets text inserted by the first and assert overlap error (or correct re-validation depending on chosen strategy).
   - Apply edits to multiple files and assert per-file stats.

3. **Memory replace integration** (`crates/ragent-tools-extended/tests/test_memory_replace_whitespace.rs`):
   - Write a memory block with CRLF and trailing spaces, then replace using `memory_replace` with a normalized `old_str`.
   - Assert the replacement succeeds and the block content is updated.

### 4.3 Regression / property tests

- Add a proptest or fuzz-like test that generates random `(content, needle, new_str)` triples and asserts:
  - the matcher never panics,
  - when a unique exact match exists, the result is identical to `replacen(..., 1)`,
  - when no match exists, the returned error is `NotFound`,
  - when >1 exact match exists, the returned error is `MultipleMatches`.

### 4.4 Manual / end-to-end validation

- Run the agent with a prompt that intentionally asks the LLM to edit a file containing CRLF, tabs, and trailing spaces; confirm no `old_str not found` errors.
- Run `/memory replace` against a project memory block with mixed whitespace.

---

## 5. Implementation Checklist

- [ ] **Ownership cleanup:** Remove `pub mod edit;` / `pub mod multiedit;` from `crates/ragent-agent/src/tool/mod.rs` and delete or deprecate the dead copies in `crates/ragent-agent/src/tool/`.
- [ ] **Matcher edge cases:** Add blank-line and final-newline normalization passes to `find_replacement_range` (`crates/ragent-tools-core/src/edit.rs`).
- [ ] **Matcher quality:** Improve collapsed-whitespace pass to avoid false `MultipleMatches` and preserve relative indentation in `reindent_with`.
- [ ] **Shared matcher:** Make `find_replacement_range` a `pub` helper in a shared module and consume it from `memory_write` in `ragent-agent` and `ragent-tools-extended`.
- [ ] **Multiedit ordering:** Compute edit byte ranges, detect overlaps, sort by offset (end-to-start), and emit actionable overlap errors.
- [ ] **Tests:** Add integration tests for `edit`, `multiedit`, and `memory_replace` whitespace scenarios; extend unit tests in `edit.rs`.
- [ ] **Docs:** Update `SPEC.md` and `CHANGELOG.md` to document that `edit`/`multiedit`/`memory_replace` now share a unified five-pass whitespace-tolerant matcher.
- [ ] **Validation:** Run `cargo test -p ragent-tools-core`, `cargo test -p ragent-tools-extended`, `cargo test -p ragent-agent` (relevant modules), and a manual agent smoke test.

---

## 6. Executable Milestone Plan

This section breaks the remediation work into five milestones with concrete, trackable tasks. Each milestone builds on the previous one, ending in a validation gate that must pass before the next milestone begins.

### Milestone 0 — Baseline & Ownership (do not change matcher behavior)

**Goal:** Eliminate dead-code confusion and establish a reproducible baseline so all subsequent edits target the correct files.

| ID | Task | Acceptance Criteria | Est. |
|---|---|---|---|
| M0-T1 | Verify active vs. dead copies | Confirm `register_extracted_core_tools` registers `ragent-tools-core` versions; confirm `crates/ragent-agent/src/tool/edit.rs` and `multiedit.rs` are not referenced in `mod.rs` or elsewhere at runtime. | 30m |
| M0-T2 | Remove dead copies | Delete `crates/ragent-agent/src/tool/edit.rs` and `crates/ragent-agent/src/tool/multiedit.rs`; remove their `pub mod` entries from `crates/ragent-agent/src/tool/mod.rs` if present. Build passes (`cargo check -p ragent-agent`). | 30m |
| M0-T3 | Capture baseline test results | Run `cargo test -p ragent-tools-core edit`, `cargo test -p ragent-tools-extended memory`, and `cargo test -p ragent-agent memory` (relevant modules). Record pass/fail counts in this plan or a separate `WSPLAN_BASELINE.md`. | 30m |
| M0-T4 | Tag decision point | If baseline tests fail, fix them before Milestone 1. If they pass, commit the ownership-cleanup change with message `wsplan: remove dead edit/multiedit copies`. | 30m |

**Milestone 0 exit gate:** `cargo check` and baseline tests pass with no `edit.rs`/`multiedit.rs` duplicates.

### Milestone 1 — Matcher Edge-Case Hardening

**Goal:** Fix the most common remaining `old_str not found` causes (blank lines, final newlines, and false `MultipleMatches`) while preserving existing behavior.

| ID | Task | Acceptance Criteria | Est. |
|---|---|---|---|
| M1-T1 | Add blank-line normalization pass | `edit` succeeds when `old_str` differs from file only by at most one leading and one trailing blank line. Add unit tests in `edit.rs`. | 1h |
| M1-T2 | Add final-newline normalization pass | `edit` succeeds when file and `old_str` disagree on trailing `\n` (four cases: file has it / needle has it, both directions). Add unit tests. | 1h |
| M1-T3 | Reduce collapsed-whitespace false positives | When collapsed matching yields >1 candidate but only one candidate is valid under earlier passes, prefer that candidate. Add unit tests for the previously false-`MultipleMatches` case. | 1.5h |
| M1-T4 | Fix relative indentation in `reindent_with` | When matching text has different per-line indentation than `new_str`, preserve the relative indentation of `new_str` instead of prepending the same indent to every line. Add tab-vs-space and nested-block unit tests. | 2h |
| M1-T5 | Regression test pass | All existing `edit` unit tests still pass; all new unit tests pass; `cargo test -p ragent-tools-core edit` reports zero failures. | 1h |

**Milestone 1 exit gate:** `cargo test -p ragent-tools-core edit` passes with new edge-case tests included.

### Milestone 2 — Shared Matcher for Memory Tools

**Goal:** Make `memory_replace` (and `memory_write` replace logic) behave identically to `edit` by reusing the same matcher.

| ID | Task | Acceptance Criteria | Est. |
|---|---|---|---|
| M2-T1 | Extract shared matcher module | Move `find_replacement_range` and helpers into `ragent_tools_core::replace` (or similar) as `pub` functions, keeping existing `edit.rs` re-exports. `cargo check -p ragent-tools-core` passes. | 1.5h |
| M2-T2 | Convert `ragent-agent` `memory_replace` | Replace exact `matches`/`replacen` logic in `crates/ragent-agent/src/tool/memory_write.rs` with the shared matcher. Add module-level unit tests for CRLF, trailing spaces, and leading indentation. | 1.5h |
| M2-T3 | Convert `ragent-tools-extended` `memory_replace` | Replace exact `matches`/`replacen` logic in `crates/ragent-tools-extended/src/memory_write.rs` with the shared matcher. Add module-level unit tests mirroring M2-T2. | 1.5h |
| M2-T4 | Decide agent vs. extended ownership | If both `memory_write.rs` files are truly duplicates, open a follow-up task to delete one; for this milestone, both must use the shared matcher. | 30m |
| M2-T5 | Regression test pass | `cargo test -p ragent-agent memory` and `cargo test -p ragent-tools-extended memory` pass, including new whitespace tests. | 1h |

**Milestone 2 exit gate:** Both `memory_replace` implementations use the shared matcher and pass whitespace-tolerant tests.

### Milestone 3 — Multiedit Ordering & Diagnostics

**Goal:** Prevent silent failures when multiple edits overlap or are supplied in an inconvenient order.

| ID | Task | Acceptance Criteria | Est. |
|---|---|---|---|
| M3-T1 | Compute per-edit byte ranges | During validation, each edit produces an absolute `(start, end)` range against the original file content. | 1h |
| M3-T2 | Detect overlapping edits | If two edits on the same file overlap, `multiedit` returns a clear error naming the overlapping edit indices and the file path. Add unit test. | 1.5h |
| M3-T3 | Sort edits end-to-start | For non-overlapping edits in the same file, apply from the highest `end` offset to the lowest so offsets remain stable. Add unit test showing JSON order independence. | 1.5h |
| M3-T4 | Improve error diagnostics | `NotFound` errors include the matching pass that failed and, when safe, the line number of the closest match attempt. | 1.5h |
| M3-T5 | Integration tests | Add `crates/ragent-tools-core/tests/test_multiedit.rs` covering two edits in one file, edits across two files, overlap detection, and whitespace-tolerant batch edits. | 1.5h |
| M3-T6 | Regression test pass | All `edit` and `multiedit` tests pass. | 1h |

**Milestone 3 exit gate:** `cargo test -p ragent-tools-core` passes with new `multiedit` integration tests.

### Milestone 4 — Integration & End-to-End Validation

**Goal:** Prove the fixes work on real files and real agent sessions, and update project documentation.

| ID | Task | Acceptance Criteria | Est. |
|---|---|---|---|
| M4-T1 | Edit integration tests | Add `crates/ragent-tools-core/tests/test_edit_integration.rs` with temp files containing CRLF, tabs, trailing spaces, missing final newline, and blank-line differences. All pass. | 2h |
| M4-T2 | Memory replace integration tests | Add `crates/ragent-tools-extended/tests/test_memory_replace_whitespace.rs` (or `ragent-agent` equivalent) exercising CRLF and trailing-space replacements in memory blocks. | 1.5h |
| M4-T3 | Full workspace test run | Run `cargo test --workspace` (or targeted crate tests) with 10-minute timeout; no new failures introduced. | 1h |
| M4-T4 | Manual agent smoke test | Run the TUI/agent with a prompt that asks it to edit a file containing CRLF, tabs, and trailing spaces; confirm no `old_str not found` errors. | 1h |
| M4-T5 | Manual memory smoke test | Run `/memory replace` against a project memory block with mixed whitespace; confirm replacement succeeds. | 30m |
| M4-T6 | Update SPEC.md | Document that `edit`, `multiedit`, and `memory_replace` share a unified whitespace-tolerant matcher and list the supported normalization passes. | 1h |
| M4-T7 | Update CHANGELOG.md | Add an entry under the next version describing matcher hardening, shared matcher, multiedit overlap detection, and memory tool whitespace tolerance. | 30m |
| M4-T8 | Mark plan complete | Update all task checkboxes in this file and append a completion note with the final commit hash. | 30m |

**Milestone 4 exit gate:** Full test suite passes, manual smoke tests succeed, and documentation is updated.

### Cross-Milestone Task Matrix

| Concern | Milestones touched | Tracking file |
|---|---|---|
| Shared matcher module | M1, M2, M3 | `crates/ragent-tools-core/src/replace.rs` (new) |
| `edit` tool | M0–M4 | `crates/ragent-tools-core/src/edit.rs` |
| `multiedit` tool | M0, M3, M4 | `crates/ragent-tools-core/src/multiedit.rs` |
| `memory_replace` | M2, M4 | `crates/ragent-agent/src/tool/memory_write.rs`, `crates/ragent-tools-extended/src/memory_write.rs` |
| Tests | M0–M4 | `crates/ragent-tools-core/tests/`, `crates/ragent-tools-extended/tests/` |
| Docs | M4 | `WSPLAN.md`, `SPEC.md`, `CHANGELOG.md` |

---

## Appendix — Key files referenced

| File | Role |
|---|---|
| `crates/ragent-tools-core/src/edit.rs` | Active `edit` tool and 5-pass matcher |
| `crates/ragent-tools-core/src/multiedit.rs` | Active `multiedit` tool |
| `crates/ragent-agent/src/tool/mod.rs:340-506` | `ExtractedCoreToolAdapter` and `register_extracted_core_tools` |
| `crates/ragent-agent/src/tool/edit.rs` | Dead copy of edit tool |
| `crates/ragent-agent/src/tool/multiedit.rs` | Dead copy of multiedit tool |
| `crates/ragent-agent/src/tool/memory_write.rs:409-481` | `memory_replace` exact-only matching |
| `crates/ragent-tools-extended/src/memory_write.rs:409-481` | Duplicate exact-only `memory_replace` |
