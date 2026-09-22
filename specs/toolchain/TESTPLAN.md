---
status: draft
---

# Manual Test Plan — /toolchain Runtime Toolchain Report

Spec: `specs/toolchain/SPEC.md`

## Prerequisites

1. A working build of `ragent` with the `/toolchain` command implemented:
   `cargo build` succeeds and the binary runs.
2. A Linux workstation terminal that supports the ragent TUI (alternate
   screen). The test machine should ideally have at least two of the listed
   toolchains installed (e.g. `rust`/`cargo` and `python3`) and at least one
   application toolchain deliberately absent (e.g. `zig`, `dart`) so both
   report states are exercised.
3. To create an "absent toolchain" condition without uninstalling anything,
   verify the tool is simply not present (`command -v dart` returns nothing)
   rather than masking it; do not uninstall system packages for this test.
4. No special provider/API-key configuration is needed — `/toolchain` never
   calls an LLM.
5. A spare terminal (outside the TUI) for cross-checking commands like
   `command -v cargo` and `rustc --version` during the test.

## Test Cases

### TC-001 — `/toolchain` with no subcommand shows help

**Preconditions:**
- ragent TUI is open with an active session.

**Steps:**
1. Type `/toolchain` in the input line (no trailing subcommand).
2. Press `Enter`.

**Expected results:**
- An assistant message appears prefixed `From: /toolchain help`.
- The message contains a table listing the subcommands `help` and `list`.
- The status bar reads `toolchain: help`.
- No probe output and no error text appear.

---

### TC-002 — `/toolchain help` shows the help page

**Preconditions:**
- ragent TUI is open.

**Steps:**
1. Type `/toolchain help`.
2. Press `Enter`.

**Expected results:**
- Identical output to TC-001.
- The help page documents the `list` argument forms (`[language]`, `--json`)
  and states the report is read-only.
- The help page contains at least one worked example (e.g.
  `/toolchain list rust`).

---

### TC-003 — `/toolchain list` renders the full report

**Preconditions:**
- ragent TUI is open.
- At least one application toolchain is installed (e.g. `cargo`) and at
  least one is absent (e.g. `dart`).

**Steps:**
1. Type `/toolchain list`.
2. Press `Enter`.
3. While the command runs, observe the status bar.
4. When the report appears, scroll through the whole table.

**Expected results:**
- The status bar shows a wait indicator (`[wait] toolchain`) during probing,
  then `toolchain: list` when done.
- A markdown table appears prefixed `From: /toolchain list` with columns
  **Language**, **Runtime**, **Status**, **Version**.
- All 50 `SUPPORTED_LANGUAGES` ids appear, in scanner-list order:
  application languages with presence/version columns, and the 20 data/format
  entries (`toml`, `yaml`, `json`, `xml`, `html`, `css`, `scss`, `sql`,
  `markdown`, `protobuf`, `verilog`, `vhdl`, `terraform`, `openscad`,
  `cmake`, `gradle`, `gradle_kts`, `maven`, `nix`, `hcl`) marked
  `(data format — runtime n/a)`.
- Installed toolchains show `installed` with a plausible version string
  (e.g. `rust` → `cargo` `installed` with a `cargo 1.x` line).
- Absent toolchains show `not installed` with a `-` version placeholder.
- A summary line reports `<n>/<m>` application runtimes installed.
- The TUI stayed responsive during the run (input remained editable or
  interruptible; no long freeze).

---

### TC-004 — Report matches manual spot-checks

**Preconditions:**
- TC-003 completed and its output visible.
- A spare terminal is available.

**Steps:**
1. In the spare terminal, run `command -v cargo && rustc --version`.
2. Run `command -v python3 && python3 --version`.
3. Run `command -v dart` (expected: no output, exit 1).
4. Compare against the `/toolchain list` rows for `rust`, `python`, `dart`.

**Expected results:**
- `rust` row status/version consistent with step 1 (presence agrees; version
  string contains the rustc/cargo version components).
- `python` row status/version consistent with step 2.
- `dart` row reads `not installed` with `-` version.

---

### TC-005 — `/toolchain list <lang>` filters to one language

**Preconditions:**
- ragent TUI is open; `cargo` is installed.

**Steps:**
1. Type `/toolchain list rust`.
2. Press `Enter`.

**Expected results:**
- The report contains only the `rust` row (`cargo`, `rustc` runtimes with
  per-command status and versions).
- No other language rows appear.
- Status bar reads `toolchain: list`.

---

### TC-006 — Language filter is case-insensitive

**Preconditions:**
- ragent TUI is open.

**Steps:**
1. Type `/toolchain list RUST`.
2. Press `Enter`.

**Expected results:**
- Same output as TC-005 (case-insensitive match against the language id).
- No unknown-language warning.

---

### TC-007 — Unknown language id warns and lists valid ids

**Preconditions:**
- ragent TUI is open.

**Steps:**
1. Type `/toolchain list nosuchlang`.
2. Press `Enter`.

