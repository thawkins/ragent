---
status: draft
---

# Manual Test Plan: `/spec update` Subcommand

## Prerequisites

1. **ragent binary built and available on PATH**
   ```bash
   cd /home/thawkins/Projects/ragent
   cargo build
   ```

2. **At least one LLM provider configured** — either via environment variable
   (e.g. `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`) or via the TUI `/provider`
   command. The test requires a working LLM to regenerate the plan and test
   plan files.

3. **A sample spec directory with a SPEC.md file** — use an existing spec (e.g.
   `specs/hermes/`) or create a throwaway spec for testing:
   ```
   /spec create testspec A simple feature for testing spec update
   ```
   This creates `specs/testspec/SPEC.md`, `specs/testspec/PLAN.md`, and
   `specs/testspec/TESTPLAN.md`.

4. **An archived spec** (for the archived-guard test case) — create a spec and
   manually set its status to `archived` in the frontmatter:
   ```yaml
   ---
   status: archived
   ---
   ```

---

## Test Cases

### TC-001 — Parse and display help entry for `/spec update`

**Preconditions:**
- ragent TUI is open in the ragent project directory.

**Steps:**
1. Type `/spec help` in the TUI input field.
2. Press `Enter`.

**Test data:**
- Input: `/spec help`

**Expected results:**
- The assistant panel displays the `/spec` command reference table.
- The table contains a row for `/spec update <spec-id>`.
- The row description mentions regenerating `PLAN.md` and `TESTPLAN.md` from
  the existing `SPEC.md`.

---

### TC-002 — Usage error when spec ID is missing

**Preconditions:**
- ragent TUI is open.

**Steps:**
1. Type `/spec update` in the TUI input field (no spec ID provided).
2. Press `Enter`.

**Test data:**
- Input: `/spec update`

**Expected results:**
- The status bar shows `Usage: /spec update — try /spec help`.
- No LLM agent is spawned.
- No files are created or modified.

---

### TC-003 — Invalid spec ID format

**Preconditions:**
- ragent TUI is open.

**Steps:**
1. Type `/spec update invalid!id` in the TUI input field (contains `!` which
   is not allowed in spec IDs).
2. Press `Enter`.

**Test data:**
- Input: `/spec update invalid!id`

**Expected results:**
- The status bar shows `spec: invalid spec ID: invalid!id`.
- The assistant panel displays an error message about invalid spec ID
  characters.
- No LLM agent is spawned.

---

### TC-004 — Spec not found

**Preconditions:**
- ragent TUI is open.
- No spec directory named `nonexistent` exists under `specs/`.

**Steps:**
1. Type `/spec update nonexistent` in the TUI input field.
2. Press `Enter`.

**Test data:**
- Input: `/spec update nonexistent`

**Expected results:**
- The assistant panel displays an error message stating that the spec was not
  found.
- The error message lists available specs (or shows `(none found)` if no specs
  exist).
- No LLM agent is spawned.

---

### TC-005 — Archived spec cannot be updated

**Preconditions:**
- A spec directory `specs/archivedspec/` exists with a `SPEC.md` whose
  frontmatter contains `status: archived`.

**Steps:**
1. Type `/spec update archivedspec` in the TUI input field.
2. Press `Enter`.

**Test data:**
- Input: `/spec update archivedspec`

**Expected results:**
- The assistant panel displays an error message: `spec: 'archivedspec' is
  archived and cannot be modified`.
- No LLM agent is spawned.
- No files are modified.

---

### TC-006 — Successful regeneration of PLAN.md and TESTPLAN.md

**Preconditions:**
- A spec directory `specs/testspec/` exists with `SPEC.md`, `PLAN.md`, and
  `TESTPLAN.md`.
- An LLM provider is configured and available.
- Note the current contents of `specs/testspec/PLAN.md` and
  `specs/testspec/TESTPLAN.md` for comparison.

**Steps:**
1. Manually edit `specs/testspec/SPEC.md` — add a new requirement (e.g.
   `### FR-010 — New test requirement`) under the `## Requirements` section.
   Save the file.
2. Return to the ragent TUI.
3. Type `/spec update testspec` in the TUI input field.
4. Press `Enter`.

**Test data:**
- Input: `/spec update testspec`
- Edit to SPEC.md: add a new `### FR-010 — New test requirement` block with an
  EARS sentence.

**Expected results:**
- The assistant panel displays a message starting with
  `From: /spec update` and `🔄 **Regenerating plan and test plan…**`.
- The status bar shows `spec: updating specs/testspec/PLAN.md +
  specs/testspec/TESTPLAN.md…`.
- After the LLM agent completes:
  - `specs/testspec/PLAN.md` has been overwritten and now contains a `## Tasks`
    section with a markdown table. The table includes at least one new task
    row linked to `FR-010` (or the new requirement added).
  - `specs/testspec/TESTPLAN.md` has been overwritten and now contains YAML
    frontmatter with `status: draft` and a `## Test Cases` section with at
    least one test case entry (`TC-001` or similar).
  - `specs/testspec/SPEC.md` has NOT been modified (its content is identical to
    what was saved in step 1).
  - The `PLAN.md` table columns are: `ID`, `Title`, `Requirement`, `Effort`,
    `Priority`, `Dependencies`.
  - Task IDs use the `T-NNN` format.
  - Effort values are one of `S`, `M`, `L`.
  - Priority values are one of `Critical`, `High`, `Medium`, `Low`.
  - `TESTPLAN.md` contains no `#[test]` functions or `cargo test` references.

---

### TC-007 — Existing task status preservation (best-effort)

**Preconditions:**
- A spec directory `specs/testspec/` exists with a `PLAN.md` that has at least
  one task with `Status` set to `completed` or `in_progress`.
- An LLM provider is configured.

**Steps:**
1. Note which task IDs in `specs/testspec/PLAN.md` have `completed` or
   `in_progress` status.
2. Type `/spec update testspec` in the TUI input field.
3. Press `Enter`.

**Test data:**
- Input: `/spec update testspec`

**Expected results:**
- After the LLM agent completes, the regenerated `PLAN.md` retains the task
  IDs that existed before (T-001, T-002, etc.) and, for tasks whose IDs are
  still present, the status column reflects the previous `completed` or
  `in_progress` value.
- This is a best-effort expectation — the LLM is instructed to preserve
  statuses but may not always do so perfectly. If statuses are not preserved,
  this is a known limitation, not a hard failure.

---

### TC-008 — Verify SPEC.md is not modified

**Preconditions:**
- A spec directory `specs/testspec/` exists with a known `SPEC.md` content.
- Record the SHA-256 hash or exact byte content of `specs/testspec/SPEC.md`.

**Steps:**
1. Type `/spec update testspec` in the TUI input field.
2. Press `Enter`.
3. Wait for the LLM agent to complete.
4. Read `specs/testspec/SPEC.md` and compare to the pre-command content.

**Test data:**
- Input: `/spec update testspec`

**Expected results:**
- `specs/testspec/SPEC.md` is byte-for-byte identical to its pre-command
  content.
- Only `PLAN.md` and `TESTPLAN.md` have been modified.

---

## Cleanup

1. Remove any throwaway test specs created during testing:
   ```bash
   rm -rf specs/testspec specs/archivedspec
   ```
2. If `specs/specupdate/` (this spec) was created solely for testing and is not
   needed, it can be retained or removed:
   ```bash
   rm -rf specs/specupdate
   ```
3. If `SPEC.md` was manually edited during TC-006, restore it from version
   control if needed:
   ```bash
   git checkout -- specs/testspec/SPEC.md
   ```