# Local source (in-project)

- Path: WSPLAN.md
- Relevance: 304 match(es) on: at, on, added, …(+22) — "# WSPLAN — `old_str not found` Remediation Plan"
- Captured (UTC): 2026-06-30T09:43:21.921236737+00:00

```text
Excerpt — 304 keyword match(es)

▶    1:    1  # WSPLAN — `old_str not found` Remediation Plan
     2:    2  
…
     4:    4  
▶    5:    5  The `edit` and `multiedit` tools (and the related `memory_write` / `memory_replace` tools) report `old_str not found` when the literal text supplied by the LLM no longer byte-matches the file co
     6:    6  
▶    7:    7  - `memory_write` and `memory_replace` in both `ragent-agent` and `ragent-tools-extended` use **exact-only** `String::matches` / `replacen`, with no whitespace fallback. They will fail on the sam
▶    8:    8  - The edit matcher still has edge-case gaps around **leading/trailing blank lines**, **final-newline mismatches**, and **over-eager collapsed-whitespace matching** that can turn a solvable uniqu
▶    9:    9  - `multiedit` applies edits sequentially in input order without overlap checking or dependency sorting. An earlier edit that touches a region later edits depend on can silently break later match
▶   10:   10  - Two stale copies of `edit.rs` and `multiedit.rs` exist in `crates/ragent-agent/src/tool/` but are **not** registered at runtime. They are a maintenance hazard and can mislead developers.
▶   11:   11  - There is no integration test coverage for `edit`/`multiedit` on real temp files, no `multiedit` tests at all, and no whitespace tests for `memory_write`/`memory_replace`.
    12:   12  
▶   13:   13  This plan documents the root causes, prioritized fixes, and a validation roadmap to make whitespace-tolerant matching consistent across all replace-style tools and to prevent regressions.
    14:   14  
…
    18:   18  
▶   19:   19  ### 2.1 Active tool implementations live in `ragent-tools-core`
    20:   20  
▶   21:   21  The runtime implementations are in `crates/ragent-tools-core/src/edit.rs` and `crates/ragent-tools-core/src/multiedit.rs` and are re-exported into the agent via `ExtractedCoreToolAdapter` (`crat
    22:   22  
▶   23:   23  ### 2.2 The five-pass matcher and its current behavior
    24:   24  
▶   25:   25  `find_replacement_range` (`crates/ragent-tools-core/src/edit.rs:173-312`) runs:
    26:   26  
▶   27:   27  1. **Exact substring** (`content.matches(needle).count()`). Fast path; requires byte-identical strings.
▶   28:   28  2. **CRLF normalization** (`strip_cr`). Handles `\r\n` files vs `\n` needles.
▶   29:   29  3. **Trailing-whitespace strip** (`strip_trailing_ws`). Maps the match back to whole original lines via `byte_offset_of_line`.
▶   30:   30  4. **Leading-whitespace strip** (`trim_start` per line). Re-applies the original first-line indentation to `new_str` via `reindent_with`.
▶   31:   31  5. **Collapsed-whitespace** (`split_whitespace().collect::<Vec<_>>().join(" ")` per line). Re-applies original first-line indentation.
    32:   32  
▶   33:   33  These passes were added reactively in commits `60ea246` (3-pass) and `172c0e3` (5-pass), evidenced by the CHANGELOG-style commit messages. The current unit tests pass (`cargo test -p ragent-tool
    34:   34  

… (274 more match(es) elided)

```
