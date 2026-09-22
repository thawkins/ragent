---
status: draft
spec_id: reqeng
---

# Manual Test Plan: Back-fill SDD Capabilities

This is a manual test plan for verifying the SDD back-fill features added
to ragent's `/spec` command family. All tests are performed by a human
operator in the ragent TUI or CLI; no automated test code is included.

## Prerequisites

1. **Build ragent** from source in the working directory:
   ```bash
   cargo build
   ```
   The binary is at `target/debug/ragent`.

2. **Configure a provider** — set at least one LLM API key:
   ```bash
   export ANTHROPIC_API_KEY="sk-..."
   ```
   (Alternatively use `OPENAI_API_KEY` or any supported provider.)

3. **Enable SDD capabilities** — create or edit `.ragent/ragent.json` (or
   `~/.config/ragent/config.json`) and add:
   ```json
   {
     "spec": {
       "sdd": true,
       "constitution": true,
       "consistency_validation": true,
       "branch_per_spec": true
     }
   }
   ```

4. **Start in a clean git workspace** — the project directory should be a
   git repository on the `main` branch with no uncommitted changes.

5. **Have the SDD spec-driven.md reference** available for comparison:
   <https://github.com/github/spec-kit/blob/main/spec-driven.md>

6. **Familiarity with existing commands** — the tester should be aware that
   ragent already has `/spec create`, `/spec update`, `/spec add`, and
   `/spec impl` commands. The new `/spec specify`, `/spec plan`, and
   `/spec tasks` commands are additional entry points that coexist with
   the existing ones.

## Test Cases

### TC-001: Create spec with `/spec specify` (separate specify stage)

**Objective:** Verify that `/spec specify` creates a SPEC.md without
generating a PLAN.md, and that `[NEEDS CLARIFICATION]` markers are detected.

**Preconditions:**
- ragent TUI is running
- No existing `specs/testspec-sdd/` directory

**Steps:**

1. Launch the ragent TUI: `ragent`
2. Type the following command in the chat input and press Enter:
   ```
   /spec specify testspec-sdd A real-time notification system for user mentions
   ```
3. Observe the status message — it should report spec creation success.
4. Open the file `specs/testspec-sdd/SPEC.md` in an editor.
5. Verify the SPEC.md contains:
   - YAML frontmatter with `status: draft`
   - A `## Requirements` section
   - At least one requirement using each EARS template (ubiquitous, event-driven, state-driven, optional, unwanted)
   - At least one `[NEEDS CLARIFICATION: ...]` marker (since the feature prompt is intentionally vague)
6. Verify that `specs/testspec-sdd/PLAN.md` does **not** exist.

**Test data to enter:**
- Command: `/spec specify testspec-sdd A real-time notification system for user mentions`

**Expected results:**
- SPEC.md is created with structured requirements and at least one clarification marker
- PLAN.md is NOT created (the specify stage is separate from planning)
- The TUI displays a success status

---

### TC-002: Validate detects `[NEEDS CLARIFICATION]` markers

**Objective:** Verify that `/spec validate` reports unresolved clarification
markers as warnings.

**Preconditions:**
- TC-001 has been completed (spec `testspec-sdd` exists with markers)

**Steps:**

1. In the ragent TUI, type:
   ```
   /spec validate testspec-sdd
   ```
   Press Enter.
2. Observe the validation output in the log panel.

**Test data to enter:**
- Command: `/spec validate testspec-sdd`

**Expected results:**
- The validation report lists each `[NEEDS CLARIFICATION: ...]` marker as a warning
- The report clearly identifies the line and marker text for each warning

---

### TC-003: Clarification gate blocks `approved` transition

**Objective:** Verify that a spec with unresolved clarification markers
cannot be transitioned to `approved`.

**Preconditions:**
- TC-001 and TC-002 completed (spec still has markers)

**Steps:**

1. In the ragent TUI, type:
   ```
   /spec status testspec-sdd approved
   ```
   Press Enter.
2. Observe the error/status message.

**Test data to enter:**
- Command: `/spec status testspec-sdd approved`

**Expected results:**
- The transition is blocked with a message explaining that unresolved `[NEEDS CLARIFICATION]` markers prevent approval
- The spec status remains `draft`

