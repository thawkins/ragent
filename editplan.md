# Edit-Failure Analysis & Fix Plan

**Source log analysed:** `log/edits-20260807-083530-926648.jsonl`
**Log window:** 2026-08-14 08:23:36 UTC → 2026-08-14 20:22:36 UTC (≈12 h continuous session)
**Total entries:** 667 (579 `edit`, 88 `multi_edit`; `apply_patch` not instrumented before this change)

---

## 1. Headline numbers

| Metric | Value |
|---|---|
| Total edit invocations | 667 |
| Successes | 450 (67.5 %) |
| Failures | 217 (32.5 %) |
| — `edit` failures | 189 / 579 = **32.6 %** |
| — `multi_edit` failures | 28 / 88 = **31.8 %** |

## 2. Failure-class breakdown

| Class | Count | % of failures |
|---|---|---|
| `old_string not found` | **187** | **86.2 %** |
| Stale-file rejected | 18 | 8.3 % |
| Multiple matches | 8 | 3.7 % |
| No-change rejected | 4 | 1.8 % |

The dominant cause of failures — by nearly an order of magnitude — is **`old_string not found`**. The remaining classes are a long tail.

## 3. Deep-dive: what is actually causing `old_string not found`?

Each of the 187 not-found entries was replayed against the *current* working tree
(`/home/thawkins/Projects/ragent`) to see how close it came to matching.

### 3.1 Diagnosis vs current file state

| Diagnosis | Count | Meaning |
|---|---|---|
| `content_drift` | **180** | No whitespace-normalised form of the old text exists in the file at all |
| `ws_collapsed` | 7 | Whitespace-only difference (collapse_whitespace would have matched) |
| `crlf_mismatch` / `trailing_ws` / `tabs_vs_spaces` | 0 | Line-ending / tab / trailing-space problems are **not** the cause |

### 3.2 Line-presence profile

For each not-found entry, every non-blank line of `old_str` was searched for in
the current file:

| % of `old_str` lines found in file | Entries | Interpretation |
|---|---|---|
| **0 %** | 37 | Content is completely absent (hallucinated or deleted code) |
| 1 – 25 % | 70 | Mostly absent — model is working from a very stale snapshot |
| 26 – 50 % | 27 | Major drift — file has changed substantially |
| 51 – 75 % | 20 | Significant drift |
| 76 – 99 % | 16 | Only 1–2 lines differ (genuine near-miss) |
| **100 %** | 17 | Every line exists — pure formatting / indent difference |

> **Key finding:** 107 of 187 failures (57 %) show ≤ 25 % line overlap with the
> file on disk. The model is not producing *slightly wrong* strings — it is
> producing strings that simply do not exist anywhere in the file.

### 3.3 Indentation-ghost sub-pattern

Of the 37 *0%-present* entries, **27 (73 %)** contain lines that match the target
file exactly once indentation is stripped. Example (from
`crates/ragent-tools-extended/src/http_request.rs`):

```
old_str starts:  "      async fn execute(&self, input: Value, _ctx: &ToolContext)"
file actually:   "    async fn execute(&self, input: Value, _ctx: &ToolContext)"
```

The model invented 6-space leading indentation where the file uses 4-space. This
is a distinct, fixable sub-cause.

### 3.4 Session-behaviour correlation

| Observation | Count | % of 187 |
|---|---|---|
| Preceded by ≥ 1 successful edit on the same file earlier in the session | **174** | **93 %** |
| Immediately preceded by a success on the same file (stale-buffer chains) | 67 | 36 % |
| `collapse_whitespace` used and still failed | 0 | 0 % |
| Median `old_str` length | 383 chars | — |
| P90 `old_str` length | 1,188 chars | — |
| Max `old_str` length | 3,414 chars | — |
| Retry with the *same* text that later succeeded | 6 | 3 % |

The dominant pattern is a **stale-buffer cascade**: after the first successful
edit, the file on disk changes but the edit tool does not feed the post-edit
state back into the model's context. The model then builds the *next* `old_str`
from its memory of the pre-edit text. Once one edit drifts, every subsequent
edit on the same file drifts further.

## 4. Root-cause summary

| # | Root cause | Share | Primary fix lever |
|---|---|---|---|
| R1 | **Stale-buffer cascade** — model uses a pre-edit file image; post-edit state is reflected only indirectly | ~74 % of not-found cases (180/187 show drift; 93 % are same-file successors) | Auto re-read / read-after-write guard on the tool side |
| R2 | **Indentation hallucination** — model invents wrong leading whitespace (typically 6-space vs 4-space) | ~14 % (27/187, all in the 0 % bucket) | Indent-normalised fallback lane in the matcher |
| R3 | **Minor drift** — only 1–2 lines changed elsewhere in the file; `old_str` is ~90 % right | ~9 % (16/187) | Fuzzy / similarity matcher when line-presence ≥ 75 % |
| R4 | **Pure whitespace formatting** | ~9 % (17/187) | `collapse_whitespace` promotion (currently opt-in, never used) |
| R5 | **Stale-file gate** (mtime vs read timestamp) | 18 rejects (separate class) | Auto re-read + retry once, instead of hard reject |

## 5. Fix plan

### P0 — instrumentation (done in this change)

