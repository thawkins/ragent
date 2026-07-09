# Edit/MultiEdit Tool Failure-to-Locate Fix Plan

## Status: Diagnostic plan, revised after Claude Code matcher research (EDITFIX.md)

**Date:** 2026-07-08  
**Scope:** `edit`, `multi_edit` / `multiedit` text-replacement tools  
**Location:** `crates/ragent-tools-core/src/edit.rs`, `multiedit.rs`, `replace.rs`

---

## 0. Research Outcome — Claude Code `Edit` Matcher

External research confirms that **Claude Code's `Edit` tool uses strict exact byte-for-byte matching**:

- Anthropic's own tool documentation and multiple third-party analyses state that `old_string` must match file contents exactly, including whitespace, indentation, and line endings.
- Anthropic closed a Windows CRLF bug report (`anthropics/claude-code#27718`) as **"not planned"** because the `Edit` tool intentionally does **not** normalize line endings.
- When Claude Code's `Edit` fails repeatedly, the agent falls back to the `Write` tool (whole-file rewrite) as its recovery strategy.

**Implication for ragent:** The current `edit`/`multi_edit` tools already implement Claude Code's matcher accurately. The frequent failures are not caused by ragent being stricter than Claude Code — they are caused by **Claude Code's matcher being intentionally strict and brittle in real-world use**. The correct response is not to copy Claude Code more faithfully, but to **exceed Claude Code's reliability** using ragent's existing whitespace-tolerant matcher while keeping the tool contract compatible.

---

## 1. Problem Statement

Users report that the `edit` and `multi_edit` tools frequently fail to locate the requested text in target files. The failure mode is typically:

> `old_string not found in <path>. Strict exact match requires the string to occur verbatim, including whitespace and indentation. Re-read the file and copy the exact text.`

This happens even when the text is "obviously" in the file, causing repeated `read` → `edit` retry loops, aborted batches, and agent frustration.

---

## 2. Root-Cause Analysis

### 2.1 The renewed `edit` tool intentionally matches Claude Code `Edit` semantics

The editrenewal spec (FR-004) mandates **strict exact-match replacement**: `old_string` must match file contents byte-for-byte, including whitespace and indentation. The implementation uses:

```rust
pub fn find_exact_replacement_range(content: &str, needle: &str, new_str: &str)
    -> Result<(usize, usize, String), FindError> {
    let count = content.matches(needle).count();
    ...
}
```

This is a raw substring search with no normalization.

**Research conclusion (see §0):** Claude Code's `Edit` tool uses the same strict exact byte-for-byte matching. ragent's implementation is therefore an accurate port of Claude Code's behavior, not an overly strict deviation. The failures are real but are a consequence of the reference behavior itself.

### 2.2 Common failure patterns

| # | Failure pattern | Why the strict exact matcher fails | Real-world frequency |
|---|-----------------|------------------------------------|----------------------|
| 1 | **Indentation mismatch** | Model emits 4 spaces; file uses 2 or tabs. | Very high |
| 2 | **Trailing/leading whitespace** | File lines end with trailing spaces; model omits them, or vice versa. | High |
| 3 | **Line-ending mismatch** | CRLF file vs LF `old_string` or vice versa. | Medium |
| 4 | **Missing/extra trailing newline** | Multi-line `old_string` ends with `\n` but model includes/excludes it inconsistently. | High |
| 5 | **Not enough context → multiple matches** | Short `old_string` (e.g. `let x = 1;`) occurs many times; strict matcher rejects with "found N times". | High |
| 6 | **Model copies from formatted `read` output** | If the `read` tool ever reformats output (line numbers, section maps, markdown wrapping), the copied text diverges from the file bytes. | Medium |
| 7 | **Stale-file rejection looks like locate failure** | FR-003 requires a recorded read timestamp. If the file was modified after read, the edit is rejected before matching, but the error message is generic. | Medium |

### 2.3 `multi_edit` shares the same matcher and adds atomicity risks

`multi_edit` uses the same `find_exact_replacement_range()` for every edit. Because the batch is atomic, a single whitespace mismatch anywhere rejects the **entire** batch, making it even more fragile than single-file edits.

