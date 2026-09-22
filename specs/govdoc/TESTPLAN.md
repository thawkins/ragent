---
status: draft
---

# Manual Test Plan: `/spec govcreate` - Architecture Document to Spec + Project Scaffold

**Spec:** [SPEC.md](SPEC.md) · **Plan:** [PLAN.md](PLAN.md)

This is a **manual** test plan. Every case is executed by a human in a real terminal
session. Where a case needs a live URL, a local fixture folder, or a hosting account, the
prerequisite is listed before the steps. This document contains no automated test code.

## Prerequisites

1. **Build the binary.** Run `cargo build` (debug is sufficient; allow up to 1000
   seconds). Confirm `./target/debug/ragent --version` runs and reports the current
   version.
2. **Configure an LLM provider.** The architecture-extraction and spec-authoring stages
   need a model. Set the API key for your configured provider in the environment, or
   point ragent at a local Ollama model. Confirm a plain `ragent run "say hello"` returns
   a response.
3. **Prepare a scratch root.** Create `~/scratch/govdoc-tests/` and inside it one
   **empty** subdirectory per scaffold case, e.g. `tc-url-rust/`, `tc-local-python/`,
   `tc-pdf/`, `tc-busy/`, `tc-badref/`, `tc-force/`, `tc-cli/`, `tc-budget/`,
   `tc-clash/`, `tc-badid/`, `tc-fail/`, `tc-escape/`. Also create an empty source
   directory `tc-escape-src/` for the path-safety case.
4. **Prepare a local architecture fixture folder.**
   `~/scratch/govdoc-tests/fixtures/datahub-docs/` containing:
   - `README.md` with a short overview of a fictional "DataHub" system (three components:
     Ingest API, Transform Worker, Query Service; one data store: Postgres).
   - `architecture.md` describing the components, their interfaces, and the data flow.
   - `nested/interfaces.md` describing the Ingest API and Query Service contracts.
   - `diagrams/notes.txt` (an unsupported plain-text note, to exercise format filtering).
5. **Prepare a single-file fixture.** `~/scratch/govdoc-tests/fixtures/legacy-crm-sad.pdf`
   - a short PDF system architecture description (or use any existing architecture PDF
     from `assets/pdf/`).
6. **Prepare a broken reference.** A path that does not exist, e.g.
   `~/scratch/govdoc-tests/fixtures/missing-sad.pdf`.
7. **Prepare a busy target.** `~/scratch/govdoc-tests/tc-busy/` containing one stray file
   `keepme.txt`.
8. **For URL cases:** a crawlable documentation site reachable from your network. Any
   small public docs site with two to five linked pages works; note its base URL (for
   example `https://example.com/architecture/`). If you are offline, skip TC-002, TC-005,
   TC-009, TC-010 and TC-012 and record them as not run.
9. **For hosting cases:** valid GitHub credentials (and separately GitLab) previously used
   successfully in a ragent session.
10. **Preserve config.** Back up `.ragent/ragent.json` before starting so any temporary
    flags can be reverted in Cleanup.

## Test Cases

### TC-001 - Usage help with no arguments

**Requirement:** FR-001, FR-002, FR-003, NFR-001, NFR-005

**Preconditions:** ragent running in the TUI from `~/scratch/govdoc-tests/`; scratch root
empty of test projects except the fixtures folder.

**Steps:**
1. Type `/spec ` into the message input and observe the autocomplete menu.
2. Confirm `govcreate` appears in the list; press `Esc` to dismiss the menu.
3. Type `/spec govcreate` and press `Enter`.
4. Read the usage text in the message window.
5. Repeat with `/spec govcreate help` and confirm the same message is shown.

**Test data:** `/spec govcreate` and `/spec govcreate help` (no further arguments).

**Expected results:**
- `govcreate` is listed in the `/spec` autocomplete menu.
- The usage block shows the argument order `[specid] <content-ref> <target-folder>`, both
  content-reference forms (URL and local file/folder), and the accepted `/new` flags
  (`--language`, `--type`, `--stack`, `--github`, `--gitlab`) with their values.
- No files or directories are created under the scratch root (verify with `ls -la`).
- Output contains no non-ASCII characters and no `[err]` marker.

### TC-002 - URL reference, Rust cmdline scaffold, no remote

**Requirement:** FR-001, FR-004, FR-007, FR-008, FR-009, FR-010, FR-012, FR-015, FR-016, FR-018

**Preconditions:** An empty target folder `~/scratch/govdoc-tests/tc-url-rust/`; a
reachable crawlable docs URL; LLM provider configured.