---

### TC-004: Generate plan with `/spec plan` (with technology context)

**Objective:** Verify that `/spec plan` generates a PLAN.md from the
existing SPEC.md using technology context. This is distinct from the
existing `/spec update` command, which regenerates the plan from an
edited SPEC.md without a technology-context argument.

**Preconditions:**
- TC-001 completed (spec `testspec-sdd` exists with SPEC.md)
- All clarification markers have been manually resolved (edit SPEC.md to remove or replace markers with concrete requirements)

**Steps:**

1. Open `specs/testspec-sdd/SPEC.md` in an editor and remove all
   `[NEEDS CLARIFICATION: ...]` markers by replacing them with concrete
   requirement text. Save the file.
2. In the ragent TUI, type:
   ```
   /spec plan testspec-sdd WebSocket transport, Redis pub/sub, PostgreSQL for persistence
   ```
   Press Enter.
3. Wait for plan generation to complete (watch the status bar).
4. Open `specs/testspec-sdd/PLAN.md` in an editor.
5. Verify the PLAN.md contains:
   - A `## Tasks` section with a markdown table (ID, Title, Requirement, Effort, Priority, Dependencies)
   - Technology choices referencing the provided context (WebSocket, Redis, PostgreSQL)
   - Task IDs in the format T-001, T-002, etc.

**Test data to enter:**
- Command: `/spec plan testspec-sdd WebSocket transport, Redis pub/sub, PostgreSQL for persistence`

**Expected results:**
- PLAN.md is generated with a task table
- Technology context from the command is reflected in the plan's rationale
- The spec's status can now be transitioned to `approved` (markers removed)

---

### TC-004b: Verify `/spec plan` coexists with `/spec update`

**Objective:** Verify that the existing `/spec update` command still works
alongside the new `/spec plan` command — they are independent entry points
that both produce a PLAN.md.

**Preconditions:**
- TC-004 completed (PLAN.md exists for `testspec-sdd`)

**Steps:**

1. Open `specs/testspec-sdd/SPEC.md` and add a new requirement FR-006
   (e.g., `The system shall support message search.`). Save the file.
2. In the ragent TUI, type:
   ```
   /spec update testspec-sdd
   ```
   Press Enter.
3. Wait for plan regeneration to complete.
4. Open `specs/testspec-sdd/PLAN.md` and verify the new requirement FR-006
   appears in the task table.

**Test data to enter:**
- Manual edit: add FR-006 to SPEC.md
- Command: `/spec update testspec-sdd`

**Expected results:**
- `/spec update` regenerates the PLAN.md from the edited SPEC.md
- The new requirement FR-006 is reflected in the updated task table
- Both `/spec plan` (with tech context) and `/spec update` (from edited SPEC.md) coexist without conflict

---

### TC-005: Generate tasks with `/spec tasks`

**Objective:** Verify that `/spec tasks` generates a TASKS.md file and a
quickstart.md file from the PLAN.md. The TASKS.md is a standalone,
flattened, ordered task list derived from the PLAN.md task table — it
does not replace the task table in PLAN.md.

**Preconditions:**
- TC-004 completed (PLAN.md exists for `testspec-sdd`)

**Steps:**

1. In the ragent TUI, type:
   ```
   /spec tasks testspec-sdd
   ```
   Press Enter.
2. Wait for task generation to complete.
3. Open `specs/testspec-sdd/TASKS.md` in an editor and verify it contains an ordered task list derived from the PLAN.md table.
4. Open `specs/testspec-sdd/quickstart.md` in an editor and verify it contains validation scenarios derived from the spec's acceptance criteria.
5. Open `specs/testspec-sdd/PLAN.md` and verify the original task table is still present (TASKS.md is a derived artifact, not a replacement).

**Test data to enter:**
- Command: `/spec tasks testspec-sdd`

**Expected results:**
- TASKS.md is created with an ordered, numbered task list
- quickstart.md is created with validation scenarios
- PLAN.md still contains its original task table (unchanged)

---

### TC-006: Constitution artifact and Phase -1 gates