**Expected results:**
- A warning message names `nosuchlang` as unknown.
- The message lists the valid language ids (the `SUPPORTED_LANGUAGES` set).
- No report table is rendered.
- The TUI does not crash and the session remains usable.

---

### TC-008 — Unknown subcommand shows usage correction

**Preconditions:**
- ragent TUI is open.

**Steps:**
1. Type `/toolchain frobnicate`.
2. Press `Enter`.

**Expected results:**
- A message prefixed `From: /toolchain` shows the usage line and points to
  `/toolchain help`.
- Status bar reads `toolchain: usage`.
- No probe table and no runtime probing occurs (output renders instantly).

---

### TC-009 — `--json` output is valid JSON

**Preconditions:**
- ragent TUI is open.

**Steps:**
1. Type `/toolchain list --json`.
2. Press `Enter`.
3. Copy the JSON document from the message window into a file (e.g.
   `target/temp/tc9.json`) and validate it with `python3 -m json.tool
   target/temp/tc9.json` in the spare terminal.

**Expected results:**
- The message window shows a JSON document (not the markdown table) with:
  - a `languages` array containing one object per application language,
    each with `id`, `runtime`, `status`, and `version` fields;
  - `installed` and `total` summary numbers where `total` equals the count
    of application-language objects.
- `python3 -m json.tool` parses the file without error.

---

### TC-010 — `--json` with language filter

**Preconditions:**
- ragent TUI is open.

**Steps:**
1. Type `/toolchain list rust --json`.
2. Press `Enter`.

**Expected results:**
- A JSON document whose `languages` array contains exactly one object
  (`id` = `rust`), and `total` = 1.

---

### TC-011 — Missing runtime does not abort the report

**Preconditions:**
- ragent TUI is open.
- At least three application toolchains are absent from `PATH` (e.g.
  `kotlinc`, `zig`, `swift` — verify with `command -v` first).

**Steps:**
1. Type `/toolchain list`.
2. Press `Enter`.

**Expected results:**
- The report completes with all 50 rows.
- The absent toolchains' rows read `not installed` / `-`.
- The rows after the absent entries (e.g. `ruby`, `swift` order) still show
  correct statuses — the report was not truncated at the first missing tool.
- The command completes well within ~10 seconds (NFR-001).

---

### TC-012 — Version-probe failure is contained

**Preconditions:**
- ragent TUI is open.
- A runtime is installed whose version probe is expected to be awkward
  (e.g. `java`, which prints its version on stderr via `-version`), OR the
  tool reports a non-zero exit for `--version`.

**Steps:**
1. Type `/toolchain list`.
2. Press `Enter`.
3. Inspect the row for the awkward runtime (e.g. `java`).

**Expected results:**
- The row shows `installed` (the binary exists on `PATH`).
- The version column shows either a captured version string or the
  `unknown`/`timeout` placeholder — never a crash, and the rest of the
  table is unaffected.
- If a tool were to hang, the per-probe timeout would mark it `timeout`
  after ~2 seconds and the report would still finish (spot-check timing
  only if a hanging tool is deliberately present).

---

### TC-013 — Read-only guarantee

**Preconditions:**
- ragent TUI is open.
- Note the output of `git status --porcelain` and `ls -la .ragent/` in the
  spare terminal before the test.

**Steps:**
1. Run `/toolchain list`, `/toolchain list rust`, and
   `/toolchain list --json` in sequence.
2. In the spare terminal, run `git status --porcelain` and
   `ls -la .ragent/` again and diff against the earlier capture.

**Expected results:**
- Identical git status and `.ragent/` contents before and after — no config
  files, index files, or working-tree files were created, modified, or
  deleted by `/toolchain`.

---

### TC-014 — Autocomplete suggests subcommands

**Preconditions:**
- ragent TUI is open; the slash menu appears when typing `/`.

**Steps:**
1. Clear the input line.
2. Type `/tool`.
3. Observe the slash-command autocomplete menu.
4. Select `toolchain` (arrow keys + `Enter`, or click if mouse is enabled).
5. With the input now showing `/toolchain `, press `Tab` or continue typing
   to observe subcommand suggestions.

**Expected results:**
- The autocomplete menu offers `toolchain` among the matches for `/tool`.
- After selecting it, subcommand suggestions include `list` and `help`.

---

### TC-015 — `/toolchain help` lists `/toolchain` in the help index

**Preconditions:**
- ragent TUI is open.

**Steps:**
1. Type `/help`.
2. Press `Enter`.
3. Inspect the command index for a `toolchain` row.

**Expected results:**
- The `/help` command index includes a `toolchain` entry describing the
  runtime toolchain report.

## Cleanup

1. Delete any temporary capture files created during TC-009
   (`target/temp/tc9.json`) — `target/temp/` is gitignored, so removal is
   optional but tidy.
2. No ragent state (sessions, config, spec data, code index) is modified by
   this test plan; no further teardown is required.
3. Close the spare terminal used for cross-checking.