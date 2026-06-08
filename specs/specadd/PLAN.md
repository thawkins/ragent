# specadd — Implementation Plan

## Architecture

The `add` subcommand extends the existing `/spec` command system with three
concerns:

1. **Command parsing** — Add a `SpecCommand::Add` variant to the existing
   `SpecCommand` enum and extend the `parse()` method.
2. **Incremental content insertion** — Pure functions that parse the existing
   `SPEC.md` and `PLAN.md` to find insertion points, then splice in new
   requirement blocks and task rows without disturbing existing content.
3. **LLM prompt + response parsing** — A new prompt template that instructs
   the LLM to generate only the incremental additions, and a parser that
   extracts structured requirement blocks and task rows from the LLM output.

```
/spec add <id> <feature>
        │
        ▼
┌──────────────────────┐
│ SpecCommand::Add     │  ← new enum variant
│ parse("add Id feat") │
└──────┬───────────────┘
       │
       ▼
┌──────────────────────┐
│ SpecManager::read    │  ← load existing spec + plan
└──────┬───────────────┘
       │
       ▼
┌──────────────────────┐
│ Guard checks         │  ← spec exists? not archived?
└──────┬───────────────┘
       │
       ▼
┌──────────────────────┐
│ Build LLM prompt     │  ← existing content + feature + instructions
└──────┬───────────────┘
       │
       ▼
┌──────────────────────┐
│ Process via LLM      │  ← agent generates incremental content
└──────┬───────────────┘
       │
       ▼
┌───────────────────────────────────┐
│ Parse LLM output                  │
│  → new requirement blocks         │
│  → new task rows                  │
│  → new task detail sections       │
└──────┬────────────────────────────┘
       │
       ▼
┌───────────────────────────────────┐
│ Insert into existing documents    │
│  → spec_inserter::insert_reqs()   │
│  → plan_inserter::insert_tasks()  │
└──────┬────────────────────────────┘
       │
       ▼
┌───────────────────────────────────┐
│ Atomic write (SPEC.md, PLAN.md)   │
└──────┬────────────────────────────┘
       │
       ▼
┌───────────────────────────────────┐
│ Validate + display summary        │
└─��─────────────────────────────────┘
```

### Crate Location

All changes are within the existing `ragent-specs` crate and the TUI handler
in `ragent-tui`. No new crates are needed.

```
crates/ragent-specs/src/
├── commands.rs         # +SpecCommand::Add variant, parse rule, help text, prompt builder
├── spec_inserter.rs    # NEW — incremental SPEC.md insertion logic
├── plan_inserter.rs    # NEW — incremental PLAN.md insertion logic
├── id_scanner.rs       # NEW — find highest FR-NNN / NFR-NNN / T-NNN IDs
├── manager.rs          # +add_requirements() method on SpecManager
└── io.rs               # unchanged (existing atomic_write handles writes)

crates/ragent-tui/src/app.rs  # +SpecCommand::Add match arm in execute_slash_command_inner
```

### Key Design Decisions

1. **Pure insertion functions** — `spec_inserter` and `plan_inserter` are
   pure functions that take the existing markdown string and a list of
   structured insertions, and return a new markdown string. No I/O, no
   side effects. This makes them trivially testable.

2. **LLM generates structured blocks** — The prompt instructs the LLM to
   output new requirements and tasks in a specific delimited format so the
   parser can extract them reliably, rather than trying to parse free-form
   markdown.

3. **Atomic write** — Write the updated SPEC.md first; if it succeeds, write
   PLAN.md. If PLAN.md fails, roll back SPEC.md to the original content
   (FR-026).