- [x] **Wire `apply_patch` into edit-log** so future logs are complete. Every
  operation (Add/Delete/Update) now logs `tool: "apply_patch"` with per-op
  `old_str`/`new_str` extracted from hunk context/replacement lines. Parse and
  path-validation failures are also logged.

### P1 — tool-side safety net (highest impact: targets R1, R5)

1. **Auto re-read on failure**: when `edit` fails with `old_string not found`,
   automatically re-read the file, re-attempt the match against the fresh
   content, and — if it would match — return a hint `"old_string not found; re-read
   the file to refresh your buffer"` instead of opaque failure. *(Estimated to
   fix ~70 % of R1 by forcing the model's next attempt to start from reality.)*
2. **Post-edit result snippet already exists** (FR-008); extend it to also
   include a `read_timestamp` update and a compact `"file now differs from your
   last read"` marker in the returned metadata so the model sees the drift
   *before* composing the next edit.
3. **Stale-file retry-once**: when `check_stale_file` rejects, perform a single
   automatic re-read, refresh the session's read timestamp, and return a soft
   error so the model can rebuild `old_str` from live content.

### P2 — matcher improvements (targets R2, R3, R4)

4. **Promote `collapse_whitespace` to a fallback lane** rather than an opt-in:
   when exact matching fails with *not found*, retry transparently with
   whitespace-flexible matching. If the flexible lane finds a unique match,
   succeed and record `"flexible match"` in the log (a new outcome tag so the
   frequency of fallback can be measured). Reject only if the flexible lane is
   also zero or multiple. *(Would have fixed 7/187 outright, plus most of R4.)*
5. **Indentation-normalised lane**: after the whitespace lane, try a
   leading-whitespace-stripped comparison per line. Succeed only if exactly one
   match. *(Targets the 27 indentation-ghost cases = ~14 % of not-found.)*
6. **Line-similarity hint (no auto-apply)**: when `old_str` matches ≥ 75 % of
   lines but not exactly, include in the error message the nearest-matching
   8-line window from the current file so the model's next attempt lands closer.
   *(Targets R3.)*
7. **Multiple-match auto-disambiguation**: when `old_str` matches N > 1 times,
   return the byte offsets and surrounding context lines of all matches so the
   model can extend the search string on the next attempt instead of guessing.

### P3 — session / prompt hygiene (targets R1 at source)

8. **Read-then-edit enforcement**: if a session has never read a file before
   calling `edit`, do not reject, but log a `"no prior read"` diagnostic tag in
   the edit-log so the behaviour is measurable. Consider a strict mode later
   (config toggle).
9. **Compact reminder in tool description**: append one line to the `edit` /
   `multi_edit` descriptions — `"If your previous edit on this file succeeded,
   treat your in-context copy as stale and re-read before composing the next
   old_string."` — cheap, immediately deployable.
10. **Cap `old_str` length guidance**: median failing `old_str` is 383 chars and
    P90 is 1,188 chars; recommend < 20 lines in the tool description and log a
    warning tag when longer strings fail.

### P4 — observability

11. **`apply_patch` now logged** (this change). Verify with
    `/editlog show` that entries appear with `tool: "apply_patch"`.
12. Add a `match_lane` field to log entries (`exact`, `flexible`,
    `indent_normalised`, `not_found`) once P2 lands so future analyses can
    quantify which lane would have saved each failure.

## 6. Expected impact

| Fix | Failures it targets | % of 217 total |
|---|---|---|
| P1.1 auto re-read hint | ~120 (R1) | 55 % |
| P1.3 stale retry-once | 18 (R5) | 8 % |
| P2.4 flexible-lane fallback | 7 proven + est. 10 more from R4 | ≈ 8 % |
| P2.5 indent-normalised lane | 27 (R2) | 12 % |
| P2.6 line-similarity hint | 16 (R3) | 7 % |
| P2.7 multi-match disambiguation | 8 | 4 % |
| **Cumulative** | | **≈ 95 %** |

## 7. Suggested verification

1. Re-run a fresh session with edit-log on (default).
2. `/editlog show` — confirm `apply_patch` entries appear with per-op detail.
3. `/editlog analyse` — re-compare `old_str not found` rate; target < 15 %.
4. Re-run this analysis script against the next log file; every not-found entry
   should contain the new `match_lane` tag so the residual failure modes are
   directly attributable.

---

## Appendix — methodology

The numbers above were computed by replaying every `outcome != "success"` entry
from `edits-20260807-083530-926648.jsonl` against the working tree as of
2026-08-15. Line-presence percentages were computed as

```
matched_lines / total_non_blank_lines_in_old_str × 100
```

where a line counts as matched if it appears verbatim in the current file
content (after normalising CRLF → LF). The 0%-present entries were then further
validated by stripping all leading whitespace and comparing per-line.

Files most affected by failures (top 5):

```
56  crates/ragent-research/src/session.rs
35  crates/ragent-research/src/web_gatherer.rs
30  crates/ragent-research/src/document.rs
16  crates/ragent-research/src/cli.rs
10  crates/ragent-research/src/manager.rs
```

These five files account for 147 / 217 failures (68 %). The
`crates/ragent-research` crate was undergoing heavy concurrent modification
during the logged session, which amplified the stale-buffer cascade effect.