**Objective:** Verify that a `CONSTITUTION.md` can be created and that
Phase -1 gates in the PLAN.md are validated before transitioning to
`in_progress`.

**Preconditions:**
- TC-004 completed (PLAN.md exists)
- SDD configuration has `constitution: true`

**Steps:**

1. In the ragent TUI, type:
   ```
   /spec constitution
   ```
   Press Enter. This generates a default `CONSTITUTION.md` in the specs root.
2. Open `specs/CONSTITUTION.md` and verify it contains the nine articles (library-first, observability, CLI-first, single model representation, library composition, modularity, simplicity, anti-abstraction, integration-first testing).
3. Open `specs/testspec-sdd/PLAN.md` and verify it includes a `### Phase -1: Pre-Implementation Gates` section with checkboxes for Simplicity, Anti-Abstraction, and Integration-First gates. If not present, manually add:
   ```markdown
   ### Phase -1: Pre-Implementation Gates

   #### Simplicity Gate
   - [ ] Using ≤3 projects?
   - [ ] No future-proofing?

   #### Anti-Abstraction Gate
   - [ ] Using framework directly?
   - [ ] Single model representation?

   #### Integration-First Gate
   - [ ] Contracts defined?
   - [ ] Contract tests written?
   ```
   Leave the checkboxes unchecked. Save the file.
4. Transition the spec to `approved`:
   ```
   /spec status testspec-sdd approved
   ```
   Press Enter.
5. Attempt to transition to `in_progress`:
   ```
   /spec status testspec-sdd in_progress
   ```
   Press Enter.
6. Observe the error message — it should list the unchecked gates.
7. Edit PLAN.md, check all gate checkboxes, and save.
8. Retry: `/spec status testspec-sdd in_progress` — the transition should succeed.

**Test data to enter:**
- Command: `/spec constitution`
- Command: `/spec status testspec-sdd approved`
- Command: `/spec status testspec-sdd in_progress` (first attempt — should fail)
- Command: `/spec status testspec-sdd in_progress` (second attempt after checking gates — should succeed)

**Expected results:**
- CONSTITUTION.md is generated with the nine articles
- Transition to `in_progress` is blocked when Phase -1 gates are unchecked
- After checking all gates, the transition succeeds

---

### TC-007: Branch-per-spec git workflow

**Objective:** Verify that `/spec specify` offers to create a git branch
named after the spec identifier.

**Preconditions:**
- Working directory is a git repository
- SDD configuration has `branch_per_spec: true`
- No existing `specs/branchspec/` directory

**Steps:**

1. In the ragent TUI, type:
   ```
   /spec specify branchspec A CSV import tool with column mapping
   ```
   Press Enter.
2. When prompted about creating a git branch, accept (type `y` or select Yes).
3. Run `git branch --show-current` in a terminal and verify the current branch is named `branchspec` (or a similarly derived name).
4. Verify `specs/branchspec/SPEC.md` exists.

**Test data to enter:**
- Command: `/spec specify branchspec A CSV import tool with column mapping`
- Prompt response: `y` (accept branch creation)

**Expected results:**
- A git branch is created and checked out
- The spec directory and SPEC.md are created on the new branch

---

### TC-008: Data-model and contracts artifacts

**Objective:** Verify that `/spec plan` generates `data-model.md` and a
`contracts/` directory when the spec involves data entities and API
contracts.

**Preconditions:**
- A spec exists that describes an API with data entities (use `branchspec` from TC-007, or create a new spec)

**Steps:**

1. In the ragent TUI, type:
   ```
   /spec plan branchspec REST API with CSV upload endpoint, ImportRecord data model
   ```
   Press Enter.
2. Wait for plan generation to complete.
3. Open `specs/branchspec/data-model.md` and verify it describes the `ImportRecord` data model with fields.
4. Open `specs/branchspec/contracts/` directory and verify it contains at least one contract file (e.g., `csv-upload.md` or `csv-upload.yaml`) describing the upload endpoint request/response.

**Test data to enter:**
- Command: `/spec plan branchspec REST API with CSV upload endpoint, ImportRecord data model`

**Expected results:**
- `data-model.md` is generated with the ImportRecord schema
- `contracts/` directory is created with at least one contract file