4. **ID scanning** — A simple regex scan of the raw markdown strings
   (`FR-\d+`, `NFR-\d+`, `T-\d+`) to find the highest numeric ID. No
   dependency on the parsed `Spec` struct — we scan the raw text so we
   catch IDs in comments, detail sections, and anywhere else they appear.

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Add SpecCommand::Add variant and parse rule | FR-001, FR-002, FR-003 | S | Critical | completed | — |
| T-002 | Implement id_scanner - find highest FR/NFR/T IDs | FR-007, NFR-002 | S | Critical | completed | — |
| T-003 | Implement spec_inserter::insert_requirements() | FR-010, FR-011, FR-012, FR-013, FR-014 | M | Critical | pending | T-002 |
| T-004 | Implement `plan_inserter::insert_tasks()` | FR-015, FR-016, FR-017, FR-018, FR-019, FR-020, FR-021, FR-022, FR-023 | M | Critical | pending | T-002 |
| T-005 | Build LLM prompt template for incremental add | FR-030, FR-031 | M | Critical | completed | — |
| T-006 | Implement LLM output parser (extract reqs + tasks) | FR-032, FR-033 | M | Critical | pending | T-005 |
| T-007 | Implement `SpecManager::add_requirements()` method | FR-004, FR-005, FR-006, FR-026, FR-027, FR-038 | M | Critical | pending | T-003, T-004 |
| T-008 | Implement `/spec add` TUI handler | FR-035, FR-036 | M | Critical | pending | T-007, T-006 |
| T-009 | Implement validation-after-update | FR-028, FR-029 | S | High | pending | T-007 |
| T-010 | Update `/spec help` message with `add` command | FR-034 | S | High | pending | T-001 |
| T-011 | Add `add` to TUI autocomplete suggestions | FR-037 | S | High | pending | T-001 |
| T-012 | Implement estimated-effort summary update | FR-024 | S | Low | pending | T-004 |
| T-013 | Implement risk table row insertion | FR-025 | S | Low | pending | T-004 |
| T-014 | Write unit tests for `id_scanner` | NFR-002 | S | Critical | pending | T-002 |
| T-015 | Write unit tests for `spec_inserter` | FR-010, FR-011, FR-012, FR-013, FR-014, NFR-003 | M | Critical | pending | T-003 |
| T-016 | Write unit tests for `plan_inserter` | FR-015, FR-016, FR-017, FR-018, FR-019, FR-020, FR-021, FR-022, FR-023 | M | Critical | pending | T-004 |
| T-017 | Write unit tests for LLM output parser | FR-032, FR-033 | M | Critical | pending | T-006 |
| T-018 | Write integration test (end-to-end add) | NFR-001, NFR-004 | M | High | pending | T-008 |
| T-019 | Update SPEC.md documentation | — | S | Low | pending | T-008 |
## Task Details

### T-001 — Add `SpecCommand::Add` Variant (S, Critical)

Extend `SpecCommand` enum in `commands.rs`:

```rust
/// Incrementally add requirements to an existing spec and update its plan.
Add {
    /// Spec identifier.
    spec_id: String,
    /// Free-text feature description for the new requirements.
    feature: String,
},
```

Add `"add"` arm to `SpecCommand::parse()`:
- Split `rest` into `specname` + `feature`
- If either is empty, return `Self::Unknown("add".to_string())`

Add `"add"` to the `is_usage_error()` match.

### T-002 — ID Scanner (S, Critical)

Create `crates/ragent-specs/src/id_scanner.rs`:

```rust
/// Find the highest numeric ID for a given prefix (e.g. "FR", "NFR", "T").
pub fn highest_id(markdown: &str, prefix: &str) -> Option<u32>

/// Find the highest FR-NNN ID in a spec markdown string.
pub fn highest_fr(spec_md: &str) -> u32

/// Find the highest NFR-NNN ID in a spec markdown string.
pub fn highest_nfr(spec_md: &str) -> u32

/// Find the highest T-NNN ID in a plan markdown string.
pub fn highest_task(plan_md: &str) -> u32
```

Implementation: regex scan for `{prefix}-(\d+)`, collect all numeric
captures, return the maximum. Handles zero-padded IDs (`FR-001` → 1) and
gaps (`FR-001, FR-007` → 7) correctly (NFR-002).