**Steps:**
1. In the TUI, type:
   `/spec govcreate payments-arch https://example.com/architecture/ ./tc-url-rust --language rust --type cmdline --stack axum`
2. Press `Enter`.
3. Approve any permission prompts shown (press the allow key displayed).
4. Watch the message window until the run reports completion.
5. In a second terminal, run `ls -R ~/scratch/govdoc-tests/tc-url-rust`.
6. Open `~/scratch/govdoc-tests/tc-url-rust/specs/payments-arch/SPEC.md`,
   `PLAN.md`, and `TESTPLAN.md`.

**Test data:**
- `specid` = `payments-arch`
- `content-ref` = the docs URL
- `target-folder` = `./tc-url-rust`
- flags = `--language rust --type cmdline --stack axum`

**Expected results:**
- The progress message updates in place with one line per stage: guard, scaffold,
  acquisition (naming the number of pages gathered), extraction, authoring, completion.
- Only one progress message is used (it is edited, not appended-to-per-stage).
- The scaffold creates a Rust cmdline project (Cargo layout) with the `axum` stack
  overlay applied, `specs/`, `log/`, `.ragent/`, `AGENTS.md`, and a git repository.
- `specs/payments-arch/` contains `SPEC.md`, `PLAN.md`, and `TESTPLAN.md`.
- `SPEC.md` starts with `status: draft` YAML frontmatter and contains an
  `## Requirements` section with FR-001, FR-002, ... requirements using at least one
  ubiquitous, event-driven, state-driven, optional, and unwanted EARS template.
- `PLAN.md` has a `## Tasks` table with columns ID, Title, Requirement, Effort, Priority,
  Status, Dependencies and T-001... rows with `Pending` status.
- `TESTPLAN.md` has a `## Test Cases` section with TC-001... manual cases and no
  automated test code.
- The invocation (spec ID, URL, target folder, and the `/new` flags) is recorded in the
  `SPEC.md` frontmatter.
- The acquisition stage reports that the pages available were fewer than the page cap,
  without failing.

### TC-003 - Local folder reference, Python library scaffold

**Requirement:** FR-005, FR-007, FR-008, FR-009, FR-012, FR-015, NFR-004

**Preconditions:** An empty target folder `~/scratch/govdoc-tests/tc-local-python/`; the
`fixtures/datahub-docs/` folder prepared per Prerequisites item 4.

**Steps:**
1. In the TUI, type:
   `/spec govcreate datahub-arch ./fixtures/datahub-docs ./tc-local-python --language python --type library`
2. Press `Enter`; approve permission prompts.
3. Wait for completion.
4. Run `ls -R ~/scratch/govdoc-tests/tc-local-python`.
5. Open `~/scratch/govdoc-tests/tc-local-python/specs/datahub-arch/SPEC.md`.
6. Read the `## Background` / overview text and the requirement list.

**Test data:**
- `specid` = `datahub-arch`
- `content-ref` = `./fixtures/datahub-docs`
- `target-folder` = `./tc-local-python`
- flags = `--language python --type library`

**Expected results:**
- The acquisition progress line names the number of files gathered and reports that the
  nested subdirectory (`nested/interfaces.md`) was included and the unsupported
  `diagrams/notes.txt` was filtered.
- Acquisition completes within 30 seconds for this small fixture.
- The scaffold creates a Python library layout with `pyproject.toml` (or equivalent) and
  the standard ragent workspace files.
- `specs/datahub-arch/SPEC.md` describes at least the three components (Ingest API,
  Transform Worker, Query Service) and the Postgres data store, i.e. the extracted
  structure is reflected in the generated requirements.

### TC-004 - Single-file PDF reference

**Requirement:** FR-005, FR-007, FR-008

**Preconditions:** An empty target folder `~/scratch/govdoc-tests/tc-pdf/`; the PDF
fixture from Prerequisites item 5.

**Steps:**
1. In the TUI, type:
   `/spec govcreate legacy-crm-arch ./fixtures/legacy-crm-sad.pdf ./tc-pdf --language python --type library`
2. Press `Enter`; approve permission prompts; wait for completion.
3. Open `~/scratch/govdoc-tests/tc-pdf/specs/legacy-crm-arch/SPEC.md`.

**Test data:**
- `specid` = `legacy-crm-arch`
- `content-ref` = `./fixtures/legacy-crm-sad.pdf`
- `target-folder` = `./tc-pdf`

**Expected results:**
- The acquisition stage reads the PDF and reports one file gathered.
- A spec is generated whose overview reflects the PDF's system description.
- If the PDF yields no extractable text (image-only scan), the run is refused with a
  clear "no usable text" cause per TC-008, and no spec is written.