### 2.4 Legacy alias `multiedit` only normalizes parameter names

`LegacyMultiEditAlias` in `crates/ragent-agent/src/tool/mod.rs` copies `path` → `file_path`, etc., but does **not** restore the old whitespace-tolerant matcher. Existing prompts that used to work with `multiedit` now fail on the slightest whitespace difference.

### 2.5 Agent instructions are inconsistent

`assets/config/AGENTS.md` still lists the legacy tool names and does **not** warn about the strict exact-match requirement. The built-in system prompt text (in `crates/ragent-agent/src/agent/mod.rs`) mentions strict matching, but the in-context guidance provided to the agent does not emphasize the need for large, exact context blocks or the fallback-to-`write` pattern.

---

## 3. Evidence from Code Inspection

### 3.1 Current matcher implementation

- `crates/ragent-tools-core/src/edit.rs:178-194` — `EditTool::execute` calls `find_exact_replacement_range()`.
- `crates/ragent-tools-core/src/multiedit.rs:235-239` — `MultiEditTool::execute` calls the same strict matcher.
- `crates/ragent-tools-core/src/replace.rs:57-71` — `find_exact_replacement_range()` does `content.matches(needle).count()` and nothing else.
- `crates/ragent-tools-core/src/replace.rs:133+` — `find_replacement_range()` and `find_replacement_range_diag()` provide the old seven-pass, whitespace-tolerant matcher, but they are no longer used by `edit` or `multi_edit`.

### 3.2 Current error diagnostics

Both tools produce only:

```rust
bail!("old_string not found in {}. Strict exact match requires the string to occur \
       verbatim, including whitespace and indentation. Re-read the file and copy the exact text.",
      path.display())
```

There is no guidance about:
- whether the mismatch is whitespace, line endings, or content;
- which line the closest near-match is on;
- how many occurrences exist.

### 3.3 Existing tests prove the strictness

Tests in `crates/ragent-tools-core/tests/test_edit_integration.rs` explicitly verify that CRLF and trailing-space mismatches are rejected. This confirms the behavior is intentional per the spec, and also confirms that it matches Claude Code's known-brittle behavior.

---

## 4. Design Goals for the Fix

1. **Reduce spurious NotFound failures** for the single-file `edit` tool while preserving safety.
2. **Preserve atomicity and safety** for `multi_edit`.
3. **Improve diagnostics** so the model (and user) knows why a match failed and how to fix it.
4. **Stay backward-compatible** with the old `multiedit` alias during the deprecation window.
5. **Exceed Claude Code's Edit reliability** where possible, since ragent already has a proven whitespace-tolerant matcher and need not replicate Claude Code's known failure modes.

---

## 5. Recommended Approach

> **Key correction after Claude Code research (§0):** The plan is no longer to "use the same matching algorithm as Claude Code" because Claude Code's algorithm is strict exact match and is already what ragent implements. The plan is to use a **more robust matcher than Claude Code's** while keeping the tool contract Claude-compatible.

### 5.1 Replace the single-file `edit` matcher with the proven ragent tolerant matcher

Change `EditTool` to use `find_replacement_range_diag()` (the existing seven-pass matcher) instead of `find_exact_replacement_range()`. This directly fixes patterns 1-5 in section 2.2.

- The matcher already returns the effective replacement string with indentation re-applied.
- It already handles CRLF, trailing whitespace, leading whitespace, collapsed whitespace, blank-line differences, and final-newline differences.
- It still rejects ambiguous multiple matches.
- It is well-tested (`replace.rs` + its own test suite).

This makes ragent's `edit` more reliable than Claude Code's `Edit` for the same inputs, directly addressing the brittleness documented in §0.

### 5.2 Keep `multi_edit` strict by default, but add controlled tolerance

`multi_edit` is atomic and operates on multiple files. For safety it should remain strict, **but** the batch should be allowed to pre-normalize each `old_string` in a controlled way:

- Strip trailing whitespace from each line of `old_string` before matching, because trailing spaces are invisible and commonly mismatched.
- Optionally normalize line endings to `\n`.
- Do **not** normalize indentation or collapsed whitespace for batch edits, to preserve byte-range stability and overlap detection.

This gives `multi_edit` a small usability boost without sacrificing atomicity.

### 5.3 Add rich match-failure diagnostics

When `old_string` is not found, the error message should include:

- The number of exact, leading-whitespace-normalized, and trailing-whitespace-normalized matches found.
- The line number of the closest near-match.
- A hint about the likely cause ("trailing spaces differ", "indentation differs", "line endings differ", "multiple occurrences — add context").

Use `find_replacement_range_diag()` to collect this information, even if the final matcher remains strict.

### 5.4 Add a `dry_run` / `preview` parameter (optional)

For both tools, support `"dry_run": true`. This resolves the match and reports the expected change without writing the file. This lets the model verify its `old_string` before committing, especially useful for `multi_edit`.

### 5.5 Update agent instructions and `AGENTS.md`

- In `assets/config/AGENTS.md`, update the tool list to show `edit` and `multi_edit` as canonical names.
- Add a "File editing best practices" section that tells the agent to:
  - Always read the file first.
  - Include 3-5 lines of context in `old_string`.
  - Copy the exact indentation.
  - Use `multi_edit` for multiple changes.
  - Use `dry_run`/`preview` if unsure.
  - If `edit` keeps failing, fall back to `write` for the whole file.

### 5.6 Reconcile with the editrenewal spec

Update `specs/editrenewal/SPEC.md`:
- Change FR-004 wording from "must match exactly, including whitespace" to "must match uniquely; common whitespace and line-ending differences are tolerated for the single-file `edit` tool".
- Keep `multi_edit` strict but document the optional trailing-whitespace normalization.
- Mark the change as a spec amendment and update `PLAN.md` status.

---

## 6. Implementation Plan

### Phase 1 — Single-file `edit` tolerance (Priority 0)

**Files:**
- `crates/ragent-tools-core/src/edit.rs`

**Tasks:**
1. Replace `find_exact_replacement_range` with `find_replacement_range_diag`.
2. Map the returned `FindDiag` into actionable error text.
3. Use the effective replacement string returned by the matcher when writing the file.
4. Update `EditTool::description` to state that common whitespace differences are tolerated but the match must still be unique.
5. Add tests for:
   - Indentation mismatch tolerance.
   - CRLF mismatch tolerance.
   - Trailing-space mismatch tolerance.
   - Final-newline mismatch tolerance.
   - Still rejects non-unique matches.

**Verification:** `cargo test -p ragent-tools-core --test test_edit_integration`

### Phase 2 — `multi_edit` strict-but-helpful (Priority 1)

**Files:**
- `crates/ragent-tools-core/src/multiedit.rs`
- `crates/ragent-tools-core/src/replace.rs` (helper)

**Tasks:**
1. Add a `normalize_old_string_for_batch(old_str: &str) -> String` helper that strips trailing whitespace per line and normalizes CRLF → LF, but does not change indentation.
2. Apply the normalization only when the strict exact match fails; record whether normalization was used.
3. Keep overlap detection using the original byte ranges of the normalized match.
4. Improve error diagnostics to report near-matches and line numbers.
5. Add tests for:
   - Batch with trailing-space mismatch now succeeds.
   - Batch with indentation mismatch still fails with clear diagnostics.
   - Atomic rollback still works.

**Verification:** `cargo test -p ragent-tools-core --test test_multiedit`

### Phase 3 — Diagnostics and dry-run (Priority 1)

**Files:**
- `crates/ragent-tools-core/src/edit.rs`
- `crates/ragent-tools-core/src/multiedit.rs`
- `crates/ragent-tools-core/src/replace.rs`

**Tasks:**
1. Extend `FindDiag` to carry:
   - `exact_matches: usize`
   - `normalized_matches: usize`
   - `closest_line: Option<usize>`
   - `likely_cause: &'static str`