### T-003 — Spec Inserter (M, Critical)

Create `crates/ragent-specs/src/spec_inserter.rs`:

```rust
/// A new requirement block to insert into SPEC.md.
pub struct RequirementInsertion {
    /// Section heading to insert under (existing or new).
    pub section: String,
    /// Whether this section already exists in the spec.
    pub section_exists: bool,
    /// Requirement ID (e.g. "FR-013").
    pub id: String,
    /// Full requirement block text (heading + body).
    pub block: String,
}

/// Insert new requirement blocks into the existing SPEC.md content.
///
/// - If the section exists, appends blocks under it.
/// - If the section is new, creates it before NFR section or at end.
/// - Preserves frontmatter, title, overview, and all existing content.
///
/// Returns the updated SPEC.md string.
pub fn insert_requirements(
    spec_md: &str,
    insertions: &[RequirementInsertion],
    next_fr: u32,
    next_nfr: u32,
) -> String
```

Key rules:
- Never modify existing lines (FR-014, FR-010)
- Preserve frontmatter block (NFR-003)
- Find section heading by exact match (case-insensitive)
- New sections inserted before `## Non-Functional Requirements` or at end
- Requirement IDs are renumbered starting from `next_fr`/`next_nfr`

### T-004 — Plan Inserter (M, Critical)

Create `crates/ragent-specs/src/plan_inserter.rs`:

```rust
/// A new task row to insert into PLAN.md.
pub struct TaskInsertion {
    /// Task ID (e.g. "T-008").
    pub id: String,
    /// Task title.
    pub title: String,
    /// Linked requirement IDs.
    pub requirements: Vec<String>,
    /// Effort (S, M, L).
    pub effort: String,
    /// Priority (Critical, High, Medium, Low).
    pub priority: String,
    /// Dependency task IDs.
    pub dependencies: Vec<String>,
    /// Optional task detail subsection text.
    pub detail: Option<String>,
}

/// Insert new task rows and optional detail subsections into PLAN.md.
///
/// - Appends new rows to the `## Tasks` table after the last existing row.
/// - Appends new detail subsections after the last existing detail section.
/// - Preserves all existing content (FR-020, FR-023).
///
/// Returns the updated PLAN.md string.
pub fn insert_tasks(plan_md: &str, insertions: &[TaskInsertion]) -> String
```

Key rules:
- Locate `## Tasks` table by heading, find last data row (line starting
  with `| T-`)
- Append new rows after last row, before next `##` heading or end of section
- Locate `## Task Details` section, find last `### T-NNN` subsection
- Append new detail subsections after last existing one
- Never modify existing rows or subsections (FR-020, FR-023)

### T-005 — LLM Prompt Template (M, Critical)

Add to `commands.rs`:

```rust
/// Build the prompt sent to the LLM for incremental spec updates.
pub fn build_add_prompt(
    spec_id: &str,
    feature: &str,
    spec_md: &str,
    plan_md: &str,
    next_fr: u32,
    next_nfr: u32,
    next_task: u32,
) -> String
```

The prompt instructs the LLM to:
1. Read the existing SPEC.md and PLAN.md
2. Generate **only** the new requirement blocks and task rows for the
   given feature
3. Use `FR-{next_fr}` (or `NFR-{next_nfr}`) as the first new requirement ID
4. Use `T-{next_task}` as the first new task ID
5. Output in a structured delimited format:
   - `---NEW REQUIREMENTS---` block with markdown requirement entries
   - `---NEW TASKS---` block with markdown table rows
   - `---NEW TASK DETAILS---` block with optional detail subsections

### T-006 — LLM Output Parser (M, Critical)

Create parsing logic in `commands.rs` (or a small helper module):