### TC-005 - `--github` hosting on a URL-referenced scaffold

**Requirement:** FR-010

**Preconditions:** Valid GitHub credentials; an empty target folder
`~/scratch/govdoc-tests/tc-github/`; a reachable docs URL.

**Steps:**
1. In the TUI, type:
   `/spec govcreate edge-arch https://example.com/architecture/ ./tc-github --language rust --type cmdline --github`
2. Press `Enter`; approve the hosting/permission prompts.
3. Wait for completion.
4. Run `git -C ~/scratch/govdoc-tests/tc-github remote -v`.
5. Check your GitHub account for the newly created private repository.

**Test data:** `--github` supplied; all other arguments as shown.

**Expected results:**
- The scaffold initialises git, creates the private GitHub repository, sets it as
  `origin`, and pushes the initial commit (identical behaviour to `/new --github`).
- The spec is generated into the scaffolded project.
- If the hosting step fails, the scaffolded project is preserved and the failure is
  reported with its cause (see TC-010).

### TC-006 - Conflicting hosting flags

**Requirement:** FR-002

**Preconditions:** ragent running from the scratch root.

**Steps:**
1. Type:
   `/spec govcreate clash ./fixtures/datahub-docs ./tc-clash --language rust --type cmdline --github --gitlab`
2. Press `Enter`.

**Test data:** both `--github` and `--gitlab` supplied.

**Expected results:**
- A usage error is shown naming the hosting conflict (same wording as `/new`).
- No directory `tc-clash/` is created and no spec is written.

### TC-007 - Non-empty target folder refusal

**Requirement:** FR-011

**Preconditions:** `~/scratch/govdoc-tests/tc-busy/` exists and contains `keepme.txt`.

**Steps:**
1. In the TUI, type:
   `/spec govcreate busy-arch ./fixtures/datahub-docs ./tc-busy --language rust --type cmdline`
2. Press `Enter`.
3. Run `ls -la ~/scratch/govdoc-tests/tc-busy`.

**Test data:** target folder `./tc-busy` containing `keepme.txt`.

**Expected results:**
- The run is refused before any acquisition; the message names `keepme.txt` as the
  blocking entry.
- `tc-busy/` still contains only `keepme.txt` - no `specs/`, no scaffold files, no spec.
- No progress lines beyond the guard stage are emitted.

### TC-008 - Unreadable / missing content reference

**Requirement:** FR-006, FR-014

**Preconditions:** An empty target folder `~/scratch/govdoc-tests/tc-badref/`; the
missing path from Prerequisites item 6.

**Steps:**
1. In the TUI, type:
   `/spec govcreate badref-arch ./fixtures/missing-sad.pdf ./tc-badref --language rust --type cmdline`
2. Press `Enter`.
3. Run `ls -la ~/scratch/govdoc-tests/tc-badref`.

**Test data:** `content-ref` = a path that does not exist.

**Expected results:**
- The run fails with a specific cause ("path not found"), not a generic error, and names
  the offending reference.
- The run terminates before scaffolding: `tc-badref/` is left unchanged and contains no
  scaffold files and no `specs/` directory.
- The terminal summary carries the `[err]` marker and states that no spec was generated.

### TC-009 - `--force` re-run overwriting the spec

**Requirement:** FR-017

**Preconditions:** Completed TC-002, so
`~/scratch/govdoc-tests/tc-url-rust/specs/payments-arch/` exists.

**Steps:**
1. Note the current modification time of
   `~/scratch/govdoc-tests/tc-url-rust/specs/payments-arch/SPEC.md`
   (run `stat` on it).
2. In the TUI, from `~/scratch/govdoc-tests/tc-url-rust/`, type:
   `/spec govcreate payments-arch https://example.com/architecture/ . --language rust --type cmdline`
3. Press `Enter` and observe the refusal.
4. Re-run with `--force`:
   `/spec govcreate payments-arch https://example.com/architecture/ . --language rust --type cmdline --force`
5. Press `Enter`; approve permissions; wait for completion.
6. Re-check the `stat` modification time of `SPEC.md`.

**Test data:** same spec ID and URL, second run with `--force`.

**Expected results:**
- Step 3 refuses and names the existing spec (no overwrite without `--force`).
- Step 3's refusal is scoped to the existing spec: the run does not re-scaffold an
  already-populated project.
- Step 5 regenerates the spec files under `--force`; the modification time in step 6 is
  later than in step 1.
- No unrelated project files are modified by the `--force` re-run.

### TC-010 - Post-scaffold failure containment

**Requirement:** FR-013, FR-019

**Preconditions:** An empty target folder `~/scratch/govdoc-tests/tc-fail/`; a reachable
docs URL; the ability to cancel a run (the TUI cancel key, typically `Esc`).