2. Update `format_strict_error` and `EditTool` error messages to include these hints.
3. Implement `dry_run` parameter handling in both tools (skip writes, return preview metadata).
4. Add tests for diagnostic quality and dry-run behavior.

### Phase 4 — Documentation and instructions (Priority 2)

**Files:**
- `assets/config/AGENTS.md`
- `crates/ragent-agent/src/agent/mod.rs` (system prompt text)
- `specs/editrenewal/SPEC.md`
- `specs/editrenewal/PLAN.md`
- `CHANGELOG.md`

**Tasks:**
1. Update `assets/config/AGENTS.md` tool list and add editing best-practices section.
2. Update built-in agent prompt text to recommend context blocks, `dry_run`, and `write` fallback.
3. Amend the editrenewal spec to reflect the relaxed matching rules.
4. Add `CHANGELOG.md` entry.

### Phase 5 — Full verification (Priority 1)

**Commands:**
```bash
cargo check --workspace
cargo test -p ragent-tools-core
cargo test --workspace
```

---

## 7. Acceptance Criteria

- `edit` succeeds on single-file edits with common whitespace/line-ending differences.
- `multi_edit` remains atomic and succeeds when only trailing whitespace or line endings differ.
- Error messages name the likely cause and closest line when a match fails.
- All existing `edit`/`multi_edit` tests still pass (or are updated to reflect the new tolerant behavior).
- New tests cover the failure patterns listed in section 2.2.
- `AGENTS.md` and built-in prompts no longer tell the model to match "byte-for-byte" for the single-file tool.

---

## 8. Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| Whitespace tolerance could apply the wrong replacement when two blocks differ only by indentation. | Keep unique-match enforcement; do not disambiguate by proximity unless the seven-pass matcher already does so safely. |
| `multi_edit` normalization could break overlap detection. | Apply normalization only after computing byte ranges; overlap check uses the resolved byte ranges. |
| Changing the matcher re-introduces old bugs. | Reuse the existing, tested `find_replacement_range` matcher rather than writing a new one. |
| Model over-relies on tolerance and stops copying exact context. | Update prompts to still require 3-5 lines of context and uniqueness. |

---

## 9. Open Questions

1. Should `multi_edit` remain fully strict (no normalization) and rely on `dry_run` for the model to self-correct?
2. Should the single-file `edit` tool expose an `exact: bool` parameter for callers who truly need byte-for-byte matching?
3. Should the legacy `multiedit` alias be changed to use the tolerant matcher to restore pre-renewal behavior?

---

## 10. Immediate Next Step

Implement **Phase 1** (replace the single-file `edit` matcher with the proven ragent tolerant matcher) first. It is the smallest change that addresses the most frequent failure pattern and can be verified independently.

---

## 11. Research Notes — Claude Code `Edit` Matching

Sources consulted:

1. **israynotarray.com** — "The keyword is exact match — whitespace, indentation, line endings all have to line up." This analysis is consistent with Anthropic's documented `Edit` behavior.
2. **wuu73.org** — Groups Codex/Claude Code under patch-based editing; notes Codex's Rust `seek_sequence` with 4 levels of tolerance, but distinguishes Cline's tiered fallback from Claude Code's stricter approach. The Unicode-normalization detail applies to Codex, not necessarily Claude Code.
3. **medium.com/trukhinyuri (ByteBurst #8)** — "Claude Code: Search-Replace Without Fuzzy Matching. Claude Code uses strictly exact matching for `old_string` in `FileEditTool`. No fuzzy matching."
4. **github.com/anthropics/claude-code/issues/27718** — Windows CRLF multiline-edit bug closed as **"not planned"**; confirms Claude Code intentionally does not normalize line endings.
5. **oraios.github.io/serena** — Evaluation notes Claude Code `Edit`'s text matching is "more resilient than line numbers" but still exact-content based; chained edits often require re-reads, and the fallback pattern is to rewrite the whole file.

**Conclusion:** Claude Code's `Edit` is strict exact match. ragent already implements this accurately. The failures are therefore expected under Claude Code semantics. The strategic fix is to be **more tolerant than Claude Code**, not to copy Claude Code's matcher more faithfully.