---

### TC-009: Consistency validation (ambiguity, contradictions, gaps)

**Objective:** Verify that `/spec validate` produces warnings for
ambiguity, contradictions, and gaps beyond EARS syntax checks.

**Preconditions:**
- A spec exists with intentionally ambiguous, contradictory, or incomplete requirements
- SDD configuration has `consistency_validation: true`

**Steps:**

1. Create a test spec:
   ```
   /spec specify consisttest A user authentication system
   ```
2. Manually edit `specs/consisttest/SPEC.md` to add:
   - An ambiguous requirement: `The system shall handle errors.` (no detail)
   - A contradiction: FR-001 says "The system shall use email/password auth" and FR-002 says "The system shall use OAuth-only auth"
   - A requirement without acceptance criteria: FR-003 "The system shall log events."
3. Save the file.
4. In the ragent TUI, type:
   ```
   /spec validate consisttest
   ```
   Press Enter.
5. Observe the validation output.

**Test data to enter:**
- Command: `/spec specify consisttest A user authentication system`
- Manual edits to SPEC.md (ambiguous, contradictory, and incomplete requirements)
- Command: `/spec validate consisttest`

**Expected results:**
- The validation report includes warnings for:
  - Ambiguous requirement ("handle errors" — too vague)
  - Contradiction between FR-001 and FR-002 (conflicting auth methods)
  - Gap in FR-003 (no acceptance criteria)
- All warnings are clearly labeled and reference the requirement IDs

---

### TC-010: Production feedback loop

**Objective:** Verify that feedback notes can be added to a spec and are
surfaced during plan regeneration.

**Preconditions:**
- A spec exists with a PLAN.md (e.g., `testspec-sdd` from TC-004)

**Steps:**

1. In the ragent TUI, type:
   ```
   /spec feedback testspec-sdd Production latency spike at 5000 concurrent users — consider connection pooling
   ```
   Press Enter.
2. Verify `specs/testspec-sdd/FEEDBACK.md` is created/updated with the note.
3. Regenerate the plan:
   ```
   /spec plan testspec-sdd WebSocket transport, Redis pub/sub, PostgreSQL with connection pooling
   ```
   Press Enter.
4. Open the regenerated `specs/testspec-sdd/PLAN.md` and verify it references the feedback note (e.g., in a "## Production Feedback" section or inline rationale).

**Test data to enter:**
- Command: `/spec feedback testspec-sdd Production latency spike at 5000 concurrent users — consider connection pooling`
- Command: `/spec plan testspec-sdd WebSocket transport, Redis pub/sub, PostgreSQL with connection pooling`

**Expected results:**
- FEEDBACK.md is created with the note
- The regenerated PLAN.md references the feedback in its rationale

---

### TC-011: Backward compatibility with existing specs

**Objective:** Verify that existing spec directories (without new SDD
artifacts) continue to validate, list, and search without errors.

**Preconditions:**
- At least one existing spec directory in `specs/` that predates the SDD changes (e.g., `specs/compact/`, `specs/hermes/`, etc.)

**Steps:**

1. In the ragent TUI, type:
   ```
   /spec validate compact
   ```
   Press Enter. (Substitute any existing spec ID.)
2. Verify validation completes without errors related to missing CONSTITUTION.md, TASKS.md, data-model.md, or contracts/.
3. Type:
   ```
   /spec list
   ```
   Press Enter.
4. Verify the existing spec appears in the list.
5. Type:
   ```
   /spec search hermes
   ```
   Press Enter. (Substitute an existing spec name.)
6. Verify search results include the existing spec.

**Test data to enter:**
- Command: `/spec validate compact`
- Command: `/spec list`
- Command: `/spec search hermes`

**Expected results:**
- Existing specs validate without errors about missing SDD artifacts
- Existing specs appear in list and search results
- No backward-incompatible behavior

---

### TC-012: Quality checklists in generated templates

**Objective:** Verify that newly generated SPEC.md and PLAN.md templates
include quality checklists.

**Preconditions:**
- SDD configuration has `sdd: true`

**Steps:**