```rust
/// Parse the LLM output for an add operation.
///
/// Extracts three delimited sections from the LLM response:
/// - New requirement blocks (between `---NEW REQUIREMENTS---` and next delimiter)
/// - New task table rows (between `---NEW TASKS---` and next delimiter)
/// - New task detail subsections (between `---NEW TASK DETAILS---` and end)
///
/// Returns structured `AddOutput` or an error if the format is unexpected.
pub fn parse_add_output(llm_response: &str) -> Result<AddOutput, SpecError>
```

```rust
pub struct AddOutput {
    /// Parsed requirement insertions (section, id, block text).
    pub requirements: Vec<RequirementInsertion>,
    /// Parsed task insertions (id, title, requirements, effort, priority, deps, detail).
    pub tasks: Vec<TaskInsertion>,
}
```

### T-007 — SpecManager::add_requirements() (M, Critical)

Add method to `SpecManager` in `manager.rs`:

```rust
/// Incrementally add new requirements to an existing spec.
///
/// 1. Load the spec (FR-004).
/// 2. Guard: reject if archived (FR-006).
/// 3. Write updated SPEC.md.
/// 4. Write updated PLAN.md.
/// 5. If PLAN.md write fails, roll back SPEC.md (FR-026).
/// 6. Record audit trail entry (FR-038).
pub async fn add_requirements(
    &self,
    spec: &mut Spec,
    new_spec_md: &str,
    new_plan_md: &str,
) -> Result<(), SpecError>
```

Rollback strategy: keep original `spec_md` and `plan_md` in local variables.
If `write_spec` succeeds for SPEC.md but fails for PLAN.md, re-write the
original SPEC.md. This is safe because `write_spec` writes SPEC.md first,
then PLAN.md, using atomic writes.

### T-008 — TUI Handler (M, Critical)

Add `SpecCommand::Add { spec_id, feature }` match arm in
`execute_slash_command_inner` (app.rs), following the same pattern as
`SpecCommand::Create`:

1. Display status message (FR-035)
2. Build the prompt using `build_add_prompt()` with the loaded spec content
3. Spawn the agent task to process the message
4. When the agent completes, parse the output with `parse_add_output()`
5. Apply `insert_requirements()` and `insert_tasks()` to the existing content
6. Call `SpecManager::add_requirements()` to write the updates
7. Run validation (FR-028)
8. Display the summary (FR-036)

The handler runs in two phases:
- **Phase 1**: Send the LLM prompt (async, spawned task)
- **Phase 2**: After LLM completes, parse output, insert content, write
  files, validate, display summary

### T-009 — Validation After Update (S, High)

After `add_requirements()` writes the updated spec, call
`validate::validate()` on the updated `Spec` and display the report. If
there are errors, show a warning advising the user to fix them before
transitioning status (FR-029).

This reuses the existing validation infrastructure — no new code, just a
call in the TUI handler after the add completes.

### T-010 — Update /spec help (S, High)

Add a row to the help table in `SpecCommand::build_help_message()`:

```
| `/spec add <spec-id> <feature description>` | required `spec-id` + `feature description` | Incrementally add requirements to an existing spec and update its plan. |
```

### T-011 — TUI Autocomplete (S, High)

Update the `SLASH_COMMANDS` entry for `"spec"` trigger in
`crates/ragent-tui/src/app/state.rs` to include `add` in the description:

Change from:
```
"Specification management: /spec create|list|search|validate|status|task|help"
```
To:
```
"Specification management: /spec create|add|list|search|validate|status|task|help"
```

### T-012 — Estimated Effort Summary Update (S, Low)

If the PLAN.md contains an effort summary table (typically under a heading
like `## Estimated Effort` with a table of priority × effort counts), parse
and update the counts to include the new tasks. This is optional because
not all plans include this table.

### T-013 — Risk Table Row Insertion (S, Low)

If the PLAN.md contains a risk table (under `## Risks`), and the LLM output
includes new risk entries, append them to the existing table. This is
optional because risk identification is best-effort.

### T-014 — ID Scanner Tests (S, Critical)

