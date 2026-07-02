---
status: implemented
audit:
  - { time: 1782956669, from: "none", to: "draft", actor: "system" }
---
# Specification: Edit Tool Renewal — Align ragent `edit` and `multiedit` with Claude Code's `Edit`

## Executive Summary

This document specifies a renewal of ragent's two edit tools (`edit` and `multiedit`) so that their behaviour, user-facing contract, and safety model match the semantics of Claude Code's `Edit` tool. The goal is to reduce surprise for users and agents moving between the two clients while preserving ragent's existing atomic-batch strengths.

Claude Code's `Edit` is a single-file, single-instance search-and-replace tool that:
- operates only after the file has been read by the agent,
- rejects edits when the file has been modified since it was read,
- requires absolute file paths,
- performs strict exact-match replacement (no whitespace tolerance),
- supports create, delete, and update operations through empty `old_string` / `new_string`,
- reports a structured snippet of the edited file in the result.

ragent currently exposes two tools: `edit` (single replacement, whitespace-tolerant seven-pass matcher) and `multiedit` (atomic batch replacement across files). The renewal unifies these behind a Claude Code-compatible `edit` tool and introduces a `multi_edit` tool for the existing atomic-batch use case.

## Scope & Objectives

### Scope

- Rename and re-specify ragent's single-file edit tool to match Claude Code `Edit` semantics.
- Introduce a separate `multi_edit` tool for the existing atomic batch-edit workflow.
- Update parameter schemas, descriptions, prompts, validation, and error messages.
- Preserve read-timestamps, permission checks, and path-root guards.
- Update tests, documentation, and built-in agent instructions.

### Out of Scope

- Removing the `Write` or `Create` tools (they remain available for large overwrites and file creation).
- Changing the underlying patch/unified-diff based `patch` tool.
- Adding notebook-specific edit logic (use the existing `memory_replace` or future notebook tool).
- GUI rendering of structured diffs in the TUI (result formatting remains text-oriented).

### Objectives

1. Make ragent `edit` behave as closely as practical to Claude Code `Edit`.
2. Keep the existing atomic multi-file batch edit capability available as `multi_edit`.
3. Ensure every requirement is testable and traceable to implementation tasks.
4. Minimize breaking changes for existing ragent users by retaining the old tool name as an alias during a deprecation window.

---

## Requirements

### FR-001 — Parameter Name Alignment

`The <renewed edit tool> shall <use the parameter names file_path, old_string, and new_string>.`

`The <renewed edit tool> shall <reject invocations that use the legacy parameter names path, old_str, or new_str>.`

`When <a legacy ragent tool call is received>, the <system> shall <optionally accept it under the old tool name with a deprecation warning>.`

### FR-002 — Absolute Path Requirement

`The <renewed edit tool> shall <require file_path to be an absolute path>.`

`When <file_path is relative>, the <renewed edit tool> shall <reject the request with a clear error message>.`

`The <renewed edit tool> shall <resolve the absolute file_path against the working directory if a relative path is supplied only when an explicit compatibility flag is enabled>.`

### FR-003 — Read-Before-Write Validation

`The <renewed edit tool> shall <reject an edit if the target file has not been read by the current session>.`

`When <the target file has been read>, the <renewed edit tool> shall <record the file's last-modified time>.`

`If <the file's last-modified time on disk is later than the recorded read time>, the <renewed edit tool> shall <reject the edit with a stale-file error>.`

### FR-004 — Strict Exact-Match Replacement

`The <renewed edit tool> shall <require old_string to match file contents exactly, including whitespace and indentation>.`

`When <old_string occurs more than once in the file>, the <renewed edit tool> shall <reject the edit and report the number of matches>.`

`When <old_string does not occur in the file>, the <renewed edit tool> shall <reject the edit and report that the string was not found>.`

### FR-005 — Single-Instance Restriction

`The <renewed edit tool> shall <replace exactly one occurrence of old_string per invocation>.`

`When <multiple occurrences of old_string exist>, the <renewed edit tool> shall <require the caller to add context and issue separate tool calls>.`

### FR-006 — Create, Update, and Delete Operations

`The <renewed edit tool> shall <create a new file when old_string is empty and the file does not exist>.`

`When <old_string is empty and the file already exists>, the <renewed edit tool> shall <reject the request with a file-already-exists error>.`

`The <renewed edit tool> shall <delete the matched text when new_string is empty>.`

### FR-007 — No-Change Rejection

`When <old_string and new_string are identical>, the <renewed edit tool> shall <reject the request with a no-changes error>.`

### FR-008 — Structured Result Snippet

`After <a successful edit>, the <renewed edit tool> shall <return a snippet of the edited file with line numbers>.`

`The <snippet> shall <include at least four lines of context before and after the change>.`

`If <the change occurs near the start or end of the file>, the <snippet> shall <be clamped to the file boundaries>.`

### FR-009 — Batch Edit Tool (`multi_edit`)

`The <multi_edit tool> shall <accept an array of edit objects, each containing file_path, old_string, and new_string>.`

`The <multi_edit tool> shall <apply all edits atomically: if any edit fails validation, no files are modified>.`

`The <multi_edit tool> shall <enforce FR-002 through FR-007 for every edit in the batch>.`

`When <two edits in the same file overlap>, the <multi_edit tool> shall <reject the entire batch and report the overlapping indices>.`

### FR-010 — Permission and Path Guard Retention

`Both <the renewed edit tool and multi_edit tool> shall <continue to require file:write permission>.`

`Both <tools> shall <reject paths that escape the configured project root>.`

`The <multi_edit tool> shall <acquire file locks in a consistent order to avoid deadlocks>.`

### FR-011 — Prompt and Description Update

`The <renewed edit tool description> shall <state that old_string must be unique within the file and must match exactly>.`

`The <renewed edit tool prompt> shall <instruct the model to include 3–5 lines of context around the change point>.`

`The <multi_edit tool description> shall <clarify that each edit in the batch is a single-instance exact replacement>.`

### FR-012 — Legacy Alias and Deprecation

`While <a deprecation window is active>, the <legacy tool name "edit" (when invoked with path/old_str/new_str)> shall <continue to work and emit a deprecation warning>.`

`When <the deprecation window expires>, the <legacy "edit" tool> shall <be removed or redirected to the new tool>.`

### FR-013 — Test Coverage

`The <renewed edit tool> shall <have unit and integration tests covering exact match, multiple matches, missing file, stale file, absolute-path requirement, create/delete/update, and snippet generation>.`

`The <multi_edit tool> shall <have integration tests covering cross-file batches, overlap rejection, and atomic rollback>.`

### FR-014 — Documentation and Agent Instructions

`The <ragent user documentation> shall <describe the new edit and multi_edit tools and their differences from the legacy edit/multiedit tools>.`

`The <built-in agent instructions> shall <be updated to prefer the new tool names and parameter names>.`

---

## Glossary

| Term | Definition |
|------|------------|
| `edit` | The renewed single-file edit tool, matching Claude Code `Edit` semantics. |
| `multi_edit` | The atomic batch edit tool that replaces the old `multiedit`. |
| `old_string` | The exact text to be replaced in the target file. |
| `new_string` | The replacement text. |
| `file_path` | The absolute path to the file to edit. |
| `read timestamp` | The last-modified time of a file recorded when the session reads it. |

---

## Open Questions

1. Should the old `multiedit` name be retained as an alias alongside `multi_edit`, or renamed entirely?
2. How long should the deprecation window for the legacy `edit` tool be (one release, two releases)?
3. Should the TUI render a structured diff for successful edits, or only the text snippet?