1. In the ragent TUI, type:
   ```
   /spec specify checklisttest A feature with quality gates
   ```
   Press Enter.
2. Open `specs/checklisttest/SPEC.md` and verify it includes a quality checklist section with items like:
   - [ ] No [NEEDS CLARIFICATION] markers remain
   - [ ] Requirements are testable and unambiguous
   - [ ] Success criteria are measurable
3. Generate a plan:
   ```
   /spec plan checklisttest Simple REST API
   ```
   Press Enter.
4. Open `specs/checklisttest/PLAN.md` and verify it includes a quality checklist section with items like:
   - [ ] No speculative or "might need" features
   - [ ] All phases have clear prerequisites and deliverables

**Test data to enter:**
- Command: `/spec specify checklisttest A feature with quality gates`
- Command: `/spec plan checklisttest Simple REST API`

**Expected results:**
- SPEC.md includes a requirement-completeness checklist
- PLAN.md includes a plan-completeness checklist

---

### TC-013: Research artifact integration via `/spec specify`

**Objective:** Verify that research artifacts can be linked during spec
creation via `/spec specify`, producing a `research:` frontmatter field
and a `## Related Research` section. This builds on the existing
`--from-research` flag available in `/spec create`.

**Preconditions:**
- At least one completed research artifact exists in the `research/` directory (e.g., `research/websearch/RESEARCH.md`)
- SDD configuration has `sdd: true`

**Steps:**

1. In the ragent TUI, type:
   ```
   /spec specify researchtest A web search aggregation tool --from-research websearch
   ```
   Press Enter.
2. Open `specs/researchtest/SPEC.md` in an editor.
3. Verify the YAML frontmatter includes a `research:` field listing the linked research artifact(s).
4. Verify the SPEC.md includes a `## Related Research` section with a link to `research/websearch/RESEARCH.md`.
5. Verify that `specs/researchtest/PLAN.md` does **not** exist (specify stage creates SPEC.md only).

**Test data to enter:**
- Command: `/spec specify researchtest A web search aggregation tool --from-research websearch`

**Expected results:**
- SPEC.md frontmatter includes a `research: ["websearch"]` field
- SPEC.md includes a `## Related Research` section referencing the research artifact
- PLAN.md is NOT created (consistent with the specify-only stage)

---

### TC-014: Test-first file creation order in plan template

**Objective:** Verify that the plan template includes the test-first
file creation ordering (contracts → tests → source) and that the
implementation runner warns when tasks violate it.

**Preconditions:**
- TC-004 completed (a PLAN.md exists for `testspec-sdd`)
- SDD configuration has `sdd: true`

**Steps:**

1. Open `specs/testspec-sdd/PLAN.md` in an editor.
2. Verify the plan includes a "File Creation Order" section documenting:
   - Contracts first
   - Test files in order: contract → integration → e2e → unit
   - Source files last
3. Manually reorder a task so that a source-file task appears before a
   contract task (e.g., move T-001 above the contracts task). Save the file.
4. In the ragent TUI, run the implementation:
   ```
   /spec impl testspec-sdd --dry-run
   ```
   Press Enter.
5. Observe the dry-run output.

**Test data to enter:**
- Manual edit: reorder tasks in PLAN.md to violate file creation order
- Command: `/spec impl testspec-sdd --dry-run`

**Expected results:**
- The PLAN.md template includes the "File Creation Order" section
- The dry-run output includes an advisory warning about tasks violating the file creation order
- The warning is advisory only — execution is not blocked

---

## Cleanup

After all test cases are complete, remove the test spec directories and
reset the git state:

1. Delete test spec directories:
   ```bash
   rm -rf specs/testspec-sdd specs/branchspec specs/consisttest specs/checklisttest specs/researchtest
   ```
2. If a git branch was created (TC-007), switch back to `main` and delete the test branch:
   ```bash
   git checkout main
   git branch -d branchspec
   ```
3. Remove the generated constitution if it was only for testing:
   ```bash
   rm -f specs/CONSTITUTION.md
   ```
4. Optionally revert the SDD configuration changes in `.ragent/ragent.json` if they were only for testing.
5. Verify the workspace is clean: `git status`