Test `id_scanner` with:
- Standard numbering: FR-001, FR-002, FR-003 → highest = 3
- Non-contiguous: FR-001, FR-007, FR-012 → highest = 12
- Zero-padded vs non-padded: FR-1, FR-01, FR-001 → highest = 1
- Mixed prefixes: FR-003 and NFR-002 in the same file → FR highest = 3, NFR highest = 2
- Empty spec → highest = 0
- Task IDs: T-001 through T-010 → highest = 10

### T-015 — Spec Inserter Tests (M, Critical)

Test `spec_inserter::insert_requirements()` with:
- Insert under existing section heading
- Insert under new section heading (placed before NFR section)
- Insert under new section heading (no NFR section, placed at end)
- Multiple insertions under the same section
- Preserve frontmatter unchanged
- Preserve all existing content unchanged
- IDs are renumbered correctly starting from `next_fr`
- NFR-variant insertions under NFR section

### T-016 — Plan Inserter Tests (M, Critical)

Test `plan_inserter::insert_tasks()` with:
- Insert single task row at end of table
- Insert multiple task rows
- Insert task rows with dependencies on existing tasks
- Insert task detail subsections under `## Task Details`
- Insert tasks when no `## Task Details` section exists
- Preserve existing task rows and detail subsections unchanged
- Preserve content outside the task table (effort tables, risk tables)

### T-017 — LLM Output Parser Tests (M, Critical)

Test `parse_add_output()` with:
- Well-formed output with all three delimited sections
- Output with only requirements (no tasks)
- Output with only tasks (no requirements)
- Output with missing delimiters → error
- Output with malformed task rows → skip with warning
- Output with requirement IDs that don't match expected numbering → renumber

### T-018 — Integration Test (M, High)

End-to-end test:
1. Create a spec with `/spec create`
2. Load the spec
3. Build an `add` prompt with a feature description
4. Manually simulate LLM output (no actual LLM call)
5. Parse the output, insert into the spec, write to disk
6. Re-read the spec from disk
7. Verify new requirements are present with correct IDs
8. Verify existing requirements are unchanged
9. Verify new tasks are present with correct IDs
10. Verify existing tasks are unchanged
11. Verify validation passes

### T-019 — Documentation (S, Low)

Update SPEC.md section on spec management to document the `/spec add`
command. No other documentation files need updating.

## Estimated Effort

| Priority | Tasks | Estimated Total |
|---|---|---|
| Critical | T-001, T-002, T-003, T-004, T-005, T-006, T-007, T-008, T-014, T-015, T-016, T-017 | 4S + 7M ≈ 11 days |
| High | T-009, T-010, T-011, T-018 | 3S + 1M ≈ 2.5 days |
| Low | T-012, T-013, T-019 | 3S ≈ 1.5 days |

**Total estimate: ~15 developer-days**

Critical path: T-001 → T-008 (command is wired in)
Then: T-002 → T-003/T-004 → T-007 → T-008 (insertion logic)
Then: T-005/T-006 → T-008 (LLM prompt + parser)
Then: T-014–T-017 (tests)

## Risks

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| LLM generates full spec instead of incremental additions | Medium | High | Structured delimiters in prompt + parser; FR-033 forbids wholesale replacement; parser extracts only delimited blocks |
| LLM output doesn't follow the delimiter format | Medium | Medium | Parser returns error with helpful message; user can retry or manually edit; fallback: treat entire output as a single requirement block |
| Insertion point detection fails for non-standard markdown | Low | Medium | Section heading matching is case-insensitive and flexible; task table detection matches the existing `PlanParser` logic; tests cover edge cases |
| Requirement ID conflicts if user manually edits spec between scan and write | Low | Low | IDs are scanned from the content at write time; next-available IDs are computed immediately before insertion |
| Rollback fails (FR-026) | Very Low | High | Atomic writes use temp-file-and-rename; worst case: both files have original content (rollback succeeded) or both have new content (inconsistent but recoverable) |