**Steps:**
1. In the TUI, type:
   `/spec govcreate fail-arch https://example.com/architecture/ ./tc-fail --language rust --type cmdline`
2. Press `Enter`; approve the permission prompt.
3. As soon as the progress line reaches the acquisition stage, press the cancel key.
4. Run `ls -R ~/scratch/govdoc-tests/tc-fail`.
5. Read the terminal report in the message window.

**Test data:** cancellation during the acquisition stage.

**Expected results:**
- The run stops at the next stage boundary (no spec authoring after the cancel point).
- The scaffolded project is preserved.
- The report names the completed stages (guard, scaffold) and states that no spec was
  generated.
- `tc-fail/specs/fail-arch/` either does not exist or contains no spec files.

### TC-011 - CLI parity

**Requirement:** FR-020

**Preconditions:** A terminal at the scratch root; an empty target folder
`~/scratch/govdoc-tests/tc-cli/`; the local fixture folder.

**Steps:**
1. Run:
   `./target/debug/ragent spec govcreate cli-arch ./fixtures/datahub-docs ./tc-cli --language python --type library`
2. Observe the streamed progress on stdout.
3. Run `ls ~/scratch/govdoc-tests/tc-cli/specs/cli-arch`.
4. Run the same CLI invocation with a nonexistent content reference and check the exit
   code with `echo $?`.

**Test data:** the arguments shown; second run uses `./fixtures/missing-sad.pdf`.

**Expected results:**
- The CLI performs the same run as the TUI: it scaffolds the project and writes the three
  spec files.
- Progress and the summary are printed to stdout.
- The bad-reference run exits with a non-zero code and prints a specific cause.

### TC-012 - URL acquisition budget reporting

**Requirement:** FR-016

**Preconditions:** A docs URL with more than two linked pages; an empty target folder
`~/scratch/govdoc-tests/tc-budget/`.

**Steps:**
1. In the TUI, type:
   `/spec govcreate budget-arch https://example.com/architecture/ ./tc-budget --language rust --type cmdline`
   (choose a site deep enough to exceed a low page cap if the implementation exposes the
   caps).
2. Press `Enter`; wait for completion.
3. Read the acquisition progress line for the page count and any budget-reached notice.

**Test data:** the docs URL and target `./tc-budget`.

**Expected results:**
- The acquisition line reports the number of pages actually gathered.
- If a page/depth/character cap was reached, the message says so explicitly, and the run
  still completes (does not fail).

### TC-013 - Path-escape safety refusal

**Requirement:** NFR-003

**Preconditions:** ragent running from `~/scratch/govdoc-tests/tc-escape-src/` (create it
empty); a target folder `~/scratch/govdoc-tests/tc-escape/` (create it empty); the
fixtures folder at `~/scratch/govdoc-tests/fixtures/`.

**Steps:**
1. In the TUI, run ragent from `~/scratch/govdoc-tests/tc-escape-src/`.
2. Type:
   `/spec govcreate escape-arch ../fixtures/datahub-docs ../tc-escape --language rust --type cmdline`
3. Press `Enter`.

**Test data:** a content reference that escapes the invoking directory (`../fixtures/...`).

**Expected results:**
- The run is refused with a path-escape cause; no acquisition occurs.
- No spec is written and no scaffold is performed.

### TC-014 - Invalid spec ID rejection

**Requirement:** FR-002

**Preconditions:** ragent running from the scratch root; an empty target folder
`~/scratch/govdoc-tests/tc-badid/`; the local fixture folder.

**Steps:**
1. In the TUI, type:
   `/spec govcreate ../escape ./fixtures/datahub-docs ./tc-badid --language rust --type cmdline`
2. Press `Enter`.
3. Run `ls -la ~/scratch/govdoc-tests/tc-badid` and confirm no directory was created
   outside `specs/`.

**Test data:** `specid` = `../escape` (contains path-traversal characters).

**Expected results:**
- The run is refused with a usage error stating the spec ID is invalid (same wording as
  the existing spec-ID validation).
- No directory traversal occurs and no files are written outside the target folder.
- No scaffold is performed.

## Cleanup

1. Delete the scratch tree:
   `rm -rf ~/scratch/govdoc-tests/` (this removes all fixtures and generated projects).
2. If any GitHub or GitLab test repositories were created (TC-005), delete them from the
   hosting account and remove the associated local remote (the local project is being
   deleted anyway).
3. Restore `.ragent/ragent.json` from the backup taken in Prerequisites item 10.
4. Stop any ragent processes started for the tests and clear the TUI history if
   desired